//! Local SQLite index of the feed, plus the search queries the UI issues.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

use crate::feed::RemoteFile;

/// How recently a file must have changed to be flagged as "new".
/// The feed regenerates its snapshot daily, so a short window is meaningful here.
const RECENT_DAYS: i64 = 1;

/// One row of the file table, as handed to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct FileRow {
    pub id: i64,
    pub path: String,
    pub size: i64,
    /// RFC 3339, absent when the server reported no mtime.
    pub mtime: Option<String>,
    /// True when mtime falls inside the trailing 30-day window.
    pub recent: bool,
    /// RFC 3339 of the last ingest of this file, or `None` when it has not been
    /// processed (a file this app does not read, or one not yet synced).
    pub synced_at: Option<String>,
}

/// Aggregate counters driving the header and the stat tiles.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub total_files: i64,
    pub total_bytes: i64,
    /// Total bytes of *all* files the server listed, including any we filter out.
    pub listed_bytes: i64,
    pub newest_mtime: Option<String>,
    /// Files modified within the last 30 days.
    pub recent_count: i64,
    /// Files that arrived or disappeared in the most recent sync.
    pub added_last_sync: i64,
    pub removed_last_sync: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncOutcome {
    pub added: i64,
    pub updated: i64,
    pub removed: i64,
    pub total: i64,
}

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating data dir {}", parent.display()))?;
    }
    let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    init_schema(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS files (
            path  TEXT PRIMARY KEY,
            size  INTEGER NOT NULL,
            mtime TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_files_mtime ON files(mtime DESC);

        -- One row per completed sync, newest last.
        CREATE TABLE IF NOT EXISTS syncs (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            at            TEXT NOT NULL,
            source        TEXT NOT NULL,
            total         INTEGER NOT NULL,
            listed_bytes  INTEGER NOT NULL,
            added         INTEGER NOT NULL,
            updated       INTEGER NOT NULL,
            removed       INTEGER NOT NULL,
            error         TEXT
        );

        -- When each Base_de_Dados file was last ingested, for the explorer's
        -- "sincronizado em" badge. Keyed by base file name.
        CREATE TABLE IF NOT EXISTS processed_files (
            file_name TEXT PRIMARY KEY,
            synced_at TEXT NOT NULL
        );
        "#,
    )
    .context("creating schema")?;
    Ok(())
}

/// Merge a fresh remote listing into the index, recording what changed.
///
/// The remote listing is authoritative: paths missing from it are deleted. The
/// whole merge runs in one transaction so a failure can't leave the index torn.
pub fn apply_listing(
    conn: &mut Connection,
    source: &str,
    listed_bytes: i64,
    files: &[RemoteFile],
    error: Option<&str>,
) -> Result<SyncOutcome> {
    let tx = conn.transaction()?;

    // Snapshot the current (size, mtime) pairs so we can classify each remote
    // file as new, changed, or unchanged. 12k rows is nothing to keep in memory.
    let mut known: std::collections::HashMap<String, (i64, Option<String>)> =
        std::collections::HashMap::new();
    {
        let mut stmt = tx.prepare("SELECT path, size, mtime FROM files")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?),
            ))
        })?;
        for row in rows {
            let (path, meta) = row?;
            known.insert(path, meta);
        }
    }

    let mut added = 0i64;
    let mut updated = 0i64;
    {
        let mut upsert = tx.prepare(
            "INSERT INTO files (path, size, mtime) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime",
        )?;
        for f in files {
            match known.get(&f.path) {
                None => added += 1,
                Some((size, mtime)) if *size != f.size || *mtime != f.mtime => updated += 1,
                Some(_) => {}
            }
            upsert.execute(params![f.path, f.size, f.mtime])?;
        }
    }

    // Anything we knew about but the server no longer lists is gone.
    let mut removed = 0i64;
    {
        let mut delete = tx.prepare("DELETE FROM files WHERE path = ?1")?;
        for path in known.keys() {
            if !files.iter().any(|f| &f.path == path) {
                delete.execute(params![path])?;
                removed += 1;
            }
        }
    }

    tx.execute(
        "INSERT INTO syncs (at, source, total, listed_bytes, added, updated, removed, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            chrono::Utc::now().to_rfc3339(),
            source,
            files.len() as i64,
            listed_bytes,
            added,
            updated,
            removed,
            error
        ],
    )?;

    tx.commit()?;

    Ok(SyncOutcome {
        added,
        updated,
        removed,
        total: files.len() as i64,
    })
}

