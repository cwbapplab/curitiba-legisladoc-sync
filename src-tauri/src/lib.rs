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
mod parser;
mod remote;
mod schema;
mod store;
mod sync;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
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
    /// Guards against two notification runs overlapping — the scheduled sync and
    /// a manual "atualizar agora" both take it.
    emailing: AtomicBool,
    interval_minutes: AtomicU64,
    last_error: Mutex<Option<String>>,
    last_report: Mutex<Option<SyncReport>>,
    /// The last manual sync's matches. Kept so a window that reloads still
    /// shows the alert; the event alone would be missed.
    last_alert: Mutex<Option<email::SyncAlert>>,
}

/// Event carrying a manual sync's matches to the alert area.
pub const SYNC_MATCHES_EVENT: &str = "sync-matches";

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
    /// The last manual sync's matching acts, for the in-app alert area. Absent
    /// until a manual sync finds something.
    pub alert: Option<email::SyncAlert>,
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
async fn do_sync(app: &AppHandle, trigger: email::Trigger) -> anyhow::Result<SyncReport> {
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

    // Report the new acts over the channel the trigger chose: an in-app alert
    // for a manual sync, mail for an automatic one. Only a successful sync is
    // considered, and `notify_after_sync` decides from the watermark whether
    // anything is actually new.
    //
    // A manual sync is awaited: it sends no mail, so it is fast, and the alert
    // must be in place before `sync_now` resolves — the frontend refreshes its
    // snapshot as soon as the command returns. An automatic one stays detached
    // so a slow or dead SMTP server never delays the sync result; that is safe
    // because the watermark only ever moves forward.
    if err.is_none() {
        let handle = app.clone();
        let reported = report.clone();
        if trigger.emails() {
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = email::notify_after_sync(&handle, &reported, trigger) {
                    log::warn!("email notification run failed: {e:#}");
                }
            });
        } else if let Err(e) = email::notify_after_sync(&app, &report, trigger) {
            log::warn!("alert run failed: {e:#}");
        }
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

    // 3. Stamp the files this run ingested so the explorer can badge them. A
    //    failure here is cosmetic and must not fail an otherwise good sync.
    if !report.processed.is_empty() {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        if let Err(e) = db::record_processed(&conn, &report.processed) {
            log::warn!("recording processed files failed: {e:#}");
        }
    }

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
    do_sync(&app, email::Trigger::Manual)
        .await
        .map_err(|e| format!("{e:#}"))
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
        let alert = state.last_alert.lock().unwrap().clone();
        Ok(AppSnapshot {
            summary: db::summary(&conn).map_err(|e| format!("{e:#}"))?,
            last_sync: db::last_sync(&conn).map_err(|e| format!("{e:#}"))?,
            syncing: state.syncing.load(Ordering::SeqCst),
            source,
            interval_minutes: state.interval_minutes.load(Ordering::SeqCst),
            last_error,
            act_report,
            alert,
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

/// Email a single act to explicit recipients, at the user's request.
///
/// A side channel alongside the rule-driven notifications: it sends exactly the
/// act named, to the addresses given, using the stored SMTP settings and the
/// same message layout. It never reads or moves the watermark and writes no
/// history, so it cannot disturb the automatic runs.
#[tauri::command]
async fn send_act_email(app: AppHandle, id: i64, recipients: String) -> Result<(), String> {
    blocking(move || {
        let recipients = email::parse_recipients(&recipients);
        if recipients.is_empty() {
            return Err("informe pelo menos um destinatário".into());
        }
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let settings = email::load_settings(store.conn(), &app).map_err(|e| format!("{e:#}"))?;
        email::send_act(&store, &settings, id, &recipients).map_err(|e| format!("{e:#}"))
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

/// The simulation panel's first page: every enabled rule with what it matches
/// across the **whole acervo**, not just the acts since the last notification.
///
/// `limit` is the page size. Nothing is sent, no history is written, and the
/// watermark is left alone. There is deliberately no way to send from here: mail
/// only leaves the app on the automatic post-sync run.
#[tauri::command]
async fn simulate_email_rules(
    app: AppHandle,
    limit: i64,
) -> Result<Vec<email::RuleMatches>, String> {
    blocking(move || {
        let limit = limit.clamp(1, 200);
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let mut out = Vec::new();
        for rule in email::list_rules(store.conn())
            .map_err(|e| format!("{e:#}"))?
            .into_iter()
            .filter(|r| r.enabled)
        {
            // A rule whose column vanished from the feed must not blank the
            // whole panel; it is shown flagged instead.
            match email::query_rule(&store, &rule, None, 0, limit) {
                Ok(m) => out.push(m),
                Err(e) => {
                    log::warn!("regra {} não pôde ser simulada: {e:#}", rule.name);
                    out.push(email::failed_rule(rule));
                }
            }
        }
        Ok(out)
    })
    .await
}

/// One page of a single rule's matches over the whole acervo, for the panel's
/// pager.
#[tauri::command]
async fn simulate_email_rule_page(
    app: AppHandle,
    rule_id: i64,
    offset: i64,
    limit: i64,
) -> Result<email::RuleMatches, String> {
    blocking(move || {
        let limit = limit.clamp(1, 200);
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let rule = email::get_rule(store.conn(), rule_id).map_err(|e| format!("{e:#}"))?;
        email::query_rule(&store, &rule, None, offset.max(0), limit).map_err(|e| format!("{e:#}"))
    })
    .await
}

/// Tray menu item ids.
const TRAY_OPEN: &str = "tray-open";
const TRAY_QUIT: &str = "tray-quit";

/// Bring the main window back from the tray.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Tell the user the app is still running after the window is closed.
fn notify_background(app: &AppHandle) {
    use tauri_plugin_notification::NotificationExt;

    if let Err(e) = app
        .notification()
        .builder()
        .title("Legisladoc Sync")
        .body(
            "O aplicativo continua em segundo plano. Clique no ícone da bandeja para abrir; \
             clique com o botão direito para sair.",
        )
        .show()
    {
        log::warn!("could not show the background notification: {e}");
    }
}

/// Tray icon: left click opens the window, right click opens the menu.
fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, TRAY_OPEN, "Abrir Legisladoc Sync", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT, "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Legisladoc Sync")
        .menu(&menu)
        // The menu belongs to the right button; the left one opens the app.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_OPEN => show_main_window(app),
            TRAY_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            // Closing the window keeps the app alive in the tray; the tray menu
            // is the only place that really quits.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    notify_background(window.app_handle());
                }
            }
        })
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
                last_alert: Mutex::new(None),
            });

            setup_tray(app.handle())?;

            // Populate on first run, then keep refreshing on the configured
            // cadence. The UI loads whatever is already stored and updates when
            // `sync-complete` arrives, so a slow feed never blocks startup.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = do_sync(&handle, email::Trigger::Automatic).await {
                    log::warn!("initial refresh failed: {e:#}");
                }
                loop {
                    let minutes = {
                        let state = handle.state::<AppState>();
                        state.interval_minutes.load(Ordering::SeqCst).max(1)
                    };
                    tokio::time::sleep(Duration::from_secs(minutes * 60)).await;
                    if let Err(e) = do_sync(&handle, email::Trigger::Automatic).await {
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
            send_act_email,
            list_email_rules,
            save_email_rule,
            delete_email_rule,
            get_email_status,
            list_email_log,
            simulate_email_rules,
            simulate_email_rule_page
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
