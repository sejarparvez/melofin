//! The home page: shown by default until the user searches (see
//! `window.rs`, which keeps this underneath the search popover — see
//! `search_view.rs`'s doc comment).
//!
//! Data comes from `home_feed::fetch_home_feed()`, which tries the
//! personalized InnerTube feed first (when logged in), then falls back
//! to unpersonalized `yt-dlp` searches. The fetch runs on a background
//! thread — same `thread::spawn` + `async_channel` + `spawn_future_local`
//! pattern as `search_view.rs`'s debounced search — so the page starts in
//! a loading state and swaps in rows once the feed arrives.
//!
//! Cards reuse `search::Track` rather than inventing a parallel type, so
//! both the InnerTube feed and the yt-dlp fallback produce the same
//! `Vec<Track>`s to feed the same `build_row`/`track_card` functions.

use crate::home_feed::HomeFeed;
use crate::search::Track;
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use gtk::glib;
use std::path::PathBuf;
use std::rc::Rc;

pub struct HomeView {
    pub widget: gtk::ScrolledWindow,
}

impl HomeView {
    /// `on_select` fires when the user clicks a card — navigates to detail.
    /// `on_play` fires when the user clicks the hover play icon — plays immediately.
    pub fn new(
        cookies_path: PathBuf,
        cache_path: PathBuf,
        history_path: PathBuf,
        user_name: &str,
        on_select: Rc<dyn Fn(Track)>,
        on_play: Rc<dyn Fn(Track)>,
    ) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 32);
        content.set_margin_top(32);
        content.set_margin_bottom(32);
        content.set_margin_start(32);
        content.set_margin_end(32);

        content.append(&build_greeting(user_name));
        content.append(&build_for_you_section());

        // Hero card + rows get built into this once the feed loads; starts
        // out holding a loading spinner.
        let feed_container = gtk::Box::new(gtk::Orientation::Vertical, 24);
        feed_container.append(&loading_state());
        content.append(&feed_container);

        load_feed(
            feed_container,
            cookies_path,
            cache_path,
            history_path,
            on_select,
            on_play,
        );

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_hscrollbar_policy(gtk::PolicyType::Never);
        widget.set_child(Some(&content));

        Self { widget }
    }
}

