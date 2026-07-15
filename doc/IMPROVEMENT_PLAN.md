# Melofin Improvement Plan

Phased implementation guide for improving code quality, deduplication, tooling, and packaging.

---

## Phase 1: Quick Wins (~2 hours)

Low-risk fixes that clean up inconsistencies and improve immediate hygiene.

### 1.1 Fix `.gitignore` — Remove `Cargo.lock` Rule

Melofin is a binary application. Cargo recommends tracking `Cargo.lock` for binaries. Remove the line from `.gitignore`.

**File:** `.gitignore`

- Remove the `Cargo.lock` line

### 1.2 Deduplicate `USER_AGENT` Constant

`innertube.rs` defines `USER_AGENT` as a `pub(crate) const`, but `user.rs` hardcodes the same string. Use the shared constant everywhere.

**Files:**

- `src/user.rs:58` — Replace inline string with `crate::innertube::USER_AGENT`
- Verify `src/innertube.rs:15` is `pub(crate)` (already is)

### 1.3 Deduplicate `CLIENT_VERSION` Constant

`"1.20250710.01.00"` appears in 3+ files. Centralize in `innertube.rs` and import everywhere.

**Files:**

- `src/innertube.rs` — Ensure `CLIENT_VERSION` is `pub(crate)`
- `src/user.rs` — Replace inline string with `crate::innertube::CLIENT_VERSION`
- `src/home_feed.rs` — Replace inline strings in tests with the constant (or a local const if cfg(test) makes it awkward)

### 1.4 Demote Cookie Name Logging Level

`user.rs:56` logs cookie names at `info!` level. This leaks metadata in production logs.

**Files:**

- `src/user.rs:56` — Change `tracing::info!` to `tracing::debug!`

### 1.5 Update README to Match Current Source Structure

The README lists `bin/search_test.rs`, `bin/ui_shell.rs`, and a `src/bin/` directory that don't exist.

**Files:**

- `README.md` — Update the "Project Structure" section to reflect the actual `src/` layout with `ui/` subdirectory
- Remove references to `search-test` and `ui-shell` binaries, or add a note that they were removed

### 1.6 Mark `probe_browse` Test as `#[ignore]`

The 293-line test in `home_feed.rs` makes real HTTP requests and writes to `doc/`. It should not run in CI or `cargo test`.

**Files:**

- `src/home_feed.rs` — Add `#[ignore]` attribute to the `probe_browse` test function
- Add a comment explaining it requires network + valid cookies

### Verification

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

---

## Phase 2: Code Deduplication (~4 hours)

Extract shared patterns into reusable helpers and components.

### 2.1 Extract InnerTube Request Builder

The header setup boilerplate (~50 lines) is duplicated across `innertube.rs:browse_request`, `user.rs:fetch_profile_from_account_menu`, and `home_feed.rs` tests.

**Approach:**
Create a helper function in `innertube.rs`:

```rust
pub(crate) fn build_innertube_request(
    url: &str,
    cookies_path: &Path,
    method: ureq::HttpMethod,
) -> anyhow::Result<ureq::Request> {
    let contents = std::fs::read_to_string(cookies_path)?;
    let cookie_header = crate::user::build_cookie_header(&contents);
    anyhow::ensure!(!cookie_header.is_empty(), "no signed cookies found");

    let request = ureq::request(method, url)
        .set("Cookie", &cookie_header)
        .set("User-Agent", USER_AGENT)
        .set("Content-Type", "application/json")
        .set("X-Origin", "https://music.youtube.com")
        .set("Referer", "https://music.youtube.com")
        .set("X-Goog-Api-Format-Version", "1")
        .set("X-YouTube-Client-Name", "67")
        .set("X-YouTube-Client-Version", CLIENT_VERSION);

    Ok(request)
}
```

**Files to update:**

