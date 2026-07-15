# Melofin Design Improvement Plan

Phased implementation guide for improving the visual design of Melofin, aligned with the Stitch design system and GNOME/Adwaita conventions.

---

## Current State

### What Exists

- Single CSS file: `src/ui/style.css` (74 lines)
- Pure black backgrounds (`#000000`, `#0a0a0a`)
- Hardcoded hex colors everywhere, no design tokens
- Purple gradient on hero card (`#4a2f6b` -> `#1e1a3c`)
- Heavy reliance on libadwaita built-in classes (`suggested-action`, `card`, `boxed-list`, `pill`, etc.)
- Dark-only, no theme switching

### Problems

1. **No accent color integration** — The app ignores the user's GNOME accent color setting. Buttons use `suggested-action` (which inherits accent), but backgrounds and highlights are hardcoded.
2. **Colors are scattered** — Every hex value is inline in CSS. Changing the palette means editing many rules.
3. **No visual depth system** — All surfaces are flat solid colors with no hierarchy (sidebar vs content vs elevated surfaces).
4. **Missing polish** — No hover effects on cards, no transitions, no focus glow on inputs, no progress bar styling.

---

## Design Goals

| Goal | Approach |
|------|----------|
| Respect system accent color | Use Adwaita named colors (`@accent_bg_color`, `@accent_fg_color`) |
| Solid surfaces (no glassmorphism) | Use Adwaita surface color tokens for depth hierarchy |
| Consistent with GNOME ecosystem | Leverage existing libadwaita CSS classes, minimize overrides |
| Page-by-page implementation | Start with global tokens, then tackle one page at a time |
| Match Stitch design intent | Adopt layout proportions, spacing, and component patterns from Stitch |

---

## CSS Architecture

### File Structure

Split the monolithic `style.css` into component-scoped files under `src/ui/styles/`:

```
src/ui/styles/
  tokens.css      — @define-color tokens, global resets, spacing constants
  top_bar.css     — .top-bar, .top-bar-search
  sidebar.css     — .sidebar, .library-chip
  home.css        — .home-card, .home-art, .hero-card
  player.css      — .toolbar, progress bar styling
  skeleton.css    — .skeleton-block, animation keyframes
```

### Loading Strategy

**Compile-time concatenation** — all files merged into one string at build time via `include_str!`. Zero runtime cost, keeps current pattern:

```rust
// In window.rs load_css()
let css = [
    include_str!("styles/tokens.css"),
    include_str!("styles/top_bar.css"),
    include_str!("styles/sidebar.css"),
    include_str!("styles/home.css"),
    include_str!("styles/player.css"),
    include_str!("styles/skeleton.css"),
].join("\n");

let provider = gtk::CssProvider::new();
provider.load_from_data(&css);
gtk::style_context_add_provider_for_display(
    &gdk::Display::default().expect("Could not connect to display"),
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
);
```

### Rules

1. **One concern per file** — each file owns its component's styles
2. **tokens.css is imported first** — all other files depend on its `@define-color` definitions
3. **No cross-file selectors** — if a rule affects multiple components, it belongs in `tokens.css`
4. **Naming convention** — filenames match the Rust module names (`home_view.rs` -> `home.css`)

---

## Color Strategy

### Using Adwaita Named Colors

GTK4/libadwaita exposes the user's theme colors as named CSS colors. We reference these instead of hardcoding hex values:

