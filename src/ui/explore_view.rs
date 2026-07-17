//! Explore page: trending artists, genre grid with images, and global charts.
//! Matches the Stitch "Explore" design.

use crate::search::Track;
use adw::prelude::*;
use std::rc::Rc;

struct TrendingArtist {
    name: &'static str,
    icon: &'static str,
}

fn trending_artists() -> Vec<TrendingArtist> {
    vec![
        TrendingArtist { name: "The Weeknd", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Taylor Swift", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Drake", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Bad Bunny", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Billie Eilish", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Dua Lipa", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Ed Sheeran", icon: "avatar-default-symbolic" },
        TrendingArtist { name: "Ariana Grande", icon: "avatar-default-symbolic" },
    ]
}

struct GenreInfo {
    name: &'static str,
    icon: &'static str,
    css_class: &'static str,
    is_large: bool,
}

fn genres() -> Vec<GenreInfo> {
    vec![
        GenreInfo { name: "Pop", icon: "media-playlist-shuffle-symbolic", css_class: "genre-pop", is_large: false },
        GenreInfo { name: "Rock", icon: "emblem-rock-symbolic", css_class: "genre-rock", is_large: false },
        GenreInfo { name: "Electronic", icon: "media-equalizer-symbolic", css_class: "genre-electronic", is_large: false },
        GenreInfo { name: "Hip-Hop", icon: "microphone-sensitivity-high-symbolic", css_class: "genre-hiphop", is_large: true },
        GenreInfo { name: "Jazz", icon: "audio-x-generic-symbolic", css_class: "genre-jazz", is_large: false },
        GenreInfo { name: "Classical", icon: "media-playlist-consecutive-symbolic", css_class: "genre-classical", is_large: false },
    ]
}

struct ChartTrack {
    rank: usize,
    title: &'static str,
    artist: &'static str,
    duration: &'static str,
}

fn global_charts() -> Vec<ChartTrack> {
    vec![
        ChartTrack { rank: 1, title: "Blinding Lights", artist: "The Weeknd", duration: "3:22" },
        ChartTrack { rank: 2, title: "Shape of You", artist: "Ed Sheeran", duration: "3:53" },
        ChartTrack { rank: 3, title: "Bohemian Rhapsody", artist: "Queen", duration: "5:55" },
        ChartTrack { rank: 4, title: "Bad Guy", artist: "Billie Eilish", duration: "3:14" },
        ChartTrack { rank: 5, title: "Levitating", artist: "Dua Lipa", duration: "3:23" },
        ChartTrack { rank: 6, title: "Watermelon Sugar", artist: "Harry Styles", duration: "2:54" },
        ChartTrack { rank: 7, title: "Stay", artist: "The Kid LAROI & Justin Bieber", duration: "2:21" },
        ChartTrack { rank: 8, title: "Anti-Hero", artist: "Taylor Swift", duration: "3:20" },
    ]
}

struct FeaturedPlaylist {
    title: &'static str,
    description: &'static str,
}

fn featured_playlists() -> Vec<FeaturedPlaylist> {
    vec![
        FeaturedPlaylist { title: "Discover Weekly", description: "Your personal mixtape of fresh music" },
        FeaturedPlaylist { title: "Release Radar", description: "Catch all the latest music from artists you follow" },
        FeaturedPlaylist { title: "Daily Mix 1", description: "Radiohead, Muse, Coldplay and more" },
        FeaturedPlaylist { title: "Daily Mix 2", description: "Daft Punk, Disclosure, Bonobo and more" },
    ]
}

pub struct ExploreView {
    pub widget: gtk::ScrolledWindow,
}

impl ExploreView {
    pub fn new(_on_select: Rc<dyn Fn(Track)>, _on_play: Rc<dyn Fn(Track)>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 48);
        content.set_margin_top(32);
        content.set_margin_bottom(32);
        content.set_margin_start(32);
        content.set_margin_end(32);

        // -- Page Title --
        let title = gtk::Label::new(Some("Explore"));
        title.add_css_class("explore-title");
        title.set_halign(gtk::Align::Start);
        content.append(&title);

        // -- Trending Artists --
        content.append(&build_trending_artists_section());

        // -- Genre Grid --
        content.append(&build_genre_grid_section());

        // -- Global Charts --
        content.append(&build_global_charts_section());

        // -- Featured Playlists --
        content.append(&build_featured_playlists_section());

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_hscrollbar_policy(gtk::PolicyType::Never);
        widget.set_child(Some(&content));

        Self { widget }
    }
}

// ---------------------------------------------------------------------------
// Trending Artists
// ---------------------------------------------------------------------------

fn build_trending_artists_section() -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    let header = build_section_header("Trending Artists", "VIEW ALL");
    section.append(&header);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Automatic);
    scroll.set_vscrollbar_policy(gtk::PolicyType::Never);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    row.set_margin_top(4);
    row.set_margin_bottom(4);

    for artist in trending_artists() {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("explore-artist-card");
        card.set_halign(gtk::Align::Center);

        // Circular avatar
        let avatar = gtk::Frame::new(None);
        avatar.add_css_class("explore-artist-avatar");
        avatar.set_size_request(128, 128);
        let icon = gtk::Image::from_icon_name(artist.icon);
        icon.set_pixel_size(48);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        avatar.set_child(Some(&icon));

        let name = gtk::Label::new(Some(artist.name));
        name.add_css_class("explore-artist-name");
        name.set_halign(gtk::Align::Center);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(14);

        card.append(&avatar);
        card.append(&name);
        row.append(&card);
    }

    scroll.set_child(Some(&row));
    section.append(&scroll);
    section
}

// ---------------------------------------------------------------------------
// Genre Grid (4 columns with image backgrounds)
// ---------------------------------------------------------------------------

