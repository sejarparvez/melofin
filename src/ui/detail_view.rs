use crate::detail_fetch::DetailMetadata;
use crate::search::Track;
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use std::rc::Rc;

/// An album in the discography section.
#[derive(Clone)]
pub struct DiscographyAlbum {
    pub title: String,
    pub year: String,
    pub album_type: String,
    pub thumbnail_url: String,
    pub browse_id: String,
}

/// A related artist card.
#[derive(Clone)]
pub struct RelatedArtist {
    pub name: String,
    pub thumbnail_url: String,
    pub browse_id: String,
}

pub struct DetailView {
    pub widget: gtk::ScrolledWindow,
}

impl DetailView {
    /// Builds a fully-loaded detail view from fetched metadata and tracks.
    pub fn new(
        metadata: &DetailMetadata,
        tracks: &[Track],
        on_play_from_list: Rc<dyn Fn(Vec<Track>, usize)>,
        _on_back: Rc<dyn Fn()>,
        discography: &[DiscographyAlbum],
        related_artists: &[RelatedArtist],
    ) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_vexpand(true);

        // --- Hero Section ---
        let hero = build_hero_section(metadata, tracks, on_play_from_list.clone());
        content.append(&hero);

        // --- Content Section (Two Columns) ---
        let content_row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        content_row.set_margin_start(32);
        content_row.set_margin_end(32);
        content_row.set_margin_top(24);
        content_row.set_margin_bottom(32);

        // Left column: Top Tracks
        let left_col = build_top_tracks_section(tracks, on_play_from_list);
        left_col.set_hexpand(true);

        // Right column: About
        let right_col = build_about_section(metadata);
        right_col.set_size_request(280, -1);

        content_row.append(&left_col);
        content_row.append(&right_col);

        content.append(&content_row);

        // --- Discography Section ---
        if !discography.is_empty() {
            let disco = build_discography_section(discography);
            content.append(&disco);
        }

        // --- Related Artists Section ---
        if !related_artists.is_empty() {
            let related = build_related_artists_section(related_artists);
            content.append(&related);
        }

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

