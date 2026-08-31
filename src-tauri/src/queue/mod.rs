use std::fmt;

use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{SourceId, TrackId};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct QueueEntryId(Uuid);

impl QueueEntryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for QueueEntryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for QueueEntryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
}

impl QueueEntry {
    pub fn new(track_id: TrackId, requested_source_id: Option<SourceId>) -> Self {
        Self {
            id: QueueEntryId::new(),
            track_id,
            requested_source_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

#[derive(Clone, Debug, Default)]
pub struct TransientQueue {
    entries: Vec<QueueEntry>,
    current_index: Option<usize>,
    traversal_order: Vec<usize>,
    play_next_order: Vec<QueueEntryId>,
    shuffle_enabled: bool,
}

impl TransientQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play_now(&mut self, entry: QueueEntry) {
        self.entries.clear();
        self.play_next_order.clear();
        self.entries.push(entry);
        self.current_index = Some(0);
        self.rebuild_traversal_order(None);
    }

    pub fn append(&mut self, entry: QueueEntry) {
        self.entries.push(entry);
        self.rebuild_traversal_order(None);
    }

    pub fn insert_next(&mut self, entry: QueueEntry) {
        let entry_id = entry.id;
        self.entries.push(entry);
        self.play_next_order.insert(0, entry_id);
        self.rebuild_traversal_order(None);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
        self.traversal_order.clear();
        self.play_next_order.clear();
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        let mut rng = SmallRng::from_os_rng();
        self.set_shuffle_with_rng(enabled, &mut rng);
    }

    pub fn set_shuffle_with_seed(&mut self, enabled: bool, seed: u64) {
        let mut rng = SmallRng::seed_from_u64(seed);
        self.set_shuffle_with_rng(enabled, &mut rng);
    }

    pub fn set_shuffle_with_rng<R: RngCore>(&mut self, enabled: bool, rng: &mut R) {
        self.shuffle_enabled = enabled;
        self.rebuild_traversal_order(Some(rng));
    }

    pub fn next_index(&mut self, repeat_mode: RepeatMode) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }

        let current_position = self.current_index.and_then(|current| {
            self.traversal_order
                .iter()
                .position(|&index| index == current)
        });

        let next_index = match (current_position, repeat_mode) {
            (_, RepeatMode::One) => self
                .current_index
                .or_else(|| self.traversal_order.first().copied()),
            (None, _) => self.traversal_order.first().copied(),
            (Some(position), _) => self
                .traversal_order
                .get(position + 1)
                .copied()
                .or_else(|| (repeat_mode == RepeatMode::All).then(|| self.traversal_order[0])),
        };

        if let Some(index) = next_index {
            self.current_index = Some(index);
            self.play_next_order
                .retain(|entry_id| *entry_id != self.entries[index].id);
        }
        next_index
    }

    pub fn previous_index(&mut self) -> Option<usize> {
        self.previous_index_with_repeat(RepeatMode::Off)
    }

    pub fn previous_index_with_repeat(&mut self, repeat_mode: RepeatMode) -> Option<usize> {
        let current_position = self.current_index.and_then(|current| {
            self.traversal_order
                .iter()
                .position(|&index| index == current)
        })?;
        let previous = current_position
            .checked_sub(1)
            .map(|position| self.traversal_order[position])
            .or_else(|| {
                (repeat_mode == RepeatMode::All)
                    .then(|| self.traversal_order.last().copied())
                    .flatten()
            });
        if let Some(index) = previous {
            self.current_index = Some(index);
            self.play_next_order
                .retain(|entry_id| *entry_id != self.entries[index].id);
        }
        previous
    }

    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn current_entry(&self) -> Option<&QueueEntry> {
        self.current_index.and_then(|index| self.entries.get(index))
    }

    pub fn set_current_requested_source_id(&mut self, source_id: Option<SourceId>) -> bool {
        let Some(index) = self.current_index else {
            return false;
        };
        let Some(entry) = self.entries.get_mut(index) else {
            return false;
        };
        entry.requested_source_id = source_id;
        true
    }

    pub fn is_shuffle_enabled(&self) -> bool {
        self.shuffle_enabled
    }

    pub fn active_order(&self) -> Vec<QueueEntryId> {
        self.traversal_order
            .iter()
            .filter_map(|index| self.entries.get(*index).map(|entry| entry.id))
            .collect()
    }

    fn rebuild_traversal_order(&mut self, rng: Option<&mut dyn RngCore>) {
        let old_order = std::mem::take(&mut self.traversal_order);
        let current_index = self.current_index;
        let current_position =
            current_index.and_then(|current| old_order.iter().position(|&index| index == current));

        let mut ordered = if self.shuffle_enabled {
            let mut history = current_position
                .map(|position| old_order[..=position].to_vec())
                .unwrap_or_default();
            let mut upcoming = (0..self.entries.len())
                .filter(|index| !history.contains(index))
                .collect::<Vec<_>>();
            if let Some(rng) = rng {
                fisher_yates(&mut upcoming, rng);
            }
            history.extend(upcoming);
            history
        } else {
            (0..self.entries.len()).collect()
        };

        let prioritized_indices = self
            .play_next_order
            .iter()
            .filter_map(|entry_id| self.entries.iter().position(|entry| entry.id == *entry_id))
            .filter(|&index| Some(index) != current_index)
            .collect::<Vec<_>>();
        ordered.retain(|index| !prioritized_indices.contains(index));

        let insertion_position = current_index
            .and_then(|index| ordered.iter().position(|&item| item == index))
            .map(|position| position + 1)
            .unwrap_or(0);
        for (offset, index) in prioritized_indices.into_iter().enumerate() {
            ordered.insert(insertion_position + offset, index);
        }
        self.traversal_order = ordered;
    }
}

