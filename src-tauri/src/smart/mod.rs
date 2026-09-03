//! Typed, local smart playlists and the deterministic local shuffle policy.

use std::collections::{HashSet, VecDeque};

use crate::db::{Database, DatabaseError};
use crate::domain::{ProviderKind, SmartPlaylistId, TrackId};
use crate::sessions::normalize_label;
use chrono::{DateTime, Utc};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_RULE_DEPTH: usize = 4;
const MAX_RULE_NODES: usize = 64;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartSortMode {
    #[default]
    Title,
    Artist,
    DateAdded,
    LastPlayed,
    PlayCount,
    Rating,
    Duration,
    AudioQuality,
}

impl SmartSortMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::DateAdded => "dateAdded",
            Self::LastPlayed => "lastPlayed",
            Self::PlayCount => "playCount",
            Self::Rating => "rating",
            Self::Duration => "duration",
            Self::AudioQuality => "audioQuality",
        }
    }
}

impl TryFrom<String> for SmartSortMode {
    type Error = SmartPlaylistError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "title" => Ok(Self::Title),
            "artist" => Ok(Self::Artist),
            "dateAdded" => Ok(Self::DateAdded),
            "lastPlayed" => Ok(Self::LastPlayed),
            "playCount" => Ok(Self::PlayCount),
            "rating" => Ok(Self::Rating),
            "duration" => Ok(Self::Duration),
            "audioQuality" => Ok(Self::AudioQuality),
            _ => Err(SmartPlaylistError::InvalidInput(format!(
                "unsupported sort mode {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

impl TryFrom<String> for SortDirection {
    type Error = SmartPlaylistError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "asc" => Ok(Self::Asc),
            "desc" => Ok(Self::Desc),
            _ => Err(SmartPlaylistError::InvalidInput(format!(
                "unsupported sort direction {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalOperator {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartField {
    Artist,
    Album,
    Genre,
    Year,
    DateAdded,
    LastPlayed,
    PlayCount,
    SkipCount,
    Rating,
    Liked,
    Downloaded,
    Provider,
    AudioQuality,
    Duration,
    Tag,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartOperation {
    Contains,
    Equals,
    Before,
    After,
    Between,
    Never,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Absent,
    True,
    False,
    Has,
    Lacks,
    Is,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum SmartValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
    Range { from: SmartScalar, to: SmartScalar },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SmartScalar {
    Text(String),
    Integer(i64),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum SmartRule {
    Group {
        operator: LogicalOperator,
        children: Vec<SmartRule>,
    },
    Predicate {
        field: SmartField,
        operation: SmartOperation,
        value: Option<SmartValue>,
    },
}

impl SmartRule {
    pub fn validate(&self) -> Result<(), SmartPlaylistError> {
        let mut nodes = 0;
        validate_rule(self, 0, &mut nodes)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartPlaylist {
    pub id: SmartPlaylistId,
    pub name: String,
    pub rule: SmartRule,
    pub sort_mode: SmartSortMode,
    pub sort_direction: SortDirection,
    pub limit_count: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartPlaylistInput {
    pub name: String,
    pub rule: SmartRule,
    #[serde(default)]
    pub sort_mode: SmartSortMode,
    #[serde(default)]
    pub sort_direction: SortDirection,
    pub limit_count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartTrack {
    pub track_id: TrackId,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub date_added: DateTime<Utc>,
    pub last_played: Option<DateTime<Utc>>,
    pub play_count: u64,
    pub rating: Option<u8>,
    pub audio_quality: AudioQuality,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartPlaylistPreview {
    pub items: Vec<SmartTrack>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Lossless,
    Lossy,
    #[default]
    Unknown,
}

#[derive(Debug, Error)]
pub enum SmartPlaylistError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid smart playlist: {0}")]
    InvalidInput(String),
}

pub struct CompiledRule {
    pub sql: String,
    pub params: Vec<Value>,
}

pub fn compile_rule(rule: &SmartRule) -> Result<CompiledRule, SmartPlaylistError> {
    rule.validate()?;
    let mut params = Vec::new();
    let sql = compile_node(rule, &mut params)?;
    Ok(CompiledRule { sql, params })
}

#[derive(Clone)]
pub struct SmartPlaylistService {
    database: Database,
}

impl SmartPlaylistService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list(&self) -> Result<Vec<SmartPlaylist>, SmartPlaylistError> {
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, name, rule_json, sort_mode, sort_direction, limit_count,
                        created_at, updated_at
                 FROM smart_playlists ORDER BY name COLLATE NOCASE, id",
            )?;
            let rows = statement.query_map([], map_smart_playlist)?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        rows.into_iter().map(parse_smart_playlist).collect()
    }

    pub fn get(&self, id: SmartPlaylistId) -> Result<Option<SmartPlaylist>, SmartPlaylistError> {
        let row = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, name, rule_json, sort_mode, sort_direction, limit_count,
                        created_at, updated_at FROM smart_playlists WHERE id = ?1",
                    [id.to_string()],
                    map_smart_playlist,
                )
                .optional()
        })?;
        row.map(parse_smart_playlist).transpose()
    }

    pub fn create(&self, input: SmartPlaylistInput) -> Result<SmartPlaylist, SmartPlaylistError> {
        let name = normalize_label(input.name, 120).ok_or_else(|| {
            SmartPlaylistError::InvalidInput("name must contain 1..120 characters".to_owned())
        })?;
        validate_limit(input.limit_count)?;
        compile_rule(&input.rule)?;
        let rule_json = serde_json::to_string(&input.rule)?;
        let now = Utc::now();
        let id = SmartPlaylistId::new();
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO smart_playlists
                 (id, name, normalized_name, rule_json, sort_mode, sort_direction,
                  limit_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    id.to_string(),
                    name,
                    name.to_lowercase(),
                    rule_json,
                    input.sort_mode.as_str(),
                    input.sort_direction.as_str(),
                    input.limit_count.map(i64::from),
                    now.to_rfc3339()
                ],
            )?;
            Ok(())
        })?;
        self.get(id)?.ok_or_else(|| {
            SmartPlaylistError::InvalidInput("created playlist disappeared".to_owned())
        })
    }

    pub fn update(
        &self,
        id: SmartPlaylistId,
        input: SmartPlaylistInput,
    ) -> Result<SmartPlaylist, SmartPlaylistError> {
        let name = normalize_label(input.name, 120).ok_or_else(|| {
            SmartPlaylistError::InvalidInput("name must contain 1..120 characters".to_owned())
        })?;
        validate_limit(input.limit_count)?;
        compile_rule(&input.rule)?;
        let rule_json = serde_json::to_string(&input.rule)?;
        let changed = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE smart_playlists SET name = ?1, normalized_name = ?2,
                        rule_json = ?3, sort_mode = ?4, sort_direction = ?5,
                        limit_count = ?6, updated_at = ?7 WHERE id = ?8",
                params![
                    name,
                    name.to_lowercase(),
                    rule_json,
                    input.sort_mode.as_str(),
                    input.sort_direction.as_str(),
                    input.limit_count.map(i64::from),
                    Utc::now().to_rfc3339(),
                    id.to_string()
                ],
            )
        })?;
        if changed == 0 {
            return Err(SmartPlaylistError::InvalidInput(
                "smart playlist was not found".to_owned(),
            ));
        }
        self.get(id)?.ok_or_else(|| {
            SmartPlaylistError::InvalidInput("updated playlist disappeared".to_owned())
        })
    }

    pub fn delete(&self, id: SmartPlaylistId) -> Result<(), SmartPlaylistError> {
        let changed = self.database.with_connection(|connection| {
            connection.execute(
                "DELETE FROM smart_playlists WHERE id = ?1",
                [id.to_string()],
            )
        })?;
        if changed == 0 {
            return Err(SmartPlaylistError::InvalidInput(
                "smart playlist was not found".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn preview(
        &self,
        id: SmartPlaylistId,
        page: u32,
        page_size: u32,
    ) -> Result<SmartPlaylistPreview, SmartPlaylistError> {
        let playlist = self.get(id)?.ok_or_else(|| {
            SmartPlaylistError::InvalidInput("smart playlist was not found".to_owned())
        })?;
        let page_size = page_size.clamp(1, 100);
        let compiled = compile_rule(&playlist.rule)?;
        let offset = i64::from(page).saturating_mul(i64::from(page_size));
        self.preview_compiled(&playlist, compiled, page, page_size, offset)
    }

    pub fn candidates(
        &self,
        pool: &SmartShufflePool,
    ) -> Result<Vec<ShuffleCandidate>, SmartPlaylistError> {
        let mut sql = String::from(
            "SELECT t.id, t.title, t.created_at,
                    (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                     WHERE ta.track_id = t.id ORDER BY ta.artist_order, a.id LIMIT 1),
                    EXISTS(SELECT 1 FROM likes l WHERE l.track_id = t.id),
                    (SELECT rating FROM ratings r WHERE r.track_id = t.id),
                    (SELECT COUNT(*) FROM play_history h WHERE h.track_id = t.id AND h.qualified_play = 1),
                    (SELECT MAX(started_at) FROM play_history h WHERE h.track_id = t.id)
             FROM tracks t WHERE 1 = 1",
        );
        let mut values = Vec::new();
        match pool {
            SmartShufflePool::Library => {}
            SmartShufflePool::Liked => {
                sql.push_str(" AND EXISTS(SELECT 1 FROM likes l WHERE l.track_id = t.id)")
            }
            SmartShufflePool::SmartPlaylist(id) => {
                let playlist = self.get(*id)?.ok_or_else(|| {
                    SmartPlaylistError::InvalidInput("smart playlist was not found".to_owned())
                })?;
                let compiled = compile_rule(&playlist.rule)?;
                sql.push_str(" AND (");
                sql.push_str(&compiled.sql);
                sql.push(')');
                values = compiled.params;
            }
        }
        sql.push_str(" ORDER BY t.id");
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(values), map_candidate)?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        rows.into_iter().map(parse_candidate).collect()
    }

    pub fn generate_smart_mix(
        &self,
        pool: SmartShufflePool,
        options: SmartShuffleOptions,
        seed: Option<u64>,
    ) -> Result<Vec<TrackId>, SmartPlaylistError> {
        options.validate()?;
        let candidates = self.candidates(&pool)?;
        let seed = seed.unwrap_or_else(rand::random);
        Ok(SmartShufflePolicy::order(candidates, options, seed))
    }

    fn preview_compiled(
        &self,
        playlist: &SmartPlaylist,
        compiled: CompiledRule,
        page: u32,
        page_size: u32,
        offset: i64,
    ) -> Result<SmartPlaylistPreview, SmartPlaylistError> {
        let count_sql = format!("SELECT COUNT(*) FROM tracks t WHERE {}", compiled.sql);
        let total = self.database.with_connection(|connection| {
            connection.query_row(
                &count_sql,
                params_from_iter(compiled.params.clone()),
                |row| row.get::<_, i64>(0),
            )
        })?;
        let total = u64::try_from(total)
            .unwrap_or(0)
            .min(u64::from(playlist.limit_count.unwrap_or(u32::MAX)));
        if offset >= i64::try_from(total).unwrap_or(i64::MAX) {
            return Ok(SmartPlaylistPreview {
                items: Vec::new(),
                total,
                page,
                page_size,
            });
        }
        let sql = format!(
            "SELECT t.id, t.title,
                    (SELECT GROUP_CONCAT(a.name, char(31)) FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id),
                    al.title, t.duration_ms, t.created_at,
                    (SELECT MAX(started_at) FROM play_history h WHERE h.track_id = t.id),
                    (SELECT COUNT(*) FROM play_history h WHERE h.track_id = t.id AND h.qualified_play = 1),
                    (SELECT rating FROM ratings r WHERE r.track_id = t.id),
                    (SELECT lf.codec FROM local_files lf JOIN track_sources ts ON ts.id = lf.source_id WHERE ts.track_id = t.id ORDER BY CASE WHEN lower(coalesce(lf.codec, '')) IN ('flac', 'alac', 'wavpack', 'ape') OR lower(coalesce(lf.codec, '')) LIKE 'pcm_%' THEN 2 WHEN lower(coalesce(lf.codec, '')) <> '' THEN 1 ELSE 0 END DESC, lf.source_id LIMIT 1)
             FROM tracks t LEFT JOIN albums al ON al.id = t.album_id WHERE {} ORDER BY {} {} , t.id ASC LIMIT ? OFFSET ?",
            compiled.sql, sort_expression(playlist.sort_mode), playlist.sort_direction.as_str()
        );
        let mut values = compiled.params;
        values.push(Value::Integer(
            i64::from(page_size).min(i64::from(playlist.limit_count.unwrap_or(u32::MAX))),
        ));
        values.push(Value::Integer(offset));
        let rows = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(values), map_smart_track)?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        let items = rows
            .into_iter()
            .map(parse_smart_track)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SmartPlaylistPreview {
            items,
            total,
            page,
            page_size,
        })
    }
}

fn validate_rule(
    rule: &SmartRule,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), SmartPlaylistError> {
    *nodes += 1;
    if *nodes > MAX_RULE_NODES {
        return Err(SmartPlaylistError::InvalidInput(
            "smart rule has more than 64 nodes".to_owned(),
        ));
    }
    match rule {
        SmartRule::Group { children, .. } => {
            if children.is_empty() {
                return Err(SmartPlaylistError::InvalidInput(
                    "smart rule groups need at least one child".to_owned(),
                ));
            }
            if depth >= MAX_RULE_DEPTH {
                return Err(SmartPlaylistError::InvalidInput(
                    "smart rule nesting is deeper than four levels".to_owned(),
                ));
            }
            for child in children {
                validate_rule(child, depth + 1, nodes)?;
            }
        }
        SmartRule::Predicate {
            field,
            operation,
            value,
        } => validate_predicate(*field, *operation, value.as_ref())?,
    }
    Ok(())
}

fn validate_predicate(
    field: SmartField,
    operation: SmartOperation,
    value: Option<&SmartValue>,
) -> Result<(), SmartPlaylistError> {
    let value_required = !matches!(
        operation,
        SmartOperation::Never
            | SmartOperation::Absent
            | SmartOperation::True
            | SmartOperation::False
    );
    if value_required && value.is_none() {
        return Err(SmartPlaylistError::InvalidInput(
            "this smart operation needs a value".to_owned(),
        ));
    }
    if !value_required && value.is_some() {
        return Err(SmartPlaylistError::InvalidInput(
            "this smart operation does not accept a value".to_owned(),
        ));
    }
    let allowed = match field {
        SmartField::Artist | SmartField::Album => {
            matches!(operation, SmartOperation::Contains | SmartOperation::Equals)
        }
        SmartField::Genre => matches!(operation, SmartOperation::Equals),
        SmartField::Year => matches!(operation, SmartOperation::Equals | SmartOperation::Between),
        SmartField::DateAdded => matches!(
            operation,
            SmartOperation::Before | SmartOperation::After | SmartOperation::Between
        ),
        SmartField::LastPlayed => matches!(
            operation,
            SmartOperation::Never
                | SmartOperation::Before
                | SmartOperation::After
                | SmartOperation::Between
        ),
        SmartField::PlayCount | SmartField::SkipCount => matches!(
            operation,
            SmartOperation::Equals
                | SmartOperation::GreaterThanOrEqual
                | SmartOperation::LessThanOrEqual
        ),
        SmartField::Rating => matches!(
            operation,
            SmartOperation::Absent
                | SmartOperation::Equals
                | SmartOperation::GreaterThanOrEqual
                | SmartOperation::LessThanOrEqual
        ),
        SmartField::Liked | SmartField::Downloaded => {
            matches!(operation, SmartOperation::True | SmartOperation::False)
        }
        SmartField::Provider => matches!(operation, SmartOperation::Has),
        SmartField::AudioQuality => matches!(operation, SmartOperation::Is),
        SmartField::Duration => matches!(operation, SmartOperation::Between),
        SmartField::Tag => matches!(operation, SmartOperation::Has | SmartOperation::Lacks),
    };
    if !allowed {
        return Err(SmartPlaylistError::InvalidInput(
            "operation is not supported for this smart field".to_owned(),
        ));
    }
    Ok(())
}

fn compile_node(rule: &SmartRule, values: &mut Vec<Value>) -> Result<String, SmartPlaylistError> {
    match rule {
        SmartRule::Group { operator, children } => {
            let joiner = match operator {
                LogicalOperator::And => " AND ",
                LogicalOperator::Or => " OR ",
            };
            let children = children
                .iter()
                .map(|child| compile_node(child, values))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", children.join(joiner)))
        }
        SmartRule::Predicate {
            field,
            operation,
            value,
        } => compile_predicate(*field, *operation, value.as_ref(), values),
    }
}

fn compile_predicate(
    field: SmartField,
    operation: SmartOperation,
    value: Option<&SmartValue>,
    values: &mut Vec<Value>,
) -> Result<String, SmartPlaylistError> {
    let text = || {
        value
            .and_then(smart_text)
            .ok_or_else(|| SmartPlaylistError::InvalidInput("smart value must be text".to_owned()))
    };
    let integer = || {
        value.and_then(smart_integer).ok_or_else(|| {
            SmartPlaylistError::InvalidInput("smart value must be an integer".to_owned())
        })
    };
    let range = || {
        value.and_then(smart_range).ok_or_else(|| {
            SmartPlaylistError::InvalidInput("smart value must contain from and to".to_owned())
        })
    };
    match (field, operation) {
        (SmartField::Artist, SmartOperation::Contains) | (SmartField::Album, SmartOperation::Contains) => {
            let value = text()?; values.push(Value::Text(format!("%{}%", escape_like(&value))));
            let table = if field == SmartField::Artist { "track_artists ta JOIN artists a ON a.id = ta.artist_id" } else { "albums a" };
            let condition = if field == SmartField::Artist { "a.name LIKE ? ESCAPE '\\'" } else { "a.title LIKE ? ESCAPE '\\'" };
            let relation = if field == SmartField::Artist { "ta.track_id = t.id" } else { "a.id = t.album_id" };
            Ok(format!("EXISTS (SELECT 1 FROM {table} WHERE {relation} AND {condition})"))
        }
        (SmartField::Artist, SmartOperation::Equals) | (SmartField::Album, SmartOperation::Equals) => {
            let value = text()?; values.push(Value::Text(value));
            if field == SmartField::Artist { Ok("EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND a.name = ? COLLATE NOCASE)".to_owned()) }
            else { Ok("EXISTS (SELECT 1 FROM albums a WHERE a.id = t.album_id AND a.title = ? COLLATE NOCASE)".to_owned()) }
        }
        (SmartField::Genre, SmartOperation::Equals) => {
            let value = text()?; values.push(Value::Text(value.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()));
            Ok("EXISTS (SELECT 1 FROM track_genres g WHERE g.track_id = t.id AND g.normalized_genre = ? COLLATE NOCASE)".to_owned())
        }
        (SmartField::Year, SmartOperation::Equals) => {
            let year = text()?; validate_year(&year)?; values.push(Value::Text(year));
            Ok("substr((SELECT a.release_date FROM albums a WHERE a.id = t.album_id), 1, 4) = ?".to_owned())
        }
        (SmartField::Year, SmartOperation::Between) => {
            let (from, to) = range()?;
            let from = scalar_year(from)?;
            let to = scalar_year(to)?;
            if from > to {
                return Err(SmartPlaylistError::InvalidInput(
                    "year range is invalid".to_owned(),
                ));
            }
            values.push(Value::Text(from));
            values.push(Value::Text(to));
            Ok("substr((SELECT a.release_date FROM albums a WHERE a.id = t.album_id), 1, 4) BETWEEN ? AND ?".to_owned())
        }
        (SmartField::DateAdded, op) | (SmartField::LastPlayed, op) => compile_date(field, op, value, values),
        (SmartField::PlayCount, op) | (SmartField::SkipCount, op) => compile_count(field, op, value, values),
        (SmartField::Rating, SmartOperation::Absent) => Ok("NOT EXISTS (SELECT 1 FROM ratings r WHERE r.track_id = t.id)".to_owned()),
        (SmartField::Rating, op) => {
            let n = integer()?; validate_rating(n)?; values.push(Value::Integer(n));
            Ok(format!("COALESCE((SELECT rating FROM ratings r WHERE r.track_id = t.id), 0) {} ?", comparison(op)?))
        }
        (SmartField::Liked, SmartOperation::True) => Ok("EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id)".to_owned()),
        (SmartField::Liked, SmartOperation::False) => Ok("NOT EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id)".to_owned()),
        (SmartField::Downloaded, SmartOperation::True) => Ok("EXISTS (SELECT 1 FROM downloads d WHERE d.target_track_id = t.id AND d.state = 'completed')".to_owned()),
        (SmartField::Downloaded, SmartOperation::False) => Ok("NOT EXISTS (SELECT 1 FROM downloads d WHERE d.target_track_id = t.id AND d.state = 'completed')".to_owned()),
        (SmartField::Provider, SmartOperation::Has) => {
            let provider = text()?; let provider: ProviderKind = provider.parse().map_err(|_| SmartPlaylistError::InvalidInput("provider is invalid".to_owned()))?; values.push(Value::Text(provider.as_str().to_owned()));
            Ok("EXISTS (SELECT 1 FROM track_sources s WHERE s.track_id = t.id AND s.provider_kind = ?)".to_owned())
        }
        (SmartField::AudioQuality, SmartOperation::Is) => {
            let quality = text()?.to_lowercase();
            let expression = lossless_sql("lf.codec");
            match quality.as_str() {
                "lossless" => Ok(format!("EXISTS (SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id WHERE s.track_id = t.id AND ({expression}))")),
                "lossy" => Ok(format!("EXISTS (SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id WHERE s.track_id = t.id AND lower(coalesce(lf.codec, '')) <> '' AND NOT ({expression}))")),
                "unknown" => Ok("NOT EXISTS (SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id WHERE s.track_id = t.id AND lower(coalesce(lf.codec, '')) <> '')".to_owned()),
                _ => Err(SmartPlaylistError::InvalidInput("audio quality must be lossless, lossy, or unknown".to_owned())),
            }
        }
        (SmartField::Duration, SmartOperation::Between) => {
            let (from, to) = range()?; let from = scalar_integer(from)?; let to = scalar_integer(to)?; if from < 0 || to < from { return Err(SmartPlaylistError::InvalidInput("duration range is invalid".to_owned())); } values.push(Value::Integer(from)); values.push(Value::Integer(to));
            Ok("t.duration_ms BETWEEN ? AND ?".to_owned())
        }
        (SmartField::Tag, SmartOperation::Has) | (SmartField::Tag, SmartOperation::Lacks) => {
            let value = text()?; values.push(Value::Text(value));
            let exists = if operation == SmartOperation::Has { "EXISTS" } else { "NOT EXISTS" };
            Ok(format!("{exists} (SELECT 1 FROM track_tags tt JOIN tags tg ON tg.id = tt.tag_id WHERE tt.track_id = t.id AND tg.name = ? COLLATE NOCASE)"))
        }
        _ => Err(SmartPlaylistError::InvalidInput("unsupported smart predicate".to_owned())),
    }
}

fn compile_date(
    field: SmartField,
    operation: SmartOperation,
    value: Option<&SmartValue>,
    values: &mut Vec<Value>,
) -> Result<String, SmartPlaylistError> {
    let expression = if field == SmartField::DateAdded {
        "t.created_at"
    } else {
        "(SELECT MAX(h.started_at) FROM play_history h WHERE h.track_id = t.id)"
    };
    if operation == SmartOperation::Never {
        return Ok(if field == SmartField::DateAdded {
            "0".to_owned()
        } else {
            "NOT EXISTS (SELECT 1 FROM play_history h WHERE h.track_id = t.id)".to_owned()
        });
    }
    if operation == SmartOperation::Between {
        let (from, to) = value
            .and_then(smart_range)
            .ok_or_else(|| SmartPlaylistError::InvalidInput("date range is required".to_owned()))?;
        let from = scalar_text(from)?;
        let to = scalar_text(to)?;
        validate_smart_date(&from)?;
        validate_smart_date(&to)?;
        if from > to {
            return Err(SmartPlaylistError::InvalidInput(
                "date range is invalid".to_owned(),
            ));
        }
        values.push(Value::Text(from));
        values.push(Value::Text(to));
        return Ok(format!("{expression} BETWEEN ? AND ?"));
    }
    let value = value
        .and_then(smart_text)
        .ok_or_else(|| SmartPlaylistError::InvalidInput("date must be text".to_owned()))?;
    validate_smart_date(&value)?;
    values.push(Value::Text(value));
    Ok(format!(
        "{expression} {} ?",
        if operation == SmartOperation::Before {
            "<"
        } else if operation == SmartOperation::After {
            ">"
        } else {
            "="
        }
    ))
}

fn validate_smart_date(value: &str) -> Result<(), SmartPlaylistError> {
    if DateTime::parse_from_rfc3339(value).is_ok()
        || chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
    {
        Ok(())
    } else {
        Err(SmartPlaylistError::InvalidInput(
            "date must be RFC3339 or YYYY-MM-DD".to_owned(),
        ))
    }
}

fn compile_count(
    field: SmartField,
    operation: SmartOperation,
    value: Option<&SmartValue>,
    values: &mut Vec<Value>,
) -> Result<String, SmartPlaylistError> {
    let expression = if field == SmartField::PlayCount {
        "(SELECT COUNT(*) FROM play_history h WHERE h.track_id = t.id AND h.qualified_play = 1)"
    } else {
        "(SELECT COUNT(*) FROM play_history h WHERE h.track_id = t.id AND h.outcome = 'skipped')"
    };
    let count = value
        .and_then(smart_integer)
        .ok_or_else(|| SmartPlaylistError::InvalidInput("count must be an integer".to_owned()))?;
    if count < 0 {
        return Err(SmartPlaylistError::InvalidInput(
            "count must not be negative".to_owned(),
        ));
    }
    values.push(Value::Integer(count));
    Ok(format!("{expression} {} ?", comparison(operation)?))
}

fn comparison(operation: SmartOperation) -> Result<&'static str, SmartPlaylistError> {
    match operation {
        SmartOperation::Equals => Ok("="),
        SmartOperation::GreaterThanOrEqual => Ok(">="),
        SmartOperation::LessThanOrEqual => Ok("<="),
        _ => Err(SmartPlaylistError::InvalidInput(
            "comparison operation is invalid".to_owned(),
        )),
    }
}

fn smart_text(value: &SmartValue) -> Option<String> {
    match value {
        SmartValue::Text(value) => Some(value.clone()),
        _ => None,
    }
}
fn smart_integer(value: &SmartValue) -> Option<i64> {
    match value {
        SmartValue::Integer(value) => Some(*value),
        _ => None,
    }
}
fn smart_range(value: &SmartValue) -> Option<(&SmartScalar, &SmartScalar)> {
    match value {
        SmartValue::Range { from, to } => Some((from, to)),
        _ => None,
    }
}
fn scalar_text(value: &SmartScalar) -> Result<String, SmartPlaylistError> {
    match value {
        SmartScalar::Text(value) => Ok(value.clone()),
        SmartScalar::Integer(value) => Ok(value.to_string()),
    }
}
fn scalar_integer(value: &SmartScalar) -> Result<i64, SmartPlaylistError> {
    match value {
        SmartScalar::Integer(value) => Ok(*value),
        SmartScalar::Text(value) => value.parse().map_err(|_| {
            SmartPlaylistError::InvalidInput("range value must be an integer".to_owned())
        }),
    }
}
fn scalar_year(value: &SmartScalar) -> Result<String, SmartPlaylistError> {
    let value = scalar_text(value)?;
    validate_year(&value)?;
    Ok(value)
}
fn validate_year(value: &str) -> Result<(), SmartPlaylistError> {
    if value.len() != 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        Err(SmartPlaylistError::InvalidInput(
            "year must be a four-digit value".to_owned(),
        ))
    } else if value
        .parse::<u16>()
        .ok()
        .is_none_or(|year| !(1..=9999).contains(&year))
    {
        Err(SmartPlaylistError::InvalidInput(
            "year must be a valid four-digit value".to_owned(),
        ))
    } else {
        Ok(())
    }
}
fn validate_rating(value: i64) -> Result<(), SmartPlaylistError> {
    if (1..=5).contains(&value) {
        Ok(())
    } else {
        Err(SmartPlaylistError::InvalidInput(
            "rating must be between 1 and 5".to_owned(),
        ))
    }
}
fn validate_limit(value: Option<u32>) -> Result<(), SmartPlaylistError> {
    if value.is_some_and(|value| !(1..=5000).contains(&value)) {
        Err(SmartPlaylistError::InvalidInput(
            "limit must be between 1 and 5000".to_owned(),
        ))
    } else {
        Ok(())
    }
}
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
fn lossless_sql(codec: &str) -> String {
    format!("lower(coalesce({codec}, '')) IN ('flac', 'alac', 'wavpack', 'ape') OR lower(coalesce({codec}, '')) LIKE 'pcm_%'")
}

fn sort_expression(mode: SmartSortMode) -> &'static str {
    match mode {
        SmartSortMode::Title => "lower(t.title)",
        SmartSortMode::Artist => "lower(COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY ta.artist_order, a.id LIMIT 1), ''))",
        SmartSortMode::DateAdded => "t.created_at",
        SmartSortMode::LastPlayed => "COALESCE((SELECT MAX(h.started_at) FROM play_history h WHERE h.track_id = t.id), '')",
        SmartSortMode::PlayCount => "(SELECT COUNT(*) FROM play_history h WHERE h.track_id = t.id AND h.qualified_play = 1)",
        SmartSortMode::Rating => "COALESCE((SELECT rating FROM ratings r WHERE r.track_id = t.id), 0)",
        SmartSortMode::Duration => "COALESCE(t.duration_ms, 0)",
        SmartSortMode::AudioQuality => "CASE WHEN EXISTS(SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id WHERE s.track_id = t.id AND (lower(coalesce(lf.codec, '')) IN ('flac', 'alac', 'wavpack', 'ape') OR lower(coalesce(lf.codec, '')) LIKE 'pcm_%')) THEN 2 WHEN EXISTS(SELECT 1 FROM local_files lf JOIN track_sources s ON s.id = lf.source_id WHERE s.track_id = t.id AND lower(coalesce(lf.codec, '')) <> '') THEN 1 ELSE 0 END",
    }
}

type RawSmartPlaylist = (
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    String,
    String,
);
fn map_smart_playlist(row: &Row<'_>) -> Result<RawSmartPlaylist, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}
fn parse_smart_playlist(raw: RawSmartPlaylist) -> Result<SmartPlaylist, SmartPlaylistError> {
    Ok(SmartPlaylist {
        id: raw.0.parse().map_err(|_| {
            SmartPlaylistError::InvalidInput("invalid smart playlist id".to_owned())
        })?,
        name: raw.1,
        rule: serde_json::from_str(&raw.2)?,
        sort_mode: SmartSortMode::try_from(raw.3)?,
        sort_direction: SortDirection::try_from(raw.4)?,
        limit_count: raw
            .5
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    SmartPlaylistError::InvalidInput("invalid smart playlist limit".to_owned())
                })
            })
            .transpose()?,
        created_at: DateTime::parse_from_rfc3339(&raw.6)
            .map_err(|error| SmartPlaylistError::InvalidInput(error.to_string()))?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&raw.7)
            .map_err(|error| SmartPlaylistError::InvalidInput(error.to_string()))?
            .with_timezone(&Utc),
    })
}

