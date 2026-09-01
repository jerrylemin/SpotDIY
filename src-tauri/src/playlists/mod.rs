use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{Database, DatabaseError};
use crate::domain::{PlaylistId, PlaylistItemId, SourceId, TagId, TrackId};

pub const INBOX_ID: PlaylistId =
    PlaylistId::from_uuid(uuid::uuid!("00000000-0000-0000-0000-000000000001"));
pub const INBOX_NAME: &str = "Inbox";
const MAX_PLAYLIST_NAME_SCALARS: usize = 120;
const MAX_TAG_NAME_SCALARS: usize = 64;
const MAX_COLLECTION_TRACKS: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaylistKind {
    Normal,
    Inbox,
    Branch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BranchStatus {
    Open,
    Merged,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    pub kind: PlaylistKind,
    pub parent_playlist_id: Option<PlaylistId>,
    pub base_parent_revision: Option<u64>,
    pub branch_status: Option<BranchStatus>,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: Vec<PlaylistItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    pub id: PlaylistItemId,
    pub playlist_id: PlaylistId,
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
    pub position: u32,
    pub origin_base_item_id: Option<PlaylistItemId>,
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub normalized_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistMembership {
    pub playlist_id: PlaylistId,
    pub name: String,
    pub kind: PlaylistKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCollectionState {
    pub track_id: TrackId,
    pub liked: bool,
    pub rating: Option<u8>,
    pub tags: Vec<Tag>,
    pub playlist_memberships: Vec<PlaylistMembership>,
    pub in_inbox: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BranchChange {
    Add {
        branch_item_id: PlaylistItemId,
    },
    Remove {
        base_item_id: PlaylistItemId,
    },
    Move {
        base_item_id: PlaylistItemId,
        target_position: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchMergeResult {
    pub parent: Playlist,
    pub branch: Playlist,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaylistErrorCode {
    InvalidName,
    InvalidTagName,
    PlaylistNotFound,
    PlaylistItemNotFound,
    TrackNotFound,
    SourceNotFound,
    SourceMismatch,
    SystemPlaylist,
    BranchExists,
    CannotBranch,
    BranchNotFound,
    BranchNotOpen,
    BranchAlreadyMerged,
    BranchConflict,
    InvalidBranchChange,
    EmptySelection,
    InvalidPosition,
    TagNotFound,
    TagExists,
    InvalidRating,
    CollectionRequestTooLarge,
    SnapshotNotFound,
    Database,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistErrorDto {
    pub code: PlaylistErrorCode,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum PlaylistError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid playlist name: {0}")]
    InvalidName(String),
    #[error("invalid tag name: {0}")]
    InvalidTagName(String),
    #[error("playlist {0} was not found")]
    PlaylistNotFound(PlaylistId),
    #[error("playlist item {0} was not found")]
    PlaylistItemNotFound(PlaylistItemId),
    #[error("track {0} was not found")]
    TrackNotFound(TrackId),
    #[error("source {source_id} was not found for track {track_id}")]
    SourceNotFound {
        track_id: TrackId,
        source_id: SourceId,
    },
    #[error("source {source_id} belongs to track {actual_track_id}, not {expected_track_id}")]
    SourceMismatch {
        source_id: SourceId,
        actual_track_id: TrackId,
        expected_track_id: TrackId,
    },
    #[error("Inbox is a system playlist and cannot be changed that way")]
    SystemPlaylist,
    #[error("playlist {0} has open branches")]
    BranchExists(PlaylistId),
    #[error("only a normal playlist can be branched")]
    CannotBranch,
    #[error("branch {0} was not found")]
    BranchNotFound(PlaylistId),
    #[error("branch {0} is not open")]
    BranchNotOpen(PlaylistId),
    #[error("branch {0} was already merged")]
    BranchAlreadyMerged(PlaylistId),
    #[error("branch parent changed from revision {base_revision} to {parent_revision}")]
    BranchConflict {
        base_revision: u64,
        parent_revision: u64,
    },
    #[error("invalid branch change: {0}")]
    InvalidBranchChange(String),
    #[error("at least one branch change must be selected")]
    EmptySelection,
    #[error("playlist position {0} is invalid")]
    InvalidPosition(u32),
    #[error("tag {0} was not found")]
    TagNotFound(TagId),
    #[error("a tag named {0} already exists")]
    TagExists(String),
    #[error("rating must be between 1 and 5")]
    InvalidRating,
    #[error("collection state requests are limited to {MAX_COLLECTION_TRACKS} tracks")]
    CollectionRequestTooLarge,
    #[error("queue snapshot {0} was not found")]
    SnapshotNotFound(String),
}

impl PlaylistError {
    pub fn code(&self) -> PlaylistErrorCode {
        match self {
            Self::Database(_) | Self::Sqlite(_) => PlaylistErrorCode::Database,
            Self::InvalidName(_) => PlaylistErrorCode::InvalidName,
            Self::InvalidTagName(_) => PlaylistErrorCode::InvalidTagName,
            Self::PlaylistNotFound(_) => PlaylistErrorCode::PlaylistNotFound,
            Self::PlaylistItemNotFound(_) => PlaylistErrorCode::PlaylistItemNotFound,
            Self::TrackNotFound(_) => PlaylistErrorCode::TrackNotFound,
            Self::SourceNotFound { .. } => PlaylistErrorCode::SourceNotFound,
            Self::SourceMismatch { .. } => PlaylistErrorCode::SourceMismatch,
            Self::SystemPlaylist => PlaylistErrorCode::SystemPlaylist,
            Self::BranchExists(_) => PlaylistErrorCode::BranchExists,
            Self::CannotBranch => PlaylistErrorCode::CannotBranch,
            Self::BranchNotFound(_) => PlaylistErrorCode::BranchNotFound,
            Self::BranchNotOpen(_) => PlaylistErrorCode::BranchNotOpen,
            Self::BranchAlreadyMerged(_) => PlaylistErrorCode::BranchAlreadyMerged,
            Self::BranchConflict { .. } => PlaylistErrorCode::BranchConflict,
            Self::InvalidBranchChange(_) => PlaylistErrorCode::InvalidBranchChange,
            Self::EmptySelection => PlaylistErrorCode::EmptySelection,
            Self::InvalidPosition(_) => PlaylistErrorCode::InvalidPosition,
            Self::TagNotFound(_) => PlaylistErrorCode::TagNotFound,
            Self::TagExists(_) => PlaylistErrorCode::TagExists,
            Self::InvalidRating => PlaylistErrorCode::InvalidRating,
            Self::CollectionRequestTooLarge => PlaylistErrorCode::CollectionRequestTooLarge,
            Self::SnapshotNotFound(_) => PlaylistErrorCode::SnapshotNotFound,
        }
    }

    pub fn dto(&self) -> PlaylistErrorDto {
        PlaylistErrorDto {
            code: self.code(),
            detail: self.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct PlaylistService {
    database: Database,
}

impl PlaylistService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list_playlists(&self) -> Result<Vec<Playlist>, PlaylistError> {
        let connection = self.database.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, kind, parent_playlist_id, base_parent_revision,
                    branch_status, revision, created_at, updated_at
             FROM playlists
             ORDER BY CASE kind WHEN 'inbox' THEN 0 WHEN 'normal' THEN 1 ELSE 2 END,
                      name COLLATE NOCASE, id",
        )?;
        let rows = statement.query_map([], map_playlist_row)?;
        rows.map(|row| {
            let record = row?;
            load_playlist_from_record(&connection, record)
        })
        .collect()
    }

    pub fn get_playlist(&self, playlist_id: PlaylistId) -> Result<Option<Playlist>, PlaylistError> {
        let connection = self.database.connection()?;
        let record = connection
            .query_row(
                "SELECT id, name, kind, parent_playlist_id, base_parent_revision,
                        branch_status, revision, created_at, updated_at
                 FROM playlists WHERE id = ?1",
                params![playlist_id.to_string()],
                map_playlist_row,
            )
            .optional()?;
        record
            .map(|record| load_playlist_from_record(&connection, record))
            .transpose()
    }

    pub fn create_playlist(&self, name: impl Into<String>) -> Result<Playlist, PlaylistError> {
        let name = validate_name(name.into(), MAX_PLAYLIST_NAME_SCALARS)?;
        let playlist_id = PlaylistId::new();
        let now = Utc::now();
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO playlists (
                    id, name, kind, parent_playlist_id, base_parent_revision,
                    branch_status, revision, created_at, updated_at
                 ) VALUES (?1, ?2, 'normal', NULL, NULL, NULL, 0, ?3, ?3)",
                params![playlist_id.to_string(), name, now.to_rfc3339()],
            )?;
            Ok(())
        })?;
        self.get_playlist(playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))
    }

    pub fn rename_playlist(
        &self,
        playlist_id: PlaylistId,
        name: impl Into<String>,
    ) -> Result<Playlist, PlaylistError> {
        let name = validate_name(name.into(), MAX_PLAYLIST_NAME_SCALARS)?;
        let connection = self.database.connection()?;
        let record = load_playlist_record(&connection, playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))?;
        if record.kind == PlaylistKind::Inbox {
            return Err(PlaylistError::SystemPlaylist);
        }
        let now = Utc::now().to_rfc3339();
        connection.execute(
            "UPDATE playlists
             SET name = ?1, revision = revision + 1, updated_at = ?2
             WHERE id = ?3",
            params![name, now, playlist_id.to_string()],
        )?;
        drop(connection);
        self.get_playlist(playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))
    }

    pub fn delete_playlist(&self, playlist_id: PlaylistId) -> Result<(), PlaylistError> {
        let connection = self.database.connection()?;
        let record = load_playlist_record(&connection, playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))?;
        match record.kind {
            PlaylistKind::Inbox => Err(PlaylistError::SystemPlaylist),
            PlaylistKind::Branch => {
                drop(connection);
                self.discard_branch(playlist_id)
            }
            PlaylistKind::Normal => {
                let open_branches: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM playlists
                     WHERE parent_playlist_id = ?1 AND kind = 'branch' AND branch_status = 'open'",
                    params![playlist_id.to_string()],
                    |row| row.get(0),
                )?;
                if open_branches > 0 {
                    return Err(PlaylistError::BranchExists(playlist_id));
                }
                connection.execute(
                    "DELETE FROM playlists WHERE id = ?1",
                    params![playlist_id.to_string()],
                )?;
                Ok(())
            }
        }
    }

    pub fn duplicate_playlist(
        &self,
        playlist_id: PlaylistId,
        requested_name: Option<String>,
    ) -> Result<Playlist, PlaylistError> {
        let source = self
            .get_playlist(playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))?;
        let name = requested_name.unwrap_or_else(|| format!("{} copy", source.name));
        let name = validate_name(
            truncate_scalars(&name, MAX_PLAYLIST_NAME_SCALARS),
            MAX_PLAYLIST_NAME_SCALARS,
        )?;
        let new_id = PlaylistId::new();
        let now = Utc::now();
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO playlists (
                id, name, kind, parent_playlist_id, base_parent_revision,
                branch_status, revision, created_at, updated_at
             ) VALUES (?1, ?2, 'normal', NULL, NULL, NULL, 0, ?3, ?3)",
            params![new_id.to_string(), name, now.to_rfc3339()],
        )?;
        for (position, item) in source.items.iter().enumerate() {
            transaction.execute(
                "INSERT INTO playlist_items (
                    id, playlist_id, track_id, requested_source_id, position,
                    origin_base_item_id, added_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6)",
                params![
                    PlaylistItemId::new().to_string(),
                    new_id.to_string(),
                    item.track_id.to_string(),
                    item.requested_source_id.map(|value| value.to_string()),
                    i64::try_from(position).expect("playlist position fits SQLite"),
                    now.to_rfc3339(),
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.get_playlist(new_id)?
            .ok_or(PlaylistError::PlaylistNotFound(new_id))
    }

    pub fn add_playlist_item(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
        requested_source_id: Option<SourceId>,
    ) -> Result<PlaylistItem, PlaylistError> {
        let mut connection = self.database.connection()?;
        let playlist = load_playlist_record(&connection, playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))?;
        if playlist.kind == PlaylistKind::Inbox {
            return Err(PlaylistError::SystemPlaylist);
        }
        ensure_branch_open(&playlist)?;
        ensure_track_and_source(&connection, track_id, requested_source_id)?;
        let transaction = connection.transaction()?;
        let position: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM playlist_items WHERE playlist_id = ?1",
            params![playlist_id.to_string()],
            |row| row.get(0),
        )?;
        let item_id = PlaylistItemId::new();
        let now = Utc::now().to_rfc3339();
        transaction.execute(
            "INSERT INTO playlist_items (
                id, playlist_id, track_id, requested_source_id, position,
                origin_base_item_id, added_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6)",
            params![
                item_id.to_string(),
                playlist_id.to_string(),
                track_id.to_string(),
                requested_source_id.map(|value| value.to_string()),
                position,
                now,
            ],
        )?;
        touch_playlist(&transaction, playlist_id, &now)?;
        transaction.commit()?;
        drop(connection);
        self.get_item(item_id)
    }

    pub fn remove_playlist_item(
        &self,
        playlist_id: PlaylistId,
        item_id: PlaylistItemId,
    ) -> Result<(), PlaylistError> {
        let mut connection = self.database.connection()?;
        let playlist = load_playlist_record(&connection, playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))?;
        if playlist.kind == PlaylistKind::Inbox {
            return Err(PlaylistError::SystemPlaylist);
        }
        ensure_branch_open(&playlist)?;
        let transaction = connection.transaction()?;
        let deleted = transaction.execute(
            "DELETE FROM playlist_items WHERE id = ?1 AND playlist_id = ?2",
            params![item_id.to_string(), playlist_id.to_string()],
        )?;
        if deleted == 0 {
            return Err(PlaylistError::PlaylistItemNotFound(item_id));
        }
        rewrite_playlist_positions(&transaction, playlist_id)?;
        touch_playlist(&transaction, playlist_id, &Utc::now().to_rfc3339())?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reorder_playlist_item(
        &self,
        playlist_id: PlaylistId,
        item_id: PlaylistItemId,
        target_position: u32,
    ) -> Result<Playlist, PlaylistError> {
        let mut connection = self.database.connection()?;
        let playlist = load_playlist_record(&connection, playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))?;
        if playlist.kind == PlaylistKind::Inbox {
            return Err(PlaylistError::SystemPlaylist);
        }
        ensure_branch_open(&playlist)?;
        let transaction = connection.transaction()?;
        let mut ids = playlist_item_ids(&transaction, playlist_id)?;
        let current = ids
            .iter()
            .position(|id| *id == item_id)
            .ok_or(PlaylistError::PlaylistItemNotFound(item_id))?;
        let target = usize::try_from(target_position)
            .map_err(|_| PlaylistError::InvalidPosition(target_position))?;
        if target >= ids.len() {
            return Err(PlaylistError::InvalidPosition(target_position));
        }
        let item = ids.remove(current);
        ids.insert(target, item);
        for (position, id) in ids.iter().enumerate() {
            transaction.execute(
                "UPDATE playlist_items SET position = ?1, updated_at = ?2 WHERE id = ?3",
                params![
                    i64::try_from(position).expect("playlist position fits SQLite"),
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )?;
        }
        touch_playlist(&transaction, playlist_id, &Utc::now().to_rfc3339())?;
        transaction.commit()?;
        drop(connection);
        self.get_playlist(playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(playlist_id))
    }

    pub fn create_playlist_branch(
        &self,
        parent_playlist_id: PlaylistId,
        name: impl Into<String>,
    ) -> Result<Playlist, PlaylistError> {
        let name = validate_name(name.into(), MAX_PLAYLIST_NAME_SCALARS)?;
        let mut connection = self.database.connection()?;
        let parent = load_playlist_record(&connection, parent_playlist_id)?
            .ok_or(PlaylistError::PlaylistNotFound(parent_playlist_id))?;
        if parent.kind != PlaylistKind::Normal {
            return Err(PlaylistError::CannotBranch);
        }
        let parent_items = load_item_records(&connection, parent_playlist_id)?;
        let branch_id = PlaylistId::new();
        let now = Utc::now().to_rfc3339();
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO playlists (
                id, name, kind, parent_playlist_id, base_parent_revision,
                branch_status, revision, created_at, updated_at
             ) VALUES (?1, ?2, 'branch', ?3, ?4, 'open', 0, ?5, ?5)",
            params![
                branch_id.to_string(),
                name,
                parent_playlist_id.to_string(),
                i64::try_from(parent.revision).expect("playlist revision fits SQLite"),
                now,
            ],
        )?;
        for item in parent_items {
            let branch_item_id = PlaylistItemId::new();
            transaction.execute(
                "INSERT INTO playlist_items (
                    id, playlist_id, track_id, requested_source_id, position,
                    origin_base_item_id, added_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    branch_item_id.to_string(),
                    branch_id.to_string(),
                    item.track_id.to_string(),
                    item.requested_source_id.map(|value| value.to_string()),
                    i64::from(item.position),
                    item.id.to_string(),
                    now,
                ],
            )?;
            transaction.execute(
                "INSERT INTO playlist_branch_base_items (
                    branch_playlist_id, base_item_id, track_id, requested_source_id, position
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    branch_id.to_string(),
                    item.id.to_string(),
                    item.track_id.to_string(),
                    item.requested_source_id.map(|value| value.to_string()),
                    i64::from(item.position),
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.get_playlist(branch_id)?
            .ok_or(PlaylistError::PlaylistNotFound(branch_id))
    }

    pub fn get_branch_changes(
        &self,
        branch_playlist_id: PlaylistId,
    ) -> Result<Vec<BranchChange>, PlaylistError> {
        let connection = self.database.connection()?;
        let branch = load_playlist_record(&connection, branch_playlist_id)?
            .ok_or(PlaylistError::BranchNotFound(branch_playlist_id))?;
        if branch.kind != PlaylistKind::Branch {
            return Err(PlaylistError::CannotBranch);
        }
        let base_items = load_base_item_records(&connection, branch_playlist_id)?;
        let branch_items = load_item_records(&connection, branch_playlist_id)?;
        Ok(branch_changes(&base_items, &branch_items))
    }

    pub fn merge_branch_changes(
        &self,
        branch_playlist_id: PlaylistId,
        selected_changes: Vec<BranchChange>,
    ) -> Result<BranchMergeResult, PlaylistError> {
        if selected_changes.is_empty() {
            return Err(PlaylistError::EmptySelection);
        }
        let available_changes = self.get_branch_changes(branch_playlist_id)?;
        let mut seen = HashSet::new();
        for change in &selected_changes {
            if !available_changes.contains(change) {
                return Err(PlaylistError::InvalidBranchChange(format!("{change:?}")));
            }
            let key = serde_json::to_string(change).map_err(|source| {
                PlaylistError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(source)))
            })?;
            if !seen.insert(key) {
                return Err(PlaylistError::InvalidBranchChange(
                    "the same change was selected twice".to_owned(),
                ));
            }
        }

        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let branch = load_playlist_record_in_transaction(&transaction, branch_playlist_id)?
            .ok_or(PlaylistError::BranchNotFound(branch_playlist_id))?;
        if branch.kind != PlaylistKind::Branch {
            return Err(PlaylistError::CannotBranch);
        }
        if branch.branch_status != Some(BranchStatus::Open) {
            return Err(PlaylistError::BranchAlreadyMerged(branch_playlist_id));
        }
        let parent_id = branch
            .parent_playlist_id
            .ok_or_else(|| PlaylistError::InvalidBranchChange("branch has no parent".to_owned()))?;
        let parent = load_playlist_record_in_transaction(&transaction, parent_id)?
            .ok_or(PlaylistError::PlaylistNotFound(parent_id))?;
        let base_revision = branch.base_parent_revision.ok_or_else(|| {
            PlaylistError::InvalidBranchChange("branch has no base revision".to_owned())
        })?;
        if parent.revision != base_revision {
            return Err(PlaylistError::BranchConflict {
                base_revision,
                parent_revision: parent.revision,
            });
        }

        let branch_items = load_item_records_in_transaction(&transaction, branch_playlist_id)?;
        let mut parent_items = load_item_records_in_transaction(&transaction, parent_id)?;
        let mut ordered_changes = selected_changes;
        ordered_changes.sort_by_key(branch_change_sort_key);
        for change in &ordered_changes {
            match change {
                BranchChange::Add { branch_item_id } => {
                    let branch_item = branch_items
                        .iter()
                        .find(|item| {
                            item.id == *branch_item_id && item.origin_base_item_id.is_none()
                        })
                        .ok_or_else(|| {
                            PlaylistError::InvalidBranchChange(format!(
                                "add item {branch_item_id} is no longer present"
                            ))
                        })?;
                    let new_item = StoredItem {
                        id: PlaylistItemId::new(),
                        playlist_id: parent_id,
                        track_id: branch_item.track_id,
                        requested_source_id: branch_item.requested_source_id,
                        position: branch_item.position,
                        origin_base_item_id: None,
                        added_at: Utc::now(),
                        updated_at: Utc::now(),
                    };
                    insert_item_at(&mut parent_items, new_item);
                }
                BranchChange::Remove { base_item_id } => {
                    let index = parent_items
                        .iter()
                        .position(|item| item.id == *base_item_id)
                        .ok_or_else(|| {
                            PlaylistError::InvalidBranchChange(format!(
                                "base item {base_item_id} is not in the parent"
                            ))
                        })?;
                    parent_items.remove(index);
                }
                BranchChange::Move {
                    base_item_id,
                    target_position,
                } => {
                    let index = parent_items
                        .iter()
                        .position(|item| item.id == *base_item_id)
                        .ok_or_else(|| {
                            PlaylistError::InvalidBranchChange(format!(
                                "base item {base_item_id} is not in the parent"
                            ))
                        })?;
                    let mut item = parent_items.remove(index);
                    item.position = *target_position;
                    insert_item_at(&mut parent_items, item);
                }
            }
        }
        rewrite_items_in_transaction(&transaction, parent_id, &parent_items)?;
        let now = Utc::now().to_rfc3339();
        transaction.execute(
            "UPDATE playlists SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
            params![now, parent_id.to_string()],
        )?;
        transaction.execute(
            "UPDATE playlists
             SET branch_status = 'merged', revision = revision + 1, updated_at = ?1
             WHERE id = ?2",
            params![now, branch_playlist_id.to_string()],
        )?;
        transaction.commit()?;
        drop(connection);
        let parent = self
            .get_playlist(parent_id)?
            .ok_or(PlaylistError::PlaylistNotFound(parent_id))?;
        let branch = self
            .get_playlist(branch_playlist_id)?
            .ok_or(PlaylistError::BranchNotFound(branch_playlist_id))?;
        Ok(BranchMergeResult { parent, branch })
    }

    pub fn discard_branch(&self, branch_playlist_id: PlaylistId) -> Result<(), PlaylistError> {
        let connection = self.database.connection()?;
        let branch = load_playlist_record(&connection, branch_playlist_id)?
            .ok_or(PlaylistError::BranchNotFound(branch_playlist_id))?;
        if branch.kind != PlaylistKind::Branch {
            return Err(PlaylistError::CannotBranch);
        }
        connection.execute(
            "DELETE FROM playlists WHERE id = ?1",
            params![branch_playlist_id.to_string()],
        )?;
        Ok(())
    }

    pub fn add_track_to_inbox(&self, track_id: TrackId) -> Result<PlaylistItem, PlaylistError> {
        let connection = self.database.connection()?;
        ensure_track_and_source(&connection, track_id, None)?;
        if let Some(existing) = connection
            .query_row(
                "SELECT id, playlist_id, track_id, requested_source_id, position,
                        origin_base_item_id, added_at, updated_at
                 FROM playlist_items WHERE playlist_id = ?1 AND track_id = ?2
                 ORDER BY position, id LIMIT 1",
                params![INBOX_ID.to_string(), track_id.to_string()],
                map_item_row,
            )
            .optional()?
        {
            return Ok(stored_item_to_public(existing));
        }
        let position: i64 = connection.query_row(
            "SELECT COUNT(*) FROM playlist_items WHERE playlist_id = ?1",
            params![INBOX_ID.to_string()],
            |row| row.get(0),
        )?;
        let now = Utc::now().to_rfc3339();
        let item = StoredItem {
            id: PlaylistItemId::new(),
            playlist_id: INBOX_ID,
            track_id,
            requested_source_id: None,
            position: u32::try_from(position)
                .map_err(|_| PlaylistError::InvalidPosition(u32::MAX))?,
            origin_base_item_id: None,
            added_at: Utc::now(),
            updated_at: Utc::now(),
        };
        connection.execute(
            "INSERT INTO playlist_items (
                id, playlist_id, track_id, requested_source_id, position,
                origin_base_item_id, added_at, updated_at
             ) VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5, ?5)",
            params![
                item.id.to_string(),
                INBOX_ID.to_string(),
                track_id.to_string(),
                position,
                now,
            ],
        )?;
        connection.execute(
            "UPDATE playlists SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
            params![now, INBOX_ID.to_string()],
        )?;
        Ok(stored_item_to_public(item))
    }

    pub fn set_track_liked(&self, track_id: TrackId, liked: bool) -> Result<bool, PlaylistError> {
        let connection = self.database.connection()?;
        ensure_track(&connection, track_id)?;
        if liked {
            connection.execute(
                "INSERT INTO likes (track_id, liked_at) VALUES (?1, ?2)
                 ON CONFLICT(track_id) DO UPDATE SET liked_at = excluded.liked_at",
                params![track_id.to_string(), Utc::now().to_rfc3339()],
            )?;
        } else {
            connection.execute(
                "DELETE FROM likes WHERE track_id = ?1",
                params![track_id.to_string()],
            )?;
        }
        Ok(liked)
    }

    pub fn set_track_rating(
        &self,
        track_id: TrackId,
        rating: Option<u8>,
    ) -> Result<Option<u8>, PlaylistError> {
        let connection = self.database.connection()?;
        ensure_track(&connection, track_id)?;
        if rating.is_some_and(|value| !(1..=5).contains(&value)) {
            return Err(PlaylistError::InvalidRating);
        }
        match rating {
            Some(rating) => {
                connection.execute(
                    "INSERT INTO ratings (track_id, rating, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(track_id) DO UPDATE SET rating = excluded.rating, updated_at = excluded.updated_at",
                    params![track_id.to_string(), i64::from(rating), Utc::now().to_rfc3339()],
                )?;
            }
            None => {
                connection.execute(
                    "DELETE FROM ratings WHERE track_id = ?1",
                    params![track_id.to_string()],
                )?;
            }
        }
        Ok(rating)
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>, PlaylistError> {
        let connection = self.database.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, normalized_name, created_at, updated_at
             FROM tags ORDER BY normalized_name, id",
        )?;
        let rows = statement.query_map([], map_tag_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(PlaylistError::from)
    }

    pub fn create_tag(&self, name: impl Into<String>) -> Result<Tag, PlaylistError> {
        let name = normalize_tag_name(name.into())?;
        let normalized_name = name.to_lowercase();
        let tag_id = TagId::new();
        let now = Utc::now().to_rfc3339();
        let connection = self.database.connection()?;
        let result = connection.execute(
            "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![tag_id.to_string(), name, normalized_name, now],
        );
        if let Err(error) = result {
            if is_unique_constraint(&error) {
                return Err(PlaylistError::TagExists(normalized_name));
            }
            return Err(PlaylistError::Sqlite(error));
        }
        drop(connection);
        self.get_tag(tag_id)
    }

    pub fn rename_tag(&self, tag_id: TagId, name: impl Into<String>) -> Result<Tag, PlaylistError> {
        let name = normalize_tag_name(name.into())?;
        let normalized_name = name.to_lowercase();
        let connection = self.database.connection()?;
        let exists: Option<String> = connection
            .query_row(
                "SELECT normalized_name FROM tags WHERE id = ?1",
                params![tag_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(PlaylistError::TagNotFound(tag_id));
        }
        let result = connection.execute(
            "UPDATE tags SET name = ?1, normalized_name = ?2, updated_at = ?3 WHERE id = ?4",
            params![
                name,
                normalized_name,
                Utc::now().to_rfc3339(),
                tag_id.to_string()
            ],
        );
        if let Err(error) = result {
            if is_unique_constraint(&error) {
                return Err(PlaylistError::TagExists(normalized_name));
            }
            return Err(PlaylistError::Sqlite(error));
        }
        drop(connection);
        self.get_tag(tag_id)
    }

    pub fn delete_tag(&self, tag_id: TagId) -> Result<(), PlaylistError> {
        let connection = self.database.connection()?;
        let deleted = connection.execute(
            "DELETE FROM tags WHERE id = ?1",
            params![tag_id.to_string()],
        )?;
        if deleted == 0 {
            return Err(PlaylistError::TagNotFound(tag_id));
        }
        Ok(())
    }

    pub fn add_track_tag(&self, track_id: TrackId, tag_id: TagId) -> Result<(), PlaylistError> {
        let connection = self.database.connection()?;
        ensure_track(&connection, track_id)?;
        ensure_tag(&connection, tag_id)?;
        connection.execute(
            "INSERT INTO track_tags (track_id, tag_id, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(track_id, tag_id) DO NOTHING",
            params![
                track_id.to_string(),
                tag_id.to_string(),
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn remove_track_tag(&self, track_id: TrackId, tag_id: TagId) -> Result<(), PlaylistError> {
        let connection = self.database.connection()?;
        ensure_track(&connection, track_id)?;
        connection.execute(
            "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
            params![track_id.to_string(), tag_id.to_string()],
        )?;
        Ok(())
    }

    pub fn get_track_collection_states(
        &self,
        track_ids: &[TrackId],
    ) -> Result<Vec<TrackCollectionState>, PlaylistError> {
        if track_ids.len() > MAX_COLLECTION_TRACKS {
            return Err(PlaylistError::CollectionRequestTooLarge);
        }
        let connection = self.database.connection()?;
        let mut states = Vec::with_capacity(track_ids.len());
        for &track_id in track_ids {
            ensure_track(&connection, track_id)?;
            states.push(load_collection_state(&connection, track_id)?);
        }
        Ok(states)
    }

    fn get_item(&self, item_id: PlaylistItemId) -> Result<PlaylistItem, PlaylistError> {
        let connection = self.database.connection()?;
        connection
            .query_row(
                "SELECT id, playlist_id, track_id, requested_source_id, position,
                        origin_base_item_id, added_at, updated_at
                 FROM playlist_items WHERE id = ?1",
                params![item_id.to_string()],
                map_item_row,
            )
            .optional()?
            .map(stored_item_to_public)
            .ok_or(PlaylistError::PlaylistItemNotFound(item_id))
    }

    fn get_tag(&self, tag_id: TagId) -> Result<Tag, PlaylistError> {
        let connection = self.database.connection()?;
        connection
            .query_row(
                "SELECT id, name, normalized_name, created_at, updated_at
                 FROM tags WHERE id = ?1",
                params![tag_id.to_string()],
                map_tag_row,
            )
            .optional()?
            .ok_or(PlaylistError::TagNotFound(tag_id))
    }
}

#[derive(Clone, Debug)]
struct PlaylistRecord {
    id: PlaylistId,
    name: String,
    kind: PlaylistKind,
    parent_playlist_id: Option<PlaylistId>,
    base_parent_revision: Option<u64>,
    branch_status: Option<BranchStatus>,
    revision: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct StoredItem {
    id: PlaylistItemId,
    playlist_id: PlaylistId,
    track_id: TrackId,
    requested_source_id: Option<SourceId>,
    position: u32,
    origin_base_item_id: Option<PlaylistItemId>,
    added_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct StoredBaseItem {
    base_item_id: PlaylistItemId,
    position: u32,
}

fn map_playlist_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlaylistRecord> {
    let revision_value: i64 = row.get(6)?;
    Ok(PlaylistRecord {
        id: parse_id(&row.get::<_, String>(0)?, "playlists.id")?,
        name: row.get(1)?,
        kind: parse_playlist_kind(&row.get::<_, String>(2)?)?,
        parent_playlist_id: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_id(&value, "playlists.parent_playlist_id"))
            .transpose()?,
        base_parent_revision: row
            .get::<_, Option<i64>>(4)?
            .map(|value| {
                u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, value))
            })
            .transpose()?,
        branch_status: row
            .get::<_, Option<String>>(5)?
            .map(|value| parse_branch_status(&value))
            .transpose()?,
        revision: u64::try_from(revision_value)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(6, revision_value))?,
        created_at: parse_timestamp(&row.get::<_, String>(7)?, "playlists.created_at")?,
        updated_at: parse_timestamp(&row.get::<_, String>(8)?, "playlists.updated_at")?,
    })
}

fn map_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredItem> {
    let position_value: i64 = row.get(4)?;
    Ok(StoredItem {
        id: parse_id(&row.get::<_, String>(0)?, "playlist_items.id")?,
        playlist_id: parse_id(&row.get::<_, String>(1)?, "playlist_items.playlist_id")?,
        track_id: parse_id(&row.get::<_, String>(2)?, "playlist_items.track_id")?,
        requested_source_id: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_id(&value, "playlist_items.requested_source_id"))
            .transpose()?,
        position: u32::try_from(position_value)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, position_value))?,
        origin_base_item_id: row
            .get::<_, Option<String>>(5)?
            .map(|value| parse_id(&value, "playlist_items.origin_base_item_id"))
            .transpose()?,
        added_at: parse_timestamp(&row.get::<_, String>(6)?, "playlist_items.added_at")?,
        updated_at: parse_timestamp(&row.get::<_, String>(7)?, "playlist_items.updated_at")?,
    })
}

