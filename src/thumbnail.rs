//! Downloads and caches track thumbnail images.
//!
//! Deliberately simple: a blocking HTTP GET (`ureq`) plus an on-disk cache
//! keyed by a hash of the URL, meant to be called from a background OS
//! thread the same way `search::search` already is (see
//! `ui/search_view.rs` and `ui/player_bar.rs`) — no async runtime
//! involvement, no new UI-facing types.

use anyhow::{Context, Result};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;

/// Returns the raw image bytes for `url` (whatever format the source
/// served — JPEG/WebP/etc.), reading from the on-disk cache if present and
/// populating it on a cache miss. Callers decode the bytes themselves
/// (`gdk::Texture::from_bytes` handles the common formats directly).
pub fn fetch(url: &str) -> Result<Vec<u8>> {
    let path = cache_path(url);

    if let Some(path) = &path {
        if let Ok(bytes) = std::fs::read(path) {
            return Ok(bytes);
        }
    }

    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(8))
        .call()
        .with_context(|| format!("requesting thumbnail {url}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading thumbnail body {url}"))?;

    if let Some(path) = &path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Best-effort: a failed write just means we re-download next time,
        // not worth surfacing as an error to the caller.
        let _ = std::fs::write(path, &bytes);
    }

    Ok(bytes)
}

/// Deterministic on-disk cache path for a thumbnail URL, under
/// `$XDG_CACHE_HOME/melofin/thumbnails` (falling back to `~/.cache`), or
/// `None` if we can't determine a cache directory at all (e.g. neither
/// env var is set).
fn cache_path(url: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    Some(
        base.join("melofin")
            .join("thumbnails")
            .join(format!("{:016x}.img", hasher.finish())),
    )
}
