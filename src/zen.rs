use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::mozlz4;

#[derive(Debug, Clone)]
pub struct BackupFile {
    pub path: PathBuf,
    pub file_name: String,
    pub snapshot_label: String,
    pub sort_key: (i32, i32, i32, i32),
    pub summary: SessionSummary,
    pub raw_json: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub kind: SessionKind,
    pub saved_at_ms: Option<i64>,
    pub collections: Vec<CollectionSummary>,
    pub total_tabs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    ZenSpaces,
    FirefoxWindows,
}

impl fmt::Display for SessionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionKind::ZenSpaces => write!(f, "spaces"),
            SessionKind::FirefoxWindows => write!(f, "windows"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionSummary {
    pub title: String,
    pub workspace_id: Option<String>,
    pub tabs: Vec<TabSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TabSummary {
    pub title: String,
    pub url: Option<String>,
    pub pinned: bool,
    pub essential: bool,
}

#[derive(Debug, Clone)]
pub struct CollectionSelection {
    pub collection_index: usize,
    pub selected_tab_indices: BTreeSet<usize>,
}

pub fn detect_default_profile() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not determine home directory"))?;
    let zen_root = home.join(".zen");
    let entries = fs::read_dir(&zen_root)
        .with_context(|| format!("failed to read {}", zen_root.display()))?;

    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("zen-sessions-backup").is_dir() {
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok();
            candidates.push((modified, path));
        }
    }

    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    candidates
        .into_iter()
        .map(|(_, path)| path)
        .next()
        .ok_or_else(|| anyhow!("could not find a Zen profile under {}", zen_root.display()))
}

pub fn profile_backup_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("zen-sessions-backup")
}

pub fn main_session_file(profile_dir: &Path) -> PathBuf {
    profile_dir.join("zen-sessions.jsonlz4")
}

pub fn is_zen_running() -> bool {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system.processes().values().any(process_looks_like_zen)
}

pub fn is_profile_in_use(profile_dir: &Path) -> bool {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);

    active_profile_lock_pid(profile_dir)
        .and_then(|pid| system.process(Pid::from_u32(pid)).filter(|process| process_looks_like_zen(process)))
        .is_some()
}

pub fn launch_zen(executable: Option<&Path>) -> Result<()> {
    let program = executable
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("zen"));

    Command::new(&program)
        .spawn()
        .with_context(|| format!("failed to launch {}", program.display()))?;
    Ok(())
}

fn process_looks_like_zen(process: &sysinfo::Process) -> bool {
    let name = process.name().to_string_lossy().to_ascii_lowercase();
    let cmdline = process
        .cmd()
        .iter()
        .map(|part| part.to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    name == "zen"
        || name.contains("zen-browser")
        || cmdline.contains("/zen")
        || cmdline.contains("zen-bin")
        || cmdline.contains("zen-browser")
}

fn active_profile_lock_pid(profile_dir: &Path) -> Option<u32> {
    let lock_target = fs::read_link(profile_dir.join("lock")).ok()?;
    let lock_target = lock_target.to_string_lossy();
    let (_, pid_part) = lock_target.rsplit_once(":+")?;
    pid_part.parse::<u32>().ok()
}

pub fn scan_backups(profile_dir: &Path) -> Result<Vec<BackupFile>> {
    let backup_dir = profile_backup_dir(profile_dir);
    let entries = fs::read_dir(&backup_dir)
        .with_context(|| format!("failed to read backup directory {}", backup_dir.display()))?;

    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let file_name = file_name.to_owned();
        if !(file_name.ends_with(".jsonlz4")
            || file_name.ends_with(".baklz4")
            || file_name.ends_with(".json"))
        {
            continue;
        }

        let (raw_json, summary) = parse_backup_json_and_summary(&path)
            .with_context(|| format!("failed to parse backup file {}", path.display()))?;
        let sort_key = parse_snapshot_key(&file_name);
        let snapshot_label = snapshot_label(&file_name);

        backups.push(BackupFile {
            path,
            file_name,
            snapshot_label,
            sort_key,
            summary,
            raw_json,
        });
    }

    backups.sort_by(|left, right| {
        right
            .sort_key
            .cmp(&left.sort_key)
            .then_with(|| right.file_name.cmp(&left.file_name))
    });

    Ok(backups)
}

pub fn resolve_backup(profile_dir: &Path, selector: &str) -> Result<BackupFile> {
    let backups = scan_backups(profile_dir)?;
    if backups.is_empty() {
        bail!(
            "no backup files were found in {}",
            profile_backup_dir(profile_dir).display()
        );
    }

    if let Ok(index) = selector.parse::<usize>() {
        let Some(backup) = backups.get(index.saturating_sub(1)) else {
            bail!("backup index {} is out of range", index);
        };
        return Ok(backup.clone());
    }

    let candidate_path = PathBuf::from(selector);
    if candidate_path.exists() {
        return parse_path_as_backup(candidate_path);
    }

    backups
        .into_iter()
        .find(|backup| backup.file_name == selector)
        .ok_or_else(|| anyhow!("could not find backup matching '{}'", selector))
}

