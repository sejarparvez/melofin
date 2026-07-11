use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Controls a single `mpv` subprocess over its `--input-ipc-server` JSON socket.
///
/// mpv keeps running headlessly (`--idle`) between tracks so the same socket
/// stays valid for the lifetime of the app instead of respawning per-track.
pub struct MpvController {
    child: Mutex<Child>,
    socket: Mutex<UnixStream>,
}

impl MpvController {
    pub async fn spawn(socket_path: &str) -> Result<Self> {
        let _ = std::fs::remove_file(socket_path); // stale socket from a crashed run

        let child = Command::new("mpv")
            .arg("--idle") // keep running with no track loaded
            .arg("--no-video")
            .arg("--force-window=no")
            .arg(format!("--input-ipc-server={socket_path}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // mpv creates the socket shortly after starting; poll briefly for it.
        let socket = Self::connect_with_retry(socket_path).await?;

        Ok(Self {
            child: Mutex::new(child),
            socket: Mutex::new(socket),
        })
    }

    async fn connect_with_retry(socket_path: &str) -> Result<UnixStream> {
        for _ in 0..50 {
            if let Ok(s) = UnixStream::connect(socket_path).await {
                return Ok(s);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Err(anyhow!(
            "timed out waiting for mpv ipc socket at {socket_path}"
        ))
    }

    /// Send a raw mpv IPC command and return its `data` field, if any.
    async fn command(&self, args: Value) -> Result<Value> {
        let mut socket = self.socket.lock().await;
        let payload = json!({ "command": args });
        let mut line = serde_json::to_vec(&payload)?;
        line.push(b'\n');
        socket.write_all(&line).await?;

        // mpv may interleave unrelated event lines before the reply to this
        // command; skip those and take the first line with a "data"/"error" key.
        let mut reader = BufReader::new(&mut *socket);
        loop {
            let mut resp = String::new();
            reader.read_line(&mut resp).await?;
            if resp.trim().is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(&resp)?;
            if parsed.get("event").is_some() {
                continue; // async event notification, not our reply
            }
            if parsed.get("error").and_then(Value::as_str) != Some("success") {
                return Err(anyhow!("mpv command failed: {parsed}"));
            }
            return Ok(parsed.get("data").cloned().unwrap_or(Value::Null));
        }
    }

    pub async fn load(&self, url: &str) -> Result<()> {
        self.command(json!(["loadfile", url, "replace"])).await?;
        Ok(())
    }

    pub async fn set_pause(&self, paused: bool) -> Result<()> {
        self.command(json!(["set_property", "pause", paused]))
            .await?;
        Ok(())
    }

    pub async fn play_pause(&self) -> Result<()> {
        self.command(json!(["cycle", "pause"])).await?;
        Ok(())
    }

    pub async fn seek_absolute(&self, seconds: f64) -> Result<()> {
        self.command(json!(["seek", seconds, "absolute"])).await?;
        Ok(())
    }

    pub async fn seek_relative(&self, delta_seconds: f64) -> Result<()> {
        self.command(json!(["seek", delta_seconds, "relative"]))
            .await?;
        Ok(())
    }

    /// mpv volume is 0-100; MPRIS volume is 0.0-1.0.
    pub async fn set_volume(&self, mpris_volume: f64) -> Result<()> {
        let mpv_volume = (mpris_volume.clamp(0.0, 1.0) * 100.0).round();
        self.command(json!(["set_property", "volume", mpv_volume]))
            .await?;
        Ok(())
    }

    pub async fn position_seconds(&self) -> Result<f64> {
        let v = self.command(json!(["get_property", "time-pos"])).await?;
        Ok(v.as_f64().unwrap_or(0.0))
    }

    pub async fn duration_seconds(&self) -> Result<f64> {
        let v = self.command(json!(["get_property", "duration"])).await?;
        Ok(v.as_f64().unwrap_or(0.0))
    }

    pub async fn is_paused(&self) -> Result<bool> {
        let v = self.command(json!(["get_property", "pause"])).await?;
        Ok(v.as_bool().unwrap_or(true))
    }

    /// Ask mpv to exit gracefully, then hard-kill it if it doesn't.
    /// Takes `&self` (not `&mut self`) so it can be called through the
    /// shared `Arc<MpvController>` on shutdown; the child process handle
    /// is behind the same mutex as the socket.
    pub async fn quit(&self) -> Result<()> {
        let _ = self.command(json!(["quit"])).await;
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
        Ok(())
    }
}