/// Log a sync that failed, leaving the `files` table exactly as it was.
///
/// Keeping failures in `syncs` (rather than just logging them) is what lets the
/// UI say "last good sync was 3 hours ago, and here is why the new one failed".
pub fn record_failed_sync(conn: &Connection, source: &str, error: &str) -> Result<()> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
    let bytes: i64 = conn
        .query_row("SELECT COALESCE(SUM(size), 0) FROM files", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO syncs (at, source, total, listed_bytes, added, updated, removed, error)
         VALUES (?1, ?2, ?3, ?4, 0, 0, 0, ?5)",
        params![chrono::Utc::now().to_rfc3339(), source, total, bytes, error],
    )?;
    Ok(())
}

fn row_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<FileRow> {
    let mtime: Option<String> = r.get(2)?;
    let recent = mtime
        .as_deref()
        .and_then(|m| chrono::DateTime::parse_from_rfc3339(m).ok())
        .map(|m| chrono::Utc::now().signed_duration_since(m).num_days() < RECENT_DAYS)
        .unwrap_or(false);
    Ok(FileRow {
        id: r.get(3)?,
        path: r.get(0)?,
        size: r.get(1)?,
        mtime,
        recent,
        // Filled in by `query_files` from `processed_files`; a `row_from` alone
        // has no connection to look it up with.
        synced_at: None,
    })
}

/// Page through the index.
///
/// `query` matches the path as a case-insensitive substring; `%` and `_` are
/// escaped so they behave as literal characters rather than wildcards.
///
/// `sort_by` is whitelisted to a column name — it is interpolated into SQL, so
/// it must never come straight from the UI. `ascending` picks the direction.
pub fn query_files(
    conn: &Connection,
    query: &str,
    limit: i64,
    offset: i64,
    sort_by: &str,
    ascending: bool,
) -> Result<(Vec<FileRow>, i64)> {
    let q = query.trim();
    let like = format!("%{}%", escape_like(q));
    let has_q = !q.is_empty();

    let column = match sort_by {
        "size" => "size",
        "added" => "rowid",
        _ => "COALESCE(mtime, '')",
    };
    let direction = if ascending { "ASC" } else { "DESC" };

    // `path` is the tiebreaker so paging is stable when the sort key ties — two
    // files sharing an mtime must not swap places between requests.
    let sql = format!(
        "SELECT path, size, mtime, rowid FROM files
         WHERE (?1 = 0 OR path LIKE ?2 ESCAPE '\\')
         ORDER BY {column} {direction}, path ASC
         LIMIT ?3 OFFSET ?4"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt
        .query_map(
            params![has_q as i64, like, limit, offset],
            |r| Ok(row_from(r)?),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Attach the "sincronizado em" time. Matching by base name (rather than a
    // SQL join) holds whether the listing path is a bare name or carries a
    // directory prefix, since `processed_files` is keyed by base name.
    let processed = processed_index(conn)?;
    for row in &mut rows {
        row.synced_at = processed.get(basename(&row.path)).cloned();
    }

    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE (?1 = 0 OR path LIKE ?2 ESCAPE '\\')",
        params![has_q as i64, like],
        |r| r.get(0),
    )?;

    Ok((rows, total))
}

/// The final path component — the key `processed_files` and `snapshots` use.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// `file_name -> synced_at` for every file that has been ingested.
fn processed_index(conn: &Connection) -> Result<std::collections::HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT file_name, synced_at FROM processed_files")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let (name, at) = row?;
        map.insert(name, at);
    }
    Ok(map)
}

