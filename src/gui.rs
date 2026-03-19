use std::{
    cell::RefCell,
    env,
    path::{Path, PathBuf},
    rc::Rc,
};

use anyhow::{Context, Result};
use cpp::cpp;
use qmetaobject::*;
use serde_json::{Value, json};

use crate::zen::{self, BackupFile, CollectionSelection};

cpp! {{
    #include <QtGui/QGuiApplication>
    #include <QtGui/QIcon>
    #include <QtCore/QCoreApplication>
    #include <QtCore/QString>
}}

#[derive(Default)]
struct GuiState {
    profile: Option<PathBuf>,
    backups: Vec<BackupState>,
    active_backup: Option<usize>,
    launch_after_restore: bool,
    zen_executable: Option<PathBuf>,
}

#[derive(Clone)]
struct BackupState {
    file_name: String,
    snapshot_label: String,
    saved_at_ms: Option<i64>,
    total_tabs: usize,
    collections: Vec<CollectionState>,
    inner: BackupFile,
}

#[derive(Clone)]
struct CollectionState {
    title: String,
    workspace_id: Option<String>,
    tabs: Vec<TabState>,
}

#[derive(Clone)]
struct TabState {
    title: String,
    url: Option<String>,
    pinned: bool,
    essential: bool,
    selected: bool,
}

#[derive(QObject)]
struct AppBridge {
    base: qt_base_class!(trait QObject),
    status_text: qt_property!(QString; NOTIFY status_text_changed),
    status_text_changed: qt_signal!(),
    profile_path: qt_property!(QString; NOTIFY profile_path_changed),
    profile_path_changed: qt_signal!(),
    should_prompt_for_profile: qt_property!(bool; NOTIFY should_prompt_for_profile_changed),
    should_prompt_for_profile_changed: qt_signal!(),
    backups_json: qt_property!(QString; NOTIFY backups_json_changed),
    backups_json_changed: qt_signal!(),
    active_backup_json: qt_property!(QString; NOTIFY active_backup_json_changed),
    active_backup_json_changed: qt_signal!(),
    zen_running: qt_property!(bool; NOTIFY zen_running_changed),
    zen_running_changed: qt_signal!(),
    launch_after_restore: qt_property!(bool; NOTIFY launch_after_restore_changed),
    launch_after_restore_changed: qt_signal!(),
    show_about_on_startup: qt_property!(bool; NOTIFY show_about_on_startup_changed),
    show_about_on_startup_changed: qt_signal!(),
    refresh: qt_method!(fn refresh(&mut self) { self.do_refresh(); }),
    select_backup: qt_method!(fn select_backup(&mut self, index: i32) { self.do_select_backup(index); }),
    toggle_collection: qt_method!(fn toggle_collection(&mut self, collection_index: i32, selected: bool) {
        self.do_toggle_collection(collection_index, selected);
    }),
    toggle_tab: qt_method!(fn toggle_tab(&mut self, collection_index: i32, tab_index: i32, selected: bool) {
        self.do_toggle_tab(collection_index, tab_index, selected);
    }),
    set_launch_after_restore: qt_method!(fn set_launch_after_restore(&mut self, value: bool) {
        self.do_set_launch_after_restore(value);
    }),
    set_profile_path: qt_method!(fn set_profile_path(&mut self, profile_path: QString) {
        self.do_set_profile_path(profile_path);
    }),
    restore_full_backup: qt_method!(fn restore_full_backup(&mut self) { self.do_restore_full_backup(); }),
    restore_selected: qt_method!(fn restore_selected(&mut self) { self.do_restore_selected(); }),
    state: Rc<RefCell<GuiState>>,
}

impl Default for AppBridge {
    fn default() -> Self {
        Self {
            base: Default::default(),
            status_text: QString::from("Loading Zen backups..."),
            status_text_changed: Default::default(),
            profile_path: QString::from(""),
            profile_path_changed: Default::default(),
            should_prompt_for_profile: false,
            should_prompt_for_profile_changed: Default::default(),
            backups_json: QString::from("[]"),
            backups_json_changed: Default::default(),
            active_backup_json: QString::from("{}"),
            active_backup_json_changed: Default::default(),
            zen_running: false,
            zen_running_changed: Default::default(),
            launch_after_restore: false,
            launch_after_restore_changed: Default::default(),
            show_about_on_startup: false,
            show_about_on_startup_changed: Default::default(),
            refresh: Default::default(),
            select_backup: Default::default(),
            toggle_collection: Default::default(),
            toggle_tab: Default::default(),
            set_launch_after_restore: Default::default(),
            set_profile_path: Default::default(),
            restore_full_backup: Default::default(),
            restore_selected: Default::default(),
            state: Rc::new(RefCell::new(GuiState::default())),
        }
    }
}

