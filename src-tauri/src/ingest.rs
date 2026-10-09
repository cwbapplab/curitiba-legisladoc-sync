//! Deciding what has to be downloaded, and streaming records out of it.
//!
//! The snapshot is republished daily and is ~1.42 GB, while it grows by only a
//! few hundred KB per day. So the normal case is: the file grew at the end, and
//! only the new tail needs fetching. That assumption is verified rather than
//! trusted — the growth pattern is not strictly append-only (observed: 429 of
//! 1,316 days byte-identical, 15 days smaller than the day before), so every
//! delta is validated and falls back to a full re-ingest when it does not hold.

use anyhow::{bail, Context, Result};
use std::io::Read;

use crate::parser::{self, Row};

/// What we already ingested from this snapshot.
#[derive(Debug, Clone, Default)]
pub struct SnapshotState {
    /// Byte offset just past the last fully-ingested record.
    pub offset: u64,
    pub etag: Option<String>,
    /// Highest primary key seen, used to prove an append really appended.
    pub last_id: Option<i64>,
    pub row_count: i64,
}

/// The action to take for a refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Same snapshot, already ingested.
    Unchanged,
    /// Fetch `from..to` and append the records found there.
    Append { from: u64, to: u64 },
    /// Re-read the whole file.
    Full { reason: String },
}

/// The bytes that can end a record: the documented `NUL CR LF`, or the bare
/// `CR LF` that about 49,700 records in the feed actually use.
///
/// The delta handshake looks for these directly rather than going through
/// [`parser::split_records`], because it deals in byte offsets, not records.
const TERM_NUL: &[u8] = b"\x00\r\n";
const TERM_BARE: &[u8] = b"\r\n";

/// Does this record end on a boundary the splitter recognises?
///
/// A bare `CR LF` also needs the next record's first line to follow, which the
/// caller cannot see from the boundary alone — so this test is deliberately
/// permissive. Being permissive is safe here: the prefix check only decides
/// whether a delta may be trusted, and the ingest validates every new id.
pub fn ends_on_boundary(bytes: &[u8]) -> bool {
    bytes.ends_with(TERM_NUL) || bytes.ends_with(TERM_BARE)
}

/// Choose between a tail fetch and a full re-ingest.
///
/// `head_prefix_ok` is the result of re-reading the 3 bytes immediately before
/// the stored offset: if they are still the record terminator, the prefix we
/// already ingested has not been rewritten.
pub fn plan(
    remote_size: u64,
    remote_etag: Option<&str>,
    state: Option<&SnapshotState>,
    head_prefix_ok: bool,
) -> Plan {
    let Some(state) = state else {
        return Plan::Full {
            reason: "no local copy of this snapshot".into(),
        };
    };
    if state.offset == 0 {
        return Plan::Full {
            reason: "local copy has no recorded offset".into(),
        };
    }

    match remote_size.cmp(&state.offset) {
        std::cmp::Ordering::Equal => {
            // Same length. Trust an ETag match; otherwise re-ingest, because the
            // file could have been rewritten in place at the same size.
            match (remote_etag, state.etag.as_deref()) {
                (Some(a), Some(b)) if a == b => Plan::Unchanged,
                (None, _) => Plan::Unchanged,
                _ => Plan::Full {
                    reason: "size matches but the ETag changed".into(),
                },
            }
        }
        std::cmp::Ordering::Less => Plan::Full {
            reason: format!(
                "snapshot shrank: remote {remote_size} B < local offset {} B",
                state.offset
            ),
        },
        std::cmp::Ordering::Greater => {
            if !head_prefix_ok {
                return Plan::Full {
                    reason: "the already-ingested prefix no longer ends on a record boundary".into(),
                };
            }
            Plan::Append {
                from: state.offset,
                to: remote_size,
            }
        }
    }
}

/// Outcome of scanning a stream of records.
#[derive(Debug, Default)]
pub struct ScanStats {
    pub records: u64,
    pub bytes: u64,
    /// Bytes of a trailing partial record that never got its terminator.
    pub leftover: usize,
    pub rows: Vec<Row>,
}

