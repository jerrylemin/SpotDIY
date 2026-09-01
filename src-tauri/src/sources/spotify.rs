use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;
use uuid::Uuid;

use crate::credentials::{
    CredentialError, CredentialStoreError, KeyringCredentialStore, SharedCredentialStore,
    SpotifyCredentialRecord,
};
use crate::domain::{ProviderKind, SourceCapabilities};
use crate::search::types::{
    EngagementKind, ProviderRuntimeStatus, ProviderSearchErrorCode, ProviderSearchRequest,
    ProviderSearchSection, ProviderSearchState, SafeUrl, SearchCancellation, SearchEntityKind,
    SearchResult,
};
use crate::sources::{
    cancelled_provider_section, failed_provider_section, is_cancelled, ready_provider_section,
    validate_provider_url, SourceAdapter,
};

pub const SPOTIFY_AUTHORIZATION_ENDPOINT: &str = "https://accounts.spotify.com/authorize";
pub const SPOTIFY_TOKEN_ENDPOINT: &str = "https://accounts.spotify.com/api/token";
pub const SPOTIFY_SEARCH_ENDPOINT: &str = "https://api.spotify.com/v1/search";
pub const SPOTIFY_CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);
pub const SPOTIFY_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
pub const SPOTIFY_SEARCH_LIMIT: u8 = 10;

const SUPPORTED_ENTITIES: &[SearchEntityKind] = &[
    SearchEntityKind::Track,
    SearchEntityKind::Artist,
    SearchEntityKind::Album,
];

const SPOTIFY_CAPABILITIES: SourceCapabilities = SourceCapabilities {
    search: true,
    playback: false,
    metadata: true,
    artwork: true,
    lyrics: false,
    downloads: false,
    popularity: false,
    release_date: true,
    lyrics_metadata: false,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpotifyAuthState {
    Disabled,
    SetupRequired,
    Connected,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifySetupStatus {
    pub enabled: bool,
    pub configured: bool,
    pub available: bool,
    pub state: SpotifyAuthState,
    pub market: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyAuthorizationRequest {
    pub authorization_url: String,
    pub redirect_uri: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SpotifyMarketError {
    #[error("Spotify market must be exactly two ASCII letters")]
    Invalid,
}

pub fn validate_market(value: &str) -> Result<String, SpotifyMarketError> {
    let value = value.trim();
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(SpotifyMarketError::Invalid);
    }
    Ok(value.to_ascii_uppercase())
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SpotifyTransportError {
    #[error("Spotify authorization is required")]
    Unauthorized,
    #[error("Spotify rejected the request")]
    Forbidden,
    #[error("Spotify rate limit exceeded")]
    RateLimited { retry_after_seconds: Option<u64> },
    #[error("Spotify development quota exceeded")]
    QuotaExceeded,
    #[error("Spotify service is unavailable")]
    Server,
    #[error("Spotify request timed out")]
    Timeout,
    #[error("Spotify returned an invalid response")]
    InvalidResponse,
    #[error("Spotify network request failed")]
    Network,
}

#[derive(Clone)]
pub struct SpotifyTokenResponse {
    access_token: String,
    expires_in_seconds: u64,
    refresh_token: Option<String>,
}

impl SpotifyTokenResponse {
    pub fn new(
        access_token: impl Into<String>,
        expires_in_seconds: u64,
        refresh_token: Option<String>,
    ) -> Result<Self, SpotifyTransportError> {
        let access_token = access_token.into();
        if access_token.trim().is_empty() || expires_in_seconds == 0 {
            return Err(SpotifyTransportError::InvalidResponse);
        }
        Ok(Self {
            access_token,
            expires_in_seconds,
            refresh_token,
        })
    }

    pub(crate) fn access_token(&self) -> &str {
        &self.access_token
    }

    pub(crate) fn expires_in_seconds(&self) -> u64 {
        self.expires_in_seconds
    }

    pub(crate) fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }
}

impl fmt::Debug for SpotifyTokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpotifyTokenResponse")
            .field("access_token", &"redacted")
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "redacted"),
            )
            .finish()
    }
}