| Token | Purpose | Adwaita Value (default dark) |
|-------|---------|------------------------------|
| `@accent_bg_color` | Primary action backgrounds (buttons, active states) | User's chosen accent |
| `@accent_fg_color` | Text on accent backgrounds | Usually white |
| `@accent_color` | Accent without alpha | User's chosen accent |
| `@window_bg_color` | Main window background | `#242424` (Adwaita default dark) |
| `@headerbar_bg_color` | Headerbar / top bar | `#242424` or slightly different |
| `@view_bg_color` | Content area background | `#1e1e1e` |
| `@sidebar_bg_color` | Sidebar background | `#242424` |
| `@card_bg_color` | Card surfaces | `rgba(255,255,255,0.04)` |
| `@borders` | Border color | `rgba(255,255,255,0.08)` |
| `@shade_color` | Shadows/overlays | `rgba(0,0,0,0.36)` |
| `@header_fg_color` | Text on headerbar | `#ffffff` |
| `@view_fg_color` | Text in content areas | `#ffffff` |
| `@status_error_color` | Error/destructive | `#ff7b63` |

### Depth Hierarchy (Solid, No Transparency)

```
Layer 0 (Base):        @window_bg_color          — main background
Layer 1 (Sidebar):     @sidebar_bg_color          — left sidebar
Layer 2 (Content):     @view_bg_color             — content area behind cards
Layer 3 (Cards):        @card_bg_color             — elevated card surfaces
Layer 4 (Highest):     @window_bg_color + border  — popovers, dialogs
```

### Accent Color Usage

The user's GNOME accent color will automatically color:
- Primary buttons (`suggested-action` class)
- Active/selected states
- Toggle switches and radio buttons
- Progress bars (when using Adwaita classes)

For custom accent usage (card highlights, hover glows), use `@accent_bg_color` in CSS.

---

## Typography

### Strategy

Adwaita provides its own font stack. We keep the system fonts and only customize where needed:

- **Headings**: Adwaita default (usually Cantarell or system font)
- **Body**: Adwaita default
- **Labels/captions**: Use `dim-label` class (already in use)
- **Monospace/metadata**: Not needed for now

### Stitch Reference

The Stitch design uses Inter + Geist. Since we're targeting native GNOME, we stick with Adwaita's font system. If Inter is desired later, it can be set via:
```css
@import url("resource:///org/gnome/Adwaita/Adwaita.css");
```
Or by overriding `font-family` on the window.

---

## Spacing & Shapes

### From Stitch Design System

| Token | Value | Usage |
|-------|-------|-------|
| Spacing unit | 8px | Base grid |
| Container padding | 32px | Main content area margins |
| Gutter | 24px | Gap between cards |
| Sidebar width | 280px | Left library sidebar |
| Player height | 96px | Bottom player bar |

### Border Radius

| Element | Radius | Notes |
|---------|--------|-------|
| Search pill | 999px | Already correct |
| Standard buttons | 999px (pill) | Adwaita default for pill style |
| Cards | 12px | Slightly larger than current 8px |
| Art thumbnails | 12px | Match cards |
| Library chips | 999px | Already correct |
| Hero card | 16px | Larger, more prominent |

---

## Stitch Screens Reference

The Stitch project "Sleek Linux Music Player" contains these designs:

| Screen | Stitch ID | Status |
|--------|-----------|--------|
| Library Dashboard | `ea3ed206` | **First target** |
| Now Playing View | `0dbf4c4c` | Pending |
| Explore & Search | `9ed04f81` | Pending |
| Artist Profile | `d8c1face` | Pending |
| Settings | `6a0ea73b` | Pending |

---

## Implementation Phases

### Phase 0: Global Design Tokens

**Goal:** Establish the color and spacing foundation that all pages will use.

**Files to create/modify:**
- Create `src/ui/styles/tokens.css` — all `@define-color` tokens, global resets
- Create `src/ui/styles/top_bar.css` — top bar and search styles
- Create `src/ui/styles/sidebar.css` — sidebar and library chip styles
- Create `src/ui/styles/home.css` — home cards, art, hero card styles
- Create `src/ui/styles/player.css` — player bar and progress bar styles
- Create `src/ui/styles/skeleton.css` — skeleton loading animation
- Modify `src/ui/window.rs` — update `load_css()` to include all style files
- Delete `src/ui/style.css` — replaced by the split files

