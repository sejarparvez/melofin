use crate::player::{self, PlayerCommand};
use crate::ui::player_bar::PlayerBar;
use crate::ui::search_view::SearchView;
use crate::ui::top_bar::build_top_bar;
use adw::prelude::*;
use gtk::gio;
use gtk::glib;

const APP_ID: &str = "dev.melofin.Melofin";

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    register_app_actions(app);

    let top_bar = build_top_bar();

    // Background player thread: owns tokio + the mpv subprocess + MPRIS.
    // Every other widget only ever talks to it through `handle.commands` /
    // `handle.state` — see src/player.rs.
    let handle = player::spawn_player_thread();

    let commands_for_search = handle.commands.clone();
    // The search entry lives in the top bar (`top_bar.search_entry`), not
    // here — see ui/search_view.rs and ui/top_bar.rs.
    let search_view = SearchView::new(&top_bar.search_entry, move |track| {
        let _ = commands_for_search.send_blocking(PlayerCommand::Play(track));
    });

    let player_bar = PlayerBar::new(handle.commands.clone());

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&top_bar.root);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&search_view.widget);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&player_bar.widget);

    // search_view.widget (a ScrolledWindow) already has vexpand(true), so
    // it fills the space between the top bar and the player bar.

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Melofin"));
    window.set_default_width(480);
    window.set_default_height(640);
    window.set_content(Some(&content));

    let state_rx = handle.state;
    glib::spawn_future_local(async move {
        while let Ok(state) = state_rx.recv().await {
            player_bar.update(&state);
        }
    });

    window.present();
}

/// Registers the `app.*` actions the top bar's overflow menu references
/// (`ui/top_bar.rs::overflow_menu`). `quit` is real from day one — it's the
/// only way to close the app without window-manager decorations (Hyprland
/// tiling setup has none) — and `about` is trivial to make real too.
/// `preferences` is intentionally left unregistered: with no action bound
/// to it, GTK automatically greys the menu item out rather than doing
/// nothing silently, which is the honest state until a preferences window
/// actually exists.
fn register_app_actions(app: &adw::Application) {
    let quit_action = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit_action.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);

    let about_action = gio::SimpleAction::new("about", None);
    {
        let app = app.clone();
        about_action.connect_activate(move |_, _| {
            let Some(window) = app.active_window() else {
                return;
            };
            let about = adw::AboutWindow::builder()
                .transient_for(&window)
                .application_name("Melofin")
                .application_icon("dev.melofin.Melofin")
                .developer_name("Melofin contributors")
                .version(env!("CARGO_PKG_VERSION"))
                .website("https://github.com/sejarparvez/melofin")
                .build();
            about.present();
        });
    }
    app.add_action(&about_action);
}