impl AppBridge {
    fn do_refresh(&mut self) {
        match self.refresh_inner() {
            Ok(()) => {}
            Err(error) => self.set_status(format!("Failed to scan backups: {error}")),
        }
    }

    fn refresh_inner(&mut self) -> Result<()> {
        let profile = {
            let state = self.state.borrow();
            state.profile.clone()
        }
        .unwrap_or_else(|| zen::detect_default_profile().unwrap_or_default());

        if profile.as_os_str().is_empty() {
            self.clear_loaded_profile();
            self.set_should_prompt_for_profile(true);
            self.set_status("Could not find a Zen profile. Start the app with ZEN_PROFILE or --profile.");
            return Ok(());
        }

        let backups = zen::scan_backups(&profile)?;
        let loaded_count = {
            let mut state = self.state.borrow_mut();
            state.profile = Some(profile.clone());
            state.backups = backups.into_iter().map(BackupState::from_backup).collect();
            state.active_backup = (!state.backups.is_empty()).then_some(0);
            state.backups.len()
        };

        self.profile_path = QString::from(profile.to_string_lossy().as_ref());
        self.profile_path_changed();
        self.set_should_prompt_for_profile(false);
        self.update_zen_running();
        self.publish_backups();
        self.publish_active_backup();
        self.set_status(format!(
            "Loaded {} backup snapshot(s) from {}.",
            loaded_count,
            zen::profile_backup_dir(&profile).display()
        ));
        Ok(())
    }

    fn do_select_backup(&mut self, index: i32) {
        let mut state = self.state.borrow_mut();
        if index < 0 || index as usize >= state.backups.len() {
            return;
        }
        state.active_backup = Some(index as usize);
        drop(state);
        self.publish_backups();
        self.publish_active_backup();
    }

    fn do_toggle_collection(&mut self, collection_index: i32, selected: bool) {
        let mut state = self.state.borrow_mut();
        let Some(backup) = state.active_backup.and_then(|index| state.backups.get_mut(index)) else {
            return;
        };
        let Some(collection) = backup.collections.get_mut(collection_index as usize) else {
            return;
        };
        for tab in &mut collection.tabs {
            if tab.url.is_some() {
                tab.selected = selected;
            }
        }
        drop(state);
        self.publish_active_backup();
    }

    fn do_toggle_tab(&mut self, collection_index: i32, tab_index: i32, selected: bool) {
        let mut state = self.state.borrow_mut();
        let Some(backup) = state.active_backup.and_then(|index| state.backups.get_mut(index)) else {
            return;
        };
        let Some(collection) = backup.collections.get_mut(collection_index as usize) else {
            return;
        };
        let Some(tab) = collection.tabs.get_mut(tab_index as usize) else {
            return;
        };
        if tab.url.is_some() {
            tab.selected = selected;
        }
        drop(state);
        self.publish_active_backup();
    }

    fn do_set_launch_after_restore(&mut self, value: bool) {
        self.state.borrow_mut().launch_after_restore = value;
        self.launch_after_restore = value;
        self.launch_after_restore_changed();
    }

    fn do_set_profile_path(&mut self, profile_path: QString) {
        let profile_path = profile_path.to_string();
        if profile_path.is_empty() {
            self.set_status("The selected folder is not a valid local path.");
            self.set_should_prompt_for_profile(true);
            return;
        }

        self.state.borrow_mut().profile = Some(PathBuf::from(profile_path));
        self.do_refresh();
    }

    fn do_restore_full_backup(&mut self) {
        match self.restore_full_backup_inner() {
            Ok(()) => {}
            Err(error) => self.set_status(format!("Restore failed: {error}")),
        }
    }

    fn restore_full_backup_inner(&mut self) -> Result<()> {
        let profile = self
            .state
            .borrow()
            .profile
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no Zen profile is loaded"))?;

        if zen::is_profile_in_use(&profile) {
            self.update_zen_running();
            self.set_status("Close Zen before restoring a backup file.");
            return Ok(());
        }

        let (_profile, backup, launch_after_restore, executable) = self.current_restore_context()?;
        let destination = zen::copy_backup_to_main_session(&backup.inner, &profile)?;

        if launch_after_restore {
            zen::launch_zen(executable.as_deref()).context("restore succeeded but launching Zen failed")?;
        }

        self.update_zen_running();
        self.set_status(format!(
            "Restored full backup {} to {}.",
            backup.file_name,
            destination.display()
        ));
        Ok(())
    }

    fn do_restore_selected(&mut self) {
        match self.restore_selected_inner() {
            Ok(()) => {}
            Err(error) => self.set_status(format!("Selective restore failed: {error}")),
        }
    }

