//! Fetches the user's liked songs from YouTube Music via the InnerTube API.
//!
//! Uses `browseId: "VLLM"` to get the liked songs playlist, then
//! follows continuations to load all tracks. Returns a flat `Vec<Track>`
//! that the UI can paginate through.

use anyhow::{Context, Result};
use std::path::Path;

use crate::innertube::{browse_request, parse_song_item};
use crate::search::Track;
use crate::user::{extract_innertube_api_key, read_and_validate_cookies};

/// Fetches all liked songs from the user's YouTube Music account.
///
/// Blocking — call from a background thread.
pub fn fetch_liked_songs(cookies_path: &Path) -> Result<Vec<Track>> {
    let cookie_header = read_and_validate_cookies(cookies_path)?;

    let html = ureq::get("https://music.youtube.com")
        .set("Cookie", &cookie_header)
        .set("User-Agent", crate::innertube::USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .context("failed to fetch music.youtube.com")?
        .into_string()
        .context("couldn't read YT Music HTML")?;

    let api_key = extract_innertube_api_key(&html).context("couldn't find INNERTUBE_API_KEY")?;

    // Initial browse for the liked songs playlist (VLLM = VL + LM where LM is the liked music playlist ID).
    let initial = browse_request(&cookie_header, &api_key, Some("VLLM"), None)
        .context("liked songs browse request failed")?;

    let mut tracks = parse_music_shelf(&initial);

    // Follow continuations.
    let mut current_token = extract_continuation(&initial);
    let max_pages = 50;
    let mut page = 0;

    while let Some(token) = current_token {
        page += 1;
        if page > max_pages {
            break;
        }

        let response = browse_request(&cookie_header, &api_key, None, Some(&token))
            .context("liked songs continuation failed")?;

        let page_tracks = parse_music_shelf_continuation(&response);
        let count = page_tracks.len();
        tracks.extend(page_tracks);

        current_token = extract_continuation(&response);

        if count == 0 {
            break;
        }
    }

    tracing::debug!(total = tracks.len(), "fetched liked songs");
    Ok(tracks)
}

/// Parses tracks from the initial browse response. Liked songs use
/// `musicShelfRenderer` or `musicPlaylistShelfRenderer` with
/// `contents[].musicResponsiveListItemRenderer`. Tries both
/// `twoColumnBrowseResultsRenderer` and `singleColumnBrowseResultsRenderer`
/// layouts.
fn parse_music_shelf(json: &serde_json::Value) -> Vec<Track> {
    // Primary: twoColumn layout with musicShelfRenderer.
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer") {
        return parse_shelf_items(shelf);
    }
    // SingleColumn layout with musicShelfRenderer.
    if let Some(shelf) = json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer") {
        return parse_shelf_items(shelf);
    }
    // musicPlaylistShelfRenderer (used by some playlists).
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer") {
        return parse_shelf_items(shelf);
    }
    if let Some(shelf) = json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer") {
        return parse_shelf_items(shelf);
    }
    // secondaryContents (used by some album/playlist layouts).
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents/0/musicShelfRenderer") {
        return parse_shelf_items(shelf);
    }
    // secondaryContents musicPlaylistShelfRenderer (used by liked songs and playlists).
    if let Some(shelf) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents/0/musicPlaylistShelfRenderer") {
        return parse_shelf_items(shelf);
    }

    // Scan all sectionListRenderer contents for any shelf type.
    let contents = json
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .or_else(|| json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents"))
        .and_then(|c| c.as_array());

    if let Some(arr) = contents {
        for item in arr {
            if let Some(shelf) = item.get("musicShelfRenderer") {
                let tracks = parse_shelf_items(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
            if let Some(shelf) = item.get("musicPlaylistShelfRenderer") {
                let tracks = parse_shelf_items(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
        }
    }

    Vec::new()
}

/// Parses tracks from a continuation response.
fn parse_music_shelf_continuation(json: &serde_json::Value) -> Vec<Track> {
    // musicShelfContinuation (most common).
    if let Some(shelf) = json.pointer("/continuationContents/musicShelfContinuation") {
        return parse_shelf_items(shelf);
    }
    // musicPlaylistShelfContinuation (used by playlist continuations).
    if let Some(shelf) = json.pointer("/continuationContents/musicPlaylistShelfContinuation") {
        return parse_shelf_items(shelf);
    }
    // sectionListContinuation with musicShelfRenderer.
    if let Some(shelf) = json.pointer("/continuationContents/sectionListContinuation/contents/0/musicShelfRenderer") {
        return parse_shelf_items(shelf);
    }

    // Scan sectionListContinuation contents for any shelf type.
    let contents = json
        .pointer("/continuationContents/sectionListContinuation/contents")
        .and_then(|c| c.as_array());

    if let Some(arr) = contents {
        for item in arr {
            if let Some(shelf) = item.get("musicShelfRenderer") {
                let tracks = parse_shelf_items(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
            if let Some(shelf) = item.get("musicPlaylistShelfRenderer") {
                let tracks = parse_shelf_items(shelf);
                if !tracks.is_empty() {
                    return tracks;
                }
            }
        }
    }

    Vec::new()
}

/// Extracts tracks from a `musicShelfRenderer`'s contents array.
fn parse_shelf_items(shelf: &serde_json::Value) -> Vec<Track> {
    shelf
        .get("contents")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    item.get("musicResponsiveListItemRenderer")
                        .and_then(parse_song_item)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Extracts the next continuation token from a response. Tries all known
/// paths used by YouTube Music for playlist/liked-songs browse responses.
fn extract_continuation(json: &serde_json::Value) -> Option<String> {
    // musicShelfRenderer continuation (twoColumn layout).
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicShelfRenderer/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // musicPlaylistShelfRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // sectionListRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // tabRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // secondaryContents sectionListRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // secondaryContents musicPlaylistShelfRenderer continuation.
    if let Some(token) = json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/continuations/0/nextContinuationData/continuation")
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    // Continuation in continuationContents (from a previous continuation page).
    if let Some(token) = json.pointer("/continuationContents/musicShelfContinuation/continuations/0/nextContinuationData/continuation")
        .or_else(|| json.pointer("/continuationContents/musicPlaylistShelfContinuation/continuations/0/nextContinuationData/continuation"))
        .or_else(|| json.pointer("/continuationContents/sectionListContinuation/continuations/0/nextContinuationData/continuation"))
        .and_then(|c| c.as_str())
    {
        return Some(token.to_string());
    }

    None
}
