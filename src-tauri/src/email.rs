//! Email notifications for newly-appended acts.
//!
//! The user defines **rules**: each is a saved filter — the same column/operator
//! conditions the act search uses, plus an optional full-text term — with its own
//! recipients and an on/off switch. After a sync that appends new acts, every
//! enabled rule is run against just those acts, and a match is emailed.
//!
//! Two things make this safe:
//!
//! * A **watermark** (the highest act id already notified about) decides what is
//!   "new". A full reload clears the act table, so a raw `MAX(ID)` diff would
//!   treat the whole feed as new; the watermark is advanced without sending when
//!   that happens.
//! * The SMTP password is stored **encrypted** (AES-256-GCM) with a key file
//!   beside the database. This keeps the password out of a copied database or a
//!   backup, but a local attacker who can read both files can recover it — it is
//!   not protection against someone already running code on the machine.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::schema::Schema;
use crate::AppState;

// ---------------------------------------------------------------- constants

/// The only version byte this build writes, and the only one it reads.
const ENC_VERSION: u8 = 0x01;
/// AAD, so a blob lifted into another column/row fails to authenticate.
const ENC_AAD: &[u8] = b"legisladoc-email-v1";
const NAME_KEY_FILE: &str = "email.key";

/// Acts listed per email. Bounds message size and SMTP time; a rule that matches
/// more says so rather than sending thousands of rows.
pub const MAX_ITEMS_PER_EMAIL: i64 = 100;

/// Google's SMTP endpoint, filled in for the `google` provider preset.
pub const GOOGLE_HOST: &str = "smtp.gmail.com";
pub const GOOGLE_PORT: u16 = 587;

// ---------------------------------------------------------------- schema

/// Create the email tables. Called from `Store::init`, because these live in the
/// same SQLite file and must exist on the one startup path (and in tests).
pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- One row. The password is never stored in the clear.
        CREATE TABLE IF NOT EXISTS email_settings (
            id           INTEGER PRIMARY KEY CHECK (id = 1),
            provider     TEXT NOT NULL DEFAULT 'custom',
            host         TEXT NOT NULL DEFAULT '',
            port         INTEGER NOT NULL DEFAULT 587,
            security     TEXT NOT NULL DEFAULT 'starttls',
            username     TEXT NOT NULL DEFAULT '',
            password_enc BLOB,
            from_name    TEXT NOT NULL DEFAULT 'Legisladoc',
            from_email   TEXT NOT NULL DEFAULT '',
            updated_at   TEXT NOT NULL
        );

        -- One row per rule. `filters_json` holds a serde_json Vec<store::Filter>,
        -- so the conditions are the very shape the search UI builds and the same
        -- SQL validates.
        CREATE TABLE IF NOT EXISTS email_rules (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            name         TEXT NOT NULL,
            enabled      INTEGER NOT NULL DEFAULT 1,
            text         TEXT,
            filters_json TEXT NOT NULL DEFAULT '[]',
            recipients   TEXT NOT NULL DEFAULT '',
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL
        );

        -- What each run did, per rule. Kept even for skipped/failed rules so the
        -- UI can explain a silent notification.
        CREATE TABLE IF NOT EXISTS email_log (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            at         TEXT NOT NULL,
            rule_id    INTEGER,
            rule_name  TEXT NOT NULL,
            recipients TEXT NOT NULL,
            matched    INTEGER NOT NULL DEFAULT 0,
            status     TEXT NOT NULL,
            error      TEXT,
            act_ids    TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_email_log_at ON email_log(at DESC);

        -- The watermark: highest act id already notified about.
        CREATE TABLE IF NOT EXISTS email_state (
            id           INTEGER PRIMARY KEY CHECK (id = 1),
            watermark_id INTEGER NOT NULL DEFAULT 0,
            updated_at   TEXT NOT NULL
        );
        "#,
    )
    .context("creating email schema")?;
    Ok(())
}

// ---------------------------------------------------------------- model

#[derive(Debug, Clone, Serialize)]
pub struct EmailSettingsView {
    pub provider: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    /// Whether a password is stored. The password itself never leaves the backend.
    pub has_password: bool,
    pub from_name: String,
    pub from_email: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailSettingsInput {
    pub provider: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    /// `None` or empty keeps the stored password; `Some(s)` replaces it.
    #[serde(default)]
    pub password: Option<String>,
    /// Explicitly drop the stored password.
    #[serde(default)]
    pub clear_password: bool,
    pub from_name: String,
    pub from_email: String,
}

/// The stored settings, password included — internal to the backend only.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub provider: String,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    pub password: Option<String>,
    pub from_name: String,
    pub from_email: String,
}

impl Settings {
    pub fn view(&self) -> EmailSettingsView {
        EmailSettingsView {
            provider: self.provider.clone(),
            host: self.host.clone(),
            port: self.port,
            security: self.security.clone(),
            username: self.username.clone(),
            has_password: self.password.is_some(),
            from_name: self.from_name.clone(),
            from_email: self.from_email.clone(),
        }
    }

