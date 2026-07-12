//! Bottom player bar (à la Spotify): album art + track info + close/add on
//! the left, shuffle/prev/play-pause/next/repeat + seek scale in the
//! center, and a right-side icon strip (queue, lyrics, connect-device,
//! volume, mini-player, fullscreen) on the right.
//!
//! Most of the right-side icons and the shuffle/repeat/close/add buttons
//! are laid out and disabled (with a tooltip explaining why) rather than
//! left off entirely — the bar should already look and feel finished, and
//! each one just needs its command/backend wired up later instead of the
//! whole layout being rebuilt.
//!
//! This widget only ever talks to the player thread through
//! `PlayerHandle::commands` (outgoing) and `update()` (incoming, called by
//! `window.rs` for every `PlayerState` it receives) — it never touches mpv
//! or tokio directly, same separation as `search_view.rs`.

use crate::player::{PlayerCommand, PlayerState};
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Tooltip suffix for every button that's laid out but not wired up yet, so
/// it's obvious in the UI (not just in a code comment) that it's planned
/// rather than broken.
const NOT_YET_WIRED: &str = "coming soon";

pub struct PlayerBar {
    pub widget: gtk::CenterBox,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    play_pause_button: gtk::Button,
    elapsed_label: gtk::Label,
    remaining_label: gtk::Label,
    seek_scale: gtk::Scale,
    /// True while the user has the seek handle pressed. `update()` skips
    /// writing to `seek_scale` while this is set, so an incoming poll tick
    /// (every ~500ms) can't yank the handle back mid-drag.
    seeking: Rc<Cell<bool>>,
    art_stack: gtk::Stack,
    art_picture: gtk::Picture,
    /// Thumbnail URL the art tile currently shows (or was last asked to
    /// fetch), so `update()` — called on every ~500ms poll tick — only
    /// kicks off a new fetch when the track actually changes, not on every
    /// tick.
    current_thumbnail_url: RefCell<String>,
}

