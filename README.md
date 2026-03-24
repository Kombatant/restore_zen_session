# Zen Session Restore

`zen-session-restore` is a native Rust utility for inspecting and restoring Zen Browser session backups from a local profile.

It reads Zen backup snapshots from `zen-sessions-backup`, decodes Mozilla `.jsonlz4` session files, shows the contents as spaces or browser windows, and writes a chosen snapshot back to the live `zen-sessions.jsonlz4` session file. The application provides both a desktop GUI and a CLI.

## What The Application Does

The app is built around the Zen profile directory, usually under `~/.zen/<profile>`.

It can:

- auto-detect the most recent Zen profile that contains a `zen-sessions-backup` directory
- scan and sort available backup snapshots
- decode `.jsonlz4`, `.baklz4`, and plain `.json` session files
- summarize Zen session data as spaces and tabs
- summarize Firefox-style session data as windows and tabs
- preview backup contents before restoring
- restore an entire backup snapshot to `zen-sessions.jsonlz4`
- write a filtered restore file containing only selected tabs
- detect whether the target Zen profile is currently in use
- optionally relaunch Zen after a restore from the GUI
- mirror `zen-sessions-backup` to Google Drive

## GUI

<img width="2352" height="1643" alt="image" src="https://github.com/user-attachments/assets/2ecefc5b-da76-4dbf-b56f-ff301a1216e3" />


Running the binary without a subcommand starts the Qt desktop interface:

```bash
cargo run -- --profile "/path/to/Zen profile"
```

If `--profile` is omitted, the app tries to find a profile automatically under `~/.zen`. If no usable profile is found, the GUI prompts for one.

The GUI lets you:

- browse backup snapshots newest first
- inspect spaces or windows inside a snapshot
- review individual tabs, including pinned and essential flags
- select or deselect entire collections or individual tabs
- restore the full backup
- restore only the selected tabs
- relaunch Zen after restore if desired
- enable Google Drive sync for the backup folder
- connect Google Drive in the browser instead of pasting tokens manually
- choose a retention window from 1 to 12 months before sync
- open an About dialog with version, author, repository, and issue-reporting links

Google Drive sync requires a local `google.json` file with your own OAuth desktop client credentials. The file is not included in the repository and must sit next to the app executable. If you are running with `cargo run`, placing `google.json` in the project root also works during development. If the file is missing, the Cloud Sync sidebar stays disabled and the app points users back to this README.

For Linux desktop integration, the repository now also includes [`assets/restore-zen-session.desktop`](assets/restore-zen-session.desktop), including an `About Restore Zen Session...` desktop action. Install or reference that desktop file from your launcher if you want the panel or dock context menu to expose the About action. The GUI only advertises that desktop file to Qt when it can actually find a matching `restore-zen-session.desktop` on disk, which avoids host portal registration errors for standalone release binaries.

The GUI refuses to write a restore file while the selected profile appears to be open in Zen.

## CLI

The CLI is useful for scripting or quick inspection.

Build the project:

```bash
cargo build
```

List backups:

```bash
cargo run -- --profile "/path/to/Zen profile" list
```

Show a backup by index:

```bash
cargo run -- --profile "/path/to/Zen profile" show 1
```

Show a backup by file name:

```bash
cargo run -- --profile "/path/to/Zen profile" show zen-sessions-2026-03-18-15.jsonlz4
```

Restore a full backup:

```bash
cargo run -- --profile "/path/to/Zen profile" restore 1 --yes
```

You can also provide the profile path through `ZEN_PROFILE`:

```bash
export ZEN_PROFILE="/path/to/Zen profile"
cargo run -- list
```

## Restore Behavior And Safety

Restoring overwrites the live session file at `<profile>/zen-sessions.jsonlz4`.

Important constraints:

- close Zen before restoring a session
- the CLI requires `--yes` before it overwrites the live session file
- the GUI checks whether the selected profile is currently in use and blocks restore when it is
- selective restore rewrites a valid session file containing only the chosen tabs

## Requirements

This project is written in Rust and uses Qt via `qmetaobject` / `qttypes` for the desktop UI.

You need:

- a Rust toolchain
- Qt 6 development libraries available to the build
- access to a local Zen profile directory

In the current workspace, `cargo build` completes successfully.

## Project Layout

- `src/main.rs`: chooses GUI or CLI mode
- `src/cli.rs`: command-line parsing and command handlers
- `src/gui.rs`: Qt bridge and restore workflows
- `src/zen.rs`: profile detection, backup scanning, parsing, filtering, and restore logic
- `src/mozlz4.rs`: Mozilla LZ4 encode/decode helpers
- `qml/main.qml`: desktop interface
- `RESEARCH.md`: notes on Zen and Firefox session formats

## Notes

This tool operates directly on local session backup files. It is not a browser extension and does not depend on Zen sync or remote services.

The GUI now includes a Google Drive sync panel for `zen-sessions-backup`. Sync creates or reuses `Backup/Zen` in Google Drive, mirrors the local backup folder into it, deletes remote files that no longer exist locally, and prunes local backups older than the selected 1-12 month retention window before syncing.

The current Google integration opens the user's browser for OAuth sign-in and stores the resulting refresh token in the local app settings file under the user's config directory.

To enable Google Drive sync, create a `google.json` file with your own Google OAuth Desktop credentials.

File location:

- place `google.json` in the same folder as the app executable
- for local development with `cargo run`, placing `google.json` in the repository root also works
- do not commit this file to GitHub
- if the file is missing, all Cloud Sync controls remain disabled in the sidebar

File format:

```json
{
  "google_client_id": "your-google-oauth-client-id.apps.googleusercontent.com",
  "google_client_secret": "your-google-oauth-client-secret"
}
```

You need to create your own Google OAuth client in Google Cloud and use those values in `google.json`. Without that file, the GUI disables Google Drive sign-in.

GitHub README link for users:

- <https://github.com/Kombatant/restore_zen_session#readme>