#[async_trait]
pub trait SpotifyHttpTransport: Send + Sync {
    async fn exchange_code(
        &self,
        client_id: &str,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<SpotifyTokenResponse, SpotifyTransportError>;

    async fn refresh_token(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<SpotifyTokenResponse, SpotifyTransportError>;

    async fn search(
        &self,
        access_token: &str,
        query: &str,
        market: &str,
    ) -> Result<Vec<SearchResult>, SpotifyTransportError>;
}

#[derive(Clone)]
pub struct ReqwestSpotifyTransport {
    client: reqwest::Client,
}

impl ReqwestSpotifyTransport {
    pub fn new() -> Result<Self, SpotifyTransportError> {
        let client = reqwest::Client::builder()
            .timeout(SPOTIFY_HTTP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| SpotifyTransportError::Network)?;
        Ok(Self { client })
    }

    async fn send_json<T: DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, SpotifyTransportError> {
        if !response.status().is_success() {
            return Err(classify_error_response(response).await);
        }
        response
            .json::<T>()
            .await
            .map_err(|_| SpotifyTransportError::InvalidResponse)
    }
}

impl Default for ReqwestSpotifyTransport {
    fn default() -> Self {
        Self::new().expect("Spotify HTTP client configuration is static")
    }
}

#[async_trait]
impl SpotifyHttpTransport for ReqwestSpotifyTransport {
    async fn exchange_code(
        &self,
        client_id: &str,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<SpotifyTokenResponse, SpotifyTransportError> {
        let response = self
            .client
            .post(SPOTIFY_TOKEN_ENDPOINT)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("client_id", client_id),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let token: TokenWire = Self::send_json(response).await?;
        token.into_response()
    }

    async fn refresh_token(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<SpotifyTokenResponse, SpotifyTransportError> {
        let response = self
            .client
            .post(SPOTIFY_TOKEN_ENDPOINT)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
            ])
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let token: TokenWire = Self::send_json(response).await?;
        token.into_response()
    }

    async fn search(
        &self,
        access_token: &str,
        query: &str,
        market: &str,
    ) -> Result<Vec<SearchResult>, SpotifyTransportError> {
        let url = build_search_url(query, market)?;
        let response = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let body: SpotifySearchEnvelope = Self::send_json(response).await?;
        normalize_spotify_response(body)
    }
}

#[derive(Debug, Deserialize)]
struct TokenWire {
    access_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
}

impl TokenWire {
    fn into_response(self) -> Result<SpotifyTokenResponse, SpotifyTransportError> {
        SpotifyTokenResponse::new(
            self.access_token
                .ok_or(SpotifyTransportError::InvalidResponse)?,
            self.expires_in.unwrap_or(3600),
            self.refresh_token,
        )
    }
}

fn map_reqwest_error(error: reqwest::Error) -> SpotifyTransportError {
    if error.is_timeout() {
        SpotifyTransportError::Timeout
    } else {
        SpotifyTransportError::Network
    }
}

async fn classify_error_response(response: reqwest::Response) -> SpotifyTransportError {
    let status = response.status();
    let retry_after_seconds = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let body = response.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        SpotifyTransportError::Unauthorized
    } else if status == reqwest::StatusCode::FORBIDDEN {
        SpotifyTransportError::Forbidden
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let quota = serde_json::from_str::<SpotifyErrorWire>(&body)
            .ok()
            .and_then(|value| value.reason)
            .is_some_and(|reason| reason.eq_ignore_ascii_case("QUOTA_EXCEEDED"));
        if quota {
            SpotifyTransportError::QuotaExceeded
        } else {
            SpotifyTransportError::RateLimited {
                retry_after_seconds,
            }
        }
    } else if status.is_server_error() {
        SpotifyTransportError::Server
    } else {
        SpotifyTransportError::InvalidResponse
    }
}

#[derive(Debug, Deserialize)]
struct SpotifyErrorWire {
    reason: Option<String>,
}

#[derive(Clone)]
pub struct SpotifyAuthService {
    store: SharedCredentialStore,
    transport: Arc<dyn SpotifyHttpTransport>,
    access_token: Arc<Mutex<Option<CachedAccessToken>>>,
    pending: Arc<Mutex<Option<PendingAuthorization>>>,
    enabled_override: Option<bool>,
}

struct CachedAccessToken {
    client_id: String,
    refresh_token: String,
    access_token: String,
    expires_at: Instant,
}

struct PendingAuthorization {
    listener: TcpListener,
    state: String,
    code_verifier: String,
    client_id: String,
    market: String,
    redirect_uri: String,
}

impl SpotifyAuthService {
    pub fn new(store: SharedCredentialStore) -> Self {
        Self::with_transport(store, Arc::new(ReqwestSpotifyTransport::default()))
    }

    pub fn production() -> Self {
        Self::new(Arc::new(KeyringCredentialStore::new()))
    }

    pub fn with_transport(
        store: SharedCredentialStore,
        transport: Arc<dyn SpotifyHttpTransport>,
    ) -> Self {
        Self {
            store,
            transport,
            access_token: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(None)),
            enabled_override: None,
        }
    }

    #[cfg(test)]
    fn with_test_gate(
        store: SharedCredentialStore,
        transport: Arc<dyn SpotifyHttpTransport>,
        enabled: bool,
    ) -> Self {
        let mut service = Self::with_transport(store, transport);
        service.enabled_override = Some(enabled);
        service
    }

    pub fn enabled() -> bool {
        std::env::var("SPOTDIY_ENABLE_SPOTIFY_DEV").ok().as_deref() == Some("1")
    }

    fn gate_enabled(&self) -> bool {
        self.enabled_override.unwrap_or_else(Self::enabled)
    }

    pub fn setup_status(&self) -> SpotifySetupStatus {
        if !self.gate_enabled() {
            return SpotifySetupStatus {
                enabled: false,
                configured: false,
                available: false,
                state: SpotifyAuthState::Disabled,
                market: None,
                detail: Some("Spotify catalog search is disabled by default.".to_owned()),
            };
        }
        match self.store.load() {
            Ok(Some(record)) => SpotifySetupStatus {
                enabled: true,
                configured: true,
                available: true,
                state: SpotifyAuthState::Connected,
                market: Some(record.market().to_owned()),
                detail: None,
            },
            Ok(None) => SpotifySetupStatus {
                enabled: true,
                configured: false,
                available: false,
                state: SpotifyAuthState::SetupRequired,
                market: None,
                detail: Some(
                    "Connect Spotify with a public client ID to search the catalog.".into(),
                ),
            },
            Err(_) => SpotifySetupStatus {
                enabled: true,
                configured: false,
                available: false,
                state: SpotifyAuthState::Unavailable,
                market: None,
                detail: Some("Secure Spotify credential storage is unavailable.".into()),
            },
        }
    }

    pub fn runtime_status(&self) -> ProviderRuntimeStatus {
        match self.setup_status().state {
            SpotifyAuthState::Disabled => ProviderRuntimeStatus::Disabled,
            SpotifyAuthState::Connected => ProviderRuntimeStatus::Ready,
            SpotifyAuthState::SetupRequired => ProviderRuntimeStatus::Missing,
            SpotifyAuthState::Unavailable => ProviderRuntimeStatus::Broken,
        }
    }

    pub fn disconnect(&self) -> Result<SpotifySetupStatus, SpotifyAuthError> {
        self.store.delete().map_err(SpotifyAuthError::Store)?;
        self.access_token
            .lock()
            .map(|mut token| *token = None)
            .map_err(|_| SpotifyAuthError::StateUnavailable)?;
        self.pending
            .lock()
            .map(|mut pending| *pending = None)
            .map_err(|_| SpotifyAuthError::StateUnavailable)?;
        Ok(self.setup_status())
    }

