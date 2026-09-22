# Needle — Project Proposal

*Working title. Alternatives: Playhead, Tonearm, Cartridge, Gapless.*

**A modern library-first music player.** The power of foobar2000, the feel of
software made this decade, on all three desktop platforms.

- **Studio:** nnx
- **Status:** Proposal
- **Estimated effort:** ~17.75 engineer-months total, ~10 to a shippable v1
- **Revenue model:** One-time license + optional sync subscription

---

## 1. Summary

Serious music listeners — people with large local libraries, DAC owners, DJs,
musicians, archivists — are served by two kinds of software, and neither one is
good.

The powerful players (foobar2000, MusicBee) are Windows-first, visually stuck in
2009, and configured through mazes of nested dialogs. The pleasant players
(Plexamp, Roon, Apple Music) hide the library behind an opaque interface, require
a server or an appliance, and can't express anything more specific than a
checkbox.

Needle occupies the empty quadrant: a fast native application that treats the
library as a queryable database and the playback path as something to get
provably correct, with an interface someone would choose to look at.

The core differentiator is a single well-designed expression language that
handles display formatting, library search, playlist rules, and playback
behaviour — replacing the three separate mini-languages foobar2000 accumulated
over twenty years.

---

## 2. Problem

**foobar2000** — the reference point. Gets right: speed at 500k+ tracks,
bit-perfect exclusive output, true gapless, batch tagging, a component model, and
a library that is genuinely queryable. Gets wrong:

- Windows-first; the macOS build is a shadow of it, Linux has nothing comparable
- Title formatting syntax is write-only (`$if($greater(%length_seconds%,300),…)`)
- Three unrelated mini-languages: display formatting, library query, autoplaylists
- Configuration is dozens of nested modal dialogs
- "Themes" are fragile bundles of third-party components from forum posts
- Patchy HiDPI, no sync, no queryable listening history, no automation surface

**Everything else** trades power away for polish. Roon requires a server
appliance and costs ~$250/yr. Plexamp requires Plex. Audirvāna is hi-fi playback
without a real library engine. Apple Music is neither.

Nobody has built the powerful one *well*.

---

## 3. Audience

Primary, in order of willingness to pay:

1. **Musicians** — want to slow down, loop, and isolate parts of recordings
2. **Hi-fi / DAC owners** — want exclusive output, DSD, room correction, and no resampling surprises
3. **Large-library archivists** — 50k to 500k+ tracks, care about tagging and metadata integrity
4. **DJs and crate-diggers** — care about BPM, key, energy, and fast constrained selection
5. **Spotify refugees** — want to own their library and understand what's playing

Secondary: the very large group of people who would simply like a good desktop
music player and currently have no answer on macOS or Linux.

---

## 4. Product

### 4.1 The expression language (the differentiator)

One typed language, with autocomplete, live preview and real error messages,
used everywhere:

```
recent(30d) and bpm > 120 and not played(7d)
  shuffle by artist, max 2 per artist
  then similar(current) limit 20
```

The same language is used for:

- **Library search** — instant, incremental, over the whole library
- **Smart playlists** — saved queries that stay live
- **Display formatting** — column contents, now-playing text, file naming on export
- **Autoplay rules** — what plays when the queue runs out
- **History analytics** — `played(2019) group by artist order by count`

Consequence: **the user can always ask why a track is playing and get an
answer.** No player currently offers this. It is the one-line pitch.

### 4.2 Playback engine

- Exclusive / bit-perfect output on all three platforms (WASAPI + ASIO, CoreAudio hog mode, ALSA/PipeWire)
- True gapless, sample-accurate
- DSD, high sample rates, no hidden resampling; visible signal path
- ReplayGain 2 / EBU R128 scanning and application
- Convolution engine for room correction and headphone correction via impulse responses
- Per-output-device DSP profiles that switch automatically when the device changes
- Visible DSP chain — the user can see every stage between file and DAC

### 4.3 Library engine

