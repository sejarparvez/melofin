use crate::auth::AuthManager;
use crate::user::UserProfile;
use adw::prelude::*;
use std::path::PathBuf;

pub struct SettingsView;

impl SettingsView {
    pub fn present(
        parent: &adw::ApplicationWindow,
        _auth: AuthManager,
        data_dir: PathBuf,
        on_logout: impl Fn() + 'static,
    ) {
        let window = adw::Window::builder()
            .transient_for(parent)
            .modal(true)
            .default_width(680)
            .default_height(820)
            .title("Settings")
            .build();

        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(true);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_vexpand(true);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_child(Some(&content));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scrolled));
        window.set_content(Some(&toolbar_view));

        // Page title
        let title_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        title_box.set_margin_top(32);
        title_box.set_margin_bottom(24);
        title_box.set_margin_start(32);
        title_box.set_margin_end(32);
        let page_title = gtk::Label::new(Some("Settings"));
        page_title.add_css_class("settings-title");
        page_title.set_halign(gtk::Align::Start);
        let page_subtitle = gtk::Label::new(Some(
            "Manage your account preferences and audio experience.",
        ));
        page_subtitle.add_css_class("dim-label");
        page_subtitle.set_halign(gtk::Align::Start);
        title_box.append(&page_title);
        title_box.append(&page_subtitle);
        content.append(&title_box);

        // 2-column grid container
        let grid = gtk::Grid::new();
        grid.set_column_spacing(24);
        grid.set_row_spacing(24);
        grid.set_margin_start(32);
        grid.set_margin_end(32);
        grid.set_margin_bottom(32);
        grid.set_halign(gtk::Align::Fill);

        let profile = UserProfile::load(&data_dir).unwrap_or_else(UserProfile::guest);

        // -- Account (left column, row 0) --
        let account_card = build_account_card(&profile);
        grid.attach(&account_card, 0, 0, 1, 1);

        // -- Storage (right column, row 0) --
        let storage_card = build_storage_card();
        grid.attach(&storage_card, 1, 0, 1, 1);

        // -- Audio Quality (left column, row 1) --
        let audio_card = build_audio_quality_card();
        grid.attach(&audio_card, 0, 1, 1, 1);

        // -- Playback (right column, row 1) --
        let playback_card = build_playback_card();
        grid.attach(&playback_card, 1, 1, 1, 1);

        // -- Appearance (full width, row 2) --
        let appearance_card = build_appearance_card();
        grid.attach(&appearance_card, 0, 2, 2, 1);

        content.append(&grid);

        // -- Logout wiring --
        {
            let on_logout = std::rc::Rc::new(std::cell::RefCell::new(Some(on_logout)));
            if let Some(btn) = find_button_by_label(grid.upcast_ref(), "Sign Out") {
                let on_logout = on_logout.clone();
                let btn_clone = btn.clone();
                btn.connect_clicked(move |_| {
                    if let Some(cb) = on_logout.borrow_mut().take() {
                        cb();
                    }
                    if let Some(w) = btn_clone.root().and_then(|r| r.downcast::<adw::Window>().ok()) {
                        w.close();
                    }
                });
            }
        }

        window.present();
    }
}

// ---------------------------------------------------------------------------
// Glass panel helper
// ---------------------------------------------------------------------------

fn glass_panel() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("settings-panel");
    card
}

/// Section header row: icon in bg-primary/15 box + title + description
fn section_header(icon_name: &str, title: &str, desc: &str) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    header.set_halign(gtk::Align::Start);
    header.set_margin_bottom(24);

    let icon_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    icon_box.add_css_class("settings-section-icon");
    icon_box.set_size_request(48, 48);
    icon_box.set_halign(gtk::Align::Start);
    icon_box.set_valign(gtk::Align::Start);
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(24);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon_box.append(&icon);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_valign(gtk::Align::Center);

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("settings-section-title");
    title_label.set_halign(gtk::Align::Start);

    let desc_label = gtk::Label::new(Some(desc));
    desc_label.add_css_class("settings-section-desc");
    desc_label.set_halign(gtk::Align::Start);

    text.append(&title_label);
    text.append(&desc_label);

    header.append(&icon_box);
    header.append(&text);
    header
}

