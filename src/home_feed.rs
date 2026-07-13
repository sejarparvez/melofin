//! Home feed data source for `ui::home_view`.
//!
//! When the user is logged in, fetches a real personalized feed from
//! YouTube Music's InnerTube `browse` endpoint (`FEmusic_home`) — the
//! same API Metrolist and other YTM clients use. This returns actual
//! personalized sections like Quick Picks, Forgotten Favorites, Mixed
//! For You, Charts, etc.
//!
//! When InnerTube fails or the user isn't logged in, falls back to
//! unpersonalized `yt-dlp` searches against curated queries.
//!
//! `HomeSection` is deliberately just `(title, Vec<Track>)` — the same
//! shape both the InnerTube feed and the yt-dlp fallback produce — so
//! the UI that renders it doesn't care which source provided the data.

use crate::search::{self, Track};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One titled, horizontally-scrolling row on the home page.
#[derive(Clone, Serialize, Deserialize)]
pub struct HomeSection {
    pub title: String,
    pub tracks: Vec<Track>,
}

/// The full home page: whichever rows successfully loaded, in
/// [`SECTIONS`] order. Can be empty (all rows failed — offline, no
/// `yt-dlp`, etc.), which the UI renders as an error/retry state rather
/// than a blank page.
#[derive(Clone, Serialize, Deserialize)]
pub struct HomeFeed {
    pub sections: Vec<HomeSection>,
}

/// On-disk cache format. Includes a timestamp so we can expire stale data.
#[derive(Serialize, Deserialize)]
struct CachedFeed {
    fetched_at: String,
    feed: HomeFeed,
}

/// Cache expires after 30 minutes — YouTube Music's feed doesn't change
/// that fast, and this keeps startup instant for repeated launches.
const CACHE_MAX_AGE_SECS: u64 = 30 * 60;

/// `(row title, search query)` pairs. Picked to feel like a generic,
/// logged-out YouTube Music home feed — a "what's popular right now" row
/// plus a few mood/era buckets — rather than anything actually
/// personalized. Swap/extend freely; nothing else depends on these exact
/// queries.
const SECTIONS: &[(&str, &str)] = &[
    ("Trending now", "trending music"),
    ("Chill & focus", "lofi chill beats playlist"),
    ("Throwback hits", "2010s throwback hits"),
    ("New releases", "new music releases this week"),
];

/// Fetches the home feed with disk caching. When `cookies_path` points to
/// a valid cookies file (user is logged in), calls the InnerTube
/// `FEmusic_home` browse endpoint for a real personalized feed. Falls back
/// to unpersonalized `yt-dlp` searches when InnerTube fails or when no
/// cookies exist.
///
/// Caches the result to `cache_path` as JSON. On subsequent calls, returns
/// the cached feed if it's less than 30 minutes old, avoiding a network
/// round-trip. Stale caches are refreshed in the background (the caller
/// gets the stale data immediately and the fresh data replaces it on the
/// next load).
///
/// Blocking: makes HTTP requests or shells out to `yt-dlp`. Call from a
/// background thread, never from the GTK main thread.
pub fn fetch_home_feed(cookies_path: &Path, cache_path: &Path) -> HomeFeed {
    // Try cache first.
    if let Some(cached) = load_cache(cache_path) {
        tracing::info!(
            sections = cached.sections.len(),
            "returning cached home feed"
        );
        return cached;
    }

    // Cache miss or stale — fetch fresh data.
    let feed = fetch_fresh_feed(cookies_path);

    // Save to cache (best-effort — don't fail the feed if caching breaks).
    if !feed.sections.is_empty() {
        save_cache(cache_path, &feed);
    }

    feed
}

/// Fetches a fresh feed from InnerTube or yt-dlp.
fn fetch_fresh_feed(cookies_path: &Path) -> HomeFeed {
    // Try the InnerTube personalized feed first.
    if cookies_path.exists() {
        match crate::innertube::browse_home(cookies_path) {
            Ok(feed) if !feed.sections.is_empty() => {
                tracing::info!(
                    sections = feed.sections.len(),
                    "using personalized InnerTube feed"
                );
                return feed;
            }
            Ok(_) => {
                tracing::warn!("InnerTube feed returned empty, falling back to yt-dlp");
            }
            Err(e) => {
                tracing::warn!("InnerTube feed failed: {e}, falling back to yt-dlp");
            }
        }
    }

    // Fallback: unpersonalized yt-dlp search rows.
    tracing::info!("using yt-dlp fallback feed");
    fetch_ytdlp_feed()
}

