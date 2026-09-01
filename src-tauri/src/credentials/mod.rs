use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use thiserror::Error;

pub const KEYRING_SERVICE: &str = "SpotDIY";
pub const KEYRING_USERNAME: &str = "spotify-pkce";

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct SpotifyCredentialRecord {
    client_id: String,
    market: String,
    refresh_token: String,
}

impl SpotifyCredentialRecord {
    pub fn new(
        client_id: impl Into<String>,
        market: impl AsRef<str>,
        refresh_token: impl Into<String>,
    ) -> Result<Self, CredentialError> {
        let client_id = client_id.into();
        let refresh_token = refresh_token.into();
        let market = normalize_market(market.as_ref())?;
        if client_id.trim().is_empty() || refresh_token.is_empty() {
            return Err(CredentialError::InvalidRecord);
        }
        Ok(Self {
            client_id,
            market,
            refresh_token,
        })
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn market(&self) -> &str {
        &self.market
    }

    pub(crate) fn refresh_token(&self) -> &str {
        &self.refresh_token
    }
}

impl fmt::Debug for SpotifyCredentialRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpotifyCredentialRecord")
            .field("client_id", &self.client_id)
            .field("market", &self.market)
            .field("refresh_token", &"redacted")
            .finish()
    }
}

impl<'de> Deserialize<'de> for SpotifyCredentialRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireCredentialRecord {
            client_id: String,
            market: String,
            refresh_token: String,
        }

        let wire = WireCredentialRecord::deserialize(deserializer)?;
        Self::new(wire.client_id, wire.market, wire.refresh_token).map_err(D::Error::custom)
    }
}

fn normalize_market(value: &str) -> Result<String, CredentialError> {
    let value = value.trim();
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(CredentialError::InvalidMarket);
    }
    Ok(value.to_ascii_uppercase())
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CredentialError {
    #[error("Spotify market must be exactly two ASCII letters")]
    InvalidMarket,
    #[error("Spotify credential record is invalid")]
    InvalidRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CredentialStoreError {
    #[error("secure credential storage is unavailable")]
    Unavailable,
    #[error("secure credential storage operation failed")]
    Failed,
    #[error("stored Spotify credential is invalid")]
    InvalidRecord,
}

pub trait CredentialStore: Send + Sync {
    fn load(&self) -> Result<Option<SpotifyCredentialRecord>, CredentialStoreError>;
    fn save(&self, record: &SpotifyCredentialRecord) -> Result<(), CredentialStoreError>;
    fn delete(&self) -> Result<(), CredentialStoreError>;
}

pub type SharedCredentialStore = Arc<dyn CredentialStore>;

#[derive(Clone, Default)]
pub struct MemoryCredentialStore {
    record: Arc<Mutex<Option<SpotifyCredentialRecord>>>,
    fail_operations: Arc<AtomicBool>,
}

impl MemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_failure(&self, failed: bool) {
        self.fail_operations.store(failed, Ordering::Release);
    }

    pub fn clear(&self) -> Result<(), CredentialStoreError> {
        self.delete()
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn load(&self) -> Result<Option<SpotifyCredentialRecord>, CredentialStoreError> {
        if self.fail_operations.load(Ordering::Acquire) {
            return Err(CredentialStoreError::Unavailable);
        }
        self.record
            .lock()
            .map(|record| record.clone())
            .map_err(|_| CredentialStoreError::Failed)
    }

    fn save(&self, record: &SpotifyCredentialRecord) -> Result<(), CredentialStoreError> {
        if self.fail_operations.load(Ordering::Acquire) {
            return Err(CredentialStoreError::Unavailable);
        }
        self.record
            .lock()
            .map(|mut current| *current = Some(record.clone()))
            .map_err(|_| CredentialStoreError::Failed)
    }

    fn delete(&self) -> Result<(), CredentialStoreError> {
        if self.fail_operations.load(Ordering::Acquire) {
            return Err(CredentialStoreError::Unavailable);
        }
        self.record
            .lock()
            .map(|mut current| *current = None)
            .map_err(|_| CredentialStoreError::Failed)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self) -> Result<keyring::Entry, CredentialStoreError> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .map_err(|_| CredentialStoreError::Unavailable)
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn load(&self) -> Result<Option<SpotifyCredentialRecord>, CredentialStoreError> {
        let entry = self.entry()?;
        let value = match entry.get_password() {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(_) => return Err(CredentialStoreError::Unavailable),
        };
        serde_json::from_str(&value)
            .map(Some)
            .map_err(|_| CredentialStoreError::InvalidRecord)
    }

    fn save(&self, record: &SpotifyCredentialRecord) -> Result<(), CredentialStoreError> {
        let value = serde_json::to_string(record).map_err(|_| CredentialStoreError::Failed)?;
        self.entry()?
            .set_password(&value)
            .map_err(|_| CredentialStoreError::Unavailable)
    }

    fn delete(&self) -> Result<(), CredentialStoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialStoreError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_round_trip_uses_memory_store() {
        let store = MemoryCredentialStore::new();
        let record = SpotifyCredentialRecord::new("public-client", "vn", "refresh-token").unwrap();
        store.save(&record).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.client_id(), "public-client");
        assert_eq!(loaded.market(), "VN");
        assert_eq!(loaded.refresh_token(), "refresh-token");
        store.delete().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn credential_debug_redacts_refresh_token() {
        let record =
            SpotifyCredentialRecord::new("public-client", "VN", "secret-refresh-token").unwrap();
        let debug = format!("{record:?}");
        assert!(!debug.contains("secret-refresh-token"));
        assert!(debug.contains("redacted"));
    }

    #[test]
    fn credential_store_failure_fails_closed() {
        let store = MemoryCredentialStore::new();
        store.set_failure(true);
        let record = SpotifyCredentialRecord::new("public-client", "VN", "refresh-token").unwrap();
        assert_eq!(store.load(), Err(CredentialStoreError::Unavailable));
        assert_eq!(store.save(&record), Err(CredentialStoreError::Unavailable));
        assert_eq!(store.delete(), Err(CredentialStoreError::Unavailable));
    }
}
