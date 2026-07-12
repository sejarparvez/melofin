//! Right "Now Playing" panel: album art + title/artist reflect real
//! playback state, updated the same way as `player_bar.rs::update` — this
//! is the one sidebar element that isn't placeholder data, since it's just
//! showing what `player.rs` already tracks. The "About the artist" blurb
//! below it stays static text: there's no bio/metadata source wired up
//! (only `search.rs`'s title/artist/thumbnail via yt-dlp exists today).

use crate::player::PlayerState;
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use std::cell::RefCell;

pub struct NowPlayingPanel {
    pub widget: gtk::Box,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    art_stack: gtk::Stack,
    art_picture: gtk::Picture,
    current_thumbnail_url: RefCell<String>,
}

impl Default for NowPlayingPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl NowPlayingPanel {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 12);
        widget.add_css_class("sidebar");
        widget.set_size_request(260, -1);
        widget.set_margin_top(10);
        widget.set_margin_bottom(10);
        widget.set_margin_start(8);
        widget.set_margin_end(8);

        let heading = gtk::Label::new(Some("Now Playing"));
        heading.add_css_class("heading");
        heading.set_halign(gtk::Align::Start);
        widget.append(&heading);

        // Same placeholder/art `Stack` swap pattern as `player_bar.rs`'s
        // album art tile, just bigger.
        let art_frame = gtk::Frame::new(None);
        art_frame.add_css_class("card");
        art_frame.add_css_class("home-art");
        art_frame.set_size_request(228, 228);

        let art_stack = gtk::Stack::new();
        let placeholder_icon = gtk::Image::from_icon_name("emblem-music-symbolic");
        placeholder_icon.set_pixel_size(48);
        placeholder_icon.set_halign(gtk::Align::Center);
        placeholder_icon.set_valign(gtk::Align::Center);
        art_stack.add_named(&placeholder_icon, Some("placeholder"));

        let art_picture = gtk::Picture::new();
        art_picture.set_content_fit(gtk::ContentFit::Cover);
        art_stack.add_named(&art_picture, Some("art"));
        art_stack.set_visible_child_name("placeholder");
        art_frame.set_child(Some(&art_stack));
        widget.append(&art_frame);

        let title_label = gtk::Label::new(Some("Nothing playing"));
        title_label.add_css_class("title-2");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let artist_label = gtk::Label::new(Some(""));
        artist_label.add_css_class("dim-label");
        artist_label.set_halign(gtk::Align::Start);

        widget.append(&title_label);
        widget.append(&artist_label);

        let about_heading = gtk::Label::new(Some("About the artist"));
        about_heading.add_css_class("heading");
        about_heading.set_halign(gtk::Align::Start);
        about_heading.set_margin_top(12);

        let about_body = gtk::Label::new(Some(
            "Artist bios aren't wired up yet — this panel will show real \
             info once a metadata source is connected.",
        ));
        about_body.add_css_class("dim-label");
        about_body.add_css_class("caption");
        about_body.set_wrap(true);
        about_body.set_halign(gtk::Align::Start);
        about_body.set_xalign(0.0);

        widget.append(&about_heading);
        widget.append(&about_body);

        Self {
            widget,
            title_label,
            artist_label,
            art_stack,
            art_picture,
            current_thumbnail_url: RefCell::new(String::new()),
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

        if *self.current_thumbnail_url.borrow() != state.thumbnail_url {
            *self.current_thumbnail_url.borrow_mut() = state.thumbnail_url.clone();
            if state.thumbnail_url.is_empty() {
                self.art_stack.set_visible_child_name("placeholder");
            } else {
                let art_stack = self.art_stack.clone();
                let art_picture = self.art_picture.clone();
                thumbnail_widget::spawn_fetch(state.thumbnail_url.clone(), 228, move |texture| {
                    art_picture.set_paintable(Some(&texture));
                    art_stack.set_visible_child_name("art");
                });
            }
        }
    }
}
