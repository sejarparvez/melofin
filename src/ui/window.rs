use crate::player::{self, PlayerCommand};
use crate::ui::home_view::HomeView;
use crate::ui::library_sidebar::LibrarySidebar;
use crate::ui::now_playing_panel::NowPlayingPanel;
use crate::ui::player_bar::PlayerBar;
use crate::ui::search_view::SearchView;
use crate::ui::top_bar::build_top_bar;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;

const APP_ID: &str = "dev.melofin.Melofin";

/// Names for the two pages in the `gtk::Stack` swapped between Home and
/// Search — see `build_ui`.
const PAGE_HOME: &str = "home";
const PAGE_SEARCH: &str = "search";

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

/// Loads `ui/style.css` and registers it for the default display. Was
/// previously defined but never actually loaded — every `add_css_class`
/// call elsewhere in `ui/` only takes effect once this runs.
fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("no default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(app: &adw::Application) {
    load_css();
    register_app_actions(app);

    let top_bar = build_top_bar();

    // Background player thread: owns tokio + the mpv subprocess + MPRIS.
    // Every other widget only ever talks to it through `handle.commands` /
    // `handle.state` — see src/player.rs.
    let handle = player::spawn_player_thread();

    // Shared by both Home and Search cards: sends a track to the player,
    // except placeholder home cards (empty `url` — see `home_view.rs`),
    // which are scaffolding until a real home-feed source exists.
    let commands = handle.commands.clone();
    let play_track = move |track: crate::search::Track| {
        if track.url.is_empty() {
            tracing::debug!("ignoring click on placeholder home card: {}", track.title);
            return;
        }
        let _ = commands.send_blocking(PlayerCommand::Play(track));
    };

    let home_view = HomeView::new(play_track.clone());
    // The search entry lives in the top bar (`top_bar.search_entry`), not
    // here — see ui/search_view.rs and ui/top_bar.rs.
    let search_view = SearchView::new(&top_bar.search_entry, play_track);

    let player_bar = PlayerBar::new(handle.commands.clone());

    // Home is the default page; searching switches to Search (there's no
    // back button yet — see top_bar.rs — so the Home button is the only
    // way back for now).
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&home_view.widget, Some(PAGE_HOME));
    stack.add_named(&search_view.widget, Some(PAGE_SEARCH));
    stack.set_visible_child_name(PAGE_HOME);

    {
        let stack = stack.clone();
        top_bar.search_entry.connect_activate(move |_| {
            stack.set_visible_child_name(PAGE_SEARCH);
        });
    }
    {
        let stack = stack.clone();
        top_bar.home_button.connect_clicked(move |_| {
            stack.set_visible_child_name(PAGE_HOME);
        });
    }

    // Left/right panels are always visible (matching Spotify, which keeps
    // its library sidebar around across views) rather than toggled per
    // page — there's only one center page worth hiding them for so far.
    let library_sidebar = LibrarySidebar::new();
    let now_playing_panel = NowPlayingPanel::new();

    let middle_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    middle_row.append(&library_sidebar.widget);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&stack);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&now_playing_panel.widget);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&top_bar.root);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&middle_row);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&player_bar.widget);

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Melofin"));
    // Wide enough by default to show both sidebars alongside the center
    // page — the old 480x640 default was sized for a single-pane search
    // view, not a 3-column layout.
    window.set_default_width(1100);
    window.set_default_height(720);
    window.set_content(Some(&content));

    let state_rx = handle.state;
    glib::spawn_future_local(async move {
        while let Ok(state) = state_rx.recv().await {
            player_bar.update(&state);
            now_playing_panel.update(&state);
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
