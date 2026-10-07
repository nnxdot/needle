# Needle code audit handoff for Opus

**Implementation status (2026-10-06):** All 29 findings and all three failing check categories have fixes in the current working-tree diff. The tracking table records the evidence and distinguishes automated/emulator validation from outstanding physical-device and manual UI checks. The original audit below is retained as baseline evidence.

Prepared on 2026-10-01 for Opus to independently verify the application audit findings, then fix confirmed issues. The scope is application correctness, data handling, performance, UI behavior, and build checks.

Audit baseline commit: `d53bfc4858b80c22adc2477a4dcf0267feb9d0a8`. The working tree was clean before this document was created. No application fixes were made during the audit. Temporary diagnostic test sources used for six reproductions were removed; those reproductions must be recreated independently rather than treated as existing regression coverage.

## Instructions for Opus

Complete a verification pass across all 29 findings and the three failing check categories before starting application fixes. Treat severity and conclusions as provisional until you inspect the current code and reproduce or otherwise substantiate the behavior.

1. Read repository instructions and inspect the current revision. The source line numbers below describe the audit baseline and may move.
2. For each finding, record **Verified**, **Rejected**, or **Needs environment**, with the actual trigger, observed result, and source or test evidence. Explain rejected findings rather than silently dropping them. Do not call a static trace a device reproduction.
3. Use disposable libraries and copied media for move, import, tag, sync, and undo verification. Keep the user's real library unchanged.
4. After the verification pass, fix verified findings in priority order. Inspect shared causes before making separate fixes, especially settings persistence, organize/undo, CUE identity, theme identity, and Android operation state.
5. Add meaningful regression coverage for confirmed data-loss and correctness failures. Use runtime or profiling checks for UI, hardware, and performance findings when automated coverage cannot establish the result.
6. Run the checks appropriate to each change, then the applicable final checks below. Record the result, relevant test names or reproduction instructions, and the fix commit or diff reference in the tracking table.
7. Leave environment-dependent items explicitly open when the required device, service, or platform is unavailable. Do not mark all 11 Android lint errors as independent runtime bugs.

## Priority and evidence

- **P1:** Potential loss or unintended replacement of user files or saved settings. Verify first.
- **P2:** Incorrect data association, failed features, crashes, missing recovery, or misleading operation results.
- **P3:** Smaller UI correctness and performance issues. Performance claims require measurement before severity is finalized.

Six findings were previously reproduced in temporary diagnostics: 01, 05, 07, 09, 10, and 11. The other findings are source traces, with platform documentation or existing check output where stated. All items are pending Opus verification.

## Findings

### Issue 01 Organize overwrites existing lyrics

**Priority:** P1. **Audit evidence:** Previously reproduced in temporary diagnostics.

**Source:** [crates/needle-core/src/doctor.rs](crates/needle-core/src/doctor.rs) lines 988 to 997.

Organize checks whether the destination audio file exists, then renames companion .lrc and .txt files without checking their destinations or reporting errors. A pre-existing destination lyric file can be replaced even when the destination audio filename is free.

**Verify first:** Use disposable audio and lyric files. Leave the destination audio path absent but create a destination lyric with different contents. Organize the source and inspect both lyrics and the returned result.

**Fix after verification:** Check all companion destinations before moving; preserve existing files and report companion failures. Keep enough undo information for every moved file.

**Acceptance:** A lyric collision never overwrites either lyric. Successful companion moves can be undone, and a failed companion move is visible.

### Issue 02 Android Open with imports overwrite earlier songs

**Priority:** P1. **Audit evidence:** Source trace; not reproduced on an Android device.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/MainActivity.kt](apps/android/app/src/main/java/fyi/nnx/needle/MainActivity.kt) lines 108 to 115.

The fallback import copies a content URI into filesDir/opened using its DISPLAY_NAME. outputStream truncates an existing file with the same name, so two unrelated files named song.mp3 replace the first app-owned copy and reuse its library path.

**Verify first:** Open two content-provider URIs with the same DISPLAY_NAME and different audio. Ensure they take the copy fallback rather than the directly readable filesystem path. Inspect the first imported song after opening the second.

**Fix after verification:** Allocate a unique destination or explicitly distinguish an update from a new import. Write to a temporary file before publishing a completed copy.

**Acceptance:** Both imports remain intact with distinct library identities. A failed second copy leaves the first playable.

### Issue 03 DSP settings writes erase presets and revert preferences

**Priority:** P1. **Audit evidence:** Source trace; independent runtime verification pending.

**Source:** [crates/needle-mobile/src/settings.rs](crates/needle-mobile/src/settings.rs) lines 278 to 308; [crates/needle-core/src/audio.rs](crates/needle-core/src/audio.rs) lines 1464 to 1469.

save_preset saves an updated settings record and then sends Command::Dsp. The worker changes only its cached DSP field and saves its entire older settings snapshot, which can erase the new preset. A subsequent DSP change can also restore deleted presets or revert unrelated preferences written elsewhere.

**Verify first:** Start the worker, save a named preset, wait for the DSP command to finish, and reload settings from storage. Also delete a preset and change DSP, and change an unrelated preference before another DSP command.

**Fix after verification:** Persist only the changed fields against current settings, or provide one authoritative serialized settings update path. Review other worker branches that save a cached whole settings object.

**Acceptance:** Preset creation and deletion survive subsequent DSP commands and restart. Unrelated preferences remain unchanged.

### Issue 04 CUE content identities collide across different recordings

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-core/src/cue.rs](crates/needle-core/src/cue.rs) lines 319 to 324; [crates/needle-core/src/sync.rs](crates/needle-core/src/sync.rs) lines 111 to 143; [crates/needle-core/src/analysis.rs](crates/needle-core/src/analysis.rs) lines 245 to 269.

A CUE track content_hash is derived only from audio file byte length, track number, and start offset. Distinct recordings with those same values receive the same hash. Sync trusts it for track association, and duplicate analysis can label the recordings exact duplicates without comparing their audio.

**Verify first:** Create two distinct disposable audio files of equal byte length with CUE sheets sharing track numbers and start offsets. Compare imported hashes, duplicate reports, and sync mapping for ratings, listens, and playlist entries.

**Fix after verification:** Derive identity from the actual recording plus the CUE span. Address migration of existing weak identities before trusting them in sync or duplicate analysis.

**Acceptance:** Distinct recordings do not share identity merely because their size and CUE layout match. Identical recordings still associate correctly.

