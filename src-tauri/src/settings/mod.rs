use std::collections::HashSet;
use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{Database, DatabaseError};
use crate::domain::ProviderKind;

const SETTINGS_SCHEMA_VERSION: i64 = 1;
const CUSTOM_THEME_SCHEMA_VERSION: u32 = 1;
const MAX_CUSTOM_THEME_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
    Custom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutProfile {
    Comfortable,
    Compact,
    Dense,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeBaseMode {
    Dark,
    Light,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SpotThemeTokens {
    pub background: String,
    pub surface: String,
    pub surface_raised: String,
    pub surface_soft: String,
    pub text: String,
    pub text_muted: String,
    pub text_subtle: String,
    pub border: String,
    pub border_strong: String,
    pub accent: String,
    pub accent_contrast: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
    pub info: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SpotThemeDefinition {
    pub schema_version: u32,
    pub name: String,
    pub base_mode: ThemeBaseMode,
    pub tokens: SpotThemeTokens,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Standard,
    Portable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingClass {
    Ordinary,
    Secret,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretSettingKey {
    SpotifyClientSecret,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub theme: Theme,
    pub layout_profile: LayoutProfile,
    pub custom_theme: Option<SpotThemeDefinition>,
    pub downloads_directory: Option<PathBuf>,
    pub source_preference_order: Vec<ProviderKind>,
    pub first_run: bool,
    pub storage_mode: StorageMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "key", content = "value", rename_all = "camelCase")]
pub enum SettingValue {
    Theme(Theme),
    LayoutProfile(LayoutProfile),
    CustomTheme(Box<Option<SpotThemeDefinition>>),
    DownloadsDirectory(Option<PathBuf>),
    SourcePreferenceOrder(Vec<ProviderKind>),
}

impl SettingValue {
    pub const fn class(&self) -> SettingClass {
        SettingClass::Ordinary
    }
}

impl SpotThemeDefinition {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != CUSTOM_THEME_SCHEMA_VERSION {
            return Err(SettingsError::InvalidValue {
                key: "custom_theme",
                reason: format!("unsupported theme schema version {}", self.schema_version),
            });
        }

        let name_length = self.name.trim().chars().count();
        if !(1..=80).contains(&name_length) {
            return Err(SettingsError::InvalidValue {
                key: "custom_theme",
                reason: "name must contain 1 to 80 Unicode scalar values after trimming".to_owned(),
            });
        }

        let serialized =
            serde_json::to_vec(self).map_err(|source| SettingsError::Serialization {
                key: "custom_theme",
                source,
            })?;
        if serialized.len() > MAX_CUSTOM_THEME_BYTES {
            return Err(SettingsError::InvalidValue {
                key: "custom_theme",
                reason: format!("serialized theme exceeds the {MAX_CUSTOM_THEME_BYTES} byte limit"),
            });
        }

        for (name, color) in [
            ("background", &self.tokens.background),
            ("surface", &self.tokens.surface),
            ("surfaceRaised", &self.tokens.surface_raised),
            ("surfaceSoft", &self.tokens.surface_soft),
            ("text", &self.tokens.text),
            ("textMuted", &self.tokens.text_muted),
            ("textSubtle", &self.tokens.text_subtle),
            ("border", &self.tokens.border),
            ("borderStrong", &self.tokens.border_strong),
            ("accent", &self.tokens.accent),
            ("accentContrast", &self.tokens.accent_contrast),
            ("success", &self.tokens.success),
            ("warning", &self.tokens.warning),
            ("danger", &self.tokens.danger),
            ("info", &self.tokens.info),
        ] {
            if !is_hex_color(color) {
                return Err(SettingsError::InvalidValue {
                    key: "custom_theme",
                    reason: format!("token {name} must be a #RRGGBB color"),
                });
            }
        }

        for (foreground_name, background_name, minimum) in [
            ("text", "background", 4.5),
            ("text", "surface", 4.5),
            ("textMuted", "background", 4.5),
            ("textMuted", "surface", 4.5),
            ("accent", "accentContrast", 4.5),
            ("accent", "background", 3.0),
            ("accent", "surface", 3.0),
        ] {
            let foreground = theme_token(&self.tokens, foreground_name);
            let background = theme_token(&self.tokens, background_name);
            let ratio = contrast_ratio(foreground, background);
            if ratio < minimum {
                return Err(SettingsError::InvalidValue {
                    key: "custom_theme",
                    reason: format!(
                        "{foreground_name}/{background_name} contrast is {ratio:.2}:1; minimum is {minimum:.1}:1"
                    ),
                });
            }
        }

        Ok(())
    }
}

fn theme_token<'tokens>(tokens: &'tokens SpotThemeTokens, name: &str) -> &'tokens str {
    match name {
        "background" => &tokens.background,
        "surface" => &tokens.surface,
        "text" => &tokens.text,
        "textMuted" => &tokens.text_muted,
        "accent" => &tokens.accent,
        "accentContrast" => &tokens.accent_contrast,
        _ => unreachable!("theme contrast token is declared in the validation table"),
    }
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

fn relative_luminance(value: &str) -> f64 {
    let channels = [
        u8::from_str_radix(&value[1..3], 16).expect("validated color"),
        u8::from_str_radix(&value[3..5], 16).expect("validated color"),
        u8::from_str_radix(&value[5..7], 16).expect("validated color"),
    ]
    .map(|channel| {
        let normalized = f64::from(channel) / 255.0;
        if normalized <= 0.04045 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    });
    channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
}

fn contrast_ratio(foreground: &str, background: &str) -> f64 {
    let foreground_luminance = relative_luminance(foreground);
    let background_luminance = relative_luminance(background);
    (foreground_luminance.max(background_luminance) + 0.05)
        / (foreground_luminance.min(background_luminance) + 0.05)
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("settings database operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not serialize setting {key}: {source}")]
    Serialization {
        key: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not deserialize setting {key}: {source}")]
    Deserialization {
        key: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("setting {key} has invalid value: {reason}")]
    InvalidValue { key: &'static str, reason: String },
    #[error("setting {key} uses unsupported schema version {version}")]
    UnsupportedSchemaVersion { key: String, version: i64 },
    #[error("setting {key} has unexpected stored type {value_type}")]
    UnexpectedType {
        key: &'static str,
        value_type: String,
    },
    #[error("portable storage mode is not supported by the current standard startup path")]
    UnsupportedStorageMode,
}

pub struct SettingsRepository<'database> {
    database: &'database Database,
}

impl<'database> SettingsRepository<'database> {
    pub fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub fn get_snapshot(&self) -> Result<SettingsSnapshot, SettingsError> {
        let connection = self.database.connection()?;
        let mut snapshot = SettingsSnapshot::default();

        if let Some((value_json, value_type, schema_version)) = read_setting(&connection, "theme")?
        {
            ensure_record("theme", &value_type, "theme", schema_version)?;
            snapshot.theme = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "theme",
                    source,
                }
            })?;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "downloads_directory")?
        {
            ensure_record(
                "downloads_directory",
                &value_type,
                "downloads_directory",
                schema_version,
            )?;
            snapshot.downloads_directory = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "downloads_directory",
                    source,
                }
            })?;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "source_preference_order")?
        {
            ensure_record(
                "source_preference_order",
                &value_type,
                "source_preference_order",
                schema_version,
            )?;
            let order: Vec<ProviderKind> = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "source_preference_order",
                    source,
                }
            })?;
            validate_source_preference_order(&order)?;
            snapshot.source_preference_order = order;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "first_run")?
        {
            ensure_record("first_run", &value_type, "boolean", schema_version)?;
            snapshot.first_run = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "first_run",
                    source,
                }
            })?;
        }
        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "storage_mode")?
        {
            ensure_record("storage_mode", &value_type, "storage_mode", schema_version)?;
            snapshot.storage_mode = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "storage_mode",
                    source,
                }
            })?;
        }

        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "layout_profile")?
        {
            ensure_record(
                "layout_profile",
                &value_type,
                "layout_profile",
                schema_version,
            )?;
            snapshot.layout_profile = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "layout_profile",
                    source,
                }
            })?;
        }

        if let Some((value_json, value_type, schema_version)) =
            read_setting(&connection, "custom_theme")?
        {
            ensure_record("custom_theme", &value_type, "custom_theme", schema_version)?;
            if value_json.len() > MAX_CUSTOM_THEME_BYTES {
                return Err(SettingsError::InvalidValue {
                    key: "custom_theme",
                    reason: format!(
                        "serialized theme exceeds the {MAX_CUSTOM_THEME_BYTES} byte limit"
                    ),
                });
            }
            snapshot.custom_theme = serde_json::from_str(&value_json).map_err(|source| {
                SettingsError::Deserialization {
                    key: "custom_theme",
                    source,
                }
            })?;
            if let Some(theme) = snapshot.custom_theme.as_ref() {
                theme.validate()?;
            }
        }

        if snapshot.storage_mode == StorageMode::Portable {
            return Err(SettingsError::UnsupportedStorageMode);
        }

        if snapshot.theme == Theme::Custom && snapshot.custom_theme.is_none() {
            return Err(SettingsError::InvalidValue {
                key: "theme",
                reason: "custom theme is selected but no valid custom theme is stored".to_owned(),
            });
        }

        Ok(snapshot)
    }

    pub fn get_theme(&self) -> Result<Theme, SettingsError> {
        Ok(self.get_snapshot()?.theme)
    }

    pub fn get_layout_profile(&self) -> Result<LayoutProfile, SettingsError> {
        Ok(self.get_snapshot()?.layout_profile)
    }

    pub fn get_downloads_directory(&self) -> Result<Option<PathBuf>, SettingsError> {
        Ok(self.get_snapshot()?.downloads_directory)
    }

    pub fn get_source_preference_order(&self) -> Result<Vec<ProviderKind>, SettingsError> {
        Ok(self.get_snapshot()?.source_preference_order)
    }

    pub fn set_setting(&self, setting: SettingValue) -> Result<SettingsSnapshot, SettingsError> {
        match &setting {
            SettingValue::Theme(Theme::Custom) => {
                if self.get_snapshot()?.custom_theme.is_none() {
                    return Err(SettingsError::InvalidValue {
                        key: "theme",
                        reason: "a valid custom theme must be stored before selecting custom"
                            .to_owned(),
                    });
                }
            }
            SettingValue::CustomTheme(value)
                if value.is_none() && self.get_snapshot()?.theme == Theme::Custom =>
            {
                return Err(SettingsError::InvalidValue {
                    key: "custom_theme",
                    reason: "select another theme before clearing the active custom theme"
                        .to_owned(),
                });
            }
            _ => {}
        }
        let (key, value_type, value_json) = encode_setting(&setting)?;
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(setting_key) DO UPDATE SET
                 value_json = excluded.value_json,
                 value_type = excluded.value_type,
                 schema_version = excluded.schema_version,
                 updated_at = excluded.updated_at",
            params![
                key,
                value_json,
                value_type,
                SETTINGS_SCHEMA_VERSION,
                Utc::now().to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_snapshot()
    }

    pub fn mark_initialized(&self) -> Result<(), SettingsError> {
        let mut connection = self.database.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
             VALUES ('first_run', 'false', 'boolean', ?1, ?2)
             ON CONFLICT(setting_key) DO UPDATE SET
                 value_json = excluded.value_json,
                 value_type = excluded.value_type,
                 schema_version = excluded.schema_version,
                 updated_at = excluded.updated_at",
            params![SETTINGS_SCHEMA_VERSION, Utc::now().to_rfc3339()],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            layout_profile: LayoutProfile::Comfortable,
            custom_theme: None,
            downloads_directory: None,
            source_preference_order: vec![
                ProviderKind::Local,
                ProviderKind::Soundcloud,
                ProviderKind::Youtube,
                ProviderKind::Spotify,
            ],
            first_run: true,
            storage_mode: StorageMode::Standard,
        }
    }
}

