use crate::mpris::{self, NowPlaying};
use crate::mpv::MpvController;
use crate::queue::{Queue, RepeatMode};
use crate::search::Track;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub enum PlayerCommand {
    /// Replace the queue and start playing at the given index.
    ReplaceQueue(Vec<Track>, usize),
    /// Append tracks to the end of the queue.
    Enqueue(Vec<Track>),
    /// Insert a track right after the current one.
    PlayNext(Track),
    /// Skip to the next track.
    Next,
    /// Go back to the previous track.
    Previous,
    /// Jump to a specific track in the queue by index.
    PlayIndex(usize),
    /// Remove a track from the queue by index.
    RemoveFromQueue(usize),
    /// Toggle shuffle on/off.
    SetShuffle(bool),
    /// Set repeat mode.
    SetRepeat(RepeatMode),
    TogglePause,
    /// Seek to an absolute position, in seconds.
    Seek(f64),
    /// Set volume, 0.0-1.0.
    SetVolume(f64),
}

/// Snapshot of playback state, pushed to the UI on every tick (~2/sec) and
/// immediately after queue changes, so the player bar can redraw its track
/// info, transport icon, and seek position without polling mpv itself.
#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub title: String,
    pub artist: String,
    pub paused: bool,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub thumbnail_url: String,
    pub artist_browse_id: Option<String>,
    /// Index of the current track within the queue (0-based).
    pub queue_index: Option<usize>,
    /// Total number of tracks in the queue.
    pub queue_len: usize,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

/// Full snapshot of the queue contents. Sent less frequently than
/// `PlayerState` — only on queue changes (add/remove/replace/shuffle/toggle)
/// rather than every 500ms tick.
#[derive(Clone, Debug)]
pub struct QueueSnapshot {
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

/// What the player thread sends to the UI — either a regular playback tick
/// or a full queue snapshot.
pub enum PlayerEvent {
    State(PlayerState),
    Queue(QueueSnapshot),
}

pub struct PlayerHandle {
    pub commands: async_channel::Sender<PlayerCommand>,
    pub events: async_channel::Receiver<PlayerEvent>,
}

pub fn spawn_player_thread() -> PlayerHandle {
    let (command_tx, command_rx) = async_channel::unbounded::<PlayerCommand>();
    let (event_tx, event_rx) = async_channel::unbounded::<PlayerEvent>();

    // Clone the sender so the player thread can pass one to MPRIS.
    let command_tx_for_mpris = command_tx.clone();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build player runtime");
        let local = tokio::task::LocalSet::new();
        local.block_on(&runtime, run(command_rx, event_tx, command_tx_for_mpris));
    });

    PlayerHandle {
        commands: command_tx,
        events: event_rx,
    }
}