### Issue 05 Failed organize undo discards the remaining undo record

**Priority:** P2. **Audit evidence:** Previously reproduced in temporary diagnostics.

**Source:** [crates/needle-core/src/doctor.rs](crates/needle-core/src/doctor.rs) lines 1017 to 1032.

undo_organize ignores failed reverse moves and unconditionally clears organize_undo. If an original destination is occupied or a file is temporarily inaccessible, the app loses the information needed to retry that move.

**Verify first:** Organize disposable files, then occupy one original path before Undo. Check the return value, file positions, can_undo_organize, and whether a second Undo works after clearing the obstruction.

**Fix after verification:** Retain failed moves, remove only completed moves from the record, and return actionable partial-failure details.

**Acceptance:** Undo remains available for failed moves; retry succeeds after the obstruction is removed without repeating already completed reversals.

### Issue 06 Filtered Android playlists edit the wrong stored position

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-mobile/src/more.rs](crates/needle-mobile/src/more.rs) lines 283 to 290; [crates/needle-mobile/src/more.rs](crates/needle-mobile/src/more.rs) lines 353 to 368; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Pages.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Pages.kt) lines 389 to 430; [crates/needle-core/src/database.rs](crates/needle-core/src/database.rs) lines 352 to 364.

playlist_detail hides missing tracks, and tracks_by_ids can omit nonexistent IDs. The UI sends positions in that filtered list, while remove_from_playlist and move_in_playlist use positions in raw track_ids. Stored [missing, A, B] displays [A, B], but removing visible A removes the hidden entry instead.

**Verify first:** Create playlists with missing and nonexistent IDs before, between, and after valid songs. Remove and reorder visible songs, including repeated occurrences of the same song.

**Fix after verification:** Carry the stored position or a stable playlist-entry identity through the UI and bridge. Preserve duplicate occurrences and ordering.

**Acceptance:** Only the selected visible occurrence is removed or moved, regardless of hidden entries.

### Issue 07 Organize treats sibling folders as part of a music root

**Priority:** P2. **Audit evidence:** Previously reproduced in temporary diagnostics.

**Source:** [crates/needle-core/src/doctor.rs](crates/needle-core/src/doctor.rs) lines 788 to 795.

Root selection compares lowercase string prefixes without a directory boundary. A configured C:\Music root therefore matches C:\MusicArchive\song.wav and can include that sibling file in an organize plan.

**Verify first:** Use disposable sibling directories Music and MusicArchive with tracks in both. Configure only Music as the root and inspect the plan before executing any moves.

**Fix after verification:** Use path-component containment with the intended platform case behavior rather than a raw text prefix.

**Acceptance:** Sibling directories are excluded; true descendants and roots with trailing separators continue to work.

### Issue 08 Reimport after tag edits erases measured loudness

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-core/src/analysis.rs](crates/needle-core/src/analysis.rs) lines 61 to 68; [crates/needle-core/src/scan.rs](crates/needle-core/src/scan.rs) lines 350 to 415; [crates/needle-core/src/scan.rs](crates/needle-core/src/scan.rs) lines 569 to 571.

Loudness analysis saves ReplayGain and peak values in the library rather than writing file tags. read_track rebuilds these fields from tags without preserving the previous database values. Editing an unrelated tag triggers import_one and can erase the measurements.

**Verify first:** Measure track and album loudness on files without ReplayGain tags. Edit only a title, reimport, and compare all stored gains and peaks. Distinguish a tag-only edit from an actual change to audio content.

**Fix after verification:** Preserve library measurements across metadata-only changes and explicitly invalidate them when the underlying audio changes.

**Acceptance:** Tag edits retain measured values. Changed audio does not retain stale analysis values.

### Issue 09 Unicode metadata can panic import normalization

**Priority:** P2. **Audit evidence:** Previously reproduced in temporary diagnostics.

**Source:** [crates/needle-core/src/import.rs](crates/needle-core/src/import.rs) lines 101 to 118.

normalize finds the feat or ft marker byte offset in a lowercased string, then uses that offset to slice the original string. Unicode case conversion can change byte length. The ordinary metadata value K feat. Guest reaches an invalid UTF-8 boundary.

**Verify first:** Call normalization and the actual import-matching path with K feat. Guest and other metadata containing Unicode characters whose lowercase representation changes length. Include ordinary ASCII feat and ft cases.

**Fix after verification:** Use offsets from the same string being sliced, or operate on character-aware spans.

**Acceptance:** Valid Unicode metadata never causes a slicing panic and continues to normalize consistently.

### Issue 10 Accepted DSF headers can panic during seek

**Priority:** P2. **Audit evidence:** Previously reproduced in temporary diagnostics.

**Source:** [crates/needle-core/src/formats.rs](crates/needle-core/src/formats.rs) lines 372 to 375; [crates/needle-core/src/formats.rs](crates/needle-core/src/formats.rs) lines 465 to 480.

DSF validation accepts any positive block size and DSD sample rate. A block size of 1 to 3 produces zero frames_per_group and division by zero in try_seek. Very small accepted rates can also produce a zero PCM sample rate.

**Verify first:** Use disposable malformed DSF fixtures with block sizes 1 to 3 and invalid low sample rates. Check both opening and seeking through the normal decoder API, alongside a valid DSF control.

**Fix after verification:** Validate supported layout, block-size, and rate constraints before exposing the decoder. Return an ordinary format error for unsupported headers.

**Acceptance:** Unsupported DSF data returns an error without panicking. Valid DSF playback and seeking remain correct.

### Issue 11 Short speaker discovery records panic the parser

**Priority:** P2. **Audit evidence:** Previously reproduced in temporary diagnostics; live speaker behavior unverified.

**Source:** [crates/needle-core/src/cast/discover.rs](crates/needle-core/src/cast/discover.rs) lines 129 to 152; [crates/needle-core/src/cast/discover.rs](crates/needle-core/src/cast/discover.rs) lines 14 to 15.

The type 33 service-record branch reads body[4] and body[5] without checking that the record contains those bytes. A truncated device reply panics parsing; the discovery join fallback can then discard that attempt's mDNS results.

**Verify first:** Feed truncated service records and valid control records to the existing parser in isolation. Check that valid records in a discovery attempt remain usable when another record is incomplete.

**Fix after verification:** Validate the service-record body before indexing, and handle incomplete records without discarding unrelated discovery results.

