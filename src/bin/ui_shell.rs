//! Entry point for the GTK4/libadwaita UI binary. Actual window/widget
//! assembly lives in `melofin::ui` so it can be split by concern
//! (search view, player bar, etc.) instead of one growing file.
fn main() -> gtk::glib::ExitCode {
    melofin::ui::window::run()
}
