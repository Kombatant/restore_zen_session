use std::{
    env,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};

use crate::kwallet::{self, StoredGoogleOauthClient};

const SECRET_TOOL_BINARY: &str = "secret-tool";
const SECRET_LABEL: &str = "Restore Zen Session Google OAuth";
const SECRET_LABEL_REFRESH_TOKEN: &str = "Restore Zen Session Google Drive Token";
const SECRET_ATTR_APP: &str = "app";
const SECRET_ATTR_ENTRY: &str = "entry";
const SECRET_ATTR_VALUE_APP: &str = "restore-zen-session";
const SECRET_ATTR_VALUE_ENTRY_GOOGLE_OAUTH: &str = "google_oauth_client";
const SECRET_ATTR_VALUE_ENTRY_GOOGLE_REFRESH_TOKEN: &str = "google_refresh_token";

pub fn load_google_oauth_client() -> Result<Option<StoredGoogleOauthClient>> {
    with_backend_fallback(|backend| backend.load_google_oauth_client())
}

pub fn store_google_oauth_client(client_id: &str, client_secret: &str) -> Result<()> {
    with_backend_fallback(|backend| backend.store_google_oauth_client(client_id, client_secret))
}

pub fn load_google_refresh_token() -> Result<Option<String>> {
    with_backend_fallback(|backend| backend.load_google_refresh_token())
}

pub fn store_google_refresh_token(refresh_token: &str) -> Result<()> {
    with_backend_fallback(|backend| backend.store_google_refresh_token(refresh_token))
}

/// Removes the refresh token from both backends; the token may live in either
/// one if the desktop environment changed since it was stored.
pub fn delete_google_refresh_token() -> Result<()> {
    let results = [
        CredentialBackend::SecretService.delete_google_refresh_token(),
        CredentialBackend::KWallet.delete_google_refresh_token(),
    ];

    let mut errors = results.into_iter().filter_map(Result::err);
    match (errors.next(), errors.next()) {
        (Some(primary_error), Some(fallback_error)) => {
            Err(combined_error(primary_error, fallback_error))
        }
        _ => Ok(()),
    }
}

fn with_backend_fallback<T>(operation: impl Fn(CredentialBackend) -> Result<T>) -> Result<T> {
    let backend = preferred_backend();
    match operation(backend) {
        Ok(value) => Ok(value),
        Err(primary_error) => operation(fallback_backend(backend))
            .map_err(|fallback_error| combined_error(primary_error, fallback_error)),
    }
}

#[derive(Clone, Copy)]
enum CredentialBackend {
    SecretService,
    KWallet,
}

impl CredentialBackend {
    fn load_google_oauth_client(self) -> Result<Option<StoredGoogleOauthClient>> {
        match self {
            Self::SecretService => secret_service::load_google_oauth_client(),
            Self::KWallet => kwallet::load_google_oauth_client(),
        }
    }

    fn store_google_oauth_client(self, client_id: &str, client_secret: &str) -> Result<()> {
        match self {
            Self::SecretService => {
                secret_service::store_google_oauth_client(client_id, client_secret)
            }
            Self::KWallet => kwallet::store_google_oauth_client(client_id, client_secret),
        }
    }

    fn load_google_refresh_token(self) -> Result<Option<String>> {
        match self {
            Self::SecretService => secret_service::load_google_refresh_token(),
            Self::KWallet => kwallet::load_google_refresh_token(),
        }
    }

    fn store_google_refresh_token(self, refresh_token: &str) -> Result<()> {
        match self {
            Self::SecretService => secret_service::store_google_refresh_token(refresh_token),
            Self::KWallet => kwallet::store_google_refresh_token(refresh_token),
        }
    }

    fn delete_google_refresh_token(self) -> Result<()> {
        match self {
            Self::SecretService => secret_service::delete_google_refresh_token(),
            Self::KWallet => kwallet::delete_google_refresh_token(),
        }
    }
}

fn preferred_backend() -> CredentialBackend {
    match desktop_environment() {
        Some(desktop) if desktop.contains("kde") => CredentialBackend::KWallet,
        Some(desktop)
            if desktop.contains("gnome")
                || desktop.contains("unity")
                || desktop.contains("cinnamon")
                || desktop.contains("pantheon") =>
        {
            CredentialBackend::SecretService
        }
        _ => CredentialBackend::SecretService,
    }
}

fn fallback_backend(backend: CredentialBackend) -> CredentialBackend {
    match backend {
        CredentialBackend::SecretService => CredentialBackend::KWallet,
        CredentialBackend::KWallet => CredentialBackend::SecretService,
    }
}