/// Kicks off `home_feed::fetch_home_feed()` on a background thread (it
/// shells out to `yt-dlp` per row or makes HTTP requests — far too slow
/// for the GTK main thread) and populates `container` with the result
/// once it lands. Also called again by the error state's "Retry" button,
/// so it's careful to leave `container` empty before it starts and fully
/// replace its contents when done rather than assuming it's only ever
/// called once.
fn load_feed(
    container: gtk::Box,
    cookies_path: PathBuf,
    cache_path: PathBuf,
    history_path: PathBuf,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) {
    let (sender, receiver) = async_channel::bounded::<HomeFeed>(1);
    let cookies = cookies_path.clone();
    let cache = cache_path.clone();
    let history = history_path.clone();
    std::thread::spawn(move || {
        let _ = sender.send_blocking(crate::home_feed::fetch_home_feed(
            &cookies, &cache, &history,
        ));
    });

    glib::spawn_future_local(async move {
        let Ok(feed) = receiver.recv().await else {
            return;
        };
        clear(&container);

        if feed.sections.is_empty() {
            container.append(&error_state(
                container.clone(),
                cookies_path,
                cache_path,
                history_path,
                on_select,
                on_play,
            ));
            return;
        }

        // The first track of the first row anchors the hero card — not
        // meaningfully different from any other card in that row, just
        // given more visual weight, the way a real Quick Picks hero would
        // be.
        if let Some(first) = feed.sections[0].tracks.first() {
            container.append(&build_hero_card(
                first.clone(),
                &feed.sections[0].title,
                on_select.clone(),
                on_play.clone(),
            ));
        }

        for section in feed.sections {
            if section.title == "New releases" || section.title == "New Releases" {
                container.append(&build_numbered_row(
                    &section.title,
                    section.tracks,
                    on_select.clone(),
                    on_play.clone(),
                ));
            } else if section.title == "Recently Played" {
                container.append(&build_recently_played_row(
                    section.tracks,
                    on_select.clone(),
                    on_play.clone(),
                ));
            } else {
                container.append(&build_row(
                    &section.title,
                    section.tracks,
                    on_select.clone(),
                    on_play.clone(),
                ));
            }
        }
    });
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn loading_state() -> gtk::Widget {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_valign(gtk::Align::Center);
    box_.set_halign(gtk::Align::Center);
    box_.set_margin_top(60);

    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(32, 32);

    let label = gtk::Label::new(Some("Loading your feed…"));
    label.add_css_class("dim-label");

    box_.append(&spinner);
    box_.append(&label);
    box_.upcast()
}

/// Shown when every row failed (offline, `yt-dlp` missing/broken, etc.) —
/// see `home_feed::fetch_home_feed`'s doc comment on why a partial feed
/// beats an error mid-page, but an *entirely* empty one still needs
/// something better than a blank page.
fn error_state(
    container: gtk::Box,
    cookies_path: PathBuf,
    cache_path: PathBuf,
    history_path: PathBuf,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Widget {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_valign(gtk::Align::Center);
    box_.set_halign(gtk::Align::Center);
    box_.set_margin_top(60);

    let label = gtk::Label::new(Some(
        "Couldn't load your feed. Check that yt-dlp is installed and you're online.",
    ));
    label.add_css_class("dim-label");
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);

    let retry = gtk::Button::with_label("Retry");
    retry.add_css_class("pill");
    retry.set_halign(gtk::Align::Center);
    retry.connect_clicked(move |_| {
        clear(&container);
        container.append(&loading_state());
        load_feed(
            container.clone(),
            cookies_path.clone(),
            cache_path.clone(),
            history_path.clone(),
            on_select.clone(),
            on_play.clone(),
        );
    });

    box_.append(&label);
    box_.append(&retry);
    box_.upcast()
}

/// Time-based greeting section matching the Stitch "Library Dashboard" design.
fn build_greeting(user_name: &str) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);

    let now = glib::DateTime::now_local()
        .unwrap_or_else(|_| glib::DateTime::now_utc().expect("failed to get UTC time"));
    let hour = now.hour();
    let greeting = match hour {
        0..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    };

    let greeting_text = if user_name.is_empty() || user_name == "Guest" {
        greeting.to_string()
    } else {
        format!("{}, {}", greeting, user_name)
    };

    let greeting_label = gtk::Label::new(Some(&greeting_text));
    greeting_label.add_css_class("greeting-text");
    greeting_label.set_halign(gtk::Align::Start);

    let subtitle = gtk::Label::new(Some("Ready for some high-fidelity listening? Your personalized library is updated."));
    subtitle.add_css_class("greeting-subtitle");
    subtitle.set_halign(gtk::Align::Start);

    box_.append(&greeting_label);
    box_.append(&subtitle);
    box_
}

/// "For You" section with category cards matching the Stitch design.
fn build_for_you_section() -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    // Section header with navigation arrows
    let (header, prev_btn, next_btn) = build_section_header("For You");
    section.append(&header);

    // Category cards
    let cards = gtk::Box::new(gtk::Orientation::Horizontal, 16);

    let categories = vec![
        ("PERSONALIZED", "Daily Mix 1", "Neon Drift, Arlowe Thorne, and more.", "media-playlist-shuffle-symbolic"),
        ("PRODUCTIVITY", "Focus Flow", "Lofi beats for deep work sessions.", "audio-x-generic-symbolic"),
        ("TOP TRACKS", "Heavy Rotation", "Your most played tracks this month.", "star-new-symbolic"),
    ];

    for (label, title, desc, icon) in categories {
        let card = category_card(label, title, desc, icon);
        cards.append(&card);
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_hscrollbar_policy(gtk::PolicyType::External);
    scroller.set_child(Some(&cards));
    section.append(&scroller);

    // Wire navigation arrows with smooth scrolling and disabled states
    wire_carousel_arrows(&scroller, &prev_btn, &next_btn);

    section
}

