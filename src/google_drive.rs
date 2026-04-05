use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::blocking::{Client, multipart};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::secret_store;

const GOOGLE_DRIVE_FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const GOOGLE_DRIVE_UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";
const BACKUP_ROOT_FOLDER_NAME: &str = "Backup";
const BACKUP_ZEN_FOLDER_NAME: &str = "Zen";
const GOOGLE_DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";
#[derive(Debug, Clone)]
pub struct GoogleDriveSyncSettings {
    pub refresh_token: String,
    pub retention_months: u8,
}

#[derive(Debug, Clone)]
pub struct GoogleOauthClient {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone)]
pub struct GoogleDriveSyncSummary {
    pub pruned_local_files: usize,
    pub uploaded_files: usize,
    pub deleted_remote_files: usize,
}

#[derive(Debug, Clone)]
pub struct GoogleDriveSyncProgress {
    pub current_step: usize,
    pub total_steps: usize,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileListResponse {
    files: Vec<DriveFile>,
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    md5_checksum: Option<String>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    trashed: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CreateFileMetadata<'a> {
    name: &'a str,
    mime_type: &'a str,
    parents: Vec<&'a str>,
}

#[derive(Debug, Serialize)]
struct UpdateFileMetadata<'a> {
    name: &'a str,
}

#[derive(Debug)]
struct LocalFileState {
    name: String,
    path: PathBuf,
    size: u64,
}

pub fn clamp_retention_months(value: i32) -> u8 {
    value.clamp(1, 12) as u8
}

pub fn oauth_client_configured() -> bool {
    oauth_client().is_ok()
}

pub fn store_oauth_client(client_id: &str, client_secret: &str) -> Result<()> {
    secret_store::store_google_oauth_client(client_id, client_secret)
}

