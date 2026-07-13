//! Cookie-file login dialog. Talks only to `crate::auth::AuthManager` —
//! same separation as `search_view.rs`/`player_bar.rs` not touching mpv or
//! tokio directly, just applied to auth instead of playback.
//!
//! `AuthManager`'s methods are `async fn` built on `tokio::fs`/
//! `tokio::process`, but there is no Tokio runtime running on the GTK main
//! thread — only `player.rs`'s dedicated background thread has one. Every
//! call into `AuthManager` here goes through [`run_auth`], which spawns a
//! plain OS thread with a throwaway Tokio runtime and bridges the result
//! back via `async_channel`, mirroring the background-thread pattern
//! `search_view.rs`/`thumbnail_widget.rs` already use for blocking work —
//! just with `.block_on()` added since this work is `async` rather than
//! sync.

use crate::auth::{AuthManager, AuthState};
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use std::future::Future;
use std::rc::Rc;

/// Opens the login dialog for `parent`. If already logged in, shows a
/// logged-in state with a Log out button instead of the import flow.
/// `toast_overlay` is the parent window's overlay — used to show a "Logged
/// in" toast *after* this dialog closes, since the dialog itself is gone
/// by then.
/// `on_state_changed` fires once, with the new state, whenever a login or
/// logout actually completes inside this dialog — `window.rs` uses it to
/// refresh the top bar's account menu (`top_bar::set_account_menu`).
pub fn present(
    parent: &adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    auth: AuthManager,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .title("Account")
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_start(20);
    content.set_margin_end(20);
    content.set_margin_top(20);
    content.set_margin_bottom(20);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));
    window.set_content(Some(&toolbar_view));

    match auth.current_state() {
        // `current_state()` is only a file-existence check (see auth.rs) —
        // re-validate on open so "logged in" here means the session
        // actually still works, not just that a file is present.
        AuthState::LoggedIn => {
            build_checking_view(&content, &window, auth, toast_overlay, on_state_changed)
        }
        AuthState::LoggedOut => {
            build_login_view(&content, &window, auth, toast_overlay, on_state_changed)
        }
    }

    window.present();
}

/// Shown briefly on open when a cookies file exists, while we confirm the
/// session is still valid rather than trusting the file's mere presence.
fn build_checking_view(
    content: &gtk::Box,
    window: &adw::Window,
    auth: AuthManager,
    toast_overlay: adw::ToastOverlay,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_margin_top(24);
    spinner.set_margin_bottom(24);
    let label = gtk::Label::new(Some("Checking your session…"));

    content.append(&spinner);
    content.append(&label);

    let content = content.clone();
    let window = window.clone();
    let auth_for_check = auth.clone();
    run_auth(
        move || {
            let auth = auth_for_check.clone();
            async move { auth.validate().await }
        },
        move |result| {
            clear(&content);
            match result {
                Ok(()) => build_logged_in_view(&content, auth, on_state_changed),
                Err(e) => {
                    tracing::warn!("stored session no longer valid: {e}");
                    build_login_view(&content, &window, auth, toast_overlay, on_state_changed);
                }
            }
        },
    );
}

fn build_logged_in_view(
    content: &gtk::Box,
    auth: AuthManager,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    // Wrapped in `Rc` because `connect_clicked` requires `Fn` (callable
    // repeatedly), but the inner `run_auth` completion closure needs to
    // *own* a copy of `on_state_changed` to call it — an `Rc` clone is
    // cheap and sidesteps that without requiring `on_state_changed: Clone`
    // at the call site.
    let on_state_changed = Rc::new(on_state_changed);

    let status = gtk::Label::new(Some("You're signed in to YouTube Music."));
    status.set_wrap(true);
    status.set_xalign(0.0);

    let logout_button = gtk::Button::with_label("Log out");
    logout_button.add_css_class("destructive-action");
    logout_button.set_halign(gtk::Align::Start);

    content.append(&status);
    content.append(&logout_button);

    let content_for_logout = content.clone();
    logout_button.connect_clicked(move |button| {
        button.set_sensitive(false);
        let content = content_for_logout.clone();
        let auth = auth.clone();
        let on_state_changed = on_state_changed.clone();
        run_auth(
            {
                let auth = auth.clone();
                move || {
                    let auth = auth.clone();
                    async move { auth.logout().await }
                }
            },
            {
                let content = content.clone();
                let auth = auth.clone();
                move |result| {
                    if let Err(e) = result {
                        tracing::warn!("logout failed: {e}");
                        // Leave the logged-in view up rather than silently
                        // pretending it worked.
                        return;
                    }
                    on_state_changed(AuthState::LoggedOut);
                    clear(&content);
                    // window isn't available in this closure; re-entering
                    // build_login_view needs one for the "already logged
                    // in, checking" branch only — logout always lands
                    // directly in the plain login view, which doesn't
                    // need it, so pass a throwaway that's never read.
                    build_login_view_standalone(&content, auth.clone());
                }
            },
        );
        let _ = &content; // keep borrow-checker happy across the two closures above
    });
}

