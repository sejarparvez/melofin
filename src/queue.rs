use crate::search::Track;
use rand::seq::SliceRandom;
use rand::thread_rng;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    #[default]
    Off,
    RepeatAll,
    RepeatOne,
}

/// The playback queue. Manages the ordered list of tracks, current position,
/// shuffle state, and repeat mode.
///
/// In shuffle mode a `history` stack tracks the order tracks were actually
/// played, so `previous()` can walk back through the real playback order
/// (not the original list order). When a track is played from a specific
/// position (via `play_index`), the history is truncated at the current
/// point and the new position is pushed — this mirrors Spotify's behaviour
/// where "previous" undoes the last skip.
pub struct Queue {
    tracks: Vec<Track>,
    /// Index into `tracks` of the currently-playing track.
    current_index: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    /// Indices into `tracks` in the order they were played, used to
    /// implement prev/next in shuffle mode.
    history: Vec<usize>,
    /// Position within `history` — points at the current track's entry.
    history_pos: Option<usize>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            history: Vec::new(),
            history_pos: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|i| self.tracks.get(i))
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Replaces the entire queue with `tracks`, starts playing at
    /// `start_index`. History is reset.
    pub fn replace_with(&mut self, tracks: Vec<Track>, start_index: usize) -> Option<&Track> {
        if tracks.is_empty() {
            return None;
        }
        let idx = start_index.min(tracks.len() - 1);
        self.tracks = tracks;
        self.current_index = Some(idx);
        self.history.clear();
        self.history.push(idx);
        self.history_pos = Some(0);
        self.current_track()
    }

    /// Appends tracks to the end of the queue and returns the index of the
    /// first appended track. Does not change what's currently playing.
    pub fn enqueue(&mut self, tracks: Vec<Track>) -> Option<usize> {
        if tracks.is_empty() {
            return None;
        }
        let start = self.tracks.len();
        self.tracks.extend(tracks);
        Some(start)
    }

    /// Inserts `track` right after the current position and returns it.
    /// The currently-playing track is unaffected.
    pub fn play_next(&mut self, track: Track) -> &Track {
        let insert_at = self
            .current_index
            .map_or(0, |i| (i + 1).min(self.tracks.len()));
        self.tracks.insert(insert_at, track);
        // Correct history indices >= insert_at since we shifted everything.
        for idx in &mut self.history {
            if *idx >= insert_at {
                *idx += 1;
            }
        }
        &self.tracks[insert_at]
    }

    /// Advances to the next track. Returns `Some(&Track)` if there is one,
    /// or `None` if the queue is exhausted (only possible with
    /// `RepeatMode::Off`).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }

        match self.repeat {
            RepeatMode::RepeatOne => self.current_track(),
            RepeatMode::RepeatAll | RepeatMode::Off => {
                if self.shuffle {
                    return self.next_shuffled();
                }

                let current = self.current_index?;
                let next = current + 1;
                if next < self.tracks.len() {
                    self.current_index = Some(next);
                    self.push_history(next);
                    self.current_track()
                } else if self.repeat == RepeatMode::RepeatAll {
                    self.current_index = Some(0);
                    self.push_history(0);
                    self.current_track()
                } else {
                    None
                }
            }
        }
    }

    /// Goes back to the previous track. Returns `Some(&Track)` if there is
    /// one, otherwise stays on the current track.
    pub fn previous(&mut self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }

        if self.shuffle {
            return self.previous_shuffled();
        }

        let current = self.current_index?;
        if current > 0 {
            self.current_index = Some(current - 1);
            if let Some(ref mut pos) = self.history_pos
                && *pos > 0
            {
                *pos -= 1;
            }
        }
        self.current_track()
    }

    /// Jumps to a specific track by its queue index. Used when the user
    /// clicks a row in the queue panel or a track in a list view.
    pub fn play_index(&mut self, index: usize) -> Option<&Track> {
        if index >= self.tracks.len() {
            return None;
        }
        self.current_index = Some(index);
        // Truncate future history and push this index.
        if let Some(pos) = self.history_pos {
            self.history.truncate(pos + 1);
        }
        self.history.push(index);
        self.history_pos = Some(self.history.len() - 1);
        self.current_track()
    }

    /// Removes a track from the queue by its index. Adjusts `current_index`
    /// and history accordingly. Returns `true` if the removed track was the
    /// currently-playing one (caller should then advance).
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.tracks.len() {
            return false;
        }
        self.tracks.remove(index);

        let was_current = self.current_index == Some(index);

        // Fix current_index.
        self.current_index = self.current_index.and_then(|ci| {
            if ci >= self.tracks.len() {
                // Track at the end was removed.
                if self.tracks.is_empty() {
                    None
                } else {
                    Some(self.tracks.len() - 1)
                }
            } else if was_current {
                // Current was removed; keep same index (points to next track now),
                // unless it was the last one.
                if ci >= self.tracks.len() {
                    if ci > 0 { Some(ci - 1) } else { None }
                } else {
                    Some(ci)
                }
            } else if ci > index {
                Some(ci - 1)
            } else {
                Some(ci)
            }
        });

        // Fix history indices.
        self.history.retain_mut(|idx| *idx != index);
        for idx in &mut self.history {
            if *idx > index {
                *idx -= 1;
            }
        }

        // Fix history_pos.
        if let Some(pos) = self.history_pos
            && pos >= self.history.len()
        {
            self.history_pos = if self.history.is_empty() {
                None
            } else {
                Some(self.history.len() - 1)
            };
        }

        was_current
    }

    /// Clears the entire queue and resets all state.
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
        self.history.clear();
        self.history_pos = None;
    }

    /// Shuffles the queue while keeping the currently-playing track in place.
    pub fn shuffle_in_place(&mut self) {
        if self.tracks.len() <= 1 {
            return;
        }
        let current = self.current_index.unwrap_or(0);
        let current_track = self.tracks.remove(current);

        let mut rng = thread_rng();
        self.tracks.shuffle(&mut rng);

        // Put the current track back at position 0 so it stays "current".
        self.tracks.insert(0, current_track);
        self.current_index = Some(0);

        // Reset history to just the current track.
        self.history = vec![0];
        self.history_pos = Some(0);
    }

    // -- private helpers --

    fn push_history(&mut self, index: usize) {
        if let Some(pos) = self.history_pos {
            self.history.truncate(pos + 1);
        }
        self.history.push(index);
        self.history_pos = Some(self.history.len() - 1);
    }

    fn next_shuffled(&mut self) -> Option<&Track> {
        // If we've been going back through history, advance within it first.
        if let Some(pos) = self.history_pos
            && pos + 1 < self.history.len()
        {
            self.history_pos = Some(pos + 1);
            let idx = self.history[pos + 1];
            self.current_index = Some(idx);
            return self.current_track();
        }

        // If repeat-all and we've played everything, reshuffle and restart.
        if self.repeat == RepeatMode::RepeatAll && self.history.len() >= self.tracks.len() {
            let current = self.current_index.unwrap_or(0);
            self.history = vec![current];
            self.history_pos = Some(0);
            return self.current_track();
        }

        // Pick a random track we haven't played yet.
        let played: std::collections::HashSet<usize> = self.history.iter().copied().collect();
        let candidates: Vec<usize> = (0..self.tracks.len())
            .filter(|i| !played.contains(i))
            .collect();

        if candidates.is_empty() {
            if self.repeat == RepeatMode::RepeatAll {
                // Reshuffle everything.
                let current = self.current_index.unwrap_or(0);
                self.history = vec![current];
                self.history_pos = Some(0);
                return self.current_track();
            }
            return None;
        }

        let mut rng = thread_rng();
        let &next = candidates.choose(&mut rng).unwrap();
        self.current_index = Some(next);
        self.push_history(next);
        self.current_track()
    }

    fn previous_shuffled(&mut self) -> Option<&Track> {
        if let Some(pos) = self.history_pos
            && pos > 0
        {
            let new_pos = pos - 1;
            self.history_pos = Some(new_pos);
            let idx = self.history[new_pos];
            self.current_index = Some(idx);
        }
        self.current_track()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            title: format!("Song {id}"),
            artist: format!("Artist {id}"),
            url: format!("https://example.com/{id}"),
            thumbnail_url: String::new(),
            track_number: None,
            duration: None,
            album: None,
            artist_browse_id: None,
        }
    }

    #[test]
    fn replace_with_starts_at_index() {
        let mut q = Queue::new();
        let tracks = vec![track("a"), track("b"), track("c")];
        q.replace_with(tracks, 1);
        assert_eq!(q.current_index(), Some(1));
        assert_eq!(q.current_track().unwrap().title, "Song b");
    }

    #[test]
    fn next_advances_and_wraps() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("b"), track("c")], 0);
        assert_eq!(q.next().unwrap().title, "Song b");
        assert_eq!(q.next().unwrap().title, "Song c");
        // Off mode: at end, returns None.
        assert!(q.next().is_none());
    }

    #[test]
    fn next_repeat_all_wraps() {
        let mut q = Queue::new();
        q.set_repeat(RepeatMode::RepeatAll);
        q.replace_with(vec![track("a"), track("b")], 0);
        q.next();
        assert_eq!(q.next().unwrap().title, "Song a");
    }

    #[test]
    fn next_repeat_one_stays() {
        let mut q = Queue::new();
        q.set_repeat(RepeatMode::RepeatOne);
        q.replace_with(vec![track("a"), track("b")], 0);
        assert_eq!(q.next().unwrap().title, "Song a");
        assert_eq!(q.next().unwrap().title, "Song a");
    }

    #[test]
    fn previous_goes_back() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("b"), track("c")], 0);
        q.next(); // -> b
        assert_eq!(q.previous().unwrap().title, "Song a");
        // Already at 0, stays.
        assert_eq!(q.previous().unwrap().title, "Song a");
    }

    #[test]
    fn play_index_jumps() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("b"), track("c")], 0);
        q.play_index(2);
        assert_eq!(q.current_track().unwrap().title, "Song c");
        // Previous goes back one position in the queue.
        q.previous();
        assert_eq!(q.current_track().unwrap().title, "Song b");
    }

    #[test]
    fn enqueue_adds_to_end() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a")], 0);
        q.enqueue(vec![track("b"), track("c")]);
        assert_eq!(q.len(), 3);
        q.next();
        assert_eq!(q.current_track().unwrap().title, "Song b");
    }

    #[test]
    fn play_next_inserts_after_current() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("c")], 0);
        q.play_next(track("b"));
        // a -> b -> c
        assert_eq!(q.len(), 3);
        q.next();
        assert_eq!(q.current_track().unwrap().title, "Song b");
        q.next();
        assert_eq!(q.current_track().unwrap().title, "Song c");
    }

    #[test]
    fn remove_track_shifts_correctly() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("b"), track("c")], 1);
        q.remove(0); // remove "a"
        assert_eq!(q.len(), 2);
        // current was index 1 ("b"), shifted to index 0.
        assert_eq!(q.current_track().unwrap().title, "Song b");
    }

    #[test]
    fn remove_current_advances() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("b"), track("c")], 0);
        let was_current = q.remove(0);
        assert!(was_current);
        // "b" is now at index 0.
        assert_eq!(q.current_track().unwrap().title, "Song b");
    }

    #[test]
    fn clear_resets_everything() {
        let mut q = Queue::new();
        q.replace_with(vec![track("a"), track("b")], 0);
        q.clear();
        assert!(q.is_empty());
        assert!(q.current_track().is_none());
    }
}
