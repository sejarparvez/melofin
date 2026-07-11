# Melofin

A native desktop YouTube Music client for Linux/Hyprland, inspired by [Metrolist](https://github.com/metrolistgroup/metrolist) (Android).

## Overview

Melofin is a Rust-based YouTube Music client designed to provide a native, performant experience on Linux desktops—particularly optimized for Hyprland/Wayland environments. It leverages the InnerTube API via `rustypipe` for backend operations and `libmpv` for high-quality audio playback.

## Features

- **Search** — Songs, albums, artists, and playlists
- **Home Feed** — Quick picks and personalized recommendations
- **Playback** — Background/gapless playback with full MPRIS integration (media keys, waybar/notification controls)
- **Library Management** — Playlists, saved songs/albums, queue management
- **Offline Caching** — Cache streamed tracks for offline listening
- **Account Sync** — Optional YouTube account login (cookie-based auth via `rustypipe`) for library synchronization

## Tech Stack

| Component          | Technology                                            | Notes                                                                             |
| ------------------ | ----------------------------------------------------- | --------------------------------------------------------------------------------- |
| Language           | Rust                                                  | Edition 2024                                                                      |
| YT Music Backend   | [`rustypipe`](https://github.com/TeamPiped/rustypipe) | InnerTube client — search, browse, library, playlists, stream URL extraction      |
| Playback           | `libmpv` (via `libmpv-rs` or FFI)                     | Handles adaptive/Opus streams, gapless playback, seek/volume control              |
| UI                 | GTK4 + libadwaita                                     | Native look on Wayland/Hyprland, no custom theming needed                         |
| Local Storage      | SQLite (`rusqlite`)                                   | Library cache, playlists, downloaded track metadata                               |
| System Integration | MPRIS (`mpris-server`)                                | Media keys, waybar/notification controls                                          |
| Async Runtime      | tokio                                                 | Required by `rustypipe`; drives GTK async bridging via `glib::spawn_future_local` |

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

## Project Status

🚧 **Early Development** — Currently at **Build Step 1** (Backend spike)

The project follows an incremental build plan:

1. **Backend spike** ✅ — Cargo workspace, `rustypipe` wired up, search + stream URL resolution working (CLI test binary)
2. **Playback spike** — Pipe resolved stream URL into `libmpv`, verify headless playback
3. **MPRIS** — Wrap player with MPRIS for media key support
4. **UI shell** — GTK4/libadwaita window: search box + results list
5. **Wire UI → player service** — Connect search results and playback controls
6. **SQLite layer** — Metadata cache, then playlists/library
7. **Auth** — Cookie-based YouTube login for library sync (last, optional)

## Getting Started

### Prerequisites

- **Rust** (stable, edition 2024) — install via [rustup](https://rustup.rs/)
- **libmpv** — system dependency for audio playback
  - Arch: `pacman -S mpv`
  - Ubuntu/Debian: `apt install libmpv-dev`
  - Fedora: `dnf install mpv-devel`
- **GTK4 + libadwaita** — for UI development
  - Arch: `pacman -S gtk4 libadwaita`
  - Ubuntu/Debian: `apt install libgtk-4-dev libadwaita-1-dev`
  - Fedora: `dnf install gtk4-devel libadwaita-devel`
- **rustypipe-botguard** binary — required for PO tokens (stream access)
  - See [rustypipe docs](https://github.com/TeamPiped/rustypipe#botguard) for setup

### Building

```bash
# Clone the repository
git clone https://github.com/sejarparvez/melofin.git
cd melofin

# Build the project
cargo build --release

# Run the CLI test binary (search demo)
cargo run -- "search query"
# Example: cargo run -- "lofi"
```

### Development

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- "your search"

# Format code
cargo fmt

# Lint
cargo clippy

# Run tests (when available)
cargo test
```

## Project Structure

```
melofin/
├── src/
│   └── main.rs          # CLI entry point (search demo)
├── doc/
│   └── GUIDE.md         # Architecture & development guide
├── Cargo.toml           # Project manifest
├── Cargo.lock           # Dependency lockfile
├── clippy.toml          # Clippy configuration
├── rustfmt.toml         # Rustfmt configuration
├── rustypipe_cache.json # rustypipe cache (auto-generated)
└── .gitignore
```

## Configuration

### Environment Variables

| Variable   | Description                              | Default |
| ---------- | ---------------------------------------- | ------- |
| `RUST_LOG` | Logging level (debug, info, warn, error) | `info`  |

### rustypipe Cache

The `rustypipe_cache.json` file stores authentication cookies and botguard data. It's auto-generated and should not be committed (listed in `.gitignore`).

## Roadmap / Open Questions

- [ ] Offline download storage format/location and cache size limits
- [ ] Playlist import (M3U/CSV, matching Metrolist's feature)
- [ ] Lyrics support (Metrolist uses SimpMusic Lyrics API — not yet scoped)
- [ ] Full GTK4 UI implementation
- [ ] System tray / background daemon mode
- [ ] Keyboard shortcuts and global hotkeys

## Contributing

This project is in early development. Contributions are welcome once the core architecture is stabilized. Please check the [GUIDE.md](doc/GUIDE.md) for architecture details and build order.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).

## Acknowledgments

- [rustypipe](https://github.com/TeamPiped/rustypipe) — Excellent InnerTube client library
- [Metrolist](https://github.com/metrolistgroup/metrolist) — Design inspiration
- [libmpv](https://mpv.io/) — Powerful media playback backend
