//! Local listening history, sessions, and analytics queries.
//!
//! This module deliberately has no provider or network boundary.  The
//! recorder keeps the active interval in memory and writes one row only when
//! the interval is finalized.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{Database, DatabaseError};
use crate::domain::{ListeningSessionId, PlayHistoryId, ProviderKind, SourceId, TrackId};
use crate::sessions::{local_parts, normalize_label, ListeningModeService};

pub const SESSION_GAP: Duration = Duration::from_secs(30 * 60);
pub const QUALIFIED_PLAY_MS: u64 = 30_000;
pub const MAX_TIME_MACHINE_ROWS: u32 = 1_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryOutcome {
    Completed,
    Skipped,
    Stopped,
    Interrupted,
}

impl HistoryOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Skipped => "skipped",
            Self::Stopped => "stopped",
            Self::Interrupted => "interrupted",
        }
    }
}

impl TryFrom<String> for HistoryOutcome {
    type Error = AnalyticsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "completed" => Ok(Self::Completed),
            "skipped" => Ok(Self::Skipped),
            "stopped" => Ok(Self::Stopped),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AnalyticsError::InvalidData(format!(
                "unknown history outcome {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackMetadata {
    pub track_id: Option<TrackId>,
    pub source_id: Option<SourceId>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub provider_kind: Option<ProviderKind>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: PlayHistoryId,
    pub session_id: Option<ListeningSessionId>,
    pub track_id: Option<TrackId>,
    pub source_id: Option<SourceId>,
    pub title_snapshot: String,
    pub artists: Vec<String>,
    pub album_snapshot: Option<String>,
    pub provider_kind: Option<ProviderKind>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub local_date: String,
    pub local_hour: u8,
    pub local_weekday: u8,
    pub listened_ms: u64,
    pub duration_ms: Option<u64>,
    pub outcome: HistoryOutcome,
    pub qualified_play: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningSession {
    pub id: ListeningSessionId,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub label: Option<String>,
    pub event_count: u64,
    pub listened_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsOverview {
    pub listened_ms: u64,
    pub qualified_plays: u64,
    pub skips: u64,
    pub unique_tracks: u64,
    pub unique_artists: u64,
    pub session_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningHeatmapCell {
    pub weekday: u8,
    pub hour: u8,
    pub listened_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopTrack {
    pub track_id: Option<TrackId>,
    pub title: String,
    pub artists: Vec<String>,
    pub listened_ms: u64,
    pub qualified_plays: u64,
    pub play_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopArtist {
    pub name: String,
    pub listened_ms: u64,
    pub qualified_plays: u64,
    pub play_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteTimelineMonth {
    pub month: String,
    pub listened_ms: u64,
    pub qualified_plays: u64,
    pub top_tracks: Vec<String>,
    pub top_artists: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReopenQueueEntry {
    pub track_id: TrackId,
    pub requested_source_id: Option<SourceId>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReopenQueueResult {
    pub entries: Vec<ReopenQueueEntry>,
    pub dropped_count: u32,
}

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid analytics input: {0}")]
    InvalidInput(String),
    #[error("invalid analytics data: {0}")]
    InvalidData(String),
}

#[derive(Clone)]
pub struct AnalyticsService {
    database: Database,
}

impl AnalyticsService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn record(&self, input: &FinalizedHistory) -> Result<HistoryEntry, AnalyticsError> {
        let started_at = input.started_at.to_rfc3339();
        let ended_at = input.ended_at.to_rfc3339();
        let created_at = input.created_at.to_rfc3339();
        let (local_date, local_hour, local_weekday) = local_parts(input.started_at);
        let artists_json = serde_json::to_string(&input.metadata.artists)?;
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        let previous: Option<(String, String)> = transaction
            .query_row(
                "SELECT session_id, ended_at
                 FROM play_history
                 WHERE session_id IS NOT NULL
                 ORDER BY ended_at DESC, id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let session_id = previous
            .and_then(|(session_id, previous_ended)| {
                let previous_ended = parse_timestamp(&previous_ended).ok()?;
                let gap = input.started_at.signed_duration_since(previous_ended);
                (gap >= chrono::Duration::zero()
                    && gap <= chrono::Duration::from_std(SESSION_GAP).ok()?)
                .then_some(session_id)
            })
            .unwrap_or_else(|| ListeningSessionId::new().to_string());
        if transaction
            .query_row(
                "SELECT 1 FROM listening_sessions WHERE id = ?1",
                [&session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some()
        {
            transaction.execute(
                "UPDATE listening_sessions
                 SET started_at = CASE WHEN started_at > ?1 THEN ?1 ELSE started_at END,
                     ended_at = CASE WHEN ended_at < ?2 THEN ?2 ELSE ended_at END,
                     updated_at = ?3
                 WHERE id = ?4",
                params![started_at, ended_at, created_at, session_id],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO listening_sessions
                 (id, started_at, ended_at, label, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?4)",
                params![session_id, started_at, ended_at, created_at],
            )?;
        }
        let id = input.id.to_string();
        transaction.execute(
            "INSERT INTO play_history
             (id, session_id, track_id, source_id, title_snapshot, artists_json,
              album_snapshot, provider_kind, started_at, ended_at, local_date,
              local_hour, local_weekday, listened_ms, duration_ms, outcome,
              qualified_play, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18)",
            params![
                id,
                session_id,
                input.metadata.track_id.map(|value| value.to_string()),
                input.metadata.source_id.map(|value| value.to_string()),
                input.metadata.title,
                artists_json,
                input.metadata.album,
                input.metadata.provider_kind.map(|value| value.as_str()),
                started_at,
                ended_at,
                local_date,
                i64::from(local_hour),
                i64::from(local_weekday),
                checked_i64(input.listened_ms)?,
                input.duration_ms.map(checked_i64).transpose()?,
                input.outcome.as_str(),
                bool_i64(input.qualified_play),
                created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(HistoryEntry {
            id: input.id,
            session_id: session_id.parse().ok(),
            track_id: input.metadata.track_id,
            source_id: input.metadata.source_id,
            title_snapshot: input.metadata.title.clone(),
            artists: input.metadata.artists.clone(),
            album_snapshot: input.metadata.album.clone(),
            provider_kind: input.metadata.provider_kind,
            started_at: input.started_at,
            ended_at: input.ended_at,
            local_date,
            local_hour,
            local_weekday,
            listened_ms: input.listened_ms,
            duration_ms: input.duration_ms,
            outcome: input.outcome,
            qualified_play: input.qualified_play,
            created_at: input.created_at,
        })
    }

    pub fn overview(&self) -> Result<AnalyticsOverview, AnalyticsError> {
        self.database
            .with_connection(|connection| {
                let (listened_ms, qualified_plays, skips, unique_tracks, session_count): (
                    i64,
                    i64,
                    i64,
                    i64,
                    i64,
                ) = connection.query_row(
                    "SELECT COALESCE(SUM(listened_ms), 0),
                        COALESCE(SUM(qualified_play), 0),
                        COALESCE(SUM(outcome = 'skipped'), 0),
                        COUNT(DISTINCT track_id),
                        COUNT(DISTINCT session_id)
                 FROM play_history",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )?;
                let unique_artists: i64 = connection.query_row(
                    "SELECT COUNT(DISTINCT lower(value))
                 FROM play_history, json_each(play_history.artists_json)",
                    [],
                    |row| row.get(0),
                )?;
                Ok(AnalyticsOverview {
                    listened_ms: checked_u64(listened_ms)?,
                    qualified_plays: checked_u64(qualified_plays)?,
                    skips: checked_u64(skips)?,
                    unique_tracks: checked_u64(unique_tracks)?,
                    unique_artists: checked_u64(unique_artists)?,
                    session_count: checked_u64(session_count)?,
                })
            })
            .map_err(AnalyticsError::from)
    }

    pub fn heatmap(&self) -> Result<Vec<ListeningHeatmapCell>, AnalyticsError> {
        self.database
            .with_connection(|connection| {
                let mut values = [[0_u64; 24]; 7];
                let mut statement = connection.prepare(
                    "SELECT local_weekday, local_hour, COALESCE(SUM(listened_ms), 0)
                 FROM play_history GROUP BY local_weekday, local_hour",
                )?;
                let rows = statement.query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?;
                for row in rows {
                    let (weekday, hour, listened_ms) = row?;
                    if let (Ok(weekday), Ok(hour)) =
                        (usize::try_from(weekday), usize::try_from(hour))
                    {
                        if weekday < 7 && hour < 24 {
                            values[weekday][hour] = checked_u64(listened_ms)?;
                        }
                    }
                }
                Ok((0..7)
                    .flat_map(|weekday| {
                        (0..24).map(move |hour| ListeningHeatmapCell {
                            weekday: weekday as u8,
                            hour: hour as u8,
                            listened_ms: values[weekday][hour],
                        })
                    })
                    .collect())
            })
            .map_err(AnalyticsError::from)
    }

    pub fn top_tracks(&self, limit: u32) -> Result<Vec<TopTrack>, AnalyticsError> {
        let limit = i64::from(limit.clamp(1, 100));
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT track_id, title_snapshot, artists_json,
                        COALESCE(SUM(listened_ms), 0), COALESCE(SUM(qualified_play), 0), COUNT(*)
                 FROM play_history
                 GROUP BY track_id, title_snapshot, artists_json
                 ORDER BY SUM(listened_ms) DESC, SUM(qualified_play) DESC,
                          title_snapshot COLLATE NOCASE, COALESCE(track_id, '')
                 LIMIT ?1",
            )?;
            let rows = statement.query_map([limit], map_top_track)?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        rows.into_iter().map(parse_top_track).collect()
    }

    pub fn top_artists(&self, limit: u32) -> Result<Vec<TopArtist>, AnalyticsError> {
        let limit = i64::from(limit.clamp(1, 100));
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT value, COALESCE(SUM(play_history.listened_ms), 0),
                        COALESCE(SUM(play_history.qualified_play), 0), COUNT(*)
                 FROM play_history, json_each(play_history.artists_json)
                 GROUP BY lower(value)
                 ORDER BY SUM(play_history.listened_ms) DESC,
                          SUM(play_history.qualified_play) DESC, value COLLATE NOCASE
                 LIMIT ?1",
            )?;
            let rows = statement.query_map([limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        rows.into_iter()
            .map(|(name, listened_ms, qualified_plays, play_count)| {
                Ok(TopArtist {
                    name,
                    listened_ms: checked_u64(listened_ms).map_err(AnalyticsError::from)?,
                    qualified_plays: checked_u64(qualified_plays).map_err(AnalyticsError::from)?,
                    play_count: checked_u64(play_count).map_err(AnalyticsError::from)?,
                })
            })
            .collect()
    }

    pub fn taste_timeline(&self) -> Result<Vec<TasteTimelineMonth>, AnalyticsError> {
        self.database
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT substr(local_date, 1, 7), COALESCE(SUM(listened_ms), 0),
                        COALESCE(SUM(qualified_play), 0)
                 FROM play_history
                 GROUP BY substr(local_date, 1, 7)
                 ORDER BY substr(local_date, 1, 7) DESC LIMIT 36",
                )?;
                let rows = statement.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        checked_u64(row.get(1)?)?,
                        checked_u64(row.get(2)?)?,
                    ))
                })?;
                let months = rows.collect::<Result<Vec<_>, _>>()?;
                months
                    .into_iter()
                    .map(|(month, listened_ms, qualified_plays)| {
                        let top_tracks = timeline_names(
                            connection,
                            "SELECT title_snapshot FROM play_history
                         WHERE substr(local_date, 1, 7) = ?1
                         GROUP BY title_snapshot ORDER BY SUM(listened_ms) DESC,
                         title_snapshot COLLATE NOCASE LIMIT 5",
                            &month,
                        )?;
                        let top_artists = timeline_artists(connection, &month)?;
                        Ok(TasteTimelineMonth {
                            month,
                            listened_ms,
                            qualified_plays,
                            top_tracks,
                            top_artists,
                        })
                    })
                    .collect()
            })
            .map_err(AnalyticsError::from)
    }

    pub fn list_sessions(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<Page<ListeningSession>, AnalyticsError> {
        let page_size = page_size.clamp(1, 100);
        let offset = i64::from(page).saturating_mul(i64::from(page_size));
        let (total, rows) = self.database.with_connection(|connection| {
            let total: i64 =
                connection.query_row("SELECT COUNT(*) FROM listening_sessions", [], |row| {
                    row.get(0)
                })?;
            let mut statement = connection.prepare(
                "SELECT s.id, s.started_at, s.ended_at, s.label,
                        COUNT(h.id), COALESCE(SUM(h.listened_ms), 0)
                 FROM listening_sessions s LEFT JOIN play_history h ON h.session_id = s.id
                 GROUP BY s.id ORDER BY s.started_at DESC, s.id DESC LIMIT ?1 OFFSET ?2",
            )?;
            let rows = statement.query_map(params![i64::from(page_size), offset], map_session)?;
            Ok((total, rows.collect::<Result<Vec<_>, _>>()?))
        })?;
        let items = rows
            .into_iter()
            .map(parse_session)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Page {
            items,
            total: checked_u64(total).map_err(AnalyticsError::from)?,
            page,
            page_size,
        })
    }

    pub fn get_session(
        &self,
        session_id: ListeningSessionId,
    ) -> Result<Option<ListeningSession>, AnalyticsError> {
        let raw = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT s.id, s.started_at, s.ended_at, s.label,
                        COUNT(h.id), COALESCE(SUM(h.listened_ms), 0)
                 FROM listening_sessions s LEFT JOIN play_history h ON h.session_id = s.id
                 WHERE s.id = ?1 GROUP BY s.id",
                    [session_id.to_string()],
                    map_session,
                )
                .optional()
        })?;
        raw.map(parse_session).transpose()
    }

    pub fn set_session_label(
        &self,
        session_id: ListeningSessionId,
        label: Option<String>,
    ) -> Result<ListeningSession, AnalyticsError> {
        let label = label
            .map(|value| {
                normalize_label(value, 80).ok_or_else(|| {
                    AnalyticsError::InvalidInput(
                        "session label must contain 1..80 Unicode scalar values".to_owned(),
                    )
                })
            })
            .transpose()?;
        let raw = self.database.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE listening_sessions SET label = ?1, updated_at = ?2 WHERE id = ?3",
                params![label, Utc::now().to_rfc3339(), session_id.to_string()],
            )?;
            if changed == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            connection.query_row(
                "SELECT s.id, s.started_at, s.ended_at, s.label,
                        COUNT(h.id), COALESCE(SUM(h.listened_ms), 0)
                 FROM listening_sessions s LEFT JOIN play_history h ON h.session_id = s.id
                 WHERE s.id = ?1 GROUP BY s.id",
                [session_id.to_string()],
                map_session,
            )
        })?;
        parse_session(raw)
    }

    pub fn get_session_history(
        &self,
        session_id: ListeningSessionId,
    ) -> Result<Vec<HistoryEntry>, AnalyticsError> {
        self.history_for_session(session_id)
    }

    pub fn time_machine_day(&self, local_date: &str) -> Result<Vec<HistoryEntry>, AnalyticsError> {
        if chrono::NaiveDate::parse_from_str(local_date, "%Y-%m-%d").is_err() {
            return Err(AnalyticsError::InvalidInput(
                "localDate must be YYYY-MM-DD".to_owned(),
            ));
        }
        self.history_for_day(local_date)
    }

    pub fn reopen_session(
        &self,
        session_id: ListeningSessionId,
    ) -> Result<ReopenQueueResult, AnalyticsError> {
        self.reopen_filter(Some(session_id.to_string()), None)
    }

    pub fn reopen_day(&self, local_date: &str) -> Result<ReopenQueueResult, AnalyticsError> {
        if chrono::NaiveDate::parse_from_str(local_date, "%Y-%m-%d").is_err() {
            return Err(AnalyticsError::InvalidInput(
                "localDate must be YYYY-MM-DD".to_owned(),
            ));
        }
        self.reopen_filter(None, Some(local_date.to_owned()))
    }
}

// rusqlite's Params is intentionally not object-safe enough for a reusable
// helper.  These concrete helpers keep every query parameterized while
// supporting the two public history filters.
impl AnalyticsService {
    pub fn history_for_session(
        &self,
        session_id: ListeningSessionId,
    ) -> Result<Vec<HistoryEntry>, AnalyticsError> {
        self.history_for_filter(Some(session_id.to_string()), None, None)
    }

