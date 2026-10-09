//! HTTP access to the feed's Apache mirror.
//!
//! The rsync protocol has no way to fetch a byte range — `arrsync` always pulls
//! whole files — so the delta path goes over HTTP instead. The mirror at the
//! same host serves the identical tree and advertises `Accept-Ranges: bytes`
//! with a stable `ETag` and `Last-Modified`, which is what makes an
//! append-only tail fetch possible.

use anyhow::{bail, Context, Result};
use std::io::Read;

/// What a `HEAD` tells us about the current snapshot.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct FileInfo {
    pub size: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub accept_ranges: bool,
}

/// The HTTP root of the mirror that mirrors the rsync module.
pub const HTTP_ROOT: &str = "http://dadosabertos.c3sl.ufpr.br/curitiba/Legisladoc/";

/// Build the HTTP URL for a file that the rsync listing named.
///
/// Paths from the listing are bare file names inside the module, so the
/// Legiladoc tree's HTTP path is just the root plus the name.
pub fn url_for(file_name: &str) -> String {
    format!("{HTTP_ROOT}{file_name}")
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        // The mirror is slow to first byte on large reads; this is a transfer
        // timeout, not a total-duration cap.
        .timeout_global(Some(std::time::Duration::from_secs(30 * 60)))
        .build()
        .new_agent()
}

/// `HEAD` the current snapshot to learn its size and validators.
pub fn head(url: &str) -> Result<FileInfo> {
    let resp = agent()
        .head(url)
        .call()
        .with_context(|| format!("HEAD {url}"))?;

    let headers = resp.headers();
    let get = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    Ok(FileInfo {
        size: get("content-length").and_then(|v| v.parse().ok()),
        etag: get("etag"),
        last_modified: get("last-modified"),
        accept_ranges: get("accept-ranges").as_deref() == Some("bytes"),
    })
}

/// Fetch bytes from `from` to the end of the file.
///
/// The mirror answers a range request with `206 Partial Content`. A `200` means
/// the server ignored the range and is about to send the whole file, which the
/// caller must not mistake for a small delta — so that is reported as an error.
pub fn get_tail(url: &str, from: u64) -> Result<impl Read> {
    let resp = agent()
        .get(url)
        .header("Range", &format!("bytes={from}-"))
        .call()
        .with_context(|| format!("GET range from {from} of {url}"))?;

    let status = resp.status().as_u16();
    match status {
        206 => {}
        200 => bail!(
            "server ignored the byte range and returned the whole file; \
             a tail fetch would download {} bytes",
            resp.headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("?")
        ),
        other => bail!("unexpected HTTP {other} for range request"),
    }
    Ok(resp.into_body().into_reader())
}

/// Stream the entire file.
pub fn get_all(url: &str) -> Result<impl Read> {
    let resp = agent()
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("HTTP {} fetching {url}", resp.status().as_u16());
    }
    Ok(resp.into_body().into_reader())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls_under_the_mirror_root() {
        assert_eq!(
            url_for("2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv"),
            "http://dadosabertos.c3sl.ufpr.br/curitiba/Legisladoc/2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv"
        );
    }

    /// Hits the live mirror. Ignored by default so the suite stays offline-safe;
    /// run with `cargo test -- --ignored` when a network is available.
    #[test]
    #[ignore]
    fn live_mirror_advertises_ranges_and_answers_a_tail_request() {
        let url = url_for("2026-10-08_Legisladoc_-_ATO_-_Base_de_Dados.csv");
        let info = head(&url).unwrap();
        assert!(info.size.unwrap() > 1_000_000_000);
        assert!(info.etag.is_some(), "ETag is needed to detect rewrites");
        assert!(info.accept_ranges, "range support is what makes delta sync work");

        // A tail request must return 206 and only the requested suffix.
        let mut r = get_tail(&url, info.size.unwrap() - 300).unwrap();
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).unwrap();
        assert_eq!(buf.len(), 300);
    }
}
