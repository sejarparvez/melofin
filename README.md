# Melofin

**A native desktop YouTube Music client for Linux (Hyprland/Wayland-focused)**, built in Rust with GTK4 + libadwaita.

## Current Status (as of July 2026)

**Functional GUI application** with search, playback, bottom player bar, top bar, and full MPRIS support.

### Features Implemented
- **Search**: yt-dlp powered search (`ytsearch10:`) with robust parsing and unit tests.
- **Playback**: Long-lived headless `mpv` process controlled via JSON IPC socket.
  - Play/pause, seek (absolute/relative), volume.
  - Gapless-ready architecture.
- **UI** (GTK4 + libadwaita):
  - Clean Adwaita window (optimized for tiling).
  - Top bar with search entry + overflow menu (Quit, About).
  - Search results view.
  - Bottom player bar with progress/seek, controls, and live updates.
- **Desktop Integration**:
  - Full MPRIS server (`playerctl`, media keys, waybar, GNOME/KDE widgets work).
  - Background player thread with async channels for clean UI ↔ Player separation.
- **Developer Experience**:
  - Multiple binaries: `melofin` (full app), `search-test`, `ui-shell`.
  - Tracing logs, dotenv support, proper error handling.
  - Unit tests for search parsing.

### Architecture Overview

```
UI (GTK4/Adwaita)  ↔  async-channel  ↔  Player Service (tokio thread)
                                                  │
                                           MpvController (IPC)
                                                  │
                                           mpv subprocess (headless)
                                                  │
                                           MPRIS Server (mpris-server)
```

- Player runs in its own thread with a `LocalSet` to support MPRIS (Rc-based).
- State flows back to UI via receiver for live updates (progress, now-playing, pause state).
- Commands (Play, TogglePause, Seek, SetVolume) sent via channel.

### Tech Stack

| Component | Technology | Notes |
|-----------|------------|-------|
| Language  | Rust (2024 edition) | - |
| UI        | GTK4 + libadwaita | Native look & feel |
| Playback  | mpv (subprocess + JSON IPC) | Reliable, high quality |
| System Integration | mpris-server | Media controls |
| Search    | yt-dlp | Simple & effective |
| Async     | tokio + async-channel | - |
| Logging   | tracing + tracing-subscriber | - |

**Not yet implemented** (per original roadmap):
- rustypipe / official InnerTube integration
- Queue / playlists / library management
- SQLite persistence / offline cache
- Account sync / auth
- Lyrics, home feed, advanced browsing

---

## Getting Started

### Prerequisites

```bash
# Arch / Hyprland (recommended)
sudo pacman -S mpv yt-dlp gtk4 libadwaita

# Debian/Ubuntu
sudo apt install mpv yt-dlp libgtk-4-dev libadwaita-1-dev
```

### Build & Run

```bash
git clone https://github.com/sejarparvez/melofin.git
cd melofin

# Full application
cargo run

# Or specific binaries
cargo run --bin search-test -- "lofi beats"
cargo run --bin ui-shell
```

### Environment Variables

- `RUST_LOG=debug` — for detailed logs

See `env.example` for more.

### Development

```bash
cargo fmt
cargo clippy
cargo test

# Watch logs
RUST_LOG=debug cargo run
```

## Project Structure

```
src/
├── lib.rs
├── main.rs                 # Full app entry (CLI fallback + GUI)
├── bin/
│   ├── search_test.rs
│   └── ui_shell.rs
├── search.rs               # yt-dlp + parser + tests
├── mpv.rs                  # IPC controller
├── player.rs               # Background service + state
├── mpris.rs                # MPRIS integration
└── ui/
    ├── window.rs
    ├── top_bar.rs
    ├── search_view.rs
    └── player_bar.rs
```

## Roadmap (Next Steps)

- Polish current implementation (UI/UX, stability, error handling)
- Add queue management
- Integrate rustypipe for richer metadata & features
- Persistence (SQLite)
- Advanced features (offline, library, etc.)

## Contributing

Contributions welcome! Focus on current architecture (channels, player service, etc.).

## License

GPL-3.0-or-later

Built with ❤️ for Linux desktop music listening.