    fn restore_selected_inner(&mut self) -> Result<()> {
        let profile = self
            .state
            .borrow()
            .profile
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no Zen profile is loaded"))?;

        if zen::is_profile_in_use(&profile) {
            self.update_zen_running();
            self.set_status("Close Zen before writing a selective restore file.");
            return Ok(());
        }

        let (_profile, backup, launch_after_restore, executable) = self.current_restore_context()?;
        let selections = backup
            .collections
            .iter()
            .enumerate()
            .filter_map(|(collection_index, collection)| {
                let selected_tab_indices = collection
                    .tabs
                    .iter()
                    .enumerate()
                    .filter_map(|(tab_index, tab)| tab.selected.then_some(tab_index))
                    .collect::<std::collections::BTreeSet<_>>();
                (!selected_tab_indices.is_empty()).then_some(CollectionSelection {
                    collection_index,
                    selected_tab_indices,
                })
            })
            .collect::<Vec<_>>();

        if selections.is_empty() {
            self.set_status("No tabs are selected for restore.");
            return Ok(());
        }

        let destination = zen::write_filtered_restore(&backup.inner, &profile, &selections)?;
        if launch_after_restore {
            zen::launch_zen(executable.as_deref())
                .context("selective restore succeeded but launching Zen failed")?;
        }

        self.update_zen_running();
        self.set_status(format!(
            "Wrote a filtered restore file to {}.",
            destination.display()
        ));
        Ok(())
    }

    fn current_restore_context(&self) -> Result<(PathBuf, BackupState, bool, Option<PathBuf>)> {
        let state = self.state.borrow();
        let profile = state
            .profile
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no Zen profile is loaded"))?;
        let active_index = state
            .active_backup
            .ok_or_else(|| anyhow::anyhow!("no backup is selected"))?;
        let backup = state
            .backups
            .get(active_index)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("selected backup is missing"))?;
        Ok((
            profile,
            backup,
            state.launch_after_restore,
            state.zen_executable.clone(),
        ))
    }

    fn update_zen_running(&mut self) {
        let profile = self.state.borrow().profile.clone();
        self.zen_running = profile
            .as_ref()
            .is_some_and(|path| zen::is_profile_in_use(path));
        self.zen_running_changed();
    }

    fn publish_backups(&mut self) {
        let state = self.state.borrow();
        let active_index = state.active_backup;
        let data = state
            .backups
            .iter()
            .enumerate()
            .map(|(index, backup)| {
                json!({
                    "index": index,
                    "fileName": backup.file_name,
                    "snapshotLabel": backup.snapshot_label,
                    "savedAtMs": backup.saved_at_ms,
                    "collections": backup.collections.len(),
                    "tabs": backup.total_tabs,
                    "active": active_index == Some(index),
                })
            })
            .collect::<Vec<_>>();
        self.backups_json = QString::from(
            serde_json::to_string(&data).unwrap_or_else(|_| "[]".to_owned()),
        );
        self.backups_json_changed();
    }

    fn publish_active_backup(&mut self) {
        let state = self.state.borrow();
        let payload = state
            .active_backup
            .and_then(|index| state.backups.get(index))
            .map(BackupState::to_json)
            .unwrap_or_else(|| json!({}));

        self.active_backup_json = QString::from(
            serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_owned()),
        );
        self.active_backup_json_changed();
    }

    fn set_status(&mut self, message: impl AsRef<str>) {
        self.status_text = QString::from(message.as_ref());
        self.status_text_changed();
    }

    fn set_should_prompt_for_profile(&mut self, value: bool) {
        if self.should_prompt_for_profile == value {
            return;
        }
        self.should_prompt_for_profile = value;
        self.should_prompt_for_profile_changed();
    }

    fn clear_loaded_profile(&mut self) {
        {
            let mut state = self.state.borrow_mut();
            state.profile = None;
            state.backups.clear();
            state.active_backup = None;
        }

        self.profile_path = QString::from("");
        self.profile_path_changed();
        self.publish_backups();
        self.publish_active_backup();
        self.update_zen_running();
    }
}

impl BackupState {
    fn from_backup(inner: BackupFile) -> Self {
        let collections = inner
            .summary
            .collections
            .iter()
            .map(|collection| CollectionState {
                title: collection.title.clone(),
                workspace_id: collection.workspace_id.clone(),
                tabs: collection
                    .tabs
                    .iter()
                    .map(|tab| TabState {
                        title: tab.title.clone(),
                        url: tab.url.clone(),
                        pinned: tab.pinned,
                        essential: tab.essential,
                        selected: tab.url.is_some(),
                    })
                    .collect(),
            })
            .collect();

        Self {
            file_name: inner.file_name.clone(),
            snapshot_label: inner.snapshot_label.clone(),
            saved_at_ms: inner.summary.saved_at_ms,
            total_tabs: inner.summary.total_tabs,
            collections,
            inner,
        }
    }

