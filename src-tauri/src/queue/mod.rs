use chrono::Utc;
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

use crate::db::{Database, DatabaseError};
use crate::domain::{SourceId, TrackId};

pub const QUEUE_STATE_ID: i64 = 1;

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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueSection {
    UpNext,
    #[default]
    Later,
    Autoplay,
}

impl QueueSection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpNext => "up_next",
            Self::Later => "later",
            Self::Autoplay => "autoplay",
        }
    }

    pub(crate) fn priority(self) -> u8 {
        match self {
            Self::UpNext => 0,
            Self::Later => 1,
            Self::Autoplay => 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
    pub section: QueueSection,
    pub pinned: bool,
}

impl QueueEntry {
    pub fn new(track_id: TrackId, requested_source_id: Option<SourceId>) -> Self {
        Self {
            id: QueueEntryId::new(),
            track_id,
            requested_source_id,
            section: QueueSection::Later,
            pinned: false,
        }
    }

    pub fn with_section(mut self, section: QueueSection) -> Self {
        self.section = section;
        self
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum QueueMutationError {
    #[error("queue entry {0} was not found")]
    EntryNotFound(QueueEntryId),
    #[error("the current queue entry cannot be changed from the queue workspace")]
    CurrentEntry,
    #[error("a consumed queue entry cannot be changed from the queue workspace")]
    ConsumedEntry,
    #[error("queue target position {0} is invalid")]
    InvalidTarget(usize),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

impl RepeatMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::One => "one",
            Self::All => "all",
        }
    }
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
        self.entries.push(QueueEntry {
            section: QueueSection::Later,
            ..entry
        });
        self.rebuild_traversal_order(None);
    }

    pub fn insert_next(&mut self, entry: QueueEntry) {
        let entry_id = entry.id;
        self.entries.push(QueueEntry {
            section: QueueSection::UpNext,
            ..entry
        });
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

    pub fn entry(&self, entry_id: QueueEntryId) -> Option<&QueueEntry> {
        self.entries.iter().find(|entry| entry.id == entry_id)
    }

    pub fn traversal_order(&self) -> Vec<QueueEntryId> {
        self.traversal_order
            .iter()
            .filter_map(|index| self.entries.get(*index).map(|entry| entry.id))
            .collect()
    }

    pub fn history_order(&self) -> Vec<QueueEntryId> {
        let Some(current_index) = self.current_index else {
            return Vec::new();
        };
        let Some(current_position) = self
            .traversal_order
            .iter()
            .position(|index| *index == current_index)
        else {
            return Vec::new();
        };
        self.traversal_order[..=current_position]
            .iter()
            .filter_map(|index| self.entries.get(*index).map(|entry| entry.id))
            .collect()
    }

    pub fn upcoming_entries(&self, section: QueueSection) -> Vec<&QueueEntry> {
        let history = self.history_order();
        self.traversal_order
            .iter()
            .filter_map(|index| self.entries.get(*index))
            .filter(|entry| entry.section == section && !history.contains(&entry.id))
            .collect()
    }

    pub fn restore(
        &mut self,
        entries: Vec<QueueEntry>,
        current_entry_id: Option<QueueEntryId>,
        history_order: &[QueueEntryId],
        traversal_order: &[QueueEntryId],
        shuffle_enabled: bool,
    ) {
        self.entries = entries;
        self.shuffle_enabled = shuffle_enabled;
        self.play_next_order = self
            .entries
            .iter()
            .filter(|entry| entry.section == QueueSection::UpNext)
            .map(|entry| entry.id)
            .collect();
        self.current_index = current_entry_id.and_then(|id| self.index_of(id));

        let valid_ids = self
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let mut restored = Vec::new();
        for id in traversal_order.iter().chain(history_order) {
            if valid_ids.contains(id) && !restored.contains(id) {
                restored.push(*id);
            }
        }
        for entry in &self.entries {
            if !restored.contains(&entry.id) {
                restored.push(entry.id);
            }
        }
        self.traversal_order = restored
            .into_iter()
            .filter_map(|id| self.index_of(id))
            .collect();
        if self.traversal_order.is_empty() && !self.entries.is_empty() {
            self.rebuild_traversal_order(None);
        }
    }

    pub fn move_entry(
        &mut self,
        entry_id: QueueEntryId,
        section: QueueSection,
        target_index: usize,
    ) -> Result<(), QueueMutationError> {
        self.ensure_mutable(entry_id)?;
        let current_entry_id = self.current_entry().map(|entry| entry.id);
        let mut section_ids = self
            .upcoming_entries(section)
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == entry_id) else {
            return Err(QueueMutationError::EntryNotFound(entry_id));
        };
        let old_section = entry.section;
        if old_section == section {
            section_ids.retain(|id| *id != entry_id);
        }
        if target_index > section_ids.len() {
            return Err(QueueMutationError::InvalidTarget(target_index));
        }
        entry.section = section;
        section_ids.insert(target_index, entry_id);
        self.reorder_section(section, &section_ids);
        if old_section != section {
            let old_ids = self
                .upcoming_entries(old_section)
                .into_iter()
                .map(|entry| entry.id)
                .filter(|id| *id != entry_id)
                .collect::<Vec<_>>();
            self.reorder_section(old_section, &old_ids);
        }
        self.current_index = current_entry_id.and_then(|id| self.index_of(id));
        self.rebuild_traversal_order(None);
        Ok(())
    }

    pub fn remove_entry(
        &mut self,
        entry_id: QueueEntryId,
    ) -> Result<QueueEntry, QueueMutationError> {
        self.ensure_mutable(entry_id)?;
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == entry_id)
            .ok_or(QueueMutationError::EntryNotFound(entry_id))?;
        self.play_next_order.retain(|id| *id != entry_id);
        let removed = self.entries.remove(index);
        self.current_index = self.current_index.and_then(|current| {
            if current == index {
                None
            } else if current > index {
                Some(current - 1)
            } else {
                Some(current)
            }
        });
        self.rebuild_traversal_order(None);
        Ok(removed)
    }

    pub fn set_entry_pinned(
        &mut self,
        entry_id: QueueEntryId,
        pinned: bool,
    ) -> Result<(), QueueMutationError> {
        self.ensure_mutable(entry_id)?;
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or(QueueMutationError::EntryNotFound(entry_id))?;
        entry.pinned = pinned;
        Ok(())
    }

    pub fn clear_section(&mut self, section: QueueSection) -> Vec<QueueEntryId> {
        let history = self.history_order();
        let removed_ids = self
            .entries
            .iter()
            .filter(|entry| {
                entry.section == section && !entry.pinned && !history.contains(&entry.id)
            })
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        if removed_ids.is_empty() {
            return removed_ids;
        }
        let current_entry_id = self.current_entry().map(|entry| entry.id);
        self.entries
            .retain(|entry| !removed_ids.contains(&entry.id));
        self.play_next_order.retain(|id| !removed_ids.contains(id));
        self.current_index = current_entry_id.and_then(|id| self.index_of(id));
        self.rebuild_traversal_order(None);
        removed_ids
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

    fn index_of(&self, entry_id: QueueEntryId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == entry_id)
    }

    fn ensure_mutable(&self, entry_id: QueueEntryId) -> Result<(), QueueMutationError> {
        let index = self
            .index_of(entry_id)
            .ok_or(QueueMutationError::EntryNotFound(entry_id))?;
        if self.current_index == Some(index) {
            return Err(QueueMutationError::CurrentEntry);
        }
        if self.history_order().contains(&entry_id) {
            return Err(QueueMutationError::ConsumedEntry);
        }
        Ok(())
    }

    fn reorder_section(&mut self, section: QueueSection, ids: &[QueueEntryId]) {
        let history = self.history_order();
        let mut ordered_ids = Vec::new();
        for id in history.into_iter().chain(ids.iter().copied()).chain(
            self.entries
                .iter()
                .filter(|entry| entry.section == section)
                .map(|entry| entry.id),
        ) {
            if self.entry(id).is_some_and(|entry| entry.section == section)
                && !ordered_ids.contains(&id)
            {
                ordered_ids.push(id);
            }
        }
        let replacements = ordered_ids
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| entry.id == *id).cloned())
            .collect::<Vec<_>>();
        let mut next = replacements.into_iter();
        for entry in self
            .entries
            .iter_mut()
            .filter(|entry| entry.section == section)
        {
            if let Some(replacement) = next.next() {
                *entry = replacement;
            }
        }
        if section == QueueSection::UpNext {
            self.play_next_order = ordered_ids;
        }
    }

    fn rebuild_traversal_order(&mut self, rng: Option<&mut dyn RngCore>) {
        let old_order = std::mem::take(&mut self.traversal_order);
        let current_index = self.current_index;
        let current_position =
            current_index.and_then(|current| old_order.iter().position(|&index| index == current));

        let history = current_position
            .map(|position| old_order[..=position].to_vec())
            .unwrap_or_default();
        let history_set = history
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut up_next = self.section_indices(QueueSection::UpNext);
        up_next.retain(|index| !history_set.contains(index));
        let mut later = self.section_indices(QueueSection::Later);
        later.retain(|index| !history_set.contains(index));
        let mut autoplay = self.section_indices(QueueSection::Autoplay);
        autoplay.retain(|index| !history_set.contains(index));
        if self.shuffle_enabled {
            if let Some(rng) = rng {
                fisher_yates(&mut later, rng);
            } else {
                let canonical = later.clone();
                let mut preserved = old_order
                    .iter()
                    .copied()
                    .filter(|index| canonical.contains(index))
                    .collect::<Vec<_>>();
                preserved.dedup();
                for index in canonical {
                    if !preserved.contains(&index) {
                        preserved.push(index);
                    }
                }
                later = preserved;
            }
        }
        let mut ordered = history;
        ordered.extend(up_next);
        ordered.extend(later);
        ordered.extend(autoplay);
        self.traversal_order = ordered;
    }

    fn section_indices(&self, section: QueueSection) -> Vec<usize> {
        if section == QueueSection::UpNext {
            let mut indices = self
                .play_next_order
                .iter()
                .filter_map(|entry_id| self.index_of(*entry_id))
                .collect::<Vec<_>>();
            for (index, entry) in self.entries.iter().enumerate() {
                if entry.section == section && !indices.contains(&index) {
                    indices.push(index);
                }
            }
            return indices;
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.section == section)
            .map(|(index, _)| index)
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueWorkspaceEntry {
    pub id: QueueEntryId,
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
    pub section: QueueSection,
    pub position: u32,
    pub pinned: bool,
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueWorkspace {
    pub revision: u64,
    pub current: Option<QueueWorkspaceEntry>,
    pub up_next: Vec<QueueWorkspaceEntry>,
    pub later: Vec<QueueWorkspaceEntry>,
    pub autoplay: Vec<QueueWorkspaceEntry>,
    pub current_position_ms: u64,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshotEntry {
    pub id: crate::domain::QueueSnapshotEntryId,
    pub snapshot_id: crate::domain::QueueSnapshotId,
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
    pub section: QueueSection,
    pub position: u32,
    pub pinned: bool,
    pub traversal_position: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub id: crate::domain::QueueSnapshotId,
    pub name: String,
    pub current_track_id: Option<TrackId>,
    pub current_source_id: Option<SourceId>,
    pub current_position_ms: u64,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub current_snapshot_entry_id: Option<crate::domain::QueueSnapshotEntryId>,
    pub history_order: Vec<crate::domain::QueueSnapshotEntryId>,
    pub traversal_order: Vec<crate::domain::QueueSnapshotEntryId>,
    pub entries: Vec<QueueSnapshotEntry>,
    pub entry_count: usize,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshotSummary {
    pub id: crate::domain::QueueSnapshotId,
    pub name: String,
    pub current_track_id: Option<TrackId>,
    pub current_source_id: Option<SourceId>,
    pub current_position_ms: u64,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub entry_count: usize,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct PersistedQueue {
    pub entries: Vec<QueueEntry>,
    pub current_entry_id: Option<QueueEntryId>,
    pub current_position_ms: u64,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub history_order: Vec<QueueEntryId>,
    pub traversal_order: Vec<QueueEntryId>,
    pub revision: u64,
}

impl Default for PersistedQueue {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current_entry_id: None,
            current_position_ms: 0,
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            history_order: Vec::new(),
            traversal_order: Vec::new(),
            revision: 0,
        }
    }
}

#[derive(Debug, Error)]
pub enum QueueRepositoryError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not serialize queue state: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("invalid queue value for {field}: {value}")]
    InvalidValue { field: &'static str, value: String },
    #[error("queue snapshot name must contain 1..80 characters")]
    InvalidSnapshotName,
}

#[derive(Clone)]
pub struct QueueRepository {
    database: Database,
}

impl QueueRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn load(&self) -> Result<PersistedQueue, QueueRepositoryError> {
        let connection = self.database.connection()?;
        let Some((
            current_entry_id,
            current_position_ms,
            repeat_mode,
            shuffle_enabled,
            history,
            traversal,
            revision,
        )) = connection
            .query_row(
                "SELECT current_entry_id, current_position_ms, repeat_mode, shuffle_enabled,
                            history_order_json, shuffle_order_json, revision
                     FROM queue_state WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?
        else {
            return Ok(PersistedQueue::default());
        };

        let mut statement = connection.prepare(
            "SELECT id, track_id, requested_source_id, section, position, pinned
             FROM queue_entries ORDER BY section, position, id",
        )?;
        let rows = statement.query_map([], |row| {
            let id = parse_queue_id(&row.get::<_, String>(0)?, "queue_entries.id")?;
            let track_id = parse_domain_id(&row.get::<_, String>(1)?, "queue_entries.track_id")?;
            let requested_source_id = row
                .get::<_, Option<String>>(2)?
                .map(|value| parse_domain_id(&value, "queue_entries.requested_source_id"))
                .transpose()?;
            let section = parse_queue_section(&row.get::<_, String>(3)?)?;
            let _position = parse_u32(row.get(4)?, "queue_entries.position")?;
            let pinned = row.get::<_, i64>(5)? != 0;
            Ok(QueueEntry {
                id,
                track_id,
                requested_source_id,
                section,
                pinned,
            })
        })?;
        let entries = rows.collect::<Result<Vec<_>, _>>()?;
        let current_entry_id = current_entry_id
            .map(|value| parse_queue_id(&value, "queue_state.current_entry_id"))
            .transpose()?;
        let current_entry_id =
            current_entry_id.filter(|id| entries.iter().any(|entry| entry.id == *id));
        Ok(PersistedQueue {
            entries,
            current_entry_id,
            current_position_ms: parse_u64(current_position_ms, "queue_state.current_position_ms")?,
            repeat_mode: parse_repeat_mode(&repeat_mode)?,
            shuffle_enabled: shuffle_enabled != 0,
            history_order: parse_json_ids(&history, "queue_state.history_order_json")?,
            traversal_order: parse_json_ids(&traversal, "queue_state.shuffle_order_json")?,
            revision: parse_u64(revision, "queue_state.revision")?,
        })
    }

    pub fn replace(&self, state: &PersistedQueue) -> Result<(), QueueRepositoryError> {
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE queue_state SET current_entry_id = NULL WHERE id = 1",
            [],
        )?;
        transaction.execute("DELETE FROM queue_entries", [])?;
        let mut positions = [0_u32; 3];
        let now = Utc::now().to_rfc3339();
        for entry in &state.entries {
            let section_index = usize::from(entry.section.priority());
            let position = positions[section_index];
            positions[section_index] = positions[section_index].saturating_add(1);
            transaction.execute(
                "INSERT INTO queue_entries (
                    id, track_id, requested_source_id, section, position, pinned,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    entry.id.to_string(),
                    entry.track_id.to_string(),
                    entry.requested_source_id.map(|id| id.to_string()),
                    entry.section.as_str(),
                    i64::from(position),
                    i64::from(entry.pinned),
                    now,
                ],
            )?;
        }
        let history_json = serde_json::to_string(&state.history_order)?;
        let traversal_json = serde_json::to_string(&state.traversal_order)?;
        transaction.execute(
            "UPDATE queue_state
             SET current_entry_id = ?1, current_position_ms = ?2, repeat_mode = ?3,
                 shuffle_enabled = ?4, history_order_json = ?5, shuffle_order_json = ?6,
                 revision = ?7, updated_at = ?8
             WHERE id = 1",
            params![
                state.current_entry_id.map(|id| id.to_string()),
                i64::try_from(state.current_position_ms).map_err(|_| {
                    QueueRepositoryError::InvalidValue {
                        field: "queue_state.current_position_ms",
                        value: state.current_position_ms.to_string(),
                    }
                })?,
                state.repeat_mode.as_str(),
                i64::from(state.shuffle_enabled),
                history_json,
                traversal_json,
                i64::try_from(state.revision).map_err(|_| QueueRepositoryError::InvalidValue {
                    field: "queue_state.revision",
                    value: state.revision.to_string(),
                })?,
                now,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn save_position(
        &self,
        current_entry_id: Option<QueueEntryId>,
        position_ms: u64,
        revision: u64,
    ) -> Result<(), QueueRepositoryError> {
        let connection = self.database.connection()?;
        connection.execute(
            "UPDATE queue_state
             SET current_entry_id = ?1, current_position_ms = ?2, revision = ?3, updated_at = ?4
             WHERE id = 1",
            params![
                current_entry_id.map(|id| id.to_string()),
                i64::try_from(position_ms).map_err(|_| QueueRepositoryError::InvalidValue {
                    field: "queue_state.current_position_ms",
                    value: position_ms.to_string(),
                })?,
                i64::try_from(revision).map_err(|_| QueueRepositoryError::InvalidValue {
                    field: "queue_state.revision",
                    value: revision.to_string(),
                })?,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn save_snapshot(
        &self,
        name: impl Into<String>,
        state: &PersistedQueue,
    ) -> Result<QueueSnapshot, QueueRepositoryError> {
        let name = name.into().trim().to_owned();
        if !(1..=80).contains(&name.chars().count()) {
            return Err(QueueRepositoryError::InvalidSnapshotName);
        }
        let snapshot_id = crate::domain::QueueSnapshotId::new();
        let now = Utc::now();
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let snapshot_entry_ids = state
            .entries
            .iter()
            .map(|entry| (entry.id, crate::domain::QueueSnapshotEntryId::new()))
            .collect::<std::collections::HashMap<_, _>>();
        let current_snapshot_entry_id = state
            .current_entry_id
            .and_then(|id| snapshot_entry_ids.get(&id).copied());
        let history_order = state
            .history_order
            .iter()
            .filter_map(|id| snapshot_entry_ids.get(id).copied())
            .collect::<Vec<_>>();
        let traversal_order = state
            .traversal_order
            .iter()
            .filter_map(|id| snapshot_entry_ids.get(id).copied())
            .collect::<Vec<_>>();
        let current = state
            .current_entry_id
            .and_then(|id| state.entries.iter().find(|entry| entry.id == id));
        transaction.execute(
            "INSERT INTO queue_snapshots (
                id, name, current_track_id, current_source_id, current_position_ms,
                repeat_mode, shuffle_enabled, current_snapshot_entry_id,
                history_order_json, shuffle_order_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)",
            params![
                snapshot_id.to_string(),
                name,
                current.map(|entry| entry.track_id.to_string()),
                current.and_then(|entry| entry.requested_source_id.map(|id| id.to_string())),
                i64::try_from(state.current_position_ms).map_err(|_| {
                    QueueRepositoryError::InvalidValue {
                        field: "queue_snapshots.current_position_ms",
                        value: state.current_position_ms.to_string(),
                    }
                })?,
                state.repeat_mode.as_str(),
                i64::from(state.shuffle_enabled),
                serde_json::to_string(&history_order)?,
                serde_json::to_string(&traversal_order)?,
                now.to_rfc3339(),
            ],
        )?;
        let mut positions = [0_u32; 3];
        for entry in &state.entries {
            let snapshot_entry_id = snapshot_entry_ids[&entry.id];
            let section_index = usize::from(entry.section.priority());
            let position = positions[section_index];
            positions[section_index] = positions[section_index].saturating_add(1);
            let traversal_position = state
                .traversal_order
                .iter()
                .position(|id| *id == entry.id)
                .unwrap_or(state.traversal_order.len());
            transaction.execute(
                "INSERT INTO queue_snapshot_entries (
                    id, snapshot_id, track_id, requested_source_id, section,
                    position, pinned, traversal_position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    snapshot_entry_id.to_string(),
                    snapshot_id.to_string(),
                    entry.track_id.to_string(),
                    entry.requested_source_id.map(|id| id.to_string()),
                    entry.section.as_str(),
                    i64::from(position),
                    i64::from(entry.pinned),
                    i64::try_from(traversal_position)
                        .expect("queue traversal position fits SQLite"),
                ],
            )?;
        }
        if let Some(current_snapshot_entry_id) = current_snapshot_entry_id {
            transaction.execute(
                "UPDATE queue_snapshots SET current_snapshot_entry_id = ?1 WHERE id = ?2",
                params![
                    current_snapshot_entry_id.to_string(),
                    snapshot_id.to_string()
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.load_snapshot(snapshot_id)
    }

    pub fn list_snapshots(&self) -> Result<Vec<QueueSnapshotSummary>, QueueRepositoryError> {
        let connection = self.database.connection()?;
        let mut statement = connection.prepare(
            "SELECT s.id, s.name, s.current_track_id, s.current_source_id,
                    s.current_position_ms, s.repeat_mode, s.shuffle_enabled,
                    s.created_at, COUNT(e.id)
             FROM queue_snapshots s
             LEFT JOIN queue_snapshot_entries e ON e.snapshot_id = s.id
             GROUP BY s.id
             ORDER BY s.created_at DESC, s.id",
        )?;
        let rows = statement.query_map([], map_snapshot_summary_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(QueueRepositoryError::from)
    }

    pub fn load_snapshot(
        &self,
        snapshot_id: crate::domain::QueueSnapshotId,
    ) -> Result<QueueSnapshot, QueueRepositoryError> {
        let connection = self.database.connection()?;
        let header = connection
            .query_row(
                "SELECT id, name, current_track_id, current_source_id, current_position_ms,
                        repeat_mode, shuffle_enabled, current_snapshot_entry_id,
                        history_order_json, shuffle_order_json, created_at
                 FROM queue_snapshots WHERE id = ?1",
                params![snapshot_id.to_string()],
                map_snapshot_header_row,
            )
            .optional()?
            .ok_or_else(|| QueueRepositoryError::InvalidValue {
                field: "queue_snapshots.id",
                value: snapshot_id.to_string(),
            })?;
        let mut statement = connection.prepare(
            "SELECT id, snapshot_id, track_id, requested_source_id, section,
                    position, pinned, traversal_position
             FROM queue_snapshot_entries WHERE snapshot_id = ?1
             ORDER BY traversal_position, position, id",
        )?;
        let rows = statement.query_map(params![snapshot_id.to_string()], map_snapshot_entry_row)?;
        let entries = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(QueueSnapshot {
            id: header.id,
            name: header.name,
            current_track_id: header.current_track_id,
            current_source_id: header.current_source_id,
            current_position_ms: header.current_position_ms,
            repeat_mode: header.repeat_mode,
            shuffle_enabled: header.shuffle_enabled,
            current_snapshot_entry_id: header.current_snapshot_entry_id,
            history_order: header.history_order,
            traversal_order: header.traversal_order,
            entry_count: entries.len(),
            entries,
            created_at: header.created_at,
        })
    }

    pub fn delete_snapshot(
        &self,
        snapshot_id: crate::domain::QueueSnapshotId,
    ) -> Result<(), QueueRepositoryError> {
        let connection = self.database.connection()?;
        let deleted = connection.execute(
            "DELETE FROM queue_snapshots WHERE id = ?1",
            params![snapshot_id.to_string()],
        )?;
        if deleted == 0 {
            return Err(QueueRepositoryError::InvalidValue {
                field: "queue_snapshots.id",
                value: snapshot_id.to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct QueueSnapshotHeader {
    id: crate::domain::QueueSnapshotId,
    name: String,
    current_track_id: Option<TrackId>,
    current_source_id: Option<SourceId>,
    current_position_ms: u64,
    repeat_mode: RepeatMode,
    shuffle_enabled: bool,
    current_snapshot_entry_id: Option<crate::domain::QueueSnapshotEntryId>,
    history_order: Vec<crate::domain::QueueSnapshotEntryId>,
    traversal_order: Vec<crate::domain::QueueSnapshotEntryId>,
    created_at: chrono::DateTime<Utc>,
}

fn map_snapshot_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueueSnapshotSummary> {
    Ok(QueueSnapshotSummary {
        id: parse_snapshot_id(&row.get::<_, String>(0)?, "queue_snapshots.id")?,
        name: row.get(1)?,
        current_track_id: row
            .get::<_, Option<String>>(2)?
            .map(|value| parse_domain_id(&value, "queue_snapshots.current_track_id"))
            .transpose()?,
        current_source_id: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_domain_id(&value, "queue_snapshots.current_source_id"))
            .transpose()?,
        current_position_ms: parse_u64(row.get(4)?, "queue_snapshots.current_position_ms")?,
        repeat_mode: parse_repeat_mode(&row.get::<_, String>(5)?)?,
        shuffle_enabled: row.get::<_, i64>(6)? != 0,
        created_at: parse_timestamp(&row.get::<_, String>(7)?, "queue_snapshots.created_at")?,
        entry_count: usize::try_from(row.get::<_, i64>(8)?).map_err(|_| {
            rusqlite::Error::IntegralValueOutOfRange(8, row.get::<_, i64>(8).unwrap_or_default())
        })?,
    })
}

fn map_snapshot_header_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueueSnapshotHeader> {
    Ok(QueueSnapshotHeader {
        id: parse_snapshot_id(&row.get::<_, String>(0)?, "queue_snapshots.id")?,
        name: row.get(1)?,
        current_track_id: row
            .get::<_, Option<String>>(2)?
            .map(|value| parse_domain_id(&value, "queue_snapshots.current_track_id"))
            .transpose()?,
        current_source_id: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_domain_id(&value, "queue_snapshots.current_source_id"))
            .transpose()?,
        current_position_ms: parse_u64(row.get(4)?, "queue_snapshots.current_position_ms")?,
        repeat_mode: parse_repeat_mode(&row.get::<_, String>(5)?)?,
        shuffle_enabled: row.get::<_, i64>(6)? != 0,
        current_snapshot_entry_id: row
            .get::<_, Option<String>>(7)?
            .map(|value| {
                parse_snapshot_entry_id(&value, "queue_snapshots.current_snapshot_entry_id")
            })
            .transpose()?,
        history_order: parse_json_ids(
            &row.get::<_, String>(8)?,
            "queue_snapshots.history_order_json",
        )?,
        traversal_order: parse_json_ids(
            &row.get::<_, String>(9)?,
            "queue_snapshots.shuffle_order_json",
        )?,
        created_at: parse_timestamp(&row.get::<_, String>(10)?, "queue_snapshots.created_at")?,
    })
}

fn map_snapshot_entry_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueueSnapshotEntry> {
    Ok(QueueSnapshotEntry {
        id: parse_snapshot_entry_id(&row.get::<_, String>(0)?, "queue_snapshot_entries.id")?,
        snapshot_id: parse_snapshot_id(
            &row.get::<_, String>(1)?,
            "queue_snapshot_entries.snapshot_id",
        )?,
        track_id: parse_domain_id(&row.get::<_, String>(2)?, "queue_snapshot_entries.track_id")?,
        requested_source_id: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_domain_id(&value, "queue_snapshot_entries.requested_source_id"))
            .transpose()?,
        section: parse_queue_section(&row.get::<_, String>(4)?)?,
        position: parse_u32(row.get(5)?, "queue_snapshot_entries.position")?,
        pinned: row.get::<_, i64>(6)? != 0,
        traversal_position: parse_u32(row.get(7)?, "queue_snapshot_entries.traversal_position")?,
    })
}

fn parse_queue_section(value: &str) -> rusqlite::Result<QueueSection> {
    match value {
        "up_next" => Ok(QueueSection::UpNext),
        "later" => Ok(QueueSection::Later),
        "autoplay" => Ok(QueueSection::Autoplay),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "section".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn parse_repeat_mode(value: &str) -> rusqlite::Result<RepeatMode> {
    match value {
        "off" => Ok(RepeatMode::Off),
        "one" => Ok(RepeatMode::One),
        "all" => Ok(RepeatMode::All),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "repeat_mode".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn parse_queue_id(value: &str, field: &str) -> rusqlite::Result<QueueEntryId> {
    value
        .parse::<Uuid>()
        .map(QueueEntryId::from_uuid)
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
        })
}

fn parse_snapshot_id(value: &str, field: &str) -> rusqlite::Result<crate::domain::QueueSnapshotId> {
    value.parse().map_err(|_| {
        rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
    })
}

fn parse_snapshot_entry_id(
    value: &str,
    field: &str,
) -> rusqlite::Result<crate::domain::QueueSnapshotEntryId> {
    value.parse().map_err(|_| {
        rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
    })
}

fn parse_domain_id<T>(value: &str, field: &str) -> rusqlite::Result<T>
where
    T: std::str::FromStr,
{
    value.parse().map_err(|_| {
        rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
    })
}

fn parse_u32(value: i64, _field: &'static str) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, value))
}

fn parse_u64(value: i64, _field: &'static str) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, value))
}

fn parse_json_ids<T>(value: &str, _field: &'static str) -> rusqlite::Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn parse_timestamp(value: &str, field: &'static str) -> rusqlite::Result<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
        })
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
    use std::path::PathBuf;

    use super::*;
    use crate::db::repository::TrackRepository;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{SourceCapabilities, TrackSource, UnifiedTrack, VersionInfo};

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

    #[test]
    fn queue_sections_keep_up_next_ahead_of_later_and_protect_history() {
        let first = entry(1);
        let later = entry(2);
        let pinned = entry(3);
        let next = entry(4);
        let mut queue = TransientQueue::new();
        queue.play_now(first.clone());
        queue.append(later.clone());
        queue.append(pinned.clone());
        queue.set_entry_pinned(pinned.id, true).unwrap();
        queue.insert_next(next.clone());

        assert_eq!(
            queue.active_order(),
            vec![first.id, next.id, later.id, pinned.id]
        );
        assert_eq!(
            queue
                .upcoming_entries(QueueSection::UpNext)
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            vec![next.id]
        );
        assert_eq!(queue.clear_section(QueueSection::Later), vec![later.id]);
        assert!(queue.entry(pinned.id).is_some());
        assert_eq!(
            queue.move_entry(first.id, QueueSection::Later, 0),
            Err(QueueMutationError::CurrentEntry)
        );
        queue.next_index(RepeatMode::Off);
        assert_eq!(queue.current_entry().map(|entry| entry.id), Some(next.id));
        assert_eq!(
            queue.move_entry(first.id, QueueSection::Later, 0),
            Err(QueueMutationError::ConsumedEntry)
        );
    }

    #[test]
    fn moving_an_upcoming_entry_across_sections_keeps_current_and_priority_order() {
        let current = entry(1);
        let later = entry(2);
        let moved = entry(3);
        let next = entry(4);
        let mut queue = TransientQueue::new();
        queue.play_now(current.clone());
        queue.append(later.clone());
        queue.append(moved.clone());
        queue.insert_next(next.clone());

        queue.move_entry(moved.id, QueueSection::UpNext, 0).unwrap();

        assert_eq!(
            queue.current_entry().map(|entry| entry.id),
            Some(current.id)
        );
        assert_eq!(
            queue.active_order(),
            vec![current.id, moved.id, next.id, later.id]
        );
        assert_eq!(
            queue
                .upcoming_entries(QueueSection::UpNext)
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            vec![moved.id, next.id]
        );
    }

    #[test]
    fn shuffle_preserves_existing_later_order_when_queue_grows() {
        let entries = (1..=5).map(entry).collect::<Vec<_>>();
        let mut queue = TransientQueue::new();
        queue.play_now(entries[0].clone());
        for item in &entries[1..4] {
            queue.append(item.clone());
        }
        queue.set_shuffle_with_seed(true, 91);
        let before = queue.active_order();

        queue.append(entries[4].clone());
        let after = queue.active_order();
        let existing_after = after
            .iter()
            .copied()
            .filter(|id| before.contains(id))
            .collect::<Vec<_>>();

        assert_eq!(
            queue.current_entry().map(|entry| entry.id),
            Some(entries[0].id)
        );
        assert_eq!(existing_after, before);
        assert_eq!(after.last(), Some(&entries[4].id));
    }

    #[test]
    fn queue_repository_round_trips_state_and_creates_immutable_snapshot_entries() {
        let (_path, database, track_id, source_id) = fixture_database();
        let current = QueueEntry::new(track_id, Some(source_id));
        let next = QueueEntry::new(track_id, None).with_section(QueueSection::UpNext);
        let later = QueueEntry::new(track_id, Some(source_id));
        let state = PersistedQueue {
            entries: vec![current.clone(), next.clone(), later.clone()],
            current_entry_id: Some(current.id),
            current_position_ms: 42_000,
            repeat_mode: RepeatMode::All,
            shuffle_enabled: true,
            history_order: vec![current.id],
            traversal_order: vec![current.id, next.id, later.id],
            revision: 12,
        };
        let repository = QueueRepository::new(database);

        repository.replace(&state).unwrap();
        let loaded = repository.load().unwrap();
        assert_eq!(loaded.current_entry_id, state.current_entry_id);
        assert_eq!(loaded.current_position_ms, state.current_position_ms);
        assert_eq!(loaded.repeat_mode, state.repeat_mode);
        assert_eq!(loaded.shuffle_enabled, state.shuffle_enabled);
        assert_eq!(loaded.history_order, state.history_order);
        assert_eq!(loaded.traversal_order, state.traversal_order);
        assert_eq!(loaded.entries.len(), 3);
        assert!(loaded
            .entries
            .iter()
            .any(|entry| entry.id == next.id && entry.section == QueueSection::UpNext));

        let snapshot = repository.save_snapshot("Evening set", &state).unwrap();
        assert_eq!(snapshot.name, "Evening set");
        assert_eq!(snapshot.entries.len(), state.entries.len());
        assert_eq!(snapshot.entry_count, state.entries.len());
        assert!(snapshot.current_snapshot_entry_id.is_some());
        assert_ne!(snapshot.entries[0].id.to_string(), current.id.to_string());
        assert_eq!(repository.list_snapshots().unwrap().len(), 1);
        repository.delete_snapshot(snapshot.id).unwrap();
        assert!(repository.list_snapshots().unwrap().is_empty());
        assert!(matches!(
            repository.save_snapshot("", &state),
            Err(QueueRepositoryError::InvalidSnapshotName)
        ));
    }

    fn fixture_database() -> (TempDatabasePath, Database, TrackId, SourceId) {
        let path = TempDatabasePath::new("queue-repository");
        let database = Database::open(path.path()).unwrap();
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        let source = TrackSource::new_local(
            source_id,
            track_id,
            PathBuf::from(format!("C:\\Music\\queue-{track_id}.wav")),
            SourceCapabilities {
                playback: true,
                metadata: true,
                ..SourceCapabilities::default()
            },
        )
        .unwrap();
        let track = UnifiedTrack::new(
            track_id,
            "Queue track",
            vec![],
            None,
            Some(180_000),
            VersionInfo::standard(),
            vec![source],
        )
        .unwrap();
        TrackRepository::new(&database).create(&track).unwrap();
        (path, database, track_id, source_id)
    }
}
