use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const QDBUS_BINARY: &str = "qdbus";
const WALLET_FOLDER: &str = "Restore Zen Session";
const WALLET_ENTRY_GOOGLE_OAUTH: &str = "google_oauth_client";
const APP_ID: &str = "restore-zen-session";
const DBUS_INTERFACE: &str = "org.kde.KWallet";
const DBUS_CANDIDATES: [(&str, &str); 2] = [
    ("org.kde.kwalletd6", "/modules/kwalletd6"),
    ("org.kde.kwalletd5", "/modules/kwalletd5"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredGoogleOauthClient {
    #[serde(rename = "google_client_id")]
    pub client_id: String,
    #[serde(rename = "google_client_secret")]
    pub client_secret: String,
}

pub fn load_google_oauth_client() -> Result<Option<StoredGoogleOauthClient>> {
    let session = KWalletSession::connect()?;
    if !session.folder_exists(WALLET_FOLDER)? {
        return Ok(None);
    }
    if !session.entry_exists(WALLET_FOLDER, WALLET_ENTRY_GOOGLE_OAUTH)? {
        return Ok(None);
    }

    let payload = session.read_password(WALLET_FOLDER, WALLET_ENTRY_GOOGLE_OAUTH)?;
    let config = serde_json::from_str::<StoredGoogleOauthClient>(&payload)
        .context("failed to parse Google OAuth credentials from KDE Wallet")?;
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
    .context("failed to serialize Google OAuth credentials for KDE Wallet")?;

    let session = KWalletSession::connect()?;
    session.ensure_folder(WALLET_FOLDER)?;
    session.write_password(WALLET_FOLDER, WALLET_ENTRY_GOOGLE_OAUTH, &payload)?;
    Ok(())
}

struct KWalletSession {
    service: &'static str,
    path: &'static str,
    wallet_name: String,
    handle: i32,
}

impl KWalletSession {
    fn connect() -> Result<Self> {
        for (service, path) in DBUS_CANDIDATES {
            let enabled_output = match run_qdbus(service, path, "isEnabled", &[]) {
                Ok(output) => output,
                Err(_) => continue,
            };
            if !parse_bool(&enabled_output)? {
                bail!("KDE Wallet is disabled");
            }

            let wallet_name = resolve_wallet_name(service, path)?;
            let handle_output =
                run_qdbus(service, path, "open", &[wallet_name.as_str(), "0", APP_ID])?;
            let handle = parse_i32(&handle_output)?;
            if handle < 0 {
                bail!("KDE Wallet could not be opened");
            }

            return Ok(Self {
                service,
                path,
                wallet_name,
                handle,
            });
        }

        bail!("KDE Wallet service is not available")
    }

    fn folder_exists(&self, folder: &str) -> Result<bool> {
        let handle = self.handle.to_string();
        let output = run_qdbus(
            self.service,
            self.path,
            "hasFolder",
            &[&handle, folder, APP_ID],
        )?;
        parse_bool(&output)
    }

    fn entry_exists(&self, folder: &str, entry: &str) -> Result<bool> {
        let handle = self.handle.to_string();
        let output = run_qdbus(
            self.service,
            self.path,
            "hasEntry",
            &[&handle, folder, entry, APP_ID],
        )?;
        parse_bool(&output)
    }

    fn ensure_folder(&self, folder: &str) -> Result<()> {
        if self.folder_exists(folder)? {
            return Ok(());
        }

        let handle = self.handle.to_string();
        let output = run_qdbus(
            self.service,
            self.path,
            "createFolder",
            &[&handle, folder, APP_ID],
        )?;
        if !parse_bool(&output)? {
            bail!(
                "KDE Wallet did not create folder '{}' in '{}'",
                folder,
                self.wallet_name
            );
        }

        Ok(())
    }

    fn read_password(&self, folder: &str, entry: &str) -> Result<String> {
        let handle = self.handle.to_string();
        run_qdbus(
            self.service,
            self.path,
            "readPassword",
            &[&handle, folder, entry, APP_ID],
        )
    }

    fn write_password(&self, folder: &str, entry: &str, value: &str) -> Result<()> {
        let handle = self.handle.to_string();
        let output = run_qdbus(
            self.service,
            self.path,
            "writePassword",
            &[&handle, folder, entry, value, APP_ID],
        )?;
        let status = parse_i32(&output)?;
        if status != 0 {
            bail!(
                "KDE Wallet rejected saving '{}' in folder '{}' (status {})",
                entry,
                folder,
                status
            );
        }

        Ok(())
    }
}

fn resolve_wallet_name(service: &'static str, path: &'static str) -> Result<String> {
    for method in ["localWallet", "networkWallet"] {
        let name = run_qdbus(service, path, method, &[])?;
        if !name.trim().is_empty() {
            return Ok(name);
        }
    }

    bail!("KDE Wallet did not report a wallet name")
}

fn run_qdbus(service: &str, path: &str, method: &str, args: &[&str]) -> Result<String> {
    let method_name = format!("{DBUS_INTERFACE}.{method}");
    let output = Command::new(QDBUS_BINARY)
        .arg(service)
        .arg(path)
        .arg(method_name)
        .args(args)
        .output()
        .with_context(|| format!("failed to start {QDBUS_BINARY}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let message = if stderr.is_empty() { stdout } else { stderr };
        bail!(
            "KDE Wallet D-Bus call {} failed: {}",
            method,
            if message.is_empty() {
                "unknown error".to_owned()
            } else {
                message
            }
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("unexpected KDE Wallet boolean response: {}", other),
    }
}

fn parse_i32(value: &str) -> Result<i32> {
    value
        .trim()
        .parse::<i32>()
        .with_context(|| format!("unexpected KDE Wallet numeric response: {}", value.trim()))
}