- Incremental scanning; no full rescans, no library rebuild on move
- Sub-100ms search at 500k tracks
- Metadata repair, cross-format duplicate detection, batch tagging
- Library format that survives files being moved or reorganised externally
- Full-fidelity tag support (Vorbis comments, ID3v2.4, MP4, APE)

### 4.4 Metadata lookup (MusicBrainz)

- Look up releases, artists, and recordings against MusicBrainz
- Write MusicBrainz IDs into tags using the Picard-compatible field names, so identity survives retagging
- Fetch cover art from the Cover Art Archive
- AcoustID fingerprinting for untagged or mistagged files
- All lookups are opt-in, rate-limited to the MusicBrainz one-request-per-second rule, and cached locally
- No lookup ever overwrites a tag without showing a diff first

### 4.5 Scrobbling (Last.fm and ListenBrainz)

- Native Last.fm scrobbling using the standard rules: submit at 50% played or 4 minutes, whichever comes first
- ListenBrainz as a second target on the same queue. Both ship in v1; the second target is under a day of work once the first exists
- Offline queue: plays recorded while disconnected are submitted later, in order
- Scrobbles are derived from the local history (4.7), never the other way round. Local history stays the source of truth

### 4.6 Stem separation

On-device source separation in the transport bar: isolate or mute vocals, drums,
bass, other. Loop a bar range. Time-stretch without pitch shift.

No upload, no server, no subscription. Every competitor in this space
(Moises et al.) requires uploading the file and paying monthly. This is the
feature that makes people switch, and the feature that generates videos.

### 4.7 Listening history

Every play recorded locally, forever, queryable in the same expression language.
The user's own listening data, owned by the user, truthful, and not deleted by a
service shutting down.

### 4.8 Sync (post-v1)

Play counts, ratings, playlists, and playback position sync across desktop
machines and, later, a phone. CRDT-based, end-to-end encrypted, peer-to-peer.

Peers find each other through a small relay that only passes encrypted bytes and
holds no data. The user never runs a server. This is the specific thing Roon
and Plexamp cannot offer, because both are built around a central component the
user must operate.

### 4.9 Layouts

A layout is a single text file. Drop it in, it works. Shareable in one paste. No
component hunt, no dependency chain, no forum archaeology.

---

## 5. Architecture

**Core (Rust):** decoding, DSP graph, output device abstraction, library index
and query planner, expression language, sync engine. Fully headless and testable,
with a CLI front end used for integration testing from day one.

**UI: GPUI.** The GPU-rendered UI framework extracted from Zed and published on
crates.io. It already solves the problems that make a custom renderer expensive:
text shaping, subpixel positioning, HiDPI, platform windows on Windows, macOS and
Linux, and a retained element tree that stays fast with large lists.

Rationale: Rust has no strong cross-platform UI story. The options were a
custom GPU renderer (best result, most work), iced/egui (faster, but typography
and text layout are a persistent fight), Tauri (fastest, and the target audience
will identify the webview within seconds and hold it against us), or GPUI. GPUI
gives the custom-renderer result at a fraction of the cost, because Zed has
already paid for it. Since the entire positioning is "made by people who care
about the details," this is the only option that is both affordable and
defensible.

Known costs of GPUI: it is young, the API moves between releases, documentation
is thin, and the Windows and Linux backends are less mature than macOS. We pin a
version, vendor the crate if we must, and validate all three backends in the
two-week prototype before committing.

**Platforms:** Windows, macOS, Linux from one codebase. Mobile deferred until
sync exists, since a phone client without sync has no reason to exist.

---

## 6. Effort

| Subsystem | Engineer-months |
|---|---|
| UI shell + layout system (on GPUI) | 3.5 |
| Audio engine (decode, gapless, exclusive output, DSP) | 3.0 |
| Sync | 3.0 |
| Library index + query planner | 2.5 |
| Tagging and file management | 1.5 |
| Expression language | 1.5 |
| Stem separation | 1.5 |
| MusicBrainz lookup + AcoustID | 0.5 |
| Last.fm / ListenBrainz scrobbling | 0.25 |
| History and analytics | 0.5 |
| **Total** | **17.75** |

