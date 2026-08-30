use std::path::PathBuf;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json;
use thiserror::Error;
use url::Url;

use crate::domain::{
    Album, AlbumId, Artist, ArtistId, LocalFileSource, ProviderKind, SourceId, TrackId,
    TrackSource, UnifiedTrack, VersionInfo, VersionQualifier,
};

use super::{Database, DatabaseError};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("database operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not serialize {entity}: {source}")]
    Serialization {
        entity: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not deserialize {entity}: {source}")]
    Deserialization {
        entity: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid {field} value {value}")]
    InvalidValue { field: &'static str, value: String },
    #[error("track {0} was not found")]
    TrackNotFound(TrackId),
    #[error("source {source_id} was not found for track {track_id}")]
    SourceNotFoundForTrack {
        track_id: TrackId,
        source_id: SourceId,
    },
    #[error("provider source identity {provider_kind}:{provider_item_id} is already attached to track {track_id}")]
    DuplicateProviderIdentity {
        provider_kind: ProviderKind,
        provider_item_id: String,
        track_id: TrackId,
    },
    #[error("a local file record is required for local source {source_id}")]
    MissingLocalFile { source_id: SourceId },
    #[error("a local file record cannot be attached to non-local source {source_id}")]
    UnexpectedLocalFile { source_id: SourceId },
    #[error("local file record {source_id} does not match its source")]
    LocalFileIdMismatch { source_id: SourceId },
    #[error("source {source_id} belongs to track {actual_track_id}, not {expected_track_id}")]
    SourceTrackMismatch {
        source_id: SourceId,
        actual_track_id: TrackId,
        expected_track_id: TrackId,
    },
    #[error("numeric value for {field} is outside SQLite's supported range")]
    NumericOverflow { field: &'static str },
}

pub struct TrackRepository<'database> {
    database: &'database Database,
}

impl<'database> TrackRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn create(&self, track: &UnifiedTrack) -> Result<(), RepositoryError> {
        validate_track(track)?;
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;

        for source in &track.sources {
            let existing = transaction
                .query_row(
                    "SELECT id, track_id FROM track_sources WHERE provider_kind = ?1 AND provider_item_id = ?2",
                    params![source.provider_kind.as_str(), source.provider_item_id],
                    |row| {
                        let source_id: String = row.get(0)?;
                        let track_id: String = row.get(1)?;
                        Ok((source_id, track_id))
                    },
                )
                .optional()?;
            if let Some((_, existing_track_id)) = existing {
                return Err(RepositoryError::DuplicateProviderIdentity {
                    provider_kind: source.provider_kind,
                    provider_item_id: source.provider_item_id.clone(),
                    track_id: parse_id(&existing_track_id, "track_id")?,
                });
            }
        }

        if let Some(album) = &track.album {
            transaction.execute(
                "INSERT INTO albums (id, title, release_date, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO NOTHING",
                params![
                    album.id.to_string(),
                    album.title,
                    album.release_date,
                    timestamp(album.created_at),
                    timestamp(album.updated_at),
                ],
            )?;
        }

        transaction.execute(
            "INSERT INTO tracks (id, title, normalized_title, album_id, duration_ms, version_qualifiers_json, preferred_source_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
            params![
                track.id.to_string(),
                track.title,
                track.normalized_title,
                track.album.as_ref().map(|album| album.id.to_string()),
                track.duration_ms.map(|value| numeric_i64(value, "track.duration_ms")).transpose()?,
                version_json(&track.version)?,
                timestamp(track.created_at),
                timestamp(track.updated_at),
            ],
        )?;

        for (artist_order, artist) in track.artists.iter().enumerate() {
            transaction.execute(
                "INSERT INTO artists (id, name, sort_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO NOTHING",
                params![
                    artist.id.to_string(),
                    artist.name,
                    artist.sort_name,
                    timestamp(artist.created_at),
                    timestamp(artist.updated_at),
                ],
            )?;
            transaction.execute(
                "INSERT INTO track_artists (track_id, artist_id, artist_order, role) VALUES (?1, ?2, ?3, 'primary')",
                params![track.id.to_string(), artist.id.to_string(), artist_order as i64],
            )?;
        }

        for source in &track.sources {
            let insert_result = transaction.execute(
                "INSERT INTO track_sources (
                    id, track_id, provider_kind, provider_item_id, source_uri, duration_ms,
                    version_qualifiers_json, available, availability_detail, can_search,
                    can_metadata, can_artwork, can_playback, can_lyrics, can_downloads,
                    can_popularity, can_release_date, can_lyrics_metadata, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
                )",
                params![
                    source.id.to_string(),
                    source.track_id.to_string(),
                    source.provider_kind.as_str(),
                    source.provider_item_id,
                    source.source_uri.as_ref().map(ToString::to_string),
                    source
                        .duration_ms
                        .map(|value| numeric_i64(value, "source.duration_ms"))
                        .transpose()?,
                    version_json(&source.version)?,
                    bool_integer(source.available),
                    source.availability_detail,
                    bool_integer(source.capabilities.search),
                    bool_integer(source.capabilities.metadata),
                    bool_integer(source.capabilities.artwork),
                    bool_integer(source.capabilities.playback),
                    bool_integer(source.capabilities.lyrics),
                    bool_integer(source.capabilities.downloads),
                    bool_integer(source.capabilities.popularity),
                    bool_integer(source.capabilities.release_date),
                    bool_integer(source.capabilities.lyrics_metadata),
                    timestamp(source.created_at),
                    timestamp(source.updated_at),
                ],
            );
            insert_result.map_err(|error| {
                if is_unique_constraint(&error) {
                    RepositoryError::DuplicateProviderIdentity {
                        provider_kind: source.provider_kind,
                        provider_item_id: source.provider_item_id.clone(),
                        track_id: track.id,
                    }
                } else {
                    RepositoryError::Sqlite(error)
                }
            })?;

            if let Some(local_file) = &source.local_file {
                transaction.execute(
                    "INSERT INTO local_files (
                        source_id, path, file_size_bytes, modified_at, content_fingerprint,
                        codec, bitrate_kbps, sample_rate_hz, bit_depth, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        local_file.source_id.to_string(),
                        local_file.path.to_string_lossy().into_owned(),
                        local_file
                            .file_size_bytes
                            .map(|value| numeric_i64(value, "local_file.file_size_bytes"))
                            .transpose()?,
                        local_file.modified_at.map(timestamp),
                        local_file.content_fingerprint,
                        local_file.codec,
                        local_file
                            .bitrate_kbps
                            .map(|value| numeric_i64(value, "local_file.bitrate_kbps"))
                            .transpose()?,
                        local_file
                            .sample_rate_hz
                            .map(|value| numeric_i64(value, "local_file.sample_rate_hz"))
                            .transpose()?,
                        local_file
                            .bit_depth
                            .map(|value| numeric_i64(value.into(), "local_file.bit_depth"))
                            .transpose()?,
                        timestamp(source.created_at),
                        timestamp(source.updated_at),
                    ],
                )?;
            }
        }

        if let Some(preferred_source_id) = track.preferred_source_id {
            let source_track_id: Option<String> = transaction
                .query_row(
                    "SELECT track_id FROM track_sources WHERE id = ?1",
                    params![preferred_source_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(source_track_id) = source_track_id else {
                return Err(RepositoryError::SourceNotFoundForTrack {
                    track_id: track.id,
                    source_id: preferred_source_id,
                });
            };
            let source_track_id = parse_id(&source_track_id, "track_id")?;
            if source_track_id != track.id {
                return Err(RepositoryError::SourceTrackMismatch {
                    source_id: preferred_source_id,
                    actual_track_id: source_track_id,
                    expected_track_id: track.id,
                });
            }
            transaction.execute(
                "UPDATE tracks SET preferred_source_id = ?1 WHERE id = ?2",
                params![preferred_source_id.to_string(), track.id.to_string()],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn get(&self, track_id: TrackId) -> Result<Option<UnifiedTrack>, RepositoryError> {
        let connection = self.database.connection()?;
        let track_row = connection
            .query_row(
                "SELECT id, title, normalized_title, album_id, duration_ms, version_qualifiers_json, preferred_source_id, created_at, updated_at
                 FROM tracks WHERE id = ?1",
                params![track_id.to_string()],
                map_track_row_raw,
            )
            .optional()?
            .map(parse_track_row)
            .transpose()?;
        let Some(track_row) = track_row else {
            return Ok(None);
        };

        let album = track_row
            .album_id
            .map(|album_id| load_album(&connection, album_id))
            .transpose()?;
        let artists = load_artists(&connection, track_id)?;
        let sources = load_sources(&connection, track_id)?;

        Ok(Some(UnifiedTrack {
            id: track_row.id,
            title: track_row.title,
            normalized_title: track_row.normalized_title,
            artists,
            album,
            duration_ms: track_row.duration_ms,
            version: track_row.version,
            sources,
            preferred_source_id: track_row.preferred_source_id,
            created_at: track_row.created_at,
            updated_at: track_row.updated_at,
        }))
    }

    pub fn count(&self) -> Result<u64, RepositoryError> {
        let connection = self.database.connection()?;
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
        u64::try_from(count).map_err(|_| RepositoryError::InvalidValue {
            field: "tracks.count",
            value: count.to_string(),
        })
    }
}

pub struct ArtistRepository<'database> {
    database: &'database Database,
}

impl<'database> ArtistRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn get(&self, artist_id: ArtistId) -> Result<Option<Artist>, RepositoryError> {
        let connection = self.database.connection()?;
        let artist = connection
            .query_row(
                "SELECT id, name, sort_name, created_at, updated_at FROM artists WHERE id = ?1",
                params![artist_id.to_string()],
                map_artist_row_raw,
            )
            .optional()
            .map_err(RepositoryError::from)?;
        artist.map(parse_artist_row).transpose()
    }
}