pub fn copy_backup_to_main_session(backup: &BackupFile, profile_dir: &Path) -> Result<PathBuf> {
    let destination = main_session_file(profile_dir);
    fs::copy(&backup.path, &destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            backup.path.display(),
            destination.display()
        )
    })?;
    Ok(destination)
}

pub fn write_filtered_restore(
    backup: &BackupFile,
    profile_dir: &Path,
    selections: &[CollectionSelection],
) -> Result<PathBuf> {
    let filtered_json = build_filtered_restore_json(backup, selections)?;
    let encoded = write_session_value(&filtered_json)?;
    let destination = main_session_file(profile_dir);
    fs::write(&destination, encoded)
        .with_context(|| format!("failed to write {}", destination.display()))?;
    Ok(destination)
}

fn parse_path_as_backup(path: PathBuf) -> Result<BackupFile> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("backup path {} has no valid filename", path.display()))?
        .to_owned();
    let (raw_json, summary) = parse_backup_json_and_summary(&path)?;

    Ok(BackupFile {
        snapshot_label: snapshot_label(&file_name),
        sort_key: parse_snapshot_key(&file_name),
        file_name,
        path,
        summary,
        raw_json,
    })
}

fn parse_backup_json_and_summary(path: &Path) -> Result<(Value, SessionSummary)> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let decoded = if path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.ends_with(".json"))
    {
        bytes
    } else {
        mozlz4::decode(&bytes)?
    };
    let json: Value = serde_json::from_slice(&decoded)
        .with_context(|| format!("failed to parse JSON from {}", path.display()))?;
    let summary = summarize_session(&json)?;
    Ok((json, summary))
}

fn write_session_value(value: &Value) -> Result<Vec<u8>> {
    let json_bytes =
        serde_json::to_vec(value).context("failed to serialize filtered session JSON")?;
    Ok(mozlz4::encode(&json_bytes))
}

fn build_filtered_restore_json(
    backup: &BackupFile,
    selections: &[CollectionSelection],
) -> Result<Value> {
    match backup.summary.kind {
        SessionKind::ZenSpaces => filter_zen_session(backup, selections),
        SessionKind::FirefoxWindows => filter_firefox_session(backup, selections),
    }
}

fn filter_zen_session(backup: &BackupFile, selections: &[CollectionSelection]) -> Result<Value> {
    let mut json = backup.raw_json.clone();
    let object = json
        .as_object_mut()
        .ok_or_else(|| anyhow!("Zen session root is not an object"))?;
    let original_spaces = object
        .remove("spaces")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| anyhow!("Zen session is missing spaces"))?;
    let original_tabs = object
        .remove("tabs")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| anyhow!("Zen session is missing tabs"))?;

    let mut selected_by_workspace: HashMap<Option<String>, BTreeSet<usize>> = HashMap::new();
    for selection in selections {
        let collection = backup
            .summary
            .collections
            .get(selection.collection_index)
            .ok_or_else(|| anyhow!("collection index {} is out of range", selection.collection_index))?;
        selected_by_workspace.insert(
            collection.workspace_id.clone(),
            selection.selected_tab_indices.clone(),
        );
    }

    let selected_workspace_ids: HashSet<String> = selected_by_workspace
        .keys()
        .filter_map(|key| key.clone())
        .collect();
    let filtered_spaces = original_spaces
        .into_iter()
        .filter(|space| {
            space.get("uuid")
                .and_then(Value::as_str)
                .is_some_and(|uuid| selected_workspace_ids.contains(uuid))
        })
        .collect::<Vec<_>>();

    let mut filtered_tabs = Vec::new();
    let mut counters: HashMap<Option<String>, usize> = HashMap::new();

    for tab in original_tabs {
        let workspace_id = tab
            .get("zenWorkspace")
            .and_then(Value::as_str)
            .map(str::to_owned);

        let counter = counters.entry(workspace_id.clone()).or_insert(0);
        let keep = selected_by_workspace
            .get(&workspace_id)
            .is_some_and(|indices| indices.contains(counter));
        *counter += 1;

        if keep {
            filtered_tabs.push(tab);
        }
    }

    object.insert("spaces".to_owned(), Value::Array(filtered_spaces));
    object.insert("tabs".to_owned(), Value::Array(filtered_tabs));

    Ok(json)
}

fn filter_firefox_session(backup: &BackupFile, selections: &[CollectionSelection]) -> Result<Value> {
    let mut json = backup.raw_json.clone();
    let windows = json
        .get_mut("windows")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("Firefox session is missing windows"))?;

    let mut selected_tabs_by_window: HashMap<usize, BTreeSet<usize>> = HashMap::new();
    for selection in selections {
        selected_tabs_by_window.insert(selection.collection_index, selection.selected_tab_indices.clone());
    }

    let original_windows = windows.clone();
    windows.clear();
    for (window_index, mut window) in original_windows.into_iter().enumerate() {
        let Some(selected_indices) = selected_tabs_by_window.get(&window_index) else {
            continue;
        };

        let tabs = window
            .get_mut("tabs")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| anyhow!("window {} is missing tabs", window_index + 1))?;

        let original_tabs = tabs.clone();
        tabs.clear();
        for (tab_index, tab) in original_tabs.into_iter().enumerate() {
            if selected_indices.contains(&tab_index) {
                tabs.push(tab);
            }
        }

        if !tabs.is_empty() {
            windows.push(window);
        }
    }

    Ok(json)
}

