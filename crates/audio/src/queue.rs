use std::time::{SystemTime, UNIX_EPOCH};

use musify_core::{QueueItem, RepeatMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RemoveOutcome {
    pub current_removed: bool,
}

/// Queue plus playback order. `order` holds indices into `items`; shuffle only
/// changes the order, never the queue itself.
#[derive(Debug)]
pub(crate) struct PlaybackQueue {
    items: Vec<QueueItem>,
    order: Vec<usize>,
    order_pos: usize,
    shuffle: bool,
    repeat: RepeatMode,
    rng: u64,
}

impl PlaybackQueue {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64);
        Self::with_seed(seed)
    }

    fn with_seed(seed: u64) -> Self {
        Self {
            items: Vec::new(),
            order: Vec::new(),
            order_pos: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
            rng: seed | 1,
        }
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn current_index(&self) -> Option<usize> {
        self.order.get(self.order_pos).copied()
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.current_index().and_then(|index| self.items.get(index))
    }

    pub fn set_tracks(&mut self, items: Vec<QueueItem>, start_index: usize, shuffle: bool) {
        self.items = items;
        self.shuffle = shuffle;
        let start = if self.items.is_empty() {
            None
        } else {
            Some(start_index.min(self.items.len() - 1))
        };
        self.rebuild_order(start);
    }

    pub fn enqueue(&mut self, items: Vec<QueueItem>) {
        if items.is_empty() {
            return;
        }
        let first_new = self.items.len();
        self.items.extend(items);
        self.order.extend(first_new..self.items.len());
        if self.order_pos >= self.order.len() {
            self.order_pos = self.order.len().saturating_sub(1);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.order.clear();
        self.order_pos = 0;
    }

    /// Point the play cursor at a concrete queue index. The engine uses this to
    /// keep the cursor on the audible track after a queue edit, because prefetch
    /// may have already advanced it to the next track.
    pub fn select(&mut self, index: usize) -> bool {
        let Some(position) = self.order.iter().position(|&candidate| candidate == index) else {
            return false;
        };
        self.order_pos = position;
        true
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        if self.shuffle == shuffle {
            return;
        }
        let current = self.current_index();
        self.shuffle = shuffle;
        self.rebuild_order(current);
        if let Some(current) = current {
            self.order_pos = self
                .order
                .iter()
                .position(|&index| index == current)
                .unwrap_or(0);
        }
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Move to the next track. Manual skips ignore `RepeatMode::One`.
    pub fn advance(&mut self, manual: bool) -> Option<usize> {
        if !manual && self.repeat == RepeatMode::One && self.current_index().is_some() {
            return self.current_index();
        }
        if self.order.is_empty() {
            return None;
        }
        if self.order_pos + 1 < self.order.len() {
            self.order_pos += 1;
            return self.current_index();
        }
        if self.repeat == RepeatMode::All {
            self.order_pos = 0;
            return self.current_index();
        }
        None
    }

    /// Move to the previous track, wrapping only with repeat-all.
    pub fn previous(&mut self) -> Option<usize> {
        if self.order.is_empty() {
            return None;
        }
        if self.order_pos > 0 {
            self.order_pos -= 1;
            return self.current_index();
        }
        if self.repeat == RepeatMode::All {
            self.order_pos = self.order.len() - 1;
            return self.current_index();
        }
        None
    }

    pub fn remove(&mut self, index: usize) -> RemoveOutcome {
        if index >= self.items.len() {
            return RemoveOutcome {
                current_removed: false,
            };
        }
        let current_before = self.current_index();
        self.items.remove(index);
        self.order.retain(|&order_index| order_index != index);
        for order_index in &mut self.order {
            if *order_index > index {
                *order_index -= 1;
            }
        }
        if self.order.is_empty() {
            self.order_pos = 0;
            return RemoveOutcome {
                current_removed: current_before == Some(index),
            };
        }
        if current_before == Some(index) {
            self.order_pos = self.order_pos.min(self.order.len() - 1);
            return RemoveOutcome {
                current_removed: true,
            };
        }
        if let Some(previous_current) = current_before {
            let adjusted = if previous_current > index {
                previous_current - 1
            } else {
                previous_current
            };
            if let Some(position) = self.order.iter().position(|&i| i == adjusted) {
                self.order_pos = position;
            }
        }
        RemoveOutcome {
            current_removed: false,
        }
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return;
        }
        let current = self.current_index();
        let item = self.items.remove(from);
        self.items.insert(to, item);
        let adjusted_current = current.map(|index| {
            if index == from {
                to
            } else if from < index && index <= to {
                index - 1
            } else if to <= index && index < from {
                index + 1
            } else {
                index
            }
        });
        self.rebuild_order(adjusted_current);
        if let Some(current) = adjusted_current {
            self.order_pos = self
                .order
                .iter()
                .position(|&index| index == current)
                .unwrap_or(0);
        }
    }

    fn rebuild_order(&mut self, start: Option<usize>) {
        let mut remaining: Vec<usize> = (0..self.items.len()).collect();
        self.order.clear();
        self.order_pos = 0;
        if let Some(start) = start.filter(|&index| index < self.items.len()) {
            self.order.push(start);
            remaining.retain(|&index| index != start);
        }
        if self.shuffle {
            self.shuffle_indices(&mut remaining);
        }
        self.order.extend(remaining);
    }

    fn shuffle_indices(&mut self, indices: &mut [usize]) {
        for i in (1..indices.len()).rev() {
            let j = (self.next_random() as usize) % (i + 1);
            indices.swap(i, j);
        }
    }

    fn next_random(&mut self) -> u64 {
        self.rng = self
            .rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.rng >> 33
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use musify_core::{QueueItemId, Track, TrackId};

    use super::*;

    fn queue_item(id: u64) -> QueueItem {
        QueueItem {
            id: QueueItemId::new(id),
            track: Track {
                id: TrackId::new(id as i64),
                path: PathBuf::from(format!("track-{id}.mp3")),
                title: format!("Track {id}"),
                artist: "Artist".into(),
                album: "Album".into(),
                album_artist: None,
                track_number: None,
                disc_number: None,
                duration: Duration::from_secs(1),
                year: None,
                genre: None,
                cover_path: None,
                added_at: 0,
            },
        }
    }

    fn queue_of(n: usize) -> PlaybackQueue {
        let mut queue = PlaybackQueue::with_seed(7);
        queue.set_tracks((0..n).map(|i| queue_item(i as u64)).collect(), 0, false);
        queue
    }

    #[test]
    fn sequential_advance_and_repeat_all() {
        let mut queue = queue_of(3);
        assert_eq!(queue.advance(false), Some(1));
        assert_eq!(queue.advance(false), Some(2));
        assert_eq!(queue.advance(false), None);
        queue.set_repeat(RepeatMode::All);
        assert_eq!(queue.advance(false), Some(0));
    }

    #[test]
    fn repeat_one_repeats_only_automatic_advance() {
        let mut queue = queue_of(2);
        queue.set_repeat(RepeatMode::One);
        assert_eq!(queue.advance(false), Some(0));
        assert_eq!(queue.advance(true), Some(1));
    }

    #[test]
    fn shuffle_keeps_the_start_and_is_a_permutation() {
        let mut queue = PlaybackQueue::with_seed(42);
        queue.set_tracks((0..20).map(|i| queue_item(i as u64)).collect(), 7, true);
        assert_eq!(queue.current_index(), Some(7));
        let mut seen: Vec<usize> = queue.order.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn remove_current_selects_the_next_slot() {
        let mut queue = queue_of(4);
        assert_eq!(queue.advance(false), Some(1));
        let outcome = queue.remove(1);
        assert!(outcome.current_removed);
        assert_eq!(queue.current_index(), Some(1));
        assert_eq!(queue.current().map(|item| item.id.get()), Some(2));
    }

    #[test]
    fn removing_before_current_keeps_the_same_track() {
        let mut queue = queue_of(4);
        assert_eq!(queue.advance(false), Some(1));
        assert_eq!(queue.advance(false), Some(2));
        let outcome = queue.remove(0);
        assert!(!outcome.current_removed);
        assert_eq!(queue.current_index(), Some(1));
        assert_eq!(queue.current().map(|item| item.id.get()), Some(2));
    }

    #[test]
    fn move_item_keeps_the_current_track() {
        let mut queue = queue_of(4);
        assert_eq!(queue.advance(false), Some(1));
        assert_eq!(queue.advance(false), Some(2));
        queue.move_item(2, 0);
        assert_eq!(queue.current().map(|item| item.id.get()), Some(2));
    }
}
