//! Entry point for the GTK UI. All the actual UI logic lives in
//! `melofin::ui`, split into its own module tree (top bar, and more views
//! as they're built) — see src/ui/mod.rs.

use gtk::glib;

fn main() -> glib::ExitCode {
    melofin::ui::run()
}
