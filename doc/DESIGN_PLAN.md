# Melofin Design Improvement Plan

## Overview

Improve Melofin's UI to align with the Stitch "Sonic Integration" design system while keeping solid color surfaces (no glassmorphism). Changes are organized into 6 phases, each self-contained and independently shippable.

## Design Principles

- **Solid surfaces only** — No `backdrop-filter` / glass effects
- **8px grid spacing** — All spacing on 8px baseline
- **32px container padding** — Main content area side padding
- **24px gutters** — Gap between content sections
- **Material 3 surface hierarchy** — 4 elevation layers using `@surface-*` tokens
- **Primary accent** — `#7c4dff` (violet) for active states, buttons, highlights
- **Secondary accent** — `#14d1ff` (cyan) used sparingly
- **Typography** — Inter for body/headlines, Geist for labels/metadata

---

## Phase 1: Design Token & Global Visual Refresh

**Goal:** Align the token system and global surfaces with the Stitch design spec.

**Files:** `tokens.css`, `sidebar.css`, `top_bar.css`, `player.css`

### Changes

1. **`tokens.css`** — Add missing tokens:
   - `@tertiary` / `@tertiary-container` / `@on-tertiary-container`
   - `@surface-tint` (primary at very low opacity for subtle tints)
   - `@on-primary` (text on primary-colored backgrounds)
   - `@inverse-surface` / `@inverse-on-surface` for toast/overlay contexts

2. **`sidebar.css`** — Refine active nav state:
   - Active row: `alpha(@primary, 0.15)` background + 4px solid `@primary` left border (verify existing matches)
   - Ensure nav row padding is consistent (8px 12px)
   - Add subtle `border-radius: 8px` to all nav rows

3. **`top_bar.css`** — Polish:
   - Add 1px bottom border: `alpha(@on-surface, 0.08)`
   - Ensure search pill height is 36px, border-radius 999px
   - Consistent 32px horizontal padding

4. **`player.css`** — Enhance:
   - Progress bar: 3px default → 8px on hover (currently 5px)
   - Progress bar slider: show only on hover (hide by default via margin trick)
   - Volume slider: match progress bar styling

---

## Phase 2: Home View Enhancements

**Goal:** Add missing sections and polish existing card interactions.

**Files:** `home.css`, `home_view.rs`

### Changes

1. **New "Recently Played" section** (horizontal scroll):
   - Album art cards (square, 12px radius)
   - Title + artist below each card
   - Play-arrow overlay icon on hover (centered on art)
   - Row with section title "Recently Played" + "VIEW ALL" link

2. **New "New Releases" section** (numbered track list):
   - Each row: number (01, 02, 03...) + thumbnail + title + artist + album + heart button + duration + more button
   - Header: "New Releases" + "DISCOVER NEW" link
   - Hover state: slight background highlight

3. **Enhance existing album card hover**:
   - `translateY(-4px)` lift (keep current)
   - Add glow: `box-shadow: 0 4px 20px alpha(@primary, 0.3)` on art
   - Background: `alpha(@primary, 0.08)` on hover

4. **Refine "For You" category cards**:
   - Ensure label (uppercase, 11px, primary color), title (16px/600), description (14px/variant)
   - Play-circle icon overlay on hover

5. **Spacing pass**:
   - Section gaps: 32px vertical
   - Horizontal scroll containers: 32px side padding
   - Card gaps in horizontal rows: 16px

---

## Phase 3: Artist Detail View Enhancements

**Goal:** Add Discography and Related Artists sections, polish existing layout.

**Files:** `detail.css`, `detail_view.rs`

### Changes

1. **Add "Discography" section** below existing tracks/about:
   - Horizontal scroll of album cards
   - Each card: album art (square, 8px radius) + title + year + type ("Album" / "Single" / "EP")
   - Play-arrow overlay on hover
   - Section header: "Discography" with filter tabs ("Albums", "Singles & EPs")