    pub async fn begin_authorization(
        &self,
        client_id: impl Into<String>,
        market: &str,
    ) -> Result<SpotifyAuthorizationRequest, SpotifyAuthError> {
        self.ensure_enabled()?;
        let client_id = validate_client_id(client_id.into())?;
        let market = validate_market(market).map_err(|_| SpotifyAuthError::InvalidMarket)?;
        let (listener, address) = bind_loopback().await?;
        let redirect_uri = format!("http://127.0.0.1:{}/callback", address.port());
        let state = generate_oauth_state();
        let code_verifier = generate_pkce_verifier();
        let authorization_url =
            build_authorization_url(&client_id, &redirect_uri, &state, &code_verifier)?;
        let pending = PendingAuthorization {
            listener,
            state,
            code_verifier,
            client_id,
            market,
            redirect_uri: redirect_uri.clone(),
        };
        self.pending
            .lock()
            .map_err(|_| SpotifyAuthError::StateUnavailable)?
            .replace(pending);
        Ok(SpotifyAuthorizationRequest {
            authorization_url: authorization_url.into(),
            redirect_uri,
        })
    }

    pub async fn complete_authorization(&self) -> Result<SpotifySetupStatus, SpotifyAuthError> {
        self.complete_authorization_with_timeout(SPOTIFY_CALLBACK_TIMEOUT)
            .await
    }

    async fn complete_authorization_with_timeout(
        &self,
        timeout_duration: Duration,
    ) -> Result<SpotifySetupStatus, SpotifyAuthError> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| SpotifyAuthError::StateUnavailable)?
            .take()
            .ok_or(SpotifyAuthError::SetupRequired)?;
        let PendingAuthorization {
            listener,
            state,
            code_verifier,
            client_id,
            market,
            redirect_uri,
        } = pending;
        let accepted = tokio::time::timeout(timeout_duration, listener.accept())
            .await
            .map_err(|_| SpotifyAuthError::CallbackTimeout)?
            .map_err(|_| SpotifyAuthError::CallbackUnavailable)?;
        let (mut stream, peer) = accepted;
        if !peer.ip().is_loopback() {
            return Err(SpotifyAuthError::CallbackUnavailable);
        }
        let target = read_callback_target(&mut stream).await?;
        let callback = callback_url_from_target(&target)?;
        let code = validate_callback(&callback, &state)?;
        write_callback_response(&mut stream).await?;
        let token = self
            .transport
            .exchange_code(&client_id, &code, &redirect_uri, &code_verifier)
            .await
            .map_err(SpotifyAuthError::Transport)?;
        let refresh_token = token
            .refresh_token()
            .filter(|value| !value.trim().is_empty())
            .ok_or(SpotifyAuthError::MissingRefreshToken)?;
        let record = SpotifyCredentialRecord::new(&client_id, &market, refresh_token)
            .map_err(SpotifyAuthError::Credential)?;
        self.store.save(&record).map_err(SpotifyAuthError::Store)?;
        self.cache_token(&record, &token)?;
        Ok(self.setup_status())
    }

    pub async fn search(
        &self,
        query: &str,
        requested_market: Option<&str>,
        cancellation: &SearchCancellation,
    ) -> Result<Vec<SearchResult>, SpotifyAuthError> {
        self.ensure_enabled()?;
        if is_cancelled(cancellation) {
            return Err(SpotifyAuthError::Cancelled);
        }
        let record = self
            .store
            .load()
            .map_err(SpotifyAuthError::Store)?
            .ok_or(SpotifyAuthError::SetupRequired)?;
        let market = requested_market
            .map(|value| validate_market(value).map_err(|_| SpotifyAuthError::InvalidMarket))
            .transpose()?
            .unwrap_or_else(|| record.market().to_owned());
        let token = self.access_token_for(&record, false).await?;
        if is_cancelled(cancellation) {
            return Err(SpotifyAuthError::Cancelled);
        }
        match self.transport.search(&token, query, &market).await {
            Ok(results) => Ok(results),
            Err(SpotifyTransportError::Unauthorized) => {
                let token = self.access_token_for(&record, true).await?;
                if is_cancelled(cancellation) {
                    return Err(SpotifyAuthError::Cancelled);
                }
                self.transport
                    .search(&token, query, &market)
                    .await
                    .map_err(SpotifyAuthError::Transport)
            }
            Err(error) => Err(SpotifyAuthError::Transport(error)),
        }
    }

    fn ensure_enabled(&self) -> Result<(), SpotifyAuthError> {
        self.gate_enabled()
            .then_some(())
            .ok_or(SpotifyAuthError::Disabled)
    }

    async fn access_token_for(
        &self,
        record: &SpotifyCredentialRecord,
        force_refresh: bool,
    ) -> Result<String, SpotifyAuthError> {
        if !force_refresh {
            if let Some(cached) = self
                .access_token
                .lock()
                .map_err(|_| SpotifyAuthError::StateUnavailable)?
                .as_ref()
                .filter(|cached| {
                    cached.client_id == record.client_id()
                        && cached.refresh_token == record.refresh_token()
                        && cached.expires_at > Instant::now() + Duration::from_secs(30)
                })
            {
                return Ok(cached.access_token.clone());
            }
        } else if let Ok(mut cached) = self.access_token.lock() {
            *cached = None;
        }

        let token = self
            .transport
            .refresh_token(record.client_id(), record.refresh_token())
            .await
            .map_err(SpotifyAuthError::Transport)?;
        let record = if let Some(refresh_token) = token.refresh_token() {
            let rotated =
                SpotifyCredentialRecord::new(record.client_id(), record.market(), refresh_token)
                    .map_err(SpotifyAuthError::Credential)?;
            self.store.save(&rotated).map_err(SpotifyAuthError::Store)?;
            rotated
        } else {
            record.clone()
        };
        self.cache_token(&record, &token)?;
        Ok(token.access_token().to_owned())
    }

    fn cache_token(
        &self,
        record: &SpotifyCredentialRecord,
        token: &SpotifyTokenResponse,
    ) -> Result<(), SpotifyAuthError> {
        let expires_at = Instant::now()
            .checked_add(Duration::from_secs(token.expires_in_seconds()))
            .unwrap_or_else(Instant::now);
        let cached = CachedAccessToken {
            client_id: record.client_id().to_owned(),
            refresh_token: record.refresh_token().to_owned(),
            access_token: token.access_token().to_owned(),
            expires_at,
        };
        self.access_token
            .lock()
            .map(|mut value| *value = Some(cached))
            .map_err(|_| SpotifyAuthError::StateUnavailable)
    }
}

