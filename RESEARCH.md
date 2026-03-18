# Zen / Firefox Session File Research

This note summarizes the session file format behind Zen's `zen-sessions.jsonlz4` backups and what matters for an extension that wants to inspect and restore them.

## High-confidence conclusions

- Zen's backup files are `.jsonlz4` files in the same family as Firefox session store files.
- The compression wrapper is Mozilla's custom `mozLz40\0` header plus an LZ4-compressed payload.
- After decompression, the contents are JSON.
- Firefox's current `session.schema.json` is the best primary source for the top-level structure and most stable fields.
- A pure WebExtension can parse a user-selected `.jsonlz4` file in-browser.
- A pure WebExtension cannot directly enumerate arbitrary files in `<profile>/zen-sessions-backup/`. For automatic folder listing you will need a native helper via Native Messaging, or you must ask the user to pick files/folders manually.

## Zen-specific backup behavior

Zen's own docs describe the current backup system introduced in Zen v1.18+:

- Backups live in `<profile>/zen-sessions-backup/`
- Files are named like `zen-sessions-<date-metadata>.jsonlz4`
- Manual recovery is done by duplicating one backup file, renaming it to `zen-sessions.jsonlz4`, and copying it to the profile root while Zen is fully closed

Source:

- Zen docs: https://docs.zen-browser.app/user-manual/window-sync

This is the strongest source for the exact Zen folder and restore workflow.

## Firefox session store file locations and precedence

Firefox source code shows the standard session store paths and load order:

- `sessionstore.jsonlz4`
- `sessionstore-backups/previous.jsonlz4`
- `sessionstore-backups/recovery.jsonlz4`
- `sessionstore-backups/recovery.baklz4`
- `sessionstore-backups/upgrade.jsonlz4-<buildid>`

Load order in Firefox source:

1. `clean` (`sessionstore.jsonlz4`)
2. `recovery`
3. `recoveryBackup`
4. `cleanBackup`
5. `upgradeBackup`

Sources:

- Session file paths and load order:
  https://searchfox.org/firefox-main/source/browser/components/sessionstore/SessionFile.sys.mjs

Relevant lines visible in Searchfox:

- `clean`: `sessionstore.jsonlz4`
- `cleanBackup`: `sessionstore-backups/previous.jsonlz4`
- `recovery`: `sessionstore-backups/recovery.jsonlz4`
- `recoveryBackup`: `sessionstore-backups/recovery.baklz4`
- `upgradeBackupPrefix`: `sessionstore-backups/upgrade.jsonlz4-`

Zen's `zen-sessions.jsonlz4` and `zen-sessions-backup/` naming are Zen-specific, but the underlying payload appears intended to be equivalent enough to use Firefox session parsing techniques.

## Compression wrapper: `mozLz40`

Mozilla's `IOUtils` source documents the on-disk layout:

- 8-byte magic number: `mozLz40\0`
- 4-byte little-endian uncompressed content size
- compressed bytes produced by Mozilla's LZ4 block compression

Source:

- https://searchfox.org/firefox-main/source/xpcom/ioutils/IOUtils.h

Important details from source:

- Magic number bytes: `{'m', 'o', 'z', 'L', 'z', '4', '0', '\0'}`
- Header size: `8 + sizeof(uint32_t)` = 12 bytes

Practical decode steps:

1. Read file as bytes
2. Verify first 8 bytes are `mozLz40\0`
3. Read bytes 8..11 as little-endian uncompressed size
4. Decompress remaining bytes using LZ4 block decompression, not standard LZ4 frame parsing
5. Decode the result as UTF-8 JSON

## Session JSON top-level structure

Firefox's current schema is here:

- https://searchfox.org/firefox-main/source/browser/components/sessionstore/session.schema.json

Required top-level fields:

- `version`
- `windows`

Other common top-level fields:

- `_closedWindows`
- `savedGroups`
- `session`
- `global`
- `cookies`
- `browserConsole`
- `browserToolbox`
- `selectedWindow`
- `maxSplitViewId`

Useful session metadata:

- `session.lastUpdate`
- `session.startTime`
- `session.recentCrashes`

## Window structure

From the schema, an open window includes fields such as:

