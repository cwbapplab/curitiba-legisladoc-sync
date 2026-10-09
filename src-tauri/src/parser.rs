//! Parser for the ATO snapshot CSV.
//!
//! This file is **not** RFC 4180 and cannot be read by a standard CSV library.
//! Confirmed against the live snapshot:
//!
//! * A record ends at `NUL CR LF` (`\x00\r\n`) — or, for about 49,700 of them,
//!   at a bare `CR LF`. The NUL is also the NULL marker for a field, so
//!   `SUMULA;\x00;` means "súmula is NULL". See [`split_records`] for why the
//!   second form can be told apart from the `\r\n` that appears inside a body.
//! * The first 23 columns are `;`-separated.
//! * The act body (`AREA1`/`AREA2`) is raw HTML that contains **all** of:
//!   bare `\n` and `\r\n`, literal `"` characters, `;` inside `style="..."`, and
//!   even base64-encoded embedded images. Any quoting-aware parser desynchronises
//!   on it: `encoding_rs` + the `csv` crate yields 188,459 records for a 30 MB
//!   slice where the true count is 7,351.
//!
//! So the body is treated as an opaque remainder: everything after the 23rd `;`
//! up to the record terminator. Validated at 100% on sampled records.
//!
//! `AREA1` ("texto principal") and `AREA2` ("texto secundário") are merged into a
//! single `body` value because the published file does not delimit them reliably.

use anyhow::{bail, Result};

/// Number of `;`-separated metadata fields before the act body.
///
/// The published CSV declares 25 columns (`ID` … `AREA1`; `AREA2`). `AREA1` and
/// `AREA2` are two halves of one HTML document that the exporter happened to cut
/// at a `;` inside a `style="..."` attribute, so they are re-joined into a single
/// `body` value. A parsed [`Row`] therefore has `META_FIELDS + 1 == 24` values for
/// a 25-column schema; the storage layer collapses the schema's last two columns
/// to match.
pub const META_FIELDS: usize = 23;

/// Values produced per record: the metadata fields plus the merged body.
pub const VALUES_PER_ROW: usize = META_FIELDS + 1;

/// Signals that the bytes end part-way through a record.
#[derive(Debug)]
pub struct NeedMore;

/// One act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// One entry per live column, in schema order. `None` is SQL NULL.
    pub values: Vec<Option<String>>,
    /// Byte length of the record as it appeared in the file.
    pub src_bytes: usize,
    /// Character count, used for the transfer checksum in the delta handshake.
    pub src_chars: u64,
}

impl Row {
    /// Convenience accessor for the primary key.
    pub fn id(&self) -> Option<i64> {
        self.values.first().and_then(|v| v.as_deref())?.parse().ok()
    }
}

/// Skip the header line if present. Returns the offset of the first record.
pub fn skip_header(bytes: &[u8]) -> usize {
    // The header is a single CRLF-terminated line of column names.
    if bytes.starts_with(b"ID;") {
        if let Some(p) = find(bytes, b"\r\n", 0) {
            return p + 2;
        }
    }
    0
}

/// Find `needle` in `hay` at or after `from`.
fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The record terminator.
pub const TERM: &[u8] = b"\x00\r\n";

/// Length of [`TERM`], for callers that re-fetch the bytes just before a known
/// record boundary to check they are still a boundary.
pub const TERM_LEN: usize = TERM.len();

/// Find the `;` that ends the metadata field beginning at `from`.
///
/// Returns `None` when the record runs out before the field does — a truncated
/// record. A separator inside a quoted attribute value is skipped, so markup
/// like `<p style="a;b">` stays in one field. A quote only opens a value when it
/// follows an `=`: the feed writes a bare `"` in prose (a SUMULA containing one,
/// say) without escaping it, and treating that as an opening quote would swallow
/// the rest of the record.
fn meta_separator(rec: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i < rec.len() {
        if rec[i] == b';' {
            return Some(i);
        }
        if rec[i] == b'=' && rec.get(i + 1) == Some(&b'"') {
            // Skip the quoted value. A value with no closing quote means the
            // record was cut short.
            let close = rec[i + 2..].iter().position(|&b| b == b'"')?;
            i += 2 + close + 1;
            continue;
        }
        i += 1;
    }
    None
}

