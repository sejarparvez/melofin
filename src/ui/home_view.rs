//! The home page: shown by default until the user searches (see
//! `window.rs`, which swaps a `gtk::Stack` between this and
//! `search_view`). Everything here is placeholder data until a real
//! home-feed API exists (nothing in `search.rs`/yt-dlp currently provides
//! one) — this is UI scaffolding, same spirit as the disabled buttons in
//! `top_bar.rs`/`library_sidebar.rs`.
//!
//! Cards reuse `search::Track` rather than inventing a parallel type, so
//! swapping placeholder data for a real feed later is just a different
//! `Vec<Track>` feeding the same `build_row`/`track_card` functions.

use crate::search::Track;
use adw::prelude::*;

pub struct HomeView {
    pub widget: gtk::ScrolledWindow,
}

impl HomeView {
    /// `on_select` fires when the user clicks a card (or the hero card's
    /// play button), with the corresponding `Track` — wired the same way
    /// as `search_view::SearchView::new`'s `on_select`.
    pub fn new(on_select: impl Fn(Track) + 'static + Clone) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
        content.set_margin_top(20);
        content.set_margin_bottom(24);
        content.set_margin_start(20);
        content.set_margin_end(20);

        content.append(&build_filter_pills());
        content.append(&build_shortcuts_grid());
        content.append(&build_hero_card(on_select.clone()));
        content.append(&build_row(
            "Made for you",
            made_for_you(),
            on_select.clone(),
        ));
        content.append(&build_row("Recently played", recently_played(), on_select));

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_hscrollbar_policy(gtk::PolicyType::Never);
        widget.set_child(Some(&content));

        Self { widget }
    }
}

/// "All / Music" filter row — cosmetic for now (there's only one feed to
/// filter), matching Spotify's home filter chips so the layout already has
/// somewhere to grow into (podcasts, etc.) later.
fn build_filter_pills() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);

    let all = gtk::ToggleButton::with_label("All");
    all.add_css_class("pill");
    all.set_active(true);

    let music = gtk::ToggleButton::with_label("Music");
    music.add_css_class("pill");
    music.set_group(Some(&all));

    row.append(&all);
    row.append(&music);
    row
}

struct Shortcut {
    title: &'static str,
    icon: &'static str,
}

fn shortcuts() -> Vec<Shortcut> {
    vec![
        Shortcut {
            title: "Liked Songs",
            icon: "starred-symbolic",
        },
        Shortcut {
            title: "Recently Added",
            icon: "document-open-recent-symbolic",
        },
        Shortcut {
            title: "Top Tracks",
            icon: "star-new-symbolic",
        },
        Shortcut {
            title: "Discover Weekly",
            icon: "media-playlist-shuffle-symbolic",
        },
    ]
}

/// A 2-wide grid of compact horizontal tiles ("Liked Songs", etc.) — same
/// not-yet-wired pattern as `library_sidebar.rs` rows: disabled with a
/// tooltip rather than silently doing nothing, since none of these map to
/// a real view yet.
fn build_shortcuts_grid() -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_max_children_per_line(2);
    flow.set_min_children_per_line(1);
    flow.set_row_spacing(10);
    flow.set_column_spacing(10);
    flow.set_homogeneous(true);
    for shortcut in shortcuts() {
        flow.insert(&shortcut_tile(&shortcut), -1);
    }
    flow
}

fn shortcut_tile(shortcut: &Shortcut) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("card");

    let icon_frame = gtk::Frame::new(None);
    icon_frame.add_css_class("home-art");
    icon_frame.set_size_request(48, 48);
    let icon = gtk::Image::from_icon_name(shortcut.icon);
    icon.set_pixel_size(20);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon_frame.set_child(Some(&icon));

    let label = gtk::Label::new(Some(shortcut.title));
    label.add_css_class("heading");
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    label.set_margin_start(10);
    label.set_margin_end(10);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    row.append(&icon_frame);
    row.append(&label);

    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&row));
    button.set_sensitive(false);
    button.set_tooltip_text(Some("coming soon — no library backend yet"));
    button.upcast()
}

