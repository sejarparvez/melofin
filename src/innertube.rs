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
use crate::user::{build_cookie_header, build_sapisidhash, extract_innertube_api_key};

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const CLIENT_VERSION: &str = "1.20250710.01.00";

const ORIGIN: &str = "https://music.youtube.com";

/// Fetches the personalized home feed from YouTube Music's InnerTube API.
///
/// Requires the user's cookies (from `~/.local/share/melofin/cookies.txt`).
/// Makes two HTTP requests: one for the initial browse response, one for the
/// continuation that contains the actual feed sections.
///
/// Blocking — call from a background thread, never from the GTK main thread.
pub fn browse_home(cookies_path: &Path) -> Result<HomeFeed> {
    let contents = std::fs::read_to_string(cookies_path).context("couldn't read cookies file")?;

    let cookie_header = build_cookie_header(&contents);
    anyhow::ensure!(
        !cookie_header.is_empty(),
        "cookie header is empty — cookies.txt malformed?"
    );

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

    // Step 1: initial browse — gets sections + continuation token.
    let initial =
        browse_request(&cookie_header, &api_key, None).context("initial browse request failed")?;

    // Collect sections from the initial response — it often already has
    // real carousel data before the continuation.
    let mut sections = parse_sections(&initial);

    // Follow continuation tokens to load more sections. Each response
    // may contain a next continuation token for the next page.
    let section_list_path = &[
        "/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer",
        "/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer",
    ];

    let mut current_token = section_list_path.iter().find_map(|path| {
        initial
            .pointer(&format!(
                "{path}/continuations/0/nextContinuationData/continuation"
            ))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
    });

    let max_pages = 10;
    let mut page = 0;
    while let Some(token) = current_token {
        page += 1;
        if page > max_pages {
            tracing::debug!("reached max continuation pages ({max_pages}), stopping");
            break;
        }

        let cont_response = browse_request(&cookie_header, &api_key, Some(&token))
            .context("continuation browse request failed")?;

        let cont_sections = parse_continuation(&cont_response);
        let count = cont_sections.len();
        sections.extend(cont_sections);

        // Extract next continuation token from this response.
        current_token = cont_response
            .pointer("/continuationContents/sectionListContinuation/continuations/0/nextContinuationData/continuation")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        tracing::debug!(
            page,
            count,
            has_next = current_token.is_some(),
            "continuation page"
        );

        if count == 0 {
            break;
        }
    }

    tracing::info!(
        section_count = sections.len(),
        "fetched personalized home feed"
    );

    Ok(HomeFeed { sections })
}

/// Sends a POST to `/youtubei/v1/browse`. If `continuation` is `Some`, sends
/// the continuation token instead of the `FEmusic_home` browse ID.
fn browse_request(
    cookie_header: &str,
    api_key: &str,
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
            "browseId": "FEmusic_home"
        })
    };

    let mut req = ureq::post(&url)
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

    let text = req
        .send_string(&body.to_string())
        .context("browse endpoint request failed")?
        .into_string()
        .context("couldn't read browse response body")?;

    serde_json::from_str(&text).context("browse response is not valid JSON")
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

/// Parses a song from a `musicResponsiveListItemRenderer`. These appear in
/// "Quick picks" and similar song-list sections.
fn parse_song_item(renderer: &serde_json::Value) -> Option<Track> {
    let video_id = renderer.get("videoId")?.as_str()?;

    // Title is in the first flex column.
    let title = renderer
        .pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;

    // Artist is the first element of the second flex column's secondary line.
    let secondary = renderer
        .pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text/runs")?
        .as_array()?;

    let artist = secondary
        .iter()
        .find(|run| {
            // Skip album links (they have browseEndpoint with MPREb_ prefix).
            !run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(|id| id.as_str())
                .map_or(false, |id| id.starts_with("MPREb_"))
        })?
        .get("text")?
        .as_str()?;

    let thumbnail = renderer
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")?
        .as_array()?
        .last()?
        .get("url")?
        .as_str()?;

    Some(Track {
        title: title.to_string(),
        artist: artist.to_string(),
        url: format!("https://music.youtube.com/watch?v={video_id}"),
        thumbnail_url: thumbnail.to_string(),
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

    let thumbnail = renderer
        .pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails")?
        .as_array()?
        .last()?
        .get("url")?
        .as_str()?;

    Some(Track {
        title: title.to_string(),
        artist: artist.to_string(),
        url,
        thumbnail_url: thumbnail.to_string(),
    })
}
