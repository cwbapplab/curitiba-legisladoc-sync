//! Legisladoc feed browser.
//!
//! Two views over one remote feed:
//!
//! * The **act view** (primary) reads `Store`, a SQLite database built from the
//!   feed's own CSVs. Its tables and indexes are created at runtime from
//!   `Info.xml`, so the columns the UI can search and filter are the columns the
//!   feed actually publishes.
//! * The **rsync explorer** (secondary) reads `db`, a plain index of the remote
//!   file listing.
//!
//! Both live in the same SQLite file but are written by separate connections: the
//! listing refresh is cheap and frequent, while an act ingest is a long, batched
//! transaction that must not be interleaved with it.

mod db;
mod email;
mod feed;
mod ingest;
mod parser;
mod remote;
mod schema;
mod store;
mod sync;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use db::{LastSync, Summary, SyncOutcome};
use schema::Schema;
use store::{ActDetail, ActQuery, Facet, SearchResult, Store, StoreStats};
use sync::SyncReport;

/// Minutes between automatic refreshes unless the user changes it.
const DEFAULT_INTERVAL_MINUTES: u64 = 60;

pub struct AppState {
    /// The act data — the primary view.
    store: Mutex<Store>,
    /// The remote file listing — the rsync explorer view.
    db: Mutex<rusqlite::Connection>,
    /// rsync url currently being browsed; the UI can point at another feed.
    source: Mutex<String>,
    syncing: AtomicBool,
    /// Guards against two notification runs overlapping (a scheduled sync and a
    /// manual "rodar agora").
    emailing: AtomicBool,
    interval_minutes: AtomicU64,
    last_error: Mutex<Option<String>>,
    last_report: Mutex<Option<SyncReport>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPage {
    pub rows: Vec<db::FileRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub summary: Summary,
    pub last_sync: Option<LastSync>,
    pub syncing: bool,
    pub source: String,
    pub interval_minutes: u64,
    pub last_error: Option<String>,
    /// What the last act ingest did, so the UI can say "appended 12 new acts".
    pub act_report: Option<SyncReport>,
}

/// Which act columns the UI may filter on, and which it may sort by.
///
/// This is the runtime half of the "definitions drive the indexes" requirement:
/// the whitelists live beside the SQL that enforces them, and the UI builds its
/// controls from what is published here rather than from hard-coded field names.
#[derive(Debug, Clone, Serialize)]
pub struct QueryCapabilities {
    pub filters: Vec<&'static str>,
    pub sorts: Vec<&'static str>,
    /// `column -> help text` from `Info.xml`, for labelling the controls.
    pub labels: Vec<(String, String)>,
}

/// Run blocking work off the async runtime.
///
/// The act ingest is synchronous, long-running I/O over a SQLite connection.
/// Awaiting it inline would occupy a runtime worker for minutes, and worse, the
/// act store mutex would be held across an await point; this keeps the command
/// handlers responsive while a refresh runs.
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("falha na tarefa em segundo plano: {e}"))?
}

/// Re-list the remote feed, then bring the act database up to date.
///
/// Only one refresh runs at a time; a second caller returns an error immediately
/// rather than queueing duplicate work against a public server.
async fn do_sync(app: &AppHandle) -> anyhow::Result<SyncReport> {
    if app.state::<AppState>().syncing.swap(true, Ordering::SeqCst) {
        anyhow::bail!("uma sincronização já está em andamento");
    }
    // Clear the flag however we leave this function, including on panic.
    let _guard = SyncGuard { app };

    let source = app.state::<AppState>().source.lock().unwrap().clone();
    let _ = app.emit("sync-started", &source);

    let result = refresh_all(app, &source).await;
    let (report, err) = match result {
        Ok(r) => (r, None),
        Err(e) => {
            let msg = format!("{e:#}");
            log::warn!("refresh failed: {msg}");
            // A failed refresh must not empty either local copy; it only gets
            // recorded, so the UI can show why the data is stale.
            let state = app.state::<AppState>();
            let _ = db::record_failed_sync(&state.db.lock().unwrap(), &source, &msg);
            *state.last_error.lock().unwrap() = Some(msg.clone());
            (SyncReport::default(), Some(msg))
        }
    };

    {
        let state = app.state::<AppState>();
        *state.last_report.lock().unwrap() = Some(report.clone());
    }

    let payload = SyncOutcome {
        added: report.rows_added,
        updated: 0,
        removed: 0,
        total: report.total_rows,
    };
    let _ = app.emit("sync-complete", &payload);

    // Notify on new acts, off the async runtime and fire-and-forget: a slow or
    // dead SMTP server must never delay the sync result. Only a successful sync
    // is considered, and the email step itself decides whether the action
    // warrants sending (see `email::notify_after_sync`).
    if err.is_none() {
        let handle = app.clone();
        let action = report.action.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = email::notify_after_sync(&handle, &action) {
                log::warn!("email notification run failed: {e:#}");
            }
        });
    }

    match err {
        Some(e) => anyhow::bail!(e),
        None => Ok(report),
    }
}

