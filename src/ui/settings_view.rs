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
        page_title.add_css_class("display-lg");
        page_title.set_halign(gtk::Align::Start);
        let page_subtitle = gtk::Label::new(Some(
            "Manage your account preferences and audio experience.",
        ));
        page_subtitle.add_css_class("dim-label");
        page_subtitle.set_halign(gtk::Align::Start);
        title_box.append(&page_title);
        title_box.append(&page_subtitle);
        content.append(&title_box);

        // Bento grid container
        let grid = gtk::Grid::new();
        grid.set_column_spacing(24);
        grid.set_row_spacing(24);
        grid.set_margin_start(32);
        grid.set_margin_end(32);
        grid.set_margin_bottom(32);
        grid.set_halign(gtk::Align::Fill);

        let profile = UserProfile::load(&data_dir).unwrap_or_else(UserProfile::guest);

        // -- Account (8 cols) ---------------------------------------------------
        let account_card = build_account_card(&profile);
        grid.attach(&account_card, 0, 0, 8, 1);

        // -- Storage (4 cols) ---------------------------------------------------
        let storage_card = build_storage_card();
        grid.attach(&storage_card, 8, 0, 4, 1);

        // -- Audio Quality (6 cols) ---------------------------------------------
        let audio_card = build_audio_quality_card();
        grid.attach(&audio_card, 0, 1, 6, 1);

        // -- Playback (6 cols) --------------------------------------------------
        let playback_card = build_playback_card();
        grid.attach(&playback_card, 6, 1, 6, 1);

        // -- Appearance (12 cols) ------------------------------------------------
        let appearance_card = build_appearance_card();
        grid.attach(&appearance_card, 0, 2, 12, 1);

        content.append(&grid);

        // -- Logout wiring -------------------------------------------------------
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
// Bento cards
// ---------------------------------------------------------------------------

fn bento_card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("settings-card");
    card
}

fn build_account_card(profile: &UserProfile) -> gtk::Widget {
    let card = bento_card();
    card.set_margin_start(0);
    card.set_margin_end(0);

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    // Header row: icon + title + badge
    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header_row.set_halign(gtk::Align::Start);

    let icon = gtk::Image::from_icon_name("avatar-default-symbolic");
    icon.set_pixel_size(20);
    icon.add_css_class("settings-icon");
    header_row.append(&icon);

    let title = gtk::Label::new(Some("Account"));
    title.add_css_class("headline-md");
    header_row.append(&title);

    let badge = gtk::Label::new(Some("ACTIVE"));
    badge.add_css_class("settings-badge");
    header_row.append(&badge);

    inner.append(&header_row);

    // Profile row
    let profile_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    profile_row.set_halign(gtk::Align::Start);
    profile_row.set_valign(gtk::Align::Center);

    // Avatar circle
    let avatar_frame = gtk::Frame::new(None);
    avatar_frame.add_css_class("settings-avatar");
    avatar_frame.set_size_request(96, 96);
    let avatar_icon = gtk::Image::from_icon_name("avatar-default-symbolic");
    avatar_icon.set_pixel_size(40);
    avatar_icon.set_halign(gtk::Align::Center);
    avatar_icon.set_valign(gtk::Align::Center);
    avatar_frame.set_child(Some(&avatar_icon));
    profile_row.append(&avatar_frame);

    // Name + email
    let info = gtk::Box::new(gtk::Orientation::Vertical, 4);
    info.set_valign(gtk::Align::Center);

    let name_label = gtk::Label::new(Some(&profile.name));
    name_label.add_css_class("settings-profile-name");
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

    // Buttons row
    let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    btn_row.set_halign(gtk::Align::End);

    let manage_btn = gtk::Button::with_label("Manage Plan");
    manage_btn.add_css_class("flat");
    manage_btn.add_css_class("settings-outline-btn");
    btn_row.append(&manage_btn);

    let signout_btn = gtk::Button::with_label("Sign Out");
    signout_btn.add_css_class("destructive-action");
    btn_row.append(&signout_btn);

    inner.append(&btn_row);
    card.append(&inner);
    card.upcast()
}