**Acceptance:** Short records do not panic. Valid Chromecast and AirPlay discovery records are still retained.

### Issue 12 Phone to computer handoff cannot match normal title and artist pairs

**Priority:** P2. **Audit evidence:** Source trace and SQLite behavior; independent end-to-end reproduction pending.

**Source:** [crates/needle-mobile/src/connect.rs](crates/needle-mobile/src/connect.rs) lines 274 to 301; [crates/needle-core/src/remote.rs](crates/needle-core/src/remote.rs) lines 275 to 282; [crates/needle-core/src/query.rs](crates/needle-core/src/query.rs) lines 239 to 249; [crates/needle-core/src/database.rs](crates/needle-core/src/database.rs) lines 80 to 87.

move_to_pc sends one plain-text search phrase made from title, a space, and artist. The query compiler quotes the entire phrase for the trigram index, whose stored text separates title and artist with a newline. A shared song such as title Harbor Lights and artist Mara Quinn normally does not match Harbor Lights Mara Quinn.

**Verify first:** Put the same locally sourced song on both devices and hand it from phone to computer. Compare the combined phrase with title-only search. Include a computer-origin streamed song, which uses the existing ID shortcut.

**Fix after verification:** Match title and artist as separate fields or use a shared recording identity. Avoid treating the concatenated phrase as one substring.

**Acceptance:** A matching local recording transfers successfully. The absent-song result is reserved for genuinely unmatched recordings.

### Issue 13 Computer to phone playback loses format metadata

**Priority:** P2. **Audit evidence:** Source trace; network startup timing not measured.

**Source:** [crates/needle-mobile/src/connect.rs](crates/needle-mobile/src/connect.rs) lines 69 to 77; [crates/needle-core/src/sources.rs](crates/needle-core/src/sources.rs) lines 444 to 448; [crates/needle-core/src/sources.rs](crates/needle-core/src/sources.rs) lines 948 to 997.

streamed constructs a Track with its identity and basic display fields but no format. An uncached computer song consequently gets an .audio cache extension and fails the progressive-streaming format check, waiting for the whole file before playback. Missing format can also bypass extension-selected decoders; verify that separately before asserting affected formats.

**Verify first:** Play or move an uncached, large MP3 or FLAC from computer to phone over a deliberately slow local connection. Inspect Track.format, cache extension, and whether playback begins before download completion. Separately exercise extension-selected formats supported by the application.

**Fix after verification:** Include and preserve the actual audio format in computer song responses and streamed tracks. Keep cache and decoder selection consistent.

**Acceptance:** Supported progressive formats start before complete download. The format remains correct through handoff, caching, and replay.

### Issue 14 CUE rescans retain obsolete or unavailable entries

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-core/src/cue.rs](crates/needle-core/src/cue.rs) lines 185 to 215; [crates/needle-core/src/scan.rs](crates/needle-core/src/scan.rs) lines 97 to 105; [crates/needle-core/src/model.rs](crates/needle-core/src/model.rs) lines 67 to 74.

CUE import updates current entries but does not reconcile removed ones. Missing referenced audio is skipped, while general missing-file checks inspect the CUE sheet path. If the sheet remains, old entries can stay available even after their audio or CUE entry disappears.

**Verify first:** Import a multi-track CUE sheet, remove one entry, and rescan. In a separate case, retain the sheet but remove its referenced audio. Also confirm that deleting the whole sheet is already handled.

**Fix after verification:** Reconcile the prior entries for each imported sheet and validate referenced audio availability. Preserve history while marking or retiring unavailable entries.

**Acceptance:** Removed CUE entries and missing referenced audio stop appearing playable after rescan. Existing valid entries retain identity and history.

### Issue 15 CUE playlist export cannot be imported again

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-core/src/database.rs](crates/needle-core/src/database.rs) lines 406 to 441.

Playlist export writes a CUE track's pseudo-path such as album.cue#1. Import canonicalizes the entire string as a filesystem path, which does not exist, and silently omits the entry. An all-CUE playlist then fails with no matching entries.

**Verify first:** Export and reimport an all-CUE playlist and a mixed ordinary-file/CUE playlist. Compare track identities, order, and counts.

**Fix after verification:** Define an explicit supported round-trip representation for CUE entries and recognize it during import. State any external M3U compatibility limits.

**Acceptance:** Supported exports reimport with their intended entries and ordering; unsupported entries produce an explicit explanation.

### Issue 16 Disabled scrobbling services starve enabled services

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-core/src/integrations.rs](crates/needle-core/src/integrations.rs) lines 832 to 863; [crates/needle-core/src/database.rs](crates/needle-core/src/database.rs) lines 382 to 390.

flush_scrobbles selects the oldest 100 eligible pending rows before checking service enablement or usable credentials. If those rows belong to a disabled service, every flush skips the same rows and never reaches newer pending rows for another enabled service.

**Verify first:** Prepare at least 100 older pending Last.fm rows, disable Last.fm, and add newer ListenBrainz rows with a test transport. Flush repeatedly and inspect which service is attempted. Check missing-credential and rejected-session cases too.

**Fix after verification:** Select eligible service rows before applying the batch limit, or independently process bounded batches per service.

**Acceptance:** A disabled or unavailable service cannot prevent another enabled service from sending its queued listens. Backlogs remain recoverable.

### Issue 17 New and imported Android themes overwrite existing files

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-mobile/src/media.rs](crates/needle-mobile/src/media.rs) lines 355 to 368; [crates/needle-mobile/src/tools.rs](crates/needle-mobile/src/tools.rs) lines 651 to 677.

Theme creation and import derive a filename slug from the name and write or copy without checking for a collision. Reusing a name, or names such as A B and A-B, can replace an existing custom theme even when creating a new theme.

**Verify first:** Create and import themes with identical names and distinct names that normalize to the same slug. Inspect both files and the selectable themes after each operation.

**Fix after verification:** Allocate a free ID for new themes and imports. Reserve replacement for an explicit edit of an existing theme.

**Acceptance:** New themes preserve existing themes; explicit edits update only the chosen theme.

### Issue 18 Android theme IDs collide across providers and built in names

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-mobile/src/media.rs](crates/needle-mobile/src/media.rs) lines 300 to 350; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Theme.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Theme.kt) lines 123 to 125.

Theme IDs are filename stems across both user and enabled-plugin folders. Two providers with the same stem share an ID and firstOrNull chooses only one. IDs night, midnight, and day are treated as built-ins, so custom themes with those IDs cannot apply their custom colors.