/// A toggle row with label, description, and switch
fn toggle_row(title: &str, desc: &str, active: bool) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("settings-row");

    let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
    info.set_hexpand(true);

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("settings-row-label");
    title_label.set_halign(gtk::Align::Start);

    let desc_label = gtk::Label::new(Some(desc));
    desc_label.add_css_class("settings-row-desc");
    desc_label.set_halign(gtk::Align::Start);

    info.append(&title_label);
    info.append(&desc_label);

    let switch = gtk::Switch::new();
    switch.set_active(active);
    switch.set_valign(gtk::Align::Center);

    row.append(&info);
    row.append(&switch);
    row
}

// ---------------------------------------------------------------------------
// Card builders
// ---------------------------------------------------------------------------

fn build_account_card(profile: &UserProfile) -> gtk::Widget {
    let card = glass_panel();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    inner.append(&section_header("avatar-default-symbolic", "Account", "Manage your account details"));

    // Profile row
    let profile_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    profile_row.set_halign(gtk::Align::Start);
    profile_row.set_valign(gtk::Align::Center);

    let avatar_frame = gtk::Frame::new(None);
    avatar_frame.add_css_class("settings-avatar");
    avatar_frame.set_size_request(96, 96);
    let avatar_icon = gtk::Image::from_icon_name("avatar-default-symbolic");
    avatar_icon.set_pixel_size(40);
    avatar_icon.set_halign(gtk::Align::Center);
    avatar_icon.set_valign(gtk::Align::Center);
    avatar_frame.set_child(Some(&avatar_icon));
    profile_row.append(&avatar_frame);

    let info = gtk::Box::new(gtk::Orientation::Vertical, 4);
    info.set_valign(gtk::Align::Center);

    let name_label = gtk::Label::new(Some(&profile.name));
    name_label.add_css_class("settings-section-title");
    name_label.set_halign(gtk::Align::Start);
    info.append(&name_label);

    if let Some(email) = &profile.email {
        let email_label = gtk::Label::new(Some(email));
        email_label.add_css_class("dim-label");
        email_label.set_halign(gtk::Align::Start);
        info.append(&email_label);
    }

    profile_row.append(&info);
    inner.append(&profile_row);

    // Buttons
    let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    btn_row.set_halign(gtk::Align::End);

    let manage_btn = gtk::Button::with_label("Manage Plan");
    manage_btn.add_css_class("flat");
    btn_row.append(&manage_btn);

    let signout_btn = gtk::Button::with_label("Sign Out");
    signout_btn.add_css_class("destructive-action");
    btn_row.append(&signout_btn);

    inner.append(&btn_row);
    card.append(&inner);
    card.upcast()
}

fn build_storage_card() -> gtk::Widget {
    let card = glass_panel();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 20);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    inner.append(&section_header("drive-harddisk-symbolic", "Storage", "Manage your local cache"));

    // Usage bar placeholder
    let usage_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let usage_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let used_label = gtk::Label::new(Some("Used: --"));
    used_label.add_css_class("dim-label");
    used_label.set_hexpand(true);
    let free_label = gtk::Label::new(Some("Free: --"));
    free_label.add_css_class("dim-label");
    usage_header.append(&used_label);
    usage_header.append(&free_label);
    usage_box.append(&usage_header);

    let bar_bg = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar_bg.set_size_request(-1, 8);
    bar_bg.add_css_class("settings-row");
    let bar_fill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar_fill.set_size_request(0, 8);
    bar_bg.append(&bar_fill);
    usage_box.append(&bar_bg);
    inner.append(&usage_box);

    // Action rows
    inner.append(&action_row("Cache Management", "folder-documents-symbolic"));
    inner.append(&action_row("Change Location", "folder-open-symbolic"));

    card.append(&inner);
    card.upcast()
}

