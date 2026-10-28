//! SQLite storage for the act data, plus the query API the UI drives.
//!
//! Tables are built from the feed's own `Info.xml` rather than hard-coded, so a
//! snapshot that adds or renames a column is picked up instead of being silently
//! dropped on the floor.

use anyhow::{bail, Context, Result};
use rusqlite::{params, params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::parser::{self, Row};
use crate::schema::Schema;

/// The act fact table and its full-text index.
const ACTS: &str = "acts";
const FTS: &str = "acts_fts";

/// Staging table a freshly downloaded snapshot is loaded into before its new
/// rows are migrated into `acts`. Dropped as soon as the merge commits.
const IMPORT: &str = "acts_import";

/// Rows copied per transaction while merging the staging table. Small enough
/// that the write-ahead log never has to hold a whole snapshot load at once.
const MERGE_BATCH: i64 = 5_000;

/// Columns that are safe to filter or sort on. Anything else is rejected rather
/// than interpolated into SQL.
/// The filterable set, published to the UI so it builds its controls from the
/// same list the SQL validates against.
pub fn filter_columns() -> &'static [&'static str] {
    filterable_columns()
}

/// The sortable set, published to the UI for the same reason.
pub fn sort_columns() -> &'static [&'static str] {
    sortable_columns()
}

fn filterable_columns() -> &'static [&'static str] {
    &[
        "NUMERO_IDENTIFICACAO",
        "NUMERO_OFICIAL",
        "DIARIO_OFICIAL",
        "ANO_PUBLICACAO",
        "VERSAO",
        "TIPO_VERSAO",
        "DATA_CRIACAO",
        "DATA_ASSINATURA",
        "DATA_PUBLICACAO",
        "TRAMITAVEL",
        "TIPO_ATO_ID",
        "TIPO_ATO_GRUPO",
        "STATUS_ATO_ID",
        "SITUACAO_ACAO_ID",
        "ORGAO_ID__ORIGEM",
        "NUMERO_UNICO",
        "MARCADOR",
    ]
}

fn sortable_columns() -> &'static [&'static str] {
    &["ID", "ANO_PUBLICACAO", "DATA_PUBLICACAO", "DATA_CRIACAO", "NUMERO_OFICIAL"]
}

/// Columns whose declared `smallint`/`int` type is a lie: the feed publishes the
/// *name* rather than the id (e.g. `TIPO_ATO_ID` holds "DECRETO"). They must be
/// stored as text, and the UI filters them by name. Verified against the live
/// snapshot — see the sample rows in the README.
fn text_despite_numeric_declaration() -> &'static [&'static str] {
    &["TIPO_ATO_ID", "STATUS_ATO_ID", "SITUACAO_ACAO_ID", "ORGAO_ID__ORIGEM"]
}