fn build_storage_card() -> gtk::Widget {
    let card = bento_card();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 20);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    // Header
    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header_row.set_halign(gtk::Align::Start);
    let icon = gtk::Image::from_icon_name("drive-harddisk-symbolic");
    icon.set_pixel_size(20);
    icon.add_css_class("settings-icon");
    header_row.append(&icon);
    let title = gtk::Label::new(Some("Storage"));
    title.add_css_class("headline-md");
    header_row.append(&title);
    inner.append(&header_row);

    // Usage bar (placeholder)
    let usage_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let usage_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let used_label = gtk::Label::new(Some("Used: --"));
    used_label.add_css_class("caption");
    used_label.add_css_class("dim-label");
    used_label.set_hexpand(true);
    let free_label = gtk::Label::new(Some("Free: --"));
    free_label.add_css_class("caption");
    free_label.add_css_class("dim-label");
    usage_header.append(&used_label);
    usage_header.append(&free_label);
    usage_box.append(&usage_header);

    let bar_bg = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar_bg.add_css_class("settings-progress-track");
    bar_bg.set_size_request(-1, 8);
    let bar_fill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar_fill.add_css_class("settings-progress-fill");
    bar_fill.set_size_request(0, 8); // placeholder: no data
    bar_bg.append(&bar_fill);
    usage_box.append(&bar_bg);
    inner.append(&usage_box);

    // Action rows
    inner.append(&settings_row("Cache Management", "folder-documents-symbolic"));
    inner.append(&settings_row("Change Location", "folder-open-symbolic"));

    card.append(&inner);
    card.upcast()
}

fn build_audio_quality_card() -> gtk::Widget {
    let card = bento_card();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    // Header
    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header_row.set_halign(gtk::Align::Start);
    let icon = gtk::Image::from_icon_name("audio-x-generic-symbolic");
    icon.set_pixel_size(20);
    icon.add_css_class("settings-icon");
    header_row.append(&icon);
    let title = gtk::Label::new(Some("Audio Quality"));
    title.add_css_class("headline-md");
    header_row.append(&title);
    inner.append(&header_row);

    // Streaming quality segmented control (placeholder)
    let quality_label = gtk::Label::new(Some("STREAMING QUALITY"));
    quality_label.add_css_class("caption");
    quality_label.add_css_class("dim-label");
    quality_label.set_halign(gtk::Align::Start);
    inner.append(&quality_label);

    let seg_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    seg_box.add_css_class("settings-segmented");
    for (i, label) in ["Normal", "High", "Extreme"].iter().enumerate() {
        let btn = gtk::Button::with_label(label);
        btn.add_css_class("settings-seg-btn");
        if i == 2 {
            btn.add_css_class("active");
        }
        seg_box.append(&btn);
    }
    inner.append(&seg_box);

    // Download quality (placeholder dropdown)
    let dl_label = gtk::Label::new(Some("DOWNLOAD QUALITY"));
    dl_label.add_css_class("caption");
    dl_label.add_css_class("dim-label");
    dl_label.set_halign(gtk::Align::Start);
    inner.append(&dl_label);

    let combo = gtk::DropDown::from_strings(&[
        "Lossless (FLAC 24-bit/192kHz)",
        "High (AAC 320kbps)",
        "Standard (AAC 128kbps)",
    ]);
    combo.add_css_class("settings-dropdown");
    combo.set_selected(0);
    inner.append(&combo);

    // Equalizer toggle (placeholder)
    let eq_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    eq_row.add_css_class("settings-toggle-row");
    let eq_info = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let eq_title = gtk::Label::new(Some("Equalizer"));
    eq_title.set_halign(gtk::Align::Start);
    eq_title.set_hexpand(true);
    let eq_sub = gtk::Label::new(Some("Custom profile active"));
    eq_sub.add_css_class("dim-label");
    eq_sub.set_halign(gtk::Align::Start);
    eq_info.append(&eq_title);
    eq_info.append(&eq_sub);
    eq_row.append(&eq_info);
    let eq_switch = gtk::Switch::new();
    eq_switch.set_active(true);
    eq_switch.set_valign(gtk::Align::Center);
    eq_row.append(&eq_switch);
    inner.append(&eq_row);

    card.append(&inner);
    card.upcast()
}

fn build_playback_card() -> gtk::Widget {
    let card = bento_card();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    // Header
    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header_row.set_halign(gtk::Align::Start);
    let icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
    icon.set_pixel_size(20);
    icon.add_css_class("settings-icon");
    header_row.append(&icon);
    let title = gtk::Label::new(Some("Playback"));
    title.add_css_class("headline-md");
    header_row.append(&title);
    inner.append(&header_row);

    // Crossfade slider (placeholder)
    let cf_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let cf_label = gtk::Label::new(Some("Crossfade"));
    cf_label.set_hexpand(true);
    cf_label.set_halign(gtk::Align::Start);
    cf_label.add_css_class("body-bold");
    cf_header.append(&cf_label);
    let cf_value = gtk::Label::new(Some("6 Seconds"));
    cf_value.add_css_class("settings-value");
    cf_header.append(&cf_value);
    inner.append(&cf_header);

    let cf_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 12.0, 1.0);
    cf_scale.set_value(6.0);
    cf_scale.add_css_class("settings-scale");
    inner.append(&cf_scale);

    // Gapless toggle
    inner.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let gapless_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    gapless_row.add_css_class("settings-toggle-row");
    let gap_info = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let gap_title = gtk::Label::new(Some("Gapless Playback"));
    gap_title.set_halign(gtk::Align::Start);
    gap_title.set_hexpand(true);
    let gap_sub = gtk::Label::new(Some("Eliminate silence between tracks"));
    gap_sub.add_css_class("dim-label");
    gap_sub.set_halign(gtk::Align::Start);
    gap_info.append(&gap_title);
    gap_info.append(&gap_sub);
    gapless_row.append(&gap_info);
    let gap_switch = gtk::Switch::new();
    gap_switch.set_active(true);
    gap_switch.set_valign(gtk::Align::Center);
    gapless_row.append(&gap_switch);
    inner.append(&gapless_row);

    // Hardware acceleration toggle
    inner.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let hw_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    hw_row.add_css_class("settings-toggle-row");
    let hw_info = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let hw_title = gtk::Label::new(Some("Hardware Acceleration"));
    hw_title.set_halign(gtk::Align::Start);
    hw_title.set_hexpand(true);
    let hw_sub = gtk::Label::new(Some("Offload processing to GPU"));
    hw_sub.add_css_class("dim-label");
    hw_sub.set_halign(gtk::Align::Start);
    hw_info.append(&hw_title);
    hw_info.append(&hw_sub);
    hw_row.append(&hw_info);
    let hw_switch = gtk::Switch::new();
    hw_switch.set_active(false);
    hw_switch.set_valign(gtk::Align::Center);
    hw_row.append(&hw_switch);
    inner.append(&hw_row);

    card.append(&inner);
    card.upcast()
}