Note that the UI is still the largest item even on GPUI. This is true of every
media application and it is what kills most of them. GPUI removes the renderer
from the estimate. It does not remove the layout system, the widgets, or the
polish.

---

## 7. Roadmap

**v1 — ~10 engineer-months.** Windows, macOS, Linux. One binary. Exclusive-mode
output, gapless, library index, expression language with live preview, tagging,
MusicBrainz lookup, Last.fm and ListenBrainz scrobbling, one excellent default
layout. Ships without sync, without stems, without mobile.

Scrobbling and MusicBrainz are in v1 deliberately. They are cheap, the audience
treats them as table stakes, and their absence is the first thing a foobar2000
user would post about.

Even cut this far, v1 is better than anything currently available.

**v1.1 — Stem separation.** First paid upgrade. Drives acquisition because it
demonstrates in fifteen seconds.

**v1.2 — Sync.** Drives retention and justifies a recurring line. This is the
point at which people stop evaluating alternatives.

**v2 — Mobile client, convolution/room correction UI, plugin API.**

---

## 8. Pricing

The category has a generous price anchor. Roon is ~$250/year or ~$830 lifetime
for software that requires a server appliance and that most owners tolerate
rather than love. Audirvāna sits near $100. Nothing in this space is cheap, and
free alternatives do not do any of this.

- **Needle desktop:** $79 one-time, all platforms, perpetual for that major version
- **Stems:** included, or $29 add-on (decide after v1 demand signal)
- **Sync:** $3/month, optional, never required to use the app

Deliberately not subscription-only. The audience is specifically people who
resent renting software, and pricing against that resentment is part of the
product.

---

## 9. Financial expectation

Realistic ceiling is a few thousand customers, not a few hundred thousand:
roughly **$150k–$400k/year** if it lands well.

This does not fund a studio by itself and should not be expected to. It is a
strong second revenue line alongside the developer tooling, and it is the best
available advertisement for a studio whose entire claim is that it builds careful
software. Treat the marketing value as a real part of the return.

---

## 10. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| UI work overruns (the usual killer) | High | GPUI removes the renderer from scope; two-week prototype on all three platforms; hard gate at month 3 |
| GPUI is young and its API churns | Medium | Pin a version; vendor the crate if needed; keep the UI layer thin over the headless core so a framework swap is survivable |
| Audience is small and price-sensitive | Medium | Price anchored against Roon/Audirvāna, not against free players; stems reach a second audience with proven spend |
| Streaming integration pressure | Medium | Explicitly out of scope for v1 and v2. Licensing would consume months and invites platform risk |
| Format and device edge cases are endless | Medium | Headless core plus CLI enables an automated conformance suite from day one |
| Expression language scope creep | Low | It is a query and formatting DSL, not a general-purpose language. Written specification before implementation |
| Third-party API changes (Last.fm, MusicBrainz) | Low | Both APIs have been stable for over a decade; both features are isolated modules behind a trait |
| One-year build with no revenue | High | Sequence behind or alongside a revenue-generating engagement; do not run it as the studio's only project |

---

## 11. Success criteria

- **v1:** search under 100ms at 500k tracks; bit-perfect verified on all three platforms; 1,000 paying customers in the first six months
- **v1.1:** stems become the top attributed acquisition source
- **v1.2:** sync attach rate above 30% of active licence holders
- **Qualitative:** the expression language is the thing people post about

---

## 12. Open questions

1. GPUI prototype — two weeks, all three platforms, before committing the UI estimate
2. Stems bundled or sold separately — affects the v1 price point
3. Linux audio: PipeWire-only, or also maintain a direct ALSA path
4. Plugin API in v1, or deliberately closed until the core is stable
5. Whether the expression language shares implementation lineage with Nivren tooling, or stays fully independent
