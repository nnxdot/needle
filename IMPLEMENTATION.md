# Implementation status — 0.1.0

This repository now contains a runnable native music player, its headless Rust core, command-line tools, and a Windows packaging script. It is a **Windows preview**, not completion of every release in the original 17.75-engineer-month proposal.

| Proposal area | Current implementation | Remaining work |
|---|---|---|
| Native UI | GPUI; library, album/artist views, details, queue, history, settings; dark/light themes; virtual rows and 1,000-track pages | macOS/Linux validation, comprehensive accessibility, responsive/device coverage, richer layout customization |
| Expression language | Typed predicates, boolean logic, time functions, ordering, limits, shuffle, errors; shared by search, smart playlists, autoplay | Autocomplete; constrained shuffle; similarity; grouped analytics; one unified formatting/pipeline language |
| Playback | Shared output, preloaded queue, gapless-enabled decoding, seek, repeat, A–B loop, volume, visible path, saved paused session | DSD, ASIO, sample-accurate conformance across all codecs and transitions, device-loss/reconnect coverage |
| Exclusive output | Windows WASAPI; native rate/channel count, 24-bit integer containers; bypasses gain/volume | CoreAudio hog mode, ALSA/PipeWire exclusive modes, device matrix, external DAC/loopback bit-perfect certification |
| DSP | EBU R128 integrated loudness/true peak and stored ReplayGain normalization | Album gain, gain-tag export, convolution, per-device DSP profiles, pitch-preserving time stretch |
| Library | SQLite WAL, indexed/FTS search, incremental scans, watched folders, hash-based moved-file identity, missing-file detection | Rename-plus-retag identity inference, multi-root edge cases, deeper scale/memory work |
| Tagging | Reviewed individual edits; batch CLI preview/apply; original backups; decoded-audio preservation check; Picard-compatible recording ID | Multi-selection tag editor, arbitrary native tag fields, undo UI, exhaustive tag/format matrix |
| Metadata lookup | Opt-in cached MusicBrainz recording search; local Chromaprint; opt-in AcoustID lookup with a key kept in Windows Credential Manager; Cover Art Archive cache | Dedicated release/artist explorers, production service registration/terms |
| Scrobbling | Last.fm (desktop token/browser approval) and ListenBrainz (token validation) sign-in from the CLI and core API; secrets in Windows Credential Manager with environment overrides; persistent queue; chronological retries/backoff; permanent failures and rejected sessions classified; queue summary | Sign-in UI in the desktop app; sign-in and submission not yet exercised against the live services; secure storage on macOS/Linux |
| History | Local listens, qualification, ratings/play counts, `played(...)` queries, history view | Grouped charts/analytics and full history browsing in UI (currently latest 200) |
| Sync | Authenticated encrypted manual bundle export/import; hash matching; deduplicated listens; atomic database import | CRDTs, automatic peer discovery/relay, conflict UI, playback-position and mobile sync |
| Layouts | Validated JSON geometry presets; import/export in UI and CLI | Arbitrary component arrangements and columns |
| Stems | Not implemented | Local inference/model packaging, stem controls, separation caching and model licensing |
| Mobile/plugins/licensing | Not implemented | Mobile client, plugin API, purchase/license system, commercial release operations |

## Boundaries that matter

- The executable supports local music only. There are no mock service responses, simulated playback controls, or placeholder buttons presented as implemented features.
- A large library is searched as a whole, with at most 500,000 results. Playback and static playlist creation from the UI currently use the shown page. The interface says “Play this page” for paged collections.
- A moved file retains identity when its full hash matches a record whose previous path is absent. Simultaneously moving and changing tags changes that hash; recognition is not guaranteed.
- Sync is an explicit file transfer. Nonzero local ratings take precedence, playlist timestamps choose the newer version, and whole-file hashes determine matches. It is not a CRDT, relay service, or background synchronization product.
- The Windows binary has been built and exercised on this machine. Other platforms and all-platform bit-perfect playback are unverified. The GPUI accessibility tree on this Windows backend does not yet expose the application’s full control hierarchy.
- External API clients compile and follow their documented request shapes, but account-based sign-in and submission were not exercised without user credentials. Response parsing, error classification, request signing, credential precedence, and redaction are unit-tested against captured response shapes; Credential Manager save/read/remove was exercised locally with a dummy value. Production service agreements and application registrations are not supplied by this repository.

See [VALIDATION.md](VALIDATION.md) for specific evidence, rather than interpreting roadmap requirements as completed tests.