impl Default for SpotifyAuthService {
    fn default() -> Self {
        Self::production()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SpotifyAuthError {
    #[error("Spotify catalog search is disabled")]
    Disabled,
    #[error("Spotify setup is required")]
    SetupRequired,
    #[error("Spotify client ID is invalid")]
    InvalidClientId,
    #[error("Spotify market is invalid")]
    InvalidMarket,
    #[error("Spotify callback timed out")]
    CallbackTimeout,
    #[error("Spotify callback is unavailable")]
    CallbackUnavailable,
    #[error("Spotify callback path is invalid")]
    CallbackInvalidPath,
    #[error("Spotify callback state did not match")]
    CallbackStateMismatch,
    #[error("Spotify authorization was denied")]
    AuthorizationDenied,
    #[error("Spotify callback did not contain an authorization code")]
    MissingAuthorizationCode,
    #[error("Spotify authorization did not return a refresh token")]
    MissingRefreshToken,
    #[error("Spotify authorization state is unavailable")]
    StateUnavailable,
    #[error("Spotify callback request is invalid")]
    InvalidCallbackRequest,
    #[error("secure Spotify credential record is invalid")]
    Credential(CredentialError),
    #[error("secure Spotify credential storage failed")]
    Store(CredentialStoreError),
    #[error("Spotify transport failed")]
    Transport(SpotifyTransportError),
    #[error("Spotify search was cancelled")]
    Cancelled,
}

fn validate_client_id(value: String) -> Result<String, SpotifyAuthError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > 128 || value.chars().any(char::is_whitespace) {
        return Err(SpotifyAuthError::InvalidClientId);
    }
    Ok(value)
}

pub fn generate_pkce_verifier() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn pkce_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn generate_oauth_state() -> String {
    Uuid::new_v4().to_string()
}

pub fn build_authorization_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_verifier: &str,
) -> Result<Url, SpotifyAuthError> {
    let mut url = Url::parse(SPOTIFY_AUTHORIZATION_ENDPOINT)
        .map_err(|_| SpotifyAuthError::InvalidCallbackRequest)?;
    {
        let challenge = pkce_challenge(code_verifier);
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge);
    }
    Ok(url)
}

pub fn build_search_url(query: &str, market: &str) -> Result<Url, SpotifyTransportError> {
    let market = validate_market(market).map_err(|_| SpotifyTransportError::InvalidResponse)?;
    let mut url =
        Url::parse(SPOTIFY_SEARCH_ENDPOINT).map_err(|_| SpotifyTransportError::InvalidResponse)?;
    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs
            .append_pair("q", query)
            .append_pair("type", "track,artist,album")
            .append_pair("market", &market)
            .append_pair("limit", &SPOTIFY_SEARCH_LIMIT.to_string())
            .append_pair("offset", "0");
    }
    Ok(url)
}

pub async fn bind_loopback() -> Result<(TcpListener, SocketAddr), SpotifyAuthError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| SpotifyAuthError::CallbackUnavailable)?;
    let address = listener
        .local_addr()
        .map_err(|_| SpotifyAuthError::CallbackUnavailable)?;
    if !address.ip().is_loopback() || address.ip().is_unspecified() || address.port() == 0 {
        return Err(SpotifyAuthError::CallbackUnavailable);
    }
    Ok((listener, address))
}

pub fn validate_callback(callback: &Url, expected_state: &str) -> Result<String, SpotifyAuthError> {
    if callback.scheme() != "http"
        || callback.host_str() != Some("127.0.0.1")
        || callback.path() != "/callback"
    {
        return Err(SpotifyAuthError::CallbackInvalidPath);
    }
    if callback
        .query_pairs()
        .find(|(key, _)| key == "error")
        .is_some()
    {
        return Err(SpotifyAuthError::AuthorizationDenied);
    }
    let state = callback
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .ok_or(SpotifyAuthError::CallbackStateMismatch)?;
    if state != expected_state {
        return Err(SpotifyAuthError::CallbackStateMismatch);
    }
    callback
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty())
        .ok_or(SpotifyAuthError::MissingAuthorizationCode)
}

async fn read_callback_target(
    stream: &mut tokio::net::TcpStream,
) -> Result<String, SpotifyAuthError> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 512];
    while bytes.len() < 8 * 1024 {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| SpotifyAuthError::InvalidCallbackRequest)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let request =
        std::str::from_utf8(&bytes).map_err(|_| SpotifyAuthError::InvalidCallbackRequest)?;
    let mut parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    if parts.next() != Some("GET") {
        return Err(SpotifyAuthError::InvalidCallbackRequest);
    }
    parts
        .next()
        .filter(|target| target.starts_with('/') && target.len() <= 4096)
        .map(str::to_owned)
        .ok_or(SpotifyAuthError::InvalidCallbackRequest)
}

fn callback_url_from_target(target: &str) -> Result<Url, SpotifyAuthError> {
    Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| SpotifyAuthError::InvalidCallbackRequest)
}

async fn write_callback_response(
    stream: &mut tokio::net::TcpStream,
) -> Result<(), SpotifyAuthError> {
    const BODY: &str = "Spotify authorization received. You may close this window.";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        BODY.len(),
        BODY
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|_| SpotifyAuthError::CallbackUnavailable)
}

fn normalize_spotify_response(
    response: SpotifySearchEnvelope,
) -> Result<Vec<SearchResult>, SpotifyTransportError> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    let mut rank = 0_u32;
    if let Some(page) = response.tracks {
        for item in page.items.unwrap_or_default().into_iter().take(10) {
            if let Some(result) = normalize_track(item, rank) {
                if seen.insert((result.entity_kind, result.provider_item_id.clone())) {
                    results.push(result);
                    rank = rank.saturating_add(1);
                }
            }
        }
    }
    if let Some(page) = response.artists {
        for item in page.items.unwrap_or_default().into_iter().take(10) {
            if let Some(result) = normalize_artist(item, rank) {
                if seen.insert((result.entity_kind, result.provider_item_id.clone())) {
                    results.push(result);
                    rank = rank.saturating_add(1);
                }
            }
        }
    }
    if let Some(page) = response.albums {
        for item in page.items.unwrap_or_default().into_iter().take(10) {
            if let Some(result) = normalize_album(item, rank)? {
                if seen.insert((result.entity_kind, result.provider_item_id.clone())) {
                    results.push(result);
                    rank = rank.saturating_add(1);
                }
            }
        }
    }
    Ok(results)
}