/// The body of a refresh, split out so `do_sync` does not have to thread the
/// error path through every step.
async fn refresh_all(app: &AppHandle, source: &str) -> anyhow::Result<SyncReport> {
    // 1. The rsync listing drives file discovery for both views.
    let listing = feed::fetch_listing(source).await?;

    {
        let state = app.state::<AppState>();
        let mut conn = state.db.lock().unwrap();
        db::apply_listing(&mut conn, source, listing.total_bytes, &listing.files, None)?;
    }

    // 2. The act ingest is blocking I/O; keep it off the async runtime.
    let handle = app.clone();
    let report = blocking(move || {
        let state = handle.state::<AppState>();
        let mut store = state.store.lock().unwrap();
        let report = sync::refresh(&handle, &mut store, &listing)
            .map_err(|e| format!("{e:#}"))?;
        Ok(report)
    })
    .await
    .map_err(anyhow::Error::msg)?;

    Ok(report)
}

/// Resets the syncing flag on drop, so an early return can't wedge the app.
struct SyncGuard<'a> {
    app: &'a AppHandle,
}
impl Drop for SyncGuard<'_> {
    fn drop(&mut self) {
        self.app
            .state::<AppState>()
            .syncing
            .store(false, Ordering::SeqCst);
    }
}

// ------------------------------------------------------------------ commands

#[tauri::command]
async fn sync_now(app: AppHandle) -> Result<SyncReport, String> {
    do_sync(&app).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn get_state(app: AppHandle) -> Result<AppSnapshot, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        // The lock guards are bound before the struct literal so they are dropped
        // before `state`, which is a borrowed `State<'_, _>`.
        let source = state.source.lock().unwrap().clone();
        let last_error = state.last_error.lock().unwrap().clone();
        let act_report = state.last_report.lock().unwrap().clone();
        Ok(AppSnapshot {
            summary: db::summary(&conn).map_err(|e| format!("{e:#}"))?,
            last_sync: db::last_sync(&conn).map_err(|e| format!("{e:#}"))?,
            syncing: state.syncing.load(Ordering::SeqCst),
            source,
            interval_minutes: state.interval_minutes.load(Ordering::SeqCst),
            last_error,
            act_report,
        })
    })
    .await
}

#[tauri::command]
fn set_interval(state: State<'_, AppState>, minutes: u64) -> Result<(), String> {
    if minutes == 0 {
        return Err("o intervalo deve ser de pelo menos 1 minuto".into());
    }
    state.interval_minutes.store(minutes, Ordering::SeqCst);
    Ok(())
}

/// Aggregates for the dashboard tiles.
#[tauri::command]
async fn act_stats(app: AppHandle) -> Result<StoreStats, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        store.stats().map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn search_acts(app: AppHandle, query: ActQuery) -> Result<SearchResult, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        store.search(&query).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn act_detail(app: AppHandle, id: i64) -> Result<Option<ActDetail>, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let schema = store
            .load_schema()
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_else(empty_schema);
        store.act_detail(&schema, id).map_err(|e| format!("{e:#}"))
    })
    .await
}

/// Distinct values (with counts) for one filterable column.
#[tauri::command]
async fn act_facets(app: AppHandle, column: String, limit: Option<i64>) -> Result<Vec<Facet>, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        store
            .facet(&column, limit.unwrap_or(40))
            .map_err(|e| format!("{e:#}"))
    })
    .await
}

/// The columns the UI may filter and sort on, plus their published labels.
#[tauri::command]
async fn query_capabilities(app: AppHandle) -> Result<QueryCapabilities, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let schema = store
            .load_schema()
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_else(empty_schema);
        Ok(capabilities(&schema))
    })
    .await
}

/// The values one column can take, when `Info.xml` defines a type for it.
///
/// `null` means the column is free text: the filter UI keeps a text field for it
/// instead of offering a list of choices.
#[tauri::command]
async fn act_column_values(
    app: AppHandle,
    column: String,
    limit: Option<i64>,
) -> Result<Option<Vec<store::ColumnValue>>, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let schema = store
            .load_schema()
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_else(empty_schema);
        store
            .column_values(&schema, &column, limit.unwrap_or(200))
            .map_err(|e| format!("{e:#}"))
    })
    .await
}

/// Resolve a filterable/sortable column to the label `Info.xml` gives it.
fn capabilities(schema: &Schema) -> QueryCapabilities {
    let labels = schema
        .table("ATO")
        .map(|t| {
            t.columns
                .iter()
                .map(|c| (c.name.clone(), c.help.clone().unwrap_or_else(|| c.name.clone())))
                .collect()
        })
        .unwrap_or_default();
    QueryCapabilities {
        filters: store::filter_columns().to_vec(),
        sorts: store::sort_columns().to_vec(),
        labels,
    }
}

fn empty_schema() -> Schema {
    Schema {
        version: None,
        provider: None,
        tables: Vec::new(),
    }
}

// ------------------------------------------------------- rsync explorer page

