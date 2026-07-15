use anyhow::{Context, Result};
use std::path::Path;

use crate::innertube::{browse_request, parse_song_item};
use crate::search::Track;
use crate::user::read_and_validate_cookies;

/// Metadata for a playlist, album, or artist page.
#[derive(Clone, Debug)]
pub struct DetailMetadata {
    pub title: String,
    pub artist: String,
    pub thumbnail_url: String,
    pub description: String,
    pub year: String,
    pub track_count: usize,
}

/// A fully-fetched detail page: metadata + track listing.
#[derive(Clone, Debug)]
pub struct DetailResult {
    pub metadata: DetailMetadata,
    pub tracks: Vec<Track>,
}

/// Fetches the details (metadata + track list) for a playlist, album, or
/// artist by its YouTube Music browse_id.
///
/// Blocking — call from a background thread.
pub fn fetch_detail(cookies_path: &Path, browse_id: &str) -> Result<DetailResult> {
    let cookie_header = read_and_validate_cookies(cookies_path)?;

    // Fetch the page to get the API key.
    let html = ureq::get("https://music.youtube.com")
        .set("Cookie", &cookie_header)
        .set("User-Agent", crate::innertube::USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .context("failed to fetch music.youtube.com")?
        .into_string()
        .context("couldn't read YT Music HTML")?;

    let api_key = crate::user::extract_innertube_api_key(&html)
        .context("couldn't find INNERTUBE_API_KEY in page HTML")?;

    // Fetch the browse page for this playlist/album/artist.
    let json = browse_request(&cookie_header, &api_key, Some(browse_id), None)
        .context("browse request failed")?;

    let metadata = parse_metadata(&json);
    let mut tracks = parse_tracks(&json);

    tracing::debug!(track_count = tracks.len(), "initial browse response parsed");

    // Follow continuation tokens for large playlists.
    let mut current_token = extract_continuation(&json);
    let max_pages = 10;
    let mut page = 0;
    while let Some(token) = current_token {
        page += 1;
        if page > max_pages {
            break;
        }
        let cont = match browse_request(&cookie_header, &api_key, None, Some(&token)) {
            Ok(json) => json,
            Err(_) => break,
        };
        let page_tracks = parse_continuation_tracks(&cont);
        let count = page_tracks.len();
        tracks.extend(page_tracks);
        current_token = extract_continuation_from_shelf(&cont);
        if count == 0 {
            break;
        }
    }

    Ok(DetailResult {
        metadata: DetailMetadata {
            track_count: tracks.len(),
            ..metadata
        },
        tracks,
    })
}

/// Parses header metadata from the browse response. Tries
/// `musicResponsiveHeaderRenderer` first (used by playlists/albums),
/// then `musicImmersiveHeaderRenderer` (used by artists).
fn parse_metadata(json: &serde_json::Value) -> DetailMetadata {
    // Try responsive header first.
    if let Some(header) = json
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicResponsiveHeaderRenderer")
    {
        return parse_responsive_header(header);
    }

    // Try immersive header (artists).
    if let Some(header) = json
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicImmersiveHeaderRenderer")
    {
        return parse_immersive_header(header);
    }

    // Fallback: minimal metadata.
    DetailMetadata {
        title: String::new(),
        artist: String::new(),
        thumbnail_url: String::new(),
        description: String::new(),
        year: String::new(),
        track_count: 0,
    }
}

fn parse_responsive_header(header: &serde_json::Value) -> DetailMetadata {
    let title = header
        .pointer("/title/runs")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    // Try straplineTextOne first (used by MusicResponsiveHeaderRenderer for albums).
    let artist = header
        .pointer("/straplineTextOne/runs")
        .and_then(|r| r.as_array())
        .and_then(|runs| {
            runs.first()
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())
        })
        .or_else(|| {
            // Fallback: subtitle runs (skip type labels and separators).
            header.pointer("/subtitle/runs")
                .and_then(|r| r.as_array())
                .and_then(|runs| {
                    runs.iter()
                        .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                        .find(|t| {
                            let lower = t.to_lowercase();
                            !lower.starts_with("playlist")
                                && !lower.starts_with("album")
                                && !lower.starts_with("artist")
                                && !lower.starts_with("single")
                                && !lower.starts_with("ep")
                                && !lower.starts_with("podcast")
                                && !lower.starts_with("compilation")
                                && *t != " · "
                                && *t != " • "
                                && *t != ","
                                && *t != "·"
                                && *t != "•"
                                && !t.chars().all(|c| c.is_ascii_digit())
                                && t.len() > 2
                        })
                })
        })
        .unwrap_or("")
        .to_string();

    let thumbnail_url = header
        .pointer("/image/musicThumbnailRenderer/thumbnail/thumbnails")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    let description = parse_description(header);

    let year = header
        .pointer("/subtitle/runs")
        .and_then(|r| r.as_array())
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                .find(|t| t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default();

    DetailMetadata {
        title,
        artist,
        thumbnail_url,
        description,
        year,
        track_count: 0,
    }
}

fn parse_immersive_header(header: &serde_json::Value) -> DetailMetadata {
    let title = header
        .pointer("/title/runs")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let description = parse_description(header);

    DetailMetadata {
        title,
        artist: String::new(),
        thumbnail_url: String::new(),
        description,
        year: String::new(),
        track_count: 0,
    }
}

fn parse_description(header: &serde_json::Value) -> String {
    header
        .pointer("/description/bodyBodyRenderer/runs")
        .or_else(|| header.pointer("/description/runs"))
        .and_then(|r| r.as_array())
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parses tracks from the initial browse response. Looks for tracks in
/// `musicShelfRenderer` or `musicPlaylistShelfRenderer` contents.
fn parse_tracks(json: &serde_json::Value) -> Vec<Track> {
    tracing::debug!("parse_tracks: looking for tracks");
    // Primary: musicShelfRenderer in sectionListRenderer.
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer") {
        tracing::debug!("found tracks in tabs/sectionListRenderer");
        return parse_shelf_tracks(shelf);
    }

    // Fallback: singleColumn layout.
    if let Some(shelf) = json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer") {
        tracing::debug!("found tracks in singleColumn");
        return parse_shelf_tracks(shelf);
    }

    // Fallback: musicPlaylistShelfRenderer (used by some playlists).
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer") {
        tracing::debug!("found tracks in musicPlaylistShelfRenderer");
        return parse_shelf_tracks(shelf);
    }

    // Fallback: secondaryContents (used by albums — tracks are here, not in main contents).
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents/0/musicShelfRenderer") {
        tracing::debug!("found tracks in secondaryContents");
        return parse_shelf_tracks(shelf);
    }

    // Fallback: scan all sectionListRenderer contents for any shelf.
    if let Some(contents) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        && let Some(arr) = contents.as_array()
    {
        for item in arr {
            if let Some(shelf) = item.get("musicShelfRenderer") {
                let tracks = parse_shelf_tracks(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
            if let Some(shelf) = item.get("musicPlaylistShelfRenderer") {
                let tracks = parse_shelf_tracks(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
        }
    }

    // Fallback: scan secondaryContents for any shelf.
    if let Some(contents) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents")
        && let Some(arr) = contents.as_array()
    {
        for item in arr {
            if let Some(shelf) = item.get("musicShelfRenderer") {
                let tracks = parse_shelf_tracks(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
        }
    }

    Vec::new()
}

fn parse_shelf_tracks(shelf: &serde_json::Value) -> Vec<Track> {
    let contents = shelf.get("contents").and_then(|c| c.as_array());
    tracing::debug!(count = contents.map(|a| a.len()), "parse_shelf_tracks");
    match contents {
        Some(arr) => {
            let tracks: Vec<Track> = arr.iter()
                .filter_map(|item| {
                    let renderer = item.get("musicResponsiveListItemRenderer");
                    if renderer.is_none() {
                        tracing::warn!(
                            keys = ?item.as_object().map_or(vec![], |m| m.keys().cloned().collect::<Vec<_>>()),
                            "shelf item has no musicResponsiveListItemRenderer"
                        );
                    }
                    renderer.and_then(parse_song_item)
                })
                .collect();
            tracing::debug!(parsed = tracks.len(), "parse_shelf_tracks done");
            tracks
        }
        None => {
            tracing::warn!("parse_shelf_tracks: no contents array");
            Vec::new()
        }
    }
}

fn parse_continuation_tracks(json: &serde_json::Value) -> Vec<Track> {
    if let Some(shelf) = json.pointer("/continuationContents/musicShelfContinuation") {
        return parse_shelf_tracks(shelf);
    }
    if let Some(shelf) = json.pointer("/continuationContents/sectionListContinuation/contents/0/musicShelfRenderer") {
        return parse_shelf_tracks(shelf);
    }
    if let Some(shelf) = json.pointer("/continuationContents/musicPlaylistShelfContinuation") {
        return parse_shelf_tracks(shelf);
    }
    // Scan all items in sectionListContinuation for any shelf.
    if let Some(contents) = json.pointer("/continuationContents/sectionListContinuation/contents")
        && let Some(arr) = contents.as_array()
    {
        for item in arr {
            if let Some(shelf) = item.get("musicShelfRenderer") {
                let tracks = parse_shelf_tracks(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
            if let Some(shelf) = item.get("musicPlaylistShelfRenderer") {
                let tracks = parse_shelf_tracks(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
        }
    }
    Vec::new()
}

/// Extracts continuation token from the initial browse response.
fn extract_continuation(json: &serde_json::Value) -> Option<String> {
    // Try musicShelfRenderer continuation first.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Try sectionListRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Try musicHeaderShelfRenderer continuation (used by albums).
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicResponsiveHeaderRenderer/musicHeaderShelfRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Try musicPlaylistShelfRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Try tabRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Try secondaryContents continuation (used by some album layouts).
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Try secondaryContents musicShelfRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents/0/musicShelfRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    None
}

/// Extracts continuation token from a continuation response.
fn extract_continuation_from_shelf(json: &serde_json::Value) -> Option<String> {
    json.pointer("/continuationContents/musicShelfContinuation/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/continuationContents/sectionListContinuation/continuations/0/nextContinuationData/continuation"))
        .or_else(|| json.pointer("/continuationContents/musicPlaylistShelfContinuation/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}