fn encode_setting(
    setting: &SettingValue,
) -> Result<(&'static str, &'static str, String), SettingsError> {
    match setting {
        SettingValue::Theme(value) => Ok((
            "theme",
            "theme",
            serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                key: "theme",
                source,
            })?,
        )),
        SettingValue::LayoutProfile(value) => Ok((
            "layout_profile",
            "layout_profile",
            serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                key: "layout_profile",
                source,
            })?,
        )),
        SettingValue::CustomTheme(value) => {
            if let Some(theme) = value.as_ref() {
                theme.validate()?;
            }
            let value_json =
                serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                    key: "custom_theme",
                    source,
                })?;
            if value_json.len() > MAX_CUSTOM_THEME_BYTES {
                return Err(SettingsError::InvalidValue {
                    key: "custom_theme",
                    reason: format!(
                        "serialized theme exceeds the {MAX_CUSTOM_THEME_BYTES} byte limit"
                    ),
                });
            }
            Ok(("custom_theme", "custom_theme", value_json))
        }
        SettingValue::DownloadsDirectory(value) => Ok((
            "downloads_directory",
            "downloads_directory",
            serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                key: "downloads_directory",
                source,
            })?,
        )),
        SettingValue::SourcePreferenceOrder(value) => {
            validate_source_preference_order(value)?;
            Ok((
                "source_preference_order",
                "source_preference_order",
                serde_json::to_string(value).map_err(|source| SettingsError::Serialization {
                    key: "source_preference_order",
                    source,
                })?,
            ))
        }
    }
}