pub fn authorize_with_browser() -> Result<String> {
    let oauth_client = oauth_client()?;
    let client = Client::builder()
        .user_agent("zen-session-restore/0.5.4")
        .build()
        .context("failed to create Google OAuth client")?;

    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to bind local OAuth callback port")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure OAuth callback listener")?;
    let callback_port = listener
        .local_addr()
        .context("failed to read OAuth callback port")?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{callback_port}/callback");
    let state = random_url_safe(24);
    let verifier = random_url_safe(48);
    let challenge = pkce_challenge(&verifier);

    let auth_url = Url::parse_with_params(
        GOOGLE_AUTH_URL,
        &[
            ("client_id", oauth_client.client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", GOOGLE_DRIVE_SCOPE),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
        ],
    )
    .context("failed to build Google auth URL")?;

    open_browser(auth_url.as_str())?;

    let code = wait_for_oauth_code(listener, &state, Duration::from_secs(180))?;
    let token = client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("client_id", oauth_client.client_id.as_str()),
            ("client_secret", oauth_client.client_secret.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .context("failed to exchange Google auth code")?
        .error_for_status()
        .context("Google auth code exchange failed")?
        .json::<TokenResponse>()
        .context("failed to parse Google auth token response")?;

    token
        .refresh_token
        .ok_or_else(|| anyhow::anyhow!("Google did not return a refresh token"))
}

pub fn sync_backup_folder(
    local_backup_dir: &Path,
    settings: &GoogleDriveSyncSettings,
    mut on_progress: impl FnMut(GoogleDriveSyncProgress),
) -> Result<GoogleDriveSyncSummary> {
    validate_settings(settings)?;

    if !local_backup_dir.is_dir() {
        bail!(
            "backup folder {} does not exist",
            local_backup_dir.display()
        );
    }

    on_progress(GoogleDriveSyncProgress {
        current_step: 0,
        total_steps: 1,
        message: format!("Scanning {}", local_backup_dir.display()),
    });
    let pruned_local_files = prune_old_local_backups(local_backup_dir, settings.retention_months)?;
    on_progress(GoogleDriveSyncProgress {
        current_step: 1,
        total_steps: 5,
        message: format!(
            "Pruned expired local backups older than {} month(s)",
            settings.retention_months
        ),
    });
    let local_files = collect_local_files(local_backup_dir)?;
    let client = Client::builder()
        .user_agent("zen-session-restore/0.5.4")
        .build()
        .context("failed to create Google Drive client")?;
    on_progress(GoogleDriveSyncProgress {
        current_step: 2,
        total_steps: 5,
        message: "Refreshing Google Drive access".to_owned(),
    });
    let access_token = refresh_access_token(&client, settings)?;

    let backup_folder_id = ensure_folder(&client, &access_token, None, BACKUP_ROOT_FOLDER_NAME)?;
    let zen_folder_id = ensure_folder(
        &client,
        &access_token,
        Some(backup_folder_id.as_str()),
        BACKUP_ZEN_FOLDER_NAME,
    )?;

    on_progress(GoogleDriveSyncProgress {
        current_step: 3,
        total_steps: 5,
        message: "Comparing local backups with Google Drive".to_owned(),
    });
    let remote_files = list_child_files(&client, &access_token, &zen_folder_id)?;
    let remote_by_name = remote_files
        .into_iter()
        .filter(|file| {
            !file.trashed.unwrap_or(false)
                && file.mime_type.as_deref() != Some(GOOGLE_FOLDER_MIME_TYPE)
        })
        .map(|file| (file.name.clone(), file))
        .collect::<HashMap<_, _>>();

    let mut uploaded_files = 0usize;
    let local_file_count = local_files.len();
    let remote_delete_count = remote_by_name.len();
    let total_steps = 4 + local_file_count + remote_delete_count.max(1);
    let mut current_step = 4usize;
    for local_file in &local_files {
        on_progress(GoogleDriveSyncProgress {
            current_step,
            total_steps,
            message: format!("Checking {}", local_file.name),
        });
        let should_upload = match remote_by_name.get(&local_file.name) {
            Some(remote) => remote_needs_update(remote, local_file)?,
            None => true,
        };

        if should_upload {
            on_progress(GoogleDriveSyncProgress {
                current_step,
                total_steps,
                message: format!("Uploading {}", local_file.name),
            });
            upload_file(
                &client,
                &access_token,
                &zen_folder_id,
                local_file,
                remote_by_name
                    .get(&local_file.name)
                    .map(|file| file.id.as_str()),
            )?;
            uploaded_files += 1;
        }
        current_step += 1;
    }

    let local_names = local_files
        .iter()
        .map(|file| file.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut deleted_remote_files = 0usize;
    for remote in remote_by_name.values() {
        on_progress(GoogleDriveSyncProgress {
            current_step,
            total_steps,
            message: format!("Checking remote {}", remote.name),
        });
        if !local_names.contains(remote.name.as_str()) {
            on_progress(GoogleDriveSyncProgress {
                current_step,
                total_steps,
                message: format!("Removing stale remote {}", remote.name),
            });
            delete_file(&client, &access_token, &remote.id)?;
            deleted_remote_files += 1;
        }
        current_step += 1;
    }

    on_progress(GoogleDriveSyncProgress {
        current_step: total_steps,
        total_steps,
        message: "Finalizing sync".to_owned(),
    });

    Ok(GoogleDriveSyncSummary {
        pruned_local_files,
        uploaded_files,
        deleted_remote_files,
    })
}

fn validate_settings(settings: &GoogleDriveSyncSettings) -> Result<()> {
    if settings.refresh_token.trim().is_empty() {
        bail!("Google refresh token is required");
    }
    oauth_client()?;
    Ok(())
}

fn refresh_access_token(client: &Client, settings: &GoogleDriveSyncSettings) -> Result<String> {
    let oauth_client = oauth_client()?;
    let response = client
        .post(GOOGLE_TOKEN_URL)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", oauth_client.client_id.as_str()),
            ("client_secret", oauth_client.client_secret.as_str()),
            ("refresh_token", settings.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .context("failed to reach Google OAuth token endpoint")?
        .error_for_status()
        .context("Google OAuth token refresh failed")?;

    Ok(response
        .json::<TokenResponse>()
        .context("failed to parse Google OAuth token response")?
        .access_token)
}

fn ensure_folder(
    client: &Client,
    access_token: &str,
    parent_id: Option<&str>,
    folder_name: &str,
) -> Result<String> {
    let query = match parent_id {
        Some(parent_id) => format!(
            "name = '{}' and mimeType = '{}' and '{}' in parents and trashed = false",
            escape_drive_query(folder_name),
            GOOGLE_FOLDER_MIME_TYPE,
            parent_id
        ),
        None => format!(
            "name = '{}' and mimeType = '{}' and 'root' in parents and trashed = false",
            escape_drive_query(folder_name),
            GOOGLE_FOLDER_MIME_TYPE
        ),
    };

    let response = client
        .get(GOOGLE_DRIVE_FILES_URL)
        .header(AUTHORIZATION, bearer(access_token))
        .query(&[
            ("q", query.as_str()),
            ("fields", "files(id,name,mimeType,trashed)"),
            ("spaces", "drive"),
            ("pageSize", "10"),
        ])
        .send()
        .context("failed to query Google Drive folders")?;
    let response = ensure_success(response, "Google Drive folder query failed")?;

    let existing = response
        .json::<FileListResponse>()
        .context("failed to parse Google Drive folder query")?;
    if let Some(folder) = existing.files.into_iter().next() {
        return Ok(folder.id);
    }

    let metadata = CreateFileMetadata {
        name: folder_name,
        mime_type: GOOGLE_FOLDER_MIME_TYPE,
        parents: parent_id.into_iter().collect(),
    };

    let response = client
        .post(GOOGLE_DRIVE_FILES_URL)
        .header(AUTHORIZATION, bearer(access_token))
        .json(&metadata)
        .send()
        .context("failed to create Google Drive folder")?;
    let response = ensure_success(response, "Google Drive folder creation failed")?;

    Ok(response
        .json::<DriveFile>()
        .context("failed to parse created Google Drive folder")?
        .id)
}

fn list_child_files(
    client: &Client,
    access_token: &str,
    parent_id: &str,
) -> Result<Vec<DriveFile>> {
    let query = format!("'{}' in parents and trashed = false", parent_id);
    let response = client
        .get(GOOGLE_DRIVE_FILES_URL)
        .header(AUTHORIZATION, bearer(access_token))
        .query(&[
            ("q", query.as_str()),
            ("fields", "files(id,name,mimeType,md5Checksum,size,trashed)"),
            ("spaces", "drive"),
            ("pageSize", "1000"),
        ])
        .send()
        .context("failed to list Google Drive folder contents")?;
    let response = ensure_success(response, "Google Drive folder listing failed")?;

    Ok(response
        .json::<FileListResponse>()
        .context("failed to parse Google Drive folder listing")?
        .files)
}

fn remote_needs_update(remote: &DriveFile, local: &LocalFileState) -> Result<bool> {
    let remote_size = remote
        .size
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .with_context(|| format!("invalid Google Drive size for {}", remote.name))?;

    if remote_size != local.size {
        return Ok(true);
    }

    let local_md5 = md5::compute(
        fs::read(&local.path)
            .with_context(|| format!("failed to read {}", local.path.display()))?,
    );
    let local_md5_hex = format!("{:x}", local_md5);
    Ok(remote.md5_checksum.as_deref() != Some(local_md5_hex.as_str()))
}

fn upload_file(
    client: &Client,
    access_token: &str,
    folder_id: &str,
    local: &LocalFileState,
    existing_file_id: Option<&str>,
) -> Result<()> {
    let metadata_body = if existing_file_id.is_some() {
        serde_json::to_string(&UpdateFileMetadata { name: &local.name })?
    } else {
        serde_json::to_string(&serde_json::json!({
            "name": local.name,
            "parents": [folder_id],
        }))?
    };
    let file_bytes = fs::read(&local.path)
        .with_context(|| format!("failed to read {}", local.path.display()))?;

    let form = multipart::Form::new()
        .part(
            "metadata",
            multipart::Part::text(metadata_body).mime_str("application/json; charset=UTF-8")?,
        )
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name(local.name.clone())
                .mime_str("application/octet-stream")?,
        );

    let request = match existing_file_id {
        Some(file_id) => client.patch(format!(
            "{GOOGLE_DRIVE_UPLOAD_URL}/{file_id}?uploadType=multipart&fields=id"
        )),
        None => client.post(format!(
            "{GOOGLE_DRIVE_UPLOAD_URL}?uploadType=multipart&fields=id"
        )),
    };

    let response = request
        .header(AUTHORIZATION, bearer(access_token))
        .multipart(form)
        .send()
        .with_context(|| format!("failed to upload {}", local.path.display()))?;
    let _response = ensure_success(response, &format!("Google Drive rejected {}", local.name))?;

    Ok(())
}

fn delete_file(client: &Client, access_token: &str, file_id: &str) -> Result<()> {
    let response = client
        .delete(format!("{GOOGLE_DRIVE_FILES_URL}/{file_id}"))
        .header(AUTHORIZATION, bearer(access_token))
        .send()
        .context("failed to delete stale Google Drive file")?;
    let _response = ensure_success(response, "Google Drive delete failed")?;
    Ok(())
}

fn collect_local_files(local_backup_dir: &Path) -> Result<Vec<LocalFileState>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(local_backup_dir)
        .with_context(|| format!("failed to read {}", local_backup_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata for {}", path.display()))?;
        files.push(LocalFileState {
            name: name.to_owned(),
            path,
            size: metadata.len(),
        });
    }

    files.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(files)
}

fn prune_old_local_backups(local_backup_dir: &Path, retention_months: u8) -> Result<usize> {
    let retention = Duration::from_secs(60 * 60 * 24 * 30 * u64::from(retention_months));
    let cutoff = SystemTime::now()
        .checked_sub(retention)
        .ok_or_else(|| anyhow::anyhow!("retention window underflowed"))?;

    let mut removed = 0usize;
    for entry in fs::read_dir(local_backup_dir)
        .with_context(|| format!("failed to read {}", local_backup_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let modified = entry
            .metadata()
            .with_context(|| format!("failed to read metadata for {}", path.display()))?
            .modified()
            .with_context(|| format!("failed to read modification time for {}", path.display()))?;
        if modified < cutoff {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove expired backup {}", path.display()))?;
            removed += 1;
        }
    }

    Ok(removed)
}

fn escape_drive_query(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn bearer(access_token: &str) -> String {
    format!("Bearer {access_token}")
}

fn ensure_success(
    response: reqwest::blocking::Response,
    context_message: &str,
) -> Result<reqwest::blocking::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response
        .text()
        .unwrap_or_else(|_| "<failed to read error body>".to_owned());
    bail!("{context_message}: HTTP {} {}", status.as_u16(), body)
}

fn oauth_client() -> Result<GoogleOauthClient> {
    if let Some(config) = secret_store::load_google_oauth_client()? {
        if config.client_id.trim().is_empty() || config.client_secret.trim().is_empty() {
            bail!("stored Google OAuth credentials are empty");
        }

        return Ok(GoogleOauthClient {
            client_id: config.client_id,
            client_secret: config.client_secret,
        });
    }
    bail!("Google OAuth credentials are not stored in the desktop keyring yet")
}

fn random_url_safe(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn open_browser(url: &str) -> Result<()> {
    let commands = if cfg!(target_os = "macos") {
        vec![("open", vec![url])]
    } else {
        vec![("xdg-open", vec![url]), ("gio", vec!["open", url])]
    };

    for (program, args) in commands {
        if Command::new(program).args(args).spawn().is_ok() {
            return Ok(());
        }
    }

    bail!("failed to open a browser automatically")
}

fn wait_for_oauth_code(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String> {
    let deadline = SystemTime::now()
        .checked_add(timeout)
        .ok_or_else(|| anyhow::anyhow!("OAuth timeout overflowed"))?;

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buffer = [0u8; 4096];
                let bytes_read = stream
                    .read(&mut buffer)
                    .context("failed to read OAuth callback")?;
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                let request_line = request.lines().next().unwrap_or_default();
                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .ok_or_else(|| anyhow::anyhow!("invalid OAuth callback request"))?;
                let callback_url = Url::parse(&format!("http://localhost{path}"))
                    .context("failed to parse OAuth callback URL")?;
                let params = callback_url
                    .query_pairs()
                    .into_owned()
                    .collect::<HashMap<String, String>>();

                let response = if let Some(error) = params.get("error") {
                    format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<html><body>Google sign-in failed: {error}</body></html>"
                    )
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body>You can close this window and return to Restore Zen Session.</body></html>".to_owned()
                };
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();

                if let Some(error) = params.get("error") {
                    bail!("Google sign-in failed: {error}");
                }

                let state = params
                    .get("state")
                    .ok_or_else(|| anyhow::anyhow!("Google callback was missing state"))?;
                if state != expected_state {
                    bail!("Google callback state did not match");
                }

                let code = params
                    .get("code")
                    .ok_or_else(|| anyhow::anyhow!("Google callback was missing code"))?;
                return Ok(code.clone());
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if SystemTime::now() >= deadline {
                    bail!("timed out waiting for Google sign-in");
                }
                thread::sleep(Duration::from_millis(150));
            }
            Err(error) => return Err(error).context("failed while waiting for OAuth callback"),
        }
    }
}