fn normalize_track(item: SpotifyTrackWire, rank: u32) -> Option<SearchResult> {
    let id = item.id?.trim().to_owned();
    let title = item.name?.trim().to_owned();
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let artists = item
        .artists
        .unwrap_or_default()
        .into_iter()
        .filter_map(|artist| artist.name)
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    let album = item.album;
    let (album_name, artwork_url) = album
        .as_ref()
        .map(|album| {
            (
                album.name.clone().filter(|name| !name.trim().is_empty()),
                first_artwork(album.images.as_ref()),
            )
        })
        .unwrap_or((None, None));
    let engagement_count = None;
    Some(SearchResult {
        provider: ProviderKind::Spotify,
        entity_kind: SearchEntityKind::Track,
        provider_item_id: id,
        canonical_url: item
            .external_urls
            .and_then(|urls| urls.spotify)
            .and_then(|url| validate_provider_url(ProviderKind::Spotify, &url).ok()),
        title,
        artists,
        album: album_name,
        duration_ms: optional_u64(item.duration_ms.as_ref()),
        artwork_url,
        published_at: None,
        engagement_count,
        engagement_kind: engagement_count.map(|_| EngagementKind::Views),
        explicit: item.explicit,
        local_track_id: None,
        local_source_id: None,
        original_rank: rank,
    })
}

fn normalize_artist(item: SpotifyArtistWire, rank: u32) -> Option<SearchResult> {
    let id = item.id?.trim().to_owned();
    let title = item.name?.trim().to_owned();
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let artwork_url = first_artwork(item.images.as_ref());
    Some(SearchResult {
        provider: ProviderKind::Spotify,
        entity_kind: SearchEntityKind::Artist,
        provider_item_id: id,
        canonical_url: item
            .external_urls
            .and_then(|urls| urls.spotify)
            .and_then(|url| validate_provider_url(ProviderKind::Spotify, &url).ok()),
        title: title.clone(),
        artists: vec![title],
        album: None,
        duration_ms: None,
        artwork_url,
        published_at: None,
        engagement_count: None,
        engagement_kind: None,
        explicit: None,
        local_track_id: None,
        local_source_id: None,
        original_rank: rank,
    })
}

fn normalize_album(
    item: SpotifyAlbumWire,
    rank: u32,
) -> Result<Option<SearchResult>, SpotifyTransportError> {
    let Some(id) = item.id.map(|value| value.trim().to_owned()) else {
        return Ok(None);
    };
    let Some(title) = item.name.map(|value| value.trim().to_owned()) else {
        return Ok(None);
    };
    if id.is_empty() || title.is_empty() {
        return Ok(None);
    }
    let published_at = match (
        item.release_date.as_deref(),
        item.release_date_precision.as_deref(),
    ) {
        (Some(value), Some(precision)) => Some(
            crate::search::types::PartialDate::new(
                value,
                match precision {
                    "year" => crate::search::types::PartialDatePrecision::Year,
                    "month" => crate::search::types::PartialDatePrecision::Month,
                    "day" => crate::search::types::PartialDatePrecision::Day,
                    _ => return Err(SpotifyTransportError::InvalidResponse),
                },
            )
            .map_err(|_| SpotifyTransportError::InvalidResponse)?,
        ),
        (None, None) => None,
        _ => return Err(SpotifyTransportError::InvalidResponse),
    };
    let artists = item
        .artists
        .unwrap_or_default()
        .into_iter()
        .filter_map(|artist| artist.name)
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    Ok(Some(SearchResult {
        provider: ProviderKind::Spotify,
        entity_kind: SearchEntityKind::Album,
        provider_item_id: id,
        canonical_url: item
            .external_urls
            .and_then(|urls| urls.spotify)
            .and_then(|url| validate_provider_url(ProviderKind::Spotify, &url).ok()),
        title,
        artists,
        album: None,
        duration_ms: None,
        artwork_url: first_artwork(item.images.as_ref()),
        published_at,
        engagement_count: None,
        engagement_kind: None,
        explicit: None,
        local_track_id: None,
        local_source_id: None,
        original_rank: rank,
    }))
}

fn optional_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| match value {
        Value::Number(number) => number.as_u64(),
        _ => None,
    })
}

fn first_artwork(images: Option<&Vec<SpotifyImageWire>>) -> Option<SafeUrl> {
    images
        .into_iter()
        .flatten()
        .filter_map(|image| image.url.as_deref())
        .find_map(crate::sources::sanitize_artwork_url)
}

#[derive(Debug, Deserialize)]
struct SpotifySearchEnvelope {
    tracks: Option<SpotifyPage<SpotifyTrackWire>>,
    artists: Option<SpotifyPage<SpotifyArtistWire>>,
    albums: Option<SpotifyPage<SpotifyAlbumWire>>,
}

#[derive(Debug, Deserialize)]
struct SpotifyPage<T> {
    items: Option<Vec<T>>,
}

#[derive(Debug, Deserialize)]
struct SpotifyTrackWire {
    id: Option<String>,
    name: Option<String>,
    artists: Option<Vec<SpotifyNamedWire>>,
    album: Option<SpotifyAlbumSummaryWire>,
    duration_ms: Option<Value>,
    explicit: Option<bool>,
    external_urls: Option<SpotifyExternalUrlsWire>,
}