/// Parse one record. Returns `Err(NeedMore)` when the slice is incomplete — which
/// is the expected outcome for the final chunk of a streamed body, and for the
/// leading partial record of a delta range.
pub fn parse_record(rec: &[u8]) -> Result<Row, NeedMore> {
    let mut values: Vec<Option<String>> = Vec::with_capacity(VALUES_PER_ROW);

    let mut start = 0usize;
    for _ in 0..META_FIELDS {
        // A field ends at the next `;` that is not inside an attribute value.
        //
        // `CABECALHO` carries markup of the form
        // `<p style="margin-left:0cm; margin-right:0cm">…`, where the quoted
        // styles contain semicolons. Splitting on every `;` cut those tags in
        // half: the leftover attribute text was stored as `CABECALHO` and the
        // rest of the tag opened the act's body, which then showed up on screen
        // as a fragment of CSS. The feed is not quoted CSV — a lone `"` in prose
        // is not escaped — so the only quote that can open a value is the one
        // right after an `=`.
        let Some(sep) = meta_separator(rec, start) else {
            // Ran out of separators => the record is truncated.
            return Err(NeedMore);
        };
        values.push(field(&rec[start..sep]));
        start = sep + 1;
    }

    // Whatever remains is the body, minus the trailing separator that precedes
    // the record terminator.
    let mut end = rec.len();
    if end > start && rec[end - 1] == b';' {
        end -= 1;
    }

    // `AREA1` and `AREA2` are merged by keeping the `;` between them. When
    // `AREA1` is NULL the record reads `…;CABECALHO;\x00;AREA2`, so the body
    // would open with the NUL marker and the separator that used to align the
    // columns — neither belongs to the act, and both would show up as a stray
    // character before the first tag. An act whose `AREA1` is merely empty
    // (rather than NULL) starts with the bare separator and is handled the same
    // way; no act's HTML begins with one.
    let mut body = &rec[start..end];
    if body.first() == Some(&0) {
        body = &body[1..];
    }
    if body.first() == Some(&b';') {
        body = &body[1..];
    }
    values.push(field(body));

    let src_chars = rec
        .iter()
        .map(|&b| if b & 0xC0 != 0x80 { 1u64 } else { 0 })
        .sum();

    Ok(Row {
        values,
        src_bytes: rec.len(),
        src_chars,
    })
}

/// Decode a raw field: `\x00` and empty are NULL, everything else is
/// Windows-1252 text (the feed's real encoding — `Publicação`, `Licitações`).
fn field(raw: &[u8]) -> Option<String> {
    let raw = raw.strip_suffix(b"\x00").unwrap_or(raw);
    if raw.is_empty() {
        return None;
    }
    match std::str::from_utf8(raw) {
        Ok(s) => Some(s.to_string()),
        Err(_) => {
            let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(raw);
            Some(cow.into_owned())
        }
    }
}

/// Bytes of the following line needed to recognise a record's first line.
/// The longest first line is `479980;DOC479980;` — 17 bytes — plus headroom.
const RECORD_START_LOOKAHEAD: usize = 24;

/// Does `s` open a record's first line, `<digits>;DOC<digits>;`?
fn is_record_start(s: &[u8]) -> bool {
    let mut i = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || !s[i..].starts_with(b";DOC") {
        return false;
    }
    let mut j = i + 4;
    let digits = j;
    while j < s.len() && s[j].is_ascii_digit() {
        j += 1;
    }
    j > digits && s.get(j) == Some(&b';')
}

