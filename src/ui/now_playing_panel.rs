//! Right "Now Playing" panel: album art + title/artist reflect real
//! playback state, updated the same way as `player_bar.rs::update` — this
//! is the one sidebar element that isn't placeholder data, since it's just
//! showing what `player.rs` already tracks. The "About the artist" blurb
//! fetches the artist's bio from YouTube Music's InnerTube API when the
//! track changes.
//!
//! Width note: `pub widget` is a `ScrolledWindow`, not the content `Box`
//! directly. Two rounds of trying to cap width purely via
//! `set_size_request`/`hexpand`/`max_width_chars` on the `Box` and its
//! labels still let a long real track title/artist (only ever seen once
//! playback starts — placeholder text is always short) win the size
//! negotiation and stretch the sidebar. A `Box`'s natural width is the max
//! of its children's, and a `Label`'s natural width is only *capped* by
//! `max-width-chars`, not fixed — bold/wide-glyph text can still exceed
//! the estimate, and that overflow was propagating straight up through the
//! `Box` to the window. A `ScrolledWindow` with `propagate_natural_width`
//! left at its default `false` and an explicit `min`/`max_content_width`
//! doesn't have that problem: it never asks its parent for more than the
//! width it's told, no matter how wide its child wants to be — it clips
//! (or, with `hscrollbar_policy(Never)`, just relies on the child's own
//! wrapping/ellipsizing within that fixed viewport) instead of growing.

use std::cell::RefCell;
use std::path::PathBuf;

use crate::player::PlayerState;
use crate::ui::thumbnail_widget::ThumbnailStack;
use adw::prelude::*;
use gtk::glib;

/// Fixed width of the whole panel, in pixels. Every other size in here
/// (`ART_SIZE`, the label `max_width_chars` calls) is derived from this
/// one constant so resizing the sidebar again is a one-line change instead
/// of the three-numbers-that-happened-to-agree-once problem from before.
const PANEL_WIDTH: i32 = 240;
const PANEL_MARGIN: i32 = 8;
const ART_SIZE: i32 = PANEL_WIDTH - 2 * PANEL_MARGIN;

pub struct NowPlayingPanel {
    pub widget: gtk::ScrolledWindow,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    thumbnail: ThumbnailStack,
    about_body: gtk::Label,
    cookies_path: PathBuf,
    /// The artist name whose bio is currently being displayed or fetched.
    /// Used to avoid re-fetching when the same artist plays again, and to
    /// cancel (skip) stale fetches when the track changes mid-fetch.
    current_artist: RefCell<String>,
}

impl NowPlayingPanel {
    pub fn new(cookies_path: PathBuf) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(10);
        content.set_margin_bottom(10);
        content.set_margin_start(PANEL_MARGIN);
        content.set_margin_end(PANEL_MARGIN);

        let heading = gtk::Label::new(Some("Now Playing"));
        heading.add_css_class("heading");
        heading.set_halign(gtk::Align::Start);
        content.append(&heading);

        let thumbnail = ThumbnailStack::new("emblem-music-symbolic", 48, ART_SIZE);
        let art_frame = gtk::Frame::new(None);
        art_frame.add_css_class("card");
        art_frame.add_css_class("home-art");
        art_frame.set_size_request(ART_SIZE, ART_SIZE);
        art_frame.set_hexpand(false);
        art_frame.set_child(Some(thumbnail.widget()));
        content.append(&art_frame);

        let title_label = gtk::Label::new(Some("Nothing playing"));
        title_label.add_css_class("title-2");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(22);

        let artist_label = gtk::Label::new(Some(""));
        artist_label.add_css_class("dim-label");
        artist_label.set_halign(gtk::Align::Start);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.set_max_width_chars(22);

        content.append(&title_label);
        content.append(&artist_label);

        let about_heading = gtk::Label::new(Some("About the artist"));
        about_heading.add_css_class("heading");
        about_heading.set_halign(gtk::Align::Start);
        about_heading.set_margin_top(12);

        let about_body = gtk::Label::new(Some(""));
        about_body.add_css_class("dim-label");
        about_body.add_css_class("caption");
        about_body.set_wrap(true);
        about_body.set_halign(gtk::Align::Start);
        about_body.set_xalign(0.0);
        about_body.set_max_width_chars(20);

        content.append(&about_heading);
        content.append(&about_body);