fn map_tag_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: parse_id(&row.get::<_, String>(0)?, "tags.id")?,
        name: row.get(1)?,
        normalized_name: row.get(2)?,
        created_at: parse_timestamp(&row.get::<_, String>(3)?, "tags.created_at")?,
        updated_at: parse_timestamp(&row.get::<_, String>(4)?, "tags.updated_at")?,
    })
}

fn load_playlist_record(
    connection: &Connection,
    playlist_id: PlaylistId,
) -> Result<Option<PlaylistRecord>, PlaylistError> {
    connection
        .query_row(
            "SELECT id, name, kind, parent_playlist_id, base_parent_revision,
                    branch_status, revision, created_at, updated_at
             FROM playlists WHERE id = ?1",
            params![playlist_id.to_string()],
            map_playlist_row,
        )
        .optional()
        .map_err(PlaylistError::from)
}

fn load_playlist_record_in_transaction(
    transaction: &Transaction<'_>,
    playlist_id: PlaylistId,
) -> Result<Option<PlaylistRecord>, PlaylistError> {
    transaction
        .query_row(
            "SELECT id, name, kind, parent_playlist_id, base_parent_revision,
                    branch_status, revision, created_at, updated_at
             FROM playlists WHERE id = ?1",
            params![playlist_id.to_string()],
            map_playlist_row,
        )
        .optional()
        .map_err(PlaylistError::from)
}

