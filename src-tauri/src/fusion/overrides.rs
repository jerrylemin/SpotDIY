use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::repository::{RepositoryError, TrackRepository};
use crate::db::{Database, DatabaseError};
use crate::domain::{ProviderKind, TrackId};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FusionOverrideDecision {
    Merge,
    Split,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionOverride {
    pub provider_kind: ProviderKind,
    pub provider_item_id: String,
    pub target_track_id: TrackId,
    pub decision: FusionOverrideDecision,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum FusionOverrideError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Spotify sources are excluded from source fusion overrides")]
    SpotifyExcluded,
    #[error("provider item ID cannot be empty")]
    EmptyProviderItemId,
    #[error("target track {0} was not found")]
    TrackNotFound(TrackId),
    #[error("invalid stored fusion override value for {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
}

pub struct FusionOverrideRepository<'database> {
    database: &'database Database,
}

impl<'database> FusionOverrideRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn set_override(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: impl Into<String>,
        target_track_id: TrackId,
        decision: FusionOverrideDecision,
    ) -> Result<FusionOverride, FusionOverrideError> {
        let provider_item_id = provider_item_id.into();
        validate_identity(provider_kind, &provider_item_id)?;
        if TrackRepository::new(self.database)
            .get(target_track_id)?
            .is_none()
        {
            return Err(FusionOverrideError::TrackNotFound(target_track_id));
        }

        let now = Utc::now().to_rfc3339();
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        if decision == FusionOverrideDecision::Merge {
            transaction.execute(
                "DELETE FROM user_track_overrides
                 WHERE provider_kind = ?1 AND provider_item_id = ?2 AND decision = 'merge'",
                params![provider_kind.as_str(), provider_item_id],
            )?;
        }
        transaction.execute(
            "INSERT INTO user_track_overrides (
                provider_kind, provider_item_id, target_track_id, decision,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(provider_kind, provider_item_id, target_track_id) DO UPDATE SET
                decision = excluded.decision,
                updated_at = excluded.updated_at",
            params![
                provider_kind.as_str(),
                provider_item_id,
                target_track_id.to_string(),
                decision.as_str(),
                now,
            ],
        )?;
        transaction.commit()?;
        drop(connection);

        self.get_override(provider_kind, &provider_item_id, target_track_id)?
            .ok_or(FusionOverrideError::InvalidStoredValue {
                field: "user_track_overrides",
                value: "override disappeared after write".to_owned(),
            })
    }

    pub fn clear_override(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: &str,
        target_track_id: TrackId,
    ) -> Result<(), FusionOverrideError> {
        validate_identity(provider_kind, provider_item_id)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "DELETE FROM user_track_overrides
                 WHERE provider_kind = ?1 AND provider_item_id = ?2 AND target_track_id = ?3",
                params![
                    provider_kind.as_str(),
                    provider_item_id,
                    target_track_id.to_string(),
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn get_override(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: &str,
        target_track_id: TrackId,
    ) -> Result<Option<FusionOverride>, FusionOverrideError> {
        validate_identity(provider_kind, provider_item_id)?;
        let connection = self.database.connection()?;
        connection
            .query_row(
                "SELECT provider_kind, provider_item_id, target_track_id, decision,
                        created_at, updated_at
                 FROM user_track_overrides
                 WHERE provider_kind = ?1 AND provider_item_id = ?2 AND target_track_id = ?3",
                params![
                    provider_kind.as_str(),
                    provider_item_id,
                    target_track_id.to_string(),
                ],
                map_override,
            )
            .optional()?
            .map(parse_override)
            .transpose()
    }

    pub fn forced_merge_target(
        &self,
        provider_kind: ProviderKind,
        provider_item_id: &str,
    ) -> Result<Option<TrackId>, FusionOverrideError> {
        validate_identity(provider_kind, provider_item_id)?;
        let connection = self.database.connection()?;
        let target: Option<String> = connection
            .query_row(
                "SELECT target_track_id FROM user_track_overrides
                 WHERE provider_kind = ?1 AND provider_item_id = ?2 AND decision = 'merge'",
                params![provider_kind.as_str(), provider_item_id],
                |row| row.get(0),
            )
            .optional()?;
        target
            .map(|value| parse_track_id(&value, "user_track_overrides.target_track_id"))
            .transpose()
    }
}

impl FusionOverrideDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Split => "split",
        }
    }
}

fn validate_identity(
    provider_kind: ProviderKind,
    provider_item_id: &str,
) -> Result<(), FusionOverrideError> {
    if provider_kind == ProviderKind::Spotify {
        return Err(FusionOverrideError::SpotifyExcluded);
    }
    if provider_item_id.trim().is_empty() {
        return Err(FusionOverrideError::EmptyProviderItemId);
    }
    Ok(())
}

