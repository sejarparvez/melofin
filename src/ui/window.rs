use crate::auth::{AuthManager, AuthState};
use crate::detail_fetch;
use crate::player::{self, PlayerCommand, PlayerEvent};
use crate::search::{MediaKind, Track};
use crate::ui::detail_view::DetailView;
use crate::ui::home_view::HomeView;
use crate::ui::library_sidebar::LibrarySidebar;
use crate::ui::liked_songs_view::LikedSongsView;
use crate::ui::login_dialog;
use crate::ui::now_playing_panel::NowPlayingPanel;
use crate::ui::player_bar::PlayerBar;
use crate::ui::queue_panel::QueuePanel;
use crate::ui::search_view::SearchView;
use crate::ui::top_bar::build_top_bar;
use crate::user::UserProfile;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const APP_ID: &str = "dev.melofin.Melofin";

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("no default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

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

    // -- Player & queue --------------------------------------------------------

    let handle = player::spawn_player_thread();
    let commands = handle.commands.clone();

    // Play a single track, replacing the queue.
    let play_track = {
        let commands = commands.clone();
        Rc::new(move |track: Track| {
            if track.url.is_empty() {
                tracing::debug!("ignoring click on placeholder home card: {}", track.title);
                return;
            }
            let _ = commands.send_blocking(PlayerCommand::ReplaceQueue(vec![track], 0));
        })
    };

    // Play a list of tracks starting from the clicked index, replacing the queue.
    let play_from_list = {
        let commands = commands.clone();
        Rc::new(move |tracks: Vec<Track>, index: usize| {
            if tracks.is_empty() {
                return;
            }
            let _ = commands.send_blocking(PlayerCommand::ReplaceQueue(tracks, index));
        })
    };

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

    // Navigation history for back/forward support.
    let nav_history = Rc::new(RefCell::new(vec!["home".to_string()]));
    let nav_index = Rc::new(Cell::new(0usize));
    let detail_counter = Rc::new(Cell::new(0usize));
    let cookies_for_detail = auth.cookies_path().to_path_buf();

    let navigate_to = {
        let stack = content_stack.clone();
        let history = nav_history.clone();
        let index = nav_index.clone();
        let back_btn = top_bar.back_button.clone();
        let forward_btn = top_bar.forward_button.clone();
        Rc::new(move |name: &str| {
            let (i, len) = {
                let mut h = history.borrow_mut();
                let mut i = index.get();
                h.truncate(i + 1);
                if h.last().map(|s| s.as_str()) != Some(name) {
                    h.push(name.to_string());
                    i += 1;
                }
                index.set(i);
                (i, h.len())
            };
            stack.set_visible_child_name(name);
            back_btn.set_sensitive(i > 0);
            forward_btn.set_sensitive(i < len - 1);
        })
    };

    let go_back = {
        let stack = content_stack.clone();
        let history = nav_history.clone();
        let index = nav_index.clone();
        let back_btn = top_bar.back_button.clone();
        let forward_btn = top_bar.forward_button.clone();
        Rc::new(move || {
            let mut i = index.get();
            if i > 0 {
                i -= 1;
                index.set(i);
                let h = history.borrow();
                let target = h[i].clone();
                let len = h.len();
                drop(h);
                stack.set_visible_child_name(&target);
                back_btn.set_sensitive(i > 0);
                forward_btn.set_sensitive(i < len - 1);
            }
        })
    };

    let go_forward = {
        let stack = content_stack.clone();
        let history = nav_history.clone();
        let index = nav_index.clone();
        let back_btn = top_bar.back_button.clone();
        let forward_btn = top_bar.forward_button.clone();
        Rc::new(move || {
            let mut i = index.get();
            let h = history.borrow();
            if i < h.len() - 1 {
                i += 1;
                index.set(i);
                let target = h[i].clone();
                let len = h.len();
                drop(h);
                stack.set_visible_child_name(&target);
                back_btn.set_sensitive(i > 0);
                forward_btn.set_sensitive(i < len - 1);
            }
        })
    };

    // on_select: navigate to detail page for any track/playlist/album.
    let navigate_to_for_select = navigate_to.clone();
    let go_back_for_select = go_back.clone();
    let content_stack_nav = content_stack.clone();
    let detail_counter_nav = detail_counter.clone();
    let cookies_nav = cookies_for_detail.clone();
    let play_from_list_for_select = play_from_list.clone();
    let on_select: Rc<dyn Fn(Track)> = Rc::new(move |track: Track| {
        let stack = content_stack_nav.clone();
        let counter = detail_counter_nav.clone();
        let cookies = cookies_nav.clone();
        let play_from_list = play_from_list_for_select.clone();

        match track.media_kind() {
            MediaKind::Song => {
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
                    play_from_list.clone(),
                    go_back_for_select.clone(),
                );
                detail.widget.set_hexpand(true);
                detail.widget.set_vexpand(true);
                stack.add_named(&detail.widget, Some(&name));
                navigate_to_for_select(&name);
            }
            MediaKind::Playlist | MediaKind::Album | MediaKind::Artist => {
                let browse_id = match track.browse_id() {
                    Some(id) => id.to_string(),
                    None => return,
                };
                let name = format!("detail_{}", counter.get());
                counter.set(counter.get() + 1);

                let loading = DetailView::loading();
                loading.widget.set_hexpand(true);
                loading.widget.set_vexpand(true);
                stack.add_named(&loading.widget, Some(&name));
                navigate_to_for_select(&name);

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
                let play_from_list_clone = play_from_list.clone();
                let go_back_async = go_back_for_select.clone();
                glib::spawn_future_local(async move {
                    let Ok(result) = receiver.recv().await else {
                        return;
                    };

                    if let Some(child) = stack.child_by_name(&name_clone) {
                        stack.remove(&child);
                    }

                    match result {
                        Ok(detail) => {
                            let on_back = go_back_async.clone();
                            let detail_view = DetailView::new(
                                &detail.metadata,
                                &detail.tracks,
                                play_from_list_clone,
                                on_back,
                            );
                            detail_view.widget.set_hexpand(true);
                            detail_view.widget.set_vexpand(true);
                            stack.add_named(&detail_view.widget, Some(&name_clone));
                            stack.set_visible_child(&detail_view.widget);
                        }
                        Err(e) => {
                            let on_retry = go_back_async.clone();
                            let err_view = DetailView::error(
                                &format!("Failed to load details: {e}"),
                                on_retry,
                            );
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

    {
        let navigate_to_home = navigate_to.clone();
        top_bar.home_button.connect_clicked(move |_| {
            navigate_to_home("home");
        });
    }

    let search_view = SearchView::new(&top_bar.search_entry, on_select.clone(), play_track.clone());
    let _search_view = search_view;

    let player_bar = PlayerBar::new(handle.commands.clone());
    let now_playing_panel = NowPlayingPanel::new(auth.cookies_path().to_path_buf());

    // -- Queue panel -----------------------------------------------------------

    let queue_panel = QueuePanel::new(handle.commands.clone());
    let queue_panel_widget = queue_panel.widget.clone();
    // Start hidden.
    queue_panel_widget.set_visible(false);

    {
        let panel = queue_panel_widget.clone();
        player_bar.queue_button.connect_clicked(move |_| {
            let visible = panel.is_visible();
            panel.set_visible(!visible);
        });
    }

    // -- Library sidebar -------------------------------------------------------

    let content_stack_for_sidebar = content_stack.clone();
    let cookies_for_liked = auth.cookies_path().to_path_buf();
    let library_sidebar = LibrarySidebar::new({
        let stack = content_stack_for_sidebar.clone();
        let cookies = cookies_for_liked.clone();
        let on_select = on_select.clone();
        let play_from_list = play_from_list.clone();
        let navigate_to_liked = navigate_to.clone();
        let go_back_for_liked = go_back.clone();
        move || {
            let stack = stack.clone();
            let liked_view = LikedSongsView::new(
                cookies.clone(),
                on_select.clone(),
                play_from_list.clone(),
                go_back_for_liked.clone(),
            );
            liked_view.widget.set_hexpand(true);
            liked_view.widget.set_vexpand(true);
            if let Some(old) = stack.child_by_name("liked") {
                stack.remove(&old);
            }
            stack.add_named(&liked_view.widget, Some("liked"));
            navigate_to_liked("liked");
        }
    });

    // Back/forward navigation buttons.
    {
        let go_back = go_back.clone();
        top_bar.back_button.connect_clicked(move |_| go_back());
    }
    {
        let go_forward = go_forward.clone();
        top_bar
            .forward_button
            .connect_clicked(move |_| go_forward());
    }

    content_stack.set_hexpand(true);
    content_stack.set_vexpand(true);

    let middle_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    middle_row.append(&library_sidebar.widget);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&content_stack);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&now_playing_panel.widget);
    middle_row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    middle_row.append(&queue_panel_widget);
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

    // -- Player & queue event stream -------------------------------------------

    let event_rx = handle.events;
    glib::spawn_future_local(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                PlayerEvent::State(state) => {
                    player_bar.update(&state);
                    now_playing_panel.update(&state);
                }
                PlayerEvent::Queue(snapshot) => {
                    queue_panel.update(&snapshot);
                }
            }
        }
    });

    window.present();
}

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
