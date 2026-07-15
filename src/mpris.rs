use crate::mpv::MpvController;
use crate::player::PlayerCommand;
use anyhow::Result;
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
}

pub async fn start(
    mpv: Arc<MpvController>,
    now_playing: Arc<Mutex<NowPlaying>>,
    command_tx: async_channel::Sender<PlayerCommand>,
) -> Result<Rc<Player>> {
    let player = Player::builder("melofin")
        .identity("Melofin")
        .can_play(true)
        .can_pause(true)
        .can_seek(true)
        .can_go_next(true)
        .can_go_previous(true)
        .can_control(true)
        .build()
        .await?;
    let player = Rc::new(player);

    // -- Transport controls ---------------------------------------------------

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

    // Next/Previous go through the player command channel so the queue
    // logic (shuffle history, repeat, etc.) is handled correctly.
    {
        let tx = command_tx.clone();
        player.connect_next(move |_| {
            let _ = tx.send_blocking(PlayerCommand::Next);
        });
    }
    {
        let tx = command_tx.clone();
        player.connect_previous(move |_| {
            let _ = tx.send_blocking(PlayerCommand::Previous);
        });
    }

    player.set_loop_status(LoopStatus::None).await?;
    player.set_shuffle(false).await?;

    let run_player = player.clone();
    tokio::task::spawn_local(async move {
        run_player.run().await;
    });

    poll_mpv_state(mpv, player.clone(), now_playing);

    Ok(player)
}

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