- `src/innertube.rs` — Add `build_innertube_request()`, refactor `browse_request` to use it
- `src/user.rs` — Refactor `fetch_profile_from_account_menu` to use it
- `src/home_feed.rs` — Refactor `probe_browse` test to use it (if practical)

### 2.2 Extract Cookie Read + Validate Pattern

`innertube.rs:30-36` and `liked_songs.rs:18-24` repeat: read file → build header → ensure non-empty.

**Approach:**
Add a helper in `user.rs` (or a new `cookie_utils.rs` if preferred):

```rust
pub fn read_and_validate_cookies(cookies_path: &Path) -> anyhow::Result<String> {
    let contents = std::fs::read_to_string(cookies_path)?;
    let cookie_header = build_cookie_header(&contents);
    anyhow::ensure!(!cookie_header.is_empty(), "no signed cookies found in {}", cookies_path.display());
    Ok(cookie_header)
}
```

**Files to update:**

- `src/user.rs` — Add `read_and_validate_cookies()`
- `src/innertube.rs` — Replace inline pattern at lines 30-36
- `src/liked_songs.rs` — Replace inline pattern at lines 18-24

### 2.3 Extract Thumbnail Art-Stack Widget

`PlayerBar` and `NowPlayingPanel` both implement: create Stack with "placeholder" + "art" children → track `current_thumbnail_url: RefCell<String>` → on update, check URL changed → fetch → swap stack page.

**Approach:**
Create a `ThumbnailStack` GObject or wrapper in `thumbnail_widget.rs`:

```rust
pub struct ThumbnailStack {
    stack: gtk::Stack,
    current_url: RefCell<String>,
}

impl ThumbnailStack {
    pub fn new(placeholder: gtk::Widget, size: i32) -> Self { ... }
    pub fn update(&self, url: &str) { ... }  // handles fetch + swap internally
}
```

**Files to update:**

- `src/ui/thumbnail_widget.rs` — Add `ThumbnailStack` type
- `src/ui/player_bar.rs` — Replace inline art-stack pattern with `ThumbnailStack`
- `src/ui/now_playing_panel.rs` — Replace inline art-stack pattern with `ThumbnailStack`

### 2.4 Extract ActionRow Builder

`search_view.rs` and `liked_songs_view.rs` build nearly identical `adw::ActionRow` widgets with thumbnail, title, subtitle, and activation handler.

**Approach:**
Add a helper in a shared location (e.g., `ui/mod.rs` or a new `ui/row_builder.rs`):

```rust
pub fn build_track_row(track: &Track, on_activate: impl Fn() + 'static) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&track.title)
        .subtitle(&track.artist)
        .activatable(true)
        .build();
    // attach thumbnail, connect activation signal, return row
}
```

**Files to update:**

- `src/ui/mod.rs` or new `src/ui/row_builder.rs` — Add `build_track_row()`
- `src/ui/search_view.rs` — Use `build_track_row()` in result rendering
- `src/ui/liked_songs_view.rs` — Use `build_track_row()` in `append_page`

### Verification

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
# Manual: run the app, verify search, liked songs, player bar, and now playing panel all work
```

---

## Phase 3: Dev Tooling (~3 hours)

Automated quality gates to prevent regressions.

### 3.1 Add GitHub Actions CI Workflow

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Format & Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings

  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

**Note:** CI for GTK4 apps on Ubuntu requires installing `libgtk-4-dev`, `libadwaita-1-dev`. Add these to the `test` job if `cargo test` needs them to compile. Alternatively, test-only jobs that don't need the UI can use `cargo test --lib` to skip integration tests.

### 3.2 Add `[lints]` Section to `Cargo.toml`

Enforce clippy rules project-wide so individual developers don't have to remember flags.

**File:** `Cargo.toml`

```toml
[lints.clippy]
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
cast_possible_truncation = "warn"
```

### 3.3 Add `cargo-deny` Configuration

Automated vulnerability and license scanning for a project that handles auth cookies.

**Files:**

- Create `deny.toml` at project root (use `cargo deny init` to scaffold, then customize)
- Run `cargo deny check` in CI

### 3.4 Add Pre-commit Hook (Optional)

Use a simple shell script or `git-hooks` crate:

**File:** `.git/hooks/pre-commit` (or use a crate manager)

```bash
#!/bin/sh
cargo fmt --check || exit 1
cargo clippy -- -D warnings || exit 1
```

### Verification

- Push to a branch, confirm CI runs and passes
- `cargo deny check` runs clean
- `cargo test` passes with the new lint config (may need some `#[allow]` attributes on intentional patterns)