        // The actual width fix: `widget` is this `ScrolledWindow`, and
        // `content` above is its child, not the public widget itself. See
        // the module doc comment for why capping the `Box`/`Label`s alone
        // wasn't enough. `min_content_width`/`max_content_width` set to
        // the same value hard-locks the width; `propagate_natural_width`
        // (default `false`, set explicitly here so this doesn't silently
        // break if that default ever changes) is what stops `content`'s
        // natural width from leaking through to `widget`'s own size
        // request. `hscrollbar_policy(Never)` means no horizontal
        // scrollbar ever appears — content that's still too wide for
        // `ART_SIZE`/the label caps gets clipped at the edge rather than
        // scrollable, which is fine here since the labels already
        // ellipsize.
        let widget = gtk::ScrolledWindow::new();
        widget.add_css_class("sidebar");
        widget.set_min_content_width(PANEL_WIDTH);
        widget.set_max_content_width(PANEL_WIDTH);
        widget.set_propagate_natural_width(false);
        widget.set_hscrollbar_policy(gtk::PolicyType::Never);
        widget.set_vscrollbar_policy(gtk::PolicyType::Automatic);
        widget.set_hexpand(false);
        widget.set_vexpand(true);
        widget.set_child(Some(&content));

        Self {
            widget,
            title_label,
            artist_label,
            thumbnail,
            about_body,
            cookies_path,
            current_artist: RefCell::new(String::new()),
        }
    }

    /// Redraws from a fresh `PlayerState` — called by `window.rs` right
    /// alongside `PlayerBar::update`, same `state_rx` loop.
    pub fn update(&self, state: &PlayerState) {
        let title = if state.title.is_empty() {
            "Nothing playing"
        } else {
            state.title.as_str()
        };
        self.title_label.set_label(title);
        self.artist_label.set_label(&state.artist);
        self.thumbnail.update(&state.thumbnail_url, ART_SIZE);

        // Fetch artist bio if artist changed.
        let new_browse_id = state.artist_browse_id.clone();
        let artist_name = state.artist.clone();
        let mut current = self.current_artist.borrow_mut();
        if *current == artist_name {
            return; // same artist, no need to re-fetch
        }
        *current = artist_name.clone();
        drop(current);

        // Show loading state immediately.
        self.about_body.set_label("Loading bio...");

        // Spawn a background fetch. The `current_browse_id` cell is used
        // as a generation counter: if the track changes while we're
        // fetching, the stale closure will see a mismatch and bail out.
        let about_body = self.about_body.clone();
        let cell = self.current_artist.clone();
        let cookies_path = self.cookies_path.clone();

        let (sender, receiver) = async_channel::bounded::<(Option<String>, String)>(1);
        let fetch_cookies = cookies_path.clone();
        let artist_for_thread = artist_name.clone();
        std::thread::spawn(move || {
            // If no browse ID, search by artist name to find one.
            let browse_id = new_browse_id.or_else(|| {
                let cookie_header = match crate::user::read_and_validate_cookies(&fetch_cookies) {
                    Ok(h) => h,
                    Err(_) => return None,
                };
                let html = match ureq::get("https://music.youtube.com")
                    .set("Cookie", &cookie_header)
                    .set("User-Agent", crate::innertube::USER_AGENT)
                    .timeout(std::time::Duration::from_secs(15))
                    .call()
                    .and_then(|r| r.into_string().map_err(|e| e.into()))
                {
                    Ok(h) => h,
                    Err(_) => return None,
                };
                let api_key = crate::user::extract_innertube_api_key(&html)?;
                crate::innertube::search_artist_browse_id(
                    &cookie_header,
                    &api_key,
                    &artist_for_thread,
                )
            });

            let Some(browse_id) = browse_id else {
                let _ = sender.send_blocking((None, String::new()));
                return;
            };

            let description =
                crate::detail_fetch::fetch_artist_description(&fetch_cookies, &browse_id);
            let _ = sender.send_blocking((Some(browse_id), description));
        });

        glib::spawn_future_local(async move {
            let Ok((_browse_id, description)) = receiver.recv().await else {
                return;
            };
            // Only update if we're still on the same artist.
            if *cell.borrow() == artist_name {
                if description.is_empty() {
                    about_body.set_label("");
                } else {
                    about_body.set_label(&description);
                }
            }
        });
    }
}