    fn selected_count(&self) -> usize {
        self.collections
            .iter()
            .map(|collection| {
                collection
                    .tabs
                    .iter()
                    .filter(|tab| tab.selected && tab.url.is_some())
                    .count()
            })
            .sum()
    }

    fn to_json(&self) -> Value {
        json!({
            "fileName": self.file_name,
            "snapshotLabel": self.snapshot_label,
            "savedAtMs": self.saved_at_ms,
            "totalTabs": self.total_tabs,
            "selectedTabs": self.selected_count(),
            "collections": self.collections.iter().enumerate().map(|(collection_index, collection)| {
                json!({
                    "index": collection_index,
                    "title": collection.title,
                    "workspaceId": collection.workspace_id,
                    "tabCount": collection.tabs.len(),
                    "selectedCount": collection.tabs.iter().filter(|tab| tab.selected && tab.url.is_some()).count(),
                    "tabs": collection.tabs.iter().enumerate().map(|(tab_index, tab)| {
                        json!({
                            "index": tab_index,
                            "title": tab.title,
                            "url": tab.url,
                            "pinned": tab.pinned,
                            "essential": tab.essential,
                            "selected": tab.selected,
                            "restorable": tab.url.is_some(),
                        })
                    }).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>()
        })
    }
}

pub fn run(profile: Option<PathBuf>, show_about_on_startup: bool) -> Result<()> {
    let mut engine = QmlEngine::new();
    let mut bridge = AppBridge::default();
    if let Ok(current_dir) = std::env::current_dir() {
        let icon_path = current_dir.join("assets/restore-zen-session-icon.png");
        configure_application(&icon_path, resolve_desktop_file_name());
    }
    let initial_profile = profile.or_else(|| zen::detect_default_profile().ok());
    bridge.state.borrow_mut().profile = initial_profile;
    bridge.should_prompt_for_profile = bridge.state.borrow().profile.is_none();
    bridge.show_about_on_startup = show_about_on_startup;
    bridge.do_refresh();
    let bridge = QObjectBox::new(bridge);
    engine.set_object_property("backend".into(), bridge.pinned());
    engine.load_data(include_str!("../qml/main.qml").into());
    engine.exec();
    Ok(())
}

fn configure_application(icon_path: &Path, desktop_file_name: Option<&str>) {
    let icon_path = QString::from(icon_path.to_string_lossy().as_ref());
    cpp!(unsafe [
        icon_path as "QString"
    ] {
        QGuiApplication::setWindowIcon(QIcon(icon_path));
        QCoreApplication::setApplicationName(QStringLiteral("Restore Zen Session"));
        QCoreApplication::setApplicationVersion(QStringLiteral("0.3"));
        QCoreApplication::setOrganizationName(QStringLiteral("Pete Vagiakos"));
    });

    if let Some(desktop_file_name) = desktop_file_name {
        let desktop_file_name = QString::from(desktop_file_name);
        cpp!(unsafe [desktop_file_name as "QString"] {
            QGuiApplication::setDesktopFileName(desktop_file_name);
        });
    }
}

fn resolve_desktop_file_name() -> Option<&'static str> {
    if desktop_file_exists() {
        return Some("restore-zen-session");
    }

    None
}

fn desktop_file_exists() -> bool {
    if let Some(path) = env::var_os("XDG_DESKTOP_FILE_HINT") {
        if Path::new(&path).is_file() {
            return true;
        }
    }

    if let Ok(executable_path) = env::current_exe() {
        if let Some(executable_dir) = executable_path.parent() {
            let candidates = [
                executable_dir.join("restore-zen-session.desktop"),
                executable_dir.join("assets/restore-zen-session.desktop"),
            ];
            for candidate in candidates {
                if candidate.is_file() {
                    return true;
                }
            }
        }
    }

    if let Some(local_share) = dirs::data_local_dir() {
        if local_share
            .join("applications/restore-zen-session.desktop")
            .is_file()
        {
            return true;
        }
    }

    if let Some(data_dirs) = env::var_os("XDG_DATA_DIRS") {
        for dir in env::split_paths(&data_dirs) {
            if dir.join("applications/restore-zen-session.desktop").is_file() {
                return true;
            }
        }
    }

    let fallback_dirs = [
        PathBuf::from("/usr/local/share"),
        PathBuf::from("/usr/share"),
    ];
    for dir in fallback_dirs {
        if dir.join("applications/restore-zen-session.desktop").is_file() {
            return true;
        }
    }

    false
}
