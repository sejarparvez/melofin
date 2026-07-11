use anyhow::Result;
use melofin::mpris::{self, NowPlaying};
use melofin::mpv::MpvController;
use melofin::search::search;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

// `mpris_server::Player` is Rc-based (not `Send`), so we need a
// single-threaded runtime and a `LocalSet` to host it and anything that
// touches it (see src/mpris.rs).
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let local = tokio::task::LocalSet::new();
    local.run_until(run()).await
}

async fn run() -> Result<()> {
    dotenvy::dotenv().ok(); // fine if .env doesn't exist; env vars still work
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