- `tabs`
- `selected`
- `title`
- `extData`
- `sidebar`
- `splitViews`
- `_closedTabs`
- geometry fields: `screenX`, `screenY`, `width`, `height`, `sizemode`, `zIndex`
- Zen-relevant looking field: `workspaceID`
- Other current fields in schema: `isPopup`, `isAIWindow`

For a session browser UI, the most useful display fields are usually:

- window index
- selected tab index
- tab count
- workspace ID if present
- split view metadata if present
- last modified time from the backup filename or filesystem

## Tab structure

From the schema, each tab commonly includes:

- `entries`: session history entries
- `index`: current selected history entry within `entries`
- `requestedIndex`
- `lastAccessed`
- `hidden`
- `userContextId`
- `attributes`
- `image`
- `storage`
- `canonicalUrl`
- `removeAfterRestore`

Minimum fields the schema currently requires for `tab`:

- `entries`
- `lastAccessed`
- `hidden`
- `userContextId`
- `index`

For a session browser UI, the most useful per-tab values are:

- current URL: `entries[index - 1]?.url`
- current title: `entries[index - 1]?.title`
- favicon: `image`
- container ID: `userContextId`
- hidden/pinned/group-related state if present in `attributes` or extension data

## History entry structure

The schema includes many possible history entry fields. The most useful ones for inspection are:

- `url`
- `title`
- `triggeringPrincipal_base64`
- `principalToInherit_base64`
- `resultPrincipalURI`
- `scroll`
- `presState`
- `hasUserInteraction`
- `persist`

Firefox source docs on collected data also show examples of:

- `entries`
- `index`
- `requestedIndex`
- `fromIdx`

Source:

- https://firefox-source-docs.mozilla.org/toolkit/components/sessionstore/collection.html

That page also documents the structure of collected:

- scroll data
- form data
- session storage data
- session history data

## Closed tabs and closed windows

Schema fields:

- top-level `_closedWindows`
- per-window `_closedTabs`

Closed tab objects include:

- `state`
- `title`
- `image`
- `pos`
- `closedAt`
- `closedId`
- `sourceWindowId`

Closed window objects include:

- all normal window fields
- `closedAt`
- `closedId`

This means your extension can show both:

- live/open windows from that saved session
- previously closed tabs/windows that were still preserved inside that backup

## Cookies and storage

Top-level `cookies` are in schema and include fields such as:

- `host`
- `name`
- `value`
- `path`
- `secure`
- `httponly`
- `expiry`
- `originAttributes`

`storage` exists on tabs and Firefox docs separately describe collected session storage structures.

For an MVP extension, you probably do not need to restore cookies or DOM/session-storage from parsed backup files yourself. For user-facing restore, URLs and window grouping are the main value.

## What "restore" can realistically mean in an extension

There are two different restore levels:

### 1. Reopen URLs from a backup file

This is feasible in a normal extension:

- Parse backup file
- Extract windows and tabs
- Call `browser.windows.create({ url: [...] })`
- Optionally preserve one browser window per saved window

Pros:

- Simple
- No native host needed if the user imports the file manually

Cons:

- Does not fully restore deep Firefox session state
- Will not restore exact back/forward history stacks, scroll/form state, cookies, or internal browser session metadata

### 2. True profile-level session replacement

This means writing a chosen backup back to:

- `<profile>/zen-sessions.jsonlz4`

This is the closest to Zen's documented manual recovery flow.

Pros:

- Much closer to real session restoration

Cons:

- A WebExtension cannot safely write arbitrary profile files by itself
- Zen should be closed when replacing the live session file
- This strongly points to a native helper

## WebExtension capability limits

Mozilla's extension docs are clear:

- Native Messaging is how extensions access resources outside normal WebExtension APIs
- File processing that requires local app capabilities should be done through a native app

Sources:

- Native Messaging:
  https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging
- Working with files:
  https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Working_with_files

Implication:

- If you want the extension to automatically scan `<profile>/zen-sessions-backup/`, you need a native host.
- If you want a store-publishable extension with no native helper, the UX should be "Choose backup file" or possibly "Choose backup folder" if Zen/Firefox exposes a supported picker flow you can use from extension UI.

## Recommended architecture options