/// The single big "Picked for you" card. Its play button uses a real
/// placeholder `Track` (empty `url`), so clicking it is a safe no-op —
/// `window.rs`'s shared `play_track` closure already ignores those — same
/// as every other placeholder card on this page.
fn build_hero_card(on_select: impl Fn(Track) + 'static) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    card.add_css_class("card");
    card.add_css_class("hero-card");

    let art = gtk::Frame::new(None);
    art.add_css_class("home-art");
    art.set_size_request(96, 96);
    let icon = gtk::Image::from_icon_name("emblem-music-symbolic");
    icon.set_pixel_size(32);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    art.set_child(Some(&icon));

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text_box.set_valign(gtk::Align::Center);
    text_box.set_hexpand(true);

    let eyebrow = gtk::Label::new(Some("Picked for you"));
    eyebrow.add_css_class("caption");
    eyebrow.add_css_class("dim-label");
    eyebrow.set_halign(gtk::Align::Start);

    let title = gtk::Label::new(Some("Late Night Mix"));
    title.add_css_class("title-2");
    title.set_halign(gtk::Align::Start);

    let subtitle = gtk::Label::new(Some("Top pick from your recent listens"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);

    text_box.append(&eyebrow);
    text_box.append(&title);
    text_box.append(&subtitle);

    let play_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_button.add_css_class("circular");
    play_button.add_css_class("suggested-action");
    play_button.set_valign(gtk::Align::Center);
    play_button.connect_clicked(move |_| {
        on_select(placeholder_track("Late Night Mix", "Melofin picks"));
    });

    card.append(&art);
    card.append(&text_box);
    card.append(&play_button);
    card.upcast()
}

/// A titled, horizontally-scrolling row of cards — same idea as the
/// results list in `search_view.rs`, just laid out sideways.
fn build_row(
    title: &str,
    tracks: Vec<Track>,
    on_select: impl Fn(Track) + 'static + Clone,
) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);

    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("title-2");
    heading.set_halign(gtk::Align::Start);
    section.append(&heading);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    for track in tracks {
        let on_select = on_select.clone();
        row.append(&track_card(track, on_select));
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_hscrollbar_policy(gtk::PolicyType::External);
    scroller.set_child(Some(&row));
    section.append(&scroller);

    section
}

/// A single card: art placeholder (or fetched thumbnail once a track has a
/// `thumbnail_url`) + title + artist, wrapped in a `Button` so the whole
/// card is clickable — matches `search_view.rs` treating a full
/// `ActionRow` as activatable rather than needing a separate play icon.
fn track_card(track: Track, on_select: impl Fn(Track) + 'static) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_width_request(150);

    let art_frame = gtk::Frame::new(None);
    art_frame.add_css_class("card");
    art_frame.add_css_class("home-art");
    art_frame.set_size_request(150, 150);

    let art_icon = gtk::Image::from_icon_name("emblem-music-symbolic");
    art_icon.set_pixel_size(28);
    art_icon.set_halign(gtk::Align::Center);
    art_icon.set_valign(gtk::Align::Center);
    art_frame.set_child(Some(&art_icon));

    if !track.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(150, 150);
        art_frame.set_child(Some(&picture));
        crate::ui::thumbnail_widget::spawn_fetch(track.thumbnail_url.clone(), 150, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    let title_label = gtk::Label::new(Some(&track.title));
    title_label.add_css_class("heading");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(18);

    let artist_label = gtk::Label::new(Some(&track.artist));
    artist_label.add_css_class("dim-label");
    artist_label.add_css_class("caption");
    artist_label.set_halign(gtk::Align::Start);
    artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist_label.set_max_width_chars(18);

    card.append(&art_frame);
    card.append(&title_label);
    card.append(&artist_label);

    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("home-card");
    button.set_child(Some(&card));
    button.connect_clicked(move |_| on_select(track.clone()));

    button.upcast()
}

// ---------------------------------------------------------------------
// Placeholder data — swap for a real home-feed source when one exists.
// Empty `thumbnail_url`/`url` are fine: `url` only matters once a card is
// actually played, and an empty `thumbnail_url` just keeps the icon
// placeholder instead of fetching art.
// ---------------------------------------------------------------------

fn made_for_you() -> Vec<Track> {
    [
        ("Chill Waves", "Lo-fi & chill"),
        ("Focus Flow", "Deep work beats"),
        ("Late Night Drive", "Synthwave vibes"),
        ("Morning Boost", "Upbeat energy"),
        ("Rainy Days", "Soft acoustic"),
    ]
    .into_iter()
    .map(|(title, subtitle)| placeholder_track(title, subtitle))
    .collect()
}

fn recently_played() -> Vec<Track> {
    [
        ("After Hours", "The Weeknd"),
        ("Currents", "Tame Impala"),
        ("Blonde", "Frank Ocean"),
        ("Melodrama", "Lorde"),
        ("Random Access Memories", "Daft Punk"),
    ]
    .into_iter()
    .map(|(title, artist)| placeholder_track(title, artist))
    .collect()
}

fn placeholder_track(title: &str, artist: &str) -> Track {
    Track {
        title: title.to_string(),
        artist: artist.to_string(),
        url: String::new(),
        thumbnail_url: String::new(),
    }
}
