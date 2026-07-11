//! Minimal manual-testing tool: runs a search and prints the results.
//! Useful for checking `yt-dlp` output / search parsing without spinning up
//! mpv or the MPRIS server. Run with: `cargo run --bin search-test -- <query>`

use anyhow::Result;
use melofin::search::search;
use std::env;

fn main() -> Result<()> {
    let query = env::args()
        .nth(1)
        .unwrap_or_else(|| "lofi beats".to_string());

    println!("🔍 Searching for: {query}");
    let results = search(&query)?;

    if results.is_empty() {
        println!("No results found.");
        return Ok(());
    }

    for (i, track) in results.iter().enumerate() {
        println!(
            "{:2}. {} — {} [{}]",
            i + 1,
            track.title,
            track.artist,
            track.url
        );
    }

    Ok(())
}
