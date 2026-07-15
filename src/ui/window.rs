use crate::auth::{AuthManager, AuthState};
use crate::detail_fetch;
use crate::player::{self, PlayerCommand};
use crate::search::{MediaKind, Track};
use crate::ui::detail_view::DetailView;
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
use std::cell::Cell;
use std::rc::Rc;

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
    let play_track = Rc::new(move |track: Track| {
        if track.url.is_empty() {
            tracing::debug!("ignoring click on placeholder home card: {}", track.title);
            return;
        }
        let _ = commands.send_blocking(PlayerCommand::Play(track));
    });

    let home_cache_path = glib::user_cache_dir()
        .join("melofin")
        .join("home_feed.json");

    let history_path = glib::user_data_dir()
        .join("melofin")
        .join("play_history.jsonl");

    // Content stack switches between home feed, liked songs, and detail views.
    let content_stack = gtk::Stack::new();
    content_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    content_stack.set_transition_duration(200);

    // Navigation state for detail views.
    let detail_counter = Rc::new(Cell::new(0usize));
    let cookies_for_detail = auth.cookies_path().to_path_buf();

    // on_select: navigate to detail page for any track/playlist/album.
    let content_stack_nav = content_stack.clone();
    let detail_counter_nav = detail_counter.clone();
    let cookies_nav = cookies_for_detail.clone();
    let play_track_for_select = play_track.clone();
    let on_select: Rc<dyn Fn(Track)> = Rc::new(move |track: Track| {
        eprintln!(
            "[on_select] title={}, artist={}, kind={:?}, url={}",
            track.title,
            track.artist,
            track.media_kind(),
            track.url
        );
        let stack = content_stack_nav.clone();
        let counter = detail_counter_nav.clone();
        let cookies = cookies_nav.clone();
        let play_track = play_track_for_select.clone();

        match track.media_kind() {
            MediaKind::Song => {
                // For songs, show a detail view with the song's own data.
                let name = format!("detail_{}", counter.get());
                counter.set(counter.get() + 1);

                let metadata = detail_fetch::DetailMetadata {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    thumbnail_url: track.thumbnail_url.clone(),
                    description: String::new(),
                    year: String::new(),
                    track_count: 1,
                };
                let detail = DetailView::new(
                    &metadata,
                    &[track],
                    play_track.clone(),
                    Rc::new({
                        let stack = stack.clone();
                        let name = name.clone();
                        move || {
                            if let Some(child) = stack.child_by_name(&name) {
                                stack.remove(&child);
                            }
                        }
                    }),
                );
                detail.widget.set_hexpand(true);
                detail.widget.set_vexpand(true);
                eprintln!("[on_select] Adding detail view as '{}'", name);
                stack.add_named(&detail.widget, Some(&name));
                eprintln!("[on_select] Set visible child to '{}'", name);
                stack.set_visible_child(&detail.widget);
            }
            MediaKind::Playlist | MediaKind::Album | MediaKind::Artist => {
                // For playlists/albums, fetch details from InnerTube.
                eprintln!("[on_select] Playlist/Album/Artist detected, browse_id={:?}", track.browse_id());
                let browse_id = match track.browse_id() {
                    Some(id) => id.to_string(),
                    None => return,
                };
                let name = format!("detail_{}", counter.get());
                counter.set(counter.get() + 1);

                // Show loading state.
                let loading = DetailView::loading();
                loading.widget.set_hexpand(true);
                loading.widget.set_vexpand(true);
                stack.add_named(&loading.widget, Some(&name));
                stack.set_visible_child(&loading.widget);

                // Fetch details on background thread.
                let (sender, receiver) =
                    async_channel::bounded::<anyhow::Result<detail_fetch::DetailResult>>(1);
                let fetch_cookies = cookies.clone();
                let fetch_browse_id = browse_id.clone();
                std::thread::spawn(move || {
                    let _ = sender.send_blocking(detail_fetch::fetch_detail(
                        &fetch_cookies,
                        &fetch_browse_id,
                    ));
                });

                let stack = stack.clone();
                let name_clone = name.clone();
                let play_track_clone = play_track.clone();
                glib::spawn_future_local(async move {
                    let Ok(result) = receiver.recv().await else {
                        eprintln!("[on_select] Detail fetch channel closed");
                        return;
                    };

                    // Remove loading state.
                    if let Some(child) = stack.child_by_name(&name_clone) {
                        stack.remove(&child);
                    }

                    match result {
                        Ok(detail) => {
                            eprintln!(
                                "[on_select] Detail fetch OK: title={}, tracks={}",
                                detail.metadata.title,
                                detail.tracks.len()
                            );
                            let on_back = Rc::new({
                                let stack = stack.clone();
                                let name = name_clone.clone();
                                move || {
                                    if let Some(child) = stack.child_by_name(&name) {
                                        stack.remove(&child);
                                    }
                                }
                            });
                            let detail_view = DetailView::new(
                                &detail.metadata,
                                &detail.tracks,
                                play_track_clone,
                                on_back,
                            );
                            detail_view.widget.set_hexpand(true);
                            detail_view.widget.set_vexpand(true);
                            stack.add_named(&detail_view.widget, Some(&name_clone));
                            stack.set_visible_child(&detail_view.widget);
                        }
                        Err(e) => {
                            eprintln!("[on_select] Detail fetch FAILED: {e}");
                            let on_retry: Rc<dyn Fn()> = Rc::new({
                                let stack = stack.clone();
                                let name = name_clone.clone();
                                move || {
                                    if let Some(child) = stack.child_by_name(&name) {
                                        stack.remove(&child);
                                    }
                                }
                            });
                            let err_view =
                                DetailView::error(&format!("Failed to load details: {e}"), on_retry);
                            err_view.widget.set_hexpand(true);
                            err_view.widget.set_vexpand(true);
                            stack.add_named(&err_view.widget, Some(&name_clone));
                            stack.set_visible_child(&err_view.widget);
                        }
                    }
                });
            }
        }
    });

    let home_view = HomeView::new(
        auth.cookies_path().to_path_buf(),
        home_cache_path,
        history_path,
        on_select.clone(),
        play_track.clone(),
    );
    content_stack.add_named(&home_view.widget, Some("home"));
    content_stack.set_visible_child(&home_view.widget);

    // Home button: navigate back to home view.
    {
        let stack = content_stack.clone();
        top_bar.home_button.connect_clicked(move |_| {
            stack.set_visible_child_name("home");
        });
    }

    let search_view = SearchView::new(&top_bar.search_entry, on_select.clone(), play_track.clone());
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
        let on_select = on_select.clone();
        let play_track = play_track.clone();
        move || {
            // Create liked songs view on demand, add to stack, switch to it.
            let stack = stack.clone();
            let on_back: Rc<dyn Fn()> = Rc::new({
                let stack = stack.clone();
                move || {
                    stack.set_visible_child_name("home");
                }
            });
            let liked_view = LikedSongsView::new(
                cookies.clone(),
                on_select.clone(),
                play_track.clone(),
                on_back,
            );
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
