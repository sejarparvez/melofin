//! Results popover, driven by the search entry that lives in the top bar
//! (`ui::top_bar`) instead of its own. Updates live as the user types via
//! `SearchEntry`'s `search-changed` signal, debounced 300ms so a fast
//! typist doesn't fire a search per keystroke.
//!
//! `autohide` is deliberately off: on Wayland a `gtk::Popover` with
//! autohide enabled opens as an `xdg_popup` surface, which the compositor
//! gives exclusive keyboard focus to — that's what was swallowing
//! keystrokes typed after the popover appeared. With autohide off (and
//! `can_focus(false)` so the popover itself never grabs focus either),
//! the search entry keeps keyboard input the whole time. Trade-off: the
//! popover no longer closes on an outside click, only on Escape, an empty
//! query, or picking a result — see the `EventControllerKey` below.

use crate::search::{Track, search};
use crate::ui::thumbnail_widget;
use adw::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

pub struct SearchView {
    pub popover: gtk::Popover,
}

const SKELETON_ROW_COUNT: usize = 5;
const DEBOUNCE_MS: u64 = 300;

impl SearchView {
    /// `search_entry` is the entry built by `top_bar::build_top_bar()` —
    /// this only attaches behavior to it, it doesn't create or display one
    /// of its own. `on_select` fires when the user activates a result row,
    /// with the corresponding `Track`.
    pub fn new(search_entry: &gtk::SearchEntry, on_select: impl Fn(Track) + 'static) -> Self {
        let results_list = gtk::ListBox::new();
        results_list.set_selection_mode(gtk::SelectionMode::None);
        results_list.add_css_class("boxed-list"); // native Adwaita grouped-list styling
        results_list.set_margin_start(12);
        results_list.set_margin_end(12);
        results_list.set_margin_top(12);
        results_list.set_margin_bottom(12);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_min_content_width(420);
        scrolled.set_min_content_height(300); // reserve space up front
        scrolled.set_max_content_height(600);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_child(Some(&results_list));

        let popover = gtk::Popover::new();
        popover.set_parent(search_entry);
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_has_arrow(false);
        popover.set_autohide(false); // avoid the Wayland xdg_popup keyboard grab
        popover.set_can_focus(false); // never take focus itself — entry keeps it throughout
        popover.add_css_class("search-popover");
        popover.set_child(Some(&scrolled));

        // Escape closes the popover, since autohide-on-outside-click is off.
        {
            let popover = popover.clone();
            let key_controller = gtk::EventControllerKey::new();
            key_controller.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    popover.popdown();
                }
                glib::Propagation::Proceed
            });
            search_entry.add_controller(key_controller);
        }

        // Tracks currently shown in `results_list`, in the same order as the
        // rows, so a clicked row's index tells us which Track to play.
        let current_tracks: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));
        // Bumped on every keystroke. A pending debounce timeout or an
        // in-flight search checks its captured generation against this
        // before acting — if they don't match, a newer keystroke has
        // superseded it, so it's a no-op instead of clobbering fresher work.
        let current_generation: Rc<Cell<u64>> = Rc::new(Cell::new(0));

        let list_for_search = results_list.clone();
        let tracks_for_search = current_tracks.clone();
        let popover_for_search = popover.clone();
        let generation_for_search = current_generation.clone();
        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            let generation = generation_for_search.get() + 1;
            generation_for_search.set(generation);

            if query.trim().is_empty() {
                popover_for_search.popdown();
                return;
            }

            // Skeleton + popover open immediately for responsiveness, but
            // the actual search request is debounced below.
            show_skeleton(&list_for_search);
            if !popover_for_search.is_visible() {
                popover_for_search.popup();
            }

            let list = list_for_search.clone();
            let tracks_slot = tracks_for_search.clone();
            let popover = popover_for_search.clone();
            let generation_check = generation_for_search.clone();
            glib::source::timeout_add_local_once(Duration::from_millis(DEBOUNCE_MS), move || {
                // Superseded by a later keystroke before the debounce elapsed.
                if generation_check.get() != generation {
                    return;
                }

                let (sender, receiver) = async_channel::bounded::<anyhow::Result<Vec<Track>>>(1);
                thread::spawn(move || {
                    let _ = sender.send_blocking(search(&query));
                });

                glib::spawn_future_local(async move {
                    let Ok(result) = receiver.recv().await else {
                        return;
                    };
                    // Superseded while the search itself was in flight.
                    if generation_check.get() != generation {
                        return;
                    }
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
                                // ActionRow defaults to non-activatable.
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
                    popover.queue_resize();
                });
            });
        });

        let tracks_for_play = current_tracks.clone();
        let popover_for_play = popover.clone();
        results_list.connect_row_activated(move |_list, row| {
            let index = row.index();
            if index < 0 {
                return;
            }
            let Some(track) = tracks_for_play.borrow().get(index as usize).cloned() else {
                return;
            };
            on_select(track);
            popover_for_play.popdown();
        });

        Self { popover }
    }
}

/// Removes every row currently in `list`.
fn clear_list(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
}

/// Replaces `list`'s contents with placeholder rows shaped like a real
/// result row (thumbnail block + two text-line blocks), pulsing via the
/// `.skeleton-block` CSS animation in style.css — shown while a search is
/// in flight instead of a plain "Searching…" label.
fn show_skeleton(list: &gtk::ListBox) {
    clear_list(list);
    for _ in 0..SKELETON_ROW_COUNT {
        let thumb = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        thumb.set_size_request(40, 40);
        thumb.add_css_class("skeleton-block");

        let title_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        title_bar.set_size_request(180, 12);
        title_bar.add_css_class("skeleton-block");

        let subtitle_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        subtitle_bar.set_size_request(120, 10);
        subtitle_bar.add_css_class("skeleton-block");

        let text_col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        text_col.set_valign(gtk::Align::Center);
        text_col.append(&title_bar);
        text_col.append(&subtitle_bar);

        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.append(&thumb);
        row_box.append(&text_col);

        list.append(&row_box);
    }
}
