use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};

use crate::zen;

#[derive(Debug, Parser)]
#[command(name = "zen-session-restore")]
#[command(about = "Inspect and restore Zen session backup files", long_about = None)]
pub struct Cli {
    #[arg(long, global = true, env = "ZEN_PROFILE")]
    pub profile: Option<PathBuf>,

    #[arg(long)]
    pub about_dialog: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List backups found in <profile>/zen-sessions-backup
    List,
    /// Show a summary for a chosen backup file
    Show(BackupSelector),
    /// Copy a chosen backup file into <profile>/zen-sessions.jsonlz4
    Restore(RestoreArgs),
}

#[derive(Debug, Args)]
pub struct BackupSelector {
    /// Backup index from the list command or a backup filename/path
    pub backup: String,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Backup index from the list command or a backup filename/path
    pub backup: String,

    /// Confirm overwriting the main zen-sessions.jsonlz4 file
    #[arg(long)]
    pub yes: bool,
}

pub fn run(cli: Cli) -> Result<()> {
    let profile = resolve_profile(cli.profile)?;

    match cli.command.ok_or_else(|| anyhow::anyhow!("no CLI command was provided"))? {
        Command::List => list_backups(&profile),
        Command::Show(selector) => show_backup(&profile, &selector.backup),
        Command::Restore(args) => restore_backup(&profile, &args),
    }
}

fn resolve_profile(profile: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(profile) = profile {
        return Ok(profile);
    }

    zen::detect_default_profile().or_else(|_| {
        bail!("a Zen profile path is required. Pass --profile <path> or set ZEN_PROFILE")
    })
}

fn list_backups(profile: &PathBuf) -> Result<()> {
    let backups = zen::scan_backups(profile)?;
    if backups.is_empty() {
        println!(
            "No backups found in {}",
            zen::profile_backup_dir(profile).display()
        );
        return Ok(());
    }

    println!("Profile: {}", profile.display());
    println!("Backup directory: {}", zen::profile_backup_dir(profile).display());
    println!();

    for (index, backup) in backups.iter().enumerate() {
        println!(
            "{:>2}. {}  {} {}  {} tabs",
            index + 1,
            backup.snapshot_label,
            backup.summary.collections.len(),
            backup.summary.kind,
            backup.summary.total_tabs
        );
        println!("    {}", backup.file_name);
    }

    Ok(())
}

fn show_backup(profile: &PathBuf, selector: &str) -> Result<()> {
    let backup = zen::resolve_backup(profile, selector)?;

    println!("Backup: {}", backup.file_name);
    println!("Path: {}", backup.path.display());
    println!("Snapshot: {}", backup.snapshot_label);
    if let Some(saved_at_ms) = backup.summary.saved_at_ms {
        println!("Saved at (ms): {}", saved_at_ms);
    }
    println!(
        "Contains: {} {} / {} tabs",
        backup.summary.collections.len(),
        backup.summary.kind,
        backup.summary.total_tabs
    );
    println!();

    for (index, collection) in backup.summary.collections.iter().enumerate() {
        println!(
            "{:>2}. {}  {} tabs{}",
            index + 1,
            collection.title,
            collection.tabs.len(),
            collection
                .workspace_id
                .as_ref()
                .map(|id| format!("  [{}]", id))
                .unwrap_or_default()
        );

        for tab in collection.tabs.iter().take(8) {
            let pin = if tab.pinned { " pinned" } else { "" };
            let essential = if tab.essential { " essential" } else { "" };
            let url = tab.url.as_deref().unwrap_or("<no url>");
            println!("      - {}{}{} :: {}", tab.title, pin, essential, url);
        }

        if collection.tabs.len() > 8 {
            println!("      ... {} more tab(s)", collection.tabs.len() - 8);
        }
    }

    Ok(())
}

fn restore_backup(profile: &PathBuf, args: &RestoreArgs) -> Result<()> {
    if !args.yes {
        bail!(
            "restore will overwrite {}. Re-run with --yes when Zen is fully closed.",
            zen::main_session_file(profile).display()
        );
    }

    let backup = zen::resolve_backup(profile, &args.backup)?;
    let destination = zen::copy_backup_to_main_session(&backup, profile)
        .with_context(|| format!("failed to restore {}", backup.file_name))?;

    println!("Restored backup:");
    println!("  source: {}", backup.path.display());
    println!("  target: {}", destination.display());
    println!();
    println!("Next step: start Zen again and let it load the restored session.");

    Ok(())
}
