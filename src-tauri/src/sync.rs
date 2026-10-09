//! Orchestration: turning the published feed into a queryable local database.
//!
//! One refresh does four things, in this order:
//!
//! 1. List the feed over rsync to learn what files exist *today*. The dates are
//!    in the file names, so this is also how the current snapshot is found.
//! 2. Fetch and parse `Info.xml`, which is the only published description of the
//!    CSV layout. The act table is created from it, so a snapshot that adds a
//!    column is stored rather than truncated.
//! 3. Replace the dimension tables wholesale — they are a few KB each, and a
//!    blind replace avoids having to diff them.
//! 4. Ingest the act snapshot, which is the only large file. `HEAD` first, then
//!    decide between "nothing to do", "fetch the new tail" and "re-read it all".
//!
//! Progress is reported through events so a first run (349k acts, ~1.4 GB) can
//! show what it is doing instead of looking hung.

use anyhow::{bail, Context, Result};
use tauri::{AppHandle, Emitter};

use crate::feed::{self, RemoteFile};
use crate::ingest::{self, Plan, SnapshotState};
use crate::parser;
use crate::remote;
use crate::schema::Schema;
use crate::store::Store;

/// The act snapshot, identified from its file name.
pub struct SnapshotFile {
    pub name: String,
    pub size: i64,
}

/// A dimension CSV that the feed publishes.
pub struct DimensionFile {
    /// Table name as `Info.xml` knows it, e.g. `ORGAO`.
    pub table: String,
    pub name: String,
}

/// What one refresh did, for the UI's sync banner.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SyncReport {
    pub snapshot: String,
    /// `unchanged`, `appended` or `reloaded`.
    pub action: String,
    pub rows_added: i64,
    pub total_rows: i64,
    pub dimensions: usize,
    pub bytes_fetched: u64,
    pub detail: Option<String>,
}

/// Emitted while a long ingest runs, so the UI can show real progress.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Progress {
    pub phase: String,
    pub rows: i64,
    pub bytes: u64,
    pub total_bytes: u64,
}

fn emit(app: &AppHandle, phase: &str, rows: i64, bytes: u64, total_bytes: u64) {
    let _ = app.emit(
        "ingest-progress",
        Progress {
            phase: phase.to_string(),
            rows,
            bytes,
            total_bytes,
        },
    );
}

/// The infix that identifies one table's file, e.g. `_-_ORGAO_-_Base_de_Dados`.
///
/// The published shape is `<date>_Legisladoc_-_<TABLE>_-_Base_de_Dados.csv`.
/// Pinning the whole infix — rather than looking for the bare table name — is
/// what keeps `TIPO_ATO` from being mistaken for the `ATO` snapshot, and vice
/// versa.
fn table_marker(table: &str) -> String {
    format!("_-_{table}_-_Base_de_Dados")
}

/// The newest file in the listing for one table.
///
/// The feed publishes one dated set per day, so "newest" is decided by the date
/// prefix in the name, not by mtime — mtimes on a public mirror are not reliable
/// and the name is the feed's own versioning.
fn newest_table<'a>(files: &'a [RemoteFile], table: &str) -> Option<&'a RemoteFile> {
    let marker = table_marker(table);
    files
        .iter()
        .filter(|f| {
            let name = file_name(&f.path);
            name.ends_with(".csv") && name.contains(&marker)
        })
        .max_by(|a, b| a.path.cmp(&b.path))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Find the act snapshot and the dimension CSVs in a listing.
pub fn pick_files(files: &[RemoteFile]) -> (Option<SnapshotFile>, Vec<DimensionFile>) {
    let snapshot = newest_table(files, "ATO").map(|f| SnapshotFile {
        name: file_name(&f.path).to_string(),
        size: f.size,
    });

    // Every table the schema can relate to, except the act table itself. The
    // feed does not currently publish all of them (GRUPO_TIPOATO has none), so a
    // missing one is a normal outcome rather than an error.
    let dimensions = ["ORGAO", "TIPO_ATO", "SITUACAO_ACAO", "STATUS_ATO", "GRUPO_TIPOATO"]
        .into_iter()
        .filter_map(|table| {
            newest_table(files, table).map(|f| DimensionFile {
                table: table.to_string(),
                name: file_name(&f.path).to_string(),
            })
        })
        .collect();

    (snapshot, dimensions)
}

