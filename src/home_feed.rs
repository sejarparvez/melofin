//! Home feed data source for `ui::home_view`.
//!
//! Metrolist's real home screen (Quick picks, Forgotten favorites, Mixed
//! for you, Charts, ...) comes from YouTube Music's authenticated InnerTube
//! `browse` endpoint — it's personalized to the signed-in account. Melofin
//! doesn't have that: `doc/GUIDE.md`'s "Auth" build step is still last/
//! unstarted, and `search.rs` only talks to YouTube through unauthenticated
//! `yt-dlp` searches (see its doc comment for why rustypipe got dropped).
//!
//! So rather than block a working homepage on auth landing, this builds an
//! *unpersonalized* approximation of the same shape: a hero pick plus a
//! handful of titled rows, each backed by a plain `yt-dlp` search against a
//! curated query instead of a personalized recommendation. `HomeSection` is
//! deliberately just `(title, Vec<Track>)` — the same shape a real
//! InnerTube-backed feed would need — so swapping the source later is a
//! matter of replacing [`fetch_home_feed`]'s body, not the UI that renders
//! it.

use crate::search::{self, Track};

/// One titled, horizontally-scrolling row on the home page.
pub struct HomeSection {
    pub title: String,
    pub tracks: Vec<Track>,
}

/// The full home page: whichever rows successfully loaded, in
/// [`SECTIONS`] order. Can be empty (all rows failed — offline, no
/// `yt-dlp`, etc.), which the UI renders as an error/retry state rather
/// than a blank page.
pub struct HomeFeed {
    pub sections: Vec<HomeSection>,
}

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

/// Runs every row's search in parallel (each spawns its own `yt-dlp`
/// process via [`search::search`]) and returns whichever came back with
/// results, in [`SECTIONS`] order regardless of finish order. A row whose
/// query errored or came back empty is dropped rather than shown blank —
/// a partial feed beats an error mid-page.
///
/// Blocking: this shells out to `yt-dlp` once per row and waits for all of
/// them. Call it from a background thread, same as `search::search` —
/// never from the GTK main thread. See `ui::home_view::load_feed` for the
/// `thread::spawn` + `async_channel` wiring that does that.
pub fn fetch_home_feed() -> HomeFeed {
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
        // A panicked search thread (shouldn't happen — `search::search`
        // returns `Result` rather than panicking) just drops that one row
        // instead of losing the whole feed.
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