### Option A: extension-only importer

Use this if you want the smallest build first.

Flow:

1. User opens extension popup or page
2. User chooses one `.jsonlz4` file
3. Extension decodes and previews windows/tabs
4. Extension restores by reopening URLs into new windows

What this proves:

- decoding works
- schema reading works
- backup browsing UI works

What it cannot do:

- auto-list backup directory
- replace live Zen session file

### Option B: extension + native messaging host

Use this if your real goal is seamless Zen backup browsing and true restore.

Flow:

1. Extension asks native host for available backup files in `<profile>/zen-sessions-backup/`
2. Native host returns file metadata and optionally parsed summaries
3. User selects a backup
4. For preview, either:
   - extension parses bytes returned by host, or
   - host parses JSON and returns a summary
5. For true restore, native host copies selected backup to `<profile>/zen-sessions.jsonlz4`
6. User is prompted to fully close/reopen Zen, or the host can do an OS-specific restart flow if you want to take that on

This is the architecture that matches your stated goal best.

## Minimal parsing model for UI

You do not need the entire schema for a useful first pass. A practical parser can focus on:

- top-level `windows`
- each window's `selected`, `workspaceID`, `tabs`, `_closedTabs`
- each tab's `entries`, `index`, `image`, `lastAccessed`, `userContextId`
- each entry's `url`, `title`
- top-level `_closedWindows`
- top-level `session.lastUpdate`

Derived UI helpers:

- current tab = `tab.entries[(tab.index || 1) - 1]`
- tab display title = current entry title or current entry URL
- session window count = `windows.length`
- total live tabs = sum of `window.tabs.length`
- total closed tabs = sum of `_closedTabs.length`

## Decoder notes for implementation

In JavaScript, the decode path is roughly:

```js
function decodeMozLz4(arrayBuffer, lz4Decompress) {
  const bytes = new Uint8Array(arrayBuffer);
  const magic = new TextDecoder().decode(bytes.subarray(0, 8));
  if (magic !== "mozLz40\u0000") {
    throw new Error("Invalid MOZLZ4 header");
  }

  const view = new DataView(arrayBuffer, 8, 4);
  const expectedSize = view.getUint32(0, true);
  const compressed = bytes.subarray(12);
  const decompressed = lz4Decompress(compressed, expectedSize);
  return JSON.parse(new TextDecoder().decode(decompressed));
}
```

Important:

- Use an LZ4 block decompressor compatible with Mozilla's payload, not an LZ4 frame decoder that expects standard frame headers.
- Validate the output length against the stored 32-bit size.

## Sources

- Zen backup/recovery docs:
  https://docs.zen-browser.app/user-manual/window-sync
- Firefox session store source directory:
  https://searchfox.org/firefox-main/source/browser/components/sessionstore
- Firefox session schema:
  https://searchfox.org/firefox-main/source/browser/components/sessionstore/session.schema.json
- Firefox session file paths/load order:
  https://searchfox.org/firefox-main/source/browser/components/sessionstore/SessionFile.sys.mjs
- Firefox session migration read/write with compressed JSON:
  https://searchfox.org/firefox-main/source/browser/components/sessionstore/SessionMigration.sys.mjs
- Mozilla `mozLz40` wrapper format:
  https://searchfox.org/firefox-main/source/xpcom/ioutils/IOUtils.h
- Firefox source docs, session store overview:
  https://firefox-source-docs.mozilla.org/toolkit/components/sessionstore/index.html
- Firefox source docs, collected data structures:
  https://firefox-source-docs.mozilla.org/toolkit/components/sessionstore/collection.html
- Firefox source docs, restore flow:
  https://firefox-source-docs.mozilla.org/toolkit/components/sessionstore/restoredata.html
- MDN Native Messaging:
  https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging
- MDN Working with files:
  https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Working_with_files
- MDN sessions API:
  https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/sessions
- MDN `sessions.restore()`:
  https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/sessions/restore

## Recommended next step

Build the project in two phases:

1. Extension-only prototype that imports a chosen `.jsonlz4` file and previews/restores URLs.
2. Native-messaging companion that enumerates `zen-sessions-backup/` and performs true file-based restore to `zen-sessions.jsonlz4`.