#[tauri::command]
fn search_files(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
    offset: Option<i64>,
    sort_by: Option<String>,
    ascending: Option<bool>,
) -> Result<SearchPage, String> {
    let conn = state.db.lock().unwrap();
    let (rows, total) = db::query_files(
        &conn,
        &query,
        limit.unwrap_or(200),
        offset.unwrap_or(0),
        sort_by.as_deref().unwrap_or("mtime"),
        ascending.unwrap_or(false),
    )
    .map_err(|e| format!("{e:#}"))?;
    Ok(SearchPage { rows, total })
}

// --------------------------------------------------------- email automático

use email::{EmailLogEntry, EmailRule, EmailRuleInput, EmailSettingsInput, EmailSettingsView, EmailStatus};

/// Current SMTP configuration, without the password.
#[tauri::command]
async fn get_email_settings(app: AppHandle) -> Result<EmailSettingsView, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::load_settings(store.conn(), &app)
            .map(|s| s.view())
            .map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn save_email_settings(
    app: AppHandle,
    settings: EmailSettingsInput,
) -> Result<EmailSettingsView, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::save_settings(store.conn(), &app, &settings).map_err(|e| format!("{e:#}"))
    })
    .await
}

/// Prove TLS + authentication without waiting for a sync.
#[tauri::command]
async fn test_email_connection(app: AppHandle) -> Result<(), String> {
    blocking(move || {
        let settings = {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            email::load_settings(store.conn(), &app).map_err(|e| format!("{e:#}"))?
        };
        email::test_connection(&settings, None).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn send_test_email(app: AppHandle, to: String) -> Result<(), String> {
    blocking(move || {
        let settings = {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            email::load_settings(store.conn(), &app).map_err(|e| format!("{e:#}"))?
        };
        email::test_connection(&settings, Some(&to)).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn list_email_rules(app: AppHandle) -> Result<Vec<EmailRule>, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::list_rules(store.conn()).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn save_email_rule(app: AppHandle, rule: EmailRuleInput) -> Result<EmailRule, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::save_rule(store.conn(), &rule).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn delete_email_rule(app: AppHandle, id: i64) -> Result<(), String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::delete_rule(store.conn(), id).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn get_email_status(app: AppHandle) -> Result<EmailStatus, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::status(store.conn()).map_err(|e| format!("{e:#}"))
    })
    .await
}

#[tauri::command]
async fn list_email_log(app: AppHandle, limit: Option<i64>) -> Result<Vec<EmailLogEntry>, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        email::list_log(store.conn(), limit.unwrap_or(50)).map_err(|e| format!("{e:#}"))
    })
    .await
}

/// Manual run. `send = false` is a dry run. `since_watermark = false` matches the
/// whole store, so it can be exercised without waiting for a feed append — and,
/// deliberately, it never advances the automatic watermark.
#[tauri::command]
async fn run_email_rules_now(
    app: AppHandle,
    send: bool,
    since_watermark: bool,
) -> Result<Vec<EmailLogEntry>, String> {
    blocking(move || {
        let floor = if since_watermark {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            let count = email::act_count_or_zero(store.conn()).map_err(|e| format!("{e:#}"))?;
            let max = store.max_act_id().map_err(|e| format!("{e:#}"))?;
            email::effective_floor(store.conn(), count, max).map_err(|e| format!("{e:#}"))?
        } else {
            0
        };
        let log = email::run(&app, floor, send).map_err(|e| format!("{e:#}"))?;
        let _ = app.emit("email-notified", &log);
        Ok(log)
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let file = dir.join("legisladoc.sqlite");

            app.manage(AppState {
                store: Mutex::new(Store::open(&file)?),
                db: Mutex::new(db::open(&file)?),
                source: Mutex::new(feed::default_url()),
                syncing: AtomicBool::new(false),
                emailing: AtomicBool::new(false),
                interval_minutes: AtomicU64::new(DEFAULT_INTERVAL_MINUTES),
                last_error: Mutex::new(None),
                last_report: Mutex::new(None),
            });

            // Populate on first run, then keep refreshing on the configured
            // cadence. The UI loads whatever is already stored and updates when
            // `sync-complete` arrives, so a slow feed never blocks startup.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = do_sync(&handle).await {
                    log::warn!("initial refresh failed: {e:#}");
                }
                loop {
                    let minutes = {
                        let state = handle.state::<AppState>();
                        state.interval_minutes.load(Ordering::SeqCst).max(1)
                    };
                    tokio::time::sleep(Duration::from_secs(minutes * 60)).await;
                    if let Err(e) = do_sync(&handle).await {
                        log::warn!("scheduled refresh failed: {e:#}");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sync_now,
            get_state,
            set_interval,
            act_stats,
            search_acts,
            act_detail,
            act_facets,
            act_column_values,
            query_capabilities,
            search_files,
            get_email_settings,
            save_email_settings,
            test_email_connection,
            send_test_email,
            list_email_rules,
            save_email_rule,
            delete_email_rule,
            get_email_status,
            list_email_log,
            run_email_rules_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