/// Read a stream and hand each complete record (terminator included) to `on_record`.
///
/// Streaming keeps memory bounded regardless of snapshot size: only a chunk plus
/// one in-flight record is ever held. `on_record` receives the record without
/// its terminator, which is what [`parser::parse_record`] expects.
///
/// Boundary detection is delegated to [`parser::split_records`] rather than
/// reimplemented here. It used to look for `NUL CR LF` alone, which is a
/// narrower rule than the feed follows and silently dropped every act that ends
/// with a bare `CR LF` — 49,716 of them. Two copies of a rule that subtle is how
/// they drift apart.
pub fn scan<R: Read>(
    mut reader: R,
    mut on_record: impl FnMut(&[u8]) -> Result<()>,
) -> Result<ScanStats> {
    const CHUNK: usize = 1 << 20; // 1 MiB
    let mut buf: Vec<u8> = Vec::with_capacity(CHUNK * 2);
    let mut stats = ScanStats::default();
    let mut chunk = vec![0u8; CHUNK];

    loop {
        let n = reader.read(&mut chunk).context("reading snapshot stream")?;
        if n == 0 {
            break;
        }
        stats.bytes += n as u64;
        buf.extend_from_slice(&chunk[..n]);

        let (records, rest) = parser::split_records(&buf);
        let consumed = buf.len() - rest.len();
        for record in records {
            if !record.is_empty() {
                on_record(record)?;
                stats.records += 1;
            }
        }
        buf.drain(..consumed);
    }

    stats.leftover = buf.len();
    Ok(stats)
}

/// Scan a delta tail, discarding the leading partial record.
///
/// A range that starts at a record boundary should begin with a digit, but the
/// growth check is a heuristic, not a guarantee, so the first thing up to the
/// first terminator is always dropped. The parsed rows are returned for
/// validation before anything is written.
pub fn scan_tail<R: Read>(reader: R) -> Result<ScanStats> {
    let mut first = true;
    let mut stats = ScanStats::default();
    scan(reader, |record| {
        if first {
            // Whatever precedes the first terminator is the tail of a record we
            // already have, unless the range started exactly on a boundary — in
            // which case `record` is the first byte-complete new record and the
            // caller's ID check will notice if it was dropped.
            first = false;
            if !record.first().is_some_and(|b| b.is_ascii_digit()) {
                return Ok(());
            }
        }
        stats.rows.push(parser::parse_record(record).map_err(|_| {
            anyhow::anyhow!(
                "registro incompleto ao ler a parte nova do arquivo; \
                 o trecho baixado não fecha um ato inteiro"
            )
        })?);
        Ok(())
    })?;
    Ok(stats)
}

