use crate::mpris::{self, NowPlaying};
use crate::mpv::MpvController;
use crate::search::Track;
use std::sync::Arc;
use tokio::sync::Mutex;

pub enum PlayerCommand {
    Play(Track),
    TogglePause,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub title: String,
    pub artist: String,
    pub paused: bool,
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

    while let Ok(command) = commands.recv().await {
        match command {
            PlayerCommand::Play(track) => {
                *now_playing.lock().await = NowPlaying {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                };
                if let Err(e) = mpv.load(&track.url).await {
                    tracing::warn!("mpv load failed: {e}");
                    continue;
                }
                let _ = state_tx
                    .send(PlayerState {
                        title: track.title,
                        artist: track.artist,
                        paused: false,
                    })
                    .await;
            }
            PlayerCommand::TogglePause => {
                if let Err(e) = mpv.play_pause().await {
                    tracing::warn!("mpv play_pause failed: {e}");
                }
            }
        }
    }
}
