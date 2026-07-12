//! Results list, driven by the search entry that lives in the top bar
//! (`ui::top_bar`) instead of its own — the top bar owns the visible
//! search pill (Spotify puts it there), so this widget just needs to react
//! to it.

use crate::search::{Track, search};
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use gtk::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

pub struct SearchView {
    /// Top-level widget: just the scrolled results list — the search entry
    /// itself is `top_bar::TopBar::search_entry`, owned and displayed there.
    pub widget: gtk::ScrolledWindow,
}

impl SearchView {
    /// `search_entry` is the entry built by `top_bar::build_top_bar()` —
    /// this only attaches behavior to it, it doesn't create or display one
    /// of its own. `on_select` fires when the user activates (double-clicks
    /// / presses Enter on) a result row, with the corresponding `Track`.
    pub fn new(search_entry: &gtk::SearchEntry, on_select: impl Fn(Track) + 'static) -> Self {
        let results_list = gtk::ListBox::new();
        results_list.set_selection_mode(gtk::SelectionMode::None);
        results_list.add_css_class("boxed-list"); // native Adwaita grouped-list styling
        results_list.set_margin_start(12);
        results_list.set_margin_end(12);
        results_list.set_margin_top(12);
        results_list.set_margin_bottom(12);

        let widget = gtk::ScrolledWindow::new();
        widget.set_vexpand(true);
        widget.set_child(Some(&results_list));

        // Tracks currently shown in `results_list`, in the same order as the
        // rows, so a clicked row's index tells us which Track to play.
        let current_tracks: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));

        let list_for_search = results_list.clone();
        let tracks_for_search = current_tracks.clone();
        search_entry.connect_activate(move |entry| {
            let query = entry.text().to_string();
            if query.trim().is_empty() {
                return;
            }

            let list = list_for_search.clone();
            clear_list(&list);
            list.append(&gtk::Label::new(Some("Searching…")));

            let (sender, receiver) = async_channel::bounded::<anyhow::Result<Vec<Track>>>(1);
            thread::spawn(move || {
                let _ = sender.send_blocking(search(&query));
            });

            let list = list_for_search.clone();
            let tracks_slot = tracks_for_search.clone();
            glib::spawn_future_local(async move {
                let Ok(result) = receiver.recv().await else {
                    return;
                };
                clear_list(&list);
                match result {
                    Ok(tracks) if tracks.is_empty() => {
                        tracks_slot.borrow_mut().clear();
                        list.append(&gtk::Label::new(Some("No results.")));
                    }
                    Ok(tracks) => {
                        for track in &tracks {
                            let row = adw::ActionRow::new();
                            // Title/subtitle are parsed as Pango markup, so raw
                            // "&"/"<" in a video title (common in mashup/remix
                            // titles) would otherwise break parsing.
                            row.set_title(&glib::markup_escape_text(&track.title));
                            row.set_subtitle(&glib::markup_escape_text(&track.artist));
                            // ActionRow defaults to non-activatable (it's often
                            // used as a static row elsewhere in GTK/Adwaita
                            // apps), so row-activated never fires without this.
                            row.set_activatable(true);

                            let thumbnail = gtk::Picture::new();
                            thumbnail.set_size_request(40, 40);
                            thumbnail.set_content_fit(gtk::ContentFit::Cover);
                            row.add_prefix(&thumbnail);
                            if !track.thumbnail_url.is_empty() {
                                thumbnail_widget::spawn_fetch(
                                    track.thumbnail_url.clone(),
                                    40,
                                    move |texture| thumbnail.set_paintable(Some(&texture)),
                                );
                            }

                            list.append(&row);
                        }
                        *tracks_slot.borrow_mut() = tracks;
                    }
                    Err(e) => {
                        tracks_slot.borrow_mut().clear();
                        list.append(&gtk::Label::new(Some(&format!("Search failed: {e}"))));
                    }
                }
            });
        });

        let tracks_for_play = current_tracks.clone();
        results_list.connect_row_activated(move |_list, row| {
            let index = row.index();
            if index < 0 {
                return;
            }
            let Some(track) = tracks_for_play.borrow().get(index as usize).cloned() else {
                return;
            };
            on_select(track);
        });

        Self { widget }
    }
}

/// Removes every row currently in `list`.
fn clear_list(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
}
