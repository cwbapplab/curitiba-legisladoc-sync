//! Re-listing the remote rsync feed.
//!
//! `default_url()` returns the URL that actually works against
//! `dadosabertos.c3sl.ufpr.br`. Note the shape: `dadosabertos` is the *module*
//! and `curitiba/Legisladoc/` is a path inside it. The obvious-looking
//! `rsync://host/curitiba/Legisladoc/` is rejected by the server with
//! `@ERROR: Unknown module 'curitiba'`, because the daemon takes the first path
//! element as the module name.

use arrsync::RsyncClient;
use serde::{Deserialize, Serialize};

/// The module + path this app browses. Overridable at runtime.
pub fn default_url() -> String {
    "rsync://dadosabertos.c3sl.ufpr.br/dadosabertos/curitiba/Legisladoc/".to_string()
}

/// One file as the remote server reported it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub path: String,
    pub size: i64,
    /// RFC 3339, or `None` when the server sent no timestamp.
    pub mtime: Option<String>,
}

/// Everything one `connect` call yields: the full listing plus transfer stats.
#[derive(Debug, Clone)]
pub struct Listing {
    pub files: Vec<RemoteFile>,
    /// `Stats::file_size` — total bytes of every file in the module listing.
    pub total_bytes: i64,
}

/// Connect and read the file list, then close the connection.
///
/// Only metadata is ever transferred: `connect` performs the listing and we
/// `close` immediately without calling `get`, so nothing is downloaded. That
/// matters here because the listed payload is on the order of 2.4 TB.
pub async fn fetch_listing(url: &str) -> anyhow::Result<Listing> {
    let url = url::Url::parse(url).map_err(|e| anyhow::anyhow!("invalid rsync url: {e}"))?;
    let (mut client, raw_files) = RsyncClient::connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("{e:#}"))?;

    let files = raw_files
        .into_iter()
        // Directories and symlinks are dropped: this app browses documents, and
        // the top-level "." entry would otherwise show up as a file.
        .filter(|f| f.is_file())
        .map(|f| RemoteFile {
            path: String::from_utf8_lossy(&f.path).into_owned(),
            size: f.size.min(i64::MAX as u64) as i64,
            mtime: f.mtime.map(|t| t.to_rfc3339()),
        })
        .collect();

    // `close` is what tells the server we are done; without it the connection
    // would linger. Errors here are not fatal for us — we already have the list.
    let total_bytes = client.close().await.map(|s| s.file_size).unwrap_or(0);

    Ok(Listing {
        files,
        total_bytes,
    })
}