**Changes:**
1. Create `tokens.css` with `@define-color` rules for all Adwaita named colors
2. Split existing CSS rules into their respective component files
3. Replace all hardcoded hex colors with Adwaita named color references
4. Update window background to use `@window_bg_color`
5. Update sidebar background to use `@sidebar_bg_color`
6. Update card hover effects to use `alpha(@accent_bg_color, 0.08)`
7. Update border radius values (cards → 12px, hero → 16px)
8. Add focus ring styling using `@accent_bg_color`

**Color mapping:**

| Current Hardcoded | Replace With |
|-------------------|-------------|
| `#000000` (window) | `@window_bg_color` |
| `#000000` (top bar) | `@headerbar_bg_color` |
| `#0a0a0a` (sidebar) | `@sidebar_bg_color` |
| `#2a2a2a` (search bg, art bg, skeleton) | `alpha(@window_fg_color, 0.1)` or `@card_bg_color` |
| `#3a3a3a` (search focus, art hover) | `alpha(@window_fg_color, 0.15)` |
| `alpha(#ffffff, 0.06)` (card hover) | `alpha(@accent_bg_color, 0.08)` |
| `#4a2f6b, #1e1a3c` (hero gradient) | `@accent_bg_color` (solid, or subtle gradient using accent) |

**Verification:**
```bash
cargo build
# Verify all style files compile correctly
# Manual: run app, verify backgrounds adapt to system theme
# Change GNOME accent color in Settings > Appearance, verify it updates
# Check that no styles are missing (compare with old style.css)
```

---

### Phase 1: Library Dashboard (Home Page)

**Goal:** Align the home/main view with the Stitch "Library Dashboard" design.

**Screens in Stitch:** `ea3ed206d23244e2adfa386d9bb31777` (Library Dashboard)

**Current files:**
- `src/ui/home_view.rs` — Main content area with cards
- `src/ui/library_sidebar.rs` — Left sidebar with library list
- `src/ui/now_playing_panel.rs` — Right panel (currently "Now Playing")
- `src/ui/player_bar.rs` — Bottom player bar
- `src/ui/top_bar.rs` — Top search bar

**Changes per component:**

#### Home View (`home_view.rs` + CSS)
- Update hero card gradient to use `@accent_bg_color` (solid or subtle)
- Ensure home cards use 12px border radius
- Add hover lift effect: `transition: transform 200ms ease` + `transform: translateY(-2px)`
- Add art glow on hover: `box-shadow: 0 0 20px alpha(@accent_bg_color, 0.3)`
- Ensure consistent 8px spacing grid

#### Library Sidebar (`library_sidebar.rs` + CSS)
- Verify background uses `@sidebar_bg_color`
- Active library items: `background-color: alpha(@accent_bg_color, 0.15)` with 3px left bar in `@accent_bg_color`
- Hover state: `alpha(@window_fg_color, 0.06)`
- Ensure sidebar width is 280px

#### Player Bar (`player_bar.rs` + CSS)
- Ensure background uses `@headerbar_bg_color` or `@window_bg_color`
- Add progress bar styling: 4px height, `@accent_bg_color` fill, 8px on hover
- Ensure player height is 96px

#### Top Bar (`top_bar.rs` + CSS)
- Search pill background: `alpha(@window_fg_color, 0.08)`
- Search focus: `alpha(@accent_bg_color, 0.12)` with subtle accent border
- Ensure pill shape (already 999px)

**Verification:**
```bash
cargo build
# Manual: test home page with different accent colors
# Verify hover effects work smoothly
# Check sidebar active states
# Check player bar progress styling
```

---

### Phase 2: Now Playing View

**Goal:** Implement the "Now Playing" screen from Stitch.

**Stitch screen:** `0dbf4c4ce13f4bdfa3357298922c9a09` (Now Playing View)

**Current files:**
- `src/ui/now_playing_panel.rs` — Right panel with album art + track info
- `src/ui/queue_panel.rs` — Queue list within the panel

