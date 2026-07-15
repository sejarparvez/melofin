//! InnerTube client for fetching the personalized YouTube Music home feed.
//!
//! Calls the `FEmusic_home` browse endpoint with the user's cookies to get
//! real personalized sections (Quick Picks, Forgotten Favorites, Mixed For
//! You, Charts, etc.) instead of the unpersonalized `yt-dlp` search
//! approximation in `home_feed.rs`.

use anyhow::{Context, Result};
use std::path::Path;

use crate::home_feed::{HomeFeed, HomeSection};
use crate::search::Track;
use crate::user::{build_sapisidhash, extract_innertube_api_key, read_and_validate_cookies};

pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub(crate) const CLIENT_VERSION: &str = "1.20250710.01.00";

const ORIGIN: &str = "https://music.youtube.com";

/// Builds a POST request to a YouTube Music InnerTube endpoint with the
/// standard headers (cookies, user-agent, client version, SAPISIDHASH auth).
pub(crate) fn build_innertube_request(url: &str, cookie_header: &str) -> ureq::Request {
    let mut req = ureq::post(url)
        .set("Cookie", cookie_header)
        .set("User-Agent", USER_AGENT)
        .set("Content-Type", "application/json")
        .set("X-Origin", ORIGIN)
        .set("Referer", "https://music.youtube.com/")
        .set("X-Goog-Api-Format-Version", "1")
        .set("X-YouTube-Client-Name", "67")
        .set("X-YouTube-Client-Version", CLIENT_VERSION)
        .timeout(std::time::Duration::from_secs(15));

    if let Some(auth) = build_sapisidhash(cookie_header, ORIGIN) {
        req = req.set("Authorization", &auth);
    }
    req
}

/// Extra browse IDs to fetch alongside `FEmusic_home` for a richer
/// personalized feed. Each entry is `(browse_id, fallback_title)` — if the
/// API call fails, the section is silently skipped.
const EXTRA_BROWSE_IDS: &[(&str, &str)] = &[
    ("FEmusic_charts", "Charts"),
    ("FEmusic_new_releases", "New Releases"),
    ("FEmusic_listen_again", "Listen Again"),
    ("FEmusic_mixed_for_you", "Mixed For You"),
];

/// Extracts the next continuation token from a browse response. Tries
/// both `singleColumnBrowseResultsRenderer` and
/// `twoColumnBrowseResultsRenderer` layout paths.
fn extract_continuation(json: &serde_json::Value) -> Option<String> {
    let paths = [
        "/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation",
        "/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation",
    ];
    paths.iter().find_map(|path| {
        json.pointer(path)
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
    })
}

/// Fetches a single browse_id and returns its parsed sections (initial
/// response + one level of continuation). Returns an empty `Vec` on error
/// rather than propagating, so callers can skip failed sections.
pub(crate) fn browse_section(
    cookie_header: &str,
    api_key: &str,
    browse_id: &str,
) -> Vec<HomeSection> {
    let initial = match browse_request(cookie_header, api_key, Some(browse_id), None) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!("browse_section({browse_id}) initial request failed: {e}");
            return Vec::new();
        }
    };

    let mut sections = parse_sections(&initial);

    if let Some(token) = extract_continuation(&initial)
        && let Ok(cont) = browse_request(cookie_header, api_key, None, Some(&token))
    {
        sections.extend(parse_continuation(&cont));
    }

    sections
}

