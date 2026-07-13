use crate::auth::{AuthManager, AuthState};
use crate::player::{self, PlayerCommand};
use crate::ui::home_view::HomeView;
use crate::ui::library_sidebar::LibrarySidebar;
use crate::ui::login_dialog;
use crate::ui::now_playing_panel::NowPlayingPanel;
use crate::ui::player_bar::PlayerBar;
use crate::ui::search_view::SearchView;
use crate::ui::top_bar::{build_top_bar, set_account_state};
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;

const APP_ID: &str = "dev.melofin.Melofin";

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

/// Melofin's XDG data dir (`~/.local/share/melofin` on most Linux setups),
/// created with `0700` permissions since it holds the cookies file
/// (`AuthManager` sets the file itself to `0600` — see `auth.rs`). Returns
/// the path regardless of whether creation/chmod succeeded; `AuthManager`
/// surfaces any resulting IO failure itself the first time it actually
/// tries to read or write through this path.
fn init_data_dir() -> std::path::PathBuf {
    let dir = glib::user_data_dir().join("melofin");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::error!("failed to create data dir {}: {e}", dir.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o700);
            let _ = std::fs::set_permissions(&dir, perms);
        }
    }
    dir
}

fn build_ui(app: &adw::Application) {
    load_css();
    register_app_actions(app);

    let auth = AuthManager::new(&init_data_dir());

    let top_bar = build_top_bar();
    set_account_state(&top_bar.account_button, auth.current_state());

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
    // here. SearchView no longer has a visible widget of its own — results
    // now show in a `gtk::Popover` parented to `top_bar.search_entry`
    // (see ui/search_view.rs), so home stays on screen underneath instead
    // of being swapped out.
    let search_view = SearchView::new(&top_bar.search_entry, play_track);
    // Nothing else references search_view directly, but it must stay alive
    // for the popover and its signal connections to keep working.
    let _search_view = search_view;

    let player_bar = PlayerBar::new(handle.commands.clone());

    // Left/right panels are always visible (matching Spotify, which keeps
    // its library sidebar around across views).
    let library_sidebar = LibrarySidebar::new();
    let now_playing_panel = NowPlayingPanel::new();

    let middle_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    middle_row.append(&library_sidebar.widget);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&home_view.widget);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&now_playing_panel.widget);
    middle_row.set_hexpand(true);
    middle_row.set_vexpand(true);
    home_view.widget.set_hexpand(true);
    home_view.widget.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&top_bar.root);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&middle_row);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&player_bar.widget);

    // Wraps everything so `login_dialog` (and anything else later) has
    // somewhere to show toasts — a toast added to a widget that isn't part
    // of a `ToastOverlay`'s tree silently does nothing.
    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&content));

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Melofin"));
    // Wide enough by default to show both sidebars alongside the center
    // page — the old 480x640 default was sized for a single-pane search
    // view, not a 3-column layout.
    window.set_default_width(1100);
    window.set_default_height(720);
    window.set_content(Some(&toast_overlay));

    {
        let window = window.clone();
        let toast_overlay = toast_overlay.clone();
        let auth = auth.clone();
        let account_button = top_bar.account_button.clone();
        top_bar.account_button.connect_clicked(move |_| {
            let on_state_changed = {
                let account_button = account_button.clone();
                move |state: AuthState| {
                    set_account_state(&account_button, state);
                }
            };
            login_dialog::present(
                &window,
                toast_overlay.clone(),
                auth.clone(),
                on_state_changed,
            );
        });
    }

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
