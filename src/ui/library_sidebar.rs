//! Left "Your Library" sidebar. Shows the user's library items fetched from
//! YouTube Music. "Liked Songs" is wired to switch the main content area;
//! other items show a "coming soon" tooltip until their backends are ready.

use adw::prelude::*;

const NO_BACKEND_YET: &str = "coming soon \u{2014} no library backend yet";

struct LibraryItem {
    name: &'static str,
    subtitle: &'static str,
    icon: &'static str,
}

fn dummy_library() -> Vec<LibraryItem> {
    vec![
        LibraryItem {
            name: "Liked Songs",
            subtitle: "Playlist",
            icon: "starred-symbolic",
        },
        LibraryItem {
            name: "Discover Weekly",
            subtitle: "Playlist",
            icon: "media-playlist-shuffle-symbolic",
        },
        LibraryItem {
            name: "Harry Styles",
            subtitle: "Artist",
            icon: "avatar-default-symbolic",
        },
        LibraryItem {
            name: "Eminem",
            subtitle: "Artist",
            icon: "avatar-default-symbolic",
        },
        LibraryItem {
            name: "Alan Walker",
            subtitle: "Artist",
            icon: "avatar-default-symbolic",
        },
        LibraryItem {
            name: "Imagine Dragons",
            subtitle: "Artist",
            icon: "avatar-default-symbolic",
        },
    ]
}

pub struct LibrarySidebar {
    pub widget: gtk::Box,
}
impl Default for LibrarySidebar {
    fn default() -> Self {
        Self::new(|| {})
    }
}
impl LibrarySidebar {
    /// `on_liked_songs` is called when the user clicks "Liked Songs" in the
    /// sidebar. Pass a closure that switches the main content area to the
    /// liked songs view.
    pub fn new(on_liked_songs: impl Fn() + 'static + Clone) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.add_css_class("sidebar");
        widget.set_size_request(240, -1);
        widget.set_margin_top(10);
        widget.set_margin_bottom(10);
        widget.set_margin_start(8);
        widget.set_margin_end(8);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let title = gtk::Label::new(Some("Your Library"));
        title.add_css_class("heading");
        title.set_hexpand(true);
        title.set_halign(gtk::Align::Start);

        let add_button = gtk::Button::from_icon_name("list-add-symbolic");
        add_button.add_css_class("flat");
        add_button.add_css_class("circular");
        add_button.set_tooltip_text(Some(NO_BACKEND_YET));
        add_button.set_sensitive(false);

        header.append(&title);
        header.append(&add_button);
        widget.append(&header);

        // Playlists / Albums / Artists filter chips \u{2014} cosmetic only until
        // there's more than one kind of library data to filter between.
        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for label in ["Playlists", "Albums", "Artists"] {
            let chip = gtk::ToggleButton::with_label(label);
            chip.add_css_class("pill");
            chip.add_css_class("library-chip");
            chip.set_sensitive(false);
            chip.set_tooltip_text(Some(NO_BACKEND_YET));
            chips.append(&chip);
        }
        let chips_scroller = gtk::ScrolledWindow::new();
        chips_scroller.set_vscrollbar_policy(gtk::PolicyType::Never);
        chips_scroller.set_hscrollbar_policy(gtk::PolicyType::External);
        chips_scroller.set_child(Some(&chips));
        widget.append(&chips_scroller);

        let rows = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let items = dummy_library();
        for (i, item) in items.iter().enumerate() {
            let row_widget = library_row(item);
            // "Liked Songs" (index 0) is clickable when logged in.
            if i == 0 {
                let btn = row_widget.downcast_ref::<gtk::Button>().unwrap();
                btn.set_sensitive(true);
                btn.set_tooltip_text(None);
                let on_liked_songs = on_liked_songs.clone();
                btn.connect_clicked(move |_| on_liked_songs());
            }
            rows.append(&row_widget);
        }

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroller.set_vexpand(true);
        scroller.set_child(Some(&rows));
        widget.append(&scroller);
        widget.set_hexpand(false);
        Self { widget }
    }
}

fn library_row(item: &LibraryItem) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);

    let art = gtk::Frame::new(None);
    art.add_css_class("home-art");
    art.set_size_request(40, 40);
    let icon = gtk::Image::from_icon_name(item.icon);
    icon.set_pixel_size(18);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    art.set_child(Some(&icon));

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text_box.set_valign(gtk::Align::Center);
    let name = gtk::Label::new(Some(item.name));
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let subtitle = gtk::Label::new(Some(item.subtitle));
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    subtitle.set_halign(gtk::Align::Start);
    text_box.append(&name);
    text_box.append(&subtitle);

    row.append(&art);
    row.append(&text_box);

    // A disabled `Button` wrapper (not a bare row) so it already looks and
    // behaves like every other not-yet-wired control in the app, rather
    // than looking clickable and silently doing nothing.
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&row));
    button.set_sensitive(false);
    button.set_tooltip_text(Some(NO_BACKEND_YET));
    button.upcast()
}