fn build_appearance_card() -> gtk::Widget {
    let card = bento_card();

    let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
    inner.set_margin_top(24);
    inner.set_margin_bottom(24);
    inner.set_margin_start(24);
    inner.set_margin_end(24);

    // Header
    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header_row.set_halign(gtk::Align::Start);
    let icon = gtk::Image::from_icon_name("preferences-desktop-theme-symbolic");
    icon.set_pixel_size(20);
    icon.add_css_class("settings-icon");
    header_row.append(&icon);
    let title = gtk::Label::new(Some("Appearance"));
    title.add_css_class("headline-md");
    header_row.append(&title);
    inner.append(&header_row);

    // Three-column grid
    let cols = gtk::Box::new(gtk::Orientation::Horizontal, 48);
    cols.set_halign(gtk::Align::Fill);

    // -- Theme Mode --
    let theme_col = gtk::Box::new(gtk::Orientation::Vertical, 12);
    theme_col.set_hexpand(true);

    let theme_label = gtk::Label::new(Some("THEME MODE"));
    theme_label.add_css_class("caption");
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
    accent_label.add_css_class("caption");
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
    // Build a single CSS provider for all swatch colors
    {
        let rules: String = colors
            .iter()
            .map(|(cls, hex)| format!(".{cls} {{ background-color: {hex}; }}"))
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
        swatch.set_size_request(36, 36);
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
    density_label.add_css_class("caption");
    density_label.add_css_class("dim-label");
    density_label.set_halign(gtk::Align::Start);
    density_col.append(&density_label);

    let density_header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let compact = gtk::Label::new(Some("Compact"));
    compact.add_css_class("caption");
    compact.set_hexpand(true);
    let comfortable = gtk::Label::new(Some("Comfortable"));
    comfortable.add_css_class("caption");
    comfortable.add_css_class("body-bold");
    comfortable.add_css_class("settings-accent-text");
    comfortable.set_hexpand(true);
    comfortable.set_halign(gtk::Align::Center);
    let spacious = gtk::Label::new(Some("Spacious"));
    spacious.add_css_class("caption");
    spacious.set_hexpand(true);
    spacious.set_halign(gtk::Align::End);
    density_header.append(&compact);
    density_header.append(&comfortable);
    density_header.append(&spacious);
    density_col.append(&density_header);

    let density_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    density_scale.set_value(50.0);
    density_scale.add_css_class("settings-scale");
    density_col.append(&density_scale);

    let hint = gtk::Label::new(Some(
        "\"This changes the spacing and font sizes across the application.\"",
    ));
    hint.add_css_class("dim-label");
    hint.add_css_class("caption");
    hint.set_wrap(true);
    hint.set_halign(gtk::Align::Center);
    hint.set_margin_top(8);
    hint.set_css_classes(&["dim-label", "caption", "settings-hint"]);
    density_col.append(&hint);

    cols.append(&density_col);
    inner.append(&cols);

    card.append(&inner);
    card.upcast()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn settings_row(label: &str, _icon: &str) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-action-row");

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
    // Check if this widget is a Button with matching label
    if let Ok(btn) = widget.clone().downcast::<gtk::Button>() {
        if let Some(child) = btn.child() {
            if let Ok(lbl) = child.downcast::<gtk::Label>() {
                if lbl.text().as_str() == label {
                    return Some(btn);
                }
            }
        }
    }
    // Recurse into children
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(btn) = find_button_by_label(&c, label) {
            return Some(btn);
        }
        child = c.next_sibling();
    }
    None
}
