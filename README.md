# Melofin

**A native desktop YouTube Music client for Linux (Hyprland/Wayland-focused)**, built in Rust with GTK4 + libadwaita.

## Current Status (as of July 2026)

**Functional GUI application** with search, playback, bottom player bar, top bar, and full MPRIS support.

### Features Implemented
- **Search**: yt-dlp powered search (`ytsearch10:`) with robust parsing and unit tests.
- **Playback**: Long-lived headless `mpv` process controlled via JSON IPC socket.
  - Play/pause, seek (absolute/relative), volume.
  - Gapless-ready architecture.
- **Home Feed**: Personalized InnerTube feed (when logged in) with yt-dlp fallback.
- **Liked Songs**: Paginated liked songs from InnerTube API.
- **Auth**: Cookie-based authentication (browser auto-import via `rookie` or manual file picker).
- **UI** (GTK4 + libadwaita):
  - Clean Adwaita window (optimized for tiling).
  - Top bar with search entry + overflow menu + account popover.
  - Search results view with debounced input and skeleton loading.
  - Bottom player bar with progress/seek, controls, and live updates.
  - Left sidebar ("Your Library") with navigation.
  - Right "Now Playing" panel.
- **Desktop Integration**:
  - Full MPRIS server (`playerctl`, media keys, waybar, GNOME/KDE widgets work).
  - Background player thread with async channels for clean UI ↔ Player separation.
- **Developer Experience**:
  - Tracing logs, dotenv support, proper error handling.
  - Unit tests for search parsing, auth, user profile, and cache logic.

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
| Home Feed | InnerTube API + yt-dlp fallback | Personalized when logged in |
| Auth      | rookie (browser cookie import) | No OAuth needed |
| Async     | tokio + async-channel | - |
| Logging   | tracing + tracing-subscriber | - |

**Not yet implemented** (per roadmap):
- Queue / playlists / library management
- SQLite persistence / offline cache
- Lyrics panel
- Mini player / fullscreen
- Preferences dialog

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
cargo run
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
├── lib.rs                  # Library root — re-exports all modules
├── main.rs                 # Entry point — tracing init + runs UI
├── auth.rs                 # Cookie-based YT Music auth
├── home_feed.rs            # Home feed: InnerTube + yt-dlp fallback
├── innertube.rs            # InnerTube browse API client
├── liked_songs.rs          # Liked songs via InnerTube browse
├── mpris.rs                # MPRIS server + media controls
├── mpv.rs                  # mpv IPC controller
├── player.rs               # Background player thread + channels
├── search.rs               # yt-dlp search + output parser
├── thumbnail.rs            # Thumbnail fetcher with disk cache
├── user.rs                 # User profile + cookie/auth helpers
└── ui/
    ├── mod.rs              # UI module root
    ├── window.rs           # Main window + app lifecycle
    ├── top_bar.rs          # Top bar: search, overflow menu, account
    ├── search_view.rs      # Search results with debounce + skeleton loading
    ├── player_bar.rs       # Bottom player bar (Spotify-style)
    ├── home_view.rs        # Home page with hero card + scrollable rows
    ├── library_sidebar.rs  # Left sidebar ("Your Library")
    ├── liked_songs_view.rs # Liked Songs page with lazy pagination
    ├── now_playing_panel.rs# Right "Now Playing" panel
    ├── login_dialog.rs     # Cookie import dialog
    ├── thumbnail_widget.rs # Shared fetch/decode/scale/crop pipeline
    └── style.css           # Dark theme CSS
```

## Roadmap (Next Steps)

- Queue management
- Library backend (playlists, albums, artists)
- SQLite persistence / offline cache
- Lyrics panel
- Mini player / fullscreen
- Preferences dialog
- Flatpak packaging

## Contributing

Contributions welcome! Focus on current architecture (channels, player service, etc.).

## License

GPL-3.0-or-later

Built with ❤️ for Linux desktop music listening.
