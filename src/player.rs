use crate::mpris::{self, NowPlaying};
use crate::mpv::MpvController;
use crate::search::Track;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub enum PlayerCommand {
    Play(Track),
    TogglePause,
    /// Seek to an absolute position, in seconds. Sent by the player bar's
    /// seek scale on user interaction (not on programmatic updates).
    Seek(f64),
    /// Set volume, 0.0-1.0 (mpv-side clamping/scaling happens in mpv.rs).
    SetVolume(f64),
}

/// Snapshot of playback state, pushed to the UI on every tick (~2/sec) and
/// immediately after a `Play` command, so the player bar can redraw its
/// track info, transport icon, and seek position without polling mpv itself.
#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub title: String,
    pub artist: String,
    pub paused: bool,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    /// URL of the current track's thumbnail, or empty if it doesn't have
    /// one. Kept here rather than threaded through `mpris::NowPlaying`
    /// since this only needs to reach our own UI, not MPRIS metadata.
    pub thumbnail_url: String,
    /// Browse ID of the current artist (UC-prefixed channel ID), used to
    /// fetch artist bios in the now-playing panel.
    pub artist_browse_id: Option<String>,
}

pub struct PlayerHandle {
    pub commands: async_channel::Sender<PlayerCommand>,
    pub state: async_channel::Receiver<PlayerState>,
}

pub fn spawn_player_thread() -> PlayerHandle {
    let (command_tx, command_rx) = async_channel::unbounded::<PlayerCommand>();
    let (state_tx, state_rx) = async_channel::unbounded::<PlayerState>();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build player runtime");
        let local = tokio::task::LocalSet::new();
        local.block_on(&runtime, run(command_rx, state_tx));
    });

    PlayerHandle {
        commands: command_tx,
        state: state_rx,
    }
}

async fn run(
    commands: async_channel::Receiver<PlayerCommand>,
    state_tx: async_channel::Sender<PlayerState>,
) {
    let socket_path = format!("/tmp/melofin-mpv-{}.sock", std::process::id());
    let mpv = match MpvController::spawn(&socket_path).await {
        Ok(mpv) => Arc::new(mpv),
        Err(e) => {
            tracing::error!("failed to spawn mpv: {e}");
            return;
        }
    };

    let now_playing = Arc::new(Mutex::new(NowPlaying::default()));

    let _player = match mpris::start(mpv.clone(), now_playing.clone()).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("failed to start mpris: {e}");
            return;
        }
    };

    // Mirrors `now_playing.title`/`.artist` but isn't part of
    // `mpris::NowPlaying` — MPRIS metadata doesn't need it (yet; see
    // PlayerState::thumbnail_url doc comment), so it's tracked separately
    // here instead of widening that struct's contract.
    let mut current_thumbnail_url = String::new();
    let mut current_artist_browse_id: Option<String> = None;

    // Drives the player bar's seek scale/time labels. Kept separate from
    // mpris.rs's own 500ms poll loop (that one updates MPRIS metadata for
    // external tools like waybar; this one updates our own UI) — the two
    // are intentionally independent so this module doesn't have to depend
    // on mpris's internal Player/Rc types.
    let mut ticker = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            command = commands.recv() => {
                let Ok(command) = command else { break }; // sender dropped -> shut down
                match command {
                    PlayerCommand::Play(track) => {
                        *now_playing.lock().await = NowPlaying {
                            title: track.title.clone(),
                            artist: track.artist.clone(),
                        };
                        current_thumbnail_url = track.thumbnail_url.clone();
                        current_artist_browse_id = track.artist_browse_id.clone();
                        if let Err(e) = mpv.load(&track.url).await {
                            tracing::warn!("mpv load failed: {e}");
                            continue;
                        }

                        // Record play event for history tracking.
                        let history_path = std::env::var("HOME")
                            .ok()
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                            .join(".local/share/melofin/play_history.jsonl");
                        let event = crate::play_history::PlayEvent {
                            video_id: crate::play_history::video_id_from_url(&track.url)
                                .unwrap_or_default(),
                            title: track.title.clone(),
                            artist: track.artist.clone(),
                            thumbnail_url: track.thumbnail_url.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            duration_played: 0.0,
                            duration_total: 0.0,
                        };
                        crate::play_history::append_event(&history_path, &event);

                        let _ = state_tx
                            .send(PlayerState {
                                title: track.title,
                                artist: track.artist,
                                paused: false,
                                position_seconds: 0.0,
                                duration_seconds: 0.0,
                                thumbnail_url: current_thumbnail_url.clone(),
                                artist_browse_id: current_artist_browse_id.clone(),
                            })
                            .await;
                    }
                    PlayerCommand::TogglePause => {
                        if let Err(e) = mpv.play_pause().await {
                            tracing::warn!("mpv play_pause failed: {e}");
                        }
                    }
                    PlayerCommand::Seek(seconds) => {
                        if let Err(e) = mpv.seek_absolute(seconds).await {
                            tracing::warn!("mpv seek_absolute failed: {e}");
                        }
                    }
                    PlayerCommand::SetVolume(volume) => {
                        if let Err(e) = mpv.set_volume(volume).await {
                            tracing::warn!("mpv set_volume failed: {e}");
                        }
                    }
                }
            }
            _ = ticker.tick() => {
                let now = now_playing.lock().await.clone();
                if now.title.is_empty() {
                    continue; // nothing loaded yet — don't spam empty states
                }
                let paused = mpv.is_paused().await.unwrap_or(true);
                let position = mpv.position_seconds().await.unwrap_or(0.0);
                let duration = mpv.duration_seconds().await.unwrap_or(0.0);
                let _ = state_tx
                    .send(PlayerState {
                        title: now.title,
                        artist: now.artist,
                        paused,
                        position_seconds: position,
                        duration_seconds: duration,
                        thumbnail_url: current_thumbnail_url.clone(),
                        artist_browse_id: current_artist_browse_id.clone(),
                    })
                    .await;
            }
        }
    }
}