/// Record (or refresh) when each ingested file was processed.
///
/// The explorer badges any listed file named here. `names` are base file names,
/// the shape `snapshots` already uses.
pub fn record_processed(conn: &Connection, names: &[String]) -> Result<()> {
    if names.is_empty() {
        return Ok(());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction()?;
    {
        let mut up = tx.prepare(
            "INSERT INTO processed_files (file_name, synced_at) VALUES (?1, ?2)
             ON CONFLICT(file_name) DO UPDATE SET synced_at = excluded.synced_at",
        )?;
        for name in names {
            up.execute(params![name, now])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn summary(conn: &Connection) -> Result<Summary> {
    let mut s = Summary::default();

    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM files",
        [],
        |r| {
            s.total_files = r.get(0)?;
            s.total_bytes = r.get(1)?;
            Ok(())
        },
    )?;

    s.newest_mtime = conn
        .query_row("SELECT MAX(mtime) FROM files", [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap_or(None);

    s.recent_count = conn.query_row(
        "SELECT COUNT(*) FROM files
         WHERE mtime IS NOT NULL AND mtime >= ?1",
        params![(chrono::Utc::now() - chrono::Duration::days(RECENT_DAYS)).to_rfc3339()],
        |r| r.get(0),
    )?;

    // Last sync that actually completed (no error) drives the delta numbers.
    let last = conn
        .query_row(
            "SELECT listed_bytes, added, removed FROM syncs
             WHERE error IS NULL ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .ok();
    if let Some((listed, added, removed)) = last {
        s.listed_bytes = listed;
        s.added_last_sync = added;
        s.removed_last_sync = removed;
    }

    Ok(s)
}

/// Most recent sync attempt of any kind, for the status line.
pub fn last_sync(conn: &Connection) -> Result<Option<LastSync>> {
    let mut stmt = conn.prepare(
        "SELECT at, total, added, updated, removed, error FROM syncs ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query([])?;
    if let Some(r) = rows.next()? {
        Ok(Some(LastSync {
            at: r.get(0)?,
            total: r.get(1)?,
            added: r.get(2)?,
            updated: r.get(3)?,
            removed: r.get(4)?,
            error: r.get(5)?,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LastSync {
    pub at: String,
    pub total: i64,
    pub added: i64,
    pub updated: i64,
    pub removed: i64,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(path: &str, size: i64, mtime: Option<&str>) -> RemoteFile {
        RemoteFile {
            path: path.to_string(),
            size,
            mtime: mtime.map(str::to_string),
        }
    }

    fn conn() -> Connection {
        open_in_memory().unwrap()
    }

    #[test]
    fn first_sync_inserts_everything() {
        let mut c = conn();
        let files = vec![f("a.csv", 10, Some("2024-01-01T00:00:00+00:00")), f("b.csv", 20, None)];
        let out = apply_listing(&mut c, "src", 30, &files, None).unwrap();
        assert_eq!((out.added, out.updated, out.removed, out.total), (2, 0, 0, 2));
    }

    #[test]
    fn resync_detects_added_updated_and_removed() {
        let mut c = conn();
        apply_listing(
            &mut c,
            "src",
            30,
            &[f("a.csv", 10, Some("2024-01-01T00:00:00+00:00")), f("b.csv", 20, None)],
            None,
        )
        .unwrap();

        // b grew, c is new, nothing dropped yet.
        let out = apply_listing(
            &mut c,
            "src",
            45,
            &[
                f("a.csv", 10, Some("2024-01-01T00:00:00+00:00")),
                f("b.csv", 30, None),
                f("c.csv", 5, None),
            ],
            None,
        )
        .unwrap();
        assert_eq!((out.added, out.updated, out.removed), (1, 1, 0));

        // Only a remains: b and c must be pruned.
        let out = apply_listing(&mut c, "src", 10, &[f("a.csv", 10, Some("2024-01-01T00:00:00+00:00"))], None)
            .unwrap();
        assert_eq!((out.added, out.updated, out.removed, out.total), (0, 0, 2, 1));

        let (rows, total) = query_files(&c, "", 50, 0, "mtime", false).unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows[0].path, "a.csv");
    }

    #[test]
    fn a_changed_mtime_alone_counts_as_updated() {
        let mut c = conn();
        apply_listing(&mut c, "s", 1, &[f("a", 10, Some("2024-01-01T00:00:00+00:00"))], None).unwrap();
        let out = apply_listing(&mut c, "s", 1, &[f("a", 10, Some("2025-06-06T00:00:00+00:00"))], None)
            .unwrap();
        assert_eq!(out.updated, 1);
        assert_eq!(out.added, 0);
    }

    #[test]
    fn search_matches_substrings_and_escapes_wildcards() {
        let mut c = conn();
        apply_listing(
            &mut c,
            "s",
            0,
            &[f("2016-09-21_legisladoc_-_Base_de_Dados.csv", 1, None), f("lei-1201.pdf", 1, None)],
            None,
        )
        .unwrap();

        let (rows, total) = query_files(&c, "legisladoc", 10, 0, "path", true).unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows[0].path, "2016-09-21_legisladoc_-_Base_de_Dados.csv");

        // A literal underscore must not act as a single-character wildcard:
        // "Base_de" matches, whereas the wildcard reading would also match "BaseXde".
        let (_, total) = query_files(&c, "Base_de", 10, 0, "path", true).unwrap();
        assert_eq!(total, 1);
        let (_, total) = query_files(&c, "BaseXde", 10, 0, "path", true).unwrap();
        assert_eq!(total, 0);
    }

    #[test]
    fn sorting_is_applied_and_stable() {
        let mut c = conn();
        apply_listing(
            &mut c,
            "s",
            0,
            &[
                f("z.csv", 5, Some("2024-01-01T00:00:00+00:00")),
                f("a.csv", 99, Some("2024-01-01T00:00:00+00:00")),
            ],
            None,
        )
        .unwrap();

        let (rows, _) = query_files(&c, "", 10, 0, "size", false).unwrap();
        assert_eq!(rows[0].path, "a.csv", "size desc puts the largest first");

        // Equal mtimes tie-break on path so paging stays stable.
        let (rows, _) = query_files(&c, "", 10, 0, "mtime", false).unwrap();
        assert_eq!(rows[0].path, "a.csv");
    }

    #[test]
    fn paging_walks_the_whole_set_without_repeats() {
        let mut c = conn();
        let files: Vec<_> = (0..25)
            .map(|i| f(&format!("f{i:02}.csv"), i, Some("2024-01-01T00:00:00+00:00")))
            .collect();
        apply_listing(&mut c, "s", 0, &files, None).unwrap();

        let mut seen = vec![];
        for page in 0..3 {
            let (rows, total) = query_files(&c, "", 10, page * 10, "path", true).unwrap();
            assert_eq!(total, 25);
            seen.extend(rows.into_iter().map(|r| r.path));
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 25, "every file appears exactly once across pages");
    }

    #[test]
    fn failed_sync_keeps_the_existing_index() {
        let mut c = conn();
        apply_listing(&mut c, "s", 7, &[f("a", 1, None)], None).unwrap();
        record_failed_sync(&c, "s", "connection reset").unwrap();

        // A failed sync must not touch the rows...
        let (_, total) = query_files(&c, "", 10, 0, "mtime", false).unwrap();
        assert_eq!(total, 1);

        // ...but it must be visible as the latest attempt, with its error.
        let last = last_sync(&c).unwrap().unwrap();
        assert_eq!(last.error.as_deref(), Some("connection reset"));

        // ...and the summary still reports the last *good* sync's byte count.
        assert_eq!(summary(&c).unwrap().listed_bytes, 7);
    }

    #[test]
    fn summary_flags_recent_files_and_compacts_totals() {
        let mut c = conn();
        let now = chrono::Utc::now().to_rfc3339();
        let old = "2001-01-01T00:00:00+00:00";
        apply_listing(
            &mut c,
            "s",
            100,
            &[f("new.csv", 40, Some(&now)), f("old.csv", 60, Some(old))],
            None,
        )
        .unwrap();

        let s = summary(&c).unwrap();
        assert_eq!(s.total_files, 2);
        assert_eq!(s.total_bytes, 100);
        assert_eq!(s.recent_count, 1);
        assert_eq!(s.newest_mtime.as_deref(), Some(now.as_str()));
        assert_eq!(s.added_last_sync, 2);
    }

    #[test]
    fn recent_flag_uses_the_same_window_as_the_summary() {
        let mut c = conn();
        let old = "2001-01-01T00:00:00+00:00";
        apply_listing(&mut c, "s", 0, &[f("old.csv", 1, Some(old))], None).unwrap();
        let (rows, _) = query_files(&c, "old.csv", 10, 0, "mtime", false).unwrap();
        assert!(!rows[0].recent);
    }

    #[test]
    fn processed_files_are_recorded_and_refreshed() {
        let c = conn();
        record_processed(&c, &[]).unwrap(); // no-op, must not fail
        record_processed(&c, &["2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv".to_string()])
            .unwrap();

        let first: String = c
            .query_row(
                "SELECT synced_at FROM processed_files WHERE file_name = ?1",
                params!["2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv"],
                |r| r.get(0),
            )
            .unwrap();

        // Re-recording the same name replaces its timestamp, never duplicates it.
        record_processed(&c, &["2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv".to_string()])
            .unwrap();
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM processed_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let again: String = c
            .query_row("SELECT synced_at FROM processed_files LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert!(again >= first, "the timestamp only moves forward");
    }

    #[test]
    fn a_file_row_carries_its_sync_time_by_base_name() {
        let mut c = conn();
        apply_listing(
            &mut c,
            "s",
            0,
            &[
                f("curitiba/2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv", 1, None),
                f("2026-10-08_Legisladoc_-_Info.xml", 1, None),
            ],
            None,
        )
        .unwrap();
        // Recorded by base name, matched even though the row path has a prefix.
        record_processed(&c, &["2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv".to_string()])
            .unwrap();

        let (rows, _) = query_files(&c, "", 10, 0, "path", true).unwrap();
        let ato = rows.iter().find(|r| r.path.ends_with("ATO_-_Base_de_Dados.csv")).unwrap();
        assert!(ato.synced_at.is_some(), "the ingested file carries a sync time");
        let info = rows.iter().find(|r| r.path.ends_with("Info.xml")).unwrap();
        assert!(info.synced_at.is_none(), "an unread file carries none");
    }
}
