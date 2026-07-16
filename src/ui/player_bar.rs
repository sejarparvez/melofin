use crate::player::{PlayerCommand, PlayerState};
use crate::queue::RepeatMode;
use crate::ui::thumbnail_widget::ThumbnailStack;
use adw::prelude::*;
use gtk::glib;
use std::cell::Cell;
use std::rc::Rc;

pub struct PlayerBar {
    pub widget: gtk::Box,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    play_pause_button: gtk::Button,
    prev_button: gtk::Button,
    next_button: gtk::Button,
    shuffle_button: gtk::Button,
    repeat_button: gtk::Button,
    elapsed_label: gtk::Label,
    remaining_label: gtk::Label,
    seek_scale: gtk::Scale,
    seeking: Rc<Cell<bool>>,
    thumbnail: ThumbnailStack,
    /// Public handle so `window.rs` can connect the queue toggle.
    pub queue_button: gtk::MenuButton,
    /// Clickable track info area — clicking navigates to Now Playing view.
    pub track_info_area: gtk::Box,
}

impl PlayerBar {
    pub fn new(commands: async_channel::Sender<PlayerCommand>) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("player-bar");

        // ---------------------------------------------------------------
        // Content row: left (art + info + heart) | center (transport) | right (icons + volume)
        // ---------------------------------------------------------------
        let content_row = gtk::Box::new(gtk::Orientation::Horizontal, 24);
        content_row.set_margin_start(16);
        content_row.set_margin_end(16);
        content_row.set_margin_top(8);

        // Left: album art + track info + heart
        let left_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        left_box.set_valign(gtk::Align::Center);
        left_box.set_hexpand(true);

        let thumbnail = ThumbnailStack::new("audio-x-generic-symbolic", 24, 48);
        let art_frame = gtk::Frame::new(None);
        art_frame.add_css_class("card");
        art_frame.set_child(Some(thumbnail.widget()));

        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info_box.set_valign(gtk::Align::Center);
        let title_label = gtk::Label::new(Some("Nothing playing"));
        title_label.set_halign(gtk::Align::Start);
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(22);
        title_label.add_css_class("track-title");
        let artist_label = gtk::Label::new(Some(""));
        artist_label.set_halign(gtk::Align::Start);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.set_max_width_chars(22);
        artist_label.add_css_class("track-artist");
        info_box.append(&title_label);
        info_box.append(&artist_label);

        // Heart/Like button
        let heart_button = gtk::Button::from_icon_name("emblem-favorite-symbolic");
        heart_button.add_css_class("heart-button");
        heart_button.set_tooltip_text(Some("Like"));
        let liked = Rc::new(Cell::new(false));
        {
            let liked = liked.clone();
            heart_button.connect_clicked(move |btn| {
                let new_val = !liked.get();
                liked.set(new_val);
                if new_val {
                    btn.add_css_class("liked");
                } else {
                    btn.remove_css_class("liked");
                }
            });
        }

        left_box.append(&art_frame);
        left_box.append(&info_box);
        left_box.append(&heart_button);

        // Center: transport controls + progress bar
        let center_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        center_box.set_hexpand(true);
        center_box.set_valign(gtk::Align::Center);
        center_box.set_width_request(400);

        // Transport controls row
        let transport_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        transport_box.set_halign(gtk::Align::Center);

        // Shuffle button
        let shuffle_active = Rc::new(Cell::new(false));
        let shuffle_button = gtk::Button::from_icon_name("media-playlist-shuffle-symbolic");
        shuffle_button.add_css_class("transport-button");
        shuffle_button.set_tooltip_text(Some("Shuffle"));
        {
            let commands = commands.clone();
            let shuffle_active = shuffle_active.clone();
            shuffle_button.connect_clicked(move |_| {
                let new_val = !shuffle_active.get();
                shuffle_active.set(new_val);
                let _ = commands.send_blocking(PlayerCommand::SetShuffle(new_val));
            });
        }

