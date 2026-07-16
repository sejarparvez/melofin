# Melofin Design Improvement Plan

Phased implementation guide for improving the visual design of Melofin, using the Stitch design system as the canonical source for colors, spacing, and component patterns.

---

## Current State

### What Exists

- **CSS split into 7 files** under `src/ui/styles/`: `tokens.css`, `top_bar.css`, `sidebar.css`, `home.css`, `player.css`, `skeleton.css`, `detail.css`
- Monolithic `src/ui/style.css` has been deleted
- `window.rs` loads all files via `include_str!` concatenation (compile-time)
- Card hover effects (`translateY(-4px)`, accent glow)
- Hero card with solid `#7c4dff` background, 16px radius
- Progress bar with 3px track → 5px on hover, slider handle
- Sidebar active states with accent background + left bar
- `detail_view.rs` + `detail.css` — artist hero, track table, about card
- `now_playing_panel.rs` — implemented but **not mounted** in main window layout
- `queue_panel.rs` — implemented but **not mounted** in main window layout

### What Still Needs to Be Done

1. **Hardcoded hex values scattered across CSS files** — `tokens.css` defines `@define-color` tokens but the other CSS files still use raw hex (`#131313`, `#7c4dff`, `#e5e2e1`, `rgba(255,255,255,0.05)`) instead of referencing the tokens. This means changing any color requires editing many rules.

2. **No visual depth hierarchy** — All surfaces are flat `#131313` with no differentiation between sidebar, content area, and cards. The Stitch design specifies 5 layers using surface tones.

3. **Missing CSS for some views** — `liked_songs_view.rs` and `search_view.rs` have no custom CSS files. `queue_panel.rs` has no CSS.

4. **Now Playing + Queue panels not wired** — Both exist as modules but aren't mounted in the window layout.

5. **No focus ring styling** — No focus rings defined anywhere.

6. **Duplicate `.section-title`** — Defined in both `home.css:16` and `detail.css:85`.

7. **Progress bar fill is `#e5e2e1` (white)** — Stitch design says the progress bar fill should use the accent color (`#7c4dff`).

8. **Stale references** — README and a doc comment in `search_view.rs` still reference old `style.css`.

---

## Design Goals

| Goal                              | Approach                                                              |
| --------------------------------- | --------------------------------------------------------------------- |
| Stitch brand identity             | Use the Stitch color palette as the canonical source                 |
| Solid surfaces (no glassmorphism) | Use Stitch surface tones for a 5-layer depth hierarchy               |
| Consistent with GNOME ecosystem   | Leverage existing libadwaita CSS classes, minimize overrides          |
| Page-by-page implementation       | Start with global tokens, then tackle one page at a time              |
| Match Stitch design intent        | Adopt layout proportions, spacing, and component patterns from Stitch |

---

## CSS Architecture

### File Structure

Component-scoped files under `src/ui/styles/`:

```
src/ui/styles/
  tokens.css      — @define-color tokens, global resets, spacing constants
  top_bar.css     — .top-bar, .top-bar-search
  sidebar.css     — .sidebar, .library-chip
  home.css        — .home-card, .home-art, .hero-card
  player.css      — .toolbar, progress bar styling
  skeleton.css    — .skeleton-block, animation keyframes
  detail.css      — artist hero, track table, about card
```

### Loading Strategy

**Compile-time concatenation** — all files merged into one string at build time via `include_str!`. Zero runtime cost:

```rust
// In window.rs load_css()
let css = [
    include_str!("styles/tokens.css"),
    include_str!("styles/top_bar.css"),
    include_str!("styles/sidebar.css"),
    include_str!("styles/home.css"),
    include_str!("styles/player.css"),
    include_str!("styles/skeleton.css"),
    include_str!("styles/detail.css"),
].join("\n");

let provider = gtk::CssProvider::new();
provider.load_from_string(&css);
gtk::style_context_add_provider_for_display(
    &gdk::Display::default().expect("Could not connect to display"),
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
);
```

### Rules

1. **One concern per file** — each file owns its component's styles
2. **tokens.css is imported first** — all other files depend on its `@define-color` definitions
3. **No hardcoded hex in component files** — always reference tokens from `tokens.css`
4. **No cross-file selectors** — if a rule affects multiple components, it belongs in `tokens.css`
5. **Naming convention** — filenames match the Rust module names (`home_view.rs` -> `home.css`)

---

## Color Strategy

### Stitch Palette (Canonical)