/// Fetches the personalized home feed from YouTube Music's InnerTube API.
///
/// Requires the user's cookies (from `~/.local/share/melofin/cookies.txt`).
/// Fetches `FEmusic_home` plus extra browse IDs (charts, new releases,
/// etc.) in parallel for a richer feed.
///
/// Blocking — call from a background thread, never from the GTK main thread.
pub fn browse_home(cookies_path: &Path) -> Result<HomeFeed> {
    let cookie_header = read_and_validate_cookies(cookies_path)?;

    // Fetch the page to get the API key.
    let html = ureq::get(ORIGIN)
        .set("Cookie", &cookie_header)
        .set("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .context("failed to fetch music.youtube.com")?
        .into_string()
        .context("couldn't read YT Music HTML")?;

    let api_key =
        extract_innertube_api_key(&html).context("couldn't find INNERTUBE_API_KEY in page HTML")?;

    // Fetch FEmusic_home (with full continuation following) + extra
    // browse_ids in parallel.
    let mut sections = browse_home_with_continuations(&cookie_header, &api_key);

    let handles: Vec<_> = EXTRA_BROWSE_IDS
        .iter()
        .map(|(browse_id, _title)| {
            let cookie = cookie_header.clone();
            let key = api_key.clone();
            let id = browse_id.to_string();
            std::thread::spawn(move || browse_section(&cookie, &key, &id))
        })
        .collect();

    for (handle, &(_, title)) in handles.into_iter().zip(EXTRA_BROWSE_IDS.iter()) {
        match handle.join() {
            Ok(extra) if !extra.is_empty() => {
                sections.extend(extra);
            }
            Ok(_) => {}
            Err(_) => {
                tracing::warn!(title, "extra section thread panicked, skipping");
            }
        }
    }

    Ok(HomeFeed { sections })
}

/// Fetches `FEmusic_home` with full continuation pagination (up to 10
/// pages). This is the high-volume home feed that needs multiple
/// continuation rounds, unlike the extra browse IDs which only need
/// initial + one continuation.
fn browse_home_with_continuations(cookie_header: &str, api_key: &str) -> Vec<HomeSection> {
    let initial = match browse_request(cookie_header, api_key, None, None) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!("FEmusic_home initial request failed: {e}");
            return Vec::new();
        }
    };

    let mut sections = parse_sections(&initial);
    let mut current_token = extract_continuation(&initial);

    let max_pages = 10;
    let mut page = 0;
    while let Some(token) = current_token {
        page += 1;
        if page > max_pages {
            break;
        }

        let cont_response = match browse_request(cookie_header, api_key, None, Some(&token)) {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!("FEmusic_home continuation page {page} failed: {e}");
                break;
            }
        };

        let cont_sections = parse_continuation(&cont_response);
        let count = cont_sections.len();
        sections.extend(cont_sections);

        current_token = cont_response
            .pointer("/continuationContents/sectionListContinuation/continuations/0/nextContinuationData/continuation")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        if count == 0 {
            break;
        }
    }

    sections
}

/// Sends a POST to `/youtubei/v1/browse`.
///
/// - If `continuation` is `Some`, sends the continuation token instead of a
///   fresh browse ID.
/// - Otherwise, sends `browse_id` if provided, or falls back to
///   `FEmusic_home` (the personalized home feed) if `browse_id` is `None`.
pub(crate) fn browse_request(
    cookie_header: &str,
    api_key: &str,
    browse_id: Option<&str>,
    continuation: Option<&str>,
) -> Result<serde_json::Value> {
    let url =
        format!("https://music.youtube.com/youtubei/v1/browse?key={api_key}&prettyPrint=false");

    let body = if let Some(cont) = continuation {
        serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": CLIENT_VERSION,
                    "hl": "en",
                    "gl": "US"
                }
            },
            "continuation": cont
        })
    } else {
        serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": CLIENT_VERSION,
                    "hl": "en",
                    "gl": "US"
                }
            },
            "browseId": browse_id.unwrap_or("FEmusic_home")
        })
    };

    let text = build_innertube_request(&url, cookie_header)
        .send_string(&body.to_string())
        .context("browse endpoint request failed")?
        .into_string()
        .context("couldn't read browse response body")?;

    serde_json::from_str(&text).context("browse response is not valid JSON")
}

