# Melofin Login System — Implementation Plan (v2)

**Approach:** Cookie-based auth via `yt-dlp --cookies`, no embedded WebView, no
`rustypipe`.

## Why this replaces the v1 plan

The original plan assumed melofin's backend was `rustypipe` (per the README's
original roadmap) and built login around its `user_auth_set_cookie_txt` API.
Checking the actual repo showed the project pivoted: search goes through a
`yt-dlp` subprocess (`search.rs`), and playback is a headless `mpv` process
(`mpv.rs`) that resolves YouTube URLs itself via its built-in `ytdl_hook` —
`rustypipe` isn't a dependency at all yet. `env.example` flags this pivot
explicitly.

This is simpler, not harder: `yt-dlp` has native cookie support
(`--cookies FILE`), and since both search and playback already shell out to
`yt-dlp`, **one imported cookies file covers both paths**. No auth client
library, no embedded browser needed.

---

## Phase 0 — Prep ✅ done

- [x] Confirmed there's no `rustypipe` dependency; auth goes through
      `yt-dlp --cookies` instead, shared by `search.rs` and `mpv`'s
      `ytdl_hook`.
- [x] Cache/data location: melofin's XDG data dir
      (`glib::user_data_dir().join("melofin")`, accessible as `gtk::glib`
      since `gtk4` re-exports it — no new dependency).
- [x] Permissions: data dir `0700`; the cookies file itself gets `0600` at
      write time (handled inside `AuthManager::import_cookies_file`).

## Phase 1 — Auth module ✅ done

- [x] `src/auth.rs` — flat module, following the project's existing
      convention (`search.rs`, `player.rs`, `mpv.rs` are all flat; only
      `ui/` is nested, because it holds many UI-specific files).
- [x] `AuthState { LoggedOut, LoggedIn }`
- [x] `AuthManager::new(data_dir)` — no GTK dependency, so the login dialog
      can stay a thin wrapper around it.
- [x] `current_state()` — cheap file-existence check, for startup UI state.
- [x] `import_cookies_file(source)` — copies the user's picked file into
      melofin's data dir as `cookies.txt`, `chmod 600`s it, validates it,
      and rolls back (deletes the copy) if validation fails.
- [x] `validate()` — probes YouTube Music's "Liked Music" auto-playlist
      (`music.youtube.com/playlist?list=LM`) via `yt-dlp`; only resolves
      with a real authenticated session, so this confirms the cookies
      actually work rather than just being present.
- [x] `logout()` — removes the cookies file, idempotent.
- [x] `pub mod auth;` wired into `lib.rs`.
- [x] Unit tests for state transitions and idempotent logout (no `yt-dlp`
      calls in the unit tests — `validate()` needs the real binary, so it's
      exercised manually/via integration test, not mocked).

**Outstanding before moving on:** run `cargo check` / `cargo test` yourself —
this was written and reviewed but not compiled in the sandbox (old toolchain,
no GTK libs available there).

---

## Phase 2 — Login dialog (UI) — next up

Goal: a working end-to-end login screen wired to `AuthManager`.

- [ ] New file `src/ui/login_dialog.rs`, registered in `src/ui/mod.rs`
      (`pub mod login_dialog;`) — matches how the other UI pieces
      (`search_view.rs`, `player_bar.rs`, etc.) are organized.
- [ ] Build a minimal Adwaita dialog:
  - Instructions: _"Log into music.youtube.com in your browser (an Incognito
    window is safest), export cookies with a 'Get cookies.txt' extension,
    close that browser tab, then select the file below."_ — the "close the
    tab" part isn't optional flavor text: YouTube rotates session cookies
    every few minutes, and a browser tab left open can invalidate the ones
    just exported.
  - A `gtk::FileDialog` restricted to text files.
  - A "Log in" button, disabled until a file is picked.
  - A spinner/disabled state while `import_cookies_file` runs (it does a
    real network call via `validate()` — don't block the main thread; call
    it the same way `player.rs` calls into its tokio runtime, e.g. via
    `async-channel`/`glib::spawn_future_local`).
