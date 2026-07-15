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
        on_select: Rc<dyn Fn(Track)>,
        on_play: Rc<dyn Fn(Track)>,
    ) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 24);
        content.set_margin_top(20);
        content.set_margin_bottom(24);
        content.set_margin_start(20);
        content.set_margin_end(20);

        content.append(&build_filter_pills());
        content.append(&build_shortcuts_grid());

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
            container.append(&build_row(
                &section.title,
                section.tracks,
                on_select.clone(),
                on_play.clone(),
            ));
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
/// a real view yet (they need a library/local-storage layer, not just a
/// home feed — see `doc/GUIDE.md`'s build order).
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

/// A titled, horizontally-scrolling row of cards.
fn build_row(
    title: &str,
    tracks: Vec<Track>,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);

    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("title-2");
    heading.set_halign(gtk::Align::Start);
    section.append(&heading);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
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

/// A single card with hover play icon. Clicking the card navigates to
/// detail. Hovering shows a play button that plays immediately.
fn track_card(
    track: Track,
    on_select: Rc<dyn Fn(Track)>,
    on_play: Rc<dyn Fn(Track)>,
) -> gtk::Widget {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_width_request(150);

    // Art with hover play overlay.
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

    card.append(&overlay);
    card.append(&title_label);
    card.append(&artist_label);

    // Clicking the card navigates to detail.
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("home-card");
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