/// Column names the row's values map onto, in order: the 23 metadata fields
/// followed by the merged `body`.
pub fn act_column_names(schema: &Schema) -> Result<Vec<String>> {
    let table = schema
        .table("ATO")
        .context("Info.xml declares no ATO table")?;
    if table.columns.len() < parser::META_FIELDS + 2 {
        bail!(
            "ATO declares {} columns, expected at least {}",
            table.columns.len(),
            parser::META_FIELDS + 2
        );
    }
    let mut names: Vec<String> = table.columns[..parser::META_FIELDS]
        .iter()
        .map(|c| c.name.clone())
        .collect();
    // The last two declared columns are the two halves of one HTML document, so
    // they collapse into a single stored value.
    names.push("body".to_string());
    Ok(names)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRow {
    pub id: i64,
    pub numero_identificacao: Option<String>,
    pub numero_oficial: Option<String>,
    pub ano_publicacao: Option<i64>,
    pub data_publicacao: Option<String>,
    pub data_criacao: Option<String>,
    pub tipo: Option<String>,
    pub orgao: Option<String>,
    pub situacao: Option<String>,
    pub sumula: Option<String>,
    /// Matching excerpt, present only for full-text queries.
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActDetail {
    /// Every stored column, in schema order, with the schema's `help` text and
    /// resolved dimension label where the schema declares a relation.
    pub fields: Vec<FieldValue>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValue {
    pub column: String,
    pub label: String,
    pub value: Option<String>,
    /// Value of the related dimension row, when the column is a foreign key.
    pub related_value: Option<String>,
    pub related_table: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    #[default]
    Eq,
    Ne,
    Like,
    Gte,
    Lte,
}

impl Op {
    fn sql(self) -> &'static str {
        match self {
            Op::Eq => "=",
            Op::Ne => "!=",
            Op::Like => "LIKE",
            Op::Gte => ">=",
            Op::Lte => "<=",
        }
    }
    fn param(self, v: &str) -> String {
        // LIKE filters are substring matches, so they need wildcards.
        if self == Op::Like {
            format!("%{}%", v.replace('%', "\\%").replace('_', "\\_"))
        } else {
            v.to_string()
        }
    }
}

/// How a group combines its conditions.
///
/// `And` is the default so a condition sent without a logic of its own behaves as
/// it always has. `Or` exists for the "any of these" rule — a column with a
/// defined type list is usually filtered that way (Decree *or* Law).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Logic {
    And,
    Or,
}

impl Default for Logic {
    fn default() -> Self {
        Logic::And
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Empty means "no column chosen yet" — the UI's "Nova condição" state.
    pub column: String,
    /// `Eq` by default: the conservative choice, and the one the UI starts from.
    pub op: Op,
    /// The primary value. Keep it first: the frontend and the stored rule JSON
    /// are written against this name.
    pub value: String,
    /// Extra values for "is one of". Each is bound separately, so the value
    /// never reaches the SQL text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    /// Router for this condition against the previous one. `And` when omitted.
    #[serde(default)]
    pub logic: Logic,
}

impl Filter {
    /// Every value this condition should match: `value`, then `values`, with the
    /// blanks dropped and duplicates removed. `values` is checked first so tests
    /// (and any caller) can build a multi-value condition without knowing which
    /// field the reader prefers.
    fn all_values(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for v in self.values.iter().chain(std::iter::once(&self.value)) {
            let t = v.trim();
            if !t.is_empty() && !out.contains(&t) {
                out.push(t);
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActQuery {
    /// Full-text query. When set, results are ordered by relevance unless `sort`
    /// is given explicitly.
    pub text: Option<String>,
    pub filters: Vec<Filter>,
    /// Only acts whose `ID` is strictly above this, used to restrict a query to
    /// what a sync just appended. `ID` is not exposed as a filterable column —
    /// that list is UI-facing — so the floor is its own field.
    #[serde(default)]
    pub min_id: Option<i64>,
    pub sort: Option<String>,
    pub ascending: bool,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub rows: Vec<ActRow>,
    pub total: i64,
}

/// One entry of a column's defined type list: the stored `value` and the label
/// the UI shows for it (a decoded meaning, or the dimension's `NOME`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnValue {
    pub value: String,
    pub label: String,
}

/// A value and its frequency, for the filter dropdowns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facet {
    pub value: String,
    pub count: i64,
}

/// The revision a snapshot file was last imported at.
///
/// Only the validators are kept: a refresh is skipped when the remote file still
/// reports the same size and ETag as the recorded revision.
#[derive(Debug, Clone, Default)]
pub struct SnapshotState {
    /// Size in bytes of the file that was imported.
    pub size: u64,
    pub etag: Option<String>,
}

/// Return the database's free pages to the file.
///
/// `PRAGMA incremental_vacuum` yields **one row per page** it releases and only
/// does that page's work as the row is stepped, so it has to be drained. Run once
/// — which is all `execute` or `execute_batch` do — it frees exactly one page,
/// leaving the rest of a dropped staging table's 1.4 GB in the file as a
/// freelist. Stepping every row through to the end is what actually empties it.
fn purge_free_pages(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA incremental_vacuum")?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating data dir {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // Incremental auto-vacuum is what lets the staging table's space return
        // to the file once the import drops it. Without it, `DROP TABLE` only
        // moves those pages to a freelist and the file keeps a second copy of the
        // snapshot for good. A fresh database takes the setting directly; one
        // created before it existed needs a single VACUUM to rewrite its header,
        // which is also what reclaims whatever an earlier run already stranded —
        // so this is a one-time cost, and only when the setting is still off.
        let auto_vacuum: i64 = conn.query_row("PRAGMA auto_vacuum", [], |r| r.get(0))?;
        if auto_vacuum == 0 {
            if let Err(e) = conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;") {
                log::warn!("enabling incremental auto-vacuum failed: {e:#}");
            }
        }

        // The post-import checkpoint that shrinks the write-ahead log must wait
        // for a brief reader instead of giving up on the first busy moment;
        // without this the log keeps its peak size.
        conn.busy_timeout(std::time::Duration::from_secs(10))?;

        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            -- The snapshot load is one big transaction; a large page cache keeps
            -- it from thrashing on a 1.4 GB ingest.
            PRAGMA cache_size = -262144;
            PRAGMA temp_store = MEMORY;
            "#,
        )?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS rsync_files (
                path  TEXT PRIMARY KEY,
                size  INTEGER NOT NULL,
                mtime TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_rsync_files_mtime ON rsync_files(mtime DESC);

            -- One row per file we ingest from the feed, with the byte offset we
            -- stopped at so a later refresh can fetch only the tail.
            CREATE TABLE IF NOT EXISTS snapshots (
                file_name     TEXT PRIMARY KEY,
                size          INTEGER NOT NULL,
                etag          TEXT,
                last_modified TEXT,
                byte_offset   INTEGER NOT NULL DEFAULT 0,
                last_id       INTEGER,
                row_count     INTEGER NOT NULL DEFAULT 0,
                updated_at    TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS schema_doc (
                id         INTEGER PRIMARY KEY CHECK (id = 1),
                fetched_at TEXT NOT NULL,
                xml        TEXT NOT NULL
            );

            -- A staging table left behind by a crash is never merged; drop it so
            -- `begin_import` always starts from a clean slate.
            DROP TABLE IF EXISTS acts_import;
            "#,
        )
        .context("creating base schema")?;
        // The email notification tables share this file and connection. Creating
        // them here keeps a single startup path (and covers `open_in_memory`, so
        // the email tests get the schema for free).
        crate::email::init_schema(&conn)?;

        // A crashed run can leave a dropped-but-unreclaimed staging table (a
        // freelist) or a large write-ahead log behind; with nothing else attached
        // yet, this is a good moment to give that space back.
        if let Err(e) = purge_free_pages(&conn) {
            log::warn!("initial purge of free pages failed: {e:#}");
        }
        if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
            log::warn!("initial WAL truncate failed: {e:#}");
        }
        Ok(Store { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // ---------------------------------------------------------------- schema

    /// Create the act table and indexes from the feed's schema.
    ///
    /// Idempotent: existing tables and indexes are left alone, so this is safe to
    /// run on every start.
    pub fn ensure_act_tables(&self, schema: &Schema) -> Result<()> {
        let names = act_column_names(schema)?;
        let table = schema.table("ATO").unwrap();

        let mut cols: Vec<String> = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let sql_type = if i == 0 {
                // `ID` is the primary key; every record in the snapshot has one.
                "INTEGER PRIMARY KEY".to_string()
            } else if name == "body" {
                "TEXT".to_string()
            } else {
                let declared = &table.columns[i].sql_type;
                let numeric = matches!(
                    declared.as_str(),
                    "int" | "smallint" | "tinyint" | "bit"
                );
                if numeric && !text_despite_numeric_declaration().contains(&name.as_str()) {
                    // INTEGER affinity: still stores text unchanged if a value
                    // will not convert, which keeps this safe for dirty exports.
                    "INTEGER".to_string()
                } else {
                    "TEXT".to_string()
                }
            };
            cols.push(format!("\"{name}\" {sql_type}"));
        }

        self.conn
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS {ACTS} (\n  {}\n);",
                cols.join(",\n  ")
            ))
            .context("creating the acts table")?;

        // Indexes for the filters the UI offers, plus the default sort.
        for col in [
            "DATA_PUBLICACAO",
            "ANO_PUBLICACAO",
            "TIPO_ATO_ID",
            "ORGAO_ID__ORIGEM",
            "SITUACAO_ACAO_ID",
            "STATUS_ATO_ID",
            "NUMERO_OFICIAL",
        ] {
            self.conn.execute_batch(&format!(
                "CREATE INDEX IF NOT EXISTS \"idx_acts_{col}\" ON {ACTS}(\"{col}\");"
            ))?;
        }

        // External-content FTS5: the text lives once, in `acts`, and the index
        // references it. `ID` is an INTEGER PRIMARY KEY, so it is the rowid.
        self.conn
            .execute_batch(&format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {FTS} USING fts5(
                    NUMERO_IDENTIFICACAO, NUMERO_OFICIAL, SUMULA, CABECALHO, body,
                    content='{ACTS}', content_rowid='ID',
                    tokenize=\"unicode61 remove_diacritics 2\"
                );"
            ))
            .context("creating the full-text index")?;

        Ok(())
    }

    pub fn save_schema_doc(&self, xml: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO schema_doc (id, fetched_at, xml) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET fetched_at = excluded.fetched_at, xml = excluded.xml",
            params![chrono::Utc::now().to_rfc3339(), xml],
        )?;
        Ok(())
    }

    pub fn load_schema(&self) -> Result<Option<Schema>> {
        let xml: Option<String> = self
            .conn
            .query_row("SELECT xml FROM schema_doc WHERE id = 1", [], |r| r.get(0))
            .optional()?;
        match xml {
            Some(x) => Ok(Some(Schema::parse(x.as_bytes())?)),
            None => Ok(None),
        }
    }

    // ----------------------------------------------------------- rsync index

    pub fn replace_rsync_files(&mut self, files: &[crate::feed::RemoteFile]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut up = tx.prepare(
                "INSERT INTO rsync_files (path, size, mtime) VALUES (?1, ?2, ?3)
                 ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime",
            )?;
            for f in files {
                up.execute(params![f.path, f.size, f.mtime])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn rsync_file_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM rsync_files", [], |r| r.get(0))?)
    }

    // -------------------------------------------------------- snapshot state

    /// The revision a snapshot was last imported at.
    ///
    /// Only the validators matter now: if the remote file still has the same size
    /// and ETag, the import is skipped. The stored offset is kept as "the size we
    /// imported through", for continuity with the `snapshots` table.
    pub fn snapshot_state(&self, file_name: &str) -> Result<Option<SnapshotState>> {
        let row = self
            .conn
            .query_row(
                "SELECT size, etag FROM snapshots WHERE file_name = ?1",
                params![file_name],
                |r| {
                    Ok(SnapshotState {
                        size: r.get::<_, i64>(0)? as u64,
                        etag: r.get(1)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Record the revision of a snapshot that was imported in full.
    ///
    /// Called only after the merge commits: a failed run leaves the previous
    /// revision in place, so the next refresh retries instead of skipping it.
    #[allow(clippy::too_many_arguments)]
    pub fn record_snapshot(
        &self,
        file_name: &str,
        size: u64,
        etag: Option<&str>,
        last_modified: Option<&str>,
        last_id: Option<i64>,
        row_count: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO snapshots
                (file_name, size, etag, last_modified, byte_offset, last_id, row_count, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?2, ?5, ?6, ?7)
             ON CONFLICT(file_name) DO UPDATE SET
                size = excluded.size, etag = excluded.etag,
                last_modified = excluded.last_modified, byte_offset = excluded.byte_offset,
                last_id = excluded.last_id, row_count = excluded.row_count,
                updated_at = excluded.updated_at",
            params![
                file_name,
                size as i64,
                etag,
                last_modified,
                last_id,
                row_count,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn max_act_id(&self) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT MAX(\"ID\") FROM {ACTS}"), [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .optional()?
            .flatten())
    }

    pub fn act_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {ACTS}"), [], |r| r.get(0))?)
    }

    // ------------------------------------------------------------- ingesting

    /// Create an empty staging table shaped like the act table.
    ///
    /// `DROP` first: a run interrupted after this point must never have its
    /// half-loaded rows merged, so the staging table is always rebuilt from
    /// scratch. `AS SELECT … WHERE 0` copies the act columns — and their storage
    /// affinities — without bringing any rows across, so the two always agree.
    pub fn begin_import(&mut self) -> Result<()> {
        self.conn
            .execute_batch(&format!(
                "DROP TABLE IF EXISTS {IMPORT};
                 CREATE TABLE {IMPORT} AS SELECT * FROM {ACTS} WHERE 0;"
            ))
            .context("creating the staging table")?;
        Ok(())
    }

    /// Copy the staged rows the act table does not already hold.
    ///
    /// `ID` is the merge key: only rows above the stored maximum are new. The
    /// filter runs in SQL, and the same predicate drives the index update, so the
    /// act table and its full-text index can never disagree about which rows
    /// arrived. `?1 IS NULL` covers the first import, when there is no floor yet.
    ///
    /// Copied in batches, not one statement: the first import moves ~2.5 GB (the
    /// acts plus their index), and doing that in a single transaction grows the
    /// write-ahead log to the size of the database, where it then sits. Chunking
    /// keeps each transaction small, so a checkpoint can keep the log short.
    pub fn merge_import(&mut self) -> Result<i64> {
        let quoted = self
            .columns_of(ACTS)?
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");

        let mut added = 0i64;
        loop {
            // Recompute the floor each pass: committing a batch moves the
            // maximum, so the next `ID > floor` continues where this one stopped.
            let floor = self.max_act_id()?;
            let tx = self.conn.transaction()?;
            let n = tx.execute(
                &format!(
                    "INSERT INTO {ACTS} ({quoted})
                     SELECT {quoted} FROM {IMPORT}
                     WHERE ?1 IS NULL OR \"ID\" > ?1 ORDER BY \"ID\" LIMIT ?2"
                ),
                params![floor, MERGE_BATCH],
            )?;
            if n == 0 {
                tx.commit()?;
                break;
            }
            // Same predicate, so the index receives exactly the rows just copied.
            tx.execute(
                &format!(
                    "INSERT INTO {FTS} (rowid, NUMERO_IDENTIFICACAO, NUMERO_OFICIAL, SUMULA, CABECALHO, body)
                     SELECT \"ID\", \"NUMERO_IDENTIFICACAO\", \"NUMERO_OFICIAL\", \"SUMULA\", \"CABECALHO\", \"body\"
                     FROM {IMPORT}
                     WHERE ?1 IS NULL OR \"ID\" > ?1 ORDER BY \"ID\" LIMIT ?2"
                ),
                params![floor, MERGE_BATCH],
            )?;
            tx.commit()?;
            added += n as i64;
        }
        Ok(added)
    }

    /// Drop the staging table once its rows have been merged, and give its space
    /// back to the file.
    ///
    /// `DROP TABLE` on its own only moves the pages to the freelist — the file
    /// keeps them, which is why a 1.4 GB snapshot load used to leave a second copy
    /// behind. [`purge_free_pages`] is what actually releases them, and the
    /// checkpoint then shrinks the write-ahead log, which otherwise keeps its peak
    /// size. Both are best-effort: a concurrent read can make them back off with
    /// `SQLITE_BUSY`, and the next import (or the next start) retries.
    pub fn finish_import(&mut self) -> Result<()> {
        self.conn
            .execute_batch(&format!("DROP TABLE IF EXISTS {IMPORT};"))
            .context("dropping the staging table")?;
        if let Err(e) = self.reclaim_after_import() {
            log::warn!("reclaiming space after import failed: {e:#}");
        }
        Ok(())
    }

    /// Release the freed staging pages, then shrink the write-ahead log.
    fn reclaim_after_import(&self) -> Result<()> {
        purge_free_pages(&self.conn)?;
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Insert a batch of parsed rows into `table`, either the act table or the
    /// staging table.
    ///
    /// `fts` mirrors the rows into the full-text index, which only the act table
    /// needs: the staging table is indexed once, by [`merge_import`], for exactly
    /// the rows it copies. Uses plain INSERT, not upsert — an external-content FTS
    /// index cannot be updated in place without the previous column values, which
    /// this feed does not retain.
    fn insert_into(&mut self, table: &str, rows: &[Row], fts: bool) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let cols = self.columns_of(table)?;
        let placeholders = (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let quoted = cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");

        let tx = self.conn.transaction()?;
        {
            let mut ins = tx.prepare(&format!(
                "INSERT INTO {table} ({quoted}) VALUES ({placeholders})"
            ))?;
            let mut fts_stmt = if fts {
                Some(tx.prepare(&format!(
                    "INSERT INTO {FTS} (rowid, NUMERO_IDENTIFICACAO, NUMERO_OFICIAL, SUMULA, CABECALHO, body)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ))?)
            } else {
                None
            };

            for row in rows {
                if row.values.len() != cols.len() {
                    bail!(
                        "row has {} values but the table has {} columns",
                        row.values.len(),
                        cols.len()
                    );
                }
                let owned: Vec<Box<dyn ToSql>> = row
                    .values
                    .iter()
                    .map(|v| -> Box<dyn ToSql> {
                        match v {
                            Some(s) => Box::new(s.clone()),
                            None => Box::new(rusqlite::types::Null),
                        }
                    })
                    .collect();
                let refs: Vec<&dyn ToSql> = owned.iter().map(|b| b.as_ref()).collect();
                ins.execute(params_from_iter(refs))?;

                // Mirror the searchable text into the index. Missing columns are
                // simply NULL, which FTS treats as absent.
                if let Some(fts) = fts_stmt.as_mut() {
                    let at = |name: &str| -> Option<&String> {
                        cols.iter().position(|c| c == name).and_then(|i| row.values[i].as_ref())
                    };
                    fts.execute(params![
                        row.id(),
                        at("NUMERO_IDENTIFICACAO"),
                        at("NUMERO_OFICIAL"),
                        at("SUMULA"),
                        at("CABECALHO"),
                        at("body"),
                    ])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Insert a batch of parsed rows into the act table and its index.
    pub fn insert_rows(&mut self, rows: &[Row]) -> Result<()> {
        self.insert_into(ACTS, rows, true)
    }

    /// Insert a batch of parsed rows into the staging table. The full-text index
    /// is left alone here; [`merge_import`] indexes exactly the rows it copies.
    pub fn stage_rows(&mut self, rows: &[Row]) -> Result<()> {
        self.insert_into(IMPORT, rows, false)
    }

    /// Read a column out of a row as text, whatever storage class it holds.
    ///
    /// The detail view shows every column of the table it just created, and that
    /// table declares `ID` (and a few others) as INTEGER. Asking rusqlite for a
    /// `String` there is an error, not a coercion, so the column type has to be
    /// dispatched rather than assumed.
    fn text_at(row: &rusqlite::Row<'_>, i: usize) -> rusqlite::Result<Option<String>> {
        use rusqlite::types::ValueRef::*;
        Ok(match row.get_ref(i)? {
            Null => None,
            Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
            Integer(n) => Some(n.to_string()),
            Real(f) => Some(f.to_string()),
            Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
        })
    }

    fn columns_of(&self, table: &str) -> Result<Vec<String>> {
        let stmt = self.conn.prepare(&format!("SELECT * FROM {table} LIMIT 0"))?;
        let cols = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        Ok(cols)
    }

    fn act_columns(&self) -> Result<Vec<String>> {
        self.columns_of(ACTS)
    }

    /// Replace a small lookup table (ORGAO, TIPO_ATO, ...) wholesale.
    pub fn replace_dimension(&mut self, table: &str, header: &[String], rows: &[Vec<Option<String>>]) -> Result<()> {
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            bail!("unsafe dimension table name {table:?}");
        }
        let tx = self.conn.transaction()?;
        let cols = header
            .iter()
            .map(|c| format!("\"{c}\" TEXT"))
            .collect::<Vec<_>>()
            .join(", ");
        tx.execute_batch(&format!("DROP TABLE IF EXISTS \"{table}\"; CREATE TABLE \"{table}\" ({cols});"))?;
        {
            let ph = (1..=header.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let quoted = header
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let mut ins = tx.prepare(&format!("INSERT INTO \"{table}\" ({quoted}) VALUES ({ph})"))?;
            for r in rows {
                let owned: Vec<Box<dyn ToSql>> = r
                    .iter()
                    .map(|v| -> Box<dyn ToSql> {
                        match v {
                            Some(s) => Box::new(s.clone()),
                            None => Box::new(rusqlite::types::Null),
                        }
                    })
                    .collect();
                let refs: Vec<&dyn ToSql> = owned.iter().map(|b| b.as_ref()).collect();
                ins.execute(params_from_iter(refs))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // --------------------------------------------------------------- queries

    fn build_where(&self, q: &ActQuery) -> Result<(String, Vec<Box<dyn ToSql>>, bool)> {
        let mut clauses: Vec<String> = Vec::new();
        let mut binds: Vec<Box<dyn ToSql>> = Vec::new();

        let has_text = q.text.as_deref().is_some_and(|t| !t.trim().is_empty());
        if let Some(text) = q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            // Matching through the joined FTS table (rather than an `IN (SELECT
            // rowid ...)` subquery) is what makes `rank` and `snippet()` available
            // in the outer query — both only have meaning alongside a MATCH.
            clauses.push(format!("{FTS} MATCH ?{}", binds.len() + 1));
            // Route raw user input through quoted phrases so FTS5 operators typed
            // by accident (`AND`, `*`, `-`) are treated as literal text.
            binds.push(Box::new(full_text_query(text)));
        }

        // Conditions are grouped by their own logic, and the groups are joined with
        // AND. AND is associative, so the result is exactly "every group holds" —
        // no nesting needed, and it matches the flat editor the UI offers.
        //
        // Negated conditions are emitted first so that "não é DECRETO" becomes
        // `NOT (col = ?)` rather than `col != ?`: a column that is null compares
        // as unknown and would be dropped by `!=`, but `NOT (col = ?)` keeps it.
        let mut order: Vec<(usize, &Filter)> = q.filters.iter().enumerate().collect();
        order.sort_by_key(|(i, f)| (f.op != Op::Ne, *i));

        // Each entry is one AND-group: the OR-ed terms it contains.
        let mut groups: Vec<Vec<String>> = Vec::new();

        for (_, f) in order {
            if !filterable_columns().contains(&f.column.as_str()) {
                bail!("column {:?} is not filterable", f.column);
            }
            let esc = if matches!(f.op, Op::Like) { " ESCAPE '\\'" } else { "" };
            let values = f.all_values();
            if values.is_empty() {
                continue; // a condition with no value constrains nothing
            }

            // The SQL for this one condition. Each value is bound separately, so
            // neither the column nor the values reach the SQL text.
            let term = match f.op {
                // "is one of", the SQL form of a multi-value equality. The ORs
                // must be parenthesised: this term is joined to the next clause
                // (including the `ID > floor` boundary) with AND, and AND binds
                // tighter than OR — without the parentheses only the last value
                // would respect the floor while the rest matched the whole table.
                Op::Eq => {
                    let mut any = Vec::new();
                    for v in &values {
                        any.push(format!("a.\"{}\" = ?{}", f.column, binds.len() + 1));
                        binds.push(Box::new(v.to_string()));
                    }
                    if any.len() == 1 {
                        any[0].clone()
                    } else {
                        format!("({})", any.join(" OR "))
                    }
                }
                // "is none of": keeps rows whose column holds none of the values.
                Op::Ne => {
                    let mut any = Vec::new();
                    for v in &values {
                        any.push(format!("a.\"{}\" = ?{}", f.column, binds.len() + 1));
                        binds.push(Box::new(v.to_string()));
                    }
                    format!("NOT ({})", any.join(" OR "))
                }
                // The ordering operators take a single value.
                _ => {
                    let v = values[0];
                    // `esc` trails the placeholder, as SQL requires.
                    let sql = format!(
                        "a.\"{}\" {} ?{}{esc}",
                        f.column,
                        f.op.sql(),
                        binds.len() + 1
                    );
                    binds.push(Box::new(f.op.param(v)));
                    sql
                }
            };

            // OR continues the group in progress; AND opens a new one.
            match (f.logic, groups.last_mut()) {
                (Logic::Or, Some(last)) => last.push(term),
                _ => groups.push(vec![term]),
            }
        }

        // A one-term group needs no parentheses; a multi-term one does, or its OR
        // would leak into the ANDs that join the groups.
        for terms in groups {
            clauses.push(if terms.len() == 1 {
                terms[0].clone()
            } else {
                format!("({})", terms.join(" OR "))
            });
        }

        // The "newly appended" floor. Bound like any other value; `a."ID"` is the
        // INTEGER PRIMARY KEY (rowid), and `from_clause` always aliases `acts` as `a`.
        if let Some(floor) = q.min_id {
            clauses.push(format!("a.\"ID\" > ?{}", binds.len() + 1));
            binds.push(Box::new(floor));
        }

        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        Ok((where_sql, binds, has_text))
    }

    /// `acts` alone, or `acts` joined to its full-text index when a text query is
    /// present. The join is what exposes `rank` and `snippet()`.
    ///
    /// The alias `a` is not cosmetic: `acts_fts` declares columns with the same
    /// names as `acts` (`NUMERO_IDENTIFICACAO`, `SUMULA`, ...), so an unqualified
    /// reference becomes ambiguous the moment the two are joined.
    fn from_clause(has_text: bool) -> String {
        if has_text {
            format!("{ACTS} a JOIN {FTS} ON {FTS}.rowid = a.\"ID\"")
        } else {
            format!("{ACTS} a")
        }
    }

    pub fn search(&self, q: &ActQuery) -> Result<SearchResult> {
        let (where_sql, binds, has_text) = self.build_where(q)?;
        let from = Self::from_clause(has_text);

        // COUNT(*) with the join is still exact here: `acts_fts` has one row per
        // act, so it cannot fan out duplicates.
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM {from}{where_sql}"),
            params_from_iter(binds.iter().map(|b| b.as_ref())),
            |r| r.get(0),
        )?;

        let sort = match q.sort.as_deref() {
            Some(s) => {
                if !sortable_columns().contains(&s) {
                    bail!("column {s:?} is not sortable");
                }
                s.to_string()
            }
            // With a text query and no explicit sort, relevance is the useful order.
            None => {
                if has_text {
                    "rank".to_string()
                } else {
                    "DATA_PUBLICACAO".to_string()
                }
            }
        };
        let dir = if q.ascending { "ASC" } else { "DESC" };

        let order = if sort == "rank" {
            // Only available via the join, and only meaningful with a MATCH.
            // FTS5 `rank` is negative, so ascending puts the best match first.
            format!("{FTS}.rank ASC, a.\"DATA_PUBLICACAO\" DESC")
        } else {
            format!("a.\"{sort}\" {dir}, a.\"ID\" {dir}")
        };

        // -1 selects the column with the best match; only valid alongside MATCH.
        let snippet = if has_text {
            format!("snippet({FTS}, -1, '<mark>', '</mark>', '…', 14)")
        } else {
            "NULL".to_string()
        };

        let sql = format!(
            "SELECT a.\"ID\", a.\"NUMERO_IDENTIFICACAO\", a.\"NUMERO_OFICIAL\", a.\"ANO_PUBLICACAO\",
                    a.\"DATA_PUBLICACAO\", a.\"DATA_CRIACAO\", a.\"TIPO_ATO_ID\", a.\"ORGAO_ID__ORIGEM\",
                    a.\"SITUACAO_ACAO_ID\", a.\"SUMULA\", {snippet}
             FROM {from}{where_sql} ORDER BY {order} LIMIT ?{} OFFSET ?{}",
            binds.len() + 1,
            binds.len() + 2
        );

        let mut all: Vec<Box<dyn ToSql>> = binds;
        all.push(Box::new(q.limit.max(1)));
        all.push(Box::new(q.offset.max(0)));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(all.iter().map(|b| b.as_ref())), |r| {
                Ok(ActRow {
                    id: r.get(0)?,
                    numero_identificacao: r.get(1)?,
                    numero_oficial: r.get(2)?,
                    ano_publicacao: r.get(3)?,
                    data_publicacao: r.get(4)?,
                    data_criacao: r.get(5)?,
                    tipo: r.get(6)?,
                    orgao: r.get(7)?,
                    situacao: r.get(8)?,
                    sumula: r.get(9)?,
                    snippet: r.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SearchResult { rows, total })
    }

    /// One act, in the same shape [`Store::search`] returns, for a single id.
    ///
    /// `ID` is not a filterable column — that list is UI-facing — so a search
    /// cannot pick out exactly one act; the on-demand send needs the row itself.
    /// The projection mirrors `search`'s so an act read here matches what a
    /// rule's matches carry.
    pub fn act_row(&self, id: i64) -> Result<Option<ActRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT a.\"ID\", a.\"NUMERO_IDENTIFICACAO\", a.\"NUMERO_OFICIAL\", a.\"ANO_PUBLICACAO\",
                    a.\"DATA_PUBLICACAO\", a.\"DATA_CRIACAO\", a.\"TIPO_ATO_ID\", a.\"ORGAO_ID__ORIGEM\",
                    a.\"SITUACAO_ACAO_ID\", a.\"SUMULA\"
             FROM {ACTS} a WHERE a.\"ID\" = ?1"
        ))?;
        let mut rows = stmt.query(params![id])?;
        let Some(r) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(ActRow {
            id: r.get(0)?,
            numero_identificacao: r.get(1)?,
            numero_oficial: r.get(2)?,
            ano_publicacao: r.get(3)?,
            data_publicacao: r.get(4)?,
            data_criacao: r.get(5)?,
            tipo: r.get(6)?,
            orgao: r.get(7)?,
            situacao: r.get(8)?,
            sumula: r.get(9)?,
            snippet: None,
        }))
    }

    /// Distinct values with counts, for a filter dropdown.
    pub fn facet(&self, column: &str, limit: i64) -> Result<Vec<Facet>> {
        if !filterable_columns().contains(&column) {
            bail!("column {column:?} is not filterable");
        }
        let mut stmt = self.conn.prepare(&format!(
            "SELECT \"{column}\", COUNT(*) FROM {ACTS}
             WHERE \"{column}\" IS NOT NULL AND \"{column}\" != ''
             GROUP BY \"{column}\" ORDER BY COUNT(*) DESC LIMIT ?1"
        ))?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok(Facet {
                    value: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The values a column can take, according to the schema, with a label to show.
    ///
    /// Two kinds of "defined type" meet here, and they are kept apart because they
    /// are different claims:
    ///
    /// * A column the schema restricts with `<Values>` has a *closed* list. Every
    ///   legal value is known up front, so the list is exactly that.
    /// * A foreign key (`<Related table=… key=…>`) points at a lookup table the
    ///   feed publishes. Its rows are the vocabulary; the label is the dimension's
    ///   `NOME`, the value the key the acts store.
    ///
    /// `None` means "no defined type" — a free column, where the UI offers a text
    /// field. A dimension that has not been ingested yet falls back to the values
    /// actually present in the acts, so the picker is never empty when it can help.
    pub fn column_values(
        &self,
        schema: &Schema,
        column: &str,
        limit: i64,
    ) -> Result<Option<Vec<ColumnValue>>> {
        if !filterable_columns().contains(&column) {
            bail!("column {column:?} is not filterable");
        }
        let Some(decl) = schema
            .table("ATO")
            .and_then(|t| t.columns.iter().find(|c| c.name == column))
        else {
            return Ok(None);
        };

        if !decl.values.is_empty() {
            let mut out: Vec<ColumnValue> = decl
                .values
                .iter()
                // `description` is the label the dictionary gives the value; when it
                // is absent (or just repeats the raw value) the value speaks for
                // itself. `help` is the longer prose, kept as a hint only.
                .map(|v| ColumnValue {
                    value: v.value.clone(),
                    label: if v.description.trim().is_empty() {
                        v.value.clone()
                    } else {
                        v.description.clone()
                    },
                })
                .collect();
            out.truncate(limit.max(1) as usize);
            return Ok(Some(out));
        }

        let Some(rel) = decl.related.as_ref() else {
            return Ok(None);
        };

        // The dimension CSV is small and loaded wholesale; `NOME` is its label
        // column in this feed. A missing table (not ingested yet) is not an error.
        // `key -> NOME` labels the values offered below.
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![rel.table],
            |r| r.get(0),
        )?;
        let safe = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let mut names: Vec<(String, String)> = Vec::new();
        if exists && safe(&rel.table) && safe(&rel.key) {
            let sql = format!(
                "SELECT \"{key}\", \"NOME\" FROM \"{table}\" WHERE \"{key}\" IS NOT NULL
                 ORDER BY \"NOME\" LIMIT ?1",
                key = rel.key,
                table = rel.table
            );
            let mut stmt = self.conn.prepare(&sql)?;
            names = stmt
                .query_map(params![limit.max(1)], |r| {
                    let value: String = r.get(0)?;
                    let nome: Option<String> = r.get(1)?;
                    Ok((
                        value,
                        nome.unwrap_or_default(),
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
        }

        // The offered value must be exactly what the search will bind. The schema
        // declares this a foreign key — `TIPO_ATO.ID`, a smallint — but this feed
        // stores the *name* in the act row (see the `denormalised` note above), so
        // the acts' own values are the vocabulary; the dimension only supplies
        // labels. Offering the key here would produce rules that match nothing.
        //
        // An empty acts column (no data loaded yet) falls back to the dimension
        // keys, which is still a usable list and matches once acts arrive holding
        // those values.
        let facets = self.facet(column, limit)?;
        let label_for = |value: &str| -> Option<&str> {
            names
                .iter()
                .find(|(k, n)| !n.is_empty() && (k == value || n == value))
                .map(|(_, n)| n.as_str())
        };

        if !facets.is_empty() {
            return Ok(Some(
                facets
                    .into_iter()
                    .map(|f| ColumnValue {
                        label: label_for(&f.value)
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("{} ({})", f.value, f.count)),
                        value: f.value,
                    })
                    .collect(),
            ));
        }

        if !names.is_empty() {
            return Ok(Some(
                names
                    .into_iter()
                    .map(|(value, nome)| ColumnValue {
                        label: if nome.trim().is_empty() { value.clone() } else { nome },
                        value,
                    })
                    .collect(),
            ));
        }

        // Nothing to offer: report "no list" so the UI shows a text field rather
        // than a select with nothing in it.
        Ok(None)
    }

    /// Every stored field of one act, labelled from the schema.
    pub fn act_detail(&self, schema: &Schema, id: i64) -> Result<Option<ActDetail>> {
        let cols = self.act_columns()?;
        let quoted = cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {quoted} FROM {ACTS} WHERE \"ID\" = ?1"))?;

        let mut rows = stmt.query(params![id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        let table = schema.table("ATO");
        let mut fields = Vec::new();
        for (i, name) in cols.iter().enumerate() {
            let raw: Option<String> = Self::text_at(row, i)?;
            if name == "body" {
                continue;
            }
            let decl = table.and_then(|t| t.columns.iter().find(|c| &c.name == name));
            let (related_value, related_table) = match decl.and_then(|c| c.related.as_ref()) {
                Some(rel) => (self.lookup_dimension(&rel.table, &rel.key, raw.as_deref())?, Some(rel.table.clone())),
                None => (None, None),
            };
            fields.push(FieldValue {
                column: name.clone(),
                label: decl
                    .and_then(|c| c.help.clone())
                    .unwrap_or_else(|| name.clone()),
                value: raw,
                related_value,
                related_table,
            });
        }

        let body_idx = cols.iter().position(|c| c == "body").unwrap();
        let body: Option<String> = row.get(body_idx)?;

        Ok(Some(ActDetail { fields, body }))
    }

    /// Resolve a foreign key against a loaded dimension table.
    fn lookup_dimension(&self, table: &str, key: &str, value: Option<&str>) -> Result<Option<String>> {
        let Some(v) = value else { return Ok(None) };
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Ok(None);
        }
        // The dimension is a separate CSV in the feed, so a detail page can be
        // opened before that one has been ingested. A missing table is "no
        // resolution available", not a failure of the whole lookup.
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(None);
        }

        let sql = format!("SELECT \"NOME\" FROM \"{table}\" WHERE \"{key}\" = ?1 LIMIT 1");
        let out = self
            .conn
            .query_row(&sql, params![v], |r| r.get::<_, Option<String>>(0))
            .optional()?;
        Ok(out.flatten())
    }

    /// Counts for the dashboard tiles.
    pub fn stats(&self) -> Result<StoreStats> {
        let mut s = StoreStats::default();
        if self.act_count().unwrap_or(0) == 0 {
            return Ok(s);
        }
        s.total = self.act_count()?;
        s.newest_publication = self
            .conn
            .query_row(
                &format!("SELECT MAX(\"DATA_PUBLICACAO\") FROM {ACTS}"),
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);
        s.oldest_publication = self
            .conn
            .query_row(
                &format!("SELECT MIN(\"DATA_PUBLICACAO\") FROM {ACTS}"),
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);
        s.newest_created = self
            .conn
            .query_row(
                &format!("SELECT MAX(\"DATA_CRIACAO\") FROM {ACTS}"),
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);
        s.with_body = self
            .conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {ACTS} WHERE body IS NOT NULL AND body != ''"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(s)
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StoreStats {
    pub total: i64,
    pub with_body: i64,
    pub newest_publication: Option<String>,
    pub oldest_publication: Option<String>,
    pub newest_created: Option<String>,
}

/// Turn user input into an FTS5 query that cannot fail on operator characters.
///
/// Each whitespace-separated token becomes a quoted phrase, and the terms are
/// AND-ed. `remove_diacritics 2` in the tokenizer means "publicacao" also matches
/// "publicação", which is what a Portuguese-language user expects.
fn full_text_query(input: &str) -> String {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .filter(|t| t.len() > 2)
        .collect();
    if terms.is_empty() {
        // Nothing usable (e.g. a single character): match everything rather than
        // handing FTS5 an empty MATCH, which is a syntax error.
        "*".to_string()
    } else {
        terms.join(" AND ")
    }
}

/// `rusqlite`'s `OptionalExtension` for query_row.
use rusqlite::OptionalExtension;

/// The ATO schema as a test fixture: the 23 live metadata columns the feed
/// publishes, then the two halves of the body that fold into one stored value.
///
/// Shared by the storage and ingest suites so both build their tables from the
/// same shape — and, importantly, from the feed's real column names, which the
/// full-text index references by name.
#[cfg(test)]
pub(crate) fn test_schema() -> Schema {
    Schema::parse(
        r#"<?xml version="1.0"?>
<Info version="2.0.0"><Tables provider="MSSQL">
<Table name="ATO"><Columns>
  <Column index="01" name="ID" type="int" nullable="false" help="Chave"></Column>
  <Column index="02" name="NUMERO_IDENTIFICACAO" type="varchar" size="20" nullable="false" help="Número de identificação do Ato"></Column>
  <Column index="03" name="NUMERO_OFICIAL" type="varchar" size="20" nullable="true" help="Número oficial do Ato"></Column>
  <Column index="04" name="DIARIO_OFICIAL" type="smallint" nullable="true"></Column>
  <Column index="05" name="SUPLEMENTO" type="smallint" nullable="true"></Column>
  <Column index="06" name="ANO_PUBLICACAO" type="smallint" nullable="true"></Column>
  <Column index="07" name="VERSAO" type="tinyint" nullable="false"></Column>
  <Column index="08" name="TIPO_VERSAO" type="tinyint" nullable="false"></Column>
  <Column index="09" name="DATA_CRIACAO" type="datetime" nullable="false"></Column>
  <Column index="10" name="DATA_ASSINATURA" type="datetime" nullable="true"></Column>
  <Column index="11" name="DATA_PUBLICACAO" type="datetime" nullable="true"></Column>
  <Column index="12" name="DATA_REPUBLICACAO" type="datetime" nullable="true"></Column>
  <Column index="13" name="TRAMITAVEL" type="bit" nullable="false"></Column>
  <Column index="14" name="TIPO_ATO_ID" type="smallint" nullable="false" help="Id do Tipo do Ato"><Related table="TIPO_ATO" key="ID" /></Column>
  <Column index="15" name="TIPO_ATO_GRUPO" type="varchar" nullable="true"></Column>
  <Column index="16" name="STATUS_ATO_ID" type="smallint" nullable="false" help="Id do status"><Related table="STATUS_ATO" key="ID" /></Column>
  <Column index="17" name="SITUACAO_ACAO_ID" type="smallint" nullable="false" help="Id da situação"><Related table="SITUACAO_ACAO" key="ID" /></Column>
  <Column index="18" name="ORGAO_ID__ORIGEM" type="smallint" nullable="false" help="Órgão de origem"><Related table="ORGAO" key="ID" /></Column>
  <Column index="19" name="NUMERO_UNICO" type="int" nullable="false"></Column>
  <Column index="20" name="TEXTO_REPUBLICACAO" type="varchar" nullable="true"></Column>
  <Column index="21" name="MARCADOR" type="varchar" nullable="true"></Column>
  <Column index="22" name="SUMULA" type="varchar" nullable="false"></Column>
  <Column index="23" name="CABECALHO" type="varchar" nullable="false"></Column>
  <Column index="24" name="AREA1" type="varchar" nullable="false"></Column>
  <Column index="25" name="AREA2" type="varchar" nullable="false"></Column>
</Columns></Table>
<Table name="ORGAO"><Columns>
  <Column index="01" name="ID" type="smallint" nullable="false"></Column>
  <Column index="02" name="NOME" type="varchar" nullable="false"></Column>
</Columns></Table>
<Table name="TIPO_ATO"><Columns>
  <Column index="01" name="ID" type="smallint" nullable="false"></Column>
  <Column index="02" name="NOME" type="varchar" nullable="false"></Column>
</Columns></Table>
<Table name="STATUS_ATO"><Columns>
  <Column index="01" name="ID" type="smallint" nullable="false"></Column><Column index="02" name="NOME" type="varchar" nullable="false"></Column>
</Columns></Table>
<Table name="SITUACAO_ACAO"><Columns>
  <Column index="01" name="ID" type="smallint" nullable="false"></Column><Column index="02" name="NOME" type="varchar" nullable="false"></Column>
</Columns></Table>
</Tables></Info>"#
        .as_bytes(),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::VALUES_PER_ROW;

    /// Build a row with the same shape the parser produces.
    fn row(id: i64, values: &[(&str, &str)]) -> Row {
        let get = |name: &str| {
            values
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        };
        let names = [
            "ID",
            "NUMERO_IDENTIFICACAO",
            "NUMERO_OFICIAL",
            "DIARIO_OFICIAL",
            "SUPLEMENTO",
            "ANO_PUBLICACAO",
            "VERSAO",
            "TIPO_VERSAO",
            "DATA_CRIACAO",
            "DATA_ASSINATURA",
            "DATA_PUBLICACAO",
            "DATA_REPUBLICACAO",
            "TRAMITAVEL",
            "TIPO_ATO_ID",
            "TIPO_ATO_GRUPO",
            "STATUS_ATO_ID",
            "SITUACAO_ACAO_ID",
            "ORGAO_ID__ORIGEM",
            "NUMERO_UNICO",
            "TEXTO_REPUBLICACAO",
            "MARCADOR",
            "SUMULA",
            "CABECALHO",
        ];
        let mut v: Vec<Option<String>> = names
            .iter()
            .map(|n| if *n == "ID" { Some(id.to_string()) } else { get(n) })
            .collect();
        v.push(get("body")); // the merged body
        assert_eq!(v.len(), VALUES_PER_ROW);
        Row {
            values: v,
            src_bytes: 0,
            src_chars: 0,
        }
    }

    fn seeded() -> Store {
        let mut s = Store::open_in_memory().unwrap();
        let sc = test_schema();
        s.ensure_act_tables(&sc).unwrap();
        s.insert_rows(&[
            row(
                1,
                &[
                    ("NUMERO_IDENTIFICACAO", "DOC-1"),
                    ("NUMERO_OFICIAL", "683"),
                    ("ANO_PUBLICACAO", "2005"),
                    ("DATA_PUBLICACAO", "2005-04-05 00:00:00.000"),
                    ("TIPO_ATO_ID", "DECRETO"),
                    ("ORGAO_ID__ORIGEM", "PREFEITURA MUNICIPAL DE CURITIBA"),
                    ("SITUACAO_ACAO_ID", "Revogado"),
                    ("SUMULA", "NOMEIA MEMBROS DO CONSELHO DE ADMINISTRACAO"),
                    ("body", "<p>Nomeia membros para compor o conselho</p>"),
                ],
            ),
            row(
                2,
                &[
                    ("NUMERO_IDENTIFICACAO", "DOC-2"),
                    ("NUMERO_OFICIAL", "1201"),
                    ("ANO_PUBLICACAO", "2019"),
                    ("DATA_PUBLICACAO", "2019-03-11 00:00:00.000"),
                    ("TIPO_ATO_ID", "LEI COMPLEMENTAR"),
                    ("ORGAO_ID__ORIGEM", "SECRETARIA MUNICIPAL DA SAUDE"),
                    ("SITUACAO_ACAO_ID", "Em Vigor"),
                    ("SUMULA", "Dispõe sobre a publicacao de atos oficiais"),
                    ("body", "<p>Publicação de atos e Diario Oficial</p>"),
                ],
            ),
        ])
        .unwrap();
        s
    }

    #[test]
    fn creates_columns_from_the_schema_and_merges_the_body() {
        let s = seeded();
        let cols = s.act_columns().unwrap();
        assert_eq!(cols.len(), 24, "23 metadata + body");
        assert_eq!(cols[0], "ID");
        assert_eq!(cols[22], "CABECALHO");
        assert_eq!(cols[23], "body");
        assert!(!cols.contains(&"AREA1".to_string()), "AREA1 folds into body");
        assert!(!cols.contains(&"AREA2".to_string()));
    }

    #[test]
    fn ensure_is_idempotent() {
        let mut s = Store::open_in_memory().unwrap();
        let sc = test_schema();
        s.ensure_act_tables(&sc).unwrap();
        s.ensure_act_tables(&sc).unwrap(); // must not fail on existing tables
        assert_eq!(s.act_count().unwrap(), 0);
    }

    #[test]
    fn inserts_and_counts_rows() {
        let s = seeded();
        assert_eq!(s.act_count().unwrap(), 2);
        assert_eq!(s.max_act_id().unwrap(), Some(2));
    }

    #[test]
    fn full_text_search_matches_across_columns() {
        let s = seeded();
        // Matches SUMULA of id 1.
        let r = s
            .search(&ActQuery {
                text: Some("conselho".into()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.rows[0].id, 1);
        assert!(r.rows[0].snippet.as_deref().unwrap().contains("<mark>"));
    }

    #[test]
    fn full_text_search_folds_diacritics_both_ways() {
        let s = seeded();
        // Id 2's súmula holds "publicacao" (unaccented) and its body holds
        // "Publicação" (accented). The unicode61 tokenizer with
        // `remove_diacritics 2` maps both to the same term, so either spelling
        // finds the same row — which is what a Portuguese-language user expects.
        for q in ["publicacao", "publicação", "PUBLICAÇÃO"] {
            let r = s
                .search(&ActQuery {
                    text: Some(q.into()),
                    limit: 10,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(r.total, 1, "query {q:?} should fold to the same term");
            assert_eq!(r.rows[0].id, 2, "query {q:?}");
        }
        // The unaccented súmula of id 1 is genuinely a different term.
        let r = s
            .search(&ActQuery {
                text: Some("conselho".into()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.rows[0].id, 1);
    }

    #[test]
    fn full_text_search_survives_operator_characters() {
        let s = seeded();
        // These would be FTS5 syntax errors if passed through unquoted.
        for q in ["AND", "OR", "*", "NOT", "-", "\"unterminated", "a"] {
            let r = s
                .search(&ActQuery {
                    text: Some(q.into()),
                    limit: 10,
                    ..Default::default()
                })
                .unwrap_or_else(|e| panic!("query {q:?} should not error: {e}"));
            assert!(r.total >= 0);
        }
    }

    #[test]
    fn filters_combine_with_text_and_are_bound_not_interpolated() {
        let s = seeded();
        let r = s
            .search(&ActQuery {
                text: Some("publicacao".into()),
                filters: vec![Filter {
                    column: "TIPO_ATO_ID".into(),
                    op: Op::Eq,
                    value: "DECRETO".into(),
                    ..Default::default()
                }],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 0, "only id 2 matches the text, and it is not a DECRETO");

        let r = s
            .search(&ActQuery {
                filters: vec![Filter {
                    column: "TIPO_ATO_ID".into(),
                    op: Op::Like,
                    value: "LEI".into(),
                    ..Default::default()
                }],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.rows[0].id, 2);
    }

    #[test]
    fn a_injection_attempt_in_a_filter_value_is_just_a_value() {
        let s = seeded();
        let r = s
            .search(&ActQuery {
                filters: vec![Filter {
                    column: "ORGAO_ID__ORIGEM".into(),
                    op: Op::Eq,
                    value: "' OR 1=1 --".into(),
                    ..Default::default()
                }],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 0, "the value must be bound, not executed");
    }

    #[test]
    fn a_filter_accepts_several_values_as_one_of() {
        let s = seeded();
        let f = |op, value: &str, values: &[&str]| Filter {
            column: "TIPO_ATO_ID".into(),
            op,
            value: value.into(),
            values: values.iter().map(|v| v.to_string()).collect(),
            logic: Logic::And,
        };

        // "one of": id 1 is a DECRETO, id 2 a LEI COMPLEMENTAR — both match.
        let r = s
            .search(&ActQuery {
                filters: vec![f(Op::Eq, "DECRETO", &["LEI COMPLEMENTAR"])],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 2);

        // "none of": the same list inverted excludes both.
        let r = s
            .search(&ActQuery {
                filters: vec![f(Op::Ne, "DECRETO", &["LEI COMPLEMENTAR"])],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 0);

        // A blank entry is not a value, and duplicates collapse.
        let r = s
            .search(&ActQuery {
                filters: vec![f(Op::Eq, "DECRETO", &["", "DECRETO"])],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
    }

    #[test]
    fn conditions_join_with_and_and_or() {
        let s = seeded();
        let cond = |column: &str, value: &str, logic: Logic| Filter {
            column: column.into(),
            op: Op::Eq,
            value: value.into(),
            values: Vec::new(),
            logic,
        };

        // AND: DECRETO and the Prefeitura órgão — only id 1 has both.
        let r = s
            .search(&ActQuery {
                filters: vec![
                    cond("TIPO_ATO_ID", "DECRETO", Logic::And),
                    cond("ORGAO_ID__ORIGEM", "PREFEITURA MUNICIPAL DE CURITIBA", Logic::And),
                ],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.rows[0].id, 1);

        // OR between two conditions: either tipo matches, so both rows come back.
        let r = s
            .search(&ActQuery {
                filters: vec![
                    cond("TIPO_ATO_ID", "DECRETO", Logic::And),
                    cond("TIPO_ATO_ID", "LEI COMPLEMENTAR", Logic::Or),
                ],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 2);

        // The group is parenthesised, so an OR does not leak past a following AND:
        // (DECRETO OR LEI) AND SITUACAO=Em Vigor keeps only id 2.
        let r = s
            .search(&ActQuery {
                filters: vec![
                    cond("TIPO_ATO_ID", "DECRETO", Logic::And),
                    cond("TIPO_ATO_ID", "LEI COMPLEMENTAR", Logic::Or),
                    cond("SITUACAO_ACAO_ID", "Em Vigor", Logic::And),
                ],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.rows[0].id, 2);
    }

    #[test]
    fn a_negated_filter_keeps_rows_whose_column_is_empty() {
        let mut s = seeded();
        // A third act with no tipo at all. `!= 'DECRETO'` would drop it (null
        // compares as unknown), `NOT (col = 'DECRETO')` keeps it — which is what
        // "não é decreto" means to a reader.
        s.insert_rows(&[row(3, &[("NUMERO_IDENTIFICACAO", "DOC-3"), ("TIPO_ATO_ID", "")])])
            .unwrap();
        let r = s
            .search(&ActQuery {
                filters: vec![Filter {
                    column: "TIPO_ATO_ID".into(),
                    op: Op::Ne,
                    value: "DECRETO".into(),
                    values: Vec::new(),
                    logic: Logic::And,
                }],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 2, "the LEI and the blank row both stand");
        assert!(r.rows.iter().all(|r| r.id != 1));
    }

    #[test]
    fn min_id_keeps_only_rows_above_the_floor() {
        let s = seeded();
        // Both rows match the LIKE, but only id 2 sits above the floor.
        let q = |min_id| ActQuery {
            filters: vec![Filter {
                column: "NUMERO_IDENTIFICACAO".into(),
                op: Op::Like,
                value: "DOC".into(),
                ..Default::default()
            }],
            min_id,
            limit: 10,
            ..Default::default()
        };
        assert_eq!(s.search(&q(None)).unwrap().total, 2);
        assert_eq!(s.search(&q(Some(1))).unwrap().total, 1);
        assert_eq!(s.search(&q(Some(1))).unwrap().rows[0].id, 2);
        assert_eq!(s.search(&q(Some(2))).unwrap().total, 0, "the floor is exclusive");
    }

    #[test]
    fn min_id_bounds_every_value_of_a_multi_value_filter() {
        // Reproduces a precedence bug: a multi-value `eq` was emitted as a bare
        // `"col" = ? OR "col" = ?`, so when the floor was joined on with AND only
        // the *last* value honoured it — the earlier ones matched the whole table.
        // Ids 1 (`DECRETO`) and 2 (`LEI COMPLEMENTAR`) are both matched, but only
        // id 2 is above the floor; if the first OR term escaped the floor the
        // total would be 2 instead of 1.
        let s = seeded();
        let q = |min_id| ActQuery {
            filters: vec![Filter {
                column: "TIPO_ATO_ID".into(),
                op: Op::Eq,
                // `all_values` puts `values` first, so DECRETO leads the OR.
                value: "LEI COMPLEMENTAR".into(),
                values: vec!["DECRETO".into()],
                ..Default::default()
            }],
            min_id,
            limit: 10,
            ..Default::default()
        };
        assert_eq!(s.search(&q(None)).unwrap().total, 2);
        assert_eq!(s.search(&q(Some(1))).unwrap().total, 1, "the floor must bound every value");
        assert_eq!(s.search(&q(Some(1))).unwrap().rows[0].id, 2);
    }

    #[test]
    fn min_id_composes_with_text_and_can_stand_alone() {
        let s = seeded();
        // "conselho" matches id 1, which is not above the floor; the text hit and
        // the floor must both hold, not either.
        let r = s
            .search(&ActQuery {
                text: Some("conselho".into()),
                min_id: Some(1),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 0);

        // No conditions at all plus a floor is "every act newer than the floor",
        // which is a legitimate rule (notify me about anything new).
        let r = s
            .search(&ActQuery {
                min_id: Some(1),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.rows[0].id, 2);
    }

    #[test]
    fn unknown_filter_and_sort_columns_are_rejected() {
        let s = seeded();
        assert!(s
            .search(&ActQuery {
                filters: vec![Filter {
                    column: "body".into(),
                    op: Op::Eq,
                    value: "x".into(),
                    ..Default::default()
                }],
                limit: 1,
                ..Default::default()
            })
            .is_err());
        assert!(s
            .search(&ActQuery {
                sort: Some("body".into()),
                limit: 1,
                ..Default::default()
            })
            .is_err());
    }

    #[test]
    fn sorting_and_paging_work() {
        let s = seeded();
        let r = s
            .search(&ActQuery {
                sort: Some("ANO_PUBLICACAO".into()),
                ascending: false,
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.rows[0].id, 2, "2019 is newer than 2005");
        assert_eq!(r.rows[1].id, 1);

        let page2 = s
            .search(&ActQuery {
                sort: Some("ANO_PUBLICACAO".into()),
                ascending: false,
                limit: 1,
                offset: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page2.total, 2, "total ignores paging");
        assert_eq!(page2.rows[0].id, 1);
    }

    #[test]
    fn facets_count_values() {
        let s = seeded();
        let f = s.facet("TIPO_ATO_ID", 10).unwrap();
        assert_eq!(f.len(), 2);
        assert!(f.iter().all(|x| x.count == 1));
        assert!(s.facet("body", 10).is_err(), "non-filterable column rejected");
    }

    #[test]
    fn a_related_column_offers_the_values_the_acts_actually_hold() {
        // The feed declares TIPO_ATO_ID as `TIPO_ATO.ID` (a smallint) but stores
        // the type's *name* in the act row. Offering the key would produce a rule
        // that matches nothing, so the vocabulary must come from the acts and the
        // dimension must only label it.
        let mut s = seeded();
        s.replace_dimension(
            "TIPO_ATO",
            &["ID".into(), "NOME".into()],
            &[
                vec![Some("50".into()), Some("ACORDÃO".into())],
                vec![Some("3".into()), Some("LEI COMPLEMENTAR".into())],
                vec![Some("1".into()), Some("DECRETO".into())],
            ],
        )
        .unwrap();

        let values = s.column_values(&test_schema(), "TIPO_ATO_ID", 200).unwrap().unwrap();
        let offered: Vec<&str> = values.iter().map(|v| v.value.as_str()).collect();
        assert!(offered.contains(&"DECRETO"), "value must be what the act holds: {offered:?}");
        assert!(offered.contains(&"LEI COMPLEMENTAR"));
        assert!(!offered.contains(&"50"), "the dimension key must not be offered: {offered:?}");
        // The label stays the dimension's NOME.
        let decreto = values.iter().find(|v| v.value == "DECRETO").unwrap();
        assert_eq!(decreto.label, "DECRETO");
    }

    #[test]
    fn detail_labels_fields_and_resolves_dimensions() {
        let mut s = seeded();
        // Load the ORGAO dimension, then a detail lookup should resolve it.
        s.replace_dimension(
            "ORGAO",
            &["ID".into(), "NOME".into()],
            &[vec![Some("1".into()), Some("PREFEITURA MUNICIPAL DE CURITIBA".into())]],
        )
        .unwrap();

        let d = s.act_detail(&test_schema(), 1).unwrap().unwrap();
        let orgao = d
            .fields
            .iter()
            .find(|f| f.column == "ORGAO_ID__ORIGEM")
            .unwrap();
        assert_eq!(orgao.label, "Órgão de origem");
        assert_eq!(orgao.value.as_deref(), Some("PREFEITURA MUNICIPAL DE CURITIBA"));
        assert_eq!(orgao.related_table.as_deref(), Some("ORGAO"));
        assert!(!d.body.as_deref().unwrap().is_empty());
    }

    #[test]
    fn act_row_reads_one_act_by_id() {
        let s = seeded();
        let r = s.act_row(1).unwrap().unwrap();
        assert_eq!(r.id, 1);
        assert_eq!(r.numero_identificacao.as_deref(), Some("DOC-1"));
        assert_eq!(r.numero_oficial.as_deref(), Some("683"));
        assert_eq!(r.tipo.as_deref(), Some("DECRETO"));
        assert_eq!(r.situacao.as_deref(), Some("Revogado"));
        assert!(r.snippet.is_none(), "no text query, so no excerpt");
        assert!(s.act_row(999).unwrap().is_none(), "a missing id is None");
    }

    #[test]
    fn staging_import_migrates_only_rows_above_the_stored_maximum() {
        let mut s = seeded(); // ids 1 and 2
        s.begin_import().unwrap();
        // The republished file carries the acts we already hold (1, 2) plus two
        // new ones. Only the new ids may reach the act table.
        s.stage_rows(&[
            row(1, &[("NUMERO_IDENTIFICACAO", "DOC-1")]),
            row(2, &[("NUMERO_IDENTIFICACAO", "DOC-2")]),
            row(3, &[("NUMERO_IDENTIFICACAO", "DOC-3"), ("SUMULA", "NOVO ATO TRES")]),
            row(4, &[("NUMERO_IDENTIFICACAO", "DOC-4")]),
        ])
        .unwrap();
        assert_eq!(s.merge_import().unwrap(), 2, "only ids 3 and 4 are new");
        s.finish_import().unwrap();

        assert_eq!(s.act_count().unwrap(), 4);
        assert_eq!(s.max_act_id().unwrap(), Some(4));
        assert!(s.act_detail(&test_schema(), 3).unwrap().is_some());
    }

    #[test]
    fn staging_import_into_an_empty_table_migrates_everything() {
        let mut s = Store::open_in_memory().unwrap();
        s.ensure_act_tables(&test_schema()).unwrap();
        s.begin_import().unwrap();
        s.stage_rows(&[
            row(10, &[("NUMERO_IDENTIFICACAO", "DOC-10")]),
            row(11, &[]),
        ])
        .unwrap();
        assert_eq!(s.merge_import().unwrap(), 2, "no floor: every row is new");
        s.finish_import().unwrap();
        assert_eq!(s.act_count().unwrap(), 2);
    }

    #[test]
    fn a_migrated_row_is_searchable_through_the_index() {
        // The merge must populate the full-text index for the rows it copies.
        let mut s = seeded();
        s.begin_import().unwrap();
        s.stage_rows(&[
            row(3, &[("SUMULA", "LICITACAO PARA COMPRA DE VIATURAS")]),
            row(4, &[("SUMULA", "NOMEACAO DE SERVIDORES")]),
        ])
        .unwrap();
        s.merge_import().unwrap();
        s.finish_import().unwrap();

        let hit = s
            .search(&ActQuery {
                text: Some("viaturas".into()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hit.total, 1);
        assert_eq!(hit.rows[0].id, 3);
    }

    #[test]
    fn begin_import_leaves_the_act_table_alone() {
        let mut s = seeded();
        s.begin_import().unwrap();
        assert_eq!(s.act_count().unwrap(), 2, "staging is separate from the act table");
        assert_eq!(s.merge_import().unwrap(), 0, "nothing staged yet");
        s.finish_import().unwrap();
    }

    #[test]
    fn the_staging_table_is_gone_after_the_merge() {
        let mut s = seeded();
        s.begin_import().unwrap();
        s.stage_rows(&[row(3, &[("NUMERO_IDENTIFICACAO", "DOC-3")])])
            .unwrap();
        assert_eq!(s.merge_import().unwrap(), 1);
        s.finish_import().unwrap();

        let left: i64 = s
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'acts_import'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(left, 0, "the staging table must not survive the import");
        assert_eq!(s.act_count().unwrap(), 3, "its rows are in the act table");
    }

    #[test]
    fn finish_import_returns_the_staging_space_to_the_file() {
        // This can only be seen on a real file — an in-memory database has no file
        // to shrink. It reproduces the bug where `PRAGMA incremental_vacuum` was
        // executed once (freeing a single page) instead of drained, so a dropped
        // staging table stayed in the file as a freelist.
        let dir = std::env::temp_dir().join(format!("legisladoc-vacuum-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("size.sqlite");

        let mut s = Store::open(&path).unwrap();
        s.ensure_act_tables(&test_schema()).unwrap();
        // One act with a high id, so every staged row sits under the floor and the
        // merge copies nothing — this isolates the reclaim.
        s.insert_rows(&[row(1_000_000, &[("NUMERO_IDENTIFICACAO", "KEEP")])])
            .unwrap();

        let body = "x".repeat(4000);
        let staged: Vec<Row> = (1..=3_000)
            .map(|id| row(id, &[("NUMERO_IDENTIFICACAO", "OLD"), ("body", body.as_str())]))
            .collect();

        s.begin_import().unwrap();
        s.stage_rows(&staged).unwrap();
        s.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let peak = std::fs::metadata(&path).unwrap().len();

        assert_eq!(s.merge_import().unwrap(), 0, "no staged id is above the floor");
        s.finish_import().unwrap();
        let after = std::fs::metadata(&path).unwrap().len();

        assert!(
            after < peak,
            "the staging space must be released: peak {peak} B -> after {after} B"
        );
        assert_eq!(s.act_count().unwrap(), 1, "the act table is untouched");

        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snapshot_state_round_trips() {
        let s = Store::open_in_memory().unwrap();
        let f = "2026-10-08_ATO.csv";
        assert!(s.snapshot_state(f).unwrap().is_none(), "nothing recorded yet");

        s.record_snapshot(
            f,
            1424486256,
            Some("\"abc\""),
            Some("Thu, 08 Oct 2026 08:27:57 GMT"),
            Some(393577),
            393577,
        )
        .unwrap();
        let st = s.snapshot_state(f).unwrap().unwrap();
        assert_eq!(st.size, 1424486256);
        assert_eq!(st.etag.as_deref(), Some("\"abc\""));

        // Re-recording a later revision replaces the validators.
        s.record_snapshot(f, 1424987102, Some("\"def\""), None, Some(400000), 400000)
            .unwrap();
        let st = s.snapshot_state(f).unwrap().unwrap();
        assert_eq!(st.size, 1424987102);
        assert_eq!(st.etag.as_deref(), Some("\"def\""));
    }

    #[test]
    fn stats_summarise_the_table() {
        let s = seeded();
        let st = s.stats().unwrap();
        assert_eq!(st.total, 2);
        assert_eq!(st.newest_publication.as_deref(), Some("2019-03-11 00:00:00.000"));
        assert_eq!(st.oldest_publication.as_deref(), Some("2005-04-05 00:00:00.000"));
        assert_eq!(st.with_body, 2);
    }
}