fn desktop_environment() -> Option<String> {
    [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ]
    .into_iter()
    .filter_map(|key| env::var(key).ok())
    .map(|value| value.to_ascii_lowercase())
    .find(|value| !value.trim().is_empty())
}

fn combined_error(primary_error: anyhow::Error, fallback_error: anyhow::Error) -> anyhow::Error {
    anyhow!(
        "failed to access desktop credential storage; primary backend error: {primary_error}; fallback backend error: {fallback_error}"
    )
}

mod secret_service {
    use super::*;

    pub fn load_google_oauth_client() -> Result<Option<StoredGoogleOauthClient>> {
        let Some(payload) = lookup_entry(SECRET_ATTR_VALUE_ENTRY_GOOGLE_OAUTH)? else {
            return Ok(None);
        };
        let config = serde_json::from_str::<StoredGoogleOauthClient>(&payload)
            .context("failed to parse Google OAuth credentials from Secret Service")?;
        Ok(Some(config))
    }

    pub fn store_google_oauth_client(client_id: &str, client_secret: &str) -> Result<()> {
        let client_id = client_id.trim();
        let client_secret = client_secret.trim();
        if client_id.is_empty() || client_secret.is_empty() {
            bail!("Google Client ID and Google Client Secret are required");
        }

        let payload = serde_json::to_string(&StoredGoogleOauthClient {
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
        })
        .context("failed to serialize Google OAuth credentials for Secret Service")?;

        store_entry(SECRET_ATTR_VALUE_ENTRY_GOOGLE_OAUTH, SECRET_LABEL, &payload)
    }

    pub fn load_google_refresh_token() -> Result<Option<String>> {
        Ok(lookup_entry(SECRET_ATTR_VALUE_ENTRY_GOOGLE_REFRESH_TOKEN)?
            .filter(|token| !token.trim().is_empty()))
    }

    pub fn store_google_refresh_token(refresh_token: &str) -> Result<()> {
        let refresh_token = refresh_token.trim();
        if refresh_token.is_empty() {
            bail!("Google refresh token is required");
        }

        store_entry(
            SECRET_ATTR_VALUE_ENTRY_GOOGLE_REFRESH_TOKEN,
            SECRET_LABEL_REFRESH_TOKEN,
            refresh_token,
        )
    }

    pub fn delete_google_refresh_token() -> Result<()> {
        let output = Command::new(SECRET_TOOL_BINARY)
            .arg("clear")
            .arg(SECRET_ATTR_APP)
            .arg(SECRET_ATTR_VALUE_APP)
            .arg(SECRET_ATTR_ENTRY)
            .arg(SECRET_ATTR_VALUE_ENTRY_GOOGLE_REFRESH_TOKEN)
            .output()
            .with_context(|| format!("failed to start {SECRET_TOOL_BINARY}"))?;

        // Exit code 1 means no matching secret existed, which is fine.
        if output.status.success() || output.status.code() == Some(1) {
            return Ok(());
        }

        bail!(
            "Secret Service clear failed: {}",
            command_error_message(&output)
        );
    }

    fn lookup_entry(entry_value: &str) -> Result<Option<String>> {
        let output = Command::new(SECRET_TOOL_BINARY)
            .arg("lookup")
            .arg(SECRET_ATTR_APP)
            .arg(SECRET_ATTR_VALUE_APP)
            .arg(SECRET_ATTR_ENTRY)
            .arg(entry_value)
            .output()
            .with_context(|| format!("failed to start {SECRET_TOOL_BINARY}"))?;

        if output.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            ));
        }

        if output.status.code() == Some(1) {
            return Ok(None);
        }

        bail!(
            "Secret Service lookup failed: {}",
            command_error_message(&output)
        );
    }

    fn store_entry(entry_value: &str, label: &str, payload: &str) -> Result<()> {
        let mut child = Command::new(SECRET_TOOL_BINARY)
            .arg("store")
            .arg(format!("--label={label}"))
            .arg(SECRET_ATTR_APP)
            .arg(SECRET_ATTR_VALUE_APP)
            .arg(SECRET_ATTR_ENTRY)
            .arg(entry_value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to start {SECRET_TOOL_BINARY}"))?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| anyhow!("failed to open stdin for {SECRET_TOOL_BINARY}"))?;
            stdin
                .write_all(payload.as_bytes())
                .with_context(|| format!("failed to write secret to {SECRET_TOOL_BINARY}"))?;
        }

        let output = child
            .wait_with_output()
            .with_context(|| format!("failed to wait for {SECRET_TOOL_BINARY}"))?;
        if !output.status.success() {
            bail!(
                "Secret Service store failed: {}",
                command_error_message(&output)
            );
        }

        Ok(())
    }

    fn command_error_message(output: &std::process::Output) -> String {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "unknown error".to_owned()
        }
    }
}