/// Verify that an append really appended.
///
/// Every row in a tail scan must be a *new* act with a primary key above the
/// highest one already stored. Anything else means the file was rewritten, not
/// extended, and the caller must fall back to a full re-ingest.
pub fn validate_append(rows: &[Row], last_id: Option<i64>) -> Result<()> {
    let Some(floor) = last_id else {
        return Ok(());
    };
    for r in rows {
        let Some(id) = r.id() else {
            bail!("o ato novo não traz um identificador numérico");
        };
        if id <= floor {
            bail!(
                "o ato novo tem ID {id}, que não é maior que o maior já guardado ({floor}); \
                 o arquivo foi reescrito em vez de estendido"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(offset: u64, etag: Option<&str>, last_id: i64) -> SnapshotState {
        SnapshotState {
            offset,
            etag: etag.map(str::to_string),
            last_id: Some(last_id),
            row_count: 10,
        }
    }

    #[test]
    fn first_run_does_a_full_ingest() {
        assert!(matches!(plan(1000, None, None, true), Plan::Full { .. }));
    }

    #[test]
    fn identical_size_and_etag_is_unchanged() {
        let s = state(1000, Some("\"abc\""), 5);
        assert_eq!(plan(1000, Some("\"abc\""), Some(&s), true), Plan::Unchanged);
    }

    #[test]
    fn same_size_with_a_new_etag_forces_a_full_reingest() {
        // A rewrite that happens to keep the length would be missed by a size
        // check alone, so the ETag has to disagree before we trust it.
        let s = state(1000, Some("\"abc\""), 5);
        assert!(matches!(
            plan(1000, Some("\"def\""), Some(&s), true),
            Plan::Full { .. }
        ));
    }

    #[test]
    fn growth_plans_a_tail_fetch() {
        let s = state(1000, Some("\"abc\""), 5);
        assert_eq!(
            plan(1750, Some("\"def\""), Some(&s), true),
            Plan::Append {
                from: 1000,
                to: 1750
            }
        );
    }

    #[test]
    fn shrinkage_forces_a_full_reingest() {
        // Observed in the real feed: one day's file was 908 MB against 1.42 GB.
        let s = state(1_422_213_148, None, 5);
        match plan(908_320_712, None, Some(&s), true) {
            Plan::Full { reason } => assert!(reason.contains("shrank"), "{reason}"),
            other => panic!("expected a full re-ingest, got {other:?}"),
        }
    }

    #[test]
    fn a_broken_prefix_forces_a_full_reingest() {
        let s = state(1000, None, 5);
        match plan(1750, None, Some(&s), false) {
            Plan::Full { reason } => assert!(reason.contains("prefix"), "{reason}"),
            other => panic!("expected a full re-ingest, got {other:?}"),
        }
    }

    /// Build a stream of well-formed records.
    fn records(start_id: i64, n: i64) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..n {
            let id = start_id + i;
            // 23 metadata fields, then a body with a `;` and a bare LF in it.
            out.extend_from_slice(format!("{id};DOC{id};;;").as_bytes());
            for k in 0..19 {
                out.extend_from_slice(format!("f{k};").as_bytes());
            }
            out.extend_from_slice(b"<p style=\"a;b\">line1\nline2</p>;\x00\r\n");
        }
        out
    }

    #[test]
    fn scans_records_across_chunk_boundaries() {
        let data = records(1, 5000);
        let mut ids = Vec::new();
        let stats = scan(&data[..], |rec| {
            let row = parser::parse_record(rec).map_err(|_| anyhow::anyhow!("bad record"))?;
            ids.push(row.id().unwrap());
            Ok(())
        })
        .unwrap();
        assert_eq!(stats.records, 5000);
        assert_eq!(stats.leftover, 0);
        assert_eq!(ids.len(), 5000);
        assert_eq!(ids[0], 1);
        assert_eq!(ids[4999], 5000);
    }

    #[test]
    fn a_truncated_stream_reports_its_leftover() {
        let mut data = records(1, 10);
        data.truncate(data.len() - 7); // cut the last record short
        let stats = scan(&data[..], |_| Ok(())).unwrap();
        assert_eq!(stats.records, 9);
        assert!(stats.leftover > 0, "the partial record must be reported");
    }

    #[test]
    fn tail_scan_skips_the_leading_partial_record() {
        // Simulate a range starting mid-record: the head of record 3, then
        // complete records 4..8.
        let all = records(1, 8);
        let boundary = records(1, 2).len(); // start of record 3
        let from = boundary + 12; // 12 bytes into record 3

        let stats = scan_tail(&all[from..]).unwrap();
        let ids: Vec<_> = stats.rows.iter().map(|r| r.id().unwrap()).collect();
        // Record 3 is dropped (it was partly before the range); 4..8 are kept.
        assert_eq!(ids, vec![4, 5, 6, 7, 8]);
    }

    #[test]
    fn tail_scan_keeps_all_records_when_the_range_starts_on_a_boundary() {
        let all = records(1, 5);
        let from = records(1, 2).len();
        let stats = scan_tail(&all[from..]).unwrap();
        let ids: Vec<_> = stats.rows.iter().map(|r| r.id().unwrap()).collect();
        assert_eq!(ids, vec![3, 4, 5]);
    }

    #[test]
    fn append_validation_accepts_new_ids_and_rejects_repeats() {
        let rows = |ids: &[i64]| -> Vec<Row> {
            ids.iter()
                .map(|id| {
                    let mut v = vec![None; parser::VALUES_PER_ROW];
                    v[0] = Some(id.to_string());
                    Row {
                        values: v,
                        src_bytes: 0,
                        src_chars: 0,
                    }
                })
                .collect()
        };
        assert!(validate_append(&rows(&[11, 12]), Some(10)).is_ok());
        // An ID at or below the stored maximum means a rewrite, not an append.
        let err = validate_append(&rows(&[11, 10]), Some(10)).unwrap_err();
        assert!(err.to_string().contains("reescrito"), "{err}");
        // No stored maximum yet (first ingest) accepts anything.
        assert!(validate_append(&rows(&[1]), None).is_ok());
    }

    #[test]
    fn append_validation_rejects_a_record_without_a_numeric_key() {
        let row = Row {
            values: vec![Some("not-a-number".into()); parser::VALUES_PER_ROW],
            src_bytes: 0,
            src_chars: 0,
        };
        assert!(validate_append(&[row], Some(1)).is_err());
    }
}