---

## Phase 4: Packaging & Distribution (~half day)

Make Melofin installable by end users.

### 4.1 Create Flatpak Manifest

Flatpak is the standard distribution method for Linux GTK4 apps.

**File:** `dev.melofin.Melofin.yml` (Flatpak manifest)

Key elements:

- Base runtime: `org.gnome.Platform` (includes GTK4 + libadwaita)
- Build with `cargo build --release`
- Bundle `mpv` and `yt-dlp` as runtime dependencies, or declare them as `finish-args` permissions
- Metadata: app ID, name, description, license, icon, categories

### 4.2 Create AppStream Metadata

**File:** `dev.melofin.Melofin.metainfo.xml`

Required for Flattub/GNOME Software listing:

- App description, screenshots, release history
- Component metadata (categories, keywords, license)

### 4.3 Add Git Version Tags

Establish a release process:

```bash
git tag -a v0.1.0 -m "Initial release"
git push origin v0.1.0
```

### 4.4 Add GitHub Releases Workflow

Extend `.github/workflows/` with a release workflow that triggers on tag push and builds + publishes Flatpak artifacts.

### Verification

- `flatpak-builder` builds successfully
- App launches from Flatpak sandbox
- Release workflow produces artifacts on tag push

---

## Phase 5: Future Enhancements (Ongoing)

These are larger features that build on the foundation above.

### 5.1 Typed InnerTube Response Structs

Replace `serde_json::Value` navigation with typed structs for the most stable API responses:

- `HomeFeedResponse` — for `FEmusic_home` browse results
- `LikedSongsResponse` — for `VLPLY_LIKE` browse results
- `UserProfileResponse` — for `account/account_menu` results

Use `#[serde(default)]` and `Option<T>` fields liberally to handle API volatility.

### 5.2 Search Query Length Limiting

Add a reasonable max length (e.g., 500 chars) to the search entry as a defense-in-depth measure.

### 5.3 Error Logging for MPRIS Handlers

In `mpris.rs`, replace `let _ = mpv.set_pause(false).await` with:

```rust
if let Err(e) = mpv.set_pause(false).await {
    tracing::error!("MPRIS set_pause failed: {e}");
}
```

Apply to all MPRIS transport control handlers.

### 5.4 SQLite Persistence

Replace flat-file caching with SQLite for:

- Home feed cache (queryable, structured)
- Liked songs offline cache
- User preferences

### 5.5 Queue Management

Wire up the existing placeholder buttons (shuffle, prev, next, repeat) and build a queue view panel.

---

## Summary

| Phase | Scope              | Estimated Time | Prerequisites |
| ----- | ------------------ | -------------- | ------------- |
| 1     | Quick wins         | ~2 hours       | None          |
| 2     | Code deduplication | ~4 hours       | Phase 1       |
| 3     | Dev tooling        | ~3 hours       | Phase 1       |
| 4     | Packaging          | ~half day      | Phases 1-3    |
| 5     | Future features    | Ongoing        | Phases 1-3    |

Phases 1 and 3 can be done in parallel. Phase 2 depends on Phase 1 (constants must be centralized first). Phase 4 benefits from having CI in place (Phase 3). Phase 5 items are independent of each other and can be tackled in any order.