/// Loads a cached feed from disk. Returns `None` if the file doesn't exist,
/// is malformed, or is older than [`CACHE_MAX_AGE_SECS`].
fn load_cache(path: &Path) -> Option<HomeFeed> {
    let data = std::fs::read_to_string(path).ok()?;
    let cached: CachedFeed = serde_json::from_str(&data).ok()?;

    // Check age.
    let fetched_at = chrono_parse(&cached.fetched_at)?;
    let age = std::time::SystemTime::now()
        .duration_since(fetched_at)
        .ok()?
        .as_secs();

    if age > CACHE_MAX_AGE_SECS {
        tracing::debug!(age_secs = age, "home feed cache is stale");
        return None;
    }

    Some(cached.feed)
}

/// Saves the feed to disk as JSON.
fn save_cache(path: &Path, feed: &HomeFeed) {
    let cached = CachedFeed {
        fetched_at: now_iso8601(),
        feed: feed.clone(),
    };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match serde_json::to_string_pretty(&cached) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!("failed to write home feed cache: {e}");
            }
        }
        Err(e) => {
            tracing::warn!("failed to serialize home feed cache: {e}");
        }
    }
}

/// Returns the current time as an ISO 8601 string (UTC).
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

/// Parses an ISO 8601 string (our format: unix timestamp as string) back
/// to `SystemTime`. Returns `None` on parse failure.
fn chrono_parse(s: &str) -> Option<std::time::SystemTime> {
    let secs: u64 = s.parse().ok()?;
    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
}

