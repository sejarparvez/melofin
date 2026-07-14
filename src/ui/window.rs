use crate::auth::{AuthManager, AuthState};
use crate::player::{self, PlayerCommand};
use crate::ui::home_view::HomeView;
use crate::ui::library_sidebar::LibrarySidebar;
use crate::ui::liked_songs_view::LikedSongsView;
use crate::ui::login_dialog;
use crate::ui::now_playing_panel::NowPlayingPanel;
use crate::ui::player_bar::PlayerBar;
use crate::ui::search_view::SearchView;
use crate::ui::top_bar::build_top_bar;
use crate::user::UserProfile;
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

/// Loads `ui/style.css` and registers it for the default display.
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
/// created with `0700` permissions since it holds the cookies file.
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

    let data_dir = init_data_dir();
    let auth = AuthManager::new(&data_dir);

    let top_bar = build_top_bar();

    // Show cached profile immediately, or the logged-out state.
    match auth.current_state() {
        AuthState::LoggedIn => {
            if let Some(profile) = UserProfile::load(&data_dir) {
                top_bar.set_user_profile(&profile);
            } else {
                let profile = UserProfile {
                    name: "YouTube Music".to_string(),
                    ..Default::default()
                };
                top_bar.set_user_profile(&profile);
            }
            // Validate the session in the background. If it expired,
            // flip back to logged out.
            let auth_val = auth.clone();
            let data_dir_val = data_dir.clone();
            let top_bar_val = top_bar.clone();
            login_dialog::run_auth(
                move || {
                    let auth = auth_val;
                    async move { auth.validate().await }
                },
                move |result| {
                    if result.is_err() {
                        tracing::warn!("session expired on startup, clearing");
                        UserProfile::remove_cache(&data_dir_val);
                        top_bar_val.set_logged_out();
                    }
                },
            );
        }
        AuthState::LoggedOut => {
            top_bar.set_logged_out();
        }
    }

    // -- Account popover: logout button ----------------------------------------

    {
        let auth = auth.clone();
        let data_dir = data_dir.clone();
        let top_bar = top_bar.clone();
        top_bar
            .logout_button
            .clone()
            .connect_clicked(move |button| {
                button.set_sensitive(false);
                let auth = auth.clone();
                let data_dir = data_dir.clone();
                let top_bar = top_bar.clone();
                let button = button.clone();
                login_dialog::run_auth(
                    move || {
                        let auth = auth.clone();
                        async move { auth.logout().await }
                    },
                    move |result| {
                        button.set_sensitive(true);
                        if let Err(e) = result {
                            tracing::warn!("logout failed: {e}");
                            return;
                        }
                        UserProfile::remove_cache(&data_dir);
                        top_bar.set_logged_out();
                    },
                );
            });
    }

    // -- Build the rest of the window ------------------------------------------

    let handle = player::spawn_player_thread();

    let commands = handle.commands.clone();
    let play_track = move |track: crate::search::Track| {
        if track.url.is_empty() {
            tracing::debug!("ignoring click on placeholder home card: {}", track.title);
            return;
        }
        let _ = commands.send_blocking(PlayerCommand::Play(track));
    };

    let home_cache_path = glib::user_cache_dir()
        .join("melofin")
        .join("home_feed.json");

    let history_path = glib::user_data_dir()
        .join("melofin")
        .join("play_history.jsonl");

    // Content stack switches between home feed and liked songs view.
    let content_stack = gtk::Stack::new();
    content_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    content_stack.set_transition_duration(200);

    let home_view = HomeView::new(
        auth.cookies_path().to_path_buf(),
        home_cache_path,
        history_path,
        play_track.clone(),
    );
    content_stack.add_named(&home_view.widget, Some("home"));
    content_stack.set_visible_child(&home_view.widget);

    let search_view = SearchView::new(&top_bar.search_entry, play_track.clone());
    let _search_view = search_view;

    let player_bar = PlayerBar::new(handle.commands.clone());
    let now_playing_panel = NowPlayingPanel::new();

    // Library sidebar: "Liked Songs" click switches the stack.
    // We create the sidebar after the stack so we can clone the stack into the callback.
    let content_stack_for_sidebar = content_stack.clone();
    let cookies_for_liked = auth.cookies_path().to_path_buf();
    let library_sidebar = LibrarySidebar::new({
        let stack = content_stack_for_sidebar.clone();
        let cookies = cookies_for_liked.clone();
        let play_track = play_track.clone();
        move || {
            // Create liked songs view on demand, add to stack, switch to it.
            let stack = stack.clone();
            let on_back = {
                let stack = stack.clone();
                move || {
                    stack.set_visible_child_name("home");
                }
            };
            let liked_view = LikedSongsView::new(cookies.clone(), play_track.clone(), on_back);
            liked_view.widget.set_hexpand(true);
            liked_view.widget.set_vexpand(true);
            // Remove old liked songs page if it exists.
            if let Some(old) = stack.child_by_name("liked") {
                stack.remove(&old);
            }
            stack.add_named(&liked_view.widget, Some("liked"));
            stack.set_visible_child(&liked_view.widget);
        }
    });

    content_stack.set_hexpand(true);
    content_stack.set_vexpand(true);

    let middle_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    middle_row.append(&library_sidebar.widget);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&content_stack);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&now_playing_panel.widget);
    middle_row.set_hexpand(true);
    middle_row.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&top_bar.root);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&middle_row);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&player_bar.widget);

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&content));

    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Melofin"));
    window.set_default_width(1100);
    window.set_default_height(720);
    window.set_content(Some(&toast_overlay));

    // -- Account popover: login button (needs window + toast_overlay) ----------

    {
        let window = window.clone();
        let toast_overlay = toast_overlay.clone();
        let auth = auth.clone();
        let data_dir = data_dir.clone();
        let top_bar = top_bar.clone();
        top_bar.login_button.clone().connect_clicked(move |_| {
            let window = window.clone();
            let toast_overlay = toast_overlay.clone();
            let auth = auth.clone();
            let data_dir = data_dir.clone();
            let top_bar = top_bar.clone();

            let auth_inner = auth.clone();
            let on_state_changed = move |state: AuthState| match state {
                AuthState::LoggedIn => {
                    let data_dir = data_dir.clone();
                    let auth = auth_inner.clone();
                    let top_bar = top_bar.clone();
                    login_dialog::run_auth(
                        move || {
                            let auth = auth.clone();
                            async move {
                                let cookies = auth.cookies_path().to_path_buf();
                                tokio::task::spawn_blocking(move || {
                                    UserProfile::fetch_from_cookies(&cookies)
                                })
                                .await
                                .unwrap_or_else(|_| UserProfile::guest())
                            }
                        },
                        move |profile| {
                            let _ = profile.save(&data_dir);
                            top_bar.set_user_profile(&profile);
                        },
                    );
                }
                AuthState::LoggedOut => {
                    UserProfile::remove_cache(&data_dir);
                    top_bar.set_logged_out();
                }
            };

            login_dialog::present(&window, toast_overlay, auth, on_state_changed);
        });
    }

    // -- Player state updates --------------------------------------------------

    let state_rx = handle.state;
    glib::spawn_future_local(async move {
        while let Ok(state) = state_rx.recv().await {
            player_bar.update(&state);
            now_playing_panel.update(&state);
        }
    });

    window.present();
}

/// Registers the `app.*` actions the top bar's overflow menu references.
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