- [ ] On success: close dialog, update shared `AuthState`, toast "Logged in".
- [ ] On failure: surface `AuthManager`'s `Result` error text directly in the
      dialog (it's already written to be human-readable — "cookies may be
      expired", "yt-dlp not found", etc.) — don't swallow it into a generic
      "login failed".
- [ ] Add a "Log out" action (top bar overflow menu or a new Account
      section) that calls `AuthManager::logout()`.
- [ ] Wire `AuthState` into whatever shared app-state mechanism the rest of
      the UI already uses (however `player.rs`'s state currently reaches the
      UI — reuse that pattern rather than inventing a new one).

**Done when:** you can import a real `cookies.txt`, see confirmation in the
UI, close/reopen melofin and still be logged in, and log out cleanly.

---

## Phase 3 — Wire cookies into search & playback

Goal: actually use the login for something.

- [ ] `search.rs`: when `AuthManager::current_state()` is `LoggedIn`, add
      `--cookies <path>` to the existing `yt-dlp` `Command` in `search()`.
      Keep search working unauthenticated when logged out — this should be
      an additive flag, not a required one.
- [ ] `mpv.rs`: `MpvController::spawn()` needs an optional cookies path
      parameter, passed to mpv as
      `--script-opts=ytdl_hook-ytdl_raw_options=cookies=<path>` (mpv's
      `ytdl_hook` passes this through to its internal `yt-dlp` call). Thread
      this from `player.rs`'s `spawn_player_thread()` — likely needs the
      `AuthManager`'s cookies path available at player-thread-start time,
      not just at UI level.
- [ ] Decide: does changing login state require restarting the mpv
      subprocess (simplest — `mpv.rs` already spawns once and stays
      `--idle` for the app's lifetime), or does it need to support swapping
      cookies on a live process? Start with "restart on login/logout
      change" — simpler, and login/logout isn't a hot path.
- [ ] Validate with something concrete: search for a track only visible when
      logged in (e.g. an unlisted/private upload you own), or check that a
      "Liked Music" browse view (future feature) returns real data.

**Done when:** logged-in state visibly changes what search/playback can
access, not just what the login dialog shows.

---

## Phase 4 — Automatic browser cookie import (`rookie`)

Goal: remove the manual export step for the common case. This part of the
plan is unchanged from v1 — it slots into the same `AuthManager` regardless
of the `rustypipe`→`yt-dlp` swap underneath.

- [ ] Add `rookie` as an optional dependency
      (`cargo add rookie --optional`), so the manual-import path in Phase 2
      can remain the lighter default build if desired.
- [ ] Extend `login_dialog.rs`: detect installed browsers, offer
      "Import from Firefox" / "Import from Chrome" etc. buttons alongside
      the existing manual-file picker.
- [ ] `rookie::firefox(Some(vec!["youtube.com".into()]))` (or the relevant
      browser call) → format into the same Netscape-cookie-file shape
      `import_cookies_file` expects, or add a sibling
      `AuthManager::import_cookies(cookies: Vec<rookie::Cookie>)` method.
- [ ] Handle keyring-decrypt failures (locked GNOME Keyring/KWallet) with a
      clear error pointing back at the manual-import fallback.
- [ ] Test against Firefox and at least one Chromium-based browser — cookie
      storage formats differ meaningfully between them.

**Done when:** most users can log in with one click; manual import still
works as a fallback.

---

## Phase 5 — Polish / hardening (do last)

- [ ] Session-expiry handling at runtime: if a `yt-dlp` search or mpv
      playback call fails in a way that suggests auth expired, call
      `validate()` and prompt re-login instead of silently degrading.
- [ ] Audit logging: make sure `RUST_LOG=debug` never prints cookie file
      contents (it shouldn't, since only the _path_ is passed as a CLI arg —
      but confirm no code ever reads and logs the file's contents).
- [ ] Add a "clear saved login" action distinct from soft logout, for users
      who want to be sure the file is gone (functionally identical to
      `logout()` today, but worth a UI-level distinction if Phase 4 adds
      more state, e.g. remembered browser choice).
- [ ] Document the login flow in the README (the current README's
      "Not yet implemented" list should get updated once this ships) so
      future contributors understand why there's no WebView/OAuth here.

---

## Explicitly out of scope

- **Embedded WebView login** (Metrolist's Android approach) — too heavy a
  dependency for melofin's minimal-footprint goals on desktop, and
  unnecessary now that `yt-dlp --cookies` already does the job.
- **rustypipe's own auth (OAuth device-code or cookie API)** — moot, since
  `rustypipe` isn't part of this codebase's actual architecture.
- **OAuth TV device-code flow in general** — even if you revisit
  `rustypipe` later, OAuth only grants TV-client scope (no
  playlists/subscriptions/library), which doesn't serve the actual goal of
  library sync.