All colors are defined in `tokens.css` via `@define-color`. Every CSS file references these tokens, never raw hex.

```css
/* tokens.css — Stitch Design System Tokens */

/* Surface hierarchy */
@define-color surface              #131313;
@define-color surface-dim          #0e0e0e;
@define-color surface-bright       #393939;
@define-color surface-container-lowest  #0e0e0e;
@define-color surface-container-low     #1c1b1b;
@define-color surface-container         #201f1f;
@define-color surface-container-high    #2a2a2a;
@define-color surface-container-highest #353534;
@define-color surface-variant      #353534;

/* Foreground / text */
@define-color on-surface           #e5e2e1;
@define-color on-surface-variant   #cac3d8;

/* Accent / primary */
@define-color primary              #cdbdff;
@define-color primary-container    #7c4dff;
@define-color on-primary           #370096;
@define-color on-primary-container #fcf6ff;

/* Secondary */
@define-color secondary            #a6e6ff;
@define-color secondary-container  #14d1ff;

/* Tertiary */
@define-color tertiary             #ffb688;
@define-color tertiary-container   #b55800;

/* Outline */
@define-color outline              #948ea1;
@define-color outline-variant      #494455;

/* Semantic */
@define-color error                #ffb4ab;
@define-color success              #4ade80;
```

### Depth Hierarchy

Five layers using surface tones, darkest at the bottom:

```
Layer 0 (Base):     @surface-dim           — window background (#0e0e0e)
Layer 1 (Sidebar):  @surface-container-low — left sidebar (#1c1b1b)
Layer 2 (Content):  @surface-container     — content area (#201f1f)
Layer 3 (Cards):    @surface-container-high — elevated card surfaces (#2a2a2a)
Layer 4 (Highest):  @surface-container-highest + border — popovers, dialogs (#353534)
```

### Accent Color Usage

The Stitch primary accent `#7c4dff` (Electric Violet) is used for:

- Primary action buttons (`.suggested-action` class or `.play-artist-btn`)
- Active/selected sidebar states
- Progress bar fill
- Hero card background
- Focus rings
- Card hover glows

For alpha-based accent usage (hover backgrounds, active states), use `alpha(@primary-container, 0.15)` etc.

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

| Token             | Value | Usage                     |
| ----------------- | ----- | ------------------------- |
| Spacing unit      | 8px   | Base grid                 |
| Container padding | 32px  | Main content area margins |
| Gutter            | 24px  | Gap between cards         |
| Sidebar width     | 280px | Left library sidebar      |
| Player height     | 96px  | Bottom player bar         |

### Border Radius

| Element          | Radius       | Notes                            |
| ---------------- | ------------ | -------------------------------- |
| Search pill      | 999px        | Already correct                  |
| Standard buttons | 999px (pill) | Adwaita default for pill style   |
| Cards            | 12px         | Slightly larger than current 8px |
| Art thumbnails   | 12px         | Match cards                      |
| Library chips    | 999px        | Already correct                  |
| Hero card        | 16px         | Larger, more prominent           |

---

## Stitch Screens Reference

The Stitch project "Sleek Linux Music Player" contains these designs:

| Screen            | Stitch ID  | Status           |
| ----------------- | ---------- | ---------------- |
| Library Dashboard | `ea3ed206` | **First target** |
| Now Playing View  | `0dbf4c4c` | Pending          |
| Explore & Search  | `9ed04f81` | Pending          |
| Artist Profile    | `d8c1face` | Pending          |
| Settings          | `6a0ea73b` | Pending          |

---

## Implementation Phases

### Phase 0: Global Design Tokens

**Goal:** Establish the color and spacing foundation. All CSS files reference tokens from `tokens.css`, never raw hex values.

**Status:** Partially done. CSS files exist and are loaded, but component files still use hardcoded hex instead of the defined tokens.

**Files to modify:**

- `src/ui/styles/tokens.css` — complete the token set (add missing surface hierarchy, semantic colors)
- `src/ui/styles/top_bar.css` — replace all hardcoded hex with token references
- `src/ui/styles/sidebar.css` — replace all hardcoded hex with token references
- `src/ui/styles/home.css` — replace all hardcoded hex with token references
- `src/ui/styles/player.css` — replace all hardcoded hex with token references, fix progress bar fill color
- `src/ui/styles/skeleton.css` — replace hardcoded hex with token references
- `src/ui/styles/detail.css` — replace all hardcoded hex with token references, fix duplicate `.section-title`

**Changes:**