**Verify first:** Enable two test plugins each supplying dark.toml, then select both themes. Add user themes with reserved built-in IDs and inspect selection and editing behavior.

**Fix after verification:** Use identities that include provider scope, reserve built-in IDs, and keep selection, saving, and deletion consistent with ownership.

**Acceptance:** Every listed custom theme selects its own colors. User and plugin themes with equal basenames remain distinguishable.

### Issue 19 Editing the active Android theme leaves stale colors

**Priority:** P2. **Audit evidence:** Source trace; not reproduced on an Android device.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/ui/Theme.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Theme.kt) lines 120 to 125; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt) lines 755 to 760.

NeedleTheme remembers the loaded custom theme using only app.theme as its key. Saving changes to the already active theme sets that same ID, so the stored theme object is not reloaded and the new colors do not apply immediately.

**Verify first:** Activate a custom theme, edit a conspicuous color, choose Save and use it, and inspect the result without switching themes or restarting.

**Fix after verification:** Publish a theme-content version or observable theme value, or invalidate the cache after saving or importing the active theme.

**Acceptance:** Changes to the active theme apply immediately and persist across restart.

### Issue 20 Android tool failures leave controls busy

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt) lines 107 to 112; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt) lines 476 to 485; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt) lines 595 to 606.

The shared work helper invokes done only on success. Callers set busy before starting and clear it only in done, so an exception displays a message but leaves actions such as cover lookup and tidying disabled.

**Verify first:** Trigger a failed cover lookup and failed tidy on disposable data. Stay on the same screen, remove the failure condition, and attempt the action again. Review other busy-setting callers.

**Fix after verification:** Reset operation state on every completion path, including failure and cancellation, while preserving an actionable error message.

**Acceptance:** Failed operations stop showing busy and can be retried without reopening the screen.

### Issue 21 Android loading errors remain indistinguishable from loading

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/ui/Common.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Common.kt) lines 85 to 90; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Pages.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Pages.kt) lines 414.

rememberLoaded maps exceptions to null, the same value used for an in-progress load. A failed detail or library query can therefore leave placeholders indefinitely, with no error or direct retry.

**Verify first:** Make a library/detail load throw through a test seam or disposable invalid reference. Observe the page after the worker finishes and verify whether recovery requires an unrelated library change.

**Fix after verification:** Represent loading, success, empty, and failure separately and provide a retry that reruns the actual load.

**Acceptance:** A finished failed load displays an error and allows recovery; loading placeholders are limited to work still in progress.

### Issue 22 Android USB mixer controls lack the required permission

**Priority:** P2. **Audit evidence:** Source trace and Android lint diagnostic; no USB device reproduction.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/ui/Usb.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Usb.kt) lines 89 to 98; [apps/android/app/src/main/AndroidManifest.xml](apps/android/app/src/main/AndroidManifest.xml) lines 1 to 27.

The manifest does not declare android.permission.MODIFY_AUDIO_SETTINGS, which the mixer-setting and clearing calls require. The failure is caught and converted to false, so the USB toggle can fail silently instead of enabling the requested mode.

**Verify first:** Rerun lint, inspect the merged manifest, and exercise both enabling and clearing preferred mixer attributes on API 34 or later with a compatible USB device. Keep permission failure distinct from device capability failure.

**Fix after verification:** Declare the required permission and report actual mixer-operation failures. Do not claim device support or audio-path certification from a passing lint result alone.

**Acceptance:** Lint no longer reports this omission, and supported-device operations succeed or display an accurate failure reason.

### Issue 23 Android 8 and 9 shared music editing lacks write permission

**Priority:** P2. **Audit evidence:** Source trace and Android storage documentation; untested on legacy devices.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/MainActivity.kt](apps/android/app/src/main/java/fyi/nnx/needle/MainActivity.kt) lines 118 to 128; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt) lines 116 to 119; [apps/android/app/src/main/AndroidManifest.xml](apps/android/app/src/main/AndroidManifest.xml) lines 1 to 27.

Below API 33, the app requests READ_EXTERNAL_STORAGE but does not declare or request WRITE_EXTERNAL_STORAGE. canChangeFiles assumes access below API 30. Editing existing shared-storage music on Android 8 or 9 therefore lacks the required write authorization even though the UI gate permits the operation.

**Verify first:** On API 26 and 28, try reading, editing tags, and tidying a disposable shared-storage music file after granting read access. Compare app-private files, which have different access rules. Inspect denial and retry handling.

**Fix after verification:** Declare and request the appropriate legacy write permission only on applicable Android versions, and make the editing gate reflect actual access.

**Acceptance:** Supported legacy shared-storage edits work after the required grant. Denial leaves the original file intact and explains how to proceed.

### Issue 24 Android tidying discards partial failure details

**Priority:** P2. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-mobile/src/tools.rs](crates/needle-mobile/src/tools.rs) lines 394 to 402.

tidy returns an error only when there are failures and no successful moves. If some moves succeed, it returns their count and discards every remaining failure, so the UI reports success without explaining which files did not move.

**Verify first:** Create a disposable plan with one valid move and one blocked destination. Run tidy and compare its result and UI message with actual file locations and the underlying organize report.

**Fix after verification:** Return structured successful and failed outcomes and show partial completion with enough detail to retry failed moves.

**Acceptance:** Users can identify every failed move after partial completion, and successful moves remain correctly represented in Undo.

### Issue 25 Android file picker copies can block the UI thread

**Priority:** P3. **Audit evidence:** Source trace; latency and application responsiveness not measured.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/ui/Overlays.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Overlays.kt) lines 146 to 152; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Tools.kt) lines 624 to 626; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Settings.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Settings.kt) lines 654 to 657.

copyToCache synchronously opens and copies the entire selected content stream. File-picker result callbacks call it on the UI thread. A large file or slow provider can stall interaction before the later background import starts.

**Verify first:** Select a large disposable import from a slow test content provider while recording main-thread work and UI responsiveness. Inspect every caller, including playlist pictures and theme imports.

**Fix after verification:** Move provider reads and copying to an IO dispatcher, expose progress or cancellation where useful, and publish the completed path back to the UI.

**Acceptance:** The interface remains responsive during a slow copy, and cancellation or failure does not leave a usable-looking partial import.

### Issue 26 Mobile search and media browsing materialize full results before limiting

