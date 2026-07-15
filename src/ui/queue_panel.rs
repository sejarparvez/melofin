use crate::player::PlayerCommand;
use crate::player::QueueSnapshot;
use crate::search::Track;
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use std::cell::RefCell;

pub struct QueuePanel {
    pub widget: gtk::ScrolledWindow,
    list: gtk::ListBox,
    /// Cached queue so we can look up which track to play when a row is clicked.
    tracks: RefCell<Vec<Track>>,
}

impl QueuePanel {
    pub fn new(commands: async_channel::Sender<PlayerCommand>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let heading = gtk::Label::new(Some("Queue"));
        heading.add_css_class("heading");
        heading.set_halign(gtk::Align::Start);
        heading.set_margin_top(10);
        heading.set_margin_bottom(6);
        heading.set_margin_start(12);
        heading.set_margin_end(12);
        content.append(&heading);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_margin_start(4);
        list.set_margin_end(4);
        list.set_margin_bottom(12);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_min_content_width(220);
        scrolled.set_max_content_width(220);
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_child(Some(&list));
        content.append(&scrolled);

        let panel = Self {
            widget: scrolled,
            list,
            tracks: RefCell::new(Vec::new()),
        };

        // Click on a row -> play that track.
        {
            let tracks = panel.tracks.clone();
            let commands = commands.clone();
            panel.list.connect_row_activated(move |_list, row| {
                let index = row.index() as usize;
                if tracks.borrow().get(index).is_some() {
                    let _ = commands
                        .send_blocking(PlayerCommand::ReplaceQueue(tracks.borrow().clone(), index));
                }
            });
        }

        panel
    }

    pub fn update(&self, snapshot: &QueueSnapshot) {
        // Remember which track was highlighted so we can scroll to it.
        let previous_index = snapshot.current_index;

        // Replace the list contents.
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }

        if snapshot.tracks.is_empty() {
            let label = gtk::Label::new(Some("Queue is empty"));
            label.add_css_class("dim-label");
            label.set_margin_top(20);
            label.set_margin_bottom(20);
            self.list.append(&label);
            self.tracks.borrow_mut().clear();
            return;
        }

        *self.tracks.borrow_mut() = snapshot.tracks.clone();

        for (i, track) in snapshot.tracks.iter().enumerate() {
            let is_current = snapshot.current_index == Some(i);
            let row = build_queue_row(track, is_current);
            self.list.append(&row);
        }

        // Scroll to the current track.
        if let Some(idx) = previous_index
            && let Some(row) = self.list.row_at_index(idx as i32)
        {
            self.list.select_row(Some(&row));
            row.grab_focus();
        }
    }
}

fn build_queue_row(track: &Track, is_current: bool) -> gtk::ListBoxRow {
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row_box.set_margin_top(4);
    row_box.set_margin_bottom(4);
    row_box.set_margin_start(8);
    row_box.set_margin_end(8);

    if is_current {
        row_box.add_css_class("accent");
    }

    // Thumbnail
    let thumb: gtk::Widget = if track.thumbnail_url.is_empty() {
        let img = gtk::Image::from_icon_name("audio-x-generic-symbolic");
        img.set_pixel_size(20);
        img.set_halign(gtk::Align::Center);
        img.set_valign(gtk::Align::Center);
        img.set_size_request(36, 36);
        img.upcast()
    } else {
        let frame = gtk::Frame::new(None);
        frame.set_size_request(36, 36);
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(36, 36);
        frame.set_child(Some(&picture));
        let url = track.thumbnail_url.clone();
        thumbnail_widget::spawn_fetch(url, 36, move |tex| {
            picture.set_paintable(Some(&tex));
        });
        frame.upcast()
    };
    row_box.append(&thumb);

    // Text
    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_box.set_valign(gtk::Align::Center);
    text_box.set_hexpand(true);

    let title = gtk::Label::new(Some(&track.title));
    title.set_halign(gtk::Align::Start);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(18);
    if is_current {
        title.add_css_class("heading");
    }

    let artist = gtk::Label::new(Some(&track.artist));
    artist.set_halign(gtk::Align::Start);
    artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
    artist.set_max_width_chars(18);
    artist.add_css_class("dim-label");
    artist.add_css_class("caption");

    text_box.append(&title);
    text_box.append(&artist);
    row_box.append(&text_box);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&row_box));
    row.set_activatable(true);
    row
}