1. **Complete `tokens.css`** — ensure all surface hierarchy tokens are defined:

```css
/* tokens.css — Stitch Design System Tokens */
/* Loaded first; all other style files depend on these definitions. */

/* Surface hierarchy (Layer 0–4) */
@define-color surface-dim              #0e0e0e;
@define-color surface-container-low   #1c1b1b;
@define-color surface-container       #201f1f;
@define-color surface-container-high  #2a2a2a;
@define-color surface-container-highest #353534;

/* Foreground */
@define-color on-surface              #e5e2e1;
@define-color on-surface-variant      #cac3d8;

/* Primary accent */
@define-color primary-container       #7c4dff;
@define-color on-primary-container    #fcf6ff;

/* Secondary */
@define-color secondary-container     #14d1ff;

/* Outline */
@define-color outline                 #948ea1;
@define-color outline-variant         #494455;

/* Semantic */
@define-color error                   #ffb4ab;
@define-color success                 #4ade80;

/* Window background */
window {
    background-color: @surface-dim;
}
```

2. **Replace all hardcoded hex in component files** with token references. Key mappings:

| Current Hardcoded                       | Replace With                                         |
| --------------------------------------- | ---------------------------------------------------- |
| `#131313` (window, sidebar, top bar)    | `@surface-dim`                                       |
| `#1c1b1b` (if used)                    | `@surface-container-low`                             |
| `#201f1f` (if used)                    | `@surface-container`                                 |
| `#2a2a2a` (card/skeleton bg)            | `@surface-container-high`                            |
| `#353534` (if used)                    | `@surface-container-highest`                         |
| `#7c4dff` (accent everywhere)           | `@primary-container`                                 |
| `#cdbdff` (light accent text)           | `@primary`                                           |
| `#e5e2e1` (primary text)                | `@on-surface`                                        |
| `#cac3d8` (secondary text)              | `@on-surface-variant`                                |
| `#948ea1` (muted text)                  | `@outline`                                           |
| `#494455` (if used)                    | `@outline-variant`                                   |
| `#4ade80` (verified badge)              | `@success`                                           |
| `#ffb4ab` (error, if used)             | `@error`                                             |
| `rgba(255,255,255,0.05)` (search bg)    | `alpha(@on-surface, 0.05)`                           |
| `rgba(255,255,255,0.06)` (hover)        | `alpha(@on-surface, 0.06)`                           |
| `rgba(255,255,255,0.08)` (borders)      | `alpha(@on-surface, 0.08)`                           |
| `rgba(255,255,255,0.1)` (border hover)  | `alpha(@on-surface, 0.1)`                            |
| `rgba(124,77,255,0.15)` (active bg)     | `alpha(@primary-container, 0.15)`                    |
| `rgba(124,77,255,0.08)` (card hover)    | `alpha(@primary-container, 0.08)`                    |
| `rgba(124,77,255,0.12)` (search focus)  | `alpha(@primary-container, 0.12)`                    |
| `#6a3de8` (button hover)                | `shade(@primary-container, 0.85)`                    |
| `#1a1a2e` (hero gradient start)         | `alpha(@primary-container, 0.12)`                    |
| `#1a1a2e` (about card bg)               | `alpha(@primary-container, 0.08)`                    |

3. **Fix progress bar fill** — change from `#e5e2e1` (white) to `@primary-container` (accent) per Stitch design.

4. **Add focus ring styling in `tokens.css`:**

```css
/* Focus rings */
button:focus-visible,
entry:focus-visible {
    outline-color: @primary-container;
    outline-offset: 2px;
}
```

5. **Fix duplicate `.section-title`** — remove from `detail.css` (keep only in `home.css` or move to `tokens.css`).

**Verification:**

```bash
cargo build
# Manual: run app, verify all surfaces use the depth hierarchy
# Check that no raw hex remains in component CSS files (grep for #[0-9a-fA-F])
# Verify focus rings appear on keyboard navigation
```

---

### Phase 1: Library Dashboard (Home Page)

**Goal:** Align the home/main view with the Stitch "Library Dashboard" design.

**Screens in Stitch:** `ea3ed206d23244e2adfa386d9bb31777` (Library Dashboard)

**Current files:**

- `src/ui/home_view.rs` — Main content area with cards
- `src/ui/library_sidebar.rs` — Left sidebar with library list
- `src/ui/player_bar.rs` — Bottom player bar
- `src/ui/top_bar.rs` — Top search bar

**Changes per component:**