fn build_audio_quality_card() -> gtk::Widget {
    let card = glass_panel();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    inner.append(&section_header("audio-x-generic-symbolic", "Audio Quality", "Configure streaming and download quality"));

    // Streaming quality segmented control
    let quality_label = gtk::Label::new(Some("STREAMING QUALITY"));
    quality_label.add_css_class("dim-label");
    quality_label.set_halign(gtk::Align::Start);
    inner.append(&quality_label);

    let seg_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    for (i, label) in ["Normal", "High", "Extreme"].iter().enumerate() {
        let btn = gtk::Button::with_label(label);
        if i == 2 {
            btn.add_css_class("suggested-action");
        }
        seg_box.append(&btn);
    }
    inner.append(&seg_box);

    // Download quality
    let dl_label = gtk::Label::new(Some("DOWNLOAD QUALITY"));
    dl_label.add_css_class("dim-label");
    dl_label.set_halign(gtk::Align::Start);
    inner.append(&dl_label);

    let combo = gtk::DropDown::from_strings(&[
        "Lossless (FLAC 24-bit/192kHz)",
        "High (AAC 320kbps)",
        "Standard (AAC 128kbps)",
    ]);
    combo.set_selected(0);
    inner.append(&combo);

    // Equalizer toggle
    inner.append(&toggle_row("Equalizer", "Custom profile active", true));

    card.append(&inner);
    card.upcast()
}

fn build_playback_card() -> gtk::Widget {
    let card = glass_panel();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    inner.append(&section_header("media-playback-start-symbolic", "Playback", "Configure playback behavior"));

    // Crossfade
    let cf_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let cf_label = gtk::Label::new(Some("Crossfade"));
    cf_label.set_hexpand(true);
    cf_label.set_halign(gtk::Align::Start);
    cf_header.append(&cf_label);
    let cf_value = gtk::Label::new(Some("6 Seconds"));
    cf_value.add_css_class("dim-label");
    cf_header.append(&cf_value);
    inner.append(&cf_header);

    let cf_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 12.0, 1.0);
    cf_scale.set_value(6.0);
    inner.append(&cf_scale);

    // Gapless
    inner.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    inner.append(&toggle_row("Gapless Playback", "Eliminate silence between tracks", true));

    // Hardware acceleration
    inner.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    inner.append(&toggle_row("Hardware Acceleration", "Offload processing to GPU", false));

    card.append(&inner);
    card.upcast()
}

