use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub url: String,
}

/// Runs `yt-dlp` against YouTube Music search and returns parsed results.
/// This is the only part of search that touches the outside world; the
/// actual parsing lives in [`parse_search_output`] so it can be unit tested
/// without a real `yt-dlp` binary or network access.
pub fn search(query: &str) -> Result<Vec<Track>> {
    let output = Command::new("yt-dlp")
        .arg("--flat-playlist")
        .arg("--print")
        .arg("%(title)s\t%(uploader)s\t%(webpage_url)s")
        .arg(format!("ytsearch10:{query}"))
        .output()?;

    Ok(parse_search_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Parses `yt-dlp --print "%(title)s\t%(uploader)s\t%(webpage_url)s"`
/// output (one tab-separated result per line) into [`Track`]s. Lines that
/// don't have all three fields are skipped rather than erroring, since
/// `yt-dlp` occasionally emits warnings or blank lines on stdout.
pub fn parse_search_output(raw: &str) -> Vec<Track> {
    raw.lines()
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_lines() {
        let raw = "Song A\tArtist A\thttps://youtu.be/a\nSong B\tArtist B\thttps://youtu.be/b\n";
        let tracks = parse_search_output(raw);
        assert_eq!(
            tracks,
            vec![
                Track {
                    title: "Song A".into(),
                    artist: "Artist A".into(),
                    url: "https://youtu.be/a".into(),
                },
                Track {
                    title: "Song B".into(),
                    artist: "Artist B".into(),
                    url: "https://youtu.be/b".into(),
                },
            ]
        );
    }

    #[test]
    fn skips_malformed_lines() {
        // Missing the url field, and a stray blank line yt-dlp sometimes emits.
        let raw = "Song A\tArtist A\thttps://youtu.be/a\nSong B\tArtist B\n\n";
        let tracks = parse_search_output(raw);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Song A");
    }

    #[test]
    fn empty_input_yields_no_tracks() {
        assert!(parse_search_output("").is_empty());
    }

    #[test]
    fn title_containing_extra_tabs_still_parses_first_three_fields() {
        // Titles are the first field, so extra tabs later in the line
        // (e.g. from an unusual uploader name) shouldn't break parsing.
        let raw = "My Song\tSome\tWeird\tArtist\thttps://youtu.be/x\n";
        let tracks = parse_search_output(raw);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "My Song");
        assert_eq!(tracks[0].artist, "Some");
    }
}