    pub fn history_for_day(&self, local_date: &str) -> Result<Vec<HistoryEntry>, AnalyticsError> {
        self.history_for_filter(
            None,
            Some(local_date.to_owned()),
            Some(MAX_TIME_MACHINE_ROWS),
        )
    }

    fn history_for_filter(
        &self,
        session_id: Option<String>,
        local_date: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<HistoryEntry>, AnalyticsError> {
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, session_id, track_id, source_id, title_snapshot, artists_json,
                        album_snapshot, provider_kind, started_at, ended_at, local_date,
                        local_hour, local_weekday, listened_ms, duration_ms, outcome,
                        qualified_play, created_at
                 FROM play_history
                 WHERE (?1 IS NULL OR session_id = ?1) AND (?2 IS NULL OR local_date = ?2)
                 ORDER BY started_at ASC, id ASC LIMIT ?3",
            )?;
            let rows = statement.query_map(
                params![session_id, local_date, limit.map(i64::from).unwrap_or(-1)],
                map_history,
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        rows.into_iter().map(parse_history).collect()
    }

    fn reopen_filter(
        &self,
        session_id: Option<String>,
        local_date: Option<String>,
    ) -> Result<ReopenQueueResult, AnalyticsError> {
        self.database
            .with_connection(|connection| {
                let total: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM play_history h
                 WHERE (?1 IS NULL OR h.session_id = ?1) AND (?2 IS NULL OR h.local_date = ?2)",
                    params![session_id, local_date],
                    |row| row.get(0),
                )?;
                let mut statement = connection.prepare(
                    "SELECT h.track_id, s.id
                 FROM play_history h INNER JOIN tracks t ON t.id = h.track_id
                 LEFT JOIN track_sources s ON s.id = h.source_id AND s.track_id = t.id
                 WHERE (?1 IS NULL OR h.session_id = ?1) AND (?2 IS NULL OR h.local_date = ?2)
                 ORDER BY h.started_at ASC, h.id ASC LIMIT ?3",
                )?;
                let mut entries = Vec::new();
                let rows = statement.query_map(
                    params![session_id, local_date, i64::from(MAX_TIME_MACHINE_ROWS)],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )?;
                for row in rows {
                    let (track_id, source_id) = row?;
                    let track_id = track_id
                        .parse()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let source_id = source_id.and_then(|value| value.parse().ok());
                    entries.push(ReopenQueueEntry {
                        track_id,
                        requested_source_id: source_id,
                    });
                }
                let returned = i64::try_from(entries.len()).unwrap_or(i64::MAX);
                let dropped_count =
                    u32::try_from(total.saturating_sub(returned)).unwrap_or(u32::MAX);
                Ok(ReopenQueueResult {
                    entries,
                    dropped_count,
                })
            })
            .map_err(AnalyticsError::from)
    }
}

#[derive(Clone, Debug)]
pub struct FinalizedHistory {
    pub id: PlayHistoryId,
    pub metadata: PlaybackMetadata,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub listened_ms: u64,
    pub duration_ms: Option<u64>,
    pub outcome: HistoryOutcome,
    pub qualified_play: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecorderPhase {
    Playing,
    Paused,
}

struct ActiveEvent {
    id: PlayHistoryId,
    metadata: PlaybackMetadata,
    started_at: DateTime<Utc>,
    duration_ms: Option<u64>,
    private: bool,
    listened_ms: u64,
    playing_since: Option<Instant>,
}

pub struct AnalyticsRecorder {
    service: AnalyticsService,
    modes: ListeningModeService,
    active: Option<ActiveEvent>,
}

impl AnalyticsRecorder {
    pub fn new(database: Database, modes: ListeningModeService) -> Self {
        Self {
            service: AnalyticsService::new(database),
            modes,
            active: None,
        }
    }

