//! The Spotify-style top bar: overflow menu, back/forward, home, a centered
//! search pill, and a right-hand cluster for downloads/account.
//!
//! Several buttons here are intentionally disabled — they're placeholders
//! for features that don't exist yet (view navigation, offline caching,
//! auth). Wire them up as those land instead of faking functionality now.

use adw::prelude::*;
use gtk::gio;

/// The built top bar, plus the widgets callers need to hook up behavior to
/// (currently just the search entry; more will be added as buttons go live).
pub struct TopBar {
    pub root: gtk::Box,
    pub search_entry: gtk::SearchEntry,
    pub home_button: gtk::Button,
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
    let right = build_right_cluster();

    root.append(&left.root);
    root.append(&center.root);
    root.append(&right.root);

    TopBar {
        root,
        search_entry: center.search_entry,
        home_button: left.home_button,
    }
}

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

struct RightCluster {
    root: gtk::Box,
}

/// Downloads status and account. Both disabled for now: downloads needs the
/// offline-caching layer (Step 6), account needs auth (Step 7).
fn build_right_cluster() -> RightCluster {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, 4);

    let downloads_button = gtk::Button::from_icon_name("folder-download-symbolic");
    downloads_button.add_css_class("flat");
    downloads_button.add_css_class("circular");
    downloads_button.set_tooltip_text(Some("Downloads"));
    downloads_button.set_sensitive(false); // TODO: enable once offline caching lands

    let avatar = adw::Avatar::new(28, None, true);
    let account_button = gtk::MenuButton::new();
    account_button.set_child(Some(&avatar));
    account_button.add_css_class("flat");
    account_button.add_css_class("circular");
    account_button.set_tooltip_text(Some("Account"));
    account_button.set_menu_model(Some(&account_menu()));

    root.append(&downloads_button);
    root.append(&account_button);

    RightCluster { root }
}

fn overflow_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Preferences"), Some("app.preferences"));
    menu.append(Some("About Melofin"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    menu
}

fn account_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Not signed in"), None); // TODO: real state once Step 7 (auth) lands
    menu
}
