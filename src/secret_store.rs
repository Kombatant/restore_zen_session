use std::{
    env,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};

use crate::kwallet::{self, StoredGoogleOauthClient};

const SECRET_TOOL_BINARY: &str = "secret-tool";
const SECRET_LABEL: &str = "Restore Zen Session Google OAuth";
const SECRET_ATTR_APP: &str = "app";
const SECRET_ATTR_ENTRY: &str = "entry";
const SECRET_ATTR_VALUE_APP: &str = "restore-zen-session";
const SECRET_ATTR_VALUE_ENTRY_GOOGLE_OAUTH: &str = "google_oauth_client";

pub fn load_google_oauth_client() -> Result<Option<StoredGoogleOauthClient>> {
    let backend = preferred_backend();
    match backend.load_google_oauth_client() {
        Ok(client) => Ok(client),
        Err(primary_error) => match fallback_backend(backend) {
            Some(fallback) => fallback
                .load_google_oauth_client()
                .map_err(|fallback_error| combined_error(primary_error, fallback_error)),
            None => Err(primary_error),
        },
    }
}

pub fn store_google_oauth_client(client_id: &str, client_secret: &str) -> Result<()> {
    let backend = preferred_backend();
    match backend.store_google_oauth_client(client_id, client_secret) {
        Ok(()) => Ok(()),
        Err(primary_error) => match fallback_backend(backend) {
            Some(fallback) => fallback
                .store_google_oauth_client(client_id, client_secret)
                .map_err(|fallback_error| combined_error(primary_error, fallback_error)),
            None => Err(primary_error),
        },
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

fn fallback_backend(backend: CredentialBackend) -> Option<CredentialBackend> {
    match backend {
        CredentialBackend::SecretService => Some(CredentialBackend::KWallet),
        CredentialBackend::KWallet => Some(CredentialBackend::SecretService),
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
        let output = Command::new(SECRET_TOOL_BINARY)
            .arg("lookup")
            .arg(SECRET_ATTR_APP)
            .arg(SECRET_ATTR_VALUE_APP)
            .arg(SECRET_ATTR_ENTRY)
            .arg(SECRET_ATTR_VALUE_ENTRY_GOOGLE_OAUTH)
            .output()
            .with_context(|| format!("failed to start {SECRET_TOOL_BINARY}"))?;

        if output.status.success() {
            let payload = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let config = serde_json::from_str::<StoredGoogleOauthClient>(&payload)
                .context("failed to parse Google OAuth credentials from Secret Service")?;
            return Ok(Some(config));
        }

        if output.status.code() == Some(1) {
            return Ok(None);
        }

        bail!(
            "Secret Service lookup failed: {}",
            command_error_message(&output)
        );
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

        let mut child = Command::new(SECRET_TOOL_BINARY)
            .arg("store")
            .arg(format!("--label={SECRET_LABEL}"))
            .arg(SECRET_ATTR_APP)
            .arg(SECRET_ATTR_VALUE_APP)
            .arg(SECRET_ATTR_ENTRY)
            .arg(SECRET_ATTR_VALUE_ENTRY_GOOGLE_OAUTH)
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