    pub fn service(&self) -> &AnalyticsService {
        &self.service
    }

    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn begin_track(
        &mut self,
        metadata: PlaybackMetadata,
        duration_ms: Option<u64>,
        started_at: DateTime<Utc>,
        now: Instant,
    ) {
        self.active = Some(ActiveEvent {
            id: PlayHistoryId::new(),
            metadata,
            started_at,
            duration_ms,
            private: self.modes.state().private_session,
            listened_ms: 0,
            playing_since: None,
        });
        self.set_phase(RecorderPhase::Paused, now);
    }

    pub fn set_phase(&mut self, phase: RecorderPhase, now: Instant) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        match phase {
            RecorderPhase::Playing => {
                if active.playing_since.is_none() {
                    active.playing_since = Some(now);
                }
            }
            RecorderPhase::Paused => self.accrue(now),
        }
    }

    pub fn finish(
        &mut self,
        outcome: HistoryOutcome,
        ended_at: DateTime<Utc>,
        now: Instant,
    ) -> Result<Option<HistoryEntry>, AnalyticsError> {
        self.accrue(now);
        let Some(active) = self.active.take() else {
            return Ok(None);
        };
        if active.private || self.modes.state().private_session || self.modes.state().temporary {
            return Ok(None);
        }
        let qualified_play = qualifies(active.listened_ms, active.duration_ms);
        self.service
            .record(&FinalizedHistory {
                id: active.id,
                metadata: active.metadata,
                started_at: active.started_at,
                ended_at,
                listened_ms: active.listened_ms,
                duration_ms: active.duration_ms,
                outcome,
                qualified_play,
                created_at: Utc::now(),
            })
            .map(Some)
    }

