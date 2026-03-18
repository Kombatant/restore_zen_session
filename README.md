# Zen Session Restore

This repository now contains a Rust application for inspecting and restoring Zen session backups directly from a Zen profile.

## Why Rust

A normal browser extension cannot:

- enumerate `<profile>/zen-sessions-backup/`
- copy a chosen backup back to `zen-sessions.jsonlz4`
- restart Zen

A native application can do all of that. This repository now starts with the native core and CLI first.

## Current Capabilities

- Scan `<profile>/zen-sessions-backup/`
- Detect and sort backup files like `zen-sessions-2026-03-18-15.jsonlz4`
- Decode Mozilla `mozLz40` / `.jsonlz4` session files
- Parse Zen-style session payloads with `spaces` and `tabs`
- Show a Qt 6 desktop UI with:
  - snapshot list
  - per-space tab browser
  - per-tab selection
  - full or selective restore actions
- Restore a chosen backup by copying it to `<profile>/zen-sessions.jsonlz4`
- Write a filtered restore file containing only selected spaces/tabs
- Optionally relaunch Zen after writing the restore file

## GUI

Run the desktop app:

```bash
cargo run -- --profile "/path/to/Zen profile"
```

If `--profile` is omitted, the app will try to auto-detect a profile under `~/.zen`.

Current GUI flow:

1. On launch, the app scans `<profile>/zen-sessions-backup/`
2. It shows snapshots newest first
3. You pick a snapshot and inspect spaces/tabs
4. You can deselect individual tabs or whole groups
5. You can either:
   - restore the full backup
   - write a filtered restore file from the selected tabs

Important:

- Zen should be fully closed before restoring
- if Zen is still running, the app warns instead of writing the session file

## CLI

Build:

```bash
cargo build
```

List backups:

```bash
cargo run -- --profile "/path/to/Zen profile" list
```

Show a backup summary by index:

```bash
cargo run -- --profile "/path/to/Zen profile" show 1
```

Show a backup summary by filename:

```bash
cargo run -- --profile "/path/to/Zen profile" show zen-sessions-2026-03-18-15.jsonlz4
```

Restore a backup into `zen-sessions.jsonlz4`:

```bash
cargo run -- --profile "/path/to/Zen profile" restore 1 --yes
```

Safety behavior:

- `restore` refuses to overwrite the live session file unless `--yes` is provided
- you should fully close Zen before running restore

You can also provide the profile with an environment variable:

```bash
export ZEN_PROFILE="/path/to/Zen profile"
cargo run -- list
```

## Real Example

Against your profile, the CLI is already able to:

- list backups in `/home/kombatant/.zen/e0rlo4t3.Default (release)/zen-sessions-backup`
- sort them newest first
- parse real Zen spaces and tab counts

## Project Layout

- `src/main.rs`: CLI entrypoint
- `src/cli.rs`: command parsing and command execution
- `src/mozlz4.rs`: Mozilla `.jsonlz4` decoding
- `src/zen.rs`: Zen backup scanning, parsing, summarizing, and restore copy logic
- `RESEARCH.md`: notes about Zen/Firefox session formats

## What Is Not Built Yet

- desktop GUI
- selective per-space restore copy flow
- selective per-tab restore into Zen session files
- folder/group reconstruction
- automatic Zen restart after restore
- automatic Zen profile detection

## Next Step

The current app already has the right native shape. The next improvement would be polishing the GUI behavior and theme further, especially:

- Breeze/KDE styling details
- confirmation flows
- profile chooser and settings
- more faithful selective restore behavior for groups/folders