#### Home View (`home_view.rs` + `home.css`)

- Hero card: solid `@primary-container` background, 16px radius (already done)
- Album cards: 12px border radius (already done)
- Hover lift effect: `translateY(-4px)` with smooth transition (already done)
- Art glow on hover: `alpha(@primary-container, 0.3)` box-shadow (already done, verify token usage)
- Ensure consistent 8px spacing grid throughout

#### Library Sidebar (`library_sidebar.rs` + `sidebar.css`)

- Background: `@surface-container-low` (depth layer 1)
- Active library items: `alpha(@primary-container, 0.15)` background with 4px left bar in `@primary-container`
- Hover state: `alpha(@on-surface, 0.06)`
- Ensure sidebar width is 280px

#### Player Bar (`player_bar.rs` + `player.css`)

- Background: `@surface-container` (depth layer 2) with top border `alpha(@on-surface, 0.08)`
- Progress bar fill: `@primary-container` (accent, not white)
- Progress bar track: `alpha(@on-surface, 0.15)`
- Slider handle: `@on-surface` with `@primary-container` on hover
- Ensure player height matches Stitch spec (96px)

#### Top Bar (`top_bar.rs` + `top_bar.css`)

- Background: `@surface-dim` (matches window)
- Search pill background: `alpha(@on-surface, 0.05)`
- Search focus: `alpha(@primary-container, 0.12)` background
- Pill shape: 999px radius (already done)

**Verification:**

```bash
cargo build
# Manual: test home page — hero card, carousel, section headers
# Verify hover effects work smoothly
# Check sidebar active states and transitions
# Check player bar progress bar is accent-colored
```

---

### Phase 2: Now Playing View

**Goal:** Wire the Now Playing and Queue panels into the window layout.

**Stitch screen:** `0dbf4c4ce13f4bdfa3357298922c9a09` (Now Playing View)

**Current state:**

- `src/ui/now_playing_panel.rs` — **implemented but not mounted** in `window.rs`
- `src/ui/queue_panel.rs` — **implemented but not mounted**
- Comment in `window.rs:524`: `PlayerEvent::Queue(_snapshot) => { // Queue panel removed per Stitch design }`

**Changes:**

1. **Wire `NowPlayingPanel` into `window.rs` layout** — add a vertical separator and the panel to the right of `content_stack` in `middle_row`
2. **Wire `QueuePanel` into `window.rs` layout** — add below the now playing panel or as a stacked view
3. **Connect player events** — `PlayerEvent::State` should also call `now_playing_panel.update()`, and `PlayerEvent::Queue` should call `queue_panel.update()`
4. **Create `src/ui/styles/now_playing.css`** — panel background, heading, art frame, about section
5. **Create `src/ui/styles/queue.css`** — queue list, current track highlight, empty state
6. **Add both CSS files to `window.rs` `load_css()` array**

**Styling:**

- Panel backgrounds: `@surface-container-low` (matches sidebar depth)
- Current track highlight in queue: `alpha(@primary-container, 0.15)`
- "About the artist" heading: `@on-surface` text, `@outline` separator

**Verification:**

```bash
cargo build
# Manual: play a track, verify now playing panel shows art/title/artist/bio
# Verify queue panel shows tracks and current track is highlighted
# Verify clicking a queue track plays it
# Verify the panels use the correct surface depth
```

---

### Phase 3: Explore & Search

**Goal:** Style the search results popover and ensure consistent visual treatment.

**Stitch screen:** `9ed04f816072404ab81de7e8ded6dc30` (Explore & Search)

**Current files:**

- `src/ui/search_view.rs` — Search results popover with skeleton loading
- No `search.css` file exists

**Changes:**

1. **Create `src/ui/styles/search.css`** — add it to `load_css()` in `window.rs`
2. **Style `.search-popover`:**
   - Background: `@surface-container-high` (depth layer 3)
   - Border radius: 12px
   - Border: 1px solid `alpha(@on-surface, 0.08)`
   - Shadow: `0 8px 32px rgba(0,0,0,0.4)`
3. **Skeleton loading blocks** — verify `.skeleton-block` uses `@surface-container-high`
4. **Empty state** — centered message with `dim-label` class (already done)
5. **Search result rows** — verify `boxed-list` styling is consistent with Stitch

**Verification:**

```bash
cargo build
# Manual: search for artists/albums/songs, verify result styling
# Check skeleton loading animation
# Test empty state
# Verify popover has proper depth and border
```

---