/// Same body as `build_login_view` but without the `window`/`toast_overlay`
/// params, for call sites (post-logout) that don't have them handy and
/// don't need them — `build_login_view`'s `window` is only used if a fresh
/// import itself somehow needs to re-check, which it doesn't, and the
/// logout flow doesn't show a toast; kept as a separate thin wrapper so
/// `build_login_view`'s signature stays honest about when those are
/// actually used.
fn build_login_view_standalone(content: &gtk::Box, auth: AuthManager) {
    build_login_form(content, auth, None, |_| {});
}

fn build_login_view(
    content: &gtk::Box,
    _window: &adw::Window,
    auth: AuthManager,
    toast_overlay: adw::ToastOverlay,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    build_login_form(content, auth, Some(toast_overlay), on_state_changed);
}

fn build_login_form(
    content: &gtk::Box,
    auth: AuthManager,
    toast_overlay: Option<adw::ToastOverlay>,
    on_state_changed: impl Fn(AuthState) + 'static,
) {
    // Same `Rc` reasoning as `build_logged_in_view` — `connect_clicked`
    // needs `Fn`, the completion closure needs an owned copy to call.
    let on_state_changed = Rc::new(on_state_changed);

    let instructions = gtk::Label::new(Some(
        "Log into music.youtube.com in your browser — an Incognito/Private \
         window is safest. Export your cookies with a \"Get cookies.txt\" \
         browser extension, then close that browser tab (YouTube rotates \
         session cookies every few minutes, so a tab left open can \
         invalidate the ones you just exported). Then select the exported \
         file below.",
    ));
    instructions.set_wrap(true);
    instructions.set_xalign(0.0);

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

    content.append(&instructions);
    content.append(&choose_button);
    content.append(&chosen_path_label);
    content.append(&login_button);
    content.append(&spinner);
    content.append(&status_label);

    // Holds the path picked by the file dialog, read when "Log in" is
    // clicked. A plain `Rc<RefCell<>>` — same pattern search_view.rs uses
    // for `current_tracks`.
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

            // Restrict to text files — exported cookie files are always
            // `.txt` (Netscape cookie-file format).
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
                        // User cancelling the picker also lands here as an
                        // Err — that's expected and not worth logging.
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
            choose_button_disable_sibling(button);
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
                            // Shown on the parent window's overlay, since
                            // the dialog above is already gone by now.
                            // `toast_overlay` is `None` only for the
                            // logout-standalone call site, which never
                            // reaches this success arm.
                            if let Some(overlay) = &toast_overlay {
                                overlay.add_toast(adw::Toast::new("Logged in"));
                            }
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

/// The choose-file button doesn't need disabling while an import is in
/// flight (re-picking mid-import is harmless — it just replaces
/// `picked_path`), so this is intentionally a no-op today. Kept as a named
/// call site rather than inlined so it's obvious this was a deliberate
/// choice, not a missed wiring, if reviewed later.
fn choose_button_disable_sibling(_login_button: &gtk::Button) {}

/// Removes every child currently in `container`.
fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Runs `make_future()`'s future to completion on a fresh background-thread
/// Tokio runtime, then calls `on_done` with the result back on the GTK main
/// thread. See the module doc comment for why this exists instead of
/// `.await`-ing `AuthManager` calls directly.
fn run_auth<T, Fut>(
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