#[derive(Debug, Deserialize)]
struct SpotifyNamedWire {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyAlbumSummaryWire {
    name: Option<String>,
    images: Option<Vec<SpotifyImageWire>>,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtistWire {
    id: Option<String>,
    name: Option<String>,
    images: Option<Vec<SpotifyImageWire>>,
    external_urls: Option<SpotifyExternalUrlsWire>,
}

#[derive(Debug, Deserialize)]
struct SpotifyAlbumWire {
    id: Option<String>,
    name: Option<String>,
    artists: Option<Vec<SpotifyNamedWire>>,
    images: Option<Vec<SpotifyImageWire>>,
    release_date: Option<String>,
    release_date_precision: Option<String>,
    external_urls: Option<SpotifyExternalUrlsWire>,
}

#[derive(Debug, Deserialize)]
struct SpotifyImageWire {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SpotifyExternalUrlsWire {
    spotify: Option<String>,
}

pub fn parse_spotify_search_response(
    value: &str,
) -> Result<Vec<SearchResult>, SpotifyTransportError> {
    let response = serde_json::from_str::<SpotifySearchEnvelope>(value)
        .map_err(|_| SpotifyTransportError::InvalidResponse)?;
    normalize_spotify_response(response)
}

pub struct SpotifySourceAdapter {
    auth: SpotifyAuthService,
}

impl SpotifySourceAdapter {
    pub fn new(auth: SpotifyAuthService) -> Self {
        Self { auth }
    }

    pub fn production() -> Self {
        Self::new(SpotifyAuthService::production())
    }

    pub fn auth_service(&self) -> SpotifyAuthService {
        self.auth.clone()
    }
}

impl SourceAdapter for SpotifySourceAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Spotify
    }

    fn capabilities(&self) -> SourceCapabilities {
        SPOTIFY_CAPABILITIES
    }

    fn supported_entities(&self) -> &'static [SearchEntityKind] {
        SUPPORTED_ENTITIES
    }

    fn runtime_status(&self) -> ProviderRuntimeStatus {
        self.auth.runtime_status()
    }