/// A single category card for the "For You" section.
fn category_card(label: &str, title: &str, description: &str, icon_name: &str) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    card.add_css_class("category-card");
    card.set_size_request(300, -1);

    let art = gtk::Frame::new(None);
    art.add_css_class("category-art");
    art.set_size_request(80, 80);
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(28);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    art.set_child(Some(&icon));

    let info = gtk::Box::new(gtk::Orientation::Vertical, 4);
    info.set_valign(gtk::Align::Center);
    info.set_hexpand(true);

    let label_widget = gtk::Label::new(Some(label));
    label_widget.add_css_class("category-label");
    label_widget.set_halign(gtk::Align::Start);

    let title_widget = gtk::Label::new(Some(title));
    title_widget.add_css_class("category-title");
    title_widget.set_halign(gtk::Align::Start);

    let desc_widget = gtk::Label::new(Some(description));
    desc_widget.add_css_class("category-description");
    desc_widget.set_halign(gtk::Align::Start);
    desc_widget.set_wrap(true);
    desc_widget.set_ellipsize(gtk::pango::EllipsizeMode::End);
    desc_widget.set_max_width_chars(30);

    info.append(&label_widget);
    info.append(&title_widget);
    info.append(&desc_widget);

    card.append(&art);
    card.append(&info);

    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&card));
    button.upcast()
}

/// Section header with title and navigation arrows.
fn build_section_header(title: &str) -> (gtk::Box, gtk::Button, gtk::Button) {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    header.set_hexpand(true);

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("section-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_hexpand(true);

    let arrows = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let prev_btn = gtk::Button::from_icon_name("go-previous-symbolic");
    prev_btn.add_css_class("nav-arrow");
    let next_btn = gtk::Button::from_icon_name("go-next-symbolic");
    next_btn.add_css_class("nav-arrow");
    arrows.append(&prev_btn);
    arrows.append(&next_btn);

    header.append(&title_label);
    header.append(&arrows);

    (header, prev_btn, next_btn)
}

/// Section header with title and a right-aligned link (e.g. "VIEW ALL").
fn build_section_header_with_link(title: &str, link_text: &str) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    header.set_hexpand(true);

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("section-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_hexpand(true);

    let link = gtk::Button::with_label(link_text);
    link.add_css_class("section-header-link");
    link.set_halign(gtk::Align::End);

    header.append(&title_label);
    header.append(&link);

    header
}

/// Wire carousel navigation arrows with smooth scrolling and disabled states.
fn wire_carousel_arrows(scroller: &gtk::ScrolledWindow, prev_btn: &gtk::Button, next_btn: &gtk::Button) {
    let scroll_amount = 300.0;

    // Start with both buttons enabled (content may not be laid out yet)
    prev_btn.set_sensitive(true);
    next_btn.set_sensitive(true);

    // Update button sensitivity based on scroll position
    let update_buttons = {
        let prev_btn = prev_btn.clone();
        let next_btn = next_btn.clone();
        let scroller = scroller.clone();
        move || {
            let adj = scroller.hadjustment();
            // Only update if we have valid content (upper > lower + page_size means content exists)
            if adj.upper() > adj.lower() + adj.page_size() {
                let at_start = adj.value() <= adj.lower() + 1.0;
                let at_end = adj.value() + adj.page_size() >= adj.upper() - 1.0;
                prev_btn.set_sensitive(!at_start);
                next_btn.set_sensitive(!at_end);
            }
            // If content not laid out yet, keep both enabled
        }
    };

    // Connect adjustment value-changed to update button states
    {
        let update_buttons = update_buttons.clone();
        let scroller = scroller.clone();
        scroller.hadjustment().connect_value_changed(move |_| {
            update_buttons();
        });
    }

    // Connect map signal to update button states after widget is displayed
    {
        let update_buttons = update_buttons.clone();
        scroller.connect_map(move |_| {
            let update_buttons = update_buttons.clone();
            // Delay slightly to ensure layout is complete
            glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
                update_buttons();
            });
        });
    }

    // Previous button - scroll left
    {
        let scroller = scroller.clone();
        prev_btn.connect_clicked(move |_| {
            let adj = scroller.hadjustment();
            let new_val = (adj.value() - scroll_amount).max(adj.lower());
            adj.set_value(new_val);
        });
    }

    // Next button - scroll right
    {
        let scroller = scroller.clone();
        next_btn.connect_clicked(move |_| {
            let adj = scroller.hadjustment();
            let new_val = (adj.value() + scroll_amount).min(adj.upper() - adj.page_size());
            adj.set_value(new_val);
        });
    }
}

