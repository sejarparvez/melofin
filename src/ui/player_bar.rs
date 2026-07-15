use crate::player::{PlayerCommand, PlayerState};
use crate::queue::RepeatMode;
use crate::ui::thumbnail_widget::ThumbnailStack;
use adw::prelude::*;
use gtk::glib;
use std::cell::Cell;
use std::rc::Rc;

pub struct PlayerBar {
    pub widget: gtk::CenterBox,
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
    pub queue_button: gtk::Button,
}

impl PlayerBar {
    pub fn new(commands: async_channel::Sender<PlayerCommand>) -> Self {
        let widget = gtk::CenterBox::new();
        widget.add_css_class("toolbar");
        widget.set_margin_start(12);
        widget.set_margin_end(12);
        widget.set_margin_top(8);
        widget.set_margin_bottom(8);

        // ---------------------------------------------------------------
        // Left: album art + track info
        // ---------------------------------------------------------------
        let left_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        left_box.set_valign(gtk::Align::Center);

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
        title_label.add_css_class("heading");
        let artist_label = gtk::Label::new(Some(""));
        artist_label.set_halign(gtk::Align::Start);
        artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist_label.set_max_width_chars(22);
        artist_label.add_css_class("dim-label");
        info_box.append(&title_label);
        info_box.append(&artist_label);

        left_box.append(&art_frame);
        left_box.append(&info_box);

        // ---------------------------------------------------------------
        // Center: shuffle / prev / play-pause / next / repeat + seek row
        // ---------------------------------------------------------------
        let center_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        center_box.set_hexpand(true);
        center_box.set_valign(gtk::Align::Center);

        let transport_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        transport_box.set_halign(gtk::Align::Center);

        // Shuffle button — tracks its own toggle state via a Cell so it can
        // flip between active/inactive without needing ToggleButton.
        let shuffle_active = Rc::new(Cell::new(false));
        let shuffle_button = gtk::Button::from_icon_name("media-playlist-shuffle-symbolic");
        shuffle_button.add_css_class("flat");
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
        prev_button.add_css_class("flat");
        prev_button.set_tooltip_text(Some("Previous"));
        {
            let commands = commands.clone();
            prev_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::Previous);
            });
        }

        // Play/Pause button
        let play_pause_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        play_pause_button.add_css_class("circular");
        {
            let commands = commands.clone();
            play_pause_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::TogglePause);
            });
        }

        // Next button
        let next_button = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next_button.add_css_class("flat");
        next_button.set_tooltip_text(Some("Next"));
        {
            let commands = commands.clone();
            next_button.connect_clicked(move |_| {
                let _ = commands.send_blocking(PlayerCommand::Next);
            });
        }

        // Repeat button: cycles Off -> RepeatAll -> RepeatOne -> Off on click.
        let repeat_button = gtk::Button::from_icon_name("media-playlist-repeat-symbolic");
        repeat_button.add_css_class("flat");
        repeat_button.set_tooltip_text(Some("Repeat: Off"));
        {
            let commands = commands.clone();
            repeat_button.connect_clicked(move |btn| {
                // Read current state from the button's name (set on each update).
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
                glib::Propagation::Proceed
            });
        }

        seek_row.append(&elapsed_label);
        seek_row.append(&seek_scale);
        seek_row.append(&remaining_label);

        center_box.append(&transport_box);
        center_box.append(&seek_row);

        // ---------------------------------------------------------------
        // Right: queue / volume
        // ---------------------------------------------------------------
        let right_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        right_box.set_valign(gtk::Align::Center);
        right_box.set_halign(gtk::Align::End);

        let queue_button = gtk::Button::from_icon_name("view-list-symbolic");
        queue_button.add_css_class("flat");
        queue_button.set_tooltip_text(Some("Queue"));

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

        right_box.append(&queue_button);
        right_box.append(&volume_icon);
        right_box.append(&volume_scale);

        widget.set_start_widget(Some(&left_box));
        widget.set_center_widget(Some(&center_box));
        widget.set_end_widget(Some(&right_box));

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
        // Use the widget name as a cheap state holder for the click cycle.
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
