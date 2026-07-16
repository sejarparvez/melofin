//! Left sidebar with app branding, navigation items, and settings.
//! Matches the Stitch "Library Dashboard" design.

use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

struct NavItem {
    name: &'static str,
    icon: &'static str,
}

fn nav_items() -> Vec<NavItem> {
    vec![
        NavItem {
            name: "Library",
            icon: "emblem-music-symbolic",
        },
        NavItem {
            name: "Explore",
            icon: "emblem-sychronizing-symbolic",
        },
        NavItem {
            name: "Liked Songs",
            icon: "starred-symbolic",
        },
        NavItem {
            name: "Playlists",
            icon: "playlist-symbolic",
        },
    ]
}

pub struct LibrarySidebar {
    pub widget: gtk::Box,
}
impl Default for LibrarySidebar {
    fn default() -> Self {
        Self::new(|| {}, |_| {})
    }
}
impl LibrarySidebar {
    /// `on_liked_songs` is called when the user clicks "Liked Songs".
    /// `on_navigate` is called when any nav item is clicked with the item name.
    pub fn new(
        on_liked_songs: impl Fn() + 'static + Clone,
        on_navigate: impl Fn(&str) + 'static + Clone,
    ) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.add_css_class("sidebar");
        widget.set_size_request(280, -1);
        widget.set_margin_top(8);
        widget.set_margin_bottom(8);
        widget.set_margin_start(8);
        widget.set_margin_end(8);

        // App branding header
        let header = gtk::Box::new(gtk::Orientation::Vertical, 2);
        header.set_margin_bottom(8);
        let title = gtk::Label::new(Some("Music Player"));
        title.add_css_class("title-2");
        title.set_halign(gtk::Align::Start);
        let subtitle = gtk::Label::new(Some("Premium Audio"));
        subtitle.add_css_class("dim-label");
        subtitle.set_halign(gtk::Align::Start);
        header.append(&title);
        header.append(&subtitle);
        widget.append(&header);

        // Navigation items
        let active_index = Rc::new(Cell::new(0usize));
        let nav_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let nav_items = nav_items();

        for (i, item) in nav_items.iter().enumerate() {
            let row_widget = nav_row(item, i == 0);
            let btn = row_widget.downcast_ref::<gtk::Button>().unwrap();

            // Wire click handler
            let active_index = active_index.clone();
            let on_liked_songs = on_liked_songs.clone();
            let on_navigate = on_navigate.clone();
            let item_name = item.name.to_string();
            let nav_box_ref = nav_box.clone();
            btn.connect_clicked(move |_| {
                let new_index = i;

                // Update active state visually
                if let Some(child) = nav_box_ref.first_child() {
                    let mut current = child.clone();
                    let mut idx = 0;
                    loop {
                        if idx == new_index {
                            current.add_css_class("active");
                        } else {
                            current.remove_css_class("active");
                        }
                        if let Some(next) = current.next_sibling() {
                            current = next;
                            idx += 1;
                        } else {
                            break;
                        }
                    }
                }
                active_index.set(new_index);

                // Fire navigation callback
                if item_name == "Liked Songs" {
                    on_liked_songs();
                } else {
                    on_navigate(&item_name);
                }
            });

            nav_box.append(&row_widget);
        }

        widget.append(&nav_box);

        // Spacer to push settings to bottom
        let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        spacer.set_vexpand(true);
        widget.append(&spacer);

        // Settings button at bottom
        let settings_row = nav_row(
            &NavItem {
                name: "Settings",
                icon: "emblem-system-symbolic",
            },
            false,
        );
        let settings_btn = settings_row.downcast_ref::<gtk::Button>().unwrap();
        settings_btn.set_tooltip_text(None);
        // Settings click handler — opens About dialog via parent window
        // For now, just a placeholder that does nothing visible
        settings_btn.connect_clicked(|_| {
            // The About dialog is wired in window.rs via the app action
        });
        widget.append(&settings_row);

        widget.set_hexpand(false);
        Self { widget }
    }
}

fn nav_row(item: &NavItem, active: bool) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("library-row");
    if active {
        row.add_css_class("active");
    }

    let icon = gtk::Image::from_icon_name(item.icon);
    icon.set_pixel_size(24);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);

    let label = gtk::Label::new(Some(item.name));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    row.append(&icon);
    row.append(&label);

    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&row));
    button.upcast()
}