fn build_genre_grid_section() -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    let header = build_section_header("Browse by Genre", "VIEW ALL");
    section.append(&header);

    let grid = gtk::Grid::new();
    grid.set_column_spacing(16);
    grid.set_row_spacing(16);
    grid.set_halign(gtk::Align::Fill);

    for (i, genre) in genres().iter().enumerate() {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("explore-genre-card");
        card.add_css_class(genre.css_class);
        if genre.is_large {
            card.add_css_class("explore-genre-card-large");
        } else {
            card.add_css_class("explore-genre-card-small");
        }

        // Gradient overlay
        let gradient = gtk::Box::new(gtk::Orientation::Vertical, 0);
        gradient.add_css_class("explore-genre-gradient");
        card.append(&gradient);

        // Icon (placeholder for background image)
        let icon = gtk::Image::from_icon_name(genre.icon);
        icon.set_pixel_size(48);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        icon.set_opacity(0.3);
        card.append(&icon);

        // Label at bottom
        let label = gtk::Label::new(Some(genre.name));
        label.add_css_class("explore-genre-label");
        card.append(&label);

        // Position in grid
        let col = (i % 4) as i32;
        let row_idx = (i / 4) as i32;
        let col_span = if genre.is_large { 2 } else { 1 };
        let row_span = if genre.is_large { 2 } else { 1 };
        grid.attach(&card, col, row_idx, col_span, row_span);
    }

    section.append(&grid);
    section
}

// ---------------------------------------------------------------------------
// Global Charts (2-column numbered track list)
// ---------------------------------------------------------------------------

fn build_global_charts_section() -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    let header = build_section_header("Global Charts", "VIEW ALL");
    section.append(&header);

    // Tabs
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    for (i, label_text) in ["Top 50", "Trending", "New & Hot"].iter().enumerate() {
        let tab = gtk::Button::with_label(label_text);
        tab.add_css_class("explore-chart-tab");
        if i == 0 {
            tab.add_css_class("active");
        }
        tabs.append(&tab);
    }
    section.append(&tabs);

    // 2-column chart grid
    let charts_grid = gtk::Grid::new();
    charts_grid.set_column_spacing(24);
    charts_grid.set_row_spacing(4);
    charts_grid.set_halign(gtk::Align::Fill);

    let tracks = global_charts();
    let half = (tracks.len() + 1) / 2;

    for (i, track) in tracks.iter().enumerate() {
        let col = if i < half { 0 } else { 1 };
        let row_idx = if i < half { i as i32 } else { (i - half) as i32 };

        let chart_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        chart_row.add_css_class("explore-chart-row");

        // Rank number
        let rank_label = gtk::Label::new(Some(&track.rank.to_string()));
        rank_label.add_css_class("explore-chart-number");
        rank_label.set_halign(gtk::Align::Center);
        rank_label.set_valign(gtk::Align::Center);
        rank_label.set_size_request(32, -1);

        // Art placeholder
        let art = gtk::Frame::new(None);
        art.add_css_class("explore-chart-art");
        let icon = gtk::Image::from_icon_name("emblem-music-symbolic");
        icon.set_pixel_size(24);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        art.set_child(Some(&icon));

        // Info
        let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info.set_hexpand(true);
        info.set_valign(gtk::Align::Center);

        let title = gtk::Label::new(Some(track.title));
        title.add_css_class("explore-chart-title");
        title.set_halign(gtk::Align::Start);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(30);

        let artist = gtk::Label::new(Some(track.artist));
        artist.add_css_class("explore-chart-artist");
        artist.set_halign(gtk::Align::Start);
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.set_max_width_chars(30);

        info.append(&title);
        info.append(&artist);

        // Duration
        let dur_label = gtk::Label::new(Some(track.duration));
        dur_label.add_css_class("explore-chart-duration");
        dur_label.set_halign(gtk::Align::End);
        dur_label.set_valign(gtk::Align::Center);

        chart_row.append(&rank_label);
        chart_row.append(&art);
        chart_row.append(&info);
        chart_row.append(&dur_label);

        charts_grid.attach(&chart_row, col, row_idx, 1, 1);
    }

    section.append(&charts_grid);
    section
}

// ---------------------------------------------------------------------------
// Featured Playlists
// ---------------------------------------------------------------------------

fn build_featured_playlists_section() -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    let header = build_section_header("Featured Playlists", "VIEW ALL");
    section.append(&header);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Automatic);
    scroll.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_min_content_height(200);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.set_margin_top(4);
    row.set_margin_bottom(4);

    for playlist in featured_playlists() {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.set_size_request(180, -1);

        // Playlist art placeholder
        let art = gtk::Frame::new(None);
        art.add_css_class("album-art");
        art.set_size_request(180, 180);
        let icon = gtk::Image::from_icon_name("emblem-music-symbolic");
        icon.set_pixel_size(32);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        art.set_child(Some(&icon));

        let title_label = gtk::Label::new(Some(playlist.title));
        title_label.add_css_class("album-title");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(22);

        let desc_label = gtk::Label::new(Some(playlist.description));
        desc_label.add_css_class("album-artist");
        desc_label.set_halign(gtk::Align::Start);
        desc_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        desc_label.set_max_width_chars(24);
        desc_label.set_wrap(true);
        desc_label.set_lines(2);

        card.append(&art);
        card.append(&title_label);
        card.append(&desc_label);
        row.append(&card);
    }

    scroll.set_child(Some(&row));
    section.append(&scroll);
    section
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_section_header(title: &str, link_text: &str) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    header.set_hexpand(true);

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("explore-section-header");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_hexpand(true);

    let link = gtk::Button::with_label(link_text);
    link.add_css_class("explore-view-all");

    header.append(&title_label);
    header.append(&link);
    header
}