    pub fn finish_transition(
        &mut self,
        ended_at: DateTime<Utc>,
        now: Instant,
    ) -> Result<Option<HistoryEntry>, AnalyticsError> {
        self.accrue(now);
        let outcome = self
            .active
            .as_ref()
            .filter(|active| qualifies(active.listened_ms, active.duration_ms))
            .map(|_| HistoryOutcome::Stopped)
            .unwrap_or(HistoryOutcome::Skipped);
        self.finish(outcome, ended_at, now)
    }

    pub fn set_private(
        &mut self,
        enabled: bool,
        at: DateTime<Utc>,
        now: Instant,
    ) -> Result<(), AnalyticsError> {
        if enabled {
            let _ = self.finish(HistoryOutcome::Stopped, at, now)?;
        } else if let Some(active) = self.active.as_mut() {
            let was_playing = active.playing_since.is_some();
            active.private = false;
            active.started_at = at;
            active.id = PlayHistoryId::new();
            active.listened_ms = 0;
            active.playing_since = was_playing.then_some(now);
        }
        Ok(())
    }

    pub fn finish_interrupted(
        &mut self,
        at: DateTime<Utc>,
        now: Instant,
    ) -> Result<Option<HistoryEntry>, AnalyticsError> {
        self.finish(HistoryOutcome::Interrupted, at, now)
    }

