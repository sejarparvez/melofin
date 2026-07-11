use crate::mpv::MpvController;
use anyhow::Result;
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared, mutable "now playing" info the MPRIS server reports.
#[derive(Clone, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
}

/// Starts the MPRIS server and wires its transport controls (play/pause/
/// next/previous/seek/volume) to the mpv IPC controller.
///
/// `mpris_server::Player` is built on `Rc`/`RefCell` internally (it is
/// intentionally single-threaded, not `Send`), so the event loop and the
/// state-polling task below must run via `tokio::task::spawn_local` inside
/// a `LocalSet`, not plain `tokio::spawn`. The caller (`main`) is
/// responsible for driving this from inside `LocalSet::run_until`.
pub async fn start(
    mpv: Arc<MpvController>,
    now_playing: Arc<Mutex<NowPlaying>>,
) -> Result<Rc<Player>> {
    let player = Player::builder("melofin")
        .identity("Melofin")
        .can_play(true)
        .can_pause(true)
        .can_seek(true)
        .can_go_next(false) // wire up once a queue exists
        .can_go_previous(false)
        .can_control(true)
        .build()
        .await?;
    let player = Rc::new(player);

    {
        let mpv = mpv.clone();
        player.connect_play_pause(move |_| {
            let mpv = mpv.clone();
            tokio::spawn(async move {
                if let Err(e) = mpv.play_pause().await {
                    tracing::warn!("mpris play_pause: {e}");
                }
            });
        });
    }
    {
        let mpv = mpv.clone();
        player.connect_play(move |_| {
            let mpv = mpv.clone();
            tokio::spawn(async move {
                let _ = mpv.set_pause(false).await;
            });
        });
    }
    {
        let mpv = mpv.clone();
        player.connect_pause(move |_| {
            let mpv = mpv.clone();
            tokio::spawn(async move {
                let _ = mpv.set_pause(true).await;
            });
        });
    }
    {
        let mpv = mpv.clone();
        player.connect_seek(move |_, offset| {
            let mpv = mpv.clone();
            // Time is microsecond-precision (i64), not a std Duration.
            let offset_secs = offset.as_micros() as f64 / 1_000_000.0;
            tokio::spawn(async move {
                let _ = mpv.seek_relative(offset_secs).await;
            });
        });
    }
    {
        let mpv = mpv.clone();
        player.connect_set_position(move |_, _track_id, position| {
            let mpv = mpv.clone();
            let position_secs = position.as_micros() as f64 / 1_000_000.0;
            tokio::spawn(async move {
                let _ = mpv.seek_absolute(position_secs).await;
            });
        });
    }
    {
        let mpv = mpv.clone();
        player.connect_set_volume(move |_, volume| {
            let mpv = mpv.clone();
            tokio::spawn(async move {
                let _ = mpv.set_volume(volume).await;
            });
        });
    }

    // Reasonable static defaults; loop/shuffle become meaningful once a
    // queue exists (Build Step 4/5).
    player.set_loop_status(LoopStatus::None).await?;
    player.set_shuffle(false).await?;

    // `Player` is !Send (Rc-based), so its event loop and any task that
    // touches it must be spawned locally, not with plain `tokio::spawn`.
    let run_player = player.clone();
    tokio::task::spawn_local(async move {
        run_player.run().await;
    });

    poll_mpv_state(mpv, player.clone(), now_playing);

    Ok(player)
}

/// Periodically syncs mpv's actual playback state (paused/playing, position)
/// into the MPRIS properties, so external controllers (waybar, GNOME/KDE
/// media widgets, media keys) stay accurate even for state changes that
/// didn't originate from an MPRIS call (e.g. the track ending naturally).
fn poll_mpv_state(
    mpv: Arc<MpvController>,
    player: Rc<Player>,
    now_playing: Arc<Mutex<NowPlaying>>,
) {
    tokio::task::spawn_local(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            interval.tick().await;

            let paused = mpv.is_paused().await.unwrap_or(true);
            let status = if paused {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Playing
            };
            if let Err(e) = player.set_playback_status(status).await {
                tracing::warn!("mpris set_playback_status: {e}");
            }

            let now = now_playing.lock().await.clone();
            let duration_us = (mpv.duration_seconds().await.unwrap_or(0.0) * 1_000_000.0) as i64;
            let metadata = Metadata::builder()
                .title(now.title)
                .artist([now.artist])
                .length(Time::from_micros(duration_us))
                .build();
            let _ = player.set_metadata(metadata).await;
        }
    });
}