/// Download and parse `Info.xml` from the listing.
///
/// Returns the parsed schema alongside the document text, which is what gets
/// persisted — so a restart can rebuild the UI's field labels without a network
/// round trip.
pub fn fetch_schema(files: &[RemoteFile]) -> Result<(Schema, String)> {
    let f = files
        .iter()
        .filter(|f| file_name(&f.path).ends_with("Info.xml"))
        .max_by(|a, b| a.path.cmp(&b.path))
        .context("the feed listing has no Info.xml, which describes the CSV layout")?;
    let name = file_name(&f.path).to_string();

    let mut xml = Vec::new();
    std::io::Read::read_to_end(&mut remote::get_all(&remote::url_for(&name))?, &mut xml)
        .context("reading Info.xml")?;

    let text = decode(&xml);
    let schema = Schema::parse(text.as_bytes()).context("parsing Info.xml")?;
    if schema.table("ATO").is_none() {
        bail!("o Info.xml não declara a tabela ATO");
    }
    Ok((schema, text))
}

/// Replace one dimension table from its published CSV.
///
/// These are tiny semicolon-delimited files with no embedded newlines, so the
/// act parser's `NUL CRLF` record splitting does not apply — they really are
/// line-based, and a line split is correct here.
fn load_dimension(store: &mut Store, table: &str, name: &str) -> Result<usize> {
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut remote::get_all(&remote::url_for(name))?, &mut bytes)
        .with_context(|| format!("reading {name}"))?;

    let text = decode(&bytes);
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        // A stray `NUL` turns up as a separate line in a couple of these files.
        .map(|l| l.trim_matches(['\u{0}', '\u{feff}']).to_string());

    let header: Vec<String> = lines
        .next()
        .with_context(|| format!("{name} está vazio"))?
        .split(';')
        .map(|s| s.trim().to_string())
        .collect();
    if header.is_empty() || header[0].is_empty() {
        bail!("{name} não tem cabeçalho");
    }

    let rows: Vec<Vec<Option<String>>> = lines
        .map(|line| {
            line.split(';')
                .map(|v| {
                    let v = v.trim_end_matches('\u{0}');
                    if v.is_empty() {
                        None
                    } else {
                        Some(v.to_string())
                    }
                })
                .collect()
        })
        .filter(|r: &Vec<Option<String>>| r.iter().any(|v| v.is_some()))
        .collect();

    let n = rows.len();
    store.replace_dimension(table, &header, &rows)?;
    Ok(n)
}

/// Decode feed bytes as UTF-8, falling back to Windows-1252 (the real encoding
/// for accented text, e.g. `SECRETARIA MUNICIPAL DE ADMINISTRAÇÃO`).
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            cow.into_owned()
        }
    }
}

/// Ingest the act snapshot, using the delta path when it is safe to.
pub fn ingest_snapshot(app: &AppHandle, store: &mut Store, snapshot: &SnapshotFile) -> Result<SyncReport> {
    let url = remote::url_for(&snapshot.name);

    emit(app, "checking", 0, 0, snapshot.size.max(0) as u64);

    let info = remote::head(&url).context("HEAD on the act snapshot")?;
    let remote_size = info.size.unwrap_or(snapshot.size.max(0) as u64);
    let etag = info.etag.as_deref();

    // Record the validators before touching data, so an interrupted run still
    // knows which revision it was working against.
    store.upsert_snapshot_meta(&snapshot.name, remote_size, etag, info.last_modified.as_deref())?;

    let mut state = store.snapshot_state(&snapshot.name)?;
    let plan = plan_for(&url, remote_size, etag, state.take())?;

    match plan {
        Plan::Unchanged => {
            let total = store.act_count()?;
            emit(app, "done", total, 0, remote_size);
            return Ok(SyncReport {
                snapshot: snapshot.name.clone(),
                action: "unchanged".into(),
                rows_added: 0,
                total_rows: total,
                ..Default::default()
            });
        }
        Plan::Append { from, to } => {
            emit(app, "tail", 0, 0, remote_size - from);
            let stream = remote::get_tail(&url, from)?;
            let stats = ingest::scan_tail(stream).context("scanning the snapshot tail")?;

            // The tail must be a genuine append: every new id has to sit above
            // the stored maximum. If it does not, the file was rewritten and the
            // rows must not be trusted.
            let last_id = store.max_act_id()?;
            if let Err(e) = ingest::validate_append(&stats.rows, last_id) {
                return full_reload(app, store, snapshot, &url, remote_size, etag, info.last_modified.as_deref(), &e.to_string());
            }

            store.insert_rows(&stats.rows)?;
            let total = store.act_count()?;
            store.commit_snapshot_progress(
                &snapshot.name,
                to,
                store.max_act_id()?,
                total,
            )?;
            emit(app, "done", total, 0, remote_size);
            return Ok(SyncReport {
                snapshot: snapshot.name.clone(),
                action: "appended".into(),
                rows_added: stats.rows.len() as i64,
                total_rows: total,
                bytes_fetched: to - from,
                dimensions: 0,
                detail: None,
            });
        }
        Plan::Full { reason } => {
            return full_reload(app, store, snapshot, &url, remote_size, etag, info.last_modified.as_deref(), &reason);
        }
    }
}

