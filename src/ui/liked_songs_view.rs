//! Liked Songs view — replaces the center content area when the user clicks
//! "Liked Songs" in the library sidebar. Shows all liked songs with a
//! "Show More" button for lazy display (20 tracks at a time).

use crate::liked_songs::fetch_liked_songs;
use crate::search::Track;
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

const PAGE_SIZE: usize = 20;

pub struct LikedSongsView {
    pub widget: gtk::Box,
}

impl LikedSongsView {
    pub fn new(
        cookies_path: PathBuf,
        on_select: impl Fn(Track) + 'static + Clone,
        on_back: impl Fn() + 'static + Clone,
    ) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // --- Header ---
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.set_margin_top(16);
        header.set_margin_bottom(8);
        header.set_margin_start(20);
        header.set_margin_end(20);

        let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
        back_button.add_css_class("flat");
        back_button.set_tooltip_text(Some("Back to Home"));
        back_button.connect_clicked(move |_| on_back());

        let title = gtk::Label::new(Some("Liked Songs"));
        title.add_css_class("title-1");
        title.set_hexpand(true);
        title.set_halign(gtk::Align::Start);

        header.append(&back_button);
        header.append(&title);
        widget.append(&header);

        // --- Track list ---
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.set_margin_start(12);
        list.set_margin_end(12);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_child(Some(&list));
        widget.append(&scrolled);

        // --- Show More button ---
        let show_more = gtk::Button::with_label("Show More");
        show_more.add_css_class("pill");
        show_more.set_margin_top(8);
        show_more.set_margin_bottom(16);
        show_more.set_halign(gtk::Align::Center);
        widget.append(&show_more);

        // --- State ---
        let all_tracks: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));
        let displayed: Rc<Cell<usize>> = Rc::new(Cell::new(0));

        // --- Loading state ---
        let spinner = gtk::Spinner::new();
        spinner.set_spinning(true);
        spinner.set_size_request(32, 32);
        let loading_label = gtk::Label::new(Some("Loading your liked songs\u{2026}"));
        loading_label.add_css_class("dim-label");
        let loading_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        loading_box.set_valign(gtk::Align::Center);
        loading_box.set_halign(gtk::Align::Center);
        loading_box.set_margin_top(60);
        loading_box.append(&spinner);
        loading_box.append(&loading_label);
        list.append(&loading_box);

        // --- Wire row activation (works for all rows, current and future) ---
        {
            let all_tracks = all_tracks.clone();
            let on_select = on_select.clone();
            list.connect_row_activated(move |_list, row| {
                let index = row.index() as usize;
                if let Some(track) = all_tracks.borrow().get(index).cloned() {
                    on_select(track);
                }
            });
        }

        // --- Wire Show More button ---
        {
            let show_more = show_more.clone();
            let list = list.clone();
            let all_tracks = all_tracks.clone();
            let displayed = displayed.clone();
            let on_select = on_select.clone();
            show_more.connect_clicked(move |btn| {
                append_page(&list, &all_tracks, &displayed, &on_select);
                let has_more = displayed.get() < all_tracks.borrow().len();
                btn.set_visible(has_more);
            });
        }

        // --- Fetch on background thread ---
        let (sender, receiver) = async_channel::bounded::<Result<Vec<Track>, String>>(1);
        let cookies = cookies_path;
        std::thread::spawn(move || {
            let result = fetch_liked_songs(&cookies).map_err(|e| format!("{e}"));
            let _ = sender.send_blocking(result);
        });

        let list_clone = list.clone();
        let all_tracks_clone = all_tracks.clone();
        let displayed_clone = displayed.clone();
        let show_more_clone = show_more.clone();

        glib::spawn_future_local(async move {
            let Ok(result) = receiver.recv().await else {
                return;
            };

            // Remove loading state.
            while let Some(child) = list_clone.first_child() {
                list_clone.remove(&child);
            }

            match result {
                Ok(tracks) if tracks.is_empty() => {
                    let label = gtk::Label::new(Some("No liked songs found."));
                    label.add_css_class("dim-label");
                    list_clone.append(&label);
                    show_more_clone.set_visible(false);
                }
                Ok(tracks) => {
                    *all_tracks_clone.borrow_mut() = tracks;
                    displayed_clone.set(0);

                    // Show first page.
                    append_page(&list_clone, &all_tracks_clone, &displayed_clone, &on_select);

                    // Show/hide "Show More" based on whether more tracks exist.
                    let has_more = displayed_clone.get() < all_tracks_clone.borrow().len();
                    show_more_clone.set_visible(has_more);
                }
                Err(e) => {
                    let label = gtk::Label::new(Some(&format!("Failed to load liked songs: {e}")));
                    label.add_css_class("dim-label");
                    label.set_wrap(true);
                    list_clone.append(&label);
                    show_more_clone.set_visible(false);
                }
            }
        });

        Self { widget }
    }
}

/// Appends the next PAGE_SIZE tracks from `all_tracks` to `list`.
fn append_page(
    list: &gtk::ListBox,
    all_tracks: &Rc<RefCell<Vec<Track>>>,
    displayed: &Rc<Cell<usize>>,
    _on_select: &(impl Fn(Track) + Clone),
) {
    let start = displayed.get();
    let tracks = all_tracks.borrow();
    let end = (start + PAGE_SIZE).min(tracks.len());

    for track in tracks[start..end].iter() {
        list.append(&thumbnail_widget::build_track_row(track));
    }

    drop(tracks);
    displayed.set(end);
}