    /// Google or custom endpoint, as the transport switch needs it.
    fn endpoint(&self) -> Endpoint {
        if self.provider == "google" {
            Endpoint {
                host: GOOGLE_HOST.to_string(),
                port: GOOGLE_PORT,
                security: Security::Starttls,
            }
        } else {
            Endpoint {
                host: self.host.clone(),
                port: self.port,
                security: Security::from_str(&self.security),
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Endpoint {
    host: String,
    port: u16,
    security: Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Security {
    Starttls,
    Tls,
    None,
}

impl Security {
    fn from_str(s: &str) -> Self {
        match s {
            "tls" => Security::Tls,
            "none" => Security::None,
            _ => Security::Starttls,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailRule {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub text: Option<String>,
    pub filters: Vec<crate::store::Filter>,
    pub recipients: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmailRuleInput {
    /// Absent or 0 for a new rule.
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub filters: Vec<crate::store::Filter>,
    /// Free text; addresses are split on `;`, `,` and whitespace.
    #[serde(default)]
    pub recipients: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailLogEntry {
    pub at: String,
    pub rule_id: Option<i64>,
    pub rule_name: String,
    pub recipients: String,
    pub matched: i64,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailStatus {
    pub watermark_id: i64,
    pub enabled_rules: i64,
    pub act_count: i64,
}

/// A manual sync's matches, for the in-app alert area.
///
/// Deliberately not an `EmailLogEntry`: that type is *history*, and a manual
/// sync must not write history — nothing was sent. It also cannot carry the
/// sample acts, since `run` drops `act_ids` on the way out and the log never
/// stored the fields the UI renders.
#[derive(Debug, Clone, Serialize)]
pub struct SyncAlert {
    /// The feed file the acts came from, so the alert can name the day.
    pub snapshot: String,
    pub max_id: i64,
    pub total_matched: i64,
    pub rules: Vec<SyncAlertRule>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncAlertRule {
    pub rule_id: i64,
    pub rule_name: String,
    /// The full match count, which may exceed `samples.len()`.
    pub matched: i64,
    pub samples: Vec<SyncAlertAct>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncAlertAct {
    pub id: i64,
    pub numero_identificacao: Option<String>,
    pub data_publicacao: Option<String>,
    pub tipo: Option<String>,
}

/// Acts listed per rule in the alert. The alert is a nudge, not a report; a
/// rule matching hundreds says so via `matched` rather than enumerating them.
pub const MAX_ALERT_SAMPLES: usize = 5;

/// Project a run's matches into the alert payload. Pure, so the sampling and
/// the total are testable without a database or an `AppHandle`.
pub fn build_alert(matches: &[RuleMatches], snapshot: &str, max_id: i64) -> SyncAlert {
    let mut total_matched = 0;
    let mut rules = Vec::new();
    for m in matches {
        if m.total <= 0 {
            continue; // nothing matched; not worth a line in the alert
        }
        total_matched += m.total;
        rules.push(SyncAlertRule {
            rule_id: m.rule.id,
            rule_name: m.rule.name.clone(),
            matched: m.total,
            samples: m
                .rows
                .iter()
                .take(MAX_ALERT_SAMPLES)
                .map(|r| SyncAlertAct {
                    id: r.id,
                    numero_identificacao: r.numero_identificacao.clone(),
                    data_publicacao: r.data_publicacao.clone(),
                    tipo: r.tipo.clone(),
                })
                .collect(),
        });
    }
    SyncAlert {
        snapshot: snapshot.to_string(),
        max_id,
        total_matched,
        rules,
    }
}

// ---------------------------------------------------------------- crypto

/// Encrypt `plaintext` into the on-disk blob layout:
/// `[version][12-byte nonce][ciphertext ‖ 16-byte tag]`.
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, AeadCore, OsRng, Payload};
    use aes_gcm::{Aes256Gcm, KeyInit};

    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
    let nonce_bytes = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(
            &nonce_bytes,
            Payload {
                msg: plaintext.as_bytes(),
                aad: ENC_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("falha ao cifrar a senha"))?;

    let mut out = Vec::with_capacity(1 + 12 + ct.len());
    out.push(ENC_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Inverse of [`encrypt`]. Rejects an unknown version byte and any tampering.
pub fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<String> {
    use aes_gcm::aead::{Aead, Payload};
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

    if blob.len() < 1 + 12 + 16 {
        anyhow::bail!("senha guardada está corrompida");
    }
    if blob[0] != ENC_VERSION {
        anyhow::bail!("versão de senha guardada desconhecida: {}", blob[0]);
    }
    let (nonce_bytes, ct) = blob[1..].split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
    let pt = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ct,
                aad: ENC_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("não foi possível decifrar a senha (chave trocada ou dado alterado)"))?;
    String::from_utf8(pt).context("a senha guardada não é texto válido")
}

/// Load the key file, creating it on first use. Sits beside the database.
fn load_or_create_key(app: &AppHandle) -> Result<[u8; 32]> {
    // `generate_key` is a `KeyInit` method, so that trait must be in scope (unlike
    // `generate_nonce`, which comes from `AeadCore`).
    use aes_gcm::aead::OsRng;
    use aes_gcm::{Aes256Gcm, KeyInit};

    let path = key_path(app)?;
    if path.exists() {
        let bytes = std::fs::read(&path).with_context(|| format!("lendo {}", path.display()))?;
        let key: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("arquivo de chave com tamanho inválido"))?;
        return Ok(key);
    }

    let generated = Aes256Gcm::generate_key(&mut OsRng);
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&generated);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, key_bytes).with_context(|| format!("gravando {}", path.display()))?;
    restrict_permissions(&path);
    Ok(key_bytes)
}

fn key_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .context("não foi possível localizar a pasta de dados do aplicativo")?;
    Ok(dir.join(NAME_KEY_FILE))
}

/// Best effort: on Unix the key file is owner-only. On Windows there is no
/// portable equivalent, which is part of why the encryption's guarantee is
/// limited to keeping the password out of the database file itself.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

// ---------------------------------------------------------------- settings

pub fn load_settings(conn: &Connection, app: &AppHandle) -> Result<Settings> {
    let row = conn
        .query_row(
            "SELECT provider, host, port, security, username, password_enc, from_name, from_email
             FROM email_settings WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<Vec<u8>>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?;

    let Some((provider, host, port, security, username, enc, from_name, from_email)) = row else {
        return Ok(default_settings());
    };

    let password = match enc {
        Some(blob) => Some(decrypt(&load_or_create_key(app)?, &blob)?),
        None => None,
    };
    Ok(Settings {
        provider,
        host,
        port: port as u16,
        security,
        username,
        password,
        from_name,
        from_email,
    })
}

fn default_settings() -> Settings {
    Settings {
        provider: "custom".into(),
        host: String::new(),
        port: 587,
        security: "starttls".into(),
        username: String::new(),
        password: None,
        from_name: "Legisladoc".into(),
        from_email: String::new(),
    }
}

/// Persist settings, keeping or replacing the password as the input asks.
///
/// Returns the view (never the password). Runs in one transaction; the key file
/// is only touched when a password is actually being written or cleared.
pub fn save_settings(conn: &Connection, app: &AppHandle, input: &EmailSettingsInput) -> Result<EmailSettingsView> {
    // Preserve the existing blob unless the caller supplied or cleared one.
    let existing: Option<Vec<u8>> = conn
        .query_row("SELECT password_enc FROM email_settings WHERE id = 1", [], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();

    let new_blob: Option<Vec<u8>> = if input.clear_password {
        None
    } else {
        match input.password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            // Google shows the app password with embedded spaces; strip them.
            Some(pw) => Some(encrypt(&load_or_create_key(app)?, &pw.replace(' ', ""))?),
            None => existing,
        }
    };

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO email_settings
            (id, provider, host, port, security, username, password_enc, from_name, from_email, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            provider = excluded.provider,
            host = excluded.host,
            port = excluded.port,
            security = excluded.security,
            username = excluded.username,
            password_enc = excluded.password_enc,
            from_name = excluded.from_name,
            from_email = excluded.from_email,
            updated_at = excluded.updated_at",
        params![
            input.provider,
            input.host.trim(),
            input.port as i64,
            input.security,
            input.username.trim(),
            new_blob,
            input.from_name.trim(),
            input.from_email.trim(),
            now,
        ],
    )?;

    Ok(Settings {
        provider: input.provider.clone(),
        host: input.host.trim().into(),
        port: input.port,
        security: input.security.clone(),
        username: input.username.trim().into(),
        password: new_blob.map(|_| String::new()),
        from_name: input.from_name.trim().into(),
        from_email: input.from_email.trim().into(),
    }
    .view())
}

// ---------------------------------------------------------------- rules

/// Split a free-text recipient list into individual addresses.
pub fn parse_recipients(raw: &str) -> Vec<String> {
    raw.split([';', ',', '\n'])
        .flat_map(str::split_whitespace)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Map a `email_rules` row (id, name, enabled, text, filters_json, recipients).
fn rule_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<EmailRule> {
    let filters_json: String = r.get(4)?;
    let recipients: String = r.get(5)?;
    Ok(EmailRule {
        id: r.get(0)?,
        name: r.get(1)?,
        enabled: r.get::<_, i64>(2)? != 0,
        text: r.get(3)?,
        filters: serde_json::from_str(&filters_json).unwrap_or_default(),
        recipients: parse_recipients(&recipients),
    })
}

pub fn list_rules(conn: &Connection) -> Result<Vec<EmailRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, enabled, text, filters_json, recipients FROM email_rules ORDER BY id",
    )?;
    let rows = stmt.query_map([], rule_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// One rule by id, for the simulation panel's pager.
pub fn get_rule(conn: &Connection, id: i64) -> Result<EmailRule> {
    conn.query_row(
        "SELECT id, name, enabled, text, filters_json, recipients FROM email_rules WHERE id = ?1",
        params![id],
        rule_from_row,
    )
    .optional()?
    .ok_or_else(|| anyhow::anyhow!("regra {id} não encontrada"))
}

pub fn save_rule(conn: &Connection, input: &EmailRuleInput) -> Result<EmailRule> {
    let name = input.name.trim();
    if name.is_empty() {
        anyhow::bail!("a regra precisa de um nome");
    }
    let recipients = parse_recipients(&input.recipients);
    if recipients.is_empty() {
        anyhow::bail!("a regra precisa de pelo menos um destinatário");
    }
    let filters_json = serde_json::to_string(&input.filters)?;
    let text = input.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let now = chrono::Utc::now().to_rfc3339();
    let recipients_raw = recipients.join("; ");

    let id = match input.id {
        Some(id) if id > 0 => {
            conn.execute(
                "UPDATE email_rules SET name = ?1, enabled = ?2, text = ?3, filters_json = ?4,
                        recipients = ?5, updated_at = ?6 WHERE id = ?7",
                params![name, input.enabled as i64, text, filters_json, recipients_raw, now, id],
            )?;
            id
        }
        _ => {
            conn.execute(
                "INSERT INTO email_rules (name, enabled, text, filters_json, recipients, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![name, input.enabled as i64, text, filters_json, recipients_raw, now],
            )?;
            conn.last_insert_rowid()
        }
    };

    Ok(EmailRule {
        id,
        name: name.to_string(),
        enabled: input.enabled,
        text: text.map(str::to_string),
        filters: input.filters.clone(),
        recipients,
    })
}

pub fn delete_rule(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM email_rules WHERE id = ?1", params![id])?;
    Ok(())
}

/// Rewrite saved rule filters that name a foreign-key column by its dimension
/// *key* into the value the acts actually carry.
///
/// The feed's `Info.xml` declares e.g. `TIPO_ATO_ID` as `TIPO_ATO.ID` (a
/// `smallint`), but the snapshot stores the type's **name** in that column
/// (`"ACORDÃO"`, not `"50"`). A rule saved while the schema was taken literally
/// holds the key and silently matches nothing; this maps it to the name. Both the
/// key and the name map to the name, so a second pass is a no-op.
///
/// Runs after the dimension tables are (re)loaded, which is the point at which
/// the mapping is known to be current. Returns how many rules were rewritten.
pub fn normalize_related_filters(conn: &Connection, schema: &Schema) -> Result<usize> {
    use std::collections::HashMap;

    let Some(ato) = schema.table("ATO") else {
        return Ok(0);
    };
    let safe = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');

    // column -> (stored token -> canonical name).
    let mut maps: HashMap<String, HashMap<String, String>> = HashMap::new();
    for col in &ato.columns {
        let Some(rel) = col.related.as_ref() else {
            continue;
        };
        if !safe(&rel.table) || !safe(&rel.key) {
            continue;
        }
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![rel.table],
            |r| r.get(0),
        )?;
        if !exists {
            continue;
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT \"{key}\", \"NOME\" FROM \"{table}\"
             WHERE \"NOME\" IS NOT NULL AND \"NOME\" != ''",
            key = rel.key,
            table = rel.table
        ))?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (key, nome) = row?;
            // The name maps to itself, which is what makes this idempotent.
            map.insert(nome.clone(), nome.clone());
            map.insert(key, nome);
        }
        if !map.is_empty() {
            maps.insert(col.name.clone(), map);
        }
    }
    if maps.is_empty() {
        return Ok(0);
    }

    let mut changed = 0usize;
    for rule in list_rules(conn)? {
        let mut filters = rule.filters.clone();
        let mut touched = false;
        for f in &mut filters {
            let Some(map) = maps.get(&f.column) else {
                continue;
            };
            if let Some(name) = map.get(&f.value) {
                if *name != f.value {
                    f.value = name.clone();
                    touched = true;
                }
            }
            for v in &mut f.values {
                if let Some(name) = map.get(v) {
                    if *name != *v {
                        *v = name.clone();
                        touched = true;
                    }
                }
            }
        }
        if touched {
            conn.execute(
                "UPDATE email_rules SET filters_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![
                    serde_json::to_string(&filters)?,
                    chrono::Utc::now().to_rfc3339(),
                    rule.id
                ],
            )?;
            changed += 1;
        }
    }
    Ok(changed)
}

// ---------------------------------------------------------------- watermark

pub fn watermark(conn: &Connection) -> Result<i64> {
    Ok(conn
        .query_row("SELECT watermark_id FROM email_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .optional()?
        .unwrap_or(0))
}

/// Move the watermark forward, never backward.
///
/// The notify step runs detached from the sync that triggered it (see
/// `do_sync`), so it can outlive its own sync and finish *after* a later sync
/// has already advanced the watermark. Writing its stale `max_id` verbatim
/// would rewind the floor and make the next run re-notify acts already seen —
/// hence the `MAX` in the upsert. `advance_watermark` mirrors this for tests.
pub(crate) fn set_watermark(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO email_state (id, watermark_id, updated_at) VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET watermark_id = MAX(email_state.watermark_id, excluded.watermark_id),
                                     updated_at = excluded.updated_at",
        params![id, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// The value `set_watermark` would leave behind: the larger of the two.
pub fn advance_watermark(current: i64, candidate: i64) -> i64 {
    current.max(candidate)
}

/// The floor to match new acts against, or `None` when nothing is new.
///
/// "New" means an act id above the watermark. This is deliberately **not**
/// derived from `action`: the feed publishes a new dated file each day, so a
/// daily ingest always reports `"reloaded"`, and a reload rewrites the whole
/// table — comparing against `MIN(ID)` would treat the entire acervo as new.
/// The watermark is the only safe floor, because it lives in `email_state`
/// rather than in `acts` and therefore survives the reload.
pub fn notification_floor(watermark: i64, max_id: Option<i64>) -> Option<i64> {
    match max_id {
        Some(max) if max > watermark => Some(watermark),
        _ => None,
    }
}

/// Take the notification slot, or report that a run is already in flight.
pub fn try_begin_notification(flag: &std::sync::atomic::AtomicBool) -> bool {
    !flag.swap(true, std::sync::atomic::Ordering::SeqCst)
}

/// The floor to match new acts against.
///
/// A first run against an already-populated database has no watermark yet; it is
/// seeded to the current maximum **without notifying**, which is what keeps an
/// upgrade from mailing the entire feed.
pub fn effective_floor(conn: &Connection, act_count: i64, max_id: Option<i64>) -> Result<i64> {
    let stored = watermark(conn)?;
    if stored == 0 && act_count > 0 {
        let seed = max_id.unwrap_or(0);
        set_watermark(conn, seed)?;
        return Ok(seed);
    }
    Ok(stored)
}

// ---------------------------------------------------------------- status / log

pub fn status(conn: &Connection) -> Result<EmailStatus> {
    Ok(EmailStatus {
        watermark_id: watermark(conn)?,
        enabled_rules: conn.query_row(
            "SELECT COUNT(*) FROM email_rules WHERE enabled = 1",
            [],
            |r| r.get(0),
        )?,
        act_count: act_count_or_zero(conn)?,
    })
}

/// Count the acts, treating "the table does not exist yet" as zero.
///
/// The `acts` table is created by `ensure_act_tables` on the first successful
/// sync, so before that there is simply nothing — not an error.
pub fn act_count_or_zero(conn: &Connection) -> Result<i64> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'acts')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    Ok(conn.query_row("SELECT COUNT(*) FROM acts", [], |r| r.get(0))?)
}

pub fn list_log(conn: &Connection, limit: i64) -> Result<Vec<EmailLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT at, rule_id, rule_name, recipients, matched, status, error
         FROM email_log ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit.clamp(1, 500)], |r| {
        Ok(EmailLogEntry {
            at: r.get(0)?,
            rule_id: r.get(1)?,
            rule_name: r.get(2)?,
            recipients: r.get(3)?,
            matched: r.get(4)?,
            status: r.get(5)?,
            error: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn log_entry(conn: &Connection, entry: &LogRow) -> Result<()> {
    conn.execute(
        "INSERT INTO email_log (at, rule_id, rule_name, recipients, matched, status, error, act_ids)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            entry.at,
            entry.rule_id,
            entry.rule_name,
            entry.recipients,
            entry.matched,
            entry.status,
            entry.error,
            entry.act_ids,
        ],
    )?;
    Ok(())
}

struct LogRow {
    /// When the rule was evaluated. Carded here rather than at insert time so a
    /// dry run, which is never persisted, still reports a timestamp.
    at: String,
    rule_id: Option<i64>,
    rule_name: String,
    recipients: String,
    matched: i64,
    status: String,
    error: Option<String>,
    act_ids: Option<String>,
}

// ---------------------------------------------------------------- composition

/// Escape untrusted feed text for the HTML part. The act body is rendered in a
/// sandboxed iframe elsewhere, but these summary fields go straight into a
/// message, so they are escaped rather than trusted.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// A rule's matches, ready to be rendered.
///
/// Serialized for the simulation panel, which lists the acts themselves rather
/// than only saying how many matched.
#[derive(Debug, Clone, Serialize)]
pub struct RuleMatches {
    pub rule: EmailRule,
    pub total: i64,
    pub rows: Vec<crate::store::ActRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Composed {
    pub subject: String,
    pub text: String,
    pub html: String,
}

/// Where a rule's matches came from, for the summary line.
pub fn compose(matches: &RuleMatches) -> Composed {
    let n = matches.total;
    let plural = if n == 1 { "ato novo" } else { "atos novos" };
    let subject = format!("[Legisladoc] {} — {n} {plural}", matches.rule.name);

    let mut text = format!(
        "{}: {n} {plural} encontrado(s).\n\n",
        matches.rule.name
    );
    let mut html_rows = String::new();
    for r in &matches.rows {
        let ident = r.numero_identificacao.as_deref().unwrap_or("—");
        let oficial = r.numero_oficial.as_deref().unwrap_or("—");
        let data = r.data_publicacao.as_deref().map(|d| d.get(0..10).unwrap_or(d)).unwrap_or("—");
        let tipo = r.tipo.as_deref().unwrap_or("—");
        let orgao = r.orgao.as_deref().unwrap_or("—");
        let situacao = r.situacao.as_deref().unwrap_or("—");
        let ementa = r.sumula.as_deref().unwrap_or("—");

        text.push_str(&format!(
            "- {ident} · {oficial} · {data} · {tipo}\n  {orgao}\n  {situacao}\n  {ementa}\n\n"
        ));
        html_rows.push_str(&format!(
            "<tr>
               <td>{}</td><td>{}</td><td>{}</td><td>{}</td>
               <td>{}</td><td>{}</td><td>{}</td>
             </tr>",
            escape_html(ident),
            escape_html(oficial),
            escape_html(data),
            escape_html(tipo),
            escape_html(orgao),
            escape_html(situacao),
            escape_html(ementa),
        ));
    }

    let mut note = String::new();
    let mut html_note = String::new();
    if n > matches.rows.len() as i64 {
        let rest = n - matches.rows.len() as i64;
        note = format!("\n… e mais {rest} atos. Refine a regra para receber menos itens por e-mail.\n");
        html_note = format!(
            "<p>… e mais {rest} atos. Refine a regra para receber menos itens por e-mail.</p>"
        );
    }

    let html = format!(
        "<html><body>
           <p>{}</p>
           <table cellpadding=\"6\" cellspacing=\"0\" border=\"0\" style=\"border-collapse:collapse\">
             <thead><tr style=\"text-align:left;background:#f0f0f0\">
               <th>Identificação</th><th>Nº oficial</th><th>Publicação</th><th>Tipo</th>
               <th>Órgão</th><th>Situação</th><th>Resumo/Assunto do Ato</th>
             </tr></thead>
             <tbody>{html_rows}</tbody>
           </table>
           {html_note}
         </body></html>",
        escape_html(&format!("{}: {n} {plural} encontrado(s).", matches.rule.name)),
    );

    Composed {
        subject,
        text: format!("{text}{note}"),
        html,
    }
}

/// The name that stands in for a rule when a single act is sent on demand.
///
/// A manual send has no filter behind it, but [`compose`] names the message
/// after a rule, so this stand-in keeps a one-off mail reading exactly like an
/// automatic one.
pub const MANUAL_RULE_NAME: &str = "Envio manual";

/// Email one act, to explicit recipients, built the way an automatic
/// notification is.
///
/// Composed through [`compose`] with a single row and a stand-in rule, so the
/// subject, the text and the HTML table are the automated path's. It is a side
/// channel, not a notification run: the watermark is neither read nor moved,
/// and no history is written, so it cannot disturb the automatic sends.
pub fn send_act(
    store: &crate::store::Store,
    settings: &Settings,
    act_id: i64,
    recipients: &[String],
) -> Result<()> {
    let row = store
        .act_row(act_id)?
        .ok_or_else(|| anyhow::anyhow!("ato {act_id} não encontrado"))?;
    let matches = RuleMatches {
        rule: EmailRule {
            id: 0,
            name: MANUAL_RULE_NAME.to_string(),
            enabled: true,
            text: None,
            filters: Vec::new(),
            recipients: recipients.to_vec(),
        },
        total: 1,
        rows: vec![row],
    };
    deliver(settings, recipients, &compose(&matches))
}

// ---------------------------------------------------------------- SMTP

/// Build the transport described by `settings`, with credentials when a username
/// is set. A dead server must not tie up the blocking worker, so the command
/// timeout is always bounded.
fn transport(settings: &Settings) -> Result<lettre::SmtpTransport> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::SmtpTransport;
    use std::time::Duration;

    let ep = settings.endpoint();
    if ep.host.is_empty() {
        anyhow::bail!("servidor SMTP não configurado");
    }

    let builder = match ep.security {
        Security::Starttls => SmtpTransport::starttls_relay(&ep.host)?,
        Security::Tls => SmtpTransport::relay(&ep.host)?,
        Security::None => SmtpTransport::builder_dangerous(&ep.host),
    };
    let mut builder = builder.port(ep.port).timeout(Some(Duration::from_secs(30)));

    if let Some(pw) = settings.password.as_deref() {
        builder = builder.credentials(Credentials::new(settings.username.clone(), pw.to_string()));
    }
    Ok(builder.build())
}

/// A message from `settings` plus recipients and a composed body.
fn build_message(
    settings: &Settings,
    recipients: &[String],
    composed: &Composed,
) -> Result<lettre::Message> {
    use lettre::message::{MultiPart, SinglePart};
    use lettre::Message;

    if settings.from_email.trim().is_empty() {
        anyhow::bail!("informe o e-mail do remetente");
    }
    let from = if settings.from_name.trim().is_empty() {
        settings.from_email.parse()?
    } else {
        format!("{} <{}>", settings.from_name.trim(), settings.from_email.trim()).parse()?
    };

    // The body is built first: the builder's body type is inferred from the
    // `.multipart(...)` call, so a half-built message with only `.from().to()`
    // has nothing to infer from.
    let body = MultiPart::alternative()
        .singlepart(SinglePart::plain(composed.text.clone()))
        .singlepart(SinglePart::html(composed.html.clone()));

    let mut builder = Message::builder().from(from).subject(composed.subject.clone());
    for r in recipients {
        builder = builder.to(r.parse().with_context(|| format!("destinatário inválido: {r}"))?);
    }

    Ok(builder.multipart(body)?)
}

fn deliver(settings: &Settings, recipients: &[String], composed: &Composed) -> Result<()> {
    use lettre::Transport;

    let message = build_message(settings, recipients, composed)?;
    transport(settings)?
        .send(&message)
        .map_err(|e| anyhow::anyhow!("falha ao enviar: {e}"))?;
    Ok(())
}

/// Connect and authenticate (and, if `to` is set, send a one-line probe).
pub fn test_connection(settings: &Settings, to: Option<&str>) -> Result<()> {
    // `test_connection` is inherent on `SmtpTransport`, so the `Transport` trait
    // (needed by `deliver` for `send`) is not required in this scope.
    let t = transport(settings)?;
    t.test_connection()
        .map_err(|e| anyhow::anyhow!("não foi possível conectar/autenticar: {e}"))?;

    if let Some(to) = to {
        let composed = Composed {
            subject: "[Legisladoc] E-mail de teste".into(),
            text: "Se você recebeu esta mensagem, o envio de e-mails está configurado.".into(),
            html: "<p>Se você recebeu esta mensagem, o envio de e-mails está configurado.</p>".into(),
        };
        deliver(settings, &[to.to_string()], &composed)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- orchestration

/// Resets the `emailing` flag on drop, so a panic can't wedge notifications.
struct EmailGuard<'a> {
    app: &'a AppHandle,
}

impl Drop for EmailGuard<'_> {
    fn drop(&mut self) {
        self.app
            .state::<AppState>()
            .emailing
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Match one rule against the acervo, ready to render.
///
/// `floor` bounds it to acts above the watermark (the notification path);
/// `None` matches the whole table, which is what the simulation panel wants.
/// `offset`/`limit` window the rows, so a rule that matches thousands of acts
/// can be paged instead of being truncated to the first email's worth.
pub fn query_rule(
    store: &crate::store::Store,
    rule: &EmailRule,
    floor: Option<i64>,
    offset: i64,
    limit: i64,
) -> Result<RuleMatches> {
    use crate::store::ActQuery;
    let res = store.search(&ActQuery {
        text: rule.text.clone(),
        filters: rule.filters.clone(),
        min_id: floor,
        limit,
        offset,
        ..Default::default()
    })?;
    Ok(RuleMatches {
        rule: rule.clone(),
        total: res.total,
        rows: res.rows,
    })
}

/// A rule that could not be evaluated, carrying no rows. `total = -1` marks it,
/// the same sentinel `run` records for a filter that no longer fits the data.
pub fn failed_rule(rule: EmailRule) -> RuleMatches {
    RuleMatches {
        rule,
        total: -1,
        rows: Vec::new(),
    }
}

/// Run the enabled rules and email the matches.
///
/// `floor` is the exclusive id floor. `send` false is a dry run that only logs.
/// Never advances the watermark — the caller decides that — so the manual path
/// cannot swallow a pending automatic notification.
///
/// Returns this run's entries, and the matched rules themselves, so a caller
/// that is not sending (the in-app alert) can render the acts without running
/// the same queries twice.
///
/// The store lock is held only to read settings/rules and match, and again to
/// write the log; SMTP runs in between with no lock held.
pub fn run(app: &AppHandle, floor: i64, send: bool) -> Result<(Vec<EmailLogEntry>, Vec<RuleMatches>)> {
    // --- phase 1: read + match, holding the store lock only briefly ---
    let (settings, matches, failed_rules) = {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let conn = store.conn();

        // Only a real send needs SMTP. Loading the settings decrypts the stored
        // password, and an alert must not fail because that is misconfigured —
        // it sends nothing.
        let settings = if send {
            Some(load_settings(conn, app)?)
        } else {
            None
        };
        let rules = list_rules(conn)?;
        let mut matches: Vec<RuleMatches> = Vec::new();
        let mut failed: Vec<RuleMatches> = Vec::new();

        for rule in rules.into_iter().filter(|r| r.enabled) {
            match query_rule(&store, &rule, Some(floor), 0, MAX_ITEMS_PER_EMAIL) {
                Ok(m) => matches.push(m),
                // A rule whose column vanished from the feed must not abort the
                // whole run; it is logged and skipped.
                Err(e) => {
                    log::warn!("regra {} não pôde ser avaliada: {e:#}", rule.name);
                    failed.push(failed_rule(rule));
                }
            }
        }
        (settings, matches, failed)
    };

    // --- phase 2: compose and send, no lock held ---
    let mut outcome: Vec<LogRow> = Vec::new();

    for f in &failed_rules {
        outcome.push(LogRow {
            at: chrono::Utc::now().to_rfc3339(),
            rule_id: Some(f.rule.id),
            rule_name: f.rule.name.clone(),
            recipients: f.rule.recipients.join("; "),
            matched: 0,
            status: "skipped".into(),
            error: Some("filtro inválido para os dados atuais".into()),
            act_ids: None,
        });
    }

    for m in &matches {
        if m.total == 0 {
            continue; // nothing to report; not worth a log line
        }
        let ids: Vec<String> = m.rows.iter().map(|r| r.id.to_string()).collect();
        let composed = compose(m);

        let row = if !send {
            LogRow {
                at: chrono::Utc::now().to_rfc3339(),
                rule_id: Some(m.rule.id),
                rule_name: m.rule.name.clone(),
                recipients: m.rule.recipients.join("; "),
                matched: m.total,
                status: "skipped".into(),
                error: None, // dry run
                act_ids: Some(ids.join(",")),
            }
        } else {
            // `settings` is `Some` whenever `send` is set (phase 1).
            let settings = settings.as_ref().expect("settings loaded for a send");
            match deliver(settings, &m.rule.recipients, &composed) {
                Ok(()) => LogRow {
                    at: chrono::Utc::now().to_rfc3339(),
                    rule_id: Some(m.rule.id),
                    rule_name: m.rule.name.clone(),
                    recipients: m.rule.recipients.join("; "),
                    matched: m.total,
                    status: "sent".into(),
                    error: None,
                    act_ids: Some(ids.join(",")),
                },
                Err(e) => {
                    log::warn!("envio da regra {} falhou: {e:#}", m.rule.name);
                    LogRow {
                        at: chrono::Utc::now().to_rfc3339(),
                        rule_id: Some(m.rule.id),
                        rule_name: m.rule.name.clone(),
                        recipients: m.rule.recipients.join("; "),
                        matched: m.total,
                        status: "failed".into(),
                        error: Some(format!("{e:#}")),
                        act_ids: Some(ids.join(",")),
                    }
                }
            }
        };
        outcome.push(row);
    }

    // --- phase 3: write the log, holding the lock briefly again ---
    //
    // A dry run is a preview, not a notification: persisting it would both invent
    // history and, because the caller sums the rows it gets back, let an earlier
    // preview leak into a later run's total. Only a real send is recorded.
    if send && !outcome.is_empty() {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let conn = store.conn();
        for row in &outcome {
            log_entry(conn, row)?;
        }
    }

    // Return exactly this run's outcome. `list_log` would return the whole
    // history, so a caller summing `matched` would count previous runs too.
    Ok((
        outcome
            .into_iter()
            .map(|row| EmailLogEntry {
                at: row.at,
                rule_id: row.rule_id,
                rule_name: row.rule_name,
                recipients: row.recipients,
                matched: row.matched,
                status: row.status,
                error: row.error,
            })
            .collect(),
        matches,
    ))
}

/// What surfaced the new acts, which decides how they are reported.
///
/// A manual sync must not email: the user is right there, so an in-app alert is
/// the whole notification. An automatic one runs unattended and has to reach
/// the user by mail. Both spend the same watermark, so a batch is never
/// surfaced twice — by alert and then again by mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Automatic,
}

impl Trigger {
    /// Whether this trigger reaches the user by email.
    pub fn emails(self) -> bool {
        matches!(self, Trigger::Automatic)
    }
}

/// Report the acts that arrived with a sync, over the channel its trigger chose.
///
/// `report.action` is used only for logging: it cannot gate the work, because
/// the feed publishes a *new dated file* each day rather than appending to one,
/// so every daily ingest reports `"reloaded"`. Whether anything is new is
/// decided by comparing the act maximum against the watermark instead.
pub fn notify_after_sync(
    app: &AppHandle,
    report: &crate::sync::SyncReport,
    trigger: Trigger,
) -> Result<()> {
    use tauri::Emitter;

    let state = app.state::<AppState>();
    if !try_begin_notification(&state.emailing) {
        // Another run holds the slot. Nothing is lost: the watermark has not
        // moved, so a later sync still finds these acts below `max` and
        // reports them then.
        log::info!("email: execução já em andamento; adiada");
        return Ok(());
    }
    let _guard = EmailGuard { app };

    let (floor, max_id) = {
        let store = state.store.lock().unwrap();
        let conn = store.conn();
        let count = act_count_or_zero(conn)?;
        let max = store.max_act_id()?;
        // `effective_floor` seeds the watermark on a first run against an
        // already-populated database and returns the seed, so a first load
        // reports nothing below.
        (effective_floor(conn, count, max)?, max.unwrap_or(0))
    };

    let Some(floor) = notification_floor(floor, Some(max_id)) else {
        log::info!("email: apos sincronizacao ({}) nada novo", report.action);
        // A manual sync that turns up nothing must also retract the previous
        // alert, or the window keeps showing acts from an earlier run.
        if !trigger.emails() {
            *state.last_alert.lock().unwrap() = None;
        }
        return Ok(());
    };

    log::info!(
        "email: apos sincronizacao ({}, {}), piso {floor} -> max {max_id}",
        report.action,
        if trigger.emails() { "automatico" } else { "manual" }
    );

    let (entries, matches) = run(app, floor, trigger.emails())?;

    // A failure must not move the floor: the next run has to retry rather than
    // silently drop the acts.
    if entries.iter().any(|e| e.status == "failed") {
        log::warn!("email: piso mantido em {floor}; houve falha no envio");
        return Ok(());
    }

    if !trigger.emails() {
        let alert = build_alert(&matches, &report.snapshot, max_id);
        if alert.total_matched > 0 {
            *state.last_alert.lock().unwrap() = Some(alert.clone());
            let _ = app.emit(crate::SYNC_MATCHES_EVENT, &alert);
        }
    }

    let store = state.store.lock().unwrap();
    set_watermark(store.conn(), max_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        [7u8; 32]
    }

    #[test]
    fn password_round_trips() {
        let blob = encrypt(&key(), "abcd efgh ijkl mnop").unwrap();
        assert_eq!(decrypt(&key(), &blob).unwrap(), "abcd efgh ijkl mnop");
    }

    #[test]
    fn blob_has_version_nonce_and_tag() {
        let plaintext = "secret";
        let blob = encrypt(&key(), plaintext).unwrap();
        // 1 version + 12 nonce + ciphertext + 16 tag
        assert_eq!(blob.len(), 1 + 12 + plaintext.len() + 16);
        assert_eq!(blob[0], ENC_VERSION);
    }

    #[test]
    fn tampering_is_detected() {
        let mut blob = encrypt(&key(), "secret").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(decrypt(&key(), &blob).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let blob = encrypt(&key(), "secret").unwrap();
        let mut other = key();
        other[0] ^= 0xff;
        assert!(decrypt(&other, &blob).is_err());
    }

    #[test]
    fn the_same_plaintext_encrypts_differently_each_time() {
        // Fresh nonce per encryption: identical inputs must not produce identical
        // blobs, or the ciphertext would leak equality.
        let a = encrypt(&key(), "same").unwrap();
        let b = encrypt(&key(), "same").unwrap();
        assert_ne!(a, b);
        assert_eq!(decrypt(&key(), &a).unwrap(), "same");
        assert_eq!(decrypt(&key(), &b).unwrap(), "same");
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let mut blob = encrypt(&key(), "secret").unwrap();
        blob[0] = 0x09;
        assert!(decrypt(&key(), &blob).is_err());
    }

    #[test]
    fn recipients_are_split_on_separators_and_spaces() {
        let got = parse_recipients("a@x.com; b@y.com, c@z.com\nd@w.com");
        assert_eq!(got, vec!["a@x.com", "b@y.com", "c@z.com", "d@w.com"]);
        assert!(parse_recipients("  ,  ").is_empty());
    }

    #[test]
    fn subject_pluralizes() {
        let rule = EmailRule {
            id: 1,
            name: "Decretos".into(),
            enabled: true,
            text: None,
            filters: vec![],
            recipients: vec!["a@b.com".into()],
        };
        let one = compose(&RuleMatches {
            rule: rule.clone(),
            total: 1,
            rows: vec![],
        });
        assert!(one.subject.contains("1 ato novo"), "{}", one.subject);
        let many = compose(&RuleMatches {
            rule,
            total: 3,
            rows: vec![],
        });
        assert!(many.subject.contains("3 atos novos"), "{}", many.subject);
    }

    #[test]
    fn html_values_are_escaped() {
        let rule = EmailRule {
            id: 1,
            name: "X".into(),
            enabled: true,
            text: None,
            filters: vec![],
            recipients: vec![],
        };
        let m = RuleMatches {
            rule,
            total: 1,
            rows: vec![crate::store::ActRow {
                id: 1,
                numero_identificacao: Some("<script>alert(1)</script>".into()),
                numero_oficial: None,
                ano_publicacao: None,
                data_publicacao: None,
                data_criacao: None,
                tipo: None,
                orgao: None,
                situacao: None,
                sumula: Some("a & b".into()),
                snippet: None,
            }],
        };
        let c = compose(&m);
        assert!(!c.html.contains("<script>"), "{}", c.html);
        assert!(c.html.contains("&lt;script&gt;"));
        assert!(c.html.contains("a &amp; b"));
    }

    /// A store with the email tables, a `TIPO_ATO` dimension, and one rule.
    fn store_with_rule(filters: Vec<crate::store::Filter>) -> crate::store::Store {
        let mut store = crate::store::Store::open_in_memory().unwrap();
        store
            .ensure_act_tables(&crate::store::test_schema())
            .unwrap();
        store
            .replace_dimension(
                "TIPO_ATO",
                &["ID".into(), "NOME".into()],
                &[
                    vec![Some("50".into()), Some("ACORDÃO".into())],
                    vec![Some("195".into()), Some("AVISO DE LICITAÇÃO".into())],
                ],
            )
            .unwrap();
        let conn = store.conn();
        save_rule(
            conn,
            &EmailRuleInput {
                id: None,
                name: "Regra".into(),
                enabled: true,
                text: None,
                filters,
                recipients: "a@b.com".into(),
            },
        )
        .unwrap();
        store
    }

    #[test]
    fn nothing_is_new_when_the_max_equals_the_watermark() {
        // The boundary the daily feed lands on: an unchanged acervo must not
        // report, and neither may a first run that just seeded the watermark.
        assert_eq!(notification_floor(480197, Some(480197)), None);
        assert_eq!(notification_floor(480197, None), None);
        assert_eq!(notification_floor(480090, Some(480197)), Some(480090));
    }

    #[test]
    fn a_manual_sync_never_emails() {
        assert!(!Trigger::Manual.emails());
        assert!(Trigger::Automatic.emails());
    }

    #[test]
    fn the_watermark_only_moves_forward() {
        // A stale write from a detached notification must not rewind the floor:
        // that would re-notify acts already surfaced.
        assert_eq!(advance_watermark(480197, 480090), 480197);
        assert_eq!(advance_watermark(480090, 480197), 480197);
        assert_eq!(advance_watermark(1, 1), 1);
    }

    #[test]
    fn the_notification_slot_admits_one_run_at_a_time() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        assert!(try_begin_notification(&flag));
        assert!(!try_begin_notification(&flag));
        flag.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(try_begin_notification(&flag));
    }

    fn rule_named(name: &str, id: i64) -> EmailRule {
        EmailRule {
            id,
            name: name.into(),
            enabled: true,
            text: None,
            filters: vec![],
            recipients: vec!["a@b.com".into()],
        }
    }

    fn act_row(id: i64, ident: &str) -> crate::store::ActRow {
        crate::store::ActRow {
            id,
            numero_identificacao: Some(ident.into()),
            numero_oficial: None,
            ano_publicacao: None,
            data_publicacao: Some("2026-10-08 00:00:00.000".into()),
            data_criacao: None,
            tipo: Some("DECRETO".into()),
            orgao: None,
            situacao: None,
            sumula: None,
            snippet: None,
        }
    }

    #[test]
    fn the_alert_samples_the_acts_and_keeps_the_full_count() {
        let rows: Vec<_> = (1..=8).map(|i| act_row(i, &format!("DOC{i}"))).collect();
        let matches = vec![
            RuleMatches {
                rule: rule_named("Decretos", 1),
                total: 40, // more than were fetched, so the count must stand alone
                rows,
            },
            // A rule with nothing to report is left out entirely, so the alert
            // does not list empty rules.
            RuleMatches {
                rule: rule_named("Vazio", 2),
                total: 0,
                rows: vec![],
            },
        ];

        let alert = build_alert(&matches, "2026-10-09_x.csv", 480197);
        assert_eq!(alert.total_matched, 40);
        assert_eq!(alert.snapshot, "2026-10-09_x.csv");
        assert_eq!(alert.max_id, 480197);
        assert_eq!(alert.rules.len(), 1);

        let rule = &alert.rules[0];
        assert_eq!(rule.rule_name, "Decretos");
        assert_eq!(rule.matched, 40);
        assert_eq!(rule.samples.len(), MAX_ALERT_SAMPLES);
        assert_eq!(rule.samples[0].id, 1);
        assert_eq!(rule.samples[0].numero_identificacao.as_deref(), Some("DOC1"));
        assert_eq!(rule.samples[0].tipo.as_deref(), Some("DECRETO"));
    }

    #[test]
    fn related_filters_are_mapped_from_key_to_name_and_are_idempotent() {
        let fk = |value: &str, values: Vec<&str>| crate::store::Filter {
            column: "TIPO_ATO_ID".into(),
            op: crate::store::Op::Eq,
            value: value.into(),
            values: values.into_iter().map(String::from).collect(),
            logic: crate::store::Logic::And,
        };
        let store = store_with_rule(vec![fk("50", vec!["195"])]);

        let schema = crate::store::test_schema();
        let changed = normalize_related_filters(store.conn(), &schema).unwrap();
        assert_eq!(changed, 1);

        let rule = &list_rules(store.conn()).unwrap()[0];
        assert_eq!(rule.filters[0].value, "ACORDÃO");
        assert_eq!(rule.filters[0].values, vec!["AVISO DE LICITAÇÃO".to_string()]);

        // A second pass over names must not report or write anything.
        assert_eq!(normalize_related_filters(store.conn(), &schema).unwrap(), 0);
    }

    #[test]
    fn a_filter_on_an_unknown_column_is_left_alone() {
        let store = store_with_rule(vec![crate::store::Filter {
            column: "SUMULA".into(),
            op: crate::store::Op::Eq,
            value: "50".into(),
            values: vec![],
            logic: crate::store::Logic::And,
        }]);
        let changed = normalize_related_filters(store.conn(), &crate::store::test_schema()).unwrap();
        assert_eq!(changed, 0);
        assert_eq!(list_rules(store.conn()).unwrap()[0].filters[0].value, "50");
    }
}