/// Decide what to do, including the cheap prefix check the append case needs.
fn plan_for(
    url: &str,
    remote_size: u64,
    etag: Option<&str>,
    state: Option<SnapshotState>,
) -> Result<Plan> {
    // Only worth verifying the boundary if we are about to rely on it.
    let head_prefix_ok = match &state {
        Some(s) if s.offset > 0 && remote_size > s.offset => {
            // Re-read the bytes ending just before our offset. If they still
            // close a record, the prefix we ingested has not been rewritten and
            // the bytes from `offset` onward are new. Either terminator counts:
            // the feed closes about 49,700 records with a bare `CR LF`, and
            // insisting on the `NUL CR LF` form would force a full re-ingest on
            // every one of those boundaries.
            let from = s.offset.saturating_sub(parser::TERM_LEN as u64);
            match remote::get_tail(url, from) {
                Ok(r) => {
                    let mut buf = [0u8; parser::TERM_LEN];
                    let n = std::io::Read::read(&mut std::io::Read::take(r, parser::TERM_LEN as u64), &mut buf)
                        .unwrap_or(0);
                    ingest::ends_on_boundary(&buf[..n])
                }
                Err(_) => false,
            }
        }
        _ => true,
    };
    Ok(ingest::plan(remote_size, etag, state.as_ref(), head_prefix_ok))
}

/// Re-read the whole snapshot. Used on first run and whenever the delta checks
/// fail; it clears the act table first so the result is exactly the file.
#[allow(clippy::too_many_arguments)]
fn full_reload(
    app: &AppHandle,
    store: &mut Store,
    snapshot: &SnapshotFile,
    url: &str,
    remote_size: u64,
    etag: Option<&str>,
    last_modified: Option<&str>,
    reason: &str,
) -> Result<SyncReport> {
    log::info!("full reload of {}: {reason}", snapshot.name);
    emit(app, "reloading", 0, 0, remote_size);
    store.clear_acts()?;

    let source = remote::get_all(url)?;
    let loaded = ingest_stream(store, source, remote_size, |rows, bytes| {
        emit(app, "loading", rows, bytes, remote_size)
    })?;

    store.commit_snapshot_progress(&snapshot.name, remote_size, store.max_act_id()?, loaded.rows)?;
    store.upsert_snapshot_meta(&snapshot.name, remote_size, etag, last_modified)?;
    emit(app, "done", loaded.rows, loaded.bytes, remote_size);

    Ok(SyncReport {
        snapshot: snapshot.name.clone(),
        action: "reloaded".into(),
        rows_added: loaded.rows,
        total_rows: loaded.rows,
        dimensions: 0,
        bytes_fetched: loaded.bytes,
        detail: Some(reason.to_string()),
    })
}

/// Rows held before a batch is written. Big enough that the per-transaction cost
/// disappears, small enough that a failure loses at most one batch of work.
const BATCH: usize = 5_000;

/// What a full scan of the snapshot produced.
#[derive(Debug)]
pub struct Loaded {
    pub rows: i64,
    pub bytes: u64,
}

