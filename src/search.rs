use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub url: String,
    /// URL of the track's thumbnail image, or empty if we couldn't
    /// determine a video id to build one from.
    pub thumbnail_url: String,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub artist_browse_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Song,
    Playlist,
    Album,
    Artist,
}

impl Track {
    /// Returns whether this track is a song, playlist, album, or artist
    /// based on its URL pattern.
    pub fn media_kind(&self) -> MediaKind {
        if self.url.contains("/browse/") {
            MediaKind::Playlist
        } else {
            MediaKind::Song
        }
    }

    /// Extracts the YouTube Music browse_id from a `browse/` URL.
    /// Returns `None` for song URLs (`watch?v=`).
    pub fn browse_id(&self) -> Option<&str> {
        self.url
            .strip_prefix("https://music.youtube.com/browse/")
    }
}

/// Runs `yt-dlp` against YouTube Music search and returns parsed results.
/// This is the only part of search that touches the outside world; the
/// actual parsing lives in [`parse_search_output`] so it can be unit tested
/// without a real `yt-dlp` binary or network access.
pub fn search(query: &str) -> Result<Vec<Track>> {
    let output = Command::new("yt-dlp")
        .arg("--flat-playlist")
        .arg("--print")
        .arg("%(id)s\t%(title)s\t%(uploader)s\t%(webpage_url)s")
        .arg(format!("ytsearch10:{query}"))
        .output()?;

    Ok(parse_search_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Parses `yt-dlp --print "%(id)s\t%(title)s\t%(uploader)s\t%(webpage_url)s"`
/// output (one tab-separated result per line) into [`Track`]s. Lines that
/// don't have all four fields are skipped rather than erroring, since
/// `yt-dlp` occasionally emits warnings or blank lines on stdout.
///
/// We deliberately don't ask yt-dlp for `%(thumbnail)s` here: with
/// `--flat-playlist`, yt-dlp skips full per-video extraction (that's the
/// whole point of "flat"), and the singular `thumbnail` field is only
/// derived from the `thumbnails` list during that full-extraction step
/// (`YoutubeDL.process_video_result`, confirmed by reading yt-dlp's
/// source) — so `%(thumbnail)s` always prints "NA" for flat search
/// results. The video id, on the other hand, is always present (it's set
/// unconditionally from the search renderer), so we build the thumbnail
/// URL ourselves from YouTube's standard, stable thumbnail CDN pattern
/// instead of depending on yt-dlp's per-version extraction internals.
pub fn parse_search_output(raw: &str) -> Vec<Track> {
    raw.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                let id = parts[0];
                Some(Track {
                    title: parts[1].to_string(),
                    artist: parts[2].to_string(),
                    url: parts[3].to_string(),
                    thumbnail_url: thumbnail_url_for_id(id),
                    track_number: None,
                    duration: None,
                    album: None,
                    artist_browse_id: None,
                })
            } else {
                None
            }
        })
        .collect()
}

/// YouTube serves an `mqdefault.jpg` thumbnail for essentially every public
/// video at this predictable, unauthenticated URL. We downscale to ~48px
/// for display either way, so there's no reason to fetch `hqdefault.jpg`
/// (480x360, 2-3x the bytes) or `maxresdefault.jpg` (which additionally
/// 404s for a lot of videos that never had a high-res thumbnail
/// generated) — `mqdefault.jpg` (320x180) is plenty of source resolution
/// and meaningfully faster to download. Returns an empty string if `id` is
/// missing/unresolved ("NA", yt-dlp's marker for that).
fn thumbnail_url_for_id(id: &str) -> String {
    if id.is_empty() || id == "NA" {
        String::new()
    } else {
        format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_lines() {
        let raw = "aaaaaaaaaaa\tSong A\tArtist A\thttps://youtu.be/a\n\
                   bbbbbbbbbbb\tSong B\tArtist B\thttps://youtu.be/b\n";
        let tracks = parse_search_output(raw);
        assert_eq!(
            tracks,
            vec![
                Track {
                    title: "Song A".into(),
                    artist: "Artist A".into(),
                    url: "https://youtu.be/a".into(),
                    thumbnail_url: "https://i.ytimg.com/vi/aaaaaaaaaaa/mqdefault.jpg".into(),
                    track_number: None,
                    duration: None,
                    album: None,
                    artist_browse_id: None,
                },
                Track {
                    title: "Song B".into(),
                    artist: "Artist B".into(),
                    url: "https://youtu.be/b".into(),
                    thumbnail_url: "https://i.ytimg.com/vi/bbbbbbbbbbb/mqdefault.jpg".into(),
                    track_number: None,
                    duration: None,
                    album: None,
                    artist_browse_id: None,
                },
            ]
        );
    }

    #[test]
    fn missing_id_becomes_empty_thumbnail() {
        let raw = "NA\tSong A\tArtist A\thttps://youtu.be/a\n";
        let tracks = parse_search_output(raw);
        assert_eq!(tracks.len(), 1);
        assert!(tracks[0].thumbnail_url.is_empty());
    }

    #[test]
    fn skips_malformed_lines() {
        // Missing the url field, and a stray blank line yt-dlp sometimes
        // emits.
        let raw = "aaaaaaaaaaa\tSong A\tArtist A\thttps://youtu.be/a\n\
                   bbbbbbbbbbb\tSong B\tArtist B\n\n";
        let tracks = parse_search_output(raw);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Song A");
    }

    #[test]
    fn empty_input_yields_no_tracks() {
        assert!(parse_search_output("").is_empty());
    }

    #[test]
    fn title_containing_extra_tabs_still_parses_id_and_title() {
        // Titles are the second field, so extra tabs later in the line
        // (e.g. from an unusual uploader name) shouldn't break parsing of
        // the id/title — though with a fixed 4-column layout, a stray tab
        // does shift artist/url out of position, which is an inherent
        // limitation of unescaped tab-separated parsing rather than
        // something this test tries to fully guard against.
        let raw = "aaaaaaaaaaa\tMy Song\tSome\tWeird\tArtist\thttps://youtu.be/x\n";
        let tracks = parse_search_output(raw);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "My Song");
        assert_eq!(tracks[0].artist, "Some");
    }
}