type RawSmartTrack = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<i64>,
    String,
    Option<String>,
    i64,
    Option<i64>,
    Option<String>,
);
fn map_smart_track(row: &Row<'_>) -> Result<RawSmartTrack, rusqlite::Error> {
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
    ))
}
fn parse_smart_track(raw: RawSmartTrack) -> Result<SmartTrack, SmartPlaylistError> {
    let artists = raw
        .2
        .map(|value| {
            value
                .split('\u{1f}')
                .map(str::to_owned)
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let codec = raw.9.unwrap_or_default().to_lowercase();
    let audio_quality = if ["flac", "alac", "wavpack", "ape"].contains(&codec.as_str())
        || codec.starts_with("pcm_")
    {
        AudioQuality::Lossless
    } else if codec.is_empty() {
        AudioQuality::Unknown
    } else {
        AudioQuality::Lossy
    };
    Ok(SmartTrack {
        track_id: raw
            .0
            .parse()
            .map_err(|_| SmartPlaylistError::InvalidInput("invalid smart track id".to_owned()))?,
        title: raw.1,
        artists,
        album: raw.3,
        duration_ms: raw
            .4
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| SmartPlaylistError::InvalidInput("invalid duration".to_owned()))
            })
            .transpose()?,
        date_added: DateTime::parse_from_rfc3339(&raw.5)
            .map_err(|error| SmartPlaylistError::InvalidInput(error.to_string()))?
            .with_timezone(&Utc),
        last_played: raw
            .6
            .map(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(|error| SmartPlaylistError::InvalidInput(error.to_string()))
            })
            .transpose()?,
        play_count: u64::try_from(raw.7)
            .map_err(|_| SmartPlaylistError::InvalidInput("invalid play count".to_owned()))?,
        rating: raw
            .8
            .map(|value| {
                u8::try_from(value)
                    .map_err(|_| SmartPlaylistError::InvalidInput("invalid rating".to_owned()))
            })
            .transpose()?,
        audio_quality,
    })
}