/// Stream a whole snapshot into `store`, reporting progress as it goes.
///
/// Split out from [`full_reload`] so it can be exercised against a file on disk
/// instead of the live feed — the streaming, chunk-boundary and batching
/// behaviour is the part worth testing, not the HTTP call around it.
pub fn ingest_stream<R: std::io::Read>(
    store: &mut Store,
    mut source: R,
    total_bytes: u64,
    mut progress: impl FnMut(i64, u64),
) -> Result<Loaded> {
    // The header is one CRLF line, so by the time a full chunk has arrived its
    // offset is known. Doing this before the loop keeps the streaming path below
    // free of first-chunk special cases.
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 20);
    let mut chunk = vec![0u8; 1 << 20];

    // The header is one CRLF line, dropped from the buffer as soon as it has
    // fully arrived. A read that stops mid-header leaves this set so the next
    // pass tries again — the decision is deferred, never forced.
    let mut header_pending = true;

    // Records are parsed into bounded batches so a 1.4 GB file never has to sit
    // in memory at once, and a crash loses at most one batch.
    let mut pending: Vec<parser::Row> = Vec::with_capacity(BATCH);
    let mut loaded = Loaded { rows: 0, bytes: 0 };

    loop {
        let n = std::io::Read::read(&mut source, &mut chunk).context("reading the snapshot")?;
        if n == 0 {
            break;
        }
        // Every pass goes through the record scan, including the first. Reading
        // the first chunk outside the loop would lose it whenever the source
        // hands over everything at once and then reports end-of-stream — a
        // slice, or a delta tail small enough to fit in one chunk.
        loaded.bytes += n as u64;
        buf.extend_from_slice(&chunk[..n]);

        if header_pending {
            let header = parser::skip_header(&buf);
            if header > 0 {
                buf.drain(..header);
                header_pending = false;
            } else if buf.len() >= 3 && !buf.starts_with(b"ID;") {
                // Not a header line at all, so there is nothing to skip.
                header_pending = false;
            }
        }

        // `split_records` hands back the trailing partial record, which is
        // carried into the next chunk — that is what makes a record straddling a
        // chunk boundary parse correctly.
        let (records, rest) = parser::split_records(&buf);
        for rec in records {
            match parser::parse_record(rec) {
                Ok(row) => pending.push(row),
                Err(_) => bail!("um registro completo não pôde ser lido; o formato do arquivo mudou"),
            }
        }
        buf = rest.to_vec();

        while pending.len() >= BATCH {
            let batch: Vec<parser::Row> = pending.drain(..BATCH).collect();
            loaded.rows += batch.len() as i64;
            store.insert_rows(&batch)?;
            progress(loaded.rows, loaded.bytes);
        }
    }

    // A file that ends anywhere but on a terminator was cut short in transit. The
    // parser cannot see this on its own — a body has no length marker, so a
    // half-record parses "fine" — which is why the leftover is checked here.
    if !buf.is_empty() && !buf.iter().all(|b| b.is_ascii_whitespace()) {
        bail!(
            "o arquivo termina no meio de um registro ({} bytes restantes); o download foi interrompido",
            buf.len()
        );
    }

    if !pending.is_empty() {
        loaded.rows += pending.len() as i64;
        store.insert_rows(&pending)?;
    }

    let _ = total_bytes;
    progress(loaded.rows, loaded.bytes);
    Ok(loaded)
}