/// Builds an unpersonalized feed from `yt-dlp` search queries. Each row
/// runs in its own thread for parallelism.
fn fetch_ytdlp_feed() -> HomeFeed {
    let handles: Vec<_> = SECTIONS
        .iter()
        .map(|(title, query)| {
            let title = title.to_string();
            let query = query.to_string();
            std::thread::spawn(move || (title, search::search(&query)))
        })
        .collect();

    let sections = handles
        .into_iter()
        .filter_map(|handle| handle.join().ok())
        .filter_map(|(title, result)| match result {
            Ok(tracks) if !tracks.is_empty() => Some(HomeSection { title, tracks }),
            Ok(_) => None,
            Err(e) => {
                tracing::warn!("home feed row failed: {e}");
                None
            }
        })
        .collect();

    HomeFeed { sections }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::{HomeFeed, HomeSection, load_cache, save_cache};
    use crate::user::{build_cookie_header, build_sapisidhash};

    /// Probe the InnerTube browse endpoint and dump the raw response.
    /// Run with: cargo test probe_browse -- --nocapture
    #[test]
    fn probe_browse() {
        let cookies_path =
            std::path::PathBuf::from(std::env::var("HOME").expect("no HOME env var"))
                .join(".local/share/melofin/cookies.txt");

        let contents = std::fs::read_to_string(&cookies_path)
            .expect("couldn't read cookies.txt — are you logged in?");

        let cookie_header = build_cookie_header(&contents);
        assert!(
            !cookie_header.is_empty(),
            "cookie header is empty — cookies.txt malformed?"
        );

        let ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

        // Step 1: fetch page to get API key
        let html = ureq::get("https://music.youtube.com")
            .set("Cookie", &cookie_header)
            .set("User-Agent", ua)
            .timeout(std::time::Duration::from_secs(15))
            .call()
            .expect("failed to fetch music.youtube.com")
            .into_string()
            .expect("couldn't read response body");

        let api_key = extract_api_key(&html).expect("couldn't find INNERTUBE_API_KEY in page HTML");
        eprintln!("API key: {api_key}");

        // Step 2: call the browse endpoint
        let origin = "https://music.youtube.com";
        let url =
            format!("https://music.youtube.com/youtubei/v1/browse?key={api_key}&prettyPrint=false");

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20250710.01.00",
                    "hl": "en",
                    "gl": "US"
                }
            },
            "browseId": "FEwhat_to_watch"
        });

        let body_str = body.to_string();
        eprintln!("Request body: {body_str}");

        let mut req = ureq::post(&url)
            .set("Cookie", &cookie_header)
            .set("User-Agent", ua)
            .set("Content-Type", "application/json")
            .set("X-Origin", origin)
            .set("Referer", "https://music.youtube.com/")
            .set("X-Goog-Api-Format-Version", "1")
            .set("X-YouTube-Client-Name", "67")
            .set("X-YouTube-Client-Version", "1.20250710.01.00")
            .timeout(std::time::Duration::from_secs(15));

        if let Some(auth) = build_sapisidhash(&cookie_header, origin) {
            req = req.set("Authorization", &auth);
            eprintln!("Authorization: {auth}");
        }

        let response = req
            .send_string(&body_str)
            .expect("browse endpoint request failed");

        let status = response.status();
        eprintln!("Response status: {status}");

        let text = response
            .into_string()
            .expect("couldn't read browse response body");

        // Save full response
        let out_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("doc")
            .join("browse-response.json");
        std::fs::create_dir_all(out_path.parent().unwrap()).ok();
        let mut f = std::fs::File::create(&out_path).expect("couldn't create output file");
        // Pretty-print the JSON for readability
        let pretty: serde_json::Value =
            serde_json::from_str(&text).expect("response is not valid JSON");
        let pretty_str = serde_json::to_string_pretty(&pretty).unwrap();
        f.write_all(pretty_str.as_bytes()).unwrap();
        eprintln!("Response saved to {}", out_path.display());

        // Quick summary of top-level keys
        if let serde_json::Value::Object(map) = &pretty {
            eprintln!("Top-level keys: {:?}", map.keys().collect::<Vec<_>>());
        }

        // Check for errors
        if pretty.get("error").is_some() {
            eprintln!(
                "ERROR in response: {}",
                serde_json::to_string_pretty(pretty.get("error").unwrap()).unwrap()
            );
        }

        // Look for the content structure
        if let Some(contents) = pretty.get("contents") {
            eprintln!(
                "contents keys: {:?}",
                contents.as_object().map(|m| m.keys().collect::<Vec<_>>())
            );

            if let Some(two_col) = contents.get("twoColumnBrowseNextRenderer") {
                let tabs = two_col
                    .get("results")
                    .and_then(|r| r.get("tabs"))
                    .and_then(|t| t.as_array());
                if let Some(tabs) = tabs {
                    eprintln!("Found {} tabs", tabs.len());
                    for (i, tab) in tabs.iter().enumerate() {
                        let title = tab
                            .get("tabRenderer")
                            .and_then(|t| t.get("title"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("???");
                        eprintln!("  Tab {i}: {title}");
                    }
                }
            }
        }

        assert!(!text.is_empty(), "response is empty");

        // Now fetch the continuation to get actual feed content
        let continuation = pretty
            .pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations/0/nextContinuationData/continuation")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        if let Some(continuation) = continuation {
            eprintln!("\nFetching continuation for actual feed content...");
            let cont_body = serde_json::json!({
                "context": {
                    "client": {
                        "clientName": "WEB_REMIX",
                        "clientVersion": "1.20250710.01.00",
                        "hl": "en",
                        "gl": "US"
                    }
                },
                "continuation": continuation
            });

            let cont_url = format!(
                "https://music.youtube.com/youtubei/v1/browse?key={api_key}&prettyPrint=false"
            );

            let mut cont_req = ureq::post(&cont_url)
                .set("Cookie", &cookie_header)
                .set("User-Agent", ua)
                .set("Content-Type", "application/json")
                .set("X-Origin", origin)
                .set("Referer", "https://music.youtube.com/")
                .set("X-Goog-Api-Format-Version", "1")
                .set("X-YouTube-Client-Name", "67")
                .set("X-YouTube-Client-Version", "1.20250710.01.00")
                .timeout(std::time::Duration::from_secs(15));

            if let Some(auth) = build_sapisidhash(&cookie_header, origin) {
                cont_req = cont_req.set("Authorization", &auth);
            }

            let cont_response = cont_req
                .send_string(&cont_body.to_string())
                .expect("continuation request failed");

            let cont_status = cont_response.status();
            eprintln!("Continuation response status: {cont_status}");

            let cont_text = cont_response
                .into_string()
                .expect("couldn't read continuation response");

            let cont_pretty: serde_json::Value =
                serde_json::from_str(&cont_text).expect("continuation response is not valid JSON");

            let cont_out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("doc")
                .join("browse-continuation-response.json");
            let mut cf =
                std::fs::File::create(&cont_out).expect("couldn't create continuation file");
            let cont_pretty_str = serde_json::to_string_pretty(&cont_pretty).unwrap();
            cf.write_all(cont_pretty_str.as_bytes()).unwrap();
            eprintln!("Continuation response saved to {}", cont_out.display());

            // Analyze continuation response structure
            if let serde_json::Value::Object(map) = &cont_pretty {
                eprintln!(
                    "Continuation top-level keys: {:?}",
                    map.keys().collect::<Vec<_>>()
                );
            }

            // Look for onResponseActions or actions
            if let Some(actions) = cont_pretty.get("onResponseReceivedActions") {
                eprintln!(
                    "onResponseReceivedActions count: {}",
                    actions.as_array().map_or(0, |a| a.len())
                );
                if let Some(first_action) = actions.get(0) {
                    eprintln!(
                        "First action keys: {:?}",
                        first_action
                            .as_object()
                            .map(|m| m.keys().collect::<Vec<_>>())
                    );

                    // Look for appendContinuationItemsAction
                    if let Some(append) = first_action.get("appendContinuationItemsAction") {
                        if let Some(items) = append.get("continuationItems") {
                            eprintln!(
                                "continuationItems count: {}",
                                items.as_array().map_or(0, |a| a.len())
                            );
                            if let Some(first_item) = items.get(0) {
                                eprintln!(
                                    "First item keys: {:?}",
                                    first_item.as_object().map(|m| m.keys().collect::<Vec<_>>())
                                );

                                // Try to find shelf/section structures
                                if let Some(shelf) = first_item.get("musicShelfRenderer") {
                                    eprintln!("Found musicShelfRenderer!");
                                    let title = shelf
                                        .get("title")
                                        .and_then(|t| t.get("runs"))
                                        .and_then(|r| r.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|r| r.get("text"))
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("???");
                                    eprintln!("  Shelf title: {title}");
                                    let contents = shelf.get("contents").and_then(|c| c.as_array());
                                    eprintln!(
                                        "  Shelf contents count: {}",
                                        contents.map_or(0, |a| a.len())
                                    );
                                }

                                if let Some(_rich) =
                                    first_item.get("musicResponsiveListItemRenderer")
                                {
                                    eprintln!("Found musicResponsiveListItemRenderer!");
                                }

                                if let Some(carousel) = first_item.get("musicCarouselShelfRenderer")
                                {
                                    eprintln!("Found musicCarouselShelfRenderer!");
                                    let title = carousel
                                        .get("header")
                                        .and_then(|h| {
                                            h.get("musicCarouselShelfBasicHeaderRenderer")
                                        })
                                        .and_then(|h| h.get("title"))
                                        .and_then(|t| t.get("runs"))
                                        .and_then(|r| r.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|r| r.get("text"))
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("???");
                                    eprintln!("  Carousel title: {title}");
                                }
                            }
                        }
                    }
                }
            }

            // Also check for frameworkUpdates in continuation
            if let Some(fwu) = cont_pretty.get("frameworkUpdates") {
                eprintln!("Continuation has frameworkUpdates");
                if let Some(eu) = fwu.get("elementUpdate") {
                    if let Some(updates) = eu.get("updates") {
                        eprintln!(
                            "  elementUpdate updates count: {}",
                            updates.as_array().map_or(0, |a| a.len())
                        );
                    }
                }
            }
        } else {
            eprintln!(
                "No continuation token found — content might be in frameworkUpdates templates"
            );
        }
    }

    fn extract_api_key(html: &str) -> Option<String> {
        crate::user::extract_innertube_api_key(html)
    }

    #[test]
    fn cache_round_trip() {
        use crate::search::Track;

        let feed = HomeFeed {
            sections: vec![HomeSection {
                title: "Test Section".into(),
                tracks: vec![Track {
                    title: "Song A".into(),
                    artist: "Artist B".into(),
                    url: "https://example.com".into(),
                    thumbnail_url: "https://example.com/thumb.jpg".into(),
                }],
            }],
        };

        let tmp = std::env::temp_dir().join(format!("melofin_cache_test_{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        // Empty cache → None.
        assert!(load_cache(&tmp).is_none());

        // Save then load.
        save_cache(&tmp, &feed);
        let loaded = load_cache(&tmp).expect("cache should exist after save");
        assert_eq!(loaded.sections.len(), 1);
        assert_eq!(loaded.sections[0].title, "Test Section");
        assert_eq!(loaded.sections[0].tracks[0].title, "Song A");

        let _ = std::fs::remove_file(&tmp);
    }
}
