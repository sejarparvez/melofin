//! GTK UI, split into its own module tree so it can grow (top bar, home
//! view, library view, ...) without turning into one giant file.
//!
//! `src/bin/ui_shell.rs` is just a thin entry point that calls [`run`].

pub mod top_bar;

use crate::player::{self, PlayerCommand};
use crate::search::{Track, search};
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

pub const APP_ID: &str = "dev.melofin.Melofin";

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    register_actions(&app);
    app.connect_activate(build_ui);
    app.run()
}

/// Actions backing the overflow menu. `quit` is the important one — with no
/// titlebar/close button, this menu is the only way to exit the app short
/// of a WM keybind.
fn register_actions(app: &adw::Application) {
    let quit_action = gio::SimpleAction::new("quit", None);
    quit_action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| app.quit()
    ));
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<primary>q"]);

    // Stub — no preferences UI/backing store yet.
    let preferences_action = gio::SimpleAction::new("preferences", None);
    preferences_action.connect_activate(|_, _| {
        tracing::info!("Preferences requested, but there's nothing to show yet");
    });
    app.add_action(&preferences_action);

    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| show_about_dialog(&app)
    ));
    app.add_action(&about_action);
}

fn show_about_dialog(app: &adw::Application) {
    let about = adw::AboutWindow::builder()
        .application_name("Melofin")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("sejarparvez")
        .license_type(gtk::License::Gpl30)
        .build();
    if let Some(window) = app.active_window() {
        about.set_transient_for(Some(&window));
    }
    about.present();
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("could not connect to a display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(app: &adw::Application) {
    load_css();

    let top_bar = top_bar::build_top_bar();

    let results_list = gtk::ListBox::new();
    results_list.set_selection_mode(gtk::SelectionMode::None);
    results_list.add_css_class("boxed-list");
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
    content.append(&top_bar.root);
    content.append(&scroller);
    content.append(&now_playing_label);

    // WindowHandle makes the whole bar a drag target, so the window stays
    // movable if it's ever floated in Hyprland instead of tiled. A no-op
    // when tiled.
    let handle = gtk::WindowHandle::new();
    handle.set_child(Some(&content));

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Melofin"));
    window.set_default_width(480);
    window.set_default_height(600);
    window.set_content(Some(&handle));
    window.set_decorated(false); // no CSD — Hyprland doesn't need it

    let player_handle = player::spawn_player_thread();

    // Tracks currently shown in `results_list`, in the same order as the
    // rows, so a clicked row's index tells us which Track to play.
    let current_tracks: Rc<RefCell<Vec<Track>>> = Rc::new(RefCell::new(Vec::new()));

    wire_search(&top_bar.search_entry, &results_list, &current_tracks);
    wire_playback(&results_list, &current_tracks, &player_handle.commands);
    wire_now_playing(player_handle.state, now_playing_label);

    window.present();
}

fn wire_search(
    search_entry: &gtk::SearchEntry,
    results_list: &gtk::ListBox,
    current_tracks: &Rc<RefCell<Vec<Track>>>,
) {
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
}

fn wire_playback(
    results_list: &gtk::ListBox,
    current_tracks: &Rc<RefCell<Vec<Track>>>,
    commands: &async_channel::Sender<PlayerCommand>,
) {
    let tracks_for_play = current_tracks.clone();
    let commands = commands.clone();
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
}

fn wire_now_playing(state_rx: async_channel::Receiver<player::PlayerState>, label: gtk::Label) {
    glib::spawn_future_local(async move {
        while let Ok(state) = state_rx.recv().await {
            let text = if state.paused {
                format!("Paused: {} — {}", state.title, state.artist)
            } else {
                format!("Now Playing: {} — {}", state.title, state.artist)
            };
            label.set_label(&text);
        }
    });
}

/// Removes every row currently in `list`.
fn clear_list(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
}