    fn accrue(&mut self, now: Instant) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let Some(started) = active.playing_since.take() else {
            return;
        };
        let elapsed = now.saturating_duration_since(started);
        active.listened_ms = active
            .listened_ms
            .saturating_add(elapsed.as_millis().min(u128::from(u64::MAX)) as u64);
    }
}

pub fn qualifies(listened_ms: u64, duration_ms: Option<u64>) -> bool {
    let threshold = duration_ms
        .filter(|duration| *duration > 0)
        .map(|duration| QUALIFIED_PLAY_MS.min(duration / 2))
        .unwrap_or(QUALIFIED_PLAY_MS);
    listened_ms >= threshold
}

fn bool_i64(value: bool) -> i64 {
    i64::from(value)
}

fn checked_i64(value: u64) -> Result<i64, AnalyticsError> {
    i64::try_from(value)
        .map_err(|_| AnalyticsError::InvalidInput("analytics value is too large".to_owned()))
}

fn checked_u64(value: i64) -> Result<u64, rusqlite::Error> {
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, value))
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, AnalyticsError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| AnalyticsError::InvalidData(error.to_string()))
}

type RawHistory = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    Option<i64>,
    String,
    i64,
    String,
);

fn map_history(row: &Row<'_>) -> Result<RawHistory, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
    ))
}