### Phase 4: Liked Songs & Artist Profile

**Goal:** Style the Liked Songs page and polish the artist profile/detail view.

**Stitch screens:**

- `d8c1facedfae4f389e9bd51d6cc1e611` (Artist Profile)

**Current files:**

- `src/ui/liked_songs_view.rs` — Liked Songs page (no custom CSS)
- `src/ui/detail_view.rs` — Artist/album detail with track list
- `src/ui/styles/detail.css` — Already has styling

**Changes:**

#### Liked Songs

1. **Create `src/ui/styles/liked_songs.css`** — add it to `load_css()` in `window.rs`
2. Style the header: back button + "Liked Songs" title using `@on-surface` text
3. Style the track list rows using `boxed-list` with `@surface-container-high` row backgrounds
4. Style the "Show More" button as a pill with `@primary-container` accent

#### Artist Profile

- Hero section: gradient using `alpha(@primary-container, 0.12)` → `@surface-dim` (verify current gradient)
- Track table: verify consistent row heights and `@surface-container-high` hover states
- Play button: `@primary-container` background with `@on-primary-container` text
- Follow button: transparent with `@outline` border
- About card: `@alpha(@primary-container, 0.08)` background with 12px radius
- Remove duplicate `.section-title` (keep only in `home.css`)

**Verification:**

```bash
cargo build
# Manual: navigate to artist page, verify hero layout and gradient
# Check track list styling and hover states
# Verify play/follow buttons
# Navigate to Liked Songs, verify layout and "Show More" button
```

---

### Phase 5: Settings & Polish

**Goal:** Implement settings screen and final polish pass across all CSS.

**Stitch screen:** `6a0ea73bc7074834adabdac9411e5dec` (Settings)

**Current files:**

- `src/ui/login_dialog.rs` — Login dialog (closest to settings)

**Changes:**

1. **Create a settings view** — either a new `settings_view.rs` or extend the login dialog
2. Settings page with `adw::PreferencesGroup` styling
3. Account section with profile info
4. Logout button: `destructive-action` class
5. About section with version info
6. **Final polish pass across all CSS files:**
   - Verify all hover transitions are smooth (200ms ease)
   - Verify all focus rings use `@primary-container`
   - Verify consistent spacing (8px grid)
   - Verify all border radii match the spec
   - Verify no raw hex remains in component CSS files
   - Verify the depth hierarchy is correct across all surfaces
   - Test with light theme (if supported)

**Verification:**

```bash
cargo build
# Manual: open settings, verify all sections
# Test logout flow
# Check keyboard navigation and focus rings
# Verify all transitions are smooth
# Run: grep -rn '#[0-9a-fA-F]' src/ui/styles/ --include='*.css' | grep -v tokens.css
#   (should return empty — no raw hex in component files)
```

---

## Summary

| Phase | Scope                | Status                                                   | Prerequisites |
| ----- | -------------------- | -------------------------------------------------------- | ------------- |
| 0     | Global design tokens | **In Progress** — CSS split done, token references needed | None          |
| 1     | Library Dashboard    | **Partial** — CSS exists, needs token references         | Phase 0       |
| 2     | Now Playing View     | **Not started** — panels exist but not wired              | Phase 0       |
| 3     | Explore & Search     | **Not started** — no search CSS file                      | Phase 0       |
| 4     | Liked Songs + Artist | **Partial** — detail.css exists, liked_songs has no CSS   | Phase 0       |
| 5     | Settings & Polish    | **Not started**                                           | Phases 0-4    |

**Estimated time:** ~10-12 hours (reduced from original 12-14 since Phase 0 file split is done)

Phases 1-4 can be done in parallel after Phase 0, but sequential implementation is recommended for consistency.

---

## Stitch Design System Reference

The full Stitch design system is defined in project `11712536191264151278` with these key specs:

- **Accent Color:** `#7c4dff` (Electric Violet) — primary accent for actions and highlights
- **Secondary:** `#14d1ff` (Digital Cyan) — optional, for secondary highlights
- **Surface:** `#131313` base with a 5-layer hierarchy from `#0e0e0e` to `#353534`
- **Font:** Inter — replaced by Adwaita system fonts
- **Label Font:** Geist — not used (Adwaita default)
- **Roundness:** `ROUND_EIGHT` (8px default, 12px for cards, 16px for hero)
- **Spacing Unit:** 8px

The Stitch design MD file contains the full brand narrative and component specifications. It serves as the visual reference for all implementation phases.