fn build_appearance_card() -> gtk::Widget {
    let card = glass_panel();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    inner.append(&section_header("preferences-desktop-theme-symbolic", "Appearance", "Customize the look and feel"));

    // Three-column layout
    let cols = gtk::Box::new(gtk::Orientation::Horizontal, 48);
    cols.set_halign(gtk::Align::Fill);

    // -- Theme Mode --
    let theme_col = gtk::Box::new(gtk::Orientation::Vertical, 12);
    theme_col.set_hexpand(true);

    let theme_label = gtk::Label::new(Some("THEME MODE"));
    theme_label.add_css_class("dim-label");
    theme_label.set_halign(gtk::Align::Start);
    theme_col.append(&theme_label);

    let theme_group = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for (i, (label, icon_name)) in [
        ("Deep Dark", "weather-clear-night-symbolic"),
        ("Light", "weather-clear-symbolic"),
        ("System", "computer-symbolic"),
    ]
    .iter()
    .enumerate()
    {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("settings-theme-option");
        if i == 0 {
            row.add_css_class("active");
        }
        let img = gtk::Image::from_icon_name(icon_name);
        img.set_pixel_size(20);
        row.append(&img);
        let lbl = gtk::Label::new(Some(label));
        lbl.set_hexpand(true);
        lbl.set_halign(gtk::Align::Start);
        row.append(&lbl);
        theme_group.append(&row);
    }
    theme_col.append(&theme_group);
    cols.append(&theme_col);

    // -- Accent Color --
    let accent_col = gtk::Box::new(gtk::Orientation::Vertical, 12);
    accent_col.set_hexpand(true);

    let accent_label = gtk::Label::new(Some("ACCENT COLOR"));
    accent_label.add_css_class("dim-label");
    accent_label.set_halign(gtk::Align::Start);
    accent_col.append(&accent_label);

    let color_grid = gtk::Grid::new();
    color_grid.set_column_spacing(12);
    color_grid.set_row_spacing(12);
    color_grid.set_halign(gtk::Align::Start);
    let colors = [
        ("settings-color-0", "#cdbdff"),
        ("settings-color-1", "#a6e6ff"),
        ("settings-color-2", "#ffb688"),
        ("settings-color-3", "#ffb4ab"),
        ("settings-color-4", "#4cd6ff"),
        ("settings-color-5", "#b7eaff"),
    ];
    {
        let rules: String = colors
            .iter()
            .map(|(cls, hex)| format!(".{cls} {{ background-color: {hex}; border-radius: 20px; }}"))
            .collect::<Vec<_>>()
            .join("\n");
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&rules);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("no default display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    for (i, (cls, _hex)) in colors.iter().enumerate() {
        let swatch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        swatch.set_size_request(40, 40);
        swatch.add_css_class("settings-color-swatch");
        swatch.add_css_class(cls);
        if i == 0 {
            swatch.add_css_class("active");
        }
        color_grid.attach(&swatch, (i % 3) as i32, (i / 3) as i32, 1, 1);
    }
    accent_col.append(&color_grid);
    cols.append(&accent_col);

    // -- Interface Density --
    let density_col = gtk::Box::new(gtk::Orientation::Vertical, 12);
    density_col.set_hexpand(true);

    let density_label = gtk::Label::new(Some("INTERFACE DENSITY"));
    density_label.add_css_class("dim-label");
    density_label.set_halign(gtk::Align::Start);
    density_col.append(&density_label);

    let density_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let compact = gtk::Label::new(Some("Compact"));
    compact.add_css_class("dim-label");
    compact.set_hexpand(true);
    let comfortable = gtk::Label::new(Some("Comfortable"));
    comfortable.add_css_class("dim-label");
    comfortable.set_hexpand(true);
    comfortable.set_halign(gtk::Align::Center);
    let spacious = gtk::Label::new(Some("Spacious"));
    spacious.add_css_class("dim-label");
    spacious.set_hexpand(true);
    spacious.set_halign(gtk::Align::End);
    density_header.append(&compact);
    density_header.append(&comfortable);
    density_header.append(&spacious);
    density_col.append(&density_header);

    let density_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    density_scale.set_value(50.0);
    density_col.append(&density_scale);

    let hint = gtk::Label::new(Some(
        "\"This changes the spacing and font sizes across the application.\"",
    ));
    hint.add_css_class("dim-label");
    hint.set_wrap(true);
    hint.set_halign(gtk::Align::Center);
    hint.set_margin_top(8);
    density_col.append(&hint);

    cols.append(&density_col);
    inner.append(&cols);

    card.append(&inner);
    card.upcast()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn action_row(label: &str, _icon: &str) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-row");

    let lbl = gtk::Label::new(Some(label));
    lbl.set_hexpand(true);
    lbl.set_halign(gtk::Align::Start);
    row.append(&lbl);

    let chevron = gtk::Image::from_icon_name("chevron-right-symbolic");
    chevron.set_pixel_size(16);
    chevron.add_css_class("dim-label");
    row.append(&chevron);

    row.upcast()
}

fn find_button_by_label(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Ok(btn) = widget.clone().downcast::<gtk::Button>() {
        if let Some(child) = btn.child() {
            if let Ok(lbl) = child.downcast::<gtk::Label>() {
                if lbl.text().as_str() == label {
                    return Some(btn);
                }
            }
        }
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(btn) = find_button_by_label(&c, label) {
            return Some(btn);
        }
        child = c.next_sibling();
    }
    None
}