        let label = gtk::Label::new(Some("Loading details\u{2026}"));
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

/// Build the hero section with artist info and action buttons.
fn build_hero_section(
    metadata: &DetailMetadata,
    tracks: &[Track],
    on_play_from_list: Rc<dyn Fn(Vec<Track>, usize)>,
) -> gtk::Widget {
    let hero = gtk::Box::new(gtk::Orientation::Vertical, 8);
    hero.add_css_class("artist-hero");

    // Content
    let hero_content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    hero_content.add_css_class("artist-hero-content");

    // Verified badge
    if metadata.is_verified {
        let badge = gtk::Label::new(Some("\u{2713} VERIFIED ARTIST"));
        badge.add_css_class("verified-badge");
        badge.set_halign(gtk::Align::Start);
        hero_content.append(&badge);
    }

    // Artist name
    let name_label = gtk::Label::new(Some(&metadata.title));
    name_label.add_css_class("artist-name");
    name_label.set_halign(gtk::Align::Start);
    name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    hero_content.append(&name_label);

    // Monthly listeners
    if let Some(listeners) = &metadata.monthly_listeners {
        let listeners_label = gtk::Label::new(Some(&format!("{} MONTHLY LISTENERS", listeners)));
        listeners_label.add_css_class("monthly-listeners");
        listeners_label.set_halign(gtk::Align::Start);
        hero_content.append(&listeners_label);
    }

    // Bio text
    if !metadata.description.is_empty() {
        let bio_label = gtk::Label::new(Some(&metadata.description));
        bio_label.add_css_class("artist-bio");
        bio_label.set_halign(gtk::Align::Start);
        bio_label.set_xalign(0.0);
        bio_label.set_wrap(true);
        bio_label.set_max_width_chars(80);
        hero_content.append(&bio_label);
    }

    // Action buttons
    let buttons_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    buttons_row.set_halign(gtk::Align::Start);
    buttons_row.set_margin_top(16);

    // Play Artist button
    if !tracks.is_empty() {
        let play_btn = gtk::Button::with_label("\u{25b6} Play Artist");
        play_btn.add_css_class("play-artist-btn");
        let all_tracks = tracks.to_vec();
        let on_play_from_list = on_play_from_list.clone();
        play_btn.connect_clicked(move |_| {
            on_play_from_list(all_tracks.clone(), 0);
        });
        buttons_row.append(&play_btn);
    }

    // Follow button
    let follow_btn = gtk::Button::with_label("Follow");
    follow_btn.add_css_class("follow-btn");
    buttons_row.append(&follow_btn);

    // More button
    let more_btn = gtk::Button::from_icon_name("view-more-symbolic");
    more_btn.add_css_class("more-btn");
    buttons_row.append(&more_btn);

    hero_content.append(&buttons_row);

    hero.append(&hero_content);
    hero.upcast()
}

/// Build the Top Tracks section with table-style layout.
fn build_top_tracks_section(
    tracks: &[Track],
    on_play_from_list: Rc<dyn Fn(Vec<Track>, usize)>,
) -> gtk::Box {
    let left_col = gtk::Box::new(gtk::Orientation::Vertical, 16);

    // Section header
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    let title = gtk::Label::new(Some("Top Tracks"));
    title.add_css_class("section-title");
    title.set_halign(gtk::Align::Start);
    title.set_hexpand(true);

    let show_all = gtk::Button::with_label("Show All");
    show_all.add_css_class("show-all-link");
    show_all.set_halign(gtk::Align::End);

    header.append(&title);
    header.append(&show_all);
    left_col.append(&header);

    // Track table
    let table = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Table header
    let table_header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    table_header.add_css_class("track-table-header");

    let hash_label = gtk::Label::new(Some("#"));
    hash_label.set_width_chars(4);
    hash_label.set_halign(gtk::Align::Start);

    let title_label = gtk::Label::new(Some("TITLE"));
    title_label.set_hexpand(true);
    title_label.set_halign(gtk::Align::Start);

    let plays_label = gtk::Label::new(Some("PLAYS"));
    plays_label.set_width_chars(12);
    plays_label.set_halign(gtk::Align::End);

    let duration_label = gtk::Label::new(Some("\u{23f1}"));
    duration_label.set_width_chars(6);
    duration_label.set_halign(gtk::Align::End);

    table_header.append(&hash_label);
    table_header.append(&title_label);
    table_header.append(&plays_label);
    table_header.append(&duration_label);
    table.append(&table_header);

    // Track rows
    for (i, track) in tracks.iter().enumerate() {
        let row = build_track_row(track, i + 1);
        let all_tracks = tracks.to_vec();
        let on_play_from_list = on_play_from_list.clone();
        row.connect_clicked(move |_| {
            let index = i;
            if index < all_tracks.len() {
                on_play_from_list(all_tracks.clone(), index);
            }
        });
        table.append(&row);
    }

    left_col.append(&table);
    left_col
}

/// Build a single track row for the table.
fn build_track_row(track: &Track, number: usize) -> gtk::Button {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("track-row");

    // Track number
    let num_label = gtk::Label::new(Some(&number.to_string()));
    num_label.add_css_class("track-number");
    num_label.set_width_chars(4);
    num_label.set_halign(gtk::Align::Start);

    // Album art
    let art_frame = gtk::Frame::new(None);
    art_frame.add_css_class("track-art");
    art_frame.set_size_request(40, 40);

    let art_icon = gtk::Image::from_icon_name("emblem-music-symbolic");
    art_icon.set_pixel_size(16);
    art_icon.set_halign(gtk::Align::Center);
    art_icon.set_valign(gtk::Align::Center);
    art_frame.set_child(Some(&art_icon));

    if !track.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(40, 40);
        art_frame.set_child(Some(&picture));
        let url = track.thumbnail_url.clone();
        thumbnail_widget::spawn_fetch(url, 40, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    // Text info
    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_box.set_valign(gtk::Align::Center);
    text_box.set_hexpand(true);

    let title_label = gtk::Label::new(Some(&track.title));
    title_label.add_css_class("track-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(30);

    let album_label = gtk::Label::new(Some(&track.artist));
    album_label.add_css_class("track-album");
    album_label.set_halign(gtk::Align::Start);
    album_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    album_label.set_max_width_chars(30);

    text_box.append(&title_label);
    text_box.append(&album_label);

    // Play count (placeholder)
    let plays_label = gtk::Label::new(Some("--"));
    plays_label.add_css_class("track-plays");
    plays_label.set_width_chars(12);
    plays_label.set_halign(gtk::Align::End);

    // Duration
    let duration_label = gtk::Label::new(Some(
        track.duration.as_deref().unwrap_or("0:00"),
    ));
    duration_label.add_css_class("track-duration");
    duration_label.set_width_chars(6);
    duration_label.set_halign(gtk::Align::End);

    row.append(&num_label);
    row.append(&art_frame);
    row.append(&text_box);
    row.append(&plays_label);
    row.append(&duration_label);

    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&row));
    button
}

/// Build the About section.
fn build_about_section(metadata: &DetailMetadata) -> gtk::Box {
    let right_col = gtk::Box::new(gtk::Orientation::Vertical, 16);

    // About card
    let card = gtk::Box::new(gtk::Orientation::Vertical, 16);
    card.add_css_class("about-card");

    let title = gtk::Label::new(Some("About"));
    title.add_css_class("about-title");
    title.set_halign(gtk::Align::Start);
    card.append(&title);

    // Artist image (if available)
    if !metadata.thumbnail_url.is_empty() {
        let image_frame = gtk::Frame::new(None);
        image_frame.add_css_class("about-image");
        image_frame.set_size_request(248, 248);

        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(248, 248);
        image_frame.set_child(Some(&picture));

        let url = metadata.thumbnail_url.clone();
        thumbnail_widget::spawn_fetch(url, 248, move |tex| {
            picture.set_paintable(Some(&tex));
        });

        card.append(&image_frame);
    }

    // Bio text
    if !metadata.description.is_empty() {
        let bio_label = gtk::Label::new(Some(&metadata.description));
        bio_label.add_css_class("about-text");
        bio_label.set_halign(gtk::Align::Start);
        bio_label.set_xalign(0.0);
        bio_label.set_wrap(true);
        card.append(&bio_label);
    }

    right_col.append(&card);
    right_col
}

/// Build the Discography section with horizontally-scrolling album cards.
fn build_discography_section(albums: &[DiscographyAlbum]) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);
    section.add_css_class("discography-section");

    // Header with filter tabs
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    header.set_hexpand(true);

    let title = gtk::Label::new(Some("Discography"));
    title.add_css_class("section-title");
    title.set_halign(gtk::Align::Start);
    title.set_hexpand(true);

    let filters = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    filters.add_css_class("discography-filters");

    let albums_btn = gtk::Button::with_label("Albums");
    albums_btn.add_css_class("discography-filter-btn");
    albums_btn.add_css_class("active");

    let singles_btn = gtk::Button::with_label("Singles & EPs");
    singles_btn.add_css_class("discography-filter-btn");

    filters.append(&albums_btn);
    filters.append(&singles_btn);

    header.append(&title);
    header.append(&filters);
    section.append(&header);

    // Horizontal scroll of album cards
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    for album in albums {
        row.append(&discography_card(album));
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_hscrollbar_policy(gtk::PolicyType::External);
    scroller.set_child(Some(&row));
    section.append(&scroller);

    section
}