fn parse_history(raw: RawHistory) -> Result<HistoryEntry, AnalyticsError> {
    let artists = serde_json::from_str(&raw.5)?;
    Ok(HistoryEntry {
        id: raw
            .0
            .parse()
            .map_err(|_| AnalyticsError::InvalidData("invalid play history id".to_owned()))?,
        session_id: raw
            .1
            .map(|value| value.parse())
            .transpose()
            .map_err(|_| AnalyticsError::InvalidData("invalid session id".to_owned()))?,
        track_id: raw
            .2
            .map(|value| value.parse())
            .transpose()
            .map_err(|_| AnalyticsError::InvalidData("invalid track id".to_owned()))?,
        source_id: raw
            .3
            .map(|value| value.parse())
            .transpose()
            .map_err(|_| AnalyticsError::InvalidData("invalid source id".to_owned()))?,
        title_snapshot: raw.4,
        artists,
        album_snapshot: raw.6,
        provider_kind: raw
            .7
            .map(|value| value.parse())
            .transpose()
            .map_err(|_| AnalyticsError::InvalidData("invalid provider".to_owned()))?,
        started_at: parse_timestamp(&raw.8)?,
        ended_at: parse_timestamp(&raw.9)?,
        local_date: raw.10,
        local_hour: u8::try_from(raw.11)
            .map_err(|_| AnalyticsError::InvalidData("invalid local hour".to_owned()))?,
        local_weekday: u8::try_from(raw.12)
            .map_err(|_| AnalyticsError::InvalidData("invalid local weekday".to_owned()))?,
        listened_ms: u64::try_from(raw.13)
            .map_err(|_| AnalyticsError::InvalidData("invalid listened time".to_owned()))?,
        duration_ms: raw
            .14
            .map(u64::try_from)
            .transpose()
            .map_err(|_| AnalyticsError::InvalidData("invalid duration".to_owned()))?,
        outcome: HistoryOutcome::try_from(raw.15)?,
        qualified_play: raw.16 != 0,
        created_at: parse_timestamp(&raw.17)?,
    })
}

type RawSession = (String, String, String, Option<String>, i64, i64);

fn map_session(row: &Row<'_>) -> Result<RawSession, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn parse_session(raw: RawSession) -> Result<ListeningSession, AnalyticsError> {
    Ok(ListeningSession {
        id: raw
            .0
            .parse()
            .map_err(|_| AnalyticsError::InvalidData("invalid session id".to_owned()))?,
        started_at: parse_timestamp(&raw.1)?,
        ended_at: parse_timestamp(&raw.2)?,
        label: raw.3,
        event_count: u64::try_from(raw.4)
            .map_err(|_| AnalyticsError::InvalidData("invalid event count".to_owned()))?,
        listened_ms: u64::try_from(raw.5)
            .map_err(|_| AnalyticsError::InvalidData("invalid session time".to_owned()))?,
    })
}

type RawTopTrack = (Option<String>, String, String, i64, i64, i64);

fn map_top_track(row: &Row<'_>) -> Result<RawTopTrack, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn parse_top_track(raw: RawTopTrack) -> Result<TopTrack, AnalyticsError> {
    Ok(TopTrack {
        track_id: raw
            .0
            .map(|value| value.parse())
            .transpose()
            .map_err(|_| AnalyticsError::InvalidData("invalid track id".to_owned()))?,
        title: raw.1,
        artists: serde_json::from_str(&raw.2)?,
        listened_ms: u64::try_from(raw.3)
            .map_err(|_| AnalyticsError::InvalidData("invalid listened time".to_owned()))?,
        qualified_plays: u64::try_from(raw.4)
            .map_err(|_| AnalyticsError::InvalidData("invalid qualified play count".to_owned()))?,
        play_count: u64::try_from(raw.5)
            .map_err(|_| AnalyticsError::InvalidData("invalid play count".to_owned()))?,
    })
}

fn timeline_names(
    connection: &rusqlite::Connection,
    sql: &str,
    month: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([month], |row| row.get(0))?;
    rows.collect()
}