type RawCandidate = (
    String,
    String,
    String,
    Option<String>,
    i64,
    Option<i64>,
    i64,
    Option<String>,
);
fn map_candidate(row: &Row<'_>) -> Result<RawCandidate, rusqlite::Error> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}
fn parse_candidate(raw: RawCandidate) -> Result<ShuffleCandidate, SmartPlaylistError> {
    Ok(ShuffleCandidate {
        track_id: raw
            .0
            .parse()
            .map_err(|_| SmartPlaylistError::InvalidInput("invalid shuffle track id".to_owned()))?,
        title: raw.1,
        date_added: DateTime::parse_from_rfc3339(&raw.2)
            .map_err(|error| SmartPlaylistError::InvalidInput(error.to_string()))?
            .with_timezone(&Utc),
        primary_artist: raw.3,
        liked: raw.4 != 0,
        rating: raw.5.map(|value| u8::try_from(value).unwrap_or(0)),
        qualified_play_count: u64::try_from(raw.6).unwrap_or(0),
        last_played: raw.7.and_then(|value| {
            DateTime::parse_from_rfc3339(&value)
                .ok()
                .map(|value| value.with_timezone(&Utc))
        }),
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartShufflePool {
    Library,
    Liked,
    SmartPlaylist(SmartPlaylistId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartShuffleOptions {
    pub familiarity: u8,
    pub variety: u8,
    pub freshness: u8,
    pub count: u16,
    #[serde(default)]
    pub recent_track_ids: Vec<TrackId>,
}

impl Default for SmartShuffleOptions {
    fn default() -> Self {
        Self {
            familiarity: 50,
            variety: 50,
            freshness: 50,
            count: 25,
            recent_track_ids: Vec::new(),
        }
    }
}
impl SmartShuffleOptions {
    pub fn validate(&self) -> Result<(), SmartPlaylistError> {
        if self.familiarity > 100
            || self.variety > 100
            || self.freshness > 100
            || !(1..=1000).contains(&self.count)
        {
            Err(SmartPlaylistError::InvalidInput(
                "smart shuffle options are out of range".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct ShuffleCandidate {
    pub track_id: TrackId,
    pub title: String,
    pub date_added: DateTime<Utc>,
    pub primary_artist: Option<String>,
    pub liked: bool,
    pub rating: Option<u8>,
    pub qualified_play_count: u64,
    pub last_played: Option<DateTime<Utc>>,
}

pub struct SmartShufflePolicy;

impl SmartShufflePolicy {
    pub fn order(
        mut candidates: Vec<ShuffleCandidate>,
        options: SmartShuffleOptions,
        seed: u64,
    ) -> Vec<TrackId> {
        candidates.sort_by_key(|candidate| candidate.track_id.to_string());
        let mut seen = HashSet::new();
        let mut remaining = candidates
            .into_iter()
            .filter(|candidate| seen.insert(candidate.track_id))
            .collect::<Vec<_>>();
        let count = usize::from(options.count).min(remaining.len());
        let mut rng = SmallRng::seed_from_u64(seed);
        let artist_window = 1 + (usize::from(options.variety) * 4 / 100);
        let mut selected = Vec::with_capacity(count);
        let mut recent_tracks = VecDeque::from(options.recent_track_ids.clone());
        let mut recent_artists = VecDeque::new();
        while selected.len() < count && !remaining.is_empty() {
            let track_filtered = if remaining.len() > 1 {
                remaining
                    .iter()
                    .filter(|candidate| !recent_tracks.contains(&candidate.track_id))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let mut pool = if track_filtered.is_empty() {
                remaining.clone()
            } else {
                track_filtered
            };
            let artist_filtered = pool
                .iter()
                .filter(|candidate| {
                    candidate.primary_artist.as_ref().is_none_or(|artist| {
                        !recent_artists
                            .iter()
                            .any(|seen: &String| seen.eq_ignore_ascii_case(artist))
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            if !artist_filtered.is_empty() {
                pool = artist_filtered;
            }
            let index = weighted_index(&pool, &remaining, &options, &mut rng);
            let chosen = pool[index].track_id;
            let position = remaining
                .iter()
                .position(|candidate| candidate.track_id == chosen)
                .unwrap();
            let chosen = remaining.remove(position);
            recent_tracks.push_back(chosen.track_id);
            while recent_tracks.len() > 20 {
                recent_tracks.pop_front();
            }
            if let Some(artist) = chosen.primary_artist {
                recent_artists.push_back(artist);
                while recent_artists.len() > artist_window {
                    recent_artists.pop_front();
                }
            }
            selected.push(chosen.track_id);
        }
        selected
    }
}

fn weighted_index(
    pool: &[ShuffleCandidate],
    all: &[ShuffleCandidate],
    options: &SmartShuffleOptions,
    rng: &mut SmallRng,
) -> usize {
    let weights = pool
        .iter()
        .map(|candidate| score(candidate, options, all))
        .collect::<Vec<_>>();
    let total = weights.iter().sum::<f64>();
    if total <= f64::EPSILON {
        return rng.random_range(0..pool.len());
    }
    let mut target = rng.random::<f64>() * total;
    for (index, weight) in weights.iter().enumerate() {
        if target <= *weight {
            return index;
        }
        target -= *weight;
    }
    pool.len() - 1
}

fn score(
    candidate: &ShuffleCandidate,
    options: &SmartShuffleOptions,
    all: &[ShuffleCandidate],
) -> f64 {
    let max_plays = all
        .iter()
        .map(|candidate| candidate.qualified_play_count)
        .max()
        .unwrap_or(0)
        .max(1) as f64;
    let familiarity_signal = candidate.qualified_play_count as f64 / max_plays;
    let familiarity = familiarity_signal * f64::from(options.familiarity) / 100.0
        + (1.0 - familiarity_signal) * f64::from(100 - options.familiarity) / 100.0;
    let latest_played = all
        .iter()
        .filter_map(|candidate| candidate.last_played)
        .max();
    let freshness_signal = candidate
        .last_played
        .zip(latest_played)
        .map(|(last, latest)| {
            let days = latest.signed_duration_since(last).num_days().max(0) as f64;
            (days / 30.0).min(1.0)
        })
        .unwrap_or(1.0);
    let freshness = 1.0 + freshness_signal * f64::from(options.freshness) / 100.0;
    let affinity = 1.0
        + if candidate.liked { 0.15 } else { 0.0 }
        + f64::from(candidate.rating.unwrap_or(0)) * 0.03;
    (0.1 + familiarity + freshness) * affinity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::domain::{SourceId, TrackId};
    use rusqlite::params;
    use uuid::Uuid;

    fn rule() -> SmartRule {
        SmartRule::Predicate {
            field: SmartField::Genre,
            operation: SmartOperation::Equals,
            value: Some(SmartValue::Text("Rock".to_owned())),
        }
    }

    fn database(label: &str) -> Database {
        let path =
            std::env::temp_dir().join(format!("spotdiy-smart-{label}-{}.sqlite3", Uuid::new_v4()));
        Database::open(path).unwrap()
    }

    fn insert_fixture_track(database: &Database) -> (TrackId, SourceId) {
        let track_id = TrackId::new();
        let source_id = SourceId::new();
        let artist_id = Uuid::new_v4();
        let album_id = Uuid::new_v4();
        let history_id = Uuid::new_v4();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO albums (id, title, release_date, created_at, updated_at)
                     VALUES (?1, 'Album', '2020-05-01', '2026-01-01T00:00:00Z',
                             '2026-01-01T00:00:00Z')",
                    [album_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO tracks
                     (id, title, normalized_title, album_id, duration_ms,
                      created_at, updated_at)
                     VALUES (?1, 'Fixture', 'fixture', ?2, 60000,
                             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    params![track_id.to_string(), album_id.to_string()],
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
                    "INSERT INTO local_files
                     (source_id, path, codec, index_status, created_at, updated_at)
                     VALUES (?1, ?2, 'FLAC', 'indexed',
                             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    params![
                        source_id.to_string(),
                        format!("C:/fixture-{source_id}.flac")
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
                connection.execute(
                    "INSERT INTO track_genres (track_id, genre, normalized_genre)
                     VALUES (?1, 'Rock', 'rock')",
                    [track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO likes (track_id, liked_at)
                     VALUES (?1, '2026-01-01T00:00:00Z')",
                    [track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO ratings (track_id, rating, updated_at)
                     VALUES (?1, 5, '2026-01-01T00:00:00Z')",
                    [track_id.to_string()],
                )?;
                connection.execute(
                    "INSERT INTO play_history
                     (id, track_id, source_id, title_snapshot, artists_json,
                      started_at, ended_at, local_date, local_hour, local_weekday,
                      listened_ms, duration_ms, outcome, qualified_play, created_at)
                     VALUES (?1, ?2, ?3, 'Fixture', '[\"Artist\"]',
                             '2026-01-02T00:00:00Z', '2026-01-02T00:01:00Z',
                             '2026-01-02', 0, 5, 60000, 60000, 'completed', 1,
                             '2026-01-02T00:01:00Z')",
                    params![
                        history_id.to_string(),
                        track_id.to_string(),
                        source_id.to_string()
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        (track_id, source_id)
    }

    #[test]
    fn compiler_uses_a_parameter_for_sql_looking_text() {
        let compiled = compile_rule(&rule()).unwrap();
        assert!(!compiled.sql.contains("Rock"));
        assert_eq!(compiled.params, vec![Value::Text("rock".to_owned())]);
    }

    #[test]
    fn rule_limits_are_enforced() {
        let rule = SmartRule::Group {
            operator: LogicalOperator::And,
            children: Vec::new(),
        };
        assert!(rule.validate().is_err());
    }

    #[test]
    fn seeded_shuffle_is_deterministic_and_has_no_immediate_repeat() {
        let candidates = (0..8)
            .map(|index| ShuffleCandidate {
                track_id: TrackId::new(),
                title: index.to_string(),
                date_added: Utc::now(),
                primary_artist: Some(format!("artist-{index}")),
                liked: false,
                rating: None,
                qualified_play_count: index,
                last_played: None,
            })
            .collect::<Vec<_>>();
        let options = SmartShuffleOptions {
            count: 8,
            ..Default::default()
        };
        let first = SmartShufflePolicy::order(candidates.clone(), options.clone(), 7);
        assert_eq!(first, SmartShufflePolicy::order(candidates, options, 7));
        assert!(first.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn every_supported_predicate_family_compiles_with_typed_values() {
        let predicates = vec![
            SmartRule::Predicate {
                field: SmartField::Artist,
                operation: SmartOperation::Contains,
                value: Some(SmartValue::Text("artist".to_owned())),
            },
            SmartRule::Predicate {
                field: SmartField::Album,
                operation: SmartOperation::Equals,
                value: Some(SmartValue::Text("album".to_owned())),
            },
            rule(),
            SmartRule::Predicate {
                field: SmartField::Year,
                operation: SmartOperation::Between,
                value: Some(SmartValue::Range {
                    from: SmartScalar::Text("1980".to_owned()),
                    to: SmartScalar::Text("2020".to_owned()),
                }),
            },
            SmartRule::Predicate {
                field: SmartField::DateAdded,
                operation: SmartOperation::Between,
                value: Some(SmartValue::Range {
                    from: SmartScalar::Text("2020-01-01".to_owned()),
                    to: SmartScalar::Text("2026-01-01".to_owned()),
                }),
            },
            SmartRule::Predicate {
                field: SmartField::LastPlayed,
                operation: SmartOperation::Never,
                value: None,
            },
            SmartRule::Predicate {
                field: SmartField::PlayCount,
                operation: SmartOperation::GreaterThanOrEqual,
                value: Some(SmartValue::Integer(1)),
            },
            SmartRule::Predicate {
                field: SmartField::SkipCount,
                operation: SmartOperation::LessThanOrEqual,
                value: Some(SmartValue::Integer(2)),
            },
            SmartRule::Predicate {
                field: SmartField::Rating,
                operation: SmartOperation::Absent,
                value: None,
            },
            SmartRule::Predicate {
                field: SmartField::Liked,
                operation: SmartOperation::True,
                value: None,
            },
            SmartRule::Predicate {
                field: SmartField::Downloaded,
                operation: SmartOperation::False,
                value: None,
            },
            SmartRule::Predicate {
                field: SmartField::Provider,
                operation: SmartOperation::Has,
                value: Some(SmartValue::Text("local".to_owned())),
            },
            SmartRule::Predicate {
                field: SmartField::AudioQuality,
                operation: SmartOperation::Is,
                value: Some(SmartValue::Text("lossless".to_owned())),
            },
            SmartRule::Predicate {
                field: SmartField::Duration,
                operation: SmartOperation::Between,
                value: Some(SmartValue::Range {
                    from: SmartScalar::Integer(1),
                    to: SmartScalar::Integer(120_000),
                }),
            },
            SmartRule::Predicate {
                field: SmartField::Tag,
                operation: SmartOperation::Lacks,
                value: Some(SmartValue::Text("live".to_owned())),
            },
        ];
        for predicate in predicates {
            assert!(
                compile_rule(&predicate).is_ok(),
                "predicate did not compile: {predicate:?}"
            );
        }
        assert!(compile_rule(&SmartRule::Predicate {
            field: SmartField::DateAdded,
            operation: SmartOperation::Never,
            value: None,
        })
        .is_err());
        assert!(compile_rule(&SmartRule::Predicate {
            field: SmartField::LastPlayed,
            operation: SmartOperation::Between,
            value: Some(SmartValue::Range {
                from: SmartScalar::Text("invalid".to_owned()),
                to: SmartScalar::Text("2026-01-01".to_owned()),
            }),
        })
        .is_err());
    }

    #[test]
    fn audio_quality_predicates_execute_without_cross_track_or_sql_errors() {
        let database = database("audio-quality");
        let (_, flac_source) = insert_fixture_track(&database);
        let (_, mp3_source) = insert_fixture_track(&database);
        let (_, pcm_source) = insert_fixture_track(&database);
        database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE local_files SET codec = 'MP3' WHERE source_id = ?1",
                    [mp3_source.to_string()],
                )?;
                connection.execute(
                    "UPDATE local_files SET codec = 'PCM_S16LE' WHERE source_id = ?1",
                    [pcm_source.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        for (quality, expected) in [("lossless", 2_i64), ("lossy", 1), ("unknown", 0)] {
            let compiled = compile_rule(&SmartRule::Predicate {
                field: SmartField::AudioQuality,
                operation: SmartOperation::Is,
                value: Some(SmartValue::Text(quality.to_owned())),
            })
            .unwrap();
            let sql = format!("SELECT COUNT(*) FROM tracks t WHERE {}", compiled.sql);
            let count = database
                .with_connection(|connection| {
                    connection.query_row(&sql, params_from_iter(compiled.params), |row| {
                        row.get::<_, i64>(0)
                    })
                })
                .unwrap();
            assert_eq!(
                count, expected,
                "unexpected audio quality count for {quality}"
            );
        }
        assert_ne!(flac_source, mp3_source);
        assert_ne!(mp3_source, pcm_source);
    }

    #[test]
    fn smart_playlist_crud_preview_and_limit_are_database_backed() {
        let database = database("crud");
        let (track_id, _) = insert_fixture_track(&database);
        let service = SmartPlaylistService::new(database);
        let input = SmartPlaylistInput {
            name: "  Rock  ".to_owned(),
            rule: rule(),
            sort_mode: SmartSortMode::Rating,
            sort_direction: SortDirection::Desc,
            limit_count: Some(1),
        };
        let created = service.create(input.clone()).unwrap();
        assert_eq!(created.name, "Rock");
        assert_eq!(service.list().unwrap().len(), 1);
        assert_eq!(service.get(created.id).unwrap().unwrap().id, created.id);

        let preview = service.preview(created.id, 0, 100).unwrap();
        assert_eq!(preview.total, 1);
        assert_eq!(preview.items.len(), 1);
        assert_eq!(preview.items[0].track_id, track_id);
        assert_eq!(preview.items[0].audio_quality, AudioQuality::Lossless);
        assert_eq!(preview.items[0].play_count, 1);
        assert!(preview.items[0].last_played.is_some());

        let updated = service
            .update(
                created.id,
                SmartPlaylistInput {
                    name: "Rock updated".to_owned(),
                    rule: SmartRule::Predicate {
                        field: SmartField::Liked,
                        operation: SmartOperation::True,
                        value: None,
                    },
                    ..input
                },
            )
            .unwrap();
        assert_eq!(updated.name, "Rock updated");
        service.delete(created.id).unwrap();
        assert!(service.list().unwrap().is_empty());
    }
}