fn load_playlist_from_record(
    connection: &Connection,
    record: PlaylistRecord,
) -> Result<Playlist, PlaylistError> {
    let items = load_item_records(connection, record.id)?
        .into_iter()
        .map(stored_item_to_public)
        .collect();
    Ok(Playlist {
        id: record.id,
        name: record.name,
        kind: record.kind,
        parent_playlist_id: record.parent_playlist_id,
        base_parent_revision: record.base_parent_revision,
        branch_status: record.branch_status,
        revision: record.revision,
        created_at: record.created_at,
        updated_at: record.updated_at,
        items,
    })
}

fn load_item_records(
    connection: &Connection,
    playlist_id: PlaylistId,
) -> Result<Vec<StoredItem>, PlaylistError> {
    let mut statement = connection.prepare(
        "SELECT id, playlist_id, track_id, requested_source_id, position,
                origin_base_item_id, added_at, updated_at
         FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map(params![playlist_id.to_string()], map_item_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(PlaylistError::from)
}

fn load_item_records_in_transaction(
    transaction: &Transaction<'_>,
    playlist_id: PlaylistId,
) -> Result<Vec<StoredItem>, PlaylistError> {
    let mut statement = transaction.prepare(
        "SELECT id, playlist_id, track_id, requested_source_id, position,
                origin_base_item_id, added_at, updated_at
         FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map(params![playlist_id.to_string()], map_item_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(PlaylistError::from)
}

fn load_base_item_records(
    connection: &Connection,
    branch_playlist_id: PlaylistId,
) -> Result<Vec<StoredBaseItem>, PlaylistError> {
    let mut statement = connection.prepare(
        "SELECT base_item_id, position
         FROM playlist_branch_base_items
         WHERE branch_playlist_id = ?1 ORDER BY position, base_item_id",
    )?;
    let rows = statement.query_map(params![branch_playlist_id.to_string()], |row| {
        Ok(StoredBaseItem {
            base_item_id: parse_id(
                &row.get::<_, String>(0)?,
                "playlist_branch_base_items.base_item_id",
            )?,
            position: u32::try_from(row.get::<_, i64>(1)?)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, 0))?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(PlaylistError::from)
}

fn branch_changes(base_items: &[StoredBaseItem], branch_items: &[StoredItem]) -> Vec<BranchChange> {
    let branch_origins: HashSet<PlaylistItemId> = branch_items
        .iter()
        .filter_map(|item| item.origin_base_item_id)
        .collect();
    let mut adds = branch_items
        .iter()
        .filter(|item| item.origin_base_item_id.is_none())
        .map(|item| {
            (
                item.position,
                item.id,
                BranchChange::Add {
                    branch_item_id: item.id,
                },
            )
        })
        .collect::<Vec<_>>();
    adds.sort_by_key(|(position, id, _)| (*position, id.to_string()));

    let mut removes = base_items
        .iter()
        .filter(|item| !branch_origins.contains(&item.base_item_id))
        .map(|item| {
            (
                item.position,
                item.base_item_id,
                BranchChange::Remove {
                    base_item_id: item.base_item_id,
                },
            )
        })
        .collect::<Vec<_>>();
    removes.sort_by_key(|(position, id, _)| (*position, id.to_string()));

    let base_by_id: HashMap<PlaylistItemId, &StoredBaseItem> = base_items
        .iter()
        .map(|item| (item.base_item_id, item))
        .collect();
    let mut moves = branch_items
        .iter()
        .filter_map(|item| {
            let base_id = item.origin_base_item_id?;
            let base = base_by_id.get(&base_id)?;
            (base.position != item.position).then_some((
                item.position,
                base_id,
                BranchChange::Move {
                    base_item_id: base_id,
                    target_position: item.position,
                },
            ))
        })
        .collect::<Vec<_>>();
    moves.sort_by_key(|(position, id, _)| (*position, id.to_string()));

    adds.into_iter()
        .chain(removes)
        .chain(moves)
        .map(|(_, _, change)| change)
        .collect()
}

fn branch_change_sort_key(change: &BranchChange) -> (u8, String) {
    match change {
        BranchChange::Remove { base_item_id } => (0, base_item_id.to_string()),
        BranchChange::Move { base_item_id, .. } => (1, base_item_id.to_string()),
        BranchChange::Add { branch_item_id } => (2, branch_item_id.to_string()),
    }
}

fn insert_item_at(items: &mut Vec<StoredItem>, item: StoredItem) {
    let target = usize::try_from(item.position)
        .unwrap_or(usize::MAX)
        .min(items.len());
    items.insert(target, item);
    for (position, current) in items.iter_mut().enumerate() {
        current.position = u32::try_from(position).expect("playlist position fits u32");
    }
}

fn rewrite_items_in_transaction(
    transaction: &Transaction<'_>,
    playlist_id: PlaylistId,
    items: &[StoredItem],
) -> Result<(), PlaylistError> {
    transaction.execute(
        "DELETE FROM playlist_items WHERE playlist_id = ?1",
        params![playlist_id.to_string()],
    )?;
    let now = Utc::now().to_rfc3339();
    for (position, item) in items.iter().enumerate() {
        transaction.execute(
            "INSERT INTO playlist_items (
                id, playlist_id, track_id, requested_source_id, position,
                origin_base_item_id, added_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6)",
            params![
                item.id.to_string(),
                playlist_id.to_string(),
                item.track_id.to_string(),
                item.requested_source_id.map(|value| value.to_string()),
                i64::try_from(position).expect("playlist position fits SQLite"),
                item.added_at.to_rfc3339(),
            ],
        )?;
    }
    transaction.execute(
        "UPDATE playlist_items SET updated_at = ?1 WHERE playlist_id = ?2",
        params![now, playlist_id.to_string()],
    )?;
    Ok(())
}

fn rewrite_playlist_positions(
    transaction: &Transaction<'_>,
    playlist_id: PlaylistId,
) -> Result<(), PlaylistError> {
    let ids = playlist_item_ids(transaction, playlist_id)?;
    for (position, id) in ids.iter().enumerate() {
        transaction.execute(
            "UPDATE playlist_items SET position = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                i64::try_from(position).expect("playlist position fits SQLite"),
                Utc::now().to_rfc3339(),
                id.to_string()
            ],
        )?;
    }
    Ok(())
}

fn playlist_item_ids(
    transaction: &Transaction<'_>,
    playlist_id: PlaylistId,
) -> Result<Vec<PlaylistItemId>, PlaylistError> {
    let mut statement = transaction
        .prepare("SELECT id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position, id")?;
    let rows = statement.query_map(params![playlist_id.to_string()], |row| {
        parse_id(&row.get::<_, String>(0)?, "playlist_items.id")
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(PlaylistError::from)
}

fn touch_playlist(
    transaction: &Transaction<'_>,
    playlist_id: PlaylistId,
    now: &str,
) -> Result<(), PlaylistError> {
    transaction.execute(
        "UPDATE playlists SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
        params![now, playlist_id.to_string()],
    )?;
    Ok(())
}

fn ensure_track(connection: &Connection, track_id: TrackId) -> Result<(), PlaylistError> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM tracks WHERE id = ?1",
            params![track_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(PlaylistError::TrackNotFound(track_id));
    }
    Ok(())
}

fn ensure_track_and_source(
    connection: &Connection,
    track_id: TrackId,
    source_id: Option<SourceId>,
) -> Result<(), PlaylistError> {
    ensure_track(connection, track_id)?;
    let Some(source_id) = source_id else {
        return Ok(());
    };
    let actual_track_id: Option<String> = connection
        .query_row(
            "SELECT track_id FROM track_sources WHERE id = ?1",
            params![source_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(actual_track_id) = actual_track_id else {
        return Err(PlaylistError::SourceNotFound {
            track_id,
            source_id,
        });
    };
    let actual_track_id = parse_id(&actual_track_id, "track_sources.track_id")?;
    if actual_track_id != track_id {
        return Err(PlaylistError::SourceMismatch {
            source_id,
            actual_track_id,
            expected_track_id: track_id,
        });
    }
    Ok(())
}

fn ensure_tag(connection: &Connection, tag_id: TagId) -> Result<(), PlaylistError> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM tags WHERE id = ?1",
            params![tag_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(PlaylistError::TagNotFound(tag_id));
    }
    Ok(())
}

fn ensure_branch_open(playlist: &PlaylistRecord) -> Result<(), PlaylistError> {
    if playlist.kind == PlaylistKind::Branch && playlist.branch_status != Some(BranchStatus::Open) {
        return Err(PlaylistError::BranchNotOpen(playlist.id));
    }
    Ok(())
}

fn load_collection_state(
    connection: &Connection,
    track_id: TrackId,
) -> Result<TrackCollectionState, PlaylistError> {
    let liked = connection
        .query_row(
            "SELECT 1 FROM likes WHERE track_id = ?1",
            params![track_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    let rating = connection
        .query_row(
            "SELECT rating FROM ratings WHERE track_id = ?1",
            params![track_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .map(|value| u8::try_from(value).unwrap_or_default());
    let mut tag_statement = connection.prepare(
        "SELECT t.id, t.name, t.normalized_name, t.created_at, t.updated_at
         FROM tags t INNER JOIN track_tags tt ON tt.tag_id = t.id
         WHERE tt.track_id = ?1 ORDER BY t.normalized_name, t.id",
    )?;
    let tags = tag_statement
        .query_map(params![track_id.to_string()], map_tag_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut membership_statement = connection.prepare(
        "SELECT p.id, p.name, p.kind
         FROM playlists p INNER JOIN playlist_items pi ON pi.playlist_id = p.id
         WHERE pi.track_id = ?1
         GROUP BY p.id, p.name, p.kind
         ORDER BY CASE p.kind WHEN 'inbox' THEN 0 ELSE 1 END, p.name COLLATE NOCASE, p.id",
    )?;
    let memberships = membership_statement
        .query_map(params![track_id.to_string()], |row| {
            Ok(PlaylistMembership {
                playlist_id: parse_id(&row.get::<_, String>(0)?, "playlists.id")?,
                name: row.get(1)?,
                kind: parse_playlist_kind(&row.get::<_, String>(2)?)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let in_inbox = memberships
        .iter()
        .any(|membership| membership.playlist_id == INBOX_ID);
    Ok(TrackCollectionState {
        track_id,
        liked,
        rating,
        tags,
        playlist_memberships: memberships,
        in_inbox,
    })
}

fn stored_item_to_public(item: StoredItem) -> PlaylistItem {
    PlaylistItem {
        id: item.id,
        playlist_id: item.playlist_id,
        track_id: item.track_id,
        requested_source_id: item.requested_source_id,
        position: item.position,
        origin_base_item_id: item.origin_base_item_id,
        added_at: item.added_at,
        updated_at: item.updated_at,
    }
}

fn validate_name(name: String, max_scalars: usize) -> Result<String, PlaylistError> {
    let name = name.trim().to_owned();
    let length = name.chars().count();
    if !(1..=max_scalars).contains(&length) {
        return Err(PlaylistError::InvalidName(format!(
            "must contain 1..{max_scalars} characters"
        )));
    }
    Ok(name)
}

fn normalize_tag_name(name: String) -> Result<String, PlaylistError> {
    let normalized_whitespace = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = normalized_whitespace.chars().count();
    if !(1..=MAX_TAG_NAME_SCALARS).contains(&length) {
        return Err(PlaylistError::InvalidTagName(format!(
            "must contain 1..{MAX_TAG_NAME_SCALARS} characters"
        )));
    }
    Ok(normalized_whitespace)
}

fn truncate_scalars(value: &str, max_scalars: usize) -> String {
    value.chars().take(max_scalars).collect()
}

fn parse_playlist_kind(value: &str) -> Result<PlaylistKind, rusqlite::Error> {
    match value {
        "normal" => Ok(PlaylistKind::Normal),
        "inbox" => Ok(PlaylistKind::Inbox),
        "branch" => Ok(PlaylistKind::Branch),
        _ => Err(rusqlite::Error::InvalidColumnType(
            2,
            "kind".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn parse_branch_status(value: &str) -> Result<BranchStatus, rusqlite::Error> {
    match value {
        "open" => Ok(BranchStatus::Open),
        "merged" => Ok(BranchStatus::Merged),
        _ => Err(rusqlite::Error::InvalidColumnType(
            5,
            "branch_status".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

fn parse_id<T>(value: &str, field: &str) -> Result<T, rusqlite::Error>
where
    T: FromStringId,
{
    T::parse(value).map_err(|_| {
        rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
    })
}

trait FromStringId: Sized {
    fn parse(value: &str) -> Result<Self, uuid::Error>;
}

macro_rules! impl_from_string_id {
    ($($id:ty),+ $(,)?) => {
        $(impl FromStringId for $id {
            fn parse(value: &str) -> Result<Self, uuid::Error> {
                <$id>::parse_str(value)
            }
        })+
    };
}

impl_from_string_id!(PlaylistId, PlaylistItemId, SourceId, TagId, TrackId);

fn parse_timestamp(value: &str, field: &str) -> Result<DateTime<Utc>, rusqlite::Error> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(0, field.to_owned(), rusqlite::types::Type::Text)
        })
}

fn is_unique_constraint(error: &rusqlite::Error) -> bool {
    error.sqlite_extended_error_code() == Some(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{SourceCapabilities, TrackSource, UnifiedTrack, VersionInfo};

    fn fixture_track(database: &Database) -> (TrackId, SourceId) {
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        let source = TrackSource::new_local(
            source_id,
            track_id,
            PathBuf::from(format!("C:\\Music\\plan08-{track_id}.wav")),
            SourceCapabilities {
                playback: true,
                metadata: true,
                ..SourceCapabilities::default()
            },
        )
        .unwrap();
        let track = UnifiedTrack::new(
            track_id,
            "Plan Eight",
            vec![],
            None,
            Some(180_000),
            VersionInfo::standard(),
            vec![source],
        )
        .unwrap();
        crate::db::repository::TrackRepository::new(database)
            .create(&track)
            .unwrap();
        (track_id, source_id)
    }

    fn service() -> (TempDatabasePath, Database, PlaylistService) {
        let path = TempDatabasePath::new("playlists");
        let database = Database::open(path.path()).unwrap();
        let service = PlaylistService::new(database.clone());
        (path, database, service)
    }

    #[test]
    fn inbox_is_seeded_and_add_is_idempotent() {
        let (_path, database, service) = service();
        let (track_id, _) = fixture_track(&database);
        let inbox = service.get_playlist(INBOX_ID).unwrap().unwrap();
        assert_eq!(inbox.name, INBOX_NAME);
        let first = service.add_track_to_inbox(track_id).unwrap();
        let second = service.add_track_to_inbox(track_id).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(
            service.get_playlist(INBOX_ID).unwrap().unwrap().items.len(),
            1
        );
        assert!(matches!(
            service.rename_playlist(INBOX_ID, "Other"),
            Err(PlaylistError::SystemPlaylist)
        ));
    }

    #[test]
    fn playlist_items_allow_duplicates_and_keep_dense_order() {
        let (_path, database, service) = service();
        let (track_id, source_id) = fixture_track(&database);
        let playlist = service.create_playlist("  Favorites  ").unwrap();
        let first = service
            .add_playlist_item(playlist.id, track_id, Some(source_id))
            .unwrap();
        let second = service
            .add_playlist_item(playlist.id, track_id, None)
            .unwrap();
        service
            .reorder_playlist_item(playlist.id, second.id, 0)
            .unwrap();
        service.remove_playlist_item(playlist.id, first.id).unwrap();
        let playlist = service.get_playlist(playlist.id).unwrap().unwrap();
        assert_eq!(playlist.items.len(), 1);
        assert_eq!(playlist.items[0].position, 0);
        assert_eq!(playlist.items[0].track_id, track_id);
    }

    #[test]
    fn branch_diff_and_selected_merge_are_one_shot() {
        let (_path, database, service) = service();
        let (first_track, first_source) = fixture_track(&database);
        let (second_track, _) = fixture_track(&database);
        let parent = service.create_playlist("Parent").unwrap();
        let base_item = service
            .add_playlist_item(parent.id, first_track, Some(first_source))
            .unwrap();
        let branch = service
            .create_playlist_branch(parent.id, "Experiment")
            .unwrap();
        let added = service
            .add_playlist_item(branch.id, second_track, None)
            .unwrap();
        let changes = service.get_branch_changes(branch.id).unwrap();
        assert!(changes.contains(&BranchChange::Add {
            branch_item_id: added.id
        }));
        assert!(!changes.contains(&BranchChange::Remove {
            base_item_id: base_item.id
        }));
        let merged = service.merge_branch_changes(branch.id, changes).unwrap();
        assert_eq!(merged.parent.items.len(), 2);
        assert_eq!(merged.parent.revision, parent.revision + 1 + 1);
        assert_eq!(merged.branch.branch_status, Some(BranchStatus::Merged));
        assert!(matches!(
            service.merge_branch_changes(
                branch.id,
                vec![BranchChange::Add {
                    branch_item_id: added.id
                }]
            ),
            Err(PlaylistError::BranchAlreadyMerged(_))
        ));
    }

    #[test]
    fn collections_normalize_tags_and_validate_ratings() {
        let (_path, database, service) = service();
        let (track_id, _) = fixture_track(&database);
        assert!(service.set_track_liked(track_id, true).unwrap());
        assert!(service.set_track_liked(track_id, true).unwrap());
        assert_eq!(
            service.set_track_rating(track_id, Some(5)).unwrap(),
            Some(5)
        );
        assert!(matches!(
            service.set_track_rating(track_id, Some(6)),
            Err(PlaylistError::InvalidRating)
        ));
        let tag = service.create_tag("  Road   Trip ").unwrap();
        assert_eq!(tag.name, "Road Trip");
        assert!(matches!(
            service.create_tag("road trip"),
            Err(PlaylistError::TagExists(_))
        ));
        service.add_track_tag(track_id, tag.id).unwrap();
        let state = service
            .get_track_collection_states(&[track_id])
            .unwrap()
            .remove(0);
        assert!(state.liked);
        assert_eq!(state.rating, Some(5));
        assert_eq!(state.tags.len(), 1);
        service.set_track_rating(track_id, None).unwrap();
        service.set_track_liked(track_id, false).unwrap();
    }
}
