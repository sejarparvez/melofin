//! The Spotify-style top bar: overflow menu, back/forward, home, a centered
//! search pill, and a right-hand cluster for downloads/account.
//!
//! Several buttons here are intentionally disabled — they're placeholders
//! for features that don't exist yet (view navigation, offline caching).
//! Wire them up as those land instead of faking functionality now.

use crate::user::UserProfile;
use adw::prelude::*;
use gtk::gio;

/// The built top bar, plus the widgets callers need to hook up behavior to.
///
/// All fields are GTK widgets (reference-counted), so cloning is cheap and
/// gives shared ownership — multiple closures can hold a clone.
#[derive(Clone)]
pub struct TopBar {
    pub root: gtk::Box,
    pub search_entry: gtk::SearchEntry,
    pub home_button: gtk::Button,
    /// Account `MenuButton` — clicking it toggles the user popover.
    /// `window.rs` connects signal handlers since that's where
    /// `AuthManager` and the window's `ToastOverlay` live.
    pub account_button: gtk::MenuButton,
    popover_name: gtk::Label,
    popover_email: gtk::Label,
    popover_avatar: adw::Avatar,
    /// "Log out" button inside the account popover — `window.rs` connects
    /// the click handler.
    pub logout_button: gtk::Button,
    /// "Log in" button inside the account popover — `window.rs` connects
    /// the click handler.
    pub login_button: gtk::Button,
}

pub fn build_top_bar() -> TopBar {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    root.add_css_class("top-bar");
    root.set_margin_start(12);
    root.set_margin_end(12);
    root.set_margin_top(8);
    root.set_margin_bottom(8);

    let left = build_left_cluster();
    let center = build_search_cluster();
    let (right_root, right_btn, popover_name, popover_email, popover_avatar, logout_button, login_button) =
        build_right_cluster();

    root.append(&left.root);
    root.append(&center.root);
    root.append(&right_root);

    TopBar {
        root,
        search_entry: center.search_entry,
        home_button: left.home_button,
        account_button: right_btn,
        popover_name,
        popover_email,
        popover_avatar,
        logout_button,
        login_button,
    }
}

// -- Public update helpers ---------------------------------------------------

impl TopBar {
    /// Resets the popover to the logged-out state: generic avatar, no
    /// name/email, "Log in" button visible, "Log out" hidden.
    pub fn set_logged_out(&self) {
        self.popover_avatar.set_text(None);
        self.popover_name.set_text("Not signed in");
        self.popover_email.set_visible(false);
        self.logout_button.set_visible(false);
        self.login_button.set_visible(true);
        self.account_button.set_tooltip_text(Some("Account — not signed in"));
    }

    /// Updates the popover with the given profile: avatar initial, name,
    /// email, and shows the "Log out" button.
    pub fn set_user_profile(&self, profile: &UserProfile) {
        self.popover_avatar.set_text(Some(&profile.initial()));
        self.popover_name.set_text(&profile.name);
        if let Some(email) = &profile.email {
            self.popover_email.set_text(email);
            self.popover_email.set_visible(true);
        } else {
            self.popover_email.set_visible(false);
        }
        self.logout_button.set_visible(true);
        self.login_button.set_visible(false);
        self.account_button
            .set_tooltip_text(Some("Account — signed in"));
    }
}

// -- Internals ---------------------------------------------------------------

struct LeftCluster {
    root: gtk::Box,
    home_button: gtk::Button,
}

