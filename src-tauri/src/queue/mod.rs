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
    shuffle_enabled: bool,
    next_insertion_order: u64,
}

impl TransientQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play_now(&mut self, entry: QueueEntry) {
        self.entries.clear();
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
        self.entries.push(entry);
        self.rebuild_traversal_order();

        if let Some(current_index) = self.current_index {
            let inserted_index = self.entries.len() - 1;
            let current_position = self
                .traversal_order
                .iter()
                .position(|&index| index == current_index)
                .expect("current queue entry must be in traversal order");
            self.traversal_order.remove(
                self.traversal_order
                    .iter()
                    .position(|&index| index == inserted_index)
                    .expect("inserted queue entry must be in traversal order"),
            );
            self.traversal_order
                .insert(current_position + 1, inserted_index);
        } else {
            self.current_index = None;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
        self.traversal_order.clear();
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
        let mut queue = TransientQueue::new();

        queue.play_now(first.clone());
        queue.append(third.clone());
        queue.insert_next(second.clone());

        assert_eq!(
            queue
                .entries()
                .iter()
                .map(|entry| entry.insertion_order)
                .collect::<Vec<_>>(),
            vec![1, 3, 2]
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
    }

    #[test]
    fn shuffle_retains_current_entry_and_repeat_all_wraps() {
        let entries = [entry(1), entry(2), entry(3)];
        let mut queue = TransientQueue::new();
        queue.play_now(entries[0].clone());
        queue.append(entries[1].clone());
        queue.append(entries[2].clone());
        let current_id = queue.current_entry().unwrap().entry_id;

        queue.set_shuffle(true);

        assert!(queue.is_shuffle_enabled());
        assert_eq!(queue.current_entry().unwrap().entry_id, current_id);
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