fn summarize_session(json: &Value) -> Result<SessionSummary> {
    if let Some(spaces) = json.get("spaces").and_then(Value::as_array) {
        let all_tabs = json
            .get("tabs")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Zen session JSON is missing a tabs array"))?;

        let mut collections = Vec::new();
        for space in spaces {
            let workspace_id = space.get("uuid").and_then(Value::as_str).map(str::to_owned);
            let title = space
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Unnamed Space")
                .to_owned();

            let tabs = all_tabs
                .iter()
                .filter(|tab| {
                    tab.get("zenWorkspace").and_then(Value::as_str) == workspace_id.as_deref()
                })
                .map(summarize_tab)
                .collect::<Vec<_>>();

            if !tabs.is_empty() {
                collections.push(CollectionSummary {
                    title,
                    workspace_id,
                    tabs,
                });
            }
        }

        let total_tabs = collections.iter().map(|collection| collection.tabs.len()).sum();

        return Ok(SessionSummary {
            kind: SessionKind::ZenSpaces,
            saved_at_ms: json.get("lastCollected").and_then(Value::as_i64),
            collections,
            total_tabs,
        });
    }

    if let Some(windows) = json.get("windows").and_then(Value::as_array) {
        let collections = windows
            .iter()
            .enumerate()
            .map(|(index, window)| {
                let title = derive_window_title(window, index);
                let tabs = window
                    .get("tabs")
                    .and_then(Value::as_array)
                    .map(|tabs| tabs.iter().map(summarize_tab).collect())
                    .unwrap_or_default();

                CollectionSummary {
                    title,
                    workspace_id: window
                        .get("workspaceID")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    tabs,
                }
            })
            .collect::<Vec<_>>();
        let total_tabs = collections.iter().map(|collection| collection.tabs.len()).sum();

        return Ok(SessionSummary {
            kind: SessionKind::FirefoxWindows,
            saved_at_ms: json
                .get("session")
                .and_then(|value| value.get("lastUpdate"))
                .and_then(Value::as_i64),
            collections,
            total_tabs,
        });
    }

    bail!("unrecognized session JSON shape")
}

fn derive_window_title(window: &Value, index: usize) -> String {
    window
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            window
                .get("tabs")
                .and_then(Value::as_array)
                .and_then(|tabs| tabs.first())
                .and_then(current_entry)
                .and_then(|entry| entry.get("title"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("Window {}", index + 1))
}

fn summarize_tab(tab: &Value) -> TabSummary {
    let entry = current_entry(tab);
    let url = entry
        .and_then(|entry| entry.get("url"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let title = entry
        .and_then(|entry| entry.get("title"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| url.clone())
        .unwrap_or_else(|| "Untitled Tab".to_owned());

    TabSummary {
        title,
        url,
        pinned: tab.get("pinned").and_then(Value::as_bool).unwrap_or(false),
        essential: tab
            .get("zenEssential")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn current_entry(tab: &Value) -> Option<&Value> {
    let entries = tab.get("entries")?.as_array()?;
    if entries.is_empty() {
        return None;
    }

    let index = tab
        .get("index")
        .and_then(Value::as_u64)
        .map(|value| value.saturating_sub(1) as usize)
        .unwrap_or(0);

    entries.get(index).or_else(|| entries.last())
}

fn parse_snapshot_key(file_name: &str) -> (i32, i32, i32, i32) {
    let Some(stem) = file_name.strip_prefix("zen-sessions-") else {
        return (0, 0, 0, 0);
    };

    let mut parts = stem.split('-');
    let year = parts.next().and_then(|value| value.parse().ok()).unwrap_or(0);
    let month = parts.next().and_then(|value| value.parse().ok()).unwrap_or(0);
    let day = parts.next().and_then(|value| value.parse().ok()).unwrap_or(0);
    let hour = parts
        .next()
        .and_then(|value| value.split('.').next())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    (year, month, day, hour)
}

fn snapshot_label(file_name: &str) -> String {
    let (year, month, day, hour) = parse_snapshot_key(file_name);
    if year == 0 {
        return file_name.to_owned();
    }

    format!("{year:04}-{month:02}-{day:02} {hour:02}:00")
}

#[cfg(test)]
mod tests {
    use super::parse_snapshot_key;

    #[test]
    fn parses_snapshot_key() {
        assert_eq!(
            parse_snapshot_key("zen-sessions-2026-03-18-15.jsonlz4"),
            (2026, 3, 18, 15)
        );
    }
}
