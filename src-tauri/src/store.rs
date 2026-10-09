//! SQLite storage for the act data, plus the query API the UI drives.
//!
//! Tables are built from the feed's own `Info.xml` rather than hard-coded, so a
//! snapshot that adds or renames a column is picked up instead of being silently
//! dropped on the floor.

use anyhow::{bail, Context, Result};
use rusqlite::{params, params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ingest::SnapshotState;
use crate::parser::{self, Row};
use crate::schema::Schema;

/// The act fact table and its full-text index.
const ACTS: &str = "acts";
const FTS: &str = "acts_fts";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub column: String,
    pub op: Op,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActQuery {
    /// Full-text query. When set, results are ordered by relevance unless `sort`
    /// is given explicitly.
    pub text: Option<String>,
    pub filters: Vec<Filter>,
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

/// A value and its frequency, for the filter dropdowns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facet {
    pub value: String,
    pub count: i64,
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
            "#,
        )
        .context("creating base schema")?;
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

    pub fn snapshot_state(&self, file_name: &str) -> Result<Option<SnapshotState>> {
        let row = self
            .conn
            .query_row(
                "SELECT byte_offset, etag, last_id, row_count FROM snapshots WHERE file_name = ?1",
                params![file_name],
                |r| {
                    Ok(SnapshotState {
                        offset: r.get::<_, i64>(0)? as u64,
                        etag: r.get(1)?,
                        last_id: r.get(2)?,
                        row_count: r.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Record a snapshot's validators up front, without touching the stored
    /// offset — that only moves once the rows are committed.
    pub fn upsert_snapshot_meta(
        &self,
        file_name: &str,
        size: u64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO snapshots (file_name, size, etag, last_modified, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_name) DO UPDATE SET
                size = excluded.size, etag = excluded.etag,
                last_modified = excluded.last_modified, updated_at = excluded.updated_at",
            params![
                file_name,
                size as i64,
                etag,
                last_modified,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Move the offset/row-count watermark after a successful commit.
    pub fn commit_snapshot_progress(
        &self,
        file_name: &str,
        offset: u64,
        last_id: Option<i64>,
        row_count: i64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE snapshots SET byte_offset = ?2, last_id = ?3, row_count = ?4, updated_at = ?5
             WHERE file_name = ?1",
            params![
                file_name,
                offset as i64,
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

    /// Empty the act data and its index, ready for a full load.
    pub fn clear_acts(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        // 'delete-all' is how an external-content FTS5 table is emptied.
        tx.execute_batch(&format!(
            "INSERT INTO {FTS}({FTS}) VALUES('delete-all'); DELETE FROM {ACTS};"
        ))?;
        tx.commit()?;
        Ok(())
    }

    /// Insert a batch of parsed rows.
    ///
    /// Uses plain INSERT, not upsert: the delta path guarantees new rows carry
    /// new ids, and a full load clears the tables first. That matters because an
    /// external-content FTS index cannot be updated in place without the previous
    /// column values, which this feed does not retain.
    pub fn insert_rows(&mut self, rows: &[Row]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let cols = self.act_columns()?;
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
                "INSERT INTO {ACTS} ({quoted}) VALUES ({placeholders})"
            ))?;
            let mut fts = tx.prepare(&format!(
                "INSERT INTO {FTS} (rowid, NUMERO_IDENTIFICACAO, NUMERO_OFICIAL, SUMULA, CABECALHO, body)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ))?;

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
        tx.commit()?;
        Ok(())
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

    fn act_columns(&self) -> Result<Vec<String>> {
        let stmt = self.conn.prepare(&format!("SELECT * FROM {ACTS} LIMIT 0"))?;
        let cols = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        Ok(cols)
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

        for f in &q.filters {
            if !filterable_columns().contains(&f.column.as_str()) {
                bail!("column {:?} is not filterable", f.column);
            }
            let esc = matches!(f.op, Op::Like);
            clauses.push(format!(
                "a.\"{}\" {} ?{}{}",
                f.column,
                f.op.sql(),
                binds.len() + 1,
                if esc { " ESCAPE '\\'" } else { "" }
            ));
            binds.push(Box::new(f.op.param(&f.value)));
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
                    a.\"DATA_PUBLICACAO\", a.\"TIPO_ATO_ID\", a.\"ORGAO_ID__ORIGEM\",
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
                    tipo: r.get(5)?,
                    orgao: r.get(6)?,
                    situacao: r.get(7)?,
                    sumula: r.get(8)?,
                    snippet: r.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SearchResult { rows, total })
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
                }],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 0, "the value must be bound, not executed");
    }

    #[test]
    fn unknown_filter_and_sort_columns_are_rejected() {
        let s = seeded();
        assert!(s
            .search(&ActQuery {
                filters: vec![Filter {
                    column: "body".into(),
                    op: Op::Eq,
                    value: "x".into()
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
    fn clear_acts_empties_data_and_the_index() {
        let mut s = seeded();
        assert_eq!(s.act_count().unwrap(), 2);
        s.clear_acts().unwrap();
        assert_eq!(s.act_count().unwrap(), 0);
        // A stale index would still match; it must not.
        let r = s
            .search(&ActQuery {
                text: Some("conselho".into()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, 0, "the full-text index must be emptied too");
    }

    #[test]
    fn snapshot_state_round_trips() {
        let mut s = Store::open_in_memory().unwrap();
        let sc = test_schema();
        s.ensure_act_tables(&sc).unwrap();
        let f = "2026-10-08_ATO.csv";
        s.upsert_snapshot_meta(f, 1424486256, Some("\"abc\""), Some("Thu, 08 Oct 2026 08:27:57 GMT"))
            .unwrap();
        s.commit_snapshot_progress(f, 1424486256, Some(393577), 393577).unwrap();

        let st = s.snapshot_state(f).unwrap().unwrap();
        assert_eq!(st.offset, 1424486256);
        assert_eq!(st.etag.as_deref(), Some("\"abc\""));
        assert_eq!(st.last_id, Some(393577));
        assert_eq!(st.row_count, 393577);

        // Re-recording metadata must not move the watermark backwards.
        s.upsert_snapshot_meta(f, 1424987102, Some("\"def\""), None).unwrap();
        let st = s.snapshot_state(f).unwrap().unwrap();
        assert_eq!(st.offset, 1424486256, "offset only moves on commit");
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