        // Previous button
        let prev_button = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        prev_button.add_css_class("transport-button");
        prev_button.set_tooltip_text(Some("Previous"));
        {
            let commands = commands.clone();
            prev_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::Previous);
            });
        }

        // Play/Pause button
        let play_pause_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        play_pause_button.add_css_class("play-button");
        {
            let commands = commands.clone();
            play_pause_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::TogglePause);
            });
        }

        // Next button
        let next_button = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next_button.add_css_class("transport-button");
        next_button.set_tooltip_text(Some("Next"));
        {
            let commands = commands.clone();
            next_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::Next);
            });
        }

        // Repeat button
        let repeat_button = gtk::Button::from_icon_name("media-playlist-repeat-symbolic");
        repeat_button.add_css_class("transport-button");
        repeat_button.set_tooltip_text(Some("Repeat: Off"));
        {
            let commands = commands.clone();
            repeat_button.connect_clicked(move |btn| {
                let current = match btn.widget_name().as_str() {
                    "repeat-all" => RepeatMode::RepeatAll,
                    "repeat-one" => RepeatMode::RepeatOne,
                    _ => RepeatMode::Off,
                };
                let next = match current {
                    RepeatMode::Off => RepeatMode::RepeatAll,
                    RepeatMode::RepeatAll => RepeatMode::RepeatOne,
                    RepeatMode::RepeatOne => RepeatMode::Off,
                };
                let _ = commands.send_blocking(PlayerCommand::SetRepeat(next));
            });
        }

        transport_box.append(&shuffle_button);
        transport_box.append(&prev_button);
        transport_box.append(&play_pause_button);
        transport_box.append(&next_button);
        transport_box.append(&repeat_button);

        center_box.append(&transport_box);

        // Right: queue / lyrics / volume / fullscreen
        let right_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        right_box.set_valign(gtk::Align::Center);
        right_box.set_halign(gtk::Align::End);
        right_box.set_hexpand(true);

        let queue_button = gtk::MenuButton::new();
        queue_button.set_icon_name("view-list-symbolic");
        queue_button.add_css_class("icon-button");
        queue_button.set_tooltip_text(Some("Queue"));

        let lyrics_button = gtk::Button::from_icon_name("accessories-text-editor-symbolic");
        lyrics_button.add_css_class("icon-button");
        lyrics_button.set_tooltip_text(Some("Lyrics"));

        let volume_icon = gtk::Image::from_icon_name("audio-volume-high-symbolic");
        volume_icon.add_css_class("dim-label");
        let volume_adjustment = gtk::Adjustment::new(1.0, 0.0, 1.0, 0.05, 0.1, 0.0);
        let volume_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&volume_adjustment));
        volume_scale.add_css_class("volume-scale");
        volume_scale.set_size_request(80, -1);
        volume_scale.set_draw_value(false);
        {
            let commands = commands.clone();
            volume_scale.connect_value_changed(move |scale| {
                let _ = commands.send_blocking(PlayerCommand::SetVolume(scale.value()));
            });
        }

        let fullscreen_button = gtk::Button::from_icon_name("view-fullscreen-symbolic");
        fullscreen_button.add_css_class("icon-button");
        fullscreen_button.set_tooltip_text(Some("Fullscreen"));

        right_box.append(&queue_button);
        right_box.append(&lyrics_button);
        right_box.append(&volume_icon);
        right_box.append(&volume_scale);
        right_box.append(&fullscreen_button);

        content_row.append(&left_box);
        content_row.append(&center_box);
        content_row.append(&right_box);

        // ---------------------------------------------------------------
        // Progress bar: inside center section, below transport controls
        // ---------------------------------------------------------------
        let progress_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        progress_row.set_hexpand(true);

        let elapsed_label = gtk::Label::new(Some("0:00"));
        elapsed_label.add_css_class("time-label");
        elapsed_label.set_width_chars(4);

        let remaining_label = gtk::Label::new(Some("0:00"));
        remaining_label.add_css_class("time-label");
        remaining_label.set_width_chars(4);

        let seek_adjustment = gtk::Adjustment::new(0.0, 0.0, 1.0, 1.0, 5.0, 0.0);
        let seek_scale = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&seek_adjustment));
        seek_scale.add_css_class("progress-bar");
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
                glib::Propagation::Proceed
            });
        }

        progress_row.append(&elapsed_label);
        progress_row.append(&seek_scale);
        progress_row.append(&remaining_label);

        center_box.append(&progress_row);

        widget.append(&content_row);

        Self {
            widget,
            title_label,
            artist_label,
            play_pause_button,
            prev_button,
            next_button,
            shuffle_button,
            repeat_button,
            elapsed_label,
            remaining_label,
            seek_scale,
            seeking,
            thumbnail,
            queue_button,
            track_info_area: info_box,
        }
    }

    pub fn update(&self, state: &PlayerState) {
        self.title_label.set_label(&state.title);
        self.artist_label.set_label(&state.artist);
        self.thumbnail.update(&state.thumbnail_url, 48);

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

        // Shuffle toggle visual state.
        self.shuffle_button.set_sensitive(state.queue_len > 1);
        self.shuffle_button
            .set_opacity(if state.shuffle { 1.0 } else { 0.5 });

        // Prev/Next sensitivity: only when there's something to go to.
        self.prev_button
            .set_sensitive(state.queue_index.is_some_and(|i| i > 0) || state.shuffle);
        let has_next = state.queue_index.is_some_and(|i| i + 1 < state.queue_len)
            || matches!(state.repeat, RepeatMode::RepeatAll | RepeatMode::RepeatOne)
            || state.shuffle;
        self.next_button.set_sensitive(has_next);

        // Repeat icon + tooltip.
        let (icon, label) = match state.repeat {
            RepeatMode::Off => ("media-playlist-repeat-symbolic", "Repeat: Off"),
            RepeatMode::RepeatAll => ("media-playlist-repeat-symbolic", "Repeat: All"),
            RepeatMode::RepeatOne => ("media-playlist-repeat-song-symbolic", "Repeat: One"),
        };
        self.repeat_button.set_icon_name(icon);
        self.repeat_button.set_tooltip_text(Some(label));
        let wname = match state.repeat {
            RepeatMode::Off => "repeat-off",
            RepeatMode::RepeatAll => "repeat-all",
            RepeatMode::RepeatOne => "repeat-one",
        };
        self.repeat_button.set_widget_name(wname);
        self.repeat_button
            .set_opacity(if state.repeat == RepeatMode::Off {
                0.5
            } else {
                1.0
            });
    }
}

fn format_time(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}
