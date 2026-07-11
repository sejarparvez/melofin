mod mpris;
mod mpv;

use anyhow::Result;
use mpris::NowPlaying;
use mpv::MpvController;
use std::env;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct Track {
    title: String,
    artist: String,
    url: String,
}

fn search(query: &str) -> Result<Vec<Track>> {
    let output = Command::new("yt-dlp")
        .arg("--flat-playlist")
        .arg("--print")
        .arg("%(title)s\t%(uploader)s\t%(webpage_url)s")
        .arg(format!("ytsearch10:{}", query))
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                Some(Track {
                    title: parts[0].to_string(),
                    artist: parts[1].to_string(),
                    url: parts[2].to_string(),
                })
            } else {
                None
            }
        })
        .collect())
}

// `mpris_server::Player` is Rc-based (not `Send`), so we need a
// single-threaded runtime and a `LocalSet` to host it and anything that
// touches it (see src/mpris.rs).
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let local = tokio::task::LocalSet::new();
    local.run_until(run()).await
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt::init();

    let query = env::args()
        .nth(1)
        .unwrap_or_else(|| "lofi beats".to_string());

    println!("🔍 Searching for: {}", query);
    let results = search(&query)?;

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    println!("\n🎵 Search Results:");
    for (i, track) in results.iter().enumerate() {
        println!("{:2}. {} — {}", i + 1, track.title, track.artist);
    }

    println!("\nEnter number to play (or q to quit): ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim();
    if choice.eq_ignore_ascii_case("q") {
        return Ok(());
    }
    let index: usize = match choice.parse::<usize>() {
        Ok(n) if n > 0 && n <= results.len() => n - 1,
        _ => {
            println!("Invalid choice. Playing first result.");
            0
        }
    };
    let track = results[index].clone();

    // Long-lived mpv instance, controlled over its IPC socket instead of a
    // one-shot blocking `Command::status()` call, so MPRIS can talk to it.
    let socket_path = format!("/tmp/melofin-mpv-{}.sock", std::process::id());
    let mpv = Arc::new(MpvController::spawn(&socket_path).await?);

    let now_playing = Arc::new(Mutex::new(NowPlaying {
        title: track.title.clone(),
        artist: track.artist.clone(),
    }));

    let _player = mpris::start(mpv.clone(), now_playing.clone()).await?;

    println!("\n▶️  Now Playing: {} — {}", track.title, track.artist);
    println!("MPRIS is live — try media keys / `playerctl status`.");
    mpv.load(&track.url).await?;

    // Keep the process (and the MPRIS server / mpv instance) alive.
    // Ctrl+C to quit for now; a real queue/UI loop replaces this in Step 4/5.
    tokio::signal::ctrl_c().await?;
    println!("\nShutting down…");
    mpv.quit().await?;

    Ok(())
}