    fn search(
        &self,
        request: ProviderSearchRequest,
        cancellation: SearchCancellation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ProviderSearchSection> + Send + '_>>
    {
        Box::pin(async move {
            if is_cancelled(&cancellation) {
                return cancelled_provider_section(ProviderKind::Spotify);
            }
            if request.limit == 0
                || request.query.trim().is_empty()
                || !request
                    .entities
                    .iter()
                    .any(|entity| SUPPORTED_ENTITIES.contains(entity))
            {
                return ready_provider_section(ProviderKind::Spotify, Vec::new());
            }
            match self
                .auth
                .search(&request.query, request.market.as_deref(), &cancellation)
                .await
            {
                Ok(mut results) => {
                    results.truncate(usize::from(request.limit).min(50));
                    ready_provider_section(ProviderKind::Spotify, results)
                }
                Err(error) => spotify_error_section(error),
            }
        })
    }
}

fn spotify_error_section(error: SpotifyAuthError) -> ProviderSearchSection {
    match error {
        SpotifyAuthError::Cancelled => cancelled_provider_section(ProviderKind::Spotify),
        SpotifyAuthError::Disabled => failed_provider_section(
            ProviderKind::Spotify,
            ProviderSearchErrorCode::Disabled,
            Some("Spotify catalog search is disabled by default.".into()),
        ),
        SpotifyAuthError::SetupRequired => failed_provider_section(
            ProviderKind::Spotify,
            ProviderSearchErrorCode::Unavailable,
            Some("Spotify setup is required.".into()),
        ),
        SpotifyAuthError::Transport(transport) => {
            let (code, detail) = match transport {
                SpotifyTransportError::Unauthorized => (
                    ProviderSearchErrorCode::Unavailable,
                    "Spotify authorization is required.",
                ),
                SpotifyTransportError::Forbidden => (
                    ProviderSearchErrorCode::Failed,
                    "Spotify rejected the catalog request.",
                ),
                SpotifyTransportError::RateLimited {
                    retry_after_seconds,
                } => {
                    return ProviderSearchSection {
                        provider: ProviderKind::Spotify,
                        state: ProviderSearchState::Failed,
                        results: Vec::new(),
                        error: Some(crate::search::types::ProviderSearchError {
                            code: ProviderSearchErrorCode::RateLimited,
                            detail: Some("Spotify rate limit exceeded.".into()),
                            retry_after_seconds,
                        }),
                    };
                }
                SpotifyTransportError::QuotaExceeded => (
                    ProviderSearchErrorCode::QuotaExceeded,
                    "Spotify development quota exceeded.",
                ),
                SpotifyTransportError::Timeout => (
                    ProviderSearchErrorCode::Timeout,
                    "Spotify request timed out.",
                ),
                SpotifyTransportError::InvalidResponse => (
                    ProviderSearchErrorCode::InvalidResponse,
                    "Spotify returned an invalid response.",
                ),
                SpotifyTransportError::Server | SpotifyTransportError::Network => {
                    (ProviderSearchErrorCode::Failed, "Spotify request failed.")
                }
            };
            failed_provider_section(ProviderKind::Spotify, code, Some(detail.into()))
        }
        SpotifyAuthError::InvalidMarket
        | SpotifyAuthError::InvalidClientId
        | SpotifyAuthError::CallbackTimeout
        | SpotifyAuthError::CallbackUnavailable
        | SpotifyAuthError::CallbackInvalidPath
        | SpotifyAuthError::CallbackStateMismatch
        | SpotifyAuthError::AuthorizationDenied
        | SpotifyAuthError::MissingAuthorizationCode
        | SpotifyAuthError::MissingRefreshToken
        | SpotifyAuthError::StateUnavailable
        | SpotifyAuthError::InvalidCallbackRequest
        | SpotifyAuthError::Credential(_)
        | SpotifyAuthError::Store(_) => failed_provider_section(
            ProviderKind::Spotify,
            ProviderSearchErrorCode::Failed,
            Some("Spotify provider setup failed.".into()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{CredentialStore, MemoryCredentialStore};
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    type ExchangeCall = (String, String, String, String);
    type SearchOutcome = Result<Vec<SearchResult>, SpotifyTransportError>;

    #[derive(Clone)]
    struct FakeTransport {
        exchange_calls: Arc<Mutex<Vec<ExchangeCall>>>,
        refresh_calls: Arc<Mutex<Vec<(String, String)>>>,
        search_calls: Arc<Mutex<Vec<(String, String, String)>>>,
        exchange_result: Arc<Mutex<Result<SpotifyTokenResponse, SpotifyTransportError>>>,
        refresh_results: Arc<Mutex<VecDeque<Result<SpotifyTokenResponse, SpotifyTransportError>>>>,
        search_results: Arc<Mutex<VecDeque<SearchOutcome>>>,
        search_count: Arc<AtomicUsize>,
    }

    impl FakeTransport {
        fn new() -> Self {
            Self {
                exchange_calls: Arc::new(Mutex::new(Vec::new())),
                refresh_calls: Arc::new(Mutex::new(Vec::new())),
                search_calls: Arc::new(Mutex::new(Vec::new())),
                exchange_result: Arc::new(Mutex::new(Ok(SpotifyTokenResponse::new(
                    "exchange-access",
                    3600,
                    Some("refresh".into()),
                )
                .unwrap()))),
                refresh_results: Arc::new(Mutex::new(VecDeque::from([Ok(
                    SpotifyTokenResponse::new("refresh-access", 3600, None).unwrap(),
                )]))),
                search_results: Arc::new(Mutex::new(VecDeque::from([Ok(Vec::new())]))),
                search_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn with_search_results(self, results: VecDeque<SearchOutcome>) -> Self {
            *self.search_results.lock().unwrap() = results;
            self
        }

        fn with_refresh_results(
            self,
            results: VecDeque<Result<SpotifyTokenResponse, SpotifyTransportError>>,
        ) -> Self {
            *self.refresh_results.lock().unwrap() = results;
            self
        }
    }

    #[async_trait]
    impl SpotifyHttpTransport for FakeTransport {
        async fn exchange_code(
            &self,
            client_id: &str,
            code: &str,
            redirect_uri: &str,
            code_verifier: &str,
        ) -> Result<SpotifyTokenResponse, SpotifyTransportError> {
            self.exchange_calls.lock().unwrap().push((
                client_id.to_owned(),
                code.to_owned(),
                redirect_uri.to_owned(),
                code_verifier.to_owned(),
            ));
            self.exchange_result.lock().unwrap().clone()
        }

        async fn refresh_token(
            &self,
            client_id: &str,
            refresh_token: &str,
        ) -> Result<SpotifyTokenResponse, SpotifyTransportError> {
            self.refresh_calls
                .lock()
                .unwrap()
                .push((client_id.to_owned(), refresh_token.to_owned()));
            self.refresh_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| {
                    Ok(SpotifyTokenResponse::new("refresh-access", 3600, None).unwrap())
                })
        }

        async fn search(
            &self,
            access_token: &str,
            query: &str,
            market: &str,
        ) -> Result<Vec<SearchResult>, SpotifyTransportError> {
            self.search_count.fetch_add(1, Ordering::SeqCst);
            self.search_calls.lock().unwrap().push((
                access_token.to_owned(),
                query.to_owned(),
                market.to_owned(),
            ));
            self.search_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    fn service(
        enabled: bool,
        store: &MemoryCredentialStore,
        transport: &FakeTransport,
    ) -> SpotifyAuthService {
        SpotifyAuthService::with_test_gate(
            Arc::new(store.clone()),
            Arc::new(transport.clone()),
            enabled,
        )
    }

    fn credential_store() -> MemoryCredentialStore {
        let store = MemoryCredentialStore::new();
        store
            .save(&SpotifyCredentialRecord::new("public-client", "vn", "old-refresh").unwrap())
            .unwrap();
        store
    }

    #[test]
    fn pkce_verifier_has_43_to_128_allowed_characters() {
        let verifier = generate_pkce_verifier();
        assert!((43..=128).contains(&verifier.len()));
        assert!(verifier
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._~".contains(character)));
    }

    #[test]
    fn pkce_challenge_is_sha256_urlsafe_without_padding() {
        let verifier = "a".repeat(64);
        let challenge = pkce_challenge(&verifier);
        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='));
        assert!(challenge
            .chars()
            .all(|character| character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'));
    }

    #[test]
    fn oauth_state_is_fresh() {
        assert_ne!(generate_oauth_state(), generate_oauth_state());
    }

    #[test]
    fn callback_requires_exact_path() {
        let callback = Url::parse("http://127.0.0.1:42123/callback/extra?state=s&code=c").unwrap();
        assert_eq!(
            validate_callback(&callback, "s"),
            Err(SpotifyAuthError::CallbackInvalidPath)
        );
    }

    #[test]
    fn callback_rejects_state_mismatch() {
        let callback = Url::parse("http://127.0.0.1:42123/callback?state=wrong&code=c").unwrap();
        assert_eq!(
            validate_callback(&callback, "expected"),
            Err(SpotifyAuthError::CallbackStateMismatch)
        );
    }

    #[test]
    fn callback_rejects_oauth_error() {
        let callback =
            Url::parse("http://127.0.0.1:42123/callback?state=s&error=access_denied").unwrap();
        assert_eq!(
            validate_callback(&callback, "s"),
            Err(SpotifyAuthError::AuthorizationDenied)
        );
    }

    #[test]
    fn callback_times_out_at_120_seconds() {
        assert_eq!(SPOTIFY_CALLBACK_TIMEOUT, Duration::from_secs(120));
    }

    #[tokio::test]
    async fn loopback_binds_only_127_0_0_1_with_dynamic_port() {
        let (listener, address) = bind_loopback().await.unwrap();
        assert_eq!(address.ip().to_string(), "127.0.0.1");
        assert_ne!(address.port(), 0);
        drop(listener);
    }

    #[test]
    fn authorization_requests_no_scopes() {
        let url = build_authorization_url(
            "public-client",
            "http://127.0.0.1:42123/callback",
            "state",
            &"a".repeat(64),
        )
        .unwrap();
        let keys = url
            .query_pairs()
            .map(|(key, _)| key.into_owned())
            .collect::<Vec<_>>();
        assert!(keys.contains(&"code_challenge_method".to_owned()));
        assert!(keys.contains(&"code_challenge".to_owned()));
        assert!(!keys.contains(&"scope".to_owned()));
        assert!(!keys.contains(&"client_secret".to_owned()));
    }

    #[test]
    fn market_requires_two_ascii_letters_and_uppercases() {
        assert_eq!(validate_market("vn").unwrap(), "VN");
        for market in ["V", "VNM", "1N", "éé", ""] {
            assert_eq!(validate_market(market), Err(SpotifyMarketError::Invalid));
        }
    }

    #[tokio::test]
    async fn token_exchange_uses_pkce_without_secret() {
        let store = MemoryCredentialStore::new();
        let transport = FakeTransport::new();
        let auth = service(true, &store, &transport);
        let request = auth
            .begin_authorization("public-client", "VN")
            .await
            .unwrap();
        let authorization_url = Url::parse(&request.authorization_url).unwrap();
        let state = authorization_url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .unwrap();
        let port = Url::parse(&request.redirect_uri).unwrap().port().unwrap();
        let completion = tokio::spawn({
            let auth = auth.clone();
            async move { auth.complete_authorization().await }
        });
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let callback = format!("/callback?code=authorization-code&state={state}");
        stream
            .write_all(format!("GET {callback} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        completion.await.unwrap().unwrap();
        let calls = transport.exchange_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "public-client");
        assert_eq!(calls[0].1, "authorization-code");
        assert_eq!(calls[0].3.len(), 64);
        assert!(!request.authorization_url.contains("client_secret"));
    }

    #[tokio::test]
    async fn refresh_rotates_refresh_token() {
        let store = credential_store();
        let transport = FakeTransport::new().with_refresh_results(VecDeque::from([Ok(
            SpotifyTokenResponse::new("access", 3600, Some("rotated-refresh".into())).unwrap(),
        )]));
        let auth = service(true, &store, &transport);
        auth.search("signal", None, &SearchCancellation::new())
            .await
            .unwrap();
        assert_eq!(
            store.load().unwrap().unwrap().refresh_token(),
            "rotated-refresh"
        );
    }

    #[test]
    fn spotify_search_uses_exact_endpoint_and_limit_10() {
        let url = build_search_url("a signal", "vn").unwrap();
        assert_eq!(
            url.as_str().split('?').next().unwrap(),
            SPOTIFY_SEARCH_ENDPOINT
        );
        let pairs = url.query_pairs().collect::<Vec<_>>();
        assert!(pairs.contains(&("type".into(), "track,artist,album".into())));
        assert!(pairs.contains(&("market".into(), "VN".into())));
        assert!(pairs.contains(&("limit".into(), "10".into())));
        assert!(pairs.contains(&("offset".into(), "0".into())));
        assert_eq!(SPOTIFY_SEARCH_LIMIT, 10);
    }

    #[tokio::test]
    async fn spotify_401_refreshes_once_then_retries_once() {
        let store = credential_store();
        let transport = FakeTransport::new().with_search_results(VecDeque::from([
            Err(SpotifyTransportError::Unauthorized),
            Ok(Vec::new()),
        ]));
        let auth = service(true, &store, &transport);
        let initial = SpotifyTokenResponse::new("cached", 3600, None).unwrap();
        auth.cache_token(&store.load().unwrap().unwrap(), &initial)
            .unwrap();
        auth.search("signal", None, &SearchCancellation::new())
            .await
            .unwrap();
        assert_eq!(transport.refresh_calls.lock().unwrap().len(), 1);
        assert_eq!(transport.search_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn spotify_403_maps_forbidden() {
        let section = spotify_error_section(SpotifyAuthError::Transport(
            SpotifyTransportError::Forbidden,
        ));
        assert_eq!(section.error.unwrap().code, ProviderSearchErrorCode::Failed);
    }

    #[test]
    fn spotify_429_maps_rate_limit_and_retry_after() {
        let section = spotify_error_section(SpotifyAuthError::Transport(
            SpotifyTransportError::RateLimited {
                retry_after_seconds: Some(7),
            },
        ));
        let error = section.error.unwrap();
        assert_eq!(error.code, ProviderSearchErrorCode::RateLimited);
        assert_eq!(error.retry_after_seconds, Some(7));
    }

    #[test]
    fn spotify_quota_exceeded_maps_separately() {
        let section = spotify_error_section(SpotifyAuthError::Transport(
            SpotifyTransportError::QuotaExceeded,
        ));
        assert_eq!(
            section.error.unwrap().code,
            ProviderSearchErrorCode::QuotaExceeded
        );
    }

    #[test]
    fn spotify_malformed_json_is_typed_error() {
        assert_eq!(
            parse_spotify_search_response("not json"),
            Err(SpotifyTransportError::InvalidResponse)
        );
    }

    #[test]
    fn spotify_partial_release_date_preserves_precision() {
        let results = parse_spotify_search_response(
            r#"{"albums":{"items":[{"id":"album-1","name":"Album","release_date":"1981-12","release_date_precision":"month"}]}}"#,
        )
        .unwrap();
        let date = results[0].published_at.as_ref().unwrap();
        assert_eq!(date.value(), "1981-12");
        assert_eq!(
            date.precision(),
            crate::search::types::PartialDatePrecision::Month
        );
    }

    #[test]
    fn spotify_optional_fields_are_nullable() {
        let results = parse_spotify_search_response(
            r#"{"tracks":{"items":[{"id":"track-1","name":"Signal"}]}}"#,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artists, Vec::<String>::new());
        assert_eq!(results[0].duration_ms, None);
        assert_eq!(results[0].artwork_url, None);
        assert_eq!(results[0].canonical_url, None);
        assert_eq!(results[0].engagement_count, None);
    }

    #[tokio::test]
    async fn disabled_gate_performs_no_network_or_auth() {
        let store = MemoryCredentialStore::new();
        let transport = FakeTransport::new();
        let auth = service(false, &store, &transport);
        assert_eq!(auth.runtime_status(), ProviderRuntimeStatus::Disabled);
        assert_eq!(
            auth.search("signal", None, &SearchCancellation::new())
                .await,
            Err(SpotifyAuthError::Disabled)
        );
        assert!(transport.refresh_calls.lock().unwrap().is_empty());
        assert!(transport.search_calls.lock().unwrap().is_empty());
    }
}