/// Overflow menu, back, forward, home.
///
/// Back/forward stay disabled until there's real view-stack history to
/// traverse (currently the app just toggles between two views: home and
/// search — see `window.rs`). Home is live since that toggle exists now.
/// The overflow menu is live from day one since it's the only way to quit
/// without a titlebar close button.
fn build_left_cluster() -> LeftCluster {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 4);

    let menu_button = gtk::MenuButton::new();
    menu_button.set_icon_name("view-more-symbolic");
    menu_button.add_css_class("flat");
    menu_button.add_css_class("circular");
    menu_button.set_tooltip_text(Some("Menu"));
    menu_button.set_menu_model(Some(&overflow_menu()));

    let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
    back_button.add_css_class("flat");
    back_button.add_css_class("circular");
    back_button.set_tooltip_text(Some("Back"));
    back_button.set_sensitive(false); // TODO: enable once a view stack exists

    let forward_button = gtk::Button::from_icon_name("go-next-symbolic");
    forward_button.add_css_class("flat");
    forward_button.add_css_class("circular");
    forward_button.set_tooltip_text(Some("Forward"));
    forward_button.set_sensitive(false); // TODO: enable once a view stack exists

    let home_button = gtk::Button::from_icon_name("go-home-symbolic");
    home_button.add_css_class("flat");
    home_button.add_css_class("circular");
    home_button.set_tooltip_text(Some("Home"));

    root.append(&menu_button);
    root.append(&back_button);
    root.append(&forward_button);
    root.append(&home_button);

    LeftCluster { root, home_button }
}

struct CenterCluster {
    root: gtk::Box,
    search_entry: gtk::SearchEntry,
}

fn build_search_cluster() -> CenterCluster {
    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("What do you want to play?"));
    search_entry.add_css_class("top-bar-search");
    search_entry.set_hexpand(true);
    search_entry.set_max_width_chars(50);

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.set_hexpand(true);
    root.set_halign(gtk::Align::Center);
    root.append(&search_entry);

    CenterCluster { root, search_entry }
}

/// Builds the account `MenuButton` with its user popover. Returns the
/// root container, button, plus handles to the popover labels/buttons
/// so `TopBar` can update them.
fn build_right_cluster() -> (
    gtk::Box,
    gtk::MenuButton,
    gtk::Label,
    gtk::Label,
    adw::Avatar,
    gtk::Button,
    gtk::Button,
) {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 4);

    let downloads_button = gtk::Button::from_icon_name("folder-download-symbolic");
    downloads_button.add_css_class("flat");
    downloads_button.add_css_class("circular");
    downloads_button.set_tooltip_text(Some("Downloads"));
    downloads_button.set_sensitive(false); // TODO: enable once offline caching lands

    // -- Account popover -------------------------------------------------------
    let popover_avatar = adw::Avatar::new(48, Some("G"), true);

    let popover_name = gtk::Label::new(Some("Not signed in"));
    popover_name.add_css_class("title-2");

    let popover_email = gtk::Label::new(None);
    popover_email.add_css_class("dim-label");
    popover_email.set_visible(false);

    let logout_button = gtk::Button::with_label("Log out");
    logout_button.add_css_class("destructive-action");
    logout_button.set_visible(false);

    let login_button = gtk::Button::with_label("Log in");
    login_button.add_css_class("suggested-action");

    let popover_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    popover_box.set_margin_top(12);
    popover_box.set_margin_bottom(12);
    popover_box.set_margin_start(16);
    popover_box.set_margin_end(16);
    popover_box.set_width_request(200);
    popover_box.append(&popover_avatar);
    popover_box.append(&popover_name);
    popover_box.append(&popover_email);
    popover_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    popover_box.append(&logout_button);
    popover_box.append(&login_button);

    let popover = gtk::Popover::new();
    popover.set_child(Some(&popover_box));
    popover.set_has_arrow(false);

    // -- MenuButton ------------------------------------------------------------

    let avatar_button_icon = adw::Avatar::new(28, Some("G"), true);
    let account_button = gtk::MenuButton::new();
    account_button.set_child(Some(&avatar_button_icon));
    account_button.set_popover(Some(&popover));
    account_button.add_css_class("flat");
    account_button.add_css_class("circular");
    account_button.set_tooltip_text(Some("Account — not signed in"));

    root.append(&downloads_button);
    root.append(&account_button);

    (
        root,
        account_button,
        popover_name,
        popover_email,
        popover_avatar,
        logout_button,
        login_button,
    )
}

fn overflow_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Preferences"), Some("app.preferences"));
    menu.append(Some("About Melofin"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    menu
}