impl PlayerBar {
    pub fn new(commands: async_channel::Sender<PlayerCommand>) -> Self {
        // `CenterBox` (not a plain `Box`) is what actually keeps the
        // transport controls centered in the *whole* bar — it centers its
        // middle child in the full allocation regardless of how wide the
        // start/end children are, instead of just splitting the leftover
        // space between three flex children like a plain `Box` would.
        let widget = gtk::CenterBox::new();
        widget.add_css_class("toolbar"); // subtle raised/bordered bar background
        widget.set_margin_start(12);
        widget.set_margin_end(12);
        widget.set_margin_top(8);
        widget.set_margin_bottom(8);

        // ---------------------------------------------------------------
        // Left: album art + track info + close/add
        // ---------------------------------------------------------------
        let left_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        left_box.set_valign(gtk::Align::Center);
        // No fixed/min width here on purpose — `max_width_chars` below on
        // the labels is what keeps this column compact, and `CenterBox`
        // doesn't need the left/right columns to match widths to keep the
        // center controls actually centered (see note above).

        // No artwork until a track with a thumbnail_url plays — the stack
        // starts on the placeholder page and `update()` swaps to the "art"
        // page once a thumbnail is actually fetched and decoded.
        let art_frame = gtk::Frame::new(None);
        art_frame.add_css_class("card");
        let art_stack = gtk::Stack::new();
        art_stack.set_size_request(48, 48);

        let art_placeholder = gtk::Image::from_icon_name("audio-x-generic-symbolic");
        art_placeholder.set_pixel_size(24);
        art_stack.add_named(&art_placeholder, Some("placeholder"));

        let art_picture = gtk::Picture::new();
        art_picture.set_content_fit(gtk::ContentFit::Cover);
        art_picture.set_size_request(48, 48);
        art_stack.add_named(&art_picture, Some("art"));

        art_stack.set_visible_child_name("placeholder");
        art_frame.set_child(Some(&art_stack));

        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info_box.set_valign(gtk::Align::Center);
        let title_label = gtk::Label::new(Some("Nothing playing"));
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(22);
        title_label.add_css_class("heading");
        let artist_label = gtk::Label::new(Some(""));
        artist_label.set_halign(gtk::Align::Start);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.set_max_width_chars(22);
        artist_label.add_css_class("dim-label");
        info_box.append(&title_label);
        info_box.append(&artist_label);

        let close_button = gtk::Button::from_icon_name("window-close-symbolic");
        close_button.add_css_class("flat");
        close_button.set_tooltip_text(Some(&format!("Remove from view ({NOT_YET_WIRED})")));
        close_button.set_sensitive(false);

        let add_button = gtk::Button::from_icon_name("list-add-symbolic");
        add_button.add_css_class("flat");
        add_button.set_tooltip_text(Some(&format!("Save to a playlist ({NOT_YET_WIRED})")));
        add_button.set_sensitive(false);

        left_box.append(&art_frame);
        left_box.append(&info_box);
        left_box.append(&close_button);
        left_box.append(&add_button);

        // ---------------------------------------------------------------
        // Center: shuffle / prev / play-pause / next / repeat + seek row
        // ---------------------------------------------------------------
        let center_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        center_box.set_hexpand(true);
        center_box.set_valign(gtk::Align::Center);

        let transport_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        transport_box.set_halign(gtk::Align::Center);

        let shuffle_button = gtk::Button::from_icon_name("media-playlist-shuffle-symbolic");
        shuffle_button.add_css_class("flat");
        shuffle_button.set_tooltip_text(Some(&format!("Shuffle ({NOT_YET_WIRED} — no queue yet)")));
        shuffle_button.set_sensitive(false);

        let prev_button = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        prev_button.add_css_class("flat");
        prev_button.set_tooltip_text(Some(&format!("Previous ({NOT_YET_WIRED} — no queue yet)")));
        prev_button.set_sensitive(false);

        let play_pause_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        play_pause_button.add_css_class("circular");
        {
            let commands = commands.clone();
            play_pause_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::TogglePause);
            });
        }

        let next_button = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next_button.add_css_class("flat");
        next_button.set_tooltip_text(Some(&format!("Next ({NOT_YET_WIRED} — no queue yet)")));
        next_button.set_sensitive(false);

        let repeat_button = gtk::Button::from_icon_name("media-playlist-repeat-symbolic");
        repeat_button.add_css_class("flat");
        repeat_button.set_tooltip_text(Some(&format!("Repeat ({NOT_YET_WIRED})")));
        repeat_button.set_sensitive(false);

        transport_box.append(&shuffle_button);
        transport_box.append(&prev_button);
        transport_box.append(&play_pause_button);
        transport_box.append(&next_button);
        transport_box.append(&repeat_button);

        let seek_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let elapsed_label = gtk::Label::new(Some("0:00"));
        elapsed_label.add_css_class("dim-label");
        elapsed_label.set_width_chars(5);
        let remaining_label = gtk::Label::new(Some("0:00"));
        remaining_label.add_css_class("dim-label");
        remaining_label.set_width_chars(5);

        let seek_adjustment = gtk::Adjustment::new(0.0, 0.0, 1.0, 1.0, 5.0, 0.0);
        let seek_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&seek_adjustment));
        seek_scale.set_hexpand(true);
        seek_scale.set_draw_value(false);

        let seeking = Rc::new(Cell::new(false));
        let drag_gesture = gtk::GestureClick::new();
        {
            let seeking = seeking.clone();
            drag_gesture.connect_pressed(move |_, _, _, _| seeking.set(true));
        }
        {
            let seeking = seeking.clone();
            drag_gesture.connect_released(move |_, _, _, _| seeking.set(false));
        }
        seek_scale.add_controller(drag_gesture);

        {
            let commands = commands.clone();
            seek_scale.connect_change_value(move |_, _scroll_type, value| {
                let _ = commands.send_blocking(PlayerCommand::Seek(value));
                glib::Propagation::Proceed // let the scale redraw at the new value too
            });
        }

        seek_row.append(&elapsed_label);
        seek_row.append(&seek_scale);
        seek_row.append(&remaining_label);

        center_box.append(&transport_box);
        center_box.append(&seek_row);

        // ---------------------------------------------------------------
        // Right: queue / lyrics / connect-device / volume / mini-player /
        // fullscreen icon strip
        // ---------------------------------------------------------------
        let right_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        right_box.set_valign(gtk::Align::Center);
        right_box.set_halign(gtk::Align::End);

        let queue_button = gtk::Button::from_icon_name("view-list-symbolic");
        queue_button.add_css_class("flat");
        queue_button.set_tooltip_text(Some(&format!("Queue ({NOT_YET_WIRED})")));
        queue_button.set_sensitive(false);

        let lyrics_button = gtk::Button::from_icon_name("audio-input-microphone-symbolic");
        lyrics_button.add_css_class("flat");
        lyrics_button.set_tooltip_text(Some(&format!("Lyrics ({NOT_YET_WIRED})")));
        lyrics_button.set_sensitive(false);

        let connect_button = gtk::Button::from_icon_name("video-display-symbolic");
        connect_button.add_css_class("flat");
        connect_button.set_tooltip_text(Some(&format!(
            "Connect to a device ({NOT_YET_WIRED} — MPRIS already exposes this app to \
             external controllers like waybar/playerctl)"
        )));
        connect_button.set_sensitive(false);

        let volume_icon = gtk::Image::from_icon_name("audio-volume-high-symbolic");
        let volume_adjustment = gtk::Adjustment::new(1.0, 0.0, 1.0, 0.05, 0.1, 0.0);
        let volume_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&volume_adjustment));
        volume_scale.set_size_request(100, -1);
        volume_scale.set_draw_value(false);
        {
            let commands = commands.clone();
            volume_scale.connect_value_changed(move |scale| {
                let _ = commands.send_blocking(PlayerCommand::SetVolume(scale.value()));
            });
        }

        let mini_player_button = gtk::Button::from_icon_name("focus-windows-symbolic");
        mini_player_button.add_css_class("flat");
        mini_player_button.set_tooltip_text(Some(&format!("Mini player ({NOT_YET_WIRED})")));
        mini_player_button.set_sensitive(false);

        let fullscreen_button = gtk::Button::from_icon_name("view-fullscreen-symbolic");
        fullscreen_button.add_css_class("flat");
        fullscreen_button.set_tooltip_text(Some(&format!("Fullscreen ({NOT_YET_WIRED})")));
        fullscreen_button.set_sensitive(false);

        right_box.append(&queue_button);
        right_box.append(&lyrics_button);
        right_box.append(&connect_button);
        right_box.append(&volume_icon);
        right_box.append(&volume_scale);
        right_box.append(&mini_player_button);
        right_box.append(&fullscreen_button);

        widget.set_start_widget(Some(&left_box));
        widget.set_center_widget(Some(&center_box));
        widget.set_end_widget(Some(&right_box));

        Self {
            widget,
            title_label,
            artist_label,
            play_pause_button,
            elapsed_label,
            remaining_label,
            seek_scale,
            seeking,
            art_stack,
            art_picture,
            current_thumbnail_url: RefCell::new(String::new()),
        }
    }

    /// Redraws the bar from a fresh `PlayerState`. Called by `window.rs`
    /// every time the player thread pushes an update (on `Play` and on each
    /// ~500ms poll tick).
    pub fn update(&self, state: &PlayerState) {
        self.title_label.set_label(&state.title);
        self.artist_label.set_label(&state.artist);

        if *self.current_thumbnail_url.borrow() != state.thumbnail_url {
            *self.current_thumbnail_url.borrow_mut() = state.thumbnail_url.clone();
            if state.thumbnail_url.is_empty() {
                self.art_stack.set_visible_child_name("placeholder");
            } else {
                let art_stack = self.art_stack.clone();
                let art_picture = self.art_picture.clone();
                thumbnail_widget::spawn_fetch(state.thumbnail_url.clone(), 48, move |texture| {
                    art_picture.set_paintable(Some(&texture));
                    art_stack.set_visible_child_name("art");
                });
            }
        }

        let icon = if state.paused {
            "media-playback-start-symbolic"
        } else {
            "media-playback-pause-symbolic"
        };
        self.play_pause_button.set_icon_name(icon);

        if state.duration_seconds > 0.0 {
            self.seek_scale.set_range(0.0, state.duration_seconds);
        }
        if !self.seeking.get() {
            self.seek_scale.set_value(state.position_seconds);
        }

        self.elapsed_label
            .set_label(&format_time(state.position_seconds));
        let remaining = (state.duration_seconds - state.position_seconds).max(0.0);
        self.remaining_label.set_label(&format_time(remaining));
    }
}

/// Formats a duration in seconds as `m:ss`, matching the timestamps shown
/// next to the seek bar (e.g. "2:10").
fn format_time(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}