pub struct SourceRepository<'database> {
    database: &'database Database,
}

impl<'database> SourceRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn get(&self, source_id: SourceId) -> Result<Option<TrackSource>, RepositoryError> {
        let connection = self.database.connection()?;
        let source = connection
            .query_row(
                "SELECT id, track_id, provider_kind, provider_item_id, source_uri, duration_ms,
                        version_qualifiers_json, available, availability_detail, can_search,
                        can_metadata, can_artwork, can_playback, can_lyrics, can_downloads,
                        can_popularity, can_release_date, can_lyrics_metadata, created_at, updated_at
                 FROM track_sources WHERE id = ?1",
                params![source_id.to_string()],
                map_source_row_raw,
            )
            .optional()
            .map_err(RepositoryError::from)?;
        let mut source = source.map(parse_source_row).transpose()?;
        if let Some(source) = &mut source {
            source.local_file = load_local_file(&connection, source.id)?;
        }
        Ok(source)
    }
}

struct TrackRow {
    id: TrackId,
    title: String,
    normalized_title: String,
    album_id: Option<AlbumId>,
    duration_ms: Option<u64>,
    version: VersionInfo,
    preferred_source_id: Option<SourceId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn validate_track(track: &UnifiedTrack) -> Result<(), RepositoryError> {
    if track
        .artists
        .iter()
        .any(|artist| artist.name.trim().is_empty())
    {
        return Err(RepositoryError::InvalidValue {
            field: "artist.name",
            value: String::new(),
        });
    }
    if track.title.trim().is_empty() || track.normalized_title.trim().is_empty() {
        return Err(RepositoryError::InvalidValue {
            field: "track.title",
            value: track.title.clone(),
        });
    }
    for source in &track.sources {
        if source.track_id != track.id {
            return Err(RepositoryError::SourceTrackMismatch {
                source_id: source.id,
                actual_track_id: source.track_id,
                expected_track_id: track.id,
            });
        }
        if source.provider_kind == ProviderKind::Spotify
            && (source.capabilities.playback
                || source.capabilities.downloads
                || source.capabilities.lyrics
                || source.capabilities.lyrics_metadata)
        {
            return Err(RepositoryError::InvalidValue {
                field: "track_sources.spotify_capabilities",
                value: "Spotify sources can advertise metadata only".to_owned(),
            });
        }
        match (source.provider_kind, source.local_file.is_some()) {
            (ProviderKind::Local, false) => {
                return Err(RepositoryError::MissingLocalFile {
                    source_id: source.id,
                });
            }
            (ProviderKind::Local, true) => {}
            (_, true) => {
                return Err(RepositoryError::UnexpectedLocalFile {
                    source_id: source.id,
                });
            }
            (_, false) => {}
        }
        if let Some(local_file) = &source.local_file {
            if local_file.source_id != source.id {
                return Err(RepositoryError::LocalFileIdMismatch {
                    source_id: source.id,
                });
            }
        }
    }
    Ok(())
}

fn load_album(connection: &Connection, album_id: AlbumId) -> Result<Album, RepositoryError> {
    let album = connection
        .query_row(
            "SELECT id, title, release_date, created_at, updated_at FROM albums WHERE id = ?1",
            params![album_id.to_string()],
            map_album_row_raw,
        )
        .map_err(RepositoryError::from)?;
    parse_album_row(album)
}

fn load_artists(
    connection: &Connection,
    track_id: TrackId,
) -> Result<Vec<Artist>, RepositoryError> {
    let mut statement = connection.prepare(
        "SELECT a.id, a.name, a.sort_name, a.created_at, a.updated_at
         FROM artists a
         INNER JOIN track_artists ta ON ta.artist_id = a.id
         WHERE ta.track_id = ?1
         ORDER BY ta.artist_order",
    )?;
    let rows = statement.query_map(params![track_id.to_string()], map_artist_row_raw)?;
    rows.map(|row| {
        row.map_err(RepositoryError::from)
            .and_then(parse_artist_row)
    })
    .collect()
}

fn load_sources(
    connection: &Connection,
    track_id: TrackId,
) -> Result<Vec<TrackSource>, RepositoryError> {
    let mut statement = connection.prepare(
        "SELECT id, track_id, provider_kind, provider_item_id, source_uri, duration_ms,
                version_qualifiers_json, available, availability_detail, can_search,
                can_metadata, can_artwork, can_playback, can_lyrics, can_downloads,
                can_popularity, can_release_date, can_lyrics_metadata, created_at, updated_at
         FROM track_sources WHERE track_id = ?1 ORDER BY rowid",
    )?;
    let source_rows = statement.query_map(params![track_id.to_string()], map_source_row_raw)?;
    let mut sources = Vec::new();
    for source in source_rows {
        let mut source = parse_source_row(source?)?;
        source.local_file = load_local_file(connection, source.id)?;
        sources.push(source);
    }
    Ok(sources)
}

fn load_local_file(
    connection: &Connection,
    source_id: SourceId,
) -> Result<Option<LocalFileSource>, RepositoryError> {
    let local_file = connection
        .query_row(
            "SELECT source_id, path, file_size_bytes, modified_at, content_fingerprint, codec,
                    bitrate_kbps, sample_rate_hz, bit_depth
             FROM local_files WHERE source_id = ?1",
            params![source_id.to_string()],
            map_local_file_row_raw,
        )
        .optional()?;
    local_file.map(parse_local_file_row).transpose()
}

struct RawTrackRow {
    id: String,
    title: String,
    normalized_title: String,
    album_id: Option<String>,
    duration_ms: Option<i64>,
    version_qualifiers_json: String,
    preferred_source_id: Option<String>,
    created_at: String,
    updated_at: String,
}

struct RawArtistRow {
    id: String,
    name: String,
    sort_name: Option<String>,
    created_at: String,
    updated_at: String,
}

struct RawAlbumRow {
    id: String,
    title: String,
    release_date: Option<String>,
    created_at: String,
    updated_at: String,
}

struct RawSourceRow {
    id: String,
    track_id: String,
    provider_kind: String,
    provider_item_id: String,
    source_uri: Option<String>,
    duration_ms: Option<i64>,
    version_qualifiers_json: String,
    available: i64,
    availability_detail: Option<String>,
    can_search: i64,
    can_metadata: i64,
    can_artwork: i64,
    can_playback: i64,
    can_lyrics: i64,
    can_downloads: i64,
    can_popularity: i64,
    can_release_date: i64,
    can_lyrics_metadata: i64,
    created_at: String,
    updated_at: String,
}

struct RawLocalFileRow {
    source_id: String,
    path: String,
    file_size_bytes: Option<i64>,
    modified_at: Option<String>,
    content_fingerprint: Option<String>,
    codec: Option<String>,
    bitrate_kbps: Option<i64>,
    sample_rate_hz: Option<i64>,
    bit_depth: Option<i64>,
}

fn map_track_row_raw(row: &Row<'_>) -> rusqlite::Result<RawTrackRow> {
    Ok(RawTrackRow {
        id: row.get(0)?,
        title: row.get(1)?,
        normalized_title: row.get(2)?,
        album_id: row.get(3)?,
        duration_ms: row.get(4)?,
        version_qualifiers_json: row.get(5)?,
        preferred_source_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn parse_track_row(row: RawTrackRow) -> Result<TrackRow, RepositoryError> {
    Ok(TrackRow {
        id: parse_id(&row.id, "tracks.id")?,
        title: row.title,
        normalized_title: row.normalized_title,
        album_id: row
            .album_id
            .map(|value| parse_id(&value, "tracks.album_id"))
            .transpose()?,
        duration_ms: optional_u64(row.duration_ms, "tracks.duration_ms")?,
        version: version_from_json(
            &row.version_qualifiers_json,
            "tracks.version_qualifiers_json",
        )?,
        preferred_source_id: row
            .preferred_source_id
            .map(|value| parse_id(&value, "tracks.preferred_source_id"))
            .transpose()?,
        created_at: parse_timestamp(&row.created_at, "tracks.created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "tracks.updated_at")?,
    })
}

fn map_artist_row_raw(row: &Row<'_>) -> rusqlite::Result<RawArtistRow> {
    Ok(RawArtistRow {
        id: row.get(0)?,
        name: row.get(1)?,
        sort_name: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn parse_artist_row(row: RawArtistRow) -> Result<Artist, RepositoryError> {
    Ok(Artist {
        id: parse_id(&row.id, "artists.id")?,
        name: row.name,
        sort_name: row.sort_name,
        created_at: parse_timestamp(&row.created_at, "artists.created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "artists.updated_at")?,
    })
}

fn map_album_row_raw(row: &Row<'_>) -> rusqlite::Result<RawAlbumRow> {
    Ok(RawAlbumRow {
        id: row.get(0)?,
        title: row.get(1)?,
        release_date: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn parse_album_row(row: RawAlbumRow) -> Result<Album, RepositoryError> {
    Ok(Album {
        id: parse_id(&row.id, "albums.id")?,
        title: row.title,
        release_date: row.release_date,
        created_at: parse_timestamp(&row.created_at, "albums.created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "albums.updated_at")?,
    })
}

fn map_source_row_raw(row: &Row<'_>) -> rusqlite::Result<RawSourceRow> {
    Ok(RawSourceRow {
        id: row.get(0)?,
        track_id: row.get(1)?,
        provider_kind: row.get(2)?,
        provider_item_id: row.get(3)?,
        source_uri: row.get(4)?,
        duration_ms: row.get(5)?,
        version_qualifiers_json: row.get(6)?,
        available: row.get(7)?,
        availability_detail: row.get(8)?,
        can_search: row.get(9)?,
        can_metadata: row.get(10)?,
        can_artwork: row.get(11)?,
        can_playback: row.get(12)?,
        can_lyrics: row.get(13)?,
        can_downloads: row.get(14)?,
        can_popularity: row.get(15)?,
        can_release_date: row.get(16)?,
        can_lyrics_metadata: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

fn parse_source_row(row: RawSourceRow) -> Result<TrackSource, RepositoryError> {
    let provider_kind =
        row.provider_kind
            .parse::<ProviderKind>()
            .map_err(|_| RepositoryError::InvalidValue {
                field: "track_sources.provider_kind",
                value: row.provider_kind.clone(),
            })?;
    let source_uri = row
        .source_uri
        .as_deref()
        .map(Url::parse)
        .transpose()
        .map_err(|_| RepositoryError::InvalidValue {
            field: "track_sources.source_uri",
            value: row.source_uri.clone().unwrap_or_default(),
        })?;

    Ok(TrackSource {
        id: parse_id(&row.id, "track_sources.id")?,
        track_id: parse_id(&row.track_id, "track_sources.track_id")?,
        provider_kind,
        provider_item_id: row.provider_item_id,
        source_uri,
        duration_ms: optional_u64(row.duration_ms, "track_sources.duration_ms")?,
        version: version_from_json(
            &row.version_qualifiers_json,
            "track_sources.version_qualifiers_json",
        )?,
        available: bool_from_integer(row.available, "track_sources.available")?,
        availability_detail: row.availability_detail,
        capabilities: crate::domain::SourceCapabilities {
            search: bool_from_integer(row.can_search, "track_sources.can_search")?,
            metadata: bool_from_integer(row.can_metadata, "track_sources.can_metadata")?,
            artwork: bool_from_integer(row.can_artwork, "track_sources.can_artwork")?,
            playback: bool_from_integer(row.can_playback, "track_sources.can_playback")?,
            lyrics: bool_from_integer(row.can_lyrics, "track_sources.can_lyrics")?,
            downloads: bool_from_integer(row.can_downloads, "track_sources.can_downloads")?,
            popularity: bool_from_integer(row.can_popularity, "track_sources.can_popularity")?,
            release_date: bool_from_integer(
                row.can_release_date,
                "track_sources.can_release_date",
            )?,
            lyrics_metadata: bool_from_integer(
                row.can_lyrics_metadata,
                "track_sources.can_lyrics_metadata",
            )?,
        },
        local_file: None,
        created_at: parse_timestamp(&row.created_at, "track_sources.created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "track_sources.updated_at")?,
    })
}

fn map_local_file_row_raw(row: &Row<'_>) -> rusqlite::Result<RawLocalFileRow> {
    Ok(RawLocalFileRow {
        source_id: row.get(0)?,
        path: row.get(1)?,
        file_size_bytes: row.get(2)?,
        modified_at: row.get(3)?,
        content_fingerprint: row.get(4)?,
        codec: row.get(5)?,
        bitrate_kbps: row.get(6)?,
        sample_rate_hz: row.get(7)?,
        bit_depth: row.get(8)?,
    })
}

fn parse_local_file_row(row: RawLocalFileRow) -> Result<LocalFileSource, RepositoryError> {
    Ok(LocalFileSource {
        source_id: parse_id(&row.source_id, "local_files.source_id")?,
        path: PathBuf::from(row.path),
        file_size_bytes: optional_u64(row.file_size_bytes, "local_files.file_size_bytes")?,
        modified_at: row
            .modified_at
            .map(|value| parse_timestamp(&value, "local_files.modified_at"))
            .transpose()?,
        content_fingerprint: row.content_fingerprint,
        codec: row.codec,
        bitrate_kbps: optional_u64(row.bitrate_kbps, "local_files.bitrate_kbps")?,
        sample_rate_hz: optional_u64(row.sample_rate_hz, "local_files.sample_rate_hz")?,
        bit_depth: row
            .bit_depth
            .map(|value| optional_u16(value, "local_files.bit_depth"))
            .transpose()?,
    })
}

fn parse_id<T>(value: &str, field: &'static str) -> Result<T, RepositoryError>
where
    T: FromId,
{
    T::parse(value).map_err(|_| RepositoryError::InvalidValue {
        field,
        value: value.to_owned(),
    })
}

trait FromId: Sized {
    fn parse(value: &str) -> Result<Self, uuid::Error>;
}

impl FromId for TrackId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        TrackId::parse_str(value)
    }
}

impl FromId for ArtistId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        ArtistId::parse_str(value)
    }
}

impl FromId for AlbumId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        AlbumId::parse_str(value)
    }
}

impl FromId for SourceId {
    fn parse(value: &str) -> Result<Self, uuid::Error> {
        SourceId::parse_str(value)
    }
}

fn version_json(version: &VersionInfo) -> Result<String, RepositoryError> {
    serde_json::to_string(&version.qualifiers).map_err(|source| RepositoryError::Serialization {
        entity: "version qualifiers",
        source,
    })
}

fn version_from_json(value: &str, field: &'static str) -> Result<VersionInfo, RepositoryError> {
    let qualifiers: Vec<VersionQualifier> =
        serde_json::from_str(value).map_err(|source| RepositoryError::Deserialization {
            entity: field,
            source,
        })?;
    VersionInfo::new(qualifiers).map_err(|error| RepositoryError::InvalidValue {
        field,
        value: error.to_string(),
    })
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn parse_timestamp(value: &str, field: &'static str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| RepositoryError::InvalidValue {
            field,
            value: value.to_owned(),
        })
}

fn numeric_i64(value: u64, field: &'static str) -> Result<i64, RepositoryError> {
    i64::try_from(value).map_err(|_| RepositoryError::NumericOverflow { field })
}

fn optional_u64(value: Option<i64>, field: &'static str) -> Result<Option<u64>, RepositoryError> {
    value
        .map(|value| {
            u64::try_from(value).map_err(|_| RepositoryError::InvalidValue {
                field,
                value: value.to_string(),
            })
        })
        .transpose()
}

fn optional_u16(value: i64, field: &'static str) -> Result<u16, RepositoryError> {
    u16::try_from(value).map_err(|_| RepositoryError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

fn bool_integer(value: bool) -> i64 {
    i64::from(value)
}

fn bool_from_integer(value: i64, field: &'static str) -> Result<bool, RepositoryError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(RepositoryError::InvalidValue {
            field,
            value: value.to_string(),
        }),
    }
}

fn is_unique_constraint(error: &rusqlite::Error) -> bool {
    error.sqlite_extended_error_code() == Some(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::db::{Database, DatabaseError, TempDatabasePath};

    fn capabilities() -> crate::domain::SourceCapabilities {
        crate::domain::SourceCapabilities {
            search: true,
            playback: true,
            metadata: true,
            artwork: true,
            lyrics: false,
            downloads: true,
            popularity: true,
            release_date: true,
            lyrics_metadata: false,
        }
    }

    fn fixture_track() -> UnifiedTrack {
        let track_id = TrackId::new();
        let artist_a = Artist::new(ArtistId::new(), "Test Artist A").unwrap();
        let artist_b = Artist::new(ArtistId::new(), "Test Artist B").unwrap();
        let local = TrackSource::new_local(
            SourceId::new(),
            track_id,
            PathBuf::from("C:\\Music\\signal-test.flac"),
            crate::domain::SourceCapabilities {
                playback: true,
                metadata: true,
                ..crate::domain::SourceCapabilities::default()
            },
        )
        .unwrap();
        let mut youtube = TrackSource::new(
            SourceId::new(),
            track_id,
            ProviderKind::Youtube,
            "fake-video-id",
            capabilities(),
        )
        .unwrap();
        youtube.source_uri =
            Some(Url::parse("https://youtube.example/watch?v=fake-video-id").unwrap());
        youtube.duration_ms = Some(181_000);
        youtube.version = VersionInfo::new(vec![VersionQualifier::Remix]).unwrap();
        youtube.available = false;
        youtube.availability_detail = Some("fixture unavailable".to_owned());
        UnifiedTrack::new(
            track_id,
            "Signal Test",
            vec![artist_a, artist_b],
            None,
            Some(180_000),
            VersionInfo::standard(),
            vec![local, youtube],
        )
        .unwrap()
    }

    #[test]
    fn aggregate_round_trip_preserves_artists_sources_and_provider_ids() {
        let path = TempDatabasePath::new("repository-round-trip");
        let database = Database::open(path.path()).unwrap();
        let repository = TrackRepository::new(&database);
        let track = fixture_track();

        repository.create(&track).unwrap();
        let retrieved = repository.get(track.id).unwrap().unwrap();

        assert_eq!(retrieved.id, track.id);
        assert_eq!(retrieved.artists.len(), 2);
        assert_eq!(retrieved.sources.len(), 2);
        assert_eq!(retrieved.sources[0].provider_kind, ProviderKind::Local);
        assert_eq!(
            retrieved.sources[0]
                .local_file
                .as_ref()
                .map(|local_file| local_file.path.clone()),
            Some(PathBuf::from("C:\\Music\\signal-test.flac"))
        );
        let youtube = retrieved
            .sources
            .iter()
            .find(|source| source.provider_kind == ProviderKind::Youtube)
            .unwrap();
        assert_eq!(youtube.provider_item_id, "fake-video-id");
        assert_eq!(
            youtube.source_uri.as_ref().map(ToString::to_string),
            Some("https://youtube.example/watch?v=fake-video-id".to_owned())
        );
        assert_eq!(youtube.duration_ms, Some(181_000));
        assert_eq!(
            youtube.version,
            VersionInfo::new(vec![VersionQualifier::Remix]).unwrap()
        );
        assert!(!youtube.available);
        assert_eq!(
            youtube.availability_detail.as_deref(),
            Some("fixture unavailable")
        );
    }

    #[test]
    fn optional_album_round_trips() {
        let path = TempDatabasePath::new("repository-album");
        let database = Database::open(path.path()).unwrap();
        let repository = TrackRepository::new(&database);
        let track_id = TrackId::new();
        let album = Album::new(AlbumId::new(), "Test Album").unwrap();
        let track = UnifiedTrack::new(
            track_id,
            "Signal Test",
            vec![],
            Some(album.clone()),
            None,
            VersionInfo::standard(),
            vec![],
        )
        .unwrap();

        repository.create(&track).unwrap();
        assert_eq!(
            repository.get(track_id).unwrap().unwrap().album,
            Some(album)
        );
    }

    #[test]
    fn provider_identity_is_unique_across_tracks() {
        let path = TempDatabasePath::new("repository-identity");
        let database = Database::open(path.path()).unwrap();
        let repository = TrackRepository::new(&database);
        let first = fixture_track();
        let second_id = TrackId::new();
        let duplicate = TrackSource::new(
            SourceId::new(),
            second_id,
            ProviderKind::Youtube,
            "fake-video-id",
            capabilities(),
        )
        .unwrap();
        let second = UnifiedTrack::new(
            second_id,
            "Other Track",
            vec![],
            None,
            None,
            VersionInfo::standard(),
            vec![duplicate],
        )
        .unwrap();

        repository.create(&first).unwrap();
        let result = repository.create(&second);

        assert!(matches!(
            result,
            Err(RepositoryError::DuplicateProviderIdentity { .. })
        ));
        assert_eq!(repository.count().unwrap(), 1);
    }

    #[test]
    fn multi_table_create_rolls_back_when_a_later_source_conflicts() {
        let path = TempDatabasePath::new("repository-rollback");
        let database = Database::open(path.path()).unwrap();
        let repository = TrackRepository::new(&database);
        let first = fixture_track();
        repository.create(&first).unwrap();

        let second_id = TrackId::new();
        let first_source = TrackSource::new(
            SourceId::new(),
            second_id,
            ProviderKind::Youtube,
            "same-source-id",
            capabilities(),
        )
        .unwrap();
        let duplicate_source = TrackSource::new(
            SourceId::new(),
            second_id,
            ProviderKind::Youtube,
            "same-source-id",
            capabilities(),
        )
        .unwrap();
        let second_artist = Artist::new(ArtistId::new(), "Rollback Artist").unwrap();
        let second = UnifiedTrack::new(
            second_id,
            "Rollback Track",
            vec![second_artist],
            None,
            None,
            VersionInfo::standard(),
            vec![first_source, duplicate_source],
        )
        .unwrap();

        let result = repository.create(&second);

        assert!(matches!(
            result,
            Err(RepositoryError::DuplicateProviderIdentity { .. })
        ));
        assert_eq!(repository.count().unwrap(), 1);
        let artist_count: i64 = database
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM artists", [], |row| row.get(0))
            })
            .unwrap();
        assert_eq!(artist_count, 2);
    }

    #[test]
    fn database_trigger_rejects_preferred_source_from_another_track() {
        let path = TempDatabasePath::new("preferred-source-integrity");
        let database = Database::open(path.path()).unwrap();
        let repository = TrackRepository::new(&database);
        let first = fixture_track();
        let second_id = TrackId::new();
        let second = UnifiedTrack::new(
            second_id,
            "Other Track",
            vec![],
            None,
            None,
            VersionInfo::standard(),
            vec![],
        )
        .unwrap();
        repository.create(&first).unwrap();
        repository.create(&second).unwrap();

        let foreign_source_id = first.sources[0].id;
        let cross_track_preference = database.with_connection(|connection| {
            connection.execute(
                "UPDATE tracks SET preferred_source_id = ?1 WHERE id = ?2",
                params![foreign_source_id.to_string(), second_id.to_string()],
            )
        });
        assert!(matches!(
            cross_track_preference,
            Err(DatabaseError::Query(_))
        ));

        let source_move = database.with_connection(|connection| {
            connection.execute(
                "UPDATE tracks SET preferred_source_id = ?1 WHERE id = ?2",
                params![foreign_source_id.to_string(), first.id.to_string()],
            )?;
            connection.execute(
                "UPDATE track_sources SET track_id = ?1 WHERE id = ?2",
                params![second_id.to_string(), foreign_source_id.to_string()],
            )
        });

        assert!(matches!(source_move, Err(DatabaseError::Query(_))));
    }
}
