use anyhow::Result;
use std::env;
use std::process::Command;

#[derive(Debug)]
struct Track {
    title: String,
    artist: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let query = env::args()
        .nth(1)
        .unwrap_or_else(|| "lofi beats".to_string());

    println!("🔍 Searching for: {}", query);

    // Get search results
    let output = Command::new("yt-dlp")
        .arg("--flat-playlist")
        .arg("--print")
        .arg("%(title)s\t%(uploader)s\t%(webpage_url)s")
        .arg(format!("ytsearch10:{}", query))
        .output()?;

    let results: Vec<Track> = String::from_utf8_lossy(&output.stdout)
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
        .collect();

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    // Show results
    println!("\n🎵 Search Results:");
    for (i, track) in results.iter().enumerate() {
        println!("{:2}. {} — {}", i + 1, track.title, track.artist);
    }

    // Let user choose
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

    let track = &results[index];
    println!("\n▶️  Now Playing: {} — {}", track.title, track.artist);

    // Play with mpv
    Command::new("mpv")
        .arg("--no-video")
        .arg("--force-window=no")
        .arg(&track.url)
        .status()?;

    println!("Playback finished.");

    Ok(())
}
