//! Cookie-file login dialog. Talks only to `crate::auth::AuthManager` —
//! same separation as `search_view.rs`/`player_bar.rs` not touching mpv or
//! tokio directly, just applied to auth instead of playback.
//!
//! This dialog shows browser auto-import buttons (via `rookie`) when
//! installed browsers are detected, plus a manual file-picker fallback.
//! The logged-in state lives in the account popover (`top_bar.rs`), so
//! this dialog is only presented when the user wants to log in.

use crate::auth::{self, AuthManager, AuthState};
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use std::future::Future;
use std::rc::Rc;

/// Opens the login dialog for `parent`. Shows browser auto-import buttons
/// and/or the manual cookie-file picker.
/// `on_state_changed` fires once with `AuthState::LoggedIn` when login
/// completes — `window.rs` uses it to refresh the account popover.
pub fn present(
    parent: &adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    auth: AuthManager,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .title("Log in")
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_start(20);
    content.set_margin_end(20);
    content.set_margin_top(20);
    content.set_margin_bottom(20);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));
    window.set_content(Some(&toolbar_view));

    build_login_form(&content, &window, auth, toast_overlay, on_state_changed);
    window.present();
}

fn build_login_form(
    content: &gtk::Box,
    _window: &adw::Window,
    auth: AuthManager,
    toast_overlay: adw::ToastOverlay,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    let on_state_changed = Rc::new(on_state_changed);

    // -- Browser auto-import buttons -------------------------------------------

    let detected = auth::detect_browsers();
    if !detected.is_empty() {
        let browser_label = gtk::Label::new(Some("Import cookies from your browser"));
        browser_label.add_css_class("title-3");
        browser_label.set_xalign(0.0);
        content.append(&browser_label);

        for browser in &detected {
            let btn = gtk::Button::with_label(&format!("Import from {browser}"));
            btn.add_css_class("suggested-action");
            btn.set_halign(gtk::Align::Fill);
            btn.set_hexpand(true);

            let auth = auth.clone();
            let toast_overlay = toast_overlay.clone();
            let on_state_changed = on_state_changed.clone();
            let browser = *browser;
            btn.connect_clicked(move |button| {
                button.set_sensitive(false);
                button.set_label(&format!("Importing from {browser}…"));
                let auth = auth.clone();
                let toast_overlay = toast_overlay.clone();
                let on_state_changed = on_state_changed.clone();
                let button = button.clone();
                let browser_label = browser.to_string();
                let browser_for_fetch = browser_label.clone();
                run_auth(
                    move || {
                        let auth = auth.clone();
                        let browser = browser_for_fetch;
                        async move {
                            let cookies =
                                tokio::task::spawn_blocking(move || auth::rookie_import(&browser))
                                    .await
                                    .map_err(|e| anyhow::anyhow!("{e}"))??;
                            auth.import_cookies_from_rookie(cookies).await
                        }
                    },
                    move |result| match result {
                        Ok(()) => {
                            on_state_changed(AuthState::LoggedIn);
                            if let Some(w) =
                                button.root().and_then(|r| r.downcast::<adw::Window>().ok())
                            {
                                w.close();
                            }
                            toast_overlay.add_toast(adw::Toast::new("Logged in"));
                        }
                        Err(e) => {
                            button.set_sensitive(true);
                            button.set_label(&format!("Import from {browser_label}"));
                            toast_overlay
                                .add_toast(adw::Toast::new(&format!("Couldn't import: {e}")));
                        }
                    },
                );
            });
            content.append(&btn);
        }
    }

    // -- Separator + manual file picker fallback -------------------------------

    if !detected.is_empty() {
        let sep_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        sep_box.set_halign(gtk::Align::Center);
        let sep_left = gtk::Separator::new(gtk::Orientation::Horizontal);
        sep_left.set_hexpand(true);
        let or_label = gtk::Label::new(Some("or"));
        or_label.add_css_class("dim-label");
        let sep_right = gtk::Separator::new(gtk::Orientation::Horizontal);
        sep_right.set_hexpand(true);
        sep_box.append(&sep_left);
        sep_box.append(&or_label);
        sep_box.append(&sep_right);
        content.append(&sep_box);
    }

    let manual_label = gtk::Label::new(Some("Select a cookies.txt file"));
    manual_label.add_css_class("title-3");
    manual_label.set_xalign(0.0);
    content.append(&manual_label);

    let instructions = gtk::Label::new(Some(
        "Log into music.youtube.com in your browser (an Incognito window \
         is safest). Export your cookies with a \"Get cookies.txt\" browser \
         extension, then close that browser tab before importing.",
    ));
    instructions.set_wrap(true);
    instructions.set_xalign(0.0);
    instructions.add_css_class("dim-label");
    content.append(&instructions);

    let chosen_path_label = gtk::Label::new(Some("No file selected"));
    chosen_path_label.set_wrap(true);
    chosen_path_label.set_xalign(0.0);
    chosen_path_label.add_css_class("dim-label");

    let choose_button = gtk::Button::with_label("Choose cookies.txt file…");
    choose_button.set_halign(gtk::Align::Start);

    let status_label = gtk::Label::new(None);
    status_label.set_wrap(true);
    status_label.set_xalign(0.0);
    status_label.set_visible(false);

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);

    let login_button = gtk::Button::with_label("Log in");
    login_button.add_css_class("suggested-action");
    login_button.set_halign(gtk::Align::Start);
    login_button.set_sensitive(false);

    content.append(&choose_button);
    content.append(&chosen_path_label);
    content.append(&login_button);
    content.append(&spinner);
    content.append(&status_label);

    let picked_path: Rc<std::cell::RefCell<Option<std::path::PathBuf>>> =
        Rc::new(std::cell::RefCell::new(None));

    {
        let picked_path = picked_path.clone();
        let chosen_path_label = chosen_path_label.clone();
        let login_button = login_button.clone();
        choose_button.connect_clicked(move |button| {
            let picked_path = picked_path.clone();
            let chosen_path_label = chosen_path_label.clone();
            let login_button = login_button.clone();
            let root = button.root().and_then(|r| r.downcast::<gtk::Window>().ok());

            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Text files"));
            filter.add_suffix("txt");
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);

            let file_dialog = gtk::FileDialog::builder()
                .title("Select exported cookies file")
                .filters(&filters)
                .build();

            glib::spawn_future_local(async move {
                match file_dialog.open_future(root.as_ref()).await {
                    Ok(file) => {
                        let Some(path) = file.path() else {
                            return;
                        };
                        chosen_path_label.set_text(&path.display().to_string());
                        chosen_path_label.remove_css_class("dim-label");
                        login_button.set_sensitive(true);
                        *picked_path.borrow_mut() = Some(path);
                    }
                    Err(e) => {
                        tracing::debug!("file dialog closed without a selection: {e}");
                    }
                }
            });
        });
    }

    {
        let picked_path = picked_path.clone();
        login_button.connect_clicked(move |button| {
            let Some(path) = picked_path.borrow().clone() else {
                return;
            };
            button.set_sensitive(false);
            spinner.set_visible(true);
            spinner.set_spinning(true);
            status_label.set_visible(false);

            let auth = auth.clone();
            let button = button.clone();
            let spinner = spinner.clone();
            let status_label = status_label.clone();
            let toast_overlay = toast_overlay.clone();
            let on_state_changed = on_state_changed.clone();
            run_auth(
                {
                    let auth = auth.clone();
                    move || {
                        let auth = auth.clone();
                        async move { auth.import_cookies_file(&path).await }
                    }
                },
                move |result| {
                    spinner.set_visible(false);
                    spinner.set_spinning(false);
                    match result {
                        Ok(()) => {
                            on_state_changed(AuthState::LoggedIn);
                            if let Some(window) =
                                button.root().and_then(|r| r.downcast::<adw::Window>().ok())
                            {
                                window.close();
                            }
                            toast_overlay.add_toast(adw::Toast::new("Logged in"));
                        }
                        Err(e) => {
                            button.set_sensitive(true);
                            status_label.set_text(&format!("Couldn't log in: {e}"));
                            status_label.set_visible(true);
                        }
                    }
                },
            );
        });
    }
}

/// Runs `make_future()`'s future to completion on a fresh background-thread
/// Tokio runtime, then calls `on_done` with the result back on the GTK main
/// thread.
pub(crate) fn run_auth<T, Fut>(
    make_future: impl FnOnce() -> Fut + Send + 'static,
    on_done: impl FnOnce(T) + 'static,
) where
    T: Send + 'static,
    Fut: Future<Output = T>,
{
    let (sender, receiver) = async_channel::bounded::<T>(1);
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("failed to build auth runtime: {e}");
                return;
            }
        };
        let result = rt.block_on(make_future());
        let _ = sender.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = receiver.recv().await {
            on_done(result);
        }
    });
}