fn map_override(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawFusionOverride> {
    Ok(RawFusionOverride {
        provider_kind: row.get(0)?,
        provider_item_id: row.get(1)?,
        target_track_id: row.get(2)?,
        decision: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

struct RawFusionOverride {
    provider_kind: String,
    provider_item_id: String,
    target_track_id: String,
    decision: String,
    created_at: String,
    updated_at: String,
}

fn parse_override(raw: RawFusionOverride) -> Result<FusionOverride, FusionOverrideError> {
    let provider_kind =
        raw.provider_kind
            .parse()
            .map_err(|_| FusionOverrideError::InvalidStoredValue {
                field: "user_track_overrides.provider_kind",
                value: raw.provider_kind.clone(),
            })?;
    let decision = match raw.decision.as_str() {
        "merge" => FusionOverrideDecision::Merge,
        "split" => FusionOverrideDecision::Split,
        _ => {
            return Err(FusionOverrideError::InvalidStoredValue {
                field: "user_track_overrides.decision",
                value: raw.decision,
            })
        }
    };
    Ok(FusionOverride {
        provider_kind,
        provider_item_id: raw.provider_item_id,
        target_track_id: parse_track_id(
            &raw.target_track_id,
            "user_track_overrides.target_track_id",
        )?,
        decision,
        created_at: parse_timestamp(&raw.created_at, "user_track_overrides.created_at")?,
        updated_at: parse_timestamp(&raw.updated_at, "user_track_overrides.updated_at")?,
    })
}

fn parse_track_id(value: &str, field: &'static str) -> Result<TrackId, FusionOverrideError> {
    TrackId::parse_str(value).map_err(|_| FusionOverrideError::InvalidStoredValue {
        field,
        value: value.to_owned(),
    })
}

fn parse_timestamp(value: &str, field: &'static str) -> Result<DateTime<Utc>, FusionOverrideError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| FusionOverrideError::InvalidStoredValue {
            field,
            value: value.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};
    use crate::domain::{Artist, ArtistId, UnifiedTrack, VersionInfo};

    fn track(database: &Database) -> TrackId {
        let id = TrackId::new();
        let track = UnifiedTrack::new(
            id,
            "Override target",
            vec![Artist::new(ArtistId::new(), "Artist").unwrap()],
            None,
            None,
            VersionInfo::standard(),
            vec![],
        )
        .unwrap();
        crate::db::repository::TrackRepository::new(database)
            .create(&track)
            .unwrap();
        id
    }

    #[test]
    fn merge_upsert_replaces_forced_target_transactionally() {
        let path = TempDatabasePath::new("override-merge");
        let database = Database::open(path.path()).unwrap();
        let first = track(&database);
        let second = track(&database);
        let repository = FusionOverrideRepository::new(&database);

        repository
            .set_override(
                ProviderKind::Youtube,
                "video-1",
                first,
                FusionOverrideDecision::Merge,
            )
            .unwrap();
        repository
            .set_override(
                ProviderKind::Youtube,
                "video-1",
                second,
                FusionOverrideDecision::Merge,
            )
            .unwrap();

        assert_eq!(
            repository
                .forced_merge_target(ProviderKind::Youtube, "video-1")
                .unwrap(),
            Some(second)
        );
        assert!(repository
            .get_override(ProviderKind::Youtube, "video-1", first)
            .unwrap()
            .is_none());
    }

    #[test]
    fn split_can_be_read_and_cleared() {
        let path = TempDatabasePath::new("override-split");
        let database = Database::open(path.path()).unwrap();
        let target = track(&database);
        let repository = FusionOverrideRepository::new(&database);

        let override_record = repository
            .set_override(
                ProviderKind::Soundcloud,
                "track-1",
                target,
                FusionOverrideDecision::Split,
            )
            .unwrap();
        assert_eq!(override_record.decision, FusionOverrideDecision::Split);
        repository
            .clear_override(ProviderKind::Soundcloud, "track-1", target)
            .unwrap();
        assert!(repository
            .get_override(ProviderKind::Soundcloud, "track-1", target)
            .unwrap()
            .is_none());
    }

    #[test]
    fn spotify_is_rejected_before_storage() {
        let path = TempDatabasePath::new("override-spotify");
        let database = Database::open(path.path()).unwrap();
        let target = track(&database);
        let repository = FusionOverrideRepository::new(&database);

        assert!(matches!(
            repository.set_override(
                ProviderKind::Spotify,
                "spotify-track",
                target,
                FusionOverrideDecision::Merge,
            ),
            Err(FusionOverrideError::SpotifyExcluded)
        ));
    }
}
