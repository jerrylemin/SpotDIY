use std::fmt;

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
    pub entry_id: QueueEntryId,
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
    pub insertion_order: u64,
}

impl QueueEntry {
    pub fn new(
        track_id: TrackId,
        requested_source_id: Option<SourceId>,
        insertion_order: u64,
    ) -> Self {
        Self {
            entry_id: QueueEntryId::new(),
            track_id,
            requested_source_id,
            insertion_order,
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
    next_insertion_order: u64,
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
        self.next_insertion_order = self.entries[0].insertion_order.saturating_add(1);
        self.rebuild_traversal_order();
    }

    pub fn append(&mut self, entry: QueueEntry) {
        self.next_insertion_order = self
            .next_insertion_order
            .max(entry.insertion_order.saturating_add(1));
        self.entries.push(entry);
        self.rebuild_traversal_order();
    }

    pub fn insert_next(&mut self, entry: QueueEntry) {
        self.next_insertion_order = self
            .next_insertion_order
            .max(entry.insertion_order.saturating_add(1));
        let entry_id = entry.entry_id;
        self.entries.push(entry);
        self.play_next_order.insert(0, entry_id);
        self.rebuild_traversal_order();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
        self.traversal_order.clear();
        self.play_next_order.clear();
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle_enabled = enabled;
        self.rebuild_traversal_order();
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
                .retain(|entry_id| *entry_id != self.entries[index].entry_id);
        }
        next_index
    }

    pub fn previous_index(&mut self) -> Option<usize> {
        let current_position = self.current_index.and_then(|current| {
            self.traversal_order
                .iter()
                .position(|&index| index == current)
        })?;
        let previous = current_position
            .checked_sub(1)
            .map(|position| self.traversal_order[position]);
        if let Some(index) = previous {
            self.current_index = Some(index);
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

    pub fn is_shuffle_enabled(&self) -> bool {
        self.shuffle_enabled
    }

    fn rebuild_traversal_order(&mut self) {
        self.traversal_order = (0..self.entries.len()).collect();
        if self.shuffle_enabled {
            self.traversal_order.sort_by(|&left, &right| {
                shuffle_key(self.entries[left].entry_id)
                    .cmp(&shuffle_key(self.entries[right].entry_id))
                    .then_with(|| {
                        self.entries[left]
                            .insertion_order
                            .cmp(&self.entries[right].insertion_order)
                    })
                    .then_with(|| left.cmp(&right))
            });
        }

        let current_index = self.current_index;
        let prioritized_indices = self
            .play_next_order
            .iter()
            .filter_map(|entry_id| {
                self.entries
                    .iter()
                    .position(|entry| entry.entry_id == *entry_id)
            })
            .filter(|&index| Some(index) != current_index)
            .collect::<Vec<_>>();
        self.traversal_order
            .retain(|index| !prioritized_indices.contains(index));

        let insertion_position = current_index
            .and_then(|index| self.traversal_order.iter().position(|&item| item == index))
            .map(|position| position + 1)
            .unwrap_or(0);
        for (offset, index) in prioritized_indices.into_iter().enumerate() {
            self.traversal_order
                .insert(insertion_position + offset, index);
        }
    }
}

fn shuffle_key(entry_id: QueueEntryId) -> [u8; 16] {
    entry_id.0.as_bytes().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(order: u64) -> QueueEntry {
        QueueEntry::new(TrackId::new(), None, order)
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
                .map(|entry| entry.insertion_order)
                .collect::<Vec<_>>(),
            vec![1, 3, 2, 4]
        );
        assert_eq!(
            queue.current_entry().map(|entry| entry.entry_id),
            Some(first.entry_id)
        );
        assert_eq!(queue.next_index(RepeatMode::Off), Some(2));
        assert_eq!(
            queue.current_entry().map(|entry| entry.entry_id),
            Some(second.entry_id)
        );
        assert_eq!(queue.next_index(RepeatMode::Off), Some(1));
        assert_eq!(
            queue.current_entry().map(|entry| entry.entry_id),
            Some(third.entry_id)
        );
    }

    #[test]
    fn shuffle_retains_current_entry_and_preserves_repeated_play_next() {
        let entries = [entry(1), entry(2), entry(3)];
        let appended = entry(4);
        let mut queue = TransientQueue::new();
        queue.play_now(entries[0].clone());
        queue.insert_next(entries[1].clone());
        queue.insert_next(entries[2].clone());
        queue.append(appended);
        let current_id = queue.current_entry().unwrap().entry_id;

        queue.set_shuffle(true);

        assert!(queue.is_shuffle_enabled());
        assert_eq!(queue.current_entry().unwrap().entry_id, current_id);
        assert_eq!(queue.next_index(RepeatMode::Off), Some(2));
        assert_eq!(queue.current_entry().unwrap().entry_id, entries[2].entry_id);
        assert_eq!(queue.next_index(RepeatMode::Off), Some(1));
        assert_eq!(queue.current_entry().unwrap().entry_id, entries[1].entry_id);
        while queue.next_index(RepeatMode::Off).is_some() {}
        assert_eq!(
            queue.next_index(RepeatMode::All),
            Some(queue.traversal_order[0])
        );
    }

    #[test]
    fn repeat_modes_use_lowercase_wire_names() {
        assert_eq!(serde_json::to_string(&RepeatMode::Off).unwrap(), "\"off\"");
        assert_eq!(serde_json::to_string(&RepeatMode::One).unwrap(), "\"one\"");
        assert_eq!(serde_json::to_string(&RepeatMode::All).unwrap(), "\"all\"");
    }
}