fn read_setting(
    connection: &Connection,
    key: &str,
) -> Result<Option<(String, String, i64)>, SettingsError> {
    connection
        .query_row(
            "SELECT value_json, value_type, schema_version FROM settings_metadata WHERE setting_key = ?1",
            params![key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(SettingsError::from)
}

fn ensure_record(
    key: &'static str,
    actual_type: &str,
    expected_type: &'static str,
    schema_version: i64,
) -> Result<(), SettingsError> {
    if actual_type != expected_type {
        return Err(SettingsError::UnexpectedType {
            key,
            value_type: actual_type.to_owned(),
        });
    }
    if schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(SettingsError::UnsupportedSchemaVersion {
            key: key.to_owned(),
            version: schema_version,
        });
    }
    Ok(())
}

fn validate_source_preference_order(order: &[ProviderKind]) -> Result<(), SettingsError> {
    let unique: HashSet<_> = order.iter().copied().collect();
    let expected: HashSet<_> = ProviderKind::all().iter().copied().collect();
    if unique != expected || order.len() != expected.len() {
        return Err(SettingsError::InvalidValue {
            key: "source_preference_order",
            reason: "the order must contain each supported provider exactly once".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, TempDatabasePath};

    fn valid_theme(base_mode: ThemeBaseMode) -> SpotThemeDefinition {
        let light = base_mode == ThemeBaseMode::Light;
        SpotThemeDefinition {
            schema_version: 1,
            name: "Test theme".to_owned(),
            base_mode,
            tokens: SpotThemeTokens {
                background: if light { "#F6F7F2" } else { "#101113" }.to_owned(),
                surface: if light { "#FFFFFF" } else { "#17181D" }.to_owned(),
                surface_raised: if light { "#FFFFFF" } else { "#1D1E24" }.to_owned(),
                surface_soft: if light { "#EDF0E8" } else { "#22232A" }.to_owned(),
                text: if light { "#161719" } else { "#F3F1EC" }.to_owned(),
                text_muted: if light { "#53565B" } else { "#A8A7AE" }.to_owned(),
                text_subtle: if light { "#6D7176" } else { "#807F87" }.to_owned(),
                border: if light { "#D6D9D1" } else { "#2E2F36" }.to_owned(),
                border_strong: if light { "#A7ADA4" } else { "#4B4C55" }.to_owned(),
                accent: if light { "#567800" } else { "#D7FF60" }.to_owned(),
                accent_contrast: if light { "#FFFFFF" } else { "#151617" }.to_owned(),
                success: if light { "#167C62" } else { "#81E2D0" }.to_owned(),
                warning: if light { "#A45700" } else { "#FFB570" }.to_owned(),
                danger: if light { "#BD3027" } else { "#FF806F" }.to_owned(),
                info: if light { "#5C4DE0" } else { "#8E7BFF" }.to_owned(),
            },
        }
    }

    #[test]
    fn defaults_are_available_without_speculative_rows() {
        let path = TempDatabasePath::new("settings-defaults");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        let snapshot = repository.get_snapshot().unwrap();

        assert_eq!(snapshot.theme, Theme::Dark);
        assert_eq!(snapshot.downloads_directory, None);
        assert!(snapshot.first_run);
        assert_eq!(snapshot.storage_mode, StorageMode::Standard);
        assert_eq!(snapshot.layout_profile, LayoutProfile::Comfortable);
        assert_eq!(snapshot.custom_theme, None);
        assert_eq!(snapshot.source_preference_order.len(), 4);
    }

    #[test]
    fn custom_theme_and_layout_profile_round_trip_without_a_schema_migration() {
        let path = TempDatabasePath::new("settings-theme-layout");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);
        let theme = valid_theme(ThemeBaseMode::Dark);

        repository
            .set_setting(SettingValue::CustomTheme(Box::new(Some(theme.clone()))))
            .unwrap();
        repository
            .set_setting(SettingValue::Theme(Theme::Custom))
            .unwrap();
        repository
            .set_setting(SettingValue::LayoutProfile(LayoutProfile::Dense))
            .unwrap();

        let snapshot = SettingsRepository::new(&database).get_snapshot().unwrap();
        assert_eq!(snapshot.theme, Theme::Custom);
        assert_eq!(snapshot.custom_theme, Some(theme));
        assert_eq!(snapshot.layout_profile, LayoutProfile::Dense);
        assert_eq!(database.schema_version().unwrap(), 7);
    }

    #[test]
    fn valid_light_theme_is_accepted_and_custom_theme_is_ordinary_data() {
        let theme = valid_theme(ThemeBaseMode::Light);
        assert!(theme.validate().is_ok());
        assert_eq!(
            SettingValue::CustomTheme(Box::new(Some(theme))).class(),
            SettingClass::Ordinary
        );
    }

    #[test]
    fn custom_theme_validation_rejects_schema_colors_unknown_tokens_and_contrast() {
        let mut malformed = valid_theme(ThemeBaseMode::Dark);
        malformed.schema_version = 2;
        assert!(matches!(
            malformed.validate(),
            Err(SettingsError::InvalidValue {
                key: "custom_theme",
                ..
            })
        ));

        let mut malformed = valid_theme(ThemeBaseMode::Dark);
        malformed.tokens.accent = "rgb(1, 2, 3)".to_owned();
        assert!(matches!(
            malformed.validate(),
            Err(SettingsError::InvalidValue {
                key: "custom_theme",
                ..
            })
        ));

        let mut low_contrast = valid_theme(ThemeBaseMode::Dark);
        low_contrast.tokens.text = "#111111".to_owned();
        low_contrast.tokens.text_muted = "#121212".to_owned();
        assert!(matches!(
            low_contrast.validate(),
            Err(SettingsError::InvalidValue {
                key: "custom_theme",
                ..
            })
        ));

        let unknown_token = r##"{
          "schemaVersion": 1,
          "name": "Unknown",
          "baseMode": "dark",
          "tokens": {
            "background": "#101113",
            "surface": "#17181D",
            "surfaceRaised": "#1D1E24",
            "surfaceSoft": "#22232A",
            "text": "#F3F1EC",
            "textMuted": "#A8A7AE",
            "textSubtle": "#807F87",
            "border": "#2E2F36",
            "borderStrong": "#4B4C55",
            "accent": "#D7FF60",
            "accentContrast": "#151617",
            "success": "#81E2D0",
            "warning": "#FFB570",
            "danger": "#FF806F",
            "info": "#8E7BFF",
            "extra": "#FFFFFF"
          }
        }"##;
        assert!(serde_json::from_str::<SpotThemeDefinition>(unknown_token).is_err());
    }

    #[test]
    fn custom_theme_cannot_be_selected_without_a_valid_definition() {
        let path = TempDatabasePath::new("settings-custom-missing");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        let result = repository.set_setting(SettingValue::Theme(Theme::Custom));

        assert!(matches!(
            result,
            Err(SettingsError::InvalidValue { key: "theme", .. })
        ));
    }

    #[test]
    fn clearing_custom_theme_after_switching_back_to_dark_is_supported() {
        let path = TempDatabasePath::new("settings-custom-clear");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        repository
            .set_setting(SettingValue::CustomTheme(Box::new(Some(valid_theme(
                ThemeBaseMode::Dark,
            )))))
            .unwrap();
        repository
            .set_setting(SettingValue::Theme(Theme::Custom))
            .unwrap();
        repository
            .set_setting(SettingValue::Theme(Theme::Dark))
            .unwrap();
        let snapshot = repository
            .set_setting(SettingValue::CustomTheme(Box::new(None)))
            .unwrap();

        assert_eq!(snapshot.theme, Theme::Dark);
        assert_eq!(snapshot.custom_theme, None);
    }

    #[test]
    fn setting_updates_persist_across_reopen() {
        let path = TempDatabasePath::new("settings-persist");
        {
            let database = Database::open(path.path()).unwrap();
            let repository = SettingsRepository::new(&database);
            repository
                .set_setting(SettingValue::Theme(Theme::Light))
                .unwrap();
            repository
                .set_setting(SettingValue::DownloadsDirectory(Some(PathBuf::from(
                    "C:\\SpotDIY\\Downloads",
                ))))
                .unwrap();
            repository.mark_initialized().unwrap();
        }

        let database = Database::open(path.path()).unwrap();
        let snapshot = SettingsRepository::new(&database).get_snapshot().unwrap();

        assert_eq!(snapshot.theme, Theme::Light);
        assert_eq!(
            snapshot.downloads_directory,
            Some(PathBuf::from("C:\\SpotDIY\\Downloads"))
        );
        assert!(!snapshot.first_run);
    }

    #[test]
    fn setting_overwrite_replaces_the_previous_typed_value() {
        let path = TempDatabasePath::new("settings-overwrite");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        repository
            .set_setting(SettingValue::Theme(Theme::Light))
            .unwrap();
        repository
            .set_setting(SettingValue::Theme(Theme::System))
            .unwrap();

        assert_eq!(repository.get_theme().unwrap(), Theme::System);
    }

    #[test]
    fn invalid_serialized_setting_fails_safely() {
        let path = TempDatabasePath::new("settings-invalid");
        let database = Database::open(path.path()).unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO settings_metadata (setting_key, value_json, value_type, schema_version, updated_at)
                     VALUES ('theme', '\"neon\"', 'theme', 1, '2026-01-01T00:00:00Z')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let result = SettingsRepository::new(&database).get_snapshot();

        assert!(matches!(
            result,
            Err(SettingsError::Deserialization { key: "theme", .. })
        ));
    }

    #[test]
    fn invalid_preference_order_is_rejected_before_persistence() {
        let path = TempDatabasePath::new("settings-invalid-order");
        let database = Database::open(path.path()).unwrap();
        let repository = SettingsRepository::new(&database);

        let result = repository.set_setting(SettingValue::SourcePreferenceOrder(vec![
            ProviderKind::Local,
            ProviderKind::Local,
        ]));

        assert!(matches!(result, Err(SettingsError::InvalidValue { .. })));
    }

    #[test]
    fn portable_mode_is_rejected_until_portable_startup_is_implemented() {
        let path = TempDatabasePath::new("settings-portable");
        let database = Database::open(path.path()).unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE settings_metadata SET value_json = '\"portable\"' WHERE setting_key = 'storage_mode'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let result = SettingsRepository::new(&database).get_snapshot();

        assert!(matches!(result, Err(SettingsError::UnsupportedStorageMode)));
    }
}