/// Searches YouTube Music for an artist by name and returns their UC-prefixed
/// browse ID, or `None` if no artist match is found.
///
/// Blocking — call from a background thread.
pub(crate) fn search_artist_browse_id(
    cookie_header: &str,
    api_key: &str,
    artist_name: &str,
) -> Option<String> {
    let url =
        format!("https://music.youtube.com/youtubei/v1/search?key={api_key}&prettyPrint=false");

    let body = serde_json::json!({
        "context": {
            "client": {
                "clientName": "WEB_REMIX",
                "clientVersion": CLIENT_VERSION,
                "hl": "en",
                "gl": "US"
            }
        },
        "query": artist_name
    });

    let text = build_innertube_request(&url, cookie_header)
        .send_string(&body.to_string())
        .ok()?
        .into_string()
        .ok()?;

    let json: serde_json::Value = serde_json::from_str(&text).ok()?;

    // Collect all musicResponsiveListItemRenderer items from the response.
    let all_items = collect_music_list_items(&json);

    for item in &all_items {
        // Try flexColumns first (most common for search results).
        if let Some(browse_id) = item
            .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/browseEndpoint/browseId")
            .and_then(|id| id.as_str())
            .filter(|id| id.starts_with("UC"))
        {
            return Some(browse_id.to_string());
        }
        // Try the top-level navigationEndpoint (some items have it directly).
        if let Some(browse_id) = item
            .pointer("/navigationEndpoint/browseEndpoint/browseId")
            .and_then(|id| id.as_str())
            .filter(|id| id.starts_with("UC"))
        {
            return Some(browse_id.to_string());
        }
    }

    // Fallback: look for any UC-prefixed string in the JSON that looks like
    // a browse ID, by scanning musicResponsiveListItemRenderer items more broadly.
    for item in &all_items {
        if let Some(obj) = item.as_object() {
            // Check if the item has a title run with a browseEndpoint.
            if let Some(browse_id) = obj
                .get("flexColumns")
                .and_then(|fc| fc.as_array())
                .and_then(|cols| cols.first())
                .and_then(|col| col.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs"))
                .and_then(|runs| runs.as_array())
                .and_then(|r| r.first())
                .and_then(|run| run.pointer("/navigationEndpoint/browseEndpoint/browseId"))
                .and_then(|id| id.as_str())
                .filter(|id| id.starts_with("UC"))
            {
                return Some(browse_id.to_string());
            }
        }
    }

    None
}

/// Recursively collects all `musicResponsiveListItemRenderer` values from a
/// JSON tree. Handles various nesting depths used by InnerTube responses.
fn collect_music_list_items(json: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut items = Vec::new();
    collect_music_list_items_inner(json, &mut items, 0);
    items
}

fn collect_music_list_items_inner<'a>(
    json: &'a serde_json::Value,
    items: &mut Vec<&'a serde_json::Value>,
    depth: usize,
) {
    if depth > 12 {
        return; // prevent infinite recursion
    }
    match json {
        serde_json::Value::Object(map) => {
            if let Some(renderer) = map.get("musicResponsiveListItemRenderer") {
                items.push(renderer);
            }
            for val in map.values() {
                collect_music_list_items_inner(val, items, depth + 1);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr {
                collect_music_list_items_inner(val, items, depth + 1);
            }
        }
        _ => {}
    }
}

/// Parses `musicCarouselShelfRenderer` sections from the initial browse
/// response. The response may use either `singleColumnBrowseResultsRenderer`
/// or `twoColumnBrowseResultsRenderer` — we try both.
fn parse_sections(json: &serde_json::Value) -> Vec<HomeSection> {
    let section_list_path = &[
        "/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents",
        "/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents",
    ];

    let carousels = section_list_path
        .iter()
        .find_map(|path| {
            json.pointer(path).and_then(|c| c.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("musicCarouselShelfRenderer"))
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();

    carousels.into_iter().filter_map(parse_carousel).collect()
}

/// Parses sections from a continuation response. The continuation wraps them
/// in `continuationContents.sectionListContinuation.contents`.
fn parse_continuation(json: &serde_json::Value) -> Vec<HomeSection> {
    let carousels = json
        .pointer("/continuationContents/sectionListContinuation/contents")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("musicCarouselShelfRenderer"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    carousels.into_iter().filter_map(parse_carousel).collect()
}

/// Extracts a titled section with its tracks from a
/// `musicCarouselShelfRenderer` JSON object.
fn parse_carousel(renderer: &serde_json::Value) -> Option<HomeSection> {
    let title = renderer
        .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title/runs")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;

    let contents = renderer.get("contents")?.as_array()?;

    let tracks: Vec<Track> = contents
        .iter()
        .filter_map(|item| {
            if let Some(renderer) = item.get("musicResponsiveListItemRenderer") {
                parse_song_item(renderer)
            } else if let Some(renderer) = item.get("musicTwoRowItemRenderer") {
                parse_two_row_item(renderer)
            } else {
                None
            }
        })
        .collect();

    if tracks.is_empty() {
        return None;
    }

    Some(HomeSection {
        title: title.to_string(),
        tracks,
    })
}

/// Normalizes a thumbnail URL: converts protocol-relative (`//`) to `https://`.
fn normalize_thumbnail_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

/// Parses a song from a `musicResponsiveListItemRenderer`. These appear in
/// "Quick picks", liked songs, and similar song-list sections.
pub(crate) fn parse_song_item(renderer: &serde_json::Value) -> Option<Track> {
    // YouTube Music removed playlistItemData.videoId from the renderer.
    // Use a fallback chain to find the video ID through multiple paths.
    let video_id = renderer
        .get("playlistItemData")
        .and_then(|p| p.get("videoId"))
        .or_else(|| renderer.pointer("/navigationEndpoint/watchEndpoint/videoId"))
        .or_else(|| renderer.pointer(
            "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId",
        ))
        .or_else(|| renderer.pointer(
            "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId",
        ))
        .and_then(|v| v.as_str())?;

    // Title is in the first flex column.
    let title = renderer
        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text")
        .and_then(|t| t.as_str())
        .unwrap_or("Unknown Title");

    // Artist: try flexColumns[1] first, then extract from accessibility label.
    let artist = renderer
        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
        .and_then(|r| r.as_array())
        .and_then(|runs| {
            runs.iter()
                .find(|run| {
                    // Skip album links (they have browseEndpoint with MPREb_ prefix).
                    !run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .and_then(|id| id.as_str())
                        .is_some_and(|id| id.starts_with("MPREb_"))
                })
                .and_then(|run| run.get("text"))
                .and_then(|t| t.as_str())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            // Fallback: extract from "Play Title - Artist" accessibility label.
            renderer
                .pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/accessibilityPlayData/accessibilityData/label")
                .and_then(|l| l.as_str())
                .and_then(|label| label.strip_prefix("Play "))
                .and_then(|rest| rest.rsplit_once(" - ").map(|(_, artist)| artist))
        })
        .unwrap_or("Unknown Artist");

    let thumbnail = renderer
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .filter(|s| !s.is_empty())
        .map(normalize_thumbnail_url)
        .unwrap_or_else(|| {
            // Album/playlist track items often lack a thumbnail field.
            // Build one from the video ID using YouTube's standard CDN.
            format!("https://i.ytimg.com/vi/{video_id}/mqdefault.jpg")
        });

    // Track number from index field.
    let track_number = renderer
        .pointer("/index/runs/0/text")
        .and_then(|t| t.as_str())
        .and_then(|s| s.parse::<u32>().ok());

    // Duration from fixedColumns.
    let duration = renderer
        .pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text/runs/0/text")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Album name: look for an album link (MPREb_ browseId) in flexColumns.
    let album = renderer
        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
        .and_then(|r| r.as_array())
        .and_then(|runs| {
            runs.iter()
                .find(|run| {
                    run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .and_then(|id| id.as_str())
                        .is_some_and(|id| id.starts_with("MPREb_"))
                })
                .and_then(|run| run.get("text"))
                .and_then(|t| t.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        });

    // Artist browse ID: extract UC-prefixed channel ID from artist link in flexColumns.
    let artist_browse_id = renderer
        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")
        .and_then(|r| r.as_array())
        .and_then(|runs| {
            runs.iter()
                .find(|run| {
                    run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .and_then(|id| id.as_str())
                        .is_some_and(|id| id.starts_with("UC"))
                })
                .and_then(|run| {
                    run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .and_then(|id| id.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                })
        });

    Some(Track {
        title: title.to_string(),
        artist: artist.to_string(),
        url: format!("https://music.youtube.com/watch?v={video_id}"),
        thumbnail_url: thumbnail.to_string(),
        track_number,
        duration,
        album,
        artist_browse_id,
    })
}

/// Parses a track from a `musicTwoRowItemRenderer`. These appear for albums,
/// playlists, and artists in the home feed. We map them loosely to `Track`
/// — the UI already handles these gracefully.
fn parse_two_row_item(renderer: &serde_json::Value) -> Option<Track> {
    let title = renderer
        .pointer("/title/runs")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;

    // Subtitle runs contain artist name (for songs), author (for playlists),
    // or artist list (for albums). Take the first run as the "artist".
    let artist = renderer
        .pointer("/subtitle/runs")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;

    // Build URL from browse endpoint or watch endpoint.
    let url = if let Some(browse_id) = renderer
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(|id| id.as_str())
    {
        format!("https://music.youtube.com/browse/{browse_id}")
    } else if let Some(video_id) = renderer
        .pointer("/navigationEndpoint/watchEndpoint/videoId")
        .and_then(|id| id.as_str())
    {
        format!("https://music.youtube.com/watch?v={video_id}")
    } else {
        return None;
    };

    let thumbnail = normalize_thumbnail_url(
        renderer
            .pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails")?
            .as_array()?
            .last()?
            .get("url")?
            .as_str()?,
    );

    // Artist browse ID: try longBylineText, shortBylineText, then subtitle runs.
    // Home feed song cards store the artist channel link in longBylineText/shortBylineText
    // with a browseEndpoint, while subtitle runs are usually plain text.
    let artist_browse_id = ["longBylineText", "shortBylineText", "subtitle"]
        .iter()
        .find_map(|field| {
            renderer
                .pointer(&format!("/{field}/runs"))
                .and_then(|r| r.as_array())
                .and_then(|runs| {
                    runs.iter()
                        .find(|run| {
                            run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                                .and_then(|id| id.as_str())
                                .is_some_and(|id| id.starts_with("UC"))
                        })
                        .and_then(|run| {
                            run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                                .and_then(|id| id.as_str())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                        })
                })
        });

    Some(Track {
        title: title.to_string(),
        artist: artist.to_string(),
        url,
        thumbnail_url: thumbnail,
        track_number: None,
        duration: None,
        album: None,
        artist_browse_id,
    })
}
