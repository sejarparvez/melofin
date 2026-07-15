use crate::detail_fetch::DetailMetadata;
use crate::search::Track;
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use std::rc::Rc;

/// A scrollable view showing full metadata and track list for a
/// playlist, album, or artist.
pub struct DetailView {
    pub widget: gtk::ScrolledWindow,
}

impl DetailView {
    /// Builds a fully-loaded detail view from fetched metadata and tracks.
    pub fn new(
        metadata: &DetailMetadata,
        tracks: &[Track],
        on_play_track: Rc<dyn Fn(Track)>,
        on_back: Rc<dyn Fn()>,
    ) -> Self {
        eprintln!(
            "[DetailView::new] title={}, artist={}, tracks={}",
            metadata.title,
            metadata.artist,
            tracks.len()
        );
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(20);
        content.set_margin_bottom(24);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.set_vexpand(true);

        // --- Back button ---
        let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
        back_button.add_css_class("flat");
        back_button.set_halign(gtk::Align::Start);
        {
            let on_back = on_back.clone();
            back_button.connect_clicked(move |_| on_back());
        }
        content.append(&back_button);

        // --- Header: thumbnail + metadata ---
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        header.set_valign(gtk::Align::Start);

        // Thumbnail
        let thumb_frame = gtk::Frame::new(None);
        thumb_frame.add_css_class("card");
        thumb_frame.add_css_class("home-art");
        thumb_frame.set_size_request(200, 200);

        let thumb_icon = gtk::Image::from_icon_name("emblem-music-symbolic");
        thumb_icon.set_pixel_size(48);
        thumb_icon.set_halign(gtk::Align::Center);
        thumb_icon.set_valign(gtk::Align::Center);
        thumb_frame.set_child(Some(&thumb_icon));

        if !metadata.thumbnail_url.is_empty() {
            let picture = gtk::Picture::new();
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_size_request(200, 200);
            thumb_frame.set_child(Some(&picture));
            let url = metadata.thumbnail_url.clone();
            thumbnail_widget::spawn_fetch(url, 200, move |tex| {
                picture.set_paintable(Some(&tex));
            });
        }

        // Text column
        let text_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        text_col.set_hexpand(true);
        text_col.set_valign(gtk::Align::Center);

        let title_label = gtk::Label::new(Some(&metadata.title));
        title_label.add_css_class("title-1");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_wrap(true);

        let artist_label = gtk::Label::new(Some(&metadata.artist));
        artist_label.add_css_class("heading");
        artist_label.add_css_class("dim-label");
        artist_label.set_halign(gtk::Align::Start);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

        // Year + track count line
        let meta_text = match (metadata.year.is_empty(), metadata.track_count > 0) {
            (false, true) => format!("{} · {} tracks", metadata.year, metadata.track_count),
            (false, false) => metadata.year.clone(),
            (true, true) => format!("{} tracks", metadata.track_count),
            (true, false) => String::new(),
        };
        let meta_label = gtk::Label::new(Some(&meta_text));
        meta_label.add_css_class("caption");
        meta_label.add_css_class("dim-label");
        meta_label.set_halign(gtk::Align::Start);

        text_col.append(&title_label);
        text_col.append(&artist_label);
        if !meta_text.is_empty() {
            text_col.append(&meta_label);
        }

        // Description
        if !metadata.description.is_empty() {
            let desc_label = gtk::Label::new(Some(&metadata.description));
            desc_label.add_css_class("caption");
            desc_label.add_css_class("dim-label");
            desc_label.set_halign(gtk::Align::Start);
            desc_label.set_xalign(0.0);
            desc_label.set_wrap(true);
            desc_label.set_max_width_chars(60);
            desc_label.set_margin_top(8);
            text_col.append(&desc_label);
        }

        // Play All button
        if !tracks.is_empty() {
            let play_all = gtk::Button::with_label("Play All");
            play_all.add_css_class("pill");
            play_all.add_css_class("suggested-action");
            play_all.set_halign(gtk::Align::Start);
            play_all.set_margin_top(8);
            let first_track = tracks[0].clone();
            let on_play_track = on_play_track.clone();
            play_all.connect_clicked(move |_| {
                on_play_track(first_track.clone());
            });
            text_col.append(&play_all);
        }

        header.append(&thumb_frame);
        header.append(&text_col);
        content.append(&header);

        // --- Separator ---
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // --- Track list ---
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_margin_start(12);
        list.set_margin_end(12);

        for track in tracks {
            list.append(&thumbnail_widget::build_track_row(track));
        }

        // Wire row activation to play.
        {
            let tracks = tracks.to_vec();
            let on_play_track = on_play_track.clone();
            list.connect_row_activated(move |_list, row| {
                let index = row.index() as usize;
                if let Some(track) = tracks.get(index) {
                    on_play_track(track.clone());
                }
            });
        }

        content.append(&list);

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_hexpand(true);
        widget.set_hscrollbar_policy(gtk::PolicyType::Never);
        widget.set_can_focus(false);
        widget.set_child(Some(&content));

        Self { widget }
    }

    /// Builds a loading state view (spinner + message).
    pub fn loading() -> Self {
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
        box_.set_valign(gtk::Align::Center);
        box_.set_halign(gtk::Align::Center);
        box_.set_margin_top(80);

        let spinner = gtk::Spinner::new();
        spinner.set_spinning(true);
        spinner.set_size_request(32, 32);

        let label = gtk::Label::new(Some("Loading details…"));
        label.add_css_class("dim-label");

        box_.append(&spinner);
        box_.append(&label);

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_child(Some(&box_));
        Self { widget }
    }

    /// Builds an error state view with a retry button.
    pub fn error(message: &str, on_retry: Rc<dyn Fn()>) -> Self {
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
        box_.set_valign(gtk::Align::Center);
        box_.set_halign(gtk::Align::Center);
        box_.set_margin_top(80);

        let label = gtk::Label::new(Some(message));
        label.add_css_class("dim-label");
        label.set_wrap(true);
        label.set_justify(gtk::Justification::Center);

        let retry = gtk::Button::with_label("Retry");
        retry.add_css_class("pill");
        retry.set_halign(gtk::Align::Center);
        retry.connect_clicked(move |_| on_retry());

        box_.append(&label);
        box_.append(&retry);

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_child(Some(&box_));
        Self { widget }
    }
}