**Priority:** P3. **Audit evidence:** Source trace; large-library latency and memory not measured.

**Source:** [crates/needle-mobile/src/lib.rs](crates/needle-mobile/src/lib.rs) lines 516 to 524; [apps/android/app/src/main/java/fyi/nnx/needle/PlaybackService.kt](apps/android/app/src/main/java/fyi/nnx/needle/PlaybackService.kt) lines 93 to 112.

Mobile search calls the unpaged library search before filtering missing tracks and taking 300. Media-service browsing queries and maps full collections before slicing the requested page. Work and allocations scale with the entire matching library instead of the requested result count.

**Verify first:** Profile broad search and small media-browser page requests against a disposable large library. Measure returned row count, queried/deserialized rows, allocation, and execution thread. Check missing-entry handling and ordering.

**Fix after verification:** Push bounds and pagination into the data layer while preserving query semantics and filling pages consistently. Avoid synchronous large queries on the service's main looper.

**Acceptance:** Small result requests perform bounded work and preserve correct order and page boundaries as library size grows.

### Issue 27 Album and artist browsing ignore per group query limits

**Priority:** P3. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle-core/src/browse.rs](crates/needle-core/src/browse.rs) lines 84 to 94; [crates/needle-core/src/query.rs](crates/needle-core/src/query.rs) lines 29.

browse.filtered uses the compiled SQL, order, and global limit but ignores the per-group limit. A query such as limit 1 per artist can therefore produce album/artist summaries based on a different song set from the song view.

**Verify first:** Create multiple tracks per artist and album. Compare song results with album and artist summaries for limit 1 per artist and per album, including an additional global limit.

**Fix after verification:** Share the complete query result-shaping logic between song search and browse summaries, including grouping limits and other supported modifiers.

**Acceptance:** Summary counts and contents reflect the same selected track set as the equivalent song query.

### Issue 28 Desktop cover thumbnails do not refresh when the source changes

**Priority:** P3. **Audit evidence:** Source trace; independent reproduction pending.

**Source:** [crates/needle/src/ui/thumbs.rs](crates/needle/src/ui/thumbs.rs) lines 78 to 100.

The in-memory cache key is only path and edge size. A Ready entry returns before file metadata is checked, so replacing a large cover at the same path keeps returning the old generated thumbnail until the cache is reset.

**Verify first:** Display a cover large enough to generate a thumbnail, replace its contents at the same path with a visibly different image and changed modification time, then redraw without restarting.

**Fix after verification:** Invalidate relevant entries when artwork changes or include source version information in in-memory cache lookup. Review failure and Making entries for stale-cache recovery.

**Acceptance:** Same-path artwork replacement displays the new thumbnail during the same application session.

### Issue 29 Some Android song lists do not observe playback changes

**Priority:** P3. **Audit evidence:** Source trace and Android lint diagnostics; device UI behavior unverified.

**Source:** [apps/android/app/src/main/java/fyi/nnx/needle/ui/Listening.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Listening.kt) lines 76 to 77; [apps/android/app/src/main/java/fyi/nnx/needle/ui/Listening.kt](apps/android/app/src/main/java/fyi/nnx/needle/ui/Listening.kt) lines 96 to 101.

TitledSongs and FoldersScreen read playback.value during composition without collecting the StateFlow. Playing-song highlighting in favorites, recent lists, and folders can stay unchanged until an unrelated recomposition.

**Verify first:** Remain on each affected song list while playback advances or another song is selected from outside that screen. Observe the highlighted row without navigation or a library refresh.

**Fix after verification:** Collect playback as lifecycle-aware Compose state and derive the playing ID from that observable value.

**Acceptance:** The playing indicator follows playback immediately on all affected screens, and the relevant lint errors are resolved.

## Check results and failing check categories

These are results from the audit run, not a claim about checks after any future fixes.

| Check | Recorded audit result | What Opus should verify |
| --- | --- | --- |
| `cargo test --workspace --locked` | Passed: 235 test executions and 10 ignored. The count includes the same 18 desktop tests run in two binaries, 198 core tests, and 1 mobile test. | Rerun on the final changes; inspect ignored coverage separately. |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed. Cargo also printed a dependency future-compatibility notice for proc-macro-error2 2.0.1. | Rerun; keep that notice separate from a Clippy failure. |
| `cargo fmt --all --check` | Failed. | Check category 01 below. |
| `cargo test -p needle-core --locked opus::tests:: -- --ignored --nocapture` | Passed both existing FFmpeg-based Opus accuracy and speed tests. | Rerun if decoder or related format handling changes. FFmpeg must be available. |
| Trailer `npm.cmd run lint` | Failed with three hook-rule errors. | Check category 02 below. |
| Trailer `npx.cmd tsc --noEmit` | Passed when run independently of lint. | Rerun if trailer code changes; lint's failure prevents its chained TypeScript command from running. |
| Android `gradlew.bat :app:lintDebug :app:testDebugUnitTest --console=plain` | Lint failed with 11 errors, 31 warnings, and 10 hints. The unit-test task reported NO-SOURCE. | Check category 03 below. NO-SOURCE is not a passing Android test suite. |

### Check category 01 Rust formatting blocks the standard Windows build

**Evidence:** The existing formatting check failed in `crates/needle-core/src/sources.rs` and multiple mobile files, including `lib.rs`, `settings.rs`, and `tools.rs`. [scripts/build-windows.ps1](scripts/build-windows.ps1) lines 9 to 11 runs this check before continuing the normal build.

**Verify first:** Rerun `cargo fmt --all --check` and confirm the exact current diff. Do not infer a compiler or runtime failure from formatting alone.

**Fix after verification:** Apply the repository formatter after functional fixes and review that its changes are formatting-only.

**Acceptance:** Formatting passes, and the standard Windows build can proceed beyond its formatting gate without `SkipChecks`.

### Check category 02 Trailer lint rejects hooks inside callbacks

**Evidence:** [apps/trailer/src/scenes.tsx](apps/trailer/src/scenes.tsx) lines 54, 101, and 159 call `useRise` inside callbacks. The existing lint command reports three `react-hooks/rules-of-hooks` errors. The affected scene code was not established as a crashing path in the active final composition.

**Verify first:** Run `npm.cmd run lint` from `apps/trailer`; inspect how the scenes are used and whether list length or ordering can change hook invocation.

**Fix after verification:** Move hook calls into appropriate components or top-level hook structure, preserving the intended animation.