/// Split a buffer into complete records plus the trailing partial remainder.
///
/// A record ends at `NUL CR LF`. That is **not** the only boundary the feed
/// writes: about 49,700 records end with a bare `CR LF`, so requiring the NUL
/// silently swallowed each of those acts into the body of the one before it and
/// dropped them from the archive. Measured on the published snapshot: 443,293
/// records, of which 393,576 end with `NUL CR LF` and 49,717 with `CR LF` alone.
///
/// A bare `CR LF` counts only when the next line opens a record, which is what
/// keeps the rule safe: `\r\n` is precisely the line break an act's HTML uses,
/// so `\r\n` alone would be the wrong test — but `\r\n` followed by
/// `<digits>;DOC<digits>;` never occurs inside a body. (The `<div>` produced by
/// an embedded spreadsheet puts `\n` with no `\r` at all, so it cannot be
/// mistaken for one either.) Every one of the 443,293 records this finds has
/// well-formed metadata: `VERSAO` is 0–50 throughout and `ANO_PUBLICACAO` is
/// always either empty or a four-digit year.
///
/// Recognising a bare boundary needs up to [`RECORD_START_LOOKAHEAD`] bytes past
/// the candidate, so one within reach of the end of the buffer is left to the
/// next chunk rather than guessed at. The metadata that follows is far longer
/// than that, so no real boundary is missed — the check would fail anyway.
pub fn split_records(buf: &[u8]) -> (Vec<&[u8]>, &[u8]) {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < buf.len() {
        if buf[i] != b'\n' {
            i += 1;
            continue;
        }
        if i >= 1 && buf[i - 1] == b'\r' {
            // `NUL CR LF`, the terminator the feed documents.
            if i >= 2 && buf[i - 2] == 0 {
                out.push(&buf[start..i - 2]);
                start = i + 1;
            } else if buf.len() - (i + 1) >= RECORD_START_LOOKAHEAD
                && is_record_start(&buf[i + 1..])
            {
                out.push(&buf[start..i - 1]);
                start = i + 1;
            }
        }
        i += 1;
    }
    (out, &buf[start..])
}