/// Bring the whole local database up to date with the feed.
pub fn refresh(app: &AppHandle, store: &mut Store, listing: &feed::Listing) -> Result<SyncReport> {
    let (snapshot, dimensions) = pick_files(&listing.files);
    let snapshot = snapshot.context("the feed listing has no ATO snapshot CSV")?;

    // Schema first: the act table is created from it.
    emit(app, "schema", 0, 0, 0);
    let (schema, xml) = fetch_schema(&listing.files)?;
    store.ensure_act_tables(&schema)?;
    store.save_schema_doc(&xml)?;

    let mut loaded = 0usize;
    for d in &dimensions {
        match load_dimension(store, &d.table, &d.name) {
            Ok(_) => loaded += 1,
            // One unreadable lookup table must not abort the whole refresh.
            Err(e) => log::warn!("dimension {} failed: {e:#}", d.table),
        }
    }

    let mut report = ingest_snapshot(app, store, &snapshot)?;
    report.dimensions = loaded;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rf(path: &str, size: i64) -> RemoteFile {
        RemoteFile {
            path: path.to_string(),
            size,
            mtime: None,
        }
    }

    fn listing() -> Vec<RemoteFile> {
        vec![
            rf("2026-10-07_Legisladoc_-_ATO_-_Base_de_Dados.csv", 100),
            rf("2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv", 1424486256),
            rf("2026-10-08_Legisladoc_-_Info.xml", 13088),
            rf("2026-10-08_Legisladoc_-_ORGAO_-_Base_de_Dados.csv", 4607),
            rf("2026-10-08_Legisladoc_-_TIPO_ATO_-_Base_de_Dados.csv", 13141),
            rf("2026-10-08_Legisladoc_-_SITUACAO_ACAO_-_Base_de_Dados.csv", 1127),
            rf("2026-10-08_Legisladoc_-_STATUS_ATO_-_Base_de_Dados.csv", 171),
            rf("2026-10-08_Legisladoc_-_Dicionario_de_Dados.xlsx", 13351),
        ]
    }

    #[test]
    fn picks_the_newest_snapshot_not_the_newest_ato_named_file() {
        let (snapshot, dims) = pick_files(&listing());
        let s = snapshot.unwrap();
        assert_eq!(s.name, "2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv");
        assert_eq!(s.size, 1424486256);
        // TIPO_ATO is a dimension and must not be mistaken for the snapshot.
        assert_eq!(dims.len(), 4);
        assert!(dims.iter().any(|d| d.table == "ORGAO"));
        assert!(dims.iter().any(|d| d.table == "TIPO_ATO"));
        // Not published by the feed, so it is simply absent.
        assert!(!dims.iter().any(|d| d.table == "GRUPO_TIPOATO"));
    }

    #[test]
    fn a_directory_entry_is_not_treated_as_a_file() {
        // `fetch_listing` filters these out, but the picker must not depend on
        // that having happened.
        let files = vec![rf("2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados", 0)];
        let (snapshot, _) = pick_files(&files);
        assert!(snapshot.is_none(), "only .csv files count");
    }

    #[test]
    fn a_feed_without_a_snapshot_reports_none_rather_than_a_wrong_file() {
        let files = vec![rf("2026-10-08_Legisladoc_-_ORGAO_-_Base_de_Dados.csv", 4)];
        let (snapshot, dims) = pick_files(&files);
        assert!(snapshot.is_none());
        assert_eq!(dims.len(), 1);
    }

    #[test]
    fn dimension_csvs_decode_windows_1252_names() {
        // `ADMINISTRAÇÃO` as it actually appears in the feed.
        let bytes = b"ID;SIGLA;NOME\r\n4;SMAD;SECRETARIA MUNICIPAL DE ADMINISTRA\xc7\xc3O\r\n";
        let text = decode(bytes);
        assert!(text.contains("ADMINISTRAÇÃO"), "{text}");
    }

    /// Ingest the real snapshot when one has been downloaded, to prove the whole
    /// path — header skip, chunk-boundary records, batching, truncation check —
    /// against 1.4 GB of the feed's actual bytes rather than a fixture.
    ///
    /// Skipped when the file is absent so the suite stays hermetic.
    #[test]
    fn a_real_snapshot_ingests_into_the_store_when_present() {
        let path = std::env::var("LEGISLADOC_SAMPLE_CSV").unwrap_or_else(|_| {
            r"C:\Users\Erick\AppData\Local\Temp\feed\full_ato.csv".to_string()
        });
        let Ok(file) = std::fs::File::open(&path) else {
            eprintln!("skipping: no snapshot at {path}");
            return;
        };
        let size = file.metadata().unwrap().len();

        let xml = std::fs::read(
            std::env::var("LEGISLADOC_SAMPLE_XML").unwrap_or_else(|_| {
                r"C:\Users\Erick\AppData\Local\Temp\feed\2026-10-08_Legisladoc_-_Info.xml".to_string()
            }),
        )
        .expect("the schema file is needed alongside the snapshot");
        let schema = crate::schema::Schema::parse(&xml).unwrap();

        let mut store = crate::store::Store::open_in_memory().unwrap();
        store.ensure_act_tables(&schema).unwrap();

        // Read through a reader that hands out small chunks, so records that
        // straddle a chunk boundary are actually exercised rather than
        // accidentally landing whole.
        let loaded = ingest_stream(&mut store, SlowReader::new(file), size, |_, _| {}).unwrap();

        assert_eq!(loaded.rows, store.act_count().unwrap());
        // The exact count, so a splitter that silently drops records — the
        // 49,716 acts lost to bare-`CR LF` boundaries — cannot pass unnoticed.
        assert_eq!(loaded.rows, 443_294, "the ingest lost or invented records");
        // Every record has a numeric primary key, so the max must be set.
        assert!(store.max_act_id().unwrap().unwrap() > 0);

        // Act 479980 is the case that exposed the metadata separator bug: its
        // `CABECALHO` is `<p style="margin-left:0cm; margin-right:0cm">…`, and
        // the `;` inside `style` used to end the field. The truncated tag was
        // then stored as the heading and the rest of it opened the body, so the
        // act displayed a fragment of CSS. Pinned against the real bytes because
        // a hand-written fixture proves the parser agrees with itself, not that
        // it agrees with the feed.
        let detail = store
            .act_detail(&schema, 479_980)
            .unwrap()
            .expect("act 479980 is in the snapshot");
        let cabecalho = detail
            .fields
            .iter()
            .find(|f| f.column == "CABECALHO")
            .and_then(|f| f.value.as_deref())
            .expect("act 479980 has a CABECALHO");
        assert!(
            cabecalho.contains(r#"margin-left:0cm; margin-right:0cm"#)
                && cabecalho.contains("PREFEITURA MUNICIPAL DE CURITIBA"),
            "the style attribute's semicolon must stay inside CABECALHO, got {cabecalho:?}"
        );

        // A body is HTML, so it starts with a tag. Before the fix 56,416 stored
        // bodies began with the leftover of a cut tag instead (e.g.
        // ` margin-right:0cm">PREFEITURA…`). Check a sample rather than all
        // 393k, which would only re-measure what the fixture already proves.
        let body = detail.body.as_deref().unwrap_or("");
        assert!(
            body.trim_start().starts_with('<'),
            "body must begin with markup, got {:?}",
            &body[..body.len().min(60)]
        );
    }

    /// A reader that returns at most 4 KiB per call, to force chunk boundaries
    /// through the middle of records.
    struct SlowReader<R>(R);

    impl<R: std::io::Read> SlowReader<R> {
        fn new(r: R) -> Self {
            SlowReader(r)
        }
    }

    impl<R: std::io::Read> std::io::Read for SlowReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(4 * 1024);
            self.0.read(&mut buf[..n])
        }
    }

    /// `n` records in the feed's on-disk encoding: 23 `;`-separated metadata
    /// fields, then a body, then `NUL CR LF`.
    fn encoded_records(n: i64) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 1..=n {
            out.extend_from_slice(format!("{i};").as_bytes());
            for _ in 1..parser::META_FIELDS {
                out.extend_from_slice(b";");
            }
            out.extend_from_slice(b"<p>body</p>;\x00\r\n");
        }
        out
    }

    #[test]
    fn an_ingest_that_ends_mid_record_is_rejected() {
        // Without this check a truncated download would silently load a partial
        // last act, because a body has no length marker to fail on.
        let mut store = crate::store::Store::open_in_memory().unwrap();
        store.ensure_act_tables(&crate::store::test_schema()).unwrap();

        let mut data = encoded_records(10);
        data.truncate(data.len() - 9);
        let err = ingest_stream(&mut store, &data[..], data.len() as u64, |_, _| {}).unwrap_err();
        assert!(err.to_string().contains("meio de um registro"), "{err}");
    }

    #[test]
    fn a_complete_ingest_loads_every_record() {
        let mut store = crate::store::Store::open_in_memory().unwrap();
        store.ensure_act_tables(&crate::store::test_schema()).unwrap();

        let data = encoded_records(2500);
        let loaded = ingest_stream(&mut store, &data[..], data.len() as u64, |_, _| {}).unwrap();
        assert_eq!(loaded.rows, 2500);
        assert_eq!(store.act_count().unwrap(), 2500);
        // Batching must not lose or duplicate the tail.
        assert_eq!(store.max_act_id().unwrap(), Some(2500));
    }

    #[test]
    fn the_schema_document_is_round_tripped_as_text() {
        // Whatever `fetch_schema` persists has to parse again on restart, since
        // that is what rebuilds the field labels.
        let xml = r#"<Info version="2.0.0"><Tables><Table name="ATO"><Columns>
               <Column index="01" name="ID" type="int" help="Chave"></Column>
               </Columns></Table></Tables></Info>"#;
        let schema = crate::schema::Schema::parse(xml.as_bytes()).unwrap();
        let again = crate::schema::Schema::parse(xml.as_bytes()).unwrap();
        assert_eq!(schema.table("ATO").unwrap().columns[0].help, again.table("ATO").unwrap().columns[0].help);
        assert_eq!(
            schema.table("ATO").unwrap().columns[0].help.as_deref(),
            Some("Chave")
        );
    }
}