async fn run(
    commands: async_channel::Receiver<PlayerCommand>,
    event_tx: async_channel::Sender<PlayerEvent>,
    command_tx: async_channel::Sender<PlayerCommand>,
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

    let _player = match mpris::start(mpv.clone(), now_playing.clone(), command_tx).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("failed to start mpris: {e}");
            return;
        }
    };

    let mut current_thumbnail_url = String::new();
    let mut current_artist_browse_id: Option<String> = None;
    let mut queue = Queue::new();

    let mut ticker = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            command = commands.recv() => {
                let Ok(command) = command else { break };
                match command {
                    PlayerCommand::ReplaceQueue(tracks, start_index) => {
                        let start = start_index.min(tracks.len().saturating_sub(1));
                        queue.replace_with(tracks, start);
                        if let Some(track) = queue.current_track() {
                            play_track(
                                &mpv,
                                &event_tx,
                                &now_playing,
                                &mut current_thumbnail_url,
                                &mut current_artist_browse_id,
                                track,
                            )
                            .await;
                        }
                        send_queue_snapshot(&event_tx, &queue).await;
                    }
                    PlayerCommand::Enqueue(tracks) => {
                        queue.enqueue(tracks);
                        if queue.current_track().is_none() {
                            // Queue was empty; start playing the first new track.
                            queue.replace_with(queue.tracks().to_vec(), 0);
                            if let Some(track) = queue.current_track() {
                                play_track(
                                    &mpv,
                                    &event_tx,
                                    &now_playing,
                                    &mut current_thumbnail_url,
                                    &mut current_artist_browse_id,
                                    track,
                                )
                                .await;
                            }
                        }
                        send_queue_snapshot(&event_tx, &queue).await;
                    }
                    PlayerCommand::PlayNext(track) => {
                        queue.play_next(track);
                        send_queue_snapshot(&event_tx, &queue).await;
                    }
                    PlayerCommand::Next => {
                        if let Some(track) = queue.next() {
                            play_track(
                                &mpv,
                                &event_tx,
                                &now_playing,
                                &mut current_thumbnail_url,
                                &mut current_artist_browse_id,
                                track,
                            )
                            .await;
                        }
                    }
                    PlayerCommand::Previous => {
                        if let Some(track) = queue.previous() {
                            play_track(
                                &mpv,
                                &event_tx,
                                &now_playing,
                                &mut current_thumbnail_url,
                                &mut current_artist_browse_id,
                                track,
                            )
                            .await;
                        }
                    }
                    PlayerCommand::PlayIndex(index) => {
                        if let Some(track) = queue.play_index(index) {
                            play_track(
                                &mpv,
                                &event_tx,
                                &now_playing,
                                &mut current_thumbnail_url,
                                &mut current_artist_browse_id,
                                track,
                            )
                            .await;
                        }
                    }
                    PlayerCommand::RemoveFromQueue(index) => {
                        let was_current = queue.remove(index);
                        if was_current
                            && let Some(track) = queue.current_track()
                        {
                            play_track(
                                &mpv,
                                &event_tx,
                                &now_playing,
                                &mut current_thumbnail_url,
                                &mut current_artist_browse_id,
                                track,
                            )
                            .await;
                        }
                        send_queue_snapshot(&event_tx, &queue).await;
                    }
                    PlayerCommand::SetShuffle(on) => {
                        queue.set_shuffle(on);
                        if on {
                            queue.shuffle_in_place();
                            // After reshuffling, play the track at index 0.
                            if let Some(track) = queue.current_track() {
                                play_track(
                                    &mpv,
                                    &event_tx,
                                    &now_playing,
                                    &mut current_thumbnail_url,
                                    &mut current_artist_browse_id,
                                    track,
                                )
                                .await;
                            }
                        }
                        send_queue_snapshot(&event_tx, &queue).await;
                    }
                    PlayerCommand::SetRepeat(mode) => {
                        queue.set_repeat(mode);
                        send_queue_snapshot(&event_tx, &queue).await;
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
                    continue;
                }
                let paused = mpv.is_paused().await.unwrap_or(true);
                let position = mpv.position_seconds().await.unwrap_or(0.0);
                let duration = mpv.duration_seconds().await.unwrap_or(0.0);
                let _ = event_tx
                    .send(PlayerEvent::State(PlayerState {
                        title: now.title,
                        artist: now.artist,
                        paused,
                        position_seconds: position,
                        duration_seconds: duration,
                        thumbnail_url: current_thumbnail_url.clone(),
                        artist_browse_id: current_artist_browse_id.clone(),
                        queue_index: queue.current_index(),
                        queue_len: queue.len(),
                        shuffle: queue.shuffle(),
                        repeat: queue.repeat(),
                    }))
                    .await;
            }
        }
    }
}

/// Loads a track into mpv and sends an immediate state update.
async fn play_track(
    mpv: &Arc<MpvController>,
    event_tx: &async_channel::Sender<PlayerEvent>,
    now_playing: &Arc<Mutex<NowPlaying>>,
    thumbnail_url: &mut String,
    artist_browse_id: &mut Option<String>,
    track: &Track,
) {
    *now_playing.lock().await = NowPlaying {
        title: track.title.clone(),
        artist: track.artist.clone(),
    };
    *thumbnail_url = track.thumbnail_url.clone();
    *artist_browse_id = track.artist_browse_id.clone();

    if let Err(e) = mpv.load(&track.url).await {
        tracing::warn!("mpv load failed: {e}");
        return;
    }

    // Record play event for history tracking.
    let history_path = std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".local/share/melofin/play_history.jsonl");
    let event = crate::play_history::PlayEvent {
        video_id: crate::play_history::video_id_from_url(&track.url).unwrap_or_default(),
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

    let _ = event_tx
        .send(PlayerEvent::State(PlayerState {
            title: track.title.clone(),
            artist: track.artist.clone(),
            paused: false,
            position_seconds: 0.0,
            duration_seconds: 0.0,
            thumbnail_url: thumbnail_url.clone(),
            artist_browse_id: artist_browse_id.clone(),
            queue_index: None, // will be set by next tick
            queue_len: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
        }))
        .await;
}

async fn send_queue_snapshot(event_tx: &async_channel::Sender<PlayerEvent>, queue: &Queue) {
    let _ = event_tx
        .send(PlayerEvent::Queue(QueueSnapshot {
            tracks: queue.tracks().to_vec(),
            current_index: queue.current_index(),
            shuffle: queue.shuffle(),
            repeat: queue.repeat(),
        }))
        .await;
}