2. **Add "Related Artists" section**:
   - Horizontal scroll of artist cards
   - Each card: circular avatar (120px) + artist name below
   - Hover: subtle lift + primary border glow
   - Section header: "Related Artists"

3. **Polish hero section**:
   - Gradient: `linear-gradient(180deg, alpha(@primary, 0.12) 0%, @surface-dim 100%)`
   - Ensure verified badge (11px, uppercase, success color), artist name (48px/800), listeners (14px/outline), bio (14px/variant)
   - Action buttons: Play Artist (primary pill), Follow (outline pill), More (circle outline)

4. **Track table refinement**:
   - Header row: `#`, Title, Plays, Duration — 11px uppercase labels
   - Number column: 32px wide
   - Play overlay on row hover (replaces number)

---

## Phase 4: Now Playing View Polish

**Goal:** Enhance the now-playing experience with better visual hierarchy.

**Files:** `now_playing_view.css`, `now_playing_view.rs`

### Changes

1. **Album art**:
   - Ensure `border-radius: 16px`
   - Shadow: `box-shadow: 0 20px 80px -20px alpha(@primary, 0.5)` (verify)
   - Max size: 450px

2. **Track info below art**:
   - Title: 32px/800, `-0.02em` letter-spacing
   - Artist: 18px, primary color

3. **Transport controls**:
   - Centered pill container with subtle background (`alpha(@on-surface, 0.05)`)
   - Play button: 48px circular with primary bg
   - Shuffle/repeat: toggle state with primary color when active

4. **Progress bar in now-playing**:
   - Match player bar styling (3px → 8px on hover)
   - Elapsed/remaining time labels

5. **Lyrics section** (right side):
   - Keep "No lyrics available" placeholder
   - Style consistently: `@surface-container-high` bg, 12px radius, centered text

---

## Phase 5: Explore Page (New)

**Goal:** Create the Explore page wired to the existing sidebar navigation.

**Files:** New `explore_view.rs`, new `explore.css`, `window.rs` (wire navigation), `library_sidebar.rs` (connect click)

### Page Layout

1. **Header**: "Explore" title (24px/700)
2. **Genre Grid**: 2-column grid of genre cards
   - Each card: gradient background (genre-specific), genre name (20px/700), icon
   - Genres: Pop, Rock, Electronic, Hip-Hop, Jazz, Classical, R&B, Latin, Indie, Metal, Folk, Podcasts
3. **Featured Playlists** section: Horizontal scroll of playlist cards
   - Each card: playlist art + title + description
4. **Browse by Mood** section: Horizontal scroll of mood tags
   - Tags: "Focus", "Energy", "Chill", "Party", "Sleep", "Workout"

### Wiring
- Sidebar "Explore" row → `content_stack.set_visible_child_name("explore")`
- Add `"explore"` child to `content_stack` in `window.rs`

---

## Phase 6: Settings Polish

**Goal:** Minor visual refinements to match Stitch.

**Files:** `settings.css`, `settings_view.rs`

### Changes

1. Ensure all bento cards use `border-radius: 16px` and consistent border (`alpha(@on-surface, 0.08)`)
2. Segmented control: verify active state has primary bg + shadow
3. Color swatches: ensure 32px circles with ring effect on active
4. Density slider: ensure hint card styling is consistent
5. Page title: 48px/800 matching Stitch's display-lg

---

## Implementation Order

| Phase | Estimated Effort | Risk |
|-------|-----------------|------|
| Phase 1: Tokens & Global | Low | Low — CSS-only, safe to test |
| Phase 2: Home View | Medium | Low — additive Rust + CSS |
| Phase 3: Artist Detail | Medium | Low — additive Rust + CSS |
| Phase 4: Now Playing | Low | Low — CSS refinements |
| Phase 5: Explore Page | High | Medium — new Rust module + wiring |
| Phase 6: Settings | Low | Low — CSS-only tweaks |

## Verification

After each phase:
1. `cargo build` — ensure no compile errors
2. Run the app and visually verify all affected screens
3. Check that existing functionality (navigation, playback, search) still works
