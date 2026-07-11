use rustypipe::client::RustyPipe;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let query = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "lofi".to_string());

    let rp = RustyPipe::new();
    let results = rp.query().music_search_tracks(&query).await?;

    for t in results.items.items {
        let artist = t
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown artist".to_string());

        println!(
            "{:>6}s  {} — {} [{}]",
            t.duration.unwrap_or(0),
            t.name,
            artist,
            t.id
        );
    }

    Ok(())
}