fn timeline_artists(
    connection: &rusqlite::Connection,
    month: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT value FROM play_history, json_each(play_history.artists_json)
         WHERE substr(local_date, 1, 7) = ?1 GROUP BY lower(value)
         ORDER BY SUM(play_history.listened_ms) DESC, value COLLATE NOCASE LIMIT 5",
    )?;
    let rows = statement.query_map([month], |row| row.get(0))?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::domain::{SourceId, TrackId};
    use uuid::Uuid;

    fn database(label: &str) -> Database {
        let path = std::env::temp_dir().join(format!(
            "spotdiy-analytics-{label}-{}.sqlite3",
            Uuid::new_v4()
        ));
        Database::open(path).unwrap()
    }

    fn metadata() -> PlaybackMetadata {
        PlaybackMetadata {
            track_id: None,
            source_id: None,
            title: "Synthetic".to_owned(),
            artists: vec!["Artist".to_owned()],
            album: Some("Album".to_owned()),
            provider_kind: Some(ProviderKind::Local),
        }
    }

    fn metadata_for(track_id: TrackId, source_id: SourceId, title: &str) -> PlaybackMetadata {
        PlaybackMetadata {
            track_id: Some(track_id),
            source_id: Some(source_id),
            title: title.to_owned(),
            artists: vec!["Artist".to_owned()],
            album: Some("Album".to_owned()),
            provider_kind: Some(ProviderKind::Local),
        }
    }

    fn insert_track(database: &Database, title: &str) -> (TrackId, SourceId) {
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        let artist_id = Uuid::new_v4();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO tracks (id, title, normalized_title, duration_ms,
                     created_at, updated_at)
                     VALUES (?1, ?2, ?3, 60000, '2026-01-01T00:00:00Z',
                             '2026-01-01T00:00:00Z')",
                    params![track_id.to_string(), title, title.to_lowercase()],
                )?;
                connection.execute(
                    "INSERT INTO track_sources
                     (id, track_id, provider_kind, provider_item_id, duration_ms,
                      available, can_playback, created_at, updated_at)
                     VALUES (?1, ?2, 'local', ?3, 60000, 1, 1,
                             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    params![
                        source_id.to_string(),
                        track_id.to_string(),
                        source_id.to_string()
                    ],
                )?;
                connection.execute(
                    "INSERT INTO artists (id, name, created_at, updated_at)
                     VALUES (?1, 'Artist', '2026-01-01T00:00:00Z',
                             '2026-01-01T00:00:00Z')",
                    [artist_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO track_artists (track_id, artist_id, artist_order)
                     VALUES (?1, ?2, 0)",
                    params![track_id.to_string(), artist_id.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        (track_id, source_id)
    }

    #[test]
    fn qualification_uses_known_duration_half_and_unknown_thirty_seconds() {
        assert!(qualifies(5_000, Some(10_000)));
        assert!(!qualifies(4_999, Some(10_000)));
        assert!(!qualifies(29_999, None));
        assert!(qualifies(30_000, None));
    }

    #[test]
    fn recorder_excludes_pause_and_seek_like_phase_changes() {
        let database = database("recorder");
        let modes = ListeningModeService::new();
        let mut recorder = AnalyticsRecorder::new(database.clone(), modes);
        let base = Instant::now();
        let start = Utc::now();
        recorder.begin_track(metadata(), Some(120_000), start, base);
        recorder.set_phase(RecorderPhase::Playing, base);
        recorder.set_phase(RecorderPhase::Paused, base + Duration::from_secs(10));
        recorder.set_phase(RecorderPhase::Playing, base + Duration::from_secs(30));
        let result = recorder
            .finish(
                HistoryOutcome::Stopped,
                start + chrono::Duration::seconds(40),
                base + Duration::from_secs(35),
            )
            .unwrap()
            .unwrap();
        assert_eq!(result.listened_ms, 15_000);
    }

    #[test]
    fn disabling_private_while_paused_does_not_start_listening_clock() {
        let database = database("private-paused");
        let modes = ListeningModeService::new();
        modes.set_private(true).unwrap();
        let mut recorder = AnalyticsRecorder::new(database, modes.clone());
        let base = Instant::now();
        let start = Utc::now();
        recorder.begin_track(metadata(), Some(60_000), start, base);
        modes.set_private(false).unwrap();
        recorder
            .set_private(false, start, base + Duration::from_millis(100))
            .unwrap();

        let result = recorder
            .finish(
                HistoryOutcome::Stopped,
                start + chrono::Duration::seconds(2),
                base + Duration::from_secs(2),
            )
            .unwrap()
            .unwrap();
        assert_eq!(result.listened_ms, 0);
    }

    #[test]
    fn sessions_group_at_thirty_minutes() {
        let database = database("sessions");
        let service = AnalyticsService::new(database);
        let first = Utc::now();
        for (id, offset) in [
            (PlayHistoryId::new(), 0_i64),
            (PlayHistoryId::new(), 1_800),
            (PlayHistoryId::new(), 3_602),
        ] {
            service
                .record(&FinalizedHistory {
                    id,
                    metadata: metadata(),
                    started_at: first + chrono::Duration::seconds(offset),
                    ended_at: first + chrono::Duration::seconds(offset + 1),
                    listened_ms: 30_000,
                    duration_ms: Some(60_000),
                    outcome: HistoryOutcome::Completed,
                    qualified_play: true,
                    created_at: first + chrono::Duration::seconds(offset),
                })
                .unwrap();
        }
        assert_eq!(service.list_sessions(0, 100).unwrap().items.len(), 2);
    }

    #[test]
    fn analytics_queries_are_local_typed_and_reopen_preserves_live_duplicates() {
        let database = database("queries");
        let (track_one, source_one) = insert_track(&database, "One");
        let (track_two, source_two) = insert_track(&database, "Two");
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO track_genres (track_id, genre, normalized_genre)
                     VALUES (?1, 'Rock', 'rock')",
                    params![track_one.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        let service = AnalyticsService::new(database.clone());
        let base = Utc::now() - chrono::Duration::days(2);
        let first = service
            .record(&FinalizedHistory {
                id: PlayHistoryId::new(),
                metadata: metadata_for(track_one, source_one, "One"),
                started_at: base,
                ended_at: base + chrono::Duration::seconds(10),
                listened_ms: 40_000,
                duration_ms: Some(60_000),
                outcome: HistoryOutcome::Completed,
                qualified_play: true,
                created_at: base,
            })
            .unwrap();
        service
            .record(&FinalizedHistory {
                id: PlayHistoryId::new(),
                metadata: metadata_for(track_two, source_two, "Two"),
                started_at: base + chrono::Duration::minutes(10),
                ended_at: base + chrono::Duration::minutes(10) + chrono::Duration::seconds(5),
                listened_ms: 5_000,
                duration_ms: Some(60_000),
                outcome: HistoryOutcome::Skipped,
                qualified_play: false,
                created_at: base + chrono::Duration::minutes(10),
            })
            .unwrap();
        let third_start = base + chrono::Duration::minutes(50);
        service
            .record(&FinalizedHistory {
                id: PlayHistoryId::new(),
                metadata: metadata_for(track_one, source_one, "One"),
                started_at: third_start,
                ended_at: third_start + chrono::Duration::seconds(3),
                listened_ms: 3_000,
                duration_ms: Some(60_000),
                outcome: HistoryOutcome::Stopped,
                qualified_play: false,
                created_at: third_start,
            })
            .unwrap();

        let overview = service.overview().unwrap();
        assert_eq!(overview.listened_ms, 48_000);
        assert_eq!(overview.qualified_plays, 1);
        assert_eq!(overview.skips, 1);
        assert_eq!(overview.unique_tracks, 2);
        assert_eq!(overview.unique_artists, 1);
        assert_eq!(overview.session_count, 2);
        assert_eq!(service.heatmap().unwrap().len(), 168);
        assert_eq!(service.top_tracks(10).unwrap()[0].track_id, Some(track_one));
        assert_eq!(service.top_artists(10).unwrap()[0].name, "Artist");
        assert_eq!(service.taste_timeline().unwrap().len(), 1);

        let sessions = service.list_sessions(0, 1).unwrap();
        assert_eq!(sessions.total, 2);
        assert_eq!(sessions.items.len(), 1);
        let first_session = first.session_id.unwrap();
        assert_eq!(service.history_for_session(first_session).unwrap().len(), 2);
        let labeled = service
            .set_session_label(first_session, Some("  evening\tset  ".to_owned()))
            .unwrap();
        assert_eq!(labeled.label.as_deref(), Some("evening set"));
        assert!(service
            .set_session_label(first_session, Some("x".repeat(81)))
            .is_err());

        let reopened = service.reopen_session(first_session).unwrap();
        assert_eq!(reopened.entries.len(), 2);
        assert_eq!(reopened.entries[0].track_id, track_one);
        assert_eq!(reopened.entries[1].track_id, track_two);

        database
            .with_connection(|connection| {
                connection.execute("DELETE FROM tracks WHERE id = ?1", [track_two.to_string()])
            })
            .unwrap();
        let reopened_after_delete = service.reopen_session(first_session).unwrap();
        assert_eq!(reopened_after_delete.entries.len(), 1);
        assert_eq!(reopened_after_delete.entries[0].track_id, track_one);
        assert_eq!(reopened_after_delete.dropped_count, 1);
        assert_eq!(service.history_for_session(first_session).unwrap().len(), 2);
    }

    #[test]
    fn private_and_temporary_recorders_do_not_write_private_activity() {
        let database = database("modes");
        let modes = ListeningModeService::new();
        let mut recorder = AnalyticsRecorder::new(database.clone(), modes.clone());
        let base = Instant::now();
        recorder.begin_track(metadata(), Some(60_000), Utc::now(), base);
        recorder.set_phase(RecorderPhase::Playing, base);
        recorder
            .set_private(true, Utc::now(), base + Duration::from_millis(40))
            .unwrap();
        assert_eq!(
            AnalyticsService::new(database.clone())
                .overview()
                .unwrap()
                .session_count,
            1
        );

        modes.enter_temporary().unwrap();
        recorder.begin_track(metadata(), Some(60_000), Utc::now(), base);
        recorder.set_phase(RecorderPhase::Playing, base);
        assert!(recorder
            .finish(
                HistoryOutcome::Completed,
                Utc::now(),
                base + Duration::from_secs(1)
            )
            .unwrap()
            .is_none());
        let overview = AnalyticsService::new(database).overview().unwrap();
        assert_eq!(overview.session_count, 1);
    }
}