/// A single album card in the discography section.
fn discography_card(album: &DiscographyAlbum) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("discography-card");
    card.set_size_request(180, -1);

    // Album art with play button overlay
    let art_overlay = gtk::Overlay::new();
    art_overlay.set_size_request(180, 180);

    let art_frame = gtk::Frame::new(None);
    art_frame.add_css_class("discography-art");
    art_frame.set_size_request(180, 180);

    if !album.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(180, 180);
        art_frame.set_child(Some(&picture));
        let url = album.thumbnail_url.clone();
        thumbnail_widget::spawn_fetch(url, 180, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    art_overlay.set_child(Some(&art_frame));

    let play_btn = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_btn.add_css_class("discography-art-play");
    play_btn.set_halign(gtk::Align::End);
    play_btn.set_valign(gtk::Align::End);
    art_overlay.add_overlay(&play_btn);

    // Title
    let title_label = gtk::Label::new(Some(&album.title));
    title_label.add_css_class("discography-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(22);

    // Year • Type
    let meta_text = if album.year.is_empty() {
        album.album_type.clone()
    } else {
        format!("{} • {}", album.year, album.album_type)
    };
    let meta_label = gtk::Label::new(Some(&meta_text));
    meta_label.add_css_class("discography-meta");
    meta_label.set_halign(gtk::Align::Start);

    card.append(&art_overlay);
    card.append(&title_label);
    card.append(&meta_label);

    card.upcast()
}

/// Build the Related Artists section with horizontally-scrolling circular avatars.
fn build_related_artists_section(artists: &[RelatedArtist]) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);
    section.add_css_class("related-artists-section");

    // Header
    let header = gtk::Label::new(Some("Related Artists"));
    header.add_css_class("section-title");
    header.set_halign(gtk::Align::Start);
    header.add_css_class("related-artists-header");
    section.append(&header);

    // Horizontal scroll of artist cards
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    for artist in artists {
        row.append(&related_artist_card(artist));
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_hscrollbar_policy(gtk::PolicyType::External);
    scroller.set_child(Some(&row));
    section.append(&scroller);

    section
}

/// A single related artist card with circular avatar.
fn related_artist_card(artist: &RelatedArtist) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("related-artist-card");
    card.set_size_request(140, -1);
    card.set_halign(gtk::Align::Start);

    // Circular avatar
    let avatar_frame = gtk::Frame::new(None);
    avatar_frame.add_css_class("related-artist-avatar");
    avatar_frame.set_size_request(120, 120);

    if !artist.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(120, 120);
        avatar_frame.set_child(Some(&picture));
        let url = artist.thumbnail_url.clone();
        thumbnail_widget::spawn_fetch(url, 120, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    let name_label = gtk::Label::new(Some(&artist.name));
    name_label.add_css_class("related-artist-name");
    name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name_label.set_max_width_chars(18);

    card.append(&avatar_frame);
    card.append(&name_label);

    card.upcast()
}
