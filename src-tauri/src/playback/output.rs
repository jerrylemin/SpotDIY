use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_OUTPUT_PROFILES: usize = 16;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputProfile {
    pub id: String,
    pub name: String,
    pub audio_device_name: String,
    pub volume_percent: u8,
    pub muted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[error("{detail}")]
pub struct OutputProfileValidationError {
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[error("{detail}")]
pub struct OutputProfileApplyError {
    pub code: OutputProfileApplyErrorCode,
    pub detail: String,
    pub rollback_succeeded: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputProfileApplyErrorCode {
    InvalidProfile,
    DeviceUnavailable,
    ApplyFailed,
}

impl OutputProfile {
    pub fn normalized(mut self) -> Result<Self, OutputProfileValidationError> {
        self.name = collapse_whitespace(&self.name);
        self.audio_device_name = self.audio_device_name.trim().to_owned();
        if self.audio_device_name.eq_ignore_ascii_case("auto") {
            self.audio_device_name = "auto".to_owned();
        }
        if !(1..=80).contains(&self.name.chars().count()) {
            return Err(OutputProfileValidationError {
                detail: "profile name must contain 1 to 80 Unicode scalar values after trimming"
                    .to_owned(),
            });
        }
        if self.audio_device_name.trim().is_empty() {
            return Err(OutputProfileValidationError {
                detail: "audio device name must be auto or a non-empty enumerated device name"
                    .to_owned(),
            });
        }
        if self.volume_percent > 100 {
            return Err(OutputProfileValidationError {
                detail: "volume must be between 0 and 100".to_owned(),
            });
        }
        if self.id.trim().is_empty() {
            return Err(OutputProfileValidationError {
                detail: "profile id must not be empty".to_owned(),
            });
        }
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), OutputProfileValidationError> {
        self.clone().normalized().map(|_| ())
    }
}

pub fn validate_output_profiles(
    profiles: &[OutputProfile],
) -> Result<(), OutputProfileValidationError> {
    normalize_output_profiles(profiles).map(|_| ())
}

pub fn normalize_output_profiles(
    profiles: &[OutputProfile],
) -> Result<Vec<OutputProfile>, OutputProfileValidationError> {
    if profiles.len() > MAX_OUTPUT_PROFILES {
        return Err(OutputProfileValidationError {
            detail: format!("at most {MAX_OUTPUT_PROFILES} output profiles are supported"),
        });
    }

    let mut names = std::collections::HashSet::new();
    let mut normalized_profiles = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let profile = profile.clone().normalized()?;
        let normalized = profile.name.to_lowercase();
        if !names.insert(normalized) {
            return Err(OutputProfileValidationError {
                detail: "output profile names must be unique case-insensitively".to_owned(),
            });
        }
        normalized_profiles.push(profile);
    }
    Ok(normalized_profiles)
}

pub fn apply_error(
    code: OutputProfileApplyErrorCode,
    detail: impl Into<String>,
    rollback_succeeded: bool,
) -> OutputProfileApplyError {
    OutputProfileApplyError {
        code,
        detail: detail.into(),
        rollback_succeeded,
    }
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> OutputProfile {
        OutputProfile {
            id: "profile-1".to_owned(),
            name: name.to_owned(),
            audio_device_name: "auto".to_owned(),
            volume_percent: 50,
            muted: false,
        }
    }

    #[test]
    fn names_are_trimmed_and_whitespace_is_collapsed() {
        assert_eq!(
            profile("  Desk   speakers ").normalized().unwrap().name,
            "Desk speakers"
        );
    }

    #[test]
    fn names_are_case_insensitively_unique() {
        let result = validate_output_profiles(&[
            profile("Desk"),
            OutputProfile {
                id: "2".to_owned(),
                name: " desk ".to_owned(),
                ..profile("Other")
            },
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn profile_limit_and_volume_are_enforced() {
        let mut profiles = Vec::new();
        for index in 0..=MAX_OUTPUT_PROFILES {
            profiles.push(OutputProfile {
                id: index.to_string(),
                name: format!("Profile {index}"),
                ..profile("unused")
            });
        }
        assert!(validate_output_profiles(&profiles).is_err());
        assert!(OutputProfile {
            volume_percent: 101,
            ..profile("loud")
        }
        .validate()
        .is_err());
    }
}
