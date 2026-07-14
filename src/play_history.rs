use crate::search::Track;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const MAX_EVENTS: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayEvent {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub thumbnail_url: String,
    /// Unix timestamp (seconds) when the play was recorded.
    pub timestamp: u64,
    /// How far the user got (seconds). 0 = unknown.
    pub duration_played: f64,
    /// Total track duration (seconds) at time of play.
    pub duration_total: f64,
}

/// Extracts the YouTube video ID from a URL.
/// `"https://music.youtube.com/watch?v=dQw4w9WgXcQ"` → `"dQw4w9WgXcQ"`
pub fn video_id_from_url(url: &str) -> Option<String> {
    url.split_once("v=")
        .map(|(_, rest)| rest.split('&').next().unwrap_or(rest).to_string())
}

/// Reads all play events from the JSONL file.
pub fn load_history(path: &Path) -> Vec<PlayEvent> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Appends a single play event to the JSONL file. Creates the file if it
/// doesn't exist. After appending, trims to [`MAX_EVENTS`] if needed.
pub fn append_event(path: &Path, event: &PlayEvent) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let Ok(mut file) = File::options().create(true).append(true).open(path) else {
        return;
    };

    let Ok(line) = serde_json::to_string(event) else {
        return;
    };
    let _ = writeln!(file, "{line}");

    // Best-effort trim: count lines and rewrite if over cap.
    trim_to_cap(path);
}

/// Keeps only the newest [`MAX_EVENTS`] entries, rewriting the file.
fn trim_to_cap(path: &Path) {
    let events = load_history(path);
    if events.len() <= MAX_EVENTS {
        return;
    }

    let trimmed = &events[events.len() - MAX_EVENTS..];
    if let Ok(mut file) = File::create(path) {
        for event in trimmed {
            if let Ok(line) = serde_json::to_string(event) {
                let _ = writeln!(file, "{line}");
            }
        }
    }
}

/// Returns the most recently played tracks, deduplicated by video_id.
pub fn recently_played(events: &[PlayEvent], limit: usize) -> Vec<Track> {
    let mut seen = HashMap::new();
    // Iterate newest-first so the first occurrence of each video_id wins.
    for (i, event) in events.iter().enumerate().rev() {
        if !event.video_id.is_empty() {
            seen.entry(event.video_id.clone()).or_insert(i);
        }
    }

    // Sort by index descending (most recent first), take `limit`.
    let mut indices: Vec<usize> = seen.into_values().collect();
    indices.sort_unstable_by_key(|b| std::cmp::Reverse(*b));
    indices.truncate(limit);

    indices
        .into_iter()
        .filter_map(|i| events.get(i))
        .map(|e| Track {
            title: e.title.clone(),
            artist: e.artist.clone(),
            url: format!("https://music.youtube.com/watch?v={}", e.video_id),
            thumbnail_url: e.thumbnail_url.clone(),
        })
        .collect()
}

/// Returns the most-played tracks, ranked by play count.
pub fn top_tracks(events: &[PlayEvent], limit: usize) -> Vec<Track> {
    let mut counts: HashMap<String, (usize, &PlayEvent)> = HashMap::new();
    for event in events {
        if event.video_id.is_empty() {
            continue;
        }
        let entry = counts.entry(event.video_id.clone()).or_insert((0, event));
        entry.0 += 1;
    }

    let mut ranked: Vec<_> = counts.into_values().collect();
    ranked.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));
    ranked.truncate(limit);

    ranked
        .into_iter()
        .map(|(_, event)| Track {
            title: event.title.clone(),
            artist: event.artist.clone(),
            url: format!("https://music.youtube.com/watch?v={}", event.video_id),
            thumbnail_url: event.thumbnail_url.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_id_from_url_extracts_id() {
        assert_eq!(
            video_id_from_url("https://music.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".into())
        );
    }

    #[test]
    fn video_id_from_url_strips_extra_params() {
        assert_eq!(
            video_id_from_url("https://music.youtube.com/watch?v=abc123&list=PLxyz"),
            Some("abc123".into())
        );
    }

    #[test]
    fn video_id_from_url_returns_none_for_no_v() {
        assert!(video_id_from_url("https://example.com").is_none());
    }

    #[test]
    fn recently_played_deduplicates() {
        let events = vec![
            PlayEvent {
                video_id: "a".into(),
                title: "Song A".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 1,
                duration_played: 0.0,
                duration_total: 0.0,
            },
            PlayEvent {
                video_id: "b".into(),
                title: "Song B".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 2,
                duration_played: 0.0,
                duration_total: 0.0,
            },
            PlayEvent {
                video_id: "a".into(),
                title: "Song A".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 3,
                duration_played: 0.0,
                duration_total: 0.0,
            },
        ];
        let result = recently_played(&events, 10);
        assert_eq!(result.len(), 2);
        // Most recent first: "a" (timestamp 3), then "b" (timestamp 2).
        assert_eq!(result[0].title, "Song A");
        assert_eq!(result[1].title, "Song B");
    }

    #[test]
    fn top_tracks_ranks_by_count() {
        let events = vec![
            PlayEvent {
                video_id: "a".into(),
                title: "Song A".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 1,
                duration_played: 0.0,
                duration_total: 0.0,
            },
            PlayEvent {
                video_id: "b".into(),
                title: "Song B".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 2,
                duration_played: 0.0,
                duration_total: 0.0,
            },
            PlayEvent {
                video_id: "a".into(),
                title: "Song A".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 3,
                duration_played: 0.0,
                duration_total: 0.0,
            },
            PlayEvent {
                video_id: "a".into(),
                title: "Song A".into(),
                artist: "Artist".into(),
                thumbnail_url: String::new(),
                timestamp: 4,
                duration_played: 0.0,
                duration_total: 0.0,
            },
        ];
        let result = top_tracks(&events, 10);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title, "Song A"); // 3 plays
        assert_eq!(result[1].title, "Song B"); // 1 play
    }

    #[test]
    fn jsonl_round_trip() {
        let tmp =
            std::env::temp_dir().join(format!("melofin_history_test_{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let event = PlayEvent {
            video_id: "abc".into(),
            title: "Test".into(),
            artist: "Artist".into(),
            thumbnail_url: "https://example.com/thumb.jpg".into(),
            timestamp: 1234567890,
            duration_played: 42.5,
            duration_total: 240.0,
        };

        append_event(&tmp, &event);
        let loaded = load_history(&tmp);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].video_id, "abc");
        assert_eq!(loaded[0].title, "Test");
        assert_eq!(loaded[0].duration_played, 42.5);

        let _ = std::fs::remove_file(&tmp);
    }
}
