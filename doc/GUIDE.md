# Melofin — Linux/Hyprland YouTube Music Client

A native desktop YouTube Music client for Arch Linux / Hyprland, modeled on
[Metrolist](https://github.com/metrolistgroup/metrolist) (Android).

## Tech Stack

| Piece              | Choice                                                | Notes                                                                                                                                          |
| ------------------ | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Language           | Rust                                                  |                                                                                                                                                |
| YT Music backend   | [`rustypipe`](https://github.com/TeamPiped/rustypipe) | InnerTube client — search, browse, library, playlists, stream URL extraction. Needs `rustypipe-botguard` binary for PO tokens (stream access). |
| Playback           | `libmpv` (via `libmpv-rs` or FFI)                     | Handles YouTube's adaptive/Opus streams directly, gapless playback, easy seek/volume control.                                                  |
| UI                 | GTK4 + libadwaita                                     | Native look on Wayland/Hyprland, no custom theming needed.                                                                                     |
| Local storage      | SQLite (`rusqlite`)                                   | Library cache, playlists, downloaded track metadata.                                                                                           |
| System integration | MPRIS (`mpris-server`)                                | Media keys, waybar/notification controls.                                                                                                      |
| Async runtime      | tokio                                                 | Required by `rustypipe`; also drives GTK async bridging via `glib::spawn_future_local`.                                                        |

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌────────────┐
│   UI layer   │◄───►│  App/State    │◄───►│  rustypipe  │──► YouTube Music
│ (GTK4/Adwaita)│     │  (player svc) │     │   client    │
└─────────────┘     └──────┬───────┘     └────────────┘
                            │
                    ┌───────┴───────┐
                    │ libmpv (audio) │
                    │ MPRIS (system) │
                    │ SQLite (cache) │
                    └───────────────┘
```

## Feature Set (parity target: Metrolist)

- Search (songs, albums, artists, playlists)
- Home / quick picks feed
- Playback: background/gapless, MPRIS media controls
- Library: playlists, saved songs/albums, queue management
- Offline caching of streamed tracks
- Optional YouTube account login (cookie-based auth via `rustypipe`) for library sync

## Build Order (incremental)

1. **Backend spike** — Cargo workspace scaffold, wire up `rustypipe`, get search
   - stream URL resolution working from a CLI test binary (no UI).
2. **Playback spike** — pipe a resolved stream URL into `libmpv`, verify
   headless playback (play/pause/seek/volume).
3. **MPRIS** — wrap the headless player with MPRIS so media keys work.
4. **UI shell** — GTK4/libadwaita window: search box + results list, no
   playback wired yet.
5. **Wire UI → player service** — connect search results and playback
   controls to the backend/player built in steps 1–3.
6. **SQLite layer** — metadata cache first, then playlists/library.
7. **Auth** — cookie-based YouTube login for library sync (last, optional-use).

## Open Questions / Not Yet Decided

- Offline download storage format/location and cache size limits
- Playlist import (M3U/CSV, matching Metrolist's feature)
- Lyrics support (Metrolist uses SimpMusic Lyrics API — not yet scoped)