/// Validate that a decoded column looks like the type the schema promised.
///
/// Used by the delta path to decide whether an append-only assumption held: if a
/// supposedly-numeric key column contains text, the layout changed and the caller
/// should fall back to a full re-ingest.
pub fn looks_numeric(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Parse the header names out of a snapshot header line, for cross-checking
/// against `Info.xml`.
pub fn parse_header_names(bytes: &[u8]) -> Result<Vec<String>> {
    let end = find(bytes, b"\r\n", 0).unwrap_or(bytes.len());
    let line = &bytes[..end];
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(line);
    let names: Vec<String> = cow.split(';').map(|s| s.trim().to_string()).collect();
    if names.len() < META_FIELDS + 2 {
        bail!(
            "header has only {} columns, expected at least {}",
            names.len(),
            META_FIELDS + 2
        );
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record with the awkward features that break real CSV parsers: a bare LF
    /// and a CRLF inside the body, `;` inside a style attribute, and a literal `"`.
    fn sample_record() -> Vec<u8> {
        let mut r = Vec::new();
        r.extend_from_slice(
            b"293699;DOC293699;;213;;2019;1;0;2019-11-05 09:24:28.847;2019-11-05 09:51:08.557;\
              2019-11-05 00:00:00.000;;1;CLASSIFICACAO;Licitacoes;Publicado;Em Vigor;\
              FUNDACAO DE ACAO SOCIAL;403079;;(nenhum);\x00;\x00;",
        );
        r.extend_from_slice(
            b"<p style=\"margin-left:0cm; margin-right:0cm\">line one\n\
              line two</p>\r\n<p>quoted \" here</p>",
        );
        r.push(b';');
        r.extend_from_slice(b"\x00\r\n");
        r
    }

    #[test]
    fn parses_all_columns_and_the_body() {
        let rec = sample_record();
        let (recs, rest) = split_records(&rec);
        assert_eq!(recs.len(), 1);
        assert!(rest.is_empty(), "record ends exactly on the terminator");

        let row = parse_record(recs[0]).unwrap();
        assert_eq!(row.values.len(), VALUES_PER_ROW);
        assert_eq!(row.id(), Some(293699));
        assert_eq!(row.values[1].as_deref(), Some("DOC293699"));
        // `;;` is an empty field -> NULL, and `\x00` is NULL too.
        assert_eq!(row.values[2], None, "NUMERO_OFICIAL empty");
        assert_eq!(row.values[3].as_deref(), Some("213"));
        assert_eq!(row.values[21], None, "SUMULA was the NUL marker");

        // The body survives intact, including its LF, CRLF and `;`.
        // The body is the last value: index META_FIELDS.
        let body = row.values[META_FIELDS].as_deref().unwrap();
        assert!(body.contains("line one\nline two</p>\r\n"));
        assert!(body.contains("margin-left:0cm; margin-right:0cm"));
        assert!(body.contains("quoted \" here"));
        assert!(!body.ends_with(';'), "trailing separator is stripped");
    }

    /// The real shape that broke the parser: `CABECALHO` holds a tag whose
    /// `style` attribute contains a semicolon, and the act's body follows it.
    #[test]
    fn a_semicolon_inside_an_attribute_does_not_split_the_field() {
        // Field 22 (CABECALHO) carries the markup; the body follows field 22's
        // separator. Taken verbatim from act 479980 in the published feed.
        let rec: Vec<u8> = [
            &b"479980;DOC479980;192;187;2;2026;1;0;2026-10-07 08:05:47.097;\
               2026-10-07 13:14:42.440;2026-10-07 18:30:04.987;;1;\
               EDITAL DE IMPOSICAO DE PENALIDADE ;;Publicado;Em Vigor;\
               SECRETARIA MUNICIPAL DE DEFESA SOCIAL E TRANSITO;584051;;\
               (nenhum);APLICA A PENALIDADE DE MULTA.;"[..],
            &b"<p style=\"margin-left:0cm; margin-right:0cm\">PREFEITURA</p>"[..],
            &b";<p>corpo do ato</p>;"[..],
            &b"\x00\r\n"[..],
        ]
        .concat();

        let (recs, _) = split_records(&rec);
        let row = parse_record(recs[0]).unwrap();
        assert_eq!(row.values.len(), VALUES_PER_ROW);
        assert_eq!(row.id(), Some(479980));

        // The whole tag, semicolon included, belongs to CABECALHO.
        assert_eq!(
            row.values[22].as_deref(),
            Some("<p style=\"margin-left:0cm; margin-right:0cm\">PREFEITURA</p>"),
            "the attribute's semicolon must not end the field"
        );
        // And the body is the body, not the remainder of a cut tag.
        assert_eq!(row.values[META_FIELDS].as_deref(), Some("<p>corpo do ato</p>"));
    }

    /// A bare `"` in prose must not be taken for an opening quote, or the rest
    /// of the record would be swallowed looking for its pair.
    #[test]
    fn an_unescaped_quote_in_a_field_does_not_swallow_the_record() {
        let rec: Vec<u8> = [
            &b"1;DOC1;2;3;4;2005;1;1;a;b;c;;0;DECRETO;;Publicado;Em Vigor;ORGAO;9;;\
               (nenhum);APROVA A \"ZONA\" ESPECIAL;\x00;"[..],
            &b"<p>corpo</p>;"[..],
            &b"\x00\r\n"[..],
        ]
        .concat();

        let (recs, _) = split_records(&rec);
        let row = parse_record(recs[0]).unwrap();
        assert_eq!(row.values[21].as_deref(), Some("APROVA A \"ZONA\" ESPECIAL"));
        assert_eq!(row.values[22], None, "CABECALHO is the NUL marker");
        assert_eq!(row.values[META_FIELDS].as_deref(), Some("<p>corpo</p>"));
    }

    /// A NULL `AREA1` must leave a body that starts at `AREA2`, not one that
    /// opens with the NUL marker and the separator that kept the columns
    /// aligned. Real shape: act 148226 in the published feed.
    #[test]
    fn a_null_area1_does_not_leak_its_marker_into_the_body() {
        let rec: Vec<u8> = [
            &b"148226;DOC148226;58;26;1;2012;0;1;2012-05-02 10:00:00.000;\
               2012-05-02 10:00:00.000;2012-05-02 10:00:00.000;;0;EDITAL;;Publicado;\
               Em Vigor;SECRETARIA MUNICIPAL DE ABASTECIMENTO;12345;;;\x00;"[..],
            &b"\x00;<p>EDITAL RESUMIDO</p>;"[..],
            &b"\x00\r\n"[..],
        ]
        .concat();

        let (recs, _) = split_records(&rec);
        let row = parse_record(recs[0]).unwrap();
        assert_eq!(
            row.values[META_FIELDS].as_deref(),
            Some("<p>EDITAL RESUMIDO</p>"),
            "the NUL marker and its separator must not open the body"
        );
    }

    #[test]
    fn truncation_inside_the_metadata_is_detectable() {
        let rec = sample_record();
        // Any cut before the 23rd separator is unmistakably incomplete, because
        // the fixed-width metadata runs out of `;` before it runs out of fields.
        let meta_end = rec.iter().enumerate()
            .filter(|(_, &b)| b == b';')
            .nth(META_FIELDS - 1)
            .map(|(i, _)| i)
            .expect("sample has 23 separators");
        for cut in [1usize, 10, 40, 100, meta_end] {
            assert!(
                matches!(parse_record(&rec[..cut]), Err(NeedMore)),
                "cut at {cut} should be incomplete"
            );
        }
    }

    #[test]
    fn truncation_inside_the_body_is_not_detectable() {
        // Worth pinning down: the body is an unbounded remainder with no length
        // marker, so a record sliced mid-body still parses "successfully" with a
        // short body. Callers must never treat a parsed row as proof the record
        // was complete — completeness comes from `split_records` seeing the
        // terminator, which is why it returns the partial remainder separately.
        let rec = sample_record();
        let cut = rec.len() - 5; // inside the HTML, after the 23rd `;`
        let row = parse_record(&rec[..cut]).expect("body truncation parses");
        let full = parse_record(&rec[..rec.len() - 3]).unwrap();
        assert!(
            row.values[META_FIELDS].as_deref().unwrap().len()
                < full.values[META_FIELDS].as_deref().unwrap().len()
        );
    }

    /// A record may end with a bare `CR LF` instead of `NUL CR LF`. Treating
    /// the NUL as mandatory swallowed the next act — 49,716 of them across the
    /// published snapshot. Real shape: acts 177519 and 291704 in the feed.
    #[test]
    fn a_record_ending_in_a_bare_crlf_still_ends_the_record() {
        // Act 177519 verbatim: `CABECALHO` is NULL and the body is the origin
        // note, and the record ends with `CR LF` and no NUL.
        let first = record_with(
            &[
                "177519", "DOC177519", "277920", "", "", "2013", "1", "1", "a", "b", "c",
                "", "0", "AVISO", "", "Publicado", "Em Vigor", "IPPUC", "277920", "",
                "(nenhum)", "(nenhum)", "\x00",
            ],
            "Prefeitura Municipal de Curitiba",
            b"\r\n",
        );
        let second = record_with(
            &[
                "177520", "DOC177520", "277799", "", "", "2013", "1", "1", "a", "b", "c",
                "", "0", "EDITAL", "", "Publicado", "Em Vigor", "ORGAO", "1", "", "(nenhum)",
                "(nenhum)", "\x00",
            ],
            "<p>corpo</p>",
            b"\x00\r\n",
        );

        let mut buf = first.clone();
        buf.extend_from_slice(&second);

        let (recs, rest) = split_records(&buf);
        assert_eq!(recs.len(), 2, "the bare CRLF must end the first record");
        assert!(rest.is_empty());

        let a = parse_record(recs[0]).unwrap();
        assert_eq!(a.id(), Some(177519));
        // The next act must not have been swallowed into this body.
        let body = a.values[META_FIELDS].as_deref().unwrap();
        assert_eq!(body, "Prefeitura Municipal de Curitiba", "{body:?}");
        assert!(!body.contains("177520;DOC177520"), "no following record");

        let b = parse_record(recs[1]).unwrap();
        assert_eq!(b.id(), Some(177520));
        assert_eq!(b.values[META_FIELDS].as_deref(), Some("<p>corpo</p>"));
    }

    /// A `\r\n` inside a body must not be taken for a record boundary just
    /// because the body happens to use one — only a following record's first
    /// line makes it one.
    #[test]
    fn a_crlf_inside_a_body_is_not_a_boundary() {
        let rec = record_with(
            &[
                "7", "DOC7", "1", "", "", "2005", "1", "1", "a", "b", "c", "", "0", "DECRETO",
                "", "Publicado", "Em Vigor", "O", "1", "", "(nenhum)", "(nenhum)", "\x00",
            ],
            "<p>linha um</p>\r\n<p>linha dois</p>\r\n## nao e registro",
            b"\x00\r\n",
        );

        let (recs, _) = split_records(&rec);
        assert_eq!(recs.len(), 1, "a CRLF inside the body is not a boundary");
        let row = parse_record(recs[0]).unwrap();
        let body = row.values[META_FIELDS].as_deref().unwrap();
        assert!(body.contains("linha dois"), "{body:?}");
        assert!(body.contains("## nao e registro"), "{body:?}");
    }

    /// Assemble a record from exactly [`META_FIELDS`] metadata values, a body
    /// and a terminator. Building it this way rather than by hand keeps a
    /// fixture from silently gaining a field.
    fn record_with(meta: &[&str], body: &str, term: &[u8]) -> Vec<u8> {
        assert_eq!(
            meta.len(),
            META_FIELDS,
            "the fixture needs {META_FIELDS} metadata fields"
        );
        let mut r = meta.join(";").into_bytes();
        r.push(b';');
        r.extend_from_slice(body.as_bytes());
        r.extend_from_slice(term);
        r
    }

    #[test]
    fn split_carries_a_partial_record_as_remainder() {
        let a = sample_record();
        let b = sample_record();
        let mut buf = Vec::new();
        buf.extend_from_slice(&a);
        buf.extend_from_slice(&b[..b.len() / 2]);

        let (recs, rest) = split_records(&buf);
        assert_eq!(recs.len(), 1, "only the first record is complete");
        assert_eq!(rest, &b[..b.len() / 2]);
        assert!(parse_record(rest).is_err());
    }

    #[test]
    fn null_and_empty_fields_are_distinguished_from_text() {
        assert_eq!(field(b""), None);
        assert_eq!(field(b"\x00"), None);
        assert_eq!(field(b"(nenhum)").as_deref(), Some("(nenhum)"));
        assert_eq!(field(b"0").as_deref(), Some("0"));
    }

    #[test]
    fn windows_1252_accents_decode() {
        // 0xE7 = ç, 0xF5 = õ in Windows-1252.
        assert_eq!(field(b"Licita\xe7\xf5es").as_deref(), Some("Licitações"));
    }

    #[test]
    fn header_is_skipped_and_parsed() {
        let mut buf = b"ID;NUMERO_IDENTIFICACAO;NUMERO_OFICIAL".to_vec();
        for i in 4..=25 {
            buf.extend_from_slice(format!(";C{i}").as_bytes());
        }
        buf.extend_from_slice(b"\r\n");
        buf.extend_from_slice(&sample_record());

        let off = skip_header(&buf);
        assert!(off > 0);
        let names = parse_header_names(&buf).unwrap();
        assert_eq!(names[0], "ID");
        assert_eq!(names[1], "NUMERO_IDENTIFICACAO");
        assert_eq!(names.len(), 25);

        let (recs, _) = split_records(&buf[off..]);
        assert_eq!(recs.len(), 1);
    }

    #[test]
    fn char_count_ignores_utf8_continuation_bytes() {
        // "ç" is 2 bytes in UTF-8 but one character.
        let mut r = Vec::new();
        for i in 0..META_FIELDS {
            if i > 0 {
                r.push(b';');
            }
            r.extend_from_slice(b"x");
        }
        r.extend_from_slice(b";Licita\xc3\xa7\xc3\xb5es;\x00\r\n");
        let (recs, _) = split_records(&r);
        let row = parse_record(recs[0]).unwrap();
        let utf8_len = recs[0].len();
        // "Licitações" is 12 bytes but 10 chars, so chars < bytes.
        assert!(row.src_chars < utf8_len as u64);
        assert_eq!(row.src_chars, utf8_len as u64 - 2);
    }

    #[test]
    fn a_real_snapshot_parses_end_to_end_when_present() {
        // Runs against a local snapshot when one has been downloaded; skipped
        // otherwise so the suite stays hermetic.
        let path = std::env::var("LEGISLADOC_SAMPLE_CSV").unwrap_or_else(|_| {
            r"C:\Users\Erick\AppData\Local\Temp\feed\full_ato.csv".to_string()
        });
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping: no snapshot at {path}");
            return;
        };

        let names = parse_header_names(&bytes).unwrap();
        assert_eq!(names.len(), 25);
        assert_eq!(names[0], "ID");
        assert_eq!(names[24], "AREA2");

        let off = skip_header(&bytes);
        let (recs, rest) = split_records(&bytes[off..]);
        // Every record must parse, and the file must end on a terminator.
        assert!(rest.is_empty(), "file does not end on a record boundary");
        // The exact count, not a floor: a floor of 300k is what let 49,716 acts
        // go missing unnoticed. Every record is closed by one terminator and the
        // file ends on a terminator, so the count is the terminator count:
        // 393,577 `NUL CR LF` plus 49,717 bare `CR LF`. Update this number only
        // when the feed itself gains records.
        assert_eq!(
            recs.len(),
            443_294,
            "the record count changed; if the feed grew, update this number"
        );

        let mut ids = std::collections::HashSet::new();
        for r in &recs {
            let row = parse_record(r).expect("record should parse");
            let id = row.id().expect("every record must have a numeric ID");
            // A duplicate would mean the splitter split one act in two, which is
            // exactly the failure mode of a boundary rule that is too eager.
            assert!(ids.insert(id), "act {id} appears twice; a record was split");
        }
    }
}