/// The single big hero card, anchored to a real track from the first
/// loaded row (see `load_feed`). Clicking the card navigates to detail.
/// The play button plays immediately.
fn build_hero_card(
    track: Track,
    section_title: &str,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Widget {
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

    if !track.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(96, 96);
        art.set_child(Some(&picture));
        thumbnail_widget::spawn_fetch(track.thumbnail_url.clone(), 96, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text_box.set_valign(gtk::Align::Center);
    text_box.set_hexpand(true);

    let eyebrow = gtk::Label::new(Some(section_title));
    eyebrow.add_css_class("caption");
    eyebrow.add_css_class("dim-label");
    eyebrow.set_halign(gtk::Align::Start);

    let title = gtk::Label::new(Some(&track.title));
    title.add_css_class("title-2");
    title.set_halign(gtk::Align::Start);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let subtitle = gtk::Label::new(Some(&track.artist));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);

    text_box.append(&eyebrow);
    text_box.append(&title);
    text_box.append(&subtitle);

    let play_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_button.add_css_class("circular");
    play_button.add_css_class("suggested-action");
    play_button.set_valign(gtk::Align::Center);
    {
        let on_play = on_play.clone();
        let track_clone = track.clone();
        play_button.connect_clicked(move |_| {
            on_play(track_clone.clone());
        });
    }

    // Clicking the text area navigates to detail.
    {
        let text_click = gtk::GestureClick::new();
        let track_for_nav = track.clone();
        let on_select_for_nav = on_select.clone();
        text_click.connect_pressed(move |_, _, _, _| {
            on_select_for_nav(track_for_nav.clone());
        });
        text_box.add_controller(text_click);
    }

    card.append(&art);
    card.append(&text_box);
    card.append(&play_button);
    card.upcast()
}

/// "Recently Played" section with "VIEW ALL" link and horizontal card scroll.
fn build_recently_played_row(
    tracks: Vec<Track>,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    let header = build_section_header_with_link("Recently Played", "VIEW ALL");
    section.append(&header);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    for track in tracks {
        row.append(&track_card(track, on_select.clone(), on_play.clone()));
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_hscrollbar_policy(gtk::PolicyType::External);
    scroller.set_child(Some(&row));
    section.append(&scroller);

    section
}

/// A titled, horizontally-scrolling row of cards.
fn build_row(
    title: &str,
    tracks: Vec<Track>,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    // Section header with navigation arrows
    let (header, prev_btn, next_btn) = build_section_header(title);
    section.append(&header);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    for track in tracks {
        row.append(&track_card(track, on_select.clone(), on_play.clone()));
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_hscrollbar_policy(gtk::PolicyType::External);
    scroller.set_child(Some(&row));
    section.append(&scroller);

    // Wire navigation arrows with smooth scrolling and disabled states
    wire_carousel_arrows(&scroller, &prev_btn, &next_btn);

    section
}

/// A titled, vertically-scrolling numbered track list (for "New Releases", etc.).
fn build_numbered_row(
    title: &str,
    tracks: Vec<Track>,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 16);

    // Section header with link
    let header = build_section_header_with_link(title, "DISCOVER NEW");
    section.append(&header);

    // Column headers
    let col_header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    col_header.add_css_class("numbered-list-header");
    col_header.set_margin_start(44); // align past number column
    col_header.set_margin_end(12);

    let hash_label = gtk::Label::new(Some("#"));
    hash_label.set_halign(gtk::Align::Start);
    let title_hdr = gtk::Label::new(Some("Title"));
    title_hdr.set_halign(gtk::Align::Start);
    title_hdr.set_hexpand(true);
    let album_hdr = gtk::Label::new(Some("Album"));
    album_hdr.set_halign(gtk::Align::Start);
    album_hdr.set_size_request(120, -1);
    let dur_hdr = gtk::Label::new(Some("Duration"));
    dur_hdr.set_halign(gtk::Align::End);
    dur_hdr.set_size_request(48, -1);

    col_header.append(&hash_label);
    col_header.append(&title_hdr);
    col_header.append(&album_hdr);
    col_header.append(&dur_hdr);
    section.append(&col_header);

    // Track rows
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for (i, track) in tracks.into_iter().enumerate() {
        list.append(&numbered_track_row(
            i + 1,
            track,
            on_select.clone(),
            on_play.clone(),
        ));
    }
    section.append(&list);

    section
}

/// A single row in a numbered track list.
fn numbered_track_row(
    number: usize,
    track: Track,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("numbered-track-row");

    // Number
    let num_label = gtk::Label::new(Some(&format!("{:02}", number)));
    num_label.add_css_class("numbered-track-number");
    num_label.set_halign(gtk::Align::Center);
    num_label.set_valign(gtk::Align::Center);

    // Play button (hidden, replaces number on hover)
    let play_btn = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_btn.add_css_class("numbered-track-play");
    play_btn.set_halign(gtk::Align::Center);
    play_btn.set_valign(gtk::Align::Center);
    {
        let on_play = on_play.clone();
        let track_clone = track.clone();
        play_btn.connect_clicked(move |_| {
            on_play(track_clone.clone());
        });
    }

    // Stack number + play button in same position
    let num_stack = gtk::Overlay::new();
    num_stack.set_size_request(32, -1);
    num_stack.set_child(Some(&num_label));
    num_stack.add_overlay(&play_btn);

    // Thumbnail
    let art = gtk::Frame::new(None);
    art.add_css_class("numbered-track-art");
    art.set_size_request(40, 40);

    if !track.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(40, 40);
        art.set_child(Some(&picture));
        let url = track.thumbnail_url.clone();
        crate::ui::thumbnail_widget::spawn_fetch(url, 40, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    // Title + artist
    let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
    info.add_css_class("numbered-track-info");
    info.set_valign(gtk::Align::Center);

    let title_label = gtk::Label::new(Some(&track.title));
    title_label.add_css_class("numbered-track-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(40);

    let artist_label = gtk::Label::new(Some(&track.artist));
    artist_label.add_css_class("numbered-track-artist");
    artist_label.set_halign(gtk::Align::Start);
    artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist_label.set_max_width_chars(40);

    info.append(&title_label);
    info.append(&artist_label);

    // Album name
    let album_text = track.album.as_deref().unwrap_or("");
    let album_label = gtk::Label::new(Some(album_text));
    album_label.add_css_class("numbered-track-album");
    album_label.set_halign(gtk::Align::Start);
    album_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    album_label.set_max_width_chars(20);
    album_label.set_size_request(120, -1);

    // Duration
    let dur_text = track.duration.as_deref().unwrap_or("--:--");
    let dur_label = gtk::Label::new(Some(dur_text));
    dur_label.add_css_class("numbered-track-duration");
    dur_label.set_halign(gtk::Align::End);
    dur_label.set_size_request(48, -1);

    // Favorite button (visual placeholder)
    let fav_btn = gtk::Button::from_icon_name("favorite-border-symbolic");
    fav_btn.add_css_class("flat");
    fav_btn.add_css_class("circular");
    fav_btn.set_size_request(28, 28);

    // More button
    let more_btn = gtk::Button::from_icon_name("view-more-symbolic");
    more_btn.add_css_class("flat");
    more_btn.add_css_class("circular");
    more_btn.set_size_request(28, 28);

    row.append(&num_stack);
    row.append(&art);
    row.append(&info);
    row.append(&fav_btn);
    row.append(&album_label);
    row.append(&dur_label);
    row.append(&more_btn);

    // Clicking the row navigates to detail
    {
        let on_select = on_select.clone();
        let track_clone = track.clone();
        let click = gtk::GestureClick::new();
        click.connect_pressed(move |_, _, _, _| {
            on_select(track_clone.clone());
        });
        row.add_controller(click);
    }

    row.upcast()
}

/// A single card with hover play icon. Clicking the card navigates to
/// detail. Hovering shows a play button that plays immediately.
fn track_card(
    track: Track,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.set_width_request(180);

    // Art with hover play overlay.
    let art_frame = gtk::Frame::new(None);
    art_frame.add_css_class("album-art");
    art_frame.set_size_request(180, 180);

    let art_icon = gtk::Image::from_icon_name("emblem-music-symbolic");
    art_icon.set_pixel_size(32);
    art_icon.set_halign(gtk::Align::Center);
    art_icon.set_valign(gtk::Align::Center);
    art_frame.set_child(Some(&art_icon));

    if !track.thumbnail_url.is_empty() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(180, 180);
        art_frame.set_child(Some(&picture));
        crate::ui::thumbnail_widget::spawn_fetch(track.thumbnail_url.clone(), 180, move |tex| {
            picture.set_paintable(Some(&tex));
        });
    }

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&art_frame));

    let play_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
    play_button.add_css_class("circular");
    play_button.add_css_class("suggested-action");
    play_button.set_halign(gtk::Align::End);
    play_button.set_valign(gtk::Align::End);
    play_button.set_margin_end(8);
    play_button.set_margin_bottom(8);
    play_button.set_visible(false);
    {
        let on_play = on_play.clone();
        let track_clone = track.clone();
        let play_click = gtk::GestureClick::new();
        play_click.set_propagation_phase(gtk::PropagationPhase::Capture);
        play_click.connect_pressed(move |gesture, _, _, _| {
            on_play(track_clone.clone());
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        play_button.add_controller(play_click);
    }
    overlay.add_overlay(&play_button);

    // Hover: show/hide play button.
    {
        let enter_button = play_button.clone();
        let motion = gtk::EventControllerMotion::new();
        motion.connect_enter(move |_, _, _| {
            enter_button.set_visible(true);
        });
        let leave_button = play_button.clone();
        motion.connect_leave(move |_| {
            leave_button.set_visible(false);
        });
        overlay.add_controller(motion);
    }

    let title_label = gtk::Label::new(Some(&track.title));
    title_label.add_css_class("album-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(22);

    let artist_label = gtk::Label::new(Some(&track.artist));
    artist_label.add_css_class("album-artist");
    artist_label.set_halign(gtk::Align::Start);
    artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist_label.set_max_width_chars(22);

    card.append(&overlay);
    card.append(&title_label);
    card.append(&artist_label);

    // Clicking the card navigates to detail.
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("album-card");
    button.set_child(Some(&card));
    {
        let on_select = on_select.clone();
        let track_clone = track.clone();
        button.connect_clicked(move |_| {
            on_select(track_clone.clone());
        });
    }

    button.upcast()
}
