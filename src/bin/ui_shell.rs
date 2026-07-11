//! Step 5: wire the results list to actual playback via a background
//! "player service" thread (`melofin::player`) that owns the tokio
//! runtime, mpv, and MPRIS. The GTK thread never touches tokio directly —
//! it sends `PlayerCommand`s in and receives `PlayerState` updates out
//! over plain channels, the same shape as the search wiring from before.

use adw::prelude::*;
use gtk::glib;
use melofin::player::{self, PlayerCommand};
use melofin::search::{Track, search};
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

const APP_ID: &str = "dev.melofin.Melofin";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let header = adw::HeaderBar::new();

    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search YouTube Music…"));
    search_entry.set_margin_start(12);
    search_entry.set_margin_end(12);
    search_entry.set_margin_top(12);
    search_entry.set_margin_bottom(6);

    let results_list = gtk::ListBox::new();
    results_list.set_selection_mode(gtk::SelectionMode::None);
    results_list.add_css_class("boxed-list"); // native Adwaita grouped-list styling
    results_list.set_margin_start(12);
    results_list.set_margin_end(12);
    results_list.set_margin_bottom(12);

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&results_list));

    let now_playing_label = gtk::Label::new(Some("Nothing playing"));
    now_playing_label.set_margin_start(12);
    now_playing_label.set_margin_end(12);
    now_playing_label.set_margin_bottom(12);
    now_playing_label.set_halign(gtk::Align::Start);
    now_playing_label.add_css_class("dim-label");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&header);
    content.append(&search_entry);
    content.append(&scroller);
    content.append(&now_playing_label);

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Melofin"));
    window.set_default_width(480);
    window.set_default_height(600);
    window.set_content(Some(&content));

    // Background player thread: owns tokio + the mpv subprocess + MPRIS.
    // The GTK thread only ever talks to it through `handle.commands` /
    // `handle.state`.
    let handle = player::spawn_player_thread();

    // Tracks currently shown in `results_list`, in the same order as the
    // rows, so a clicked row's index tells us which Track to play.
    let current_tracks: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));

    // Search wiring — unchanged from the previous step.
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

    // Row click -> play (new). Row index lines up with `current_tracks`
    // since rows are only ever rebuilt from that same Vec, in the same
    // order.
    let tracks_for_play = current_tracks.clone();
    let commands = handle.commands.clone();
    results_list.connect_row_activated(move |_list, row| {
        let index = row.index();
        if index < 0 {
            return;
        }
        let Some(track) = tracks_for_play.borrow().get(index as usize).cloned() else {
            return;
        };
        let _ = commands.send_blocking(PlayerCommand::Play(track));
    });

    // Player state -> now-playing label.
    let state_rx = handle.state;
    glib::spawn_future_local(async move {
        while let Ok(state) = state_rx.recv().await {
            let text = if state.paused {
                format!("Paused: {} — {}", state.title, state.artist)
            } else {
                format!("Now Playing: {} — {}", state.title, state.artist)
            };
            now_playing_label.set_label(&text);
        }
    });

    window.present();
}

/// Removes every row currently in `list`.
fn clear_list(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
}
