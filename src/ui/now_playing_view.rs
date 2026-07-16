//! Full-screen Now Playing view, shown when clicking the player bar.
//! Matches the Stitch "Now Playing View" design: large album art on the
//! left, synchronized lyrics placeholder on the right, with a back button
//! to return to the previous view.

use crate::player::PlayerState;
use crate::ui::thumbnail_widget::ThumbnailStack;
use adw::prelude::*;
use std::rc::Rc;

const ART_SIZE: i32 = 450;

pub struct NowPlayingView {
    pub widget: gtk::ScrolledWindow,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    thumbnail: ThumbnailStack,
    _back_button: gtk::Button,
}

impl NowPlayingView {
    pub fn new(on_back: Rc<dyn Fn()>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_hexpand(true);
        content.set_vexpand(true);

        // -- Header -----------------------------------------------------------
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_margin_top(12);
        header.set_margin_bottom(12);
        header.set_margin_start(32);
        header.set_margin_end(32);
        header.set_valign(gtk::Align::Center);

        let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
        back_button.add_css_class("flat");
        back_button.add_css_class("circular");
        back_button.set_tooltip_text(Some("Back"));
        {
            let on_back = on_back.clone();
            back_button.connect_clicked(move |_| on_back());
        }

        let header_info = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let now_playing_label = gtk::Label::new(Some("NOW PLAYING"));
        now_playing_label.add_css_class("caption");
        now_playing_label.add_css_class("dim-label");
        now_playing_label.set_halign(gtk::Align::Start);
        let track_info_label = gtk::Label::new(Some("Nothing playing"));
        track_info_label.add_css_class("track-title");
        track_info_label.set_halign(gtk::Align::Start);
        track_info_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        header_info.append(&now_playing_label);
        header_info.append(&track_info_label);

        header.append(&back_button);
        header.append(&header_info);

        content.append(&header);

        // -- Two-column body --------------------------------------------------
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 32);
        body.set_margin_start(32);
        body.set_margin_end(32);
        body.set_margin_bottom(32);
        body.set_hexpand(true);
        body.set_vexpand(true);
        body.set_valign(gtk::Align::Center);

        // Left: album art + track info
        let left_col = gtk::Box::new(gtk::Orientation::Vertical, 16);
        left_col.set_valign(gtk::Align::Center);

        let thumbnail = ThumbnailStack::new("emblem-music-symbolic", 64, ART_SIZE);
        let art_frame = gtk::Frame::new(None);
        art_frame.add_css_class("now-playing-art");
        art_frame.set_size_request(ART_SIZE, ART_SIZE);
        art_frame.set_hexpand(false);
        art_frame.set_child(Some(thumbnail.widget()));

        let title_label = gtk::Label::new(Some("Nothing playing"));
        title_label.add_css_class("now-playing-title");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(40);

        let artist_label = gtk::Label::new(Some(""));
        artist_label.add_css_class("now-playing-artist");
        artist_label.set_halign(gtk::Align::Start);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.set_max_width_chars(40);

        left_col.append(&art_frame);
        left_col.append(&title_label);
        left_col.append(&artist_label);

        // Right: lyrics placeholder
        let right_col = gtk::Box::new(gtk::Orientation::Vertical, 0);
        right_col.set_hexpand(true);
        right_col.set_valign(gtk::Align::Center);

        let lyrics_heading = gtk::Label::new(Some("LYRICS"));
        lyrics_heading.add_css_class("caption");
        lyrics_heading.add_css_class("dim-label");
        lyrics_heading.set_halign(gtk::Align::Start);
        lyrics_heading.set_margin_bottom(16);

        let lyrics_placeholder = gtk::Label::new(Some("No lyrics available"));
        lyrics_placeholder.add_css_class("dim-label");
        lyrics_placeholder.set_halign(gtk::Align::Start);
        lyrics_placeholder.set_valign(gtk::Align::Start);
        lyrics_placeholder.set_vexpand(true);

        right_col.append(&lyrics_heading);
        right_col.append(&lyrics_placeholder);

        body.append(&left_col);
        body.append(&right_col);
        content.append(&body);

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_hscrollbar_policy(gtk::PolicyType::Never);
        widget.set_child(Some(&content));

        Self {
            widget,
            title_label,
            artist_label,
            thumbnail,
            _back_button: back_button,
        }
    }

    pub fn update(&self, state: &PlayerState) {
        let title = if state.title.is_empty() {
            "Nothing playing"
        } else {
            state.title.as_str()
        };
        self.title_label.set_label(title);
        self.artist_label.set_label(&state.artist);
        self.thumbnail.update(&state.thumbnail_url, ART_SIZE);
    }
}