fn fisher_yates(values: &mut [usize], rng: &mut dyn RngCore) {
    for index in (1..values.len()).rev() {
        let upper = (index + 1) as u64;
        let zone = u64::MAX - u64::MAX % upper;
        let swap_index = loop {
            let value = rng.next_u64();
            if value < zone {
                break (value % upper) as usize;
            }
        };
        values.swap(index, swap_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(_order: u64) -> QueueEntry {
        QueueEntry::new(TrackId::new(), None)
    }

    #[test]
    fn insertion_order_is_retained_for_append_and_insert_next() {
        let first = entry(1);
        let second = entry(2);
        let third = entry(3);
        let fourth = entry(4);
        let mut queue = TransientQueue::new();

        queue.play_now(first.clone());
        queue.append(third.clone());
        queue.insert_next(second.clone());
        queue.append(fourth.clone());

        assert_eq!(
            queue
                .entries()
                .iter()
                .map(|entry| entry.track_id)
                .collect::<Vec<_>>(),
            vec![
                first.track_id,
                third.track_id,
                second.track_id,
                fourth.track_id
            ]
        );
        assert_eq!(queue.current_entry().map(|entry| entry.id), Some(first.id));
        assert_eq!(queue.next_index(RepeatMode::Off), Some(2));
        assert_eq!(queue.current_entry().map(|entry| entry.id), Some(second.id));
        assert_eq!(queue.next_index(RepeatMode::Off), Some(1));
    }

    #[test]
    fn play_next_without_current_is_first_in_active_order() {
        let first = entry(1);
        let second = entry(2);
        let mut queue = TransientQueue::new();
        queue.append(first.clone());
        queue.insert_next(second.clone());

        assert_eq!(queue.current_index(), None);
        assert_eq!(queue.next_index(RepeatMode::Off), Some(1));
        assert_eq!(queue.current_entry().map(|entry| entry.id), Some(second.id));
    }

    #[test]
    fn shuffle_retains_current_and_does_not_replay_consumed_history() {
        let entries = (1..=6).map(entry).collect::<Vec<_>>();
        let mut queue = TransientQueue::new();
        queue.play_now(entries[0].clone());
        for item in &entries[1..] {
            queue.append(item.clone());
        }
        assert_eq!(queue.next_index(RepeatMode::Off), Some(1));
        assert_eq!(queue.next_index(RepeatMode::Off), Some(2));
        let current = queue.current_entry().unwrap().id;

        queue.set_shuffle_with_seed(true, 7);

        assert_eq!(queue.current_entry().unwrap().id, current);
        let upcoming = queue.active_order();
        let current_position = upcoming.iter().position(|id| *id == current).unwrap();
        let history = &upcoming[..=current_position];
        assert!(history.contains(&entries[0].id));
        assert!(history.contains(&entries[1].id));
        assert!(history.contains(&entries[2].id));
        let next = queue.next_index(RepeatMode::Off).unwrap();
        assert_eq!(queue.entries()[next].id, upcoming[current_position + 1]);
        assert!(!history.contains(&queue.entries()[next].id));
    }

    #[test]
    fn disabling_shuffle_restores_canonical_traversal_and_current() {
        let entries = (1..=4).map(entry).collect::<Vec<_>>();
        let mut queue = TransientQueue::new();
        queue.play_now(entries[0].clone());
        for item in &entries[1..] {
            queue.append(item.clone());
        }
        queue.next_index(RepeatMode::Off);
        let current = queue.current_entry().unwrap().id;
        queue.set_shuffle_with_seed(true, 11);
        queue.set_shuffle(false);

        assert_eq!(queue.current_entry().unwrap().id, current);
        assert_eq!(
            queue.active_order(),
            entries.iter().map(|entry| entry.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn previous_repeat_all_wraps_to_the_last_entry() {
        let entries = (1..=3).map(entry).collect::<Vec<_>>();
        let mut queue = TransientQueue::new();
        queue.play_now(entries[0].clone());
        for item in &entries[1..] {
            queue.append(item.clone());
        }

        assert_eq!(queue.previous_index_with_repeat(RepeatMode::All), Some(2));
        assert_eq!(queue.current_entry().unwrap().id, entries[2].id);
    }

    #[test]
    fn repeat_modes_use_lowercase_wire_names() {
        assert_eq!(serde_json::to_string(&RepeatMode::Off).unwrap(), "\"off\"");
        assert_eq!(serde_json::to_string(&RepeatMode::One).unwrap(), "\"one\"");
        assert_eq!(serde_json::to_string(&RepeatMode::All).unwrap(), "\"all\"");
    }
}