**Acceptance:** Trailer lint and TypeScript checks pass. Render or preview affected scenes if their animation structure changes.

### Check category 03 Android lint fails and no unit tests are defined

**Evidence:** The audit run reported 11 errors, 31 warnings, and 10 hints. Findings 22 and 29 identify concrete permission/state observations tied to the lint output. Other lint errors require independent triage; generated Cleaner code with guarded fallback and a fragment-version diagnostic are not by themselves proof of a runtime failure.

**Verify first:** Rerun lint with the current JDK and Android SDK. Inspect every current error in [the generated lint report](apps/android/app/build/reports/lint-results-debug.html), including the merged manifest and generated code where relevant. The report is generated and ignored by Git, so it may be absent on another checkout.

**Fix after verification:** Correct verified application or build configuration errors. For false positives, document why they do not apply and use the narrowest justified handling rather than broadly hiding diagnostics.

**Acceptance:** Android lint passes or explicitly documented unresolved diagnostics remain visible. Run meaningful tests for changed mobile behavior; a NO-SOURCE test task is not behavioral validation.

## Reproduction environment and coverage limits

The audit ran on Windows with Rust, Node, a JDK, the Android SDK, and FFmpeg available. Rust checks used the locked dependency set. In this workspace, Cargo was under `C:\Users\amber\.cargo\bin`, the JDK under `C:\Users\amber\tools\android\jdk`, the SDK under `C:\Users\amber\tools\android\sdk`, and FFmpeg under `C:\ffmpeg\bin`. Resolve the actual tool locations on Opus's host instead of assuming these paths.

The existing checks generated logs named `needle-audit-tests.log`, `needle-audit-clippy.log`, `needle-audit-format.log`, `needle-audit-trailer-lint.log`, `needle-audit-trailer-types.log`, `needle-audit-android.log`, and `needle-audit-opus.log` in the audit host's temporary directory. They are optional local evidence and are not committed artifacts. Do not depend on them being available on another machine.

The review covered Rust core, desktop, mobile bridge, Android, website, trailer, packaging, and relevant local vendor patches. It did not review every upstream vendor line. Android device behavior, actual USB hardware, live casting/audio output, external service sessions, and Linux/macOS releases were not verified end to end. After separately running the two ignored Opus tests, eight default-suite ignored tests still required their own runtime conditions. Passing the existing suite does not prove the findings absent or the codebase free of other issues.

Useful primary references:

- [Android shared media storage requirements](https://developer.android.com/training/data-storage/shared/media), particularly legacy-device write access for finding 23.
- [SQLite FTS5 trigram tokenizer](https://www.sqlite.org/fts5.html#the_trigram_tokenizer), for the matching behavior underlying finding 12.

## Opus verification and fix tracking

Update each row with evidence rather than checking it off solely because a code change was made. Record the precise test, reproduction, or profiler result and the relevant fix reference. A rejected finding needs an explanation; an unavailable device needs an explicit outstanding requirement.

| Item | Priority | Opus verification | Fix and validation |
| --- | --- | --- | --- |
| Issue 01 | P1 | Verified: disposable organize collision overwrites source/destination lyrics (core-findings-before.log). | Fixed in [doctor.rs](crates/needle-core/src/doctor.rs): preflight companion destinations, no-replace moves, explicit companion journal records. `organize_preserves_colliding_lyrics_and_undo_retries_blocked_files` passes on Windows/Linux. |
| Issue 02 | P1 | Verified by source: DISPLAY_NAME is reused and outputStream truncates the existing copy. | Fixed in [FileCopies.kt](apps/android/app/src/main/java/fyi/nnx/needle/FileCopies.kt) and MainActivity: unique completed copies, temporary writes, failure cleanup. JVM collision/failure tests pass. `OpenWithTest.identicalProviderNamesKeepBothAudioFilesAndLibraryIdentities` passes on API 36 with a real provider returning two different recordings named song.wav. |
| Issue 03 | P1 | Verified: audio::tests::dsp_commands_preserve_new_presets_and_unrelated_preferences loses the saved preset. | Fixed in database.rs/audio.rs/settings.rs: transactional field updates, including volume, output selection, configuration, and DSP. The regression proves newly saved/deleted presets and unrelated preferences survive worker commands and storage reload. |
| Issue 04 | P2 | Verified: equal-sized 440 Hz/880 Hz CUE recordings share hashes; sync and analysis trust them by source inspection. | Fixed in cue.rs/sync.rs/analysis.rs: recording hash plus span, versioned cue2 identity; weak identities migrate before association or remain excluded when unavailable. `cue_identity_distinguishes_equal_size_recordings_and_matches_copies` and `cue_sync_migrates_weak_identities_before_associating_history_and_playlists` pass, including ratings, listens, playlists, and duplicates. |
| Issue 05 | P2 | Verified: failed_organize_undo_keeps_a_retry_record loses the blocked reverse move. | Fixed in doctor.rs: retain failed reversals, report path/reason, retire successful records only; record intent before moving and retain prior unresolved journals. Both undo regressions pass, including successful retry after removing the obstruction. |
| Issue 06 | P2 | Verified by source: filtered playlist_detail indices are passed to unfiltered track_ids. | Fixed in mobile more.rs and Android Pages.kt: visible occurrences carry original stored positions; remove/move uses those positions. `visible_playlist_occurrences_and_theme_ownership_are_stable` covers missing/nonexistent IDs and duplicate occurrences. |
| Issue 07 | P2 | Verified: organize_does_not_include_sibling_roots includes MusicArchive under Music. | Fixed in doctor.rs: component containment, platform-appropriate case handling and collision keys. Sibling-root regression passes on both platforms. |
| Issue 08 | P2 | Verified: metadata_edits_keep_measured_loudness_but_changed_audio_invalidates_it loses four measured values after title edit. | Fixed in analysis.rs/model.rs/scan.rs: decoded-audio provenance protects library measurements, verified tag edits initialize legacy provenance, changed audio invalidates it. Regression retains all four values through metadata changes and clears stale values after replacing samples. |
| Issue 09 | P2 | Verified: unicode_credit_markers_normalize_without_panicking panics on Kelvin-sign metadata. | Fixed in import.rs: found offset and slicing use the same lowercased string. Unicode Kelvin/İ and ASCII credit-marker cases pass. |
| Issue 10 | P2 | Verified: malformed_dsf_layouts_are_rejected_before_seeking accepts block size 1; division-by-zero seek path substantiated by source. | Fixed in formats.rs: validate full-width channels, rates, format bounds, and supported block sizes before construction. Malformed fixtures return errors; existing valid DSF decode/seek test passes. |
| Issue 11 | P2 | Verified: cast::discover::tests::short_service_records_do_not_discard_valid_records panics on empty SRV body. | Fixed in cast/discover.rs: bounded SRV/PTR/TXT parsing, skip an invalid record individually. Regression retains the valid speaker beside malformed records. Live speaker discovery remains a hardware check. |
| Issue 12 | P2 | Verified by source: move_to_pc concatenates title/artist, while indexed text separates them by newline. | Fixed in mobile connect.rs: quoted title/artist field query and refusal to transfer an unmatched current song. `the_phone_connects_to_a_computer` verifies matching through a real local HTTP server. |
| Issue 13 | P2 | Verified by source: remote search/state and PcSong/streamed omit format. | Fixed in core remote.rs/mobile connect.rs: carry format and song metadata through search/current/queue into streamed Track. The local HTTP regression asserts FLAC survives parsing and streaming conversion. |
| Issue 14 | P2 | Verified: cue_rescan_retires_removed_entries_and_unavailable_audio retains removed CUE entry. | Fixed in cue.rs: reconcile unseen sheet entries, including references to unavailable audio, while preserving retained IDs/history. Removed-entry and missing-recording regression passes. |
| Issue 15 | P2 | Verified: cue_playlists_round_trip_including_repeated_entries fails with no matching entries. | Fixed in database.rs: resolve the sheet separately from the CUE track suffix; validate unavailable sheet/track and retain repetitions. Round-trip regression passes; Needle's sheet.cue#number compatibility is documented in README.md. |
| Issue 16 | P2 | Verified: disabled_scrobble_backlogs_do_not_starve_enabled_services leaves enabled-service row pending behind 100 disabled rows. | Fixed in integrations.rs: select eligible configured/non-rejected service rows before the batch limit. Regression proves an enabled row advances behind 100 disabled rows, without network submissions. |
| Issue 17 | P2 | Verified by source: save_theme and import_theme replace the slug destination. | Fixed in mobile tools.rs: fresh unique theme identities, exclusive publication, validated explicit owned-theme edits. Regression verifies same-name creates/imports preserve earlier contents; also corrected full-document TOML parsing. |
| Issue 18 | P2 | Verified by source: theme IDs use only basename; built-in names bypass custom lookup. | Fixed in tools.rs/settings.rs and theme editor: scoped custom IDs, plugin namespace/read-only status, legacy selection migration, edit plugin themes as copies. Regression covers built-in names, two plugins with identical filenames, explicit edits, and rejected plugin/path identities. |
| Issue 19 | P2 | Verified by source: custom theme cache is keyed only by ID. | Fixed in NeedleApp.kt/Theme.kt/Overlays.kt: observable content revision invalidates active custom-theme lookup after save/import/delete. Android compiles/lints. End-to-end editor repaint remains a manual UI check. |
| Issue 20 | P2 | Verified by source: work calls done only on success and busy callers reset only in done. | Fixed in Operations.kt/Tools.kt: separate successful result and unconditional completion in finally; busy/saving callers use completion. `operationsFinishAfterSuccessFailureAndCancellation` passes; cancellation propagates. |
| Issue 21 | P2 | Verified by source: rememberLoaded converts exceptions to loading null and supplies no retry. | Fixed in LoadResult.kt/Common.kt and callers: Loading/Ready/Failed, successful empty/null values, explicit error/Retry; skeletons depend on loading state. `loadsDistinguishEmptyNullAndFailureAndCanRetry` passes. |
| Issue 22 | P2 | Verified by lint and AudioManager contract: MODIFY_AUDIO_SETTINGS missing. | Fixed manifest/Usb.kt: normal permission, success/error return for set and clear, one row/switch handler, connected-device callbacks. Lint passes. **Needs environment:** API 34+ physical USB DAC, plug/unplug and accepted/rejected mixer configuration. |
| Issue 23 | P2 | Verified by source and shared-media contract: legacy write permission missing and editing gate accepts all API <30. | Fixed manifest/Tools.kt: request actual legacy grant on API 26–28, handle denial, gate scoped API 29 and all-files API 30+, allow app-owned songs (including canonical-path aliases). Rust private/shared path regression passes; API 36 provider integration verifies private copies. **Needs environment:** API 26/28 shared-storage grant/denial and tag/restore, plus API 29 shared-storage UI. |
| Issue 24 | P2 | Verified by source: tidy discards failures whenever any move succeeds. | Fixed mobile tools.rs/Android Tools.kt: structured moved count plus every failed path/reason, retry and undo remain visible. Mobile regression combines one success and one lyric collision, preserves the existing lyric, and successfully undoes the completed move. |
| Issue 25 | P3 | Verified UI-thread provider-copy call paths. | Fixed FileCopies.kt/Tools.kt/Extras.kt: provider queries and atomic copies on IO, cancellation cleanup, picked-file lifetime through consumers. Five JVM regressions include a 300 ms slow provider with the caller yielding in about 5 ms; API 36 provider import also passes. Physical-provider/frame-time profiling is not claimed. |
| Issue 26 | P3 | Verified unbounded query/map paths in mobile search and media browser. | Fixed database.rs/browse.rs/mobile bridge/PlaybackService.kt: bounded result hydration/pages, count queries, background service executor; also implements media search. Paging regression covers order, missing IDs, duplicates, Unicode album sorting, global/per-group limits. 50,000-song fixture: same 300 results, 696.158 ms full hydration versus 6.650 ms bounded (about 105×). Global shuffle-by-group rules retain full-set spreading for semantic correctness. |
| Issue 27 | P3 | Verified: browse_summaries_use_the_same_per_group_limits_as_song_search counts 8 where song search selects 2. | Fixed browse.rs: use compiled source/filter with full group/global result shaping. Per-artist/per-album/global-limit regression passes. |
| Issue 28 | P3 | Verified: same-path replacement returns the old thumbnail (thumbnails-before.log). | Fixed thumbs.rs: metadata-versioned Ready/Making/Failed entries, reject stale worker results, recover failed/deleted copies. Existing thumbnail regression now verifies changed image bytes without restarting and regeneration after deleting the cached copy. |
| Issue 29 | P3 | Verified by source and two lint errors: affected lists read StateFlow.value without collection. | Fixed Listening.kt: lifecycle-aware playback collection in titled lists and folders. Both lint errors cleared; live highlight behavior remains a manual UI check. |
| Check category 01 | Build gate | Verified: cargo fmt fails in sources.rs and mobile files (format-baseline.log). | Fixed with cargo fmt; cargo fmt --all --check passes. sources.rs changes are formatting-only. |
| Check category 02 | Lint gate | Verified: three rules-of-hooks failures (trailer-lint-baseline.log). | Fixed scenes.tsx/ui.tsx: pure frame-based rise helper inside map callbacks; hooks remain at component top level. npm run lint (including TypeScript) passes. Colour/Player/Tour frame-30 renders are pixel-identical to baseline. |
| Check category 03 | Lint gate | Verified: 11 Android lint errors and NO-SOURCE tests (android-baseline.log). | Fixed application/configuration errors: UniFFI Android Cleaner generation, manifest permissions/search intent, AndroidX Media3 opt-in, unused constraints/Flow collection, and actual QR-scanner Fragment 1.0.0 dependency upgraded to 1.9.1. No blanket lint baseline/suppression. Final lint: zero errors, 30 warnings, 11 hints; five JVM tests and real-provider emulator test pass. |

## Completion criteria

- Every finding and failing check category has an explicit verification outcome.
- Confirmed data-loss issues have regression coverage proving that original files and saved data survive failure paths.
- Confirmed fixes meet their acceptance criteria and pass relevant checks.
- Rejected findings include enough evidence for another reviewer to assess the rejection.
- Device, platform, or service checks that could not run remain listed with the required environment.
- The final handoff states what was fixed, what was rejected, what remains open, and the actual validation results.

## Verification pass on 2026-10-06

All 29 findings and three failing check categories were inspected before application fixes. Baseline workspace tests pass (239 executions, 10 ignored). Fresh disposable-file/database regressions above fail on the audited behavior. Source traces are explicitly distinguished from runtime/device reproduction. Logs are under `artifacts/audit-verification/` (ignored build artifacts). Android API 36 emulator image is installed, but no device or USB DAC is attached; legacy shared-storage and hardware checks remain required. Performance severities remain provisional pending measurement. No findings are rejected wholesale; generated Cleaner and fragment lint diagnostics need narrow build triage rather than being treated as established runtime crashes.

## Final implementation and validation on 2026-10-06

The fixes are local changes against baseline `d53bfc4858b80c22adc2477a4dcf0267feb9d0a8`; no fix commit has been created. All file/database regressions used disposable libraries and media. The Android device test ran on the isolated `needle-audit-api36` emulator with rebuilt arm64-v8a/x86_64 native libraries. No real user library was used.

| Final check | Result and evidence |
| --- | --- |
| Windows `cargo test --workspace --locked` | 256 test executions pass; 10 pre-existing ignored tests plus the explicit performance test remain ignored in the default run. `rust-final.log`. |
| Linux/Ubuntu 24.04 `cargo test --workspace --locked` | 253 test executions pass; platform-specific test counts differ. Eight pre-existing ignored tests plus the performance test remain ignored. `linux-tests.log`. |
| Windows/Linux Clippy, all workspace targets, locked, `-D warnings` | Pass. `clippy-final.log`, `linux-clippy.log`. The existing proc-macro-error2 future-compatibility notice is not a Clippy failure. |
| Rust formatting | Pass. `format-final.log`. |
| Two ignored Opus decoder accuracy/speed checks | Pass against FFmpeg; 60 seconds of stereo decoded at about 314× real time. `opus-final.log`. |
| Explicit 50,000-track search measurement | Pass, identical 300-result IDs/order; 696.158 ms old full hydration, 6.650 ms bounded query. `search-performance.log`. This measures query time and result hydration, not whole-app frame time or total allocation. |
| Trailer lint and TypeScript | Pass. `trailer-final.log`. Colour, Player, and Tour renders at frame 30 are pixel-identical to baseline; rendered PNGs remain in the artifact folder. |
| Android lint/JVM tests/build | Pass; zero lint errors, 30 warnings, 11 hints; five behavioral JVM tests, not NO-SOURCE. `android-device-final.log` and generated lint/JUnit reports. |
| Android API 36 provider integration | Pass: a separate real content provider rejects DATA-path queries and supplies two distinct 440/880 Hz recordings with DISPLAY_NAME song.wav; MainActivity fallback produces unique complete copies, exact original bytes, distinct library IDs, and app-private editing eligibility. `OpenWithTest` and generated connected-test report. |
| Native Android release libraries | Both ABIs build successfully. `android-native-final.log`; the APK's libraries were checked against the final built libraries. |
| Whitespace/diff checks | `git diff --check` passes. No trailer diagnostic source remains. |

One existing timing-sensitive fake-output repeat test failed during a busy parallel run, then passed independently and in the final full workspace run. No playback-code change was made to conceal that failure. The final logs report actual clean results.

Outstanding validation is explicit rather than a claim of device-wide coverage:

- **Issue 22:** An API 34+ physical USB DAC is required to verify supported/rejected mixer preferences, both toggle controls, and live unplug/reconnect. Permission and error-state handling are implemented and lint-clean.
- **Issue 23:** API 26/28 grant, denial, tag edit, backup and restore on shared storage; API 29 scoped-storage explanation and app-owned editing. API 36 private-provider copies are exercised, and the shared/private and path-alias regression passes on Windows/Linux.
- **Issues 19 and 29:** Manually edit the currently selected theme and observe its repaint, and keep favorites/recent/folder lists open while playback changes. Their observable state paths compile and the corresponding lint errors are resolved; these UI journeys were not scripted.
- **Issues 11–13 and 26:** Live speakers, physical phone-to-PC audio handoff, service-client media browsing/search, and frame/allocation profiling with an actual large mobile library remain useful end-to-end checks. Parser, HTTP protocol, query semantics, bounded hydration, background service dispatch, and query timing have automated evidence.
- Linux Rust tests/Clippy ran in a container; the desktop cover-click border regression runs on both platforms, but a full Linux graphical session and macOS release were not exercised.

The Fragment version fix corrects the scanner's actual transitive dependency rather than suppressing InvalidFragmentVersion. Android test dependencies use the stable versions listed in the [AndroidX Test release notes](https://developer.android.com/jetpack/androidx/releases/test). Build warnings and hardware limits above remain visible; no blanket lint baseline was added.