**Changes:**
- Album art: larger display (224px from current `PANEL_WIDTH - 2*PANEL_MARGIN`)
- Track info: title in `title-2` class, artist in `dim-label`
- Progress bar: prominent, accent-colored, with timestamp labels
- Queue list: `boxed-list` styling, currently playing track highlighted with `@accent_bg_color`
- Heart/like button: accent-colored when active

**Verification:**
```bash
cargo build
# Manual: play a track, verify now playing panel layout
# Check queue list styling and active track highlight
```

---

### Phase 3: Explore & Search

**Goal:** Implement the search results and browse/explore page.

**Stitch screen:** `9ed04f816072404ab81de7e8ded6dc30` (Explore & Search)

**Current files:**
- `src/ui/search_view.rs` — Search results with skeleton loading

**Changes:**
- Search results: `boxed-list` with consistent row styling
- Skeleton loading blocks: use `@card_bg_color` instead of hardcoded `#2a2a2a`
- Search popover: card styling with proper border radius
- Empty state: centered message with `dim-label`

**Verification:**
```bash
cargo build
# Manual: search for artists/albums/songs, verify result styling
# Check skeleton loading animation
# Test empty state
```

---

### Phase 4: Artist Profile / Detail View

**Goal:** Implement the artist profile and detail view screens.

**Stitch screens:**
- `d8c1facedfae4f389e9bd51d6cc1e611` (Artist Profile)

**Current files:**
- `src/ui/detail_view.rs` — Artist/album detail with track list

**Changes:**
- Hero section: artist image with gradient overlay using `@accent_bg_color`
- Track list: `boxed-list` with consistent row heights
- Play all / shuffle buttons: `suggested-action` styling
- Liked songs count: `dim-label` with caption styling

**Verification:**
```bash
cargo build
# Manual: navigate to artist page, verify hero layout
# Check track list styling
# Verify play/shuffle buttons
```

---

### Phase 5: Settings & Polish

**Goal:** Implement settings screen and final polish pass.

**Stitch screen:** `6a0ea73bc7074834adabdac9411e5dec` (Settings)

**Current files:**
- `src/ui/login_dialog.rs` — Login dialog (closest to settings)

**Changes:**
- Settings page with `adw::PreferencesGroup` styling
- Account section with profile info
- Logout button: `destructive-action` class
- About section with version info
- Final polish: transitions, focus states, keyboard navigation hints

**Verification:**
```bash
cargo build
# Manual: open settings, verify all sections
# Test logout flow
# Check keyboard navigation
```

---

## Summary

| Phase | Scope | Estimated Time | Prerequisites |
|-------|-------|---------------|---------------|
| 0 | Global design tokens | ~1 hour | None |
| 1 | Library Dashboard | ~3-4 hours | Phase 0 |
| 2 | Now Playing View | ~2-3 hours | Phase 0 |
| 3 | Explore & Search | ~2 hours | Phase 0 |
| 4 | Artist Profile | ~2 hours | Phase 0 |
| 5 | Settings & Polish | ~2 hours | Phases 0-4 |

**Total estimated time:** ~12-14 hours

Phases 1-4 can be done in parallel after Phase 0, but sequential implementation is recommended for consistency.

---

## Stitch Design System Reference

The full Stitch design system is defined in project `11712536191264151278` with these key specs:

- **Accent Color:** `#7c4dff` (Electric Violet) — used as reference, actual color comes from GNOME theme
- **Secondary:** `#14d1ff` (Digital Cyan) — optional, for secondary highlights
- **Surface:** `#131313` — replaced by Adwaita surface tokens
- **Font:** Inter — replaced by Adwaita system fonts
- **Label Font:** Geist — not used (Adwaita default)
- **Roundness:** `ROUND_EIGHT` (8px default, 12px for cards, 16px for hero)
- **Spacing Unit:** 8px

The Stitch design MD file contains the full brand narrative and component specifications. It serves as the visual reference for all implementation phases.
