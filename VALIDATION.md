# Verification — 2026-11-19

Host: Apple M1 Pro, macOS, native arm64 build.

Passed:

- TypeScript and Vite production build; Rust check and formatting.
- Eight Rust unit tests covering URL/range validation, contiguous completed replay segments, GPU flags, caption escaping/timing, bounded speech candidates, and platform upload metadata validation.
- Nine browser tests covering both AI modes, save failures, caption draft persistence across polling, editor/export commands, replay controls, all eleven screens at 320/768/1024/1440 px, one-click generation, stage/download percentages, cancellation, advanced input validation, and exactly-once folder-opening commands for packages/manual exports.
- Actual FFmpeg import; original, square and vertical exports with burned captions; output dimensions and durations; multi-highlight export; cancellation; a deliberately unavailable GPU falling back to CPU.
- Real-time local fixture recording: saved 30- and 60-second replays with checked durations, then stopped and saved the full recording. Apple VideoToolbox passed its hardware probe.
- Real YouTube download/import using the previously supplied video URL; real Kick live capture from a currently live public channel, then stop/save.
- Whisper base transcription of the public JFK speech sample. Local `gemma3:1b` and ChatGPT-authenticated Codex each returned a valid highlight, title, reason, and score through the actual application backend.
- Packaged macOS app launched and created its window. Desktop app's ad-hoc signature verifies; bundled yt-dlp executes.

Limits:

- Native GUI click-through was blocked by `cua-driver`: “Accessibility permission not granted.” UI interactions were verified in a browser with mocked Tauri IPC; media and AI processing were verified through the real Rust engine. The user-provided screenshot additionally confirms the native create screen and real YouTube metadata lookup. Native playback/file-dialog behavior remains unverified.
- No physical AMD or NVIDIA GPU was available. Windows/Linux bundles were not built. Their encoder paths are implemented and must pass runtime hardware probes before becoming selectable.
- The app is ad-hoc signed for local use, not notarized for public distribution. Remote platform availability, account requirements and Codex usage limits can change.

The local speech model and Ollama language model are installed. The app data catalog was left without test projects. Workflow test media used isolated temporary directories except the initial speech verification fixture retained under the app's `work/` directory.

Download-progress update: parser boundary checks and UI checks passed for immediate feedback, live percentages, unknown totals, preparation and cancellation. A real YouTube import reported intermediate percentages (including 27.7%, 49.6% and 79.8%) and then a separate preparation stage before completing. Native click automation was not rerun because macOS Accessibility remains unavailable.

One-click package update:

- Real Whisper → local Ollama and Whisper → ChatGPT Codex pipelines each generated a vertical 1080×1920 captioned MP4 and all six upload metadata/text files. Durations, manifests, and library records were checked.
- A later-clip metadata failure left the earlier completed MP4 and texts intact; the manifest reported failure and one completed clip. Cancellation produced a cancelled manifest. Unknown export-folder job IDs were rejected.
- The small local model produced a less faithful description of the speech sample than Codex. Generated titles, descriptions and captions require review before publication; schema validation guarantees structure, not factual accuracy.
- Fresh-checkout downloader setup passed in an isolated empty directory and verified the official yt-dlp checksum.
- Updated macOS bundle built and passed strict ad-hoc signature verification. The Desktop app executable was replaced atomically, preserving the running process and bundled downloader. Reopening the app loads the updated UI.
- Folder opening is verified at the browser-to-command boundary and through backend path validation; an actual Finder window could not be exercised through native automation because of the existing Accessibility limitation.

- A full real YouTube-link → download → Whisper → Codex → captioned export run passed in 105 seconds. Download progress was measured, and the completed package contained a 50.1-second 1080×1920 MP4, nonempty description/hashtags, a completed manifest, and platform upload text. Test media stayed in an isolated temporary directory.

Clone update:

- YouTube Shorts discovery returned 30 real GTA 6 candidates, filtered by view count and sorted descending. The highest-ranked result reported approximately 135 million views. Its original download retained 1080×1920 resolution, 58.5-second duration, and the selected source format (`312+140`).
- TikTok public video discovery returned seven real candidates above 100,000 views. The leading candidate reported 9.8 million views. Its original download retained 1080×1920 resolution, 11.2-second duration, and the selected H.265 source format. Requests used public pages without browser credentials.
- Browser checks passed for the Clone defaults, one-click discovery/download, search-only mode, selected downloads, source links, failure visibility, input bounds, and responsive layouts. The existing seven browser checks also passed.
- A controlled downloader fixture verified the maximum-quality format/sort arguments and byte-identical file retention when a later video in the batch failed. The manifest recorded the completed download and the failure separately.
- TikTok keyword search relies on a public video index and verifies counts with yt-dlp. An initial domain-operator query returned unrelated results; the working implementation uses a topic query plus a second phrasing when no TikTok links are found. Discovery remains bounded to 30 Shorts per topic and 12 TikTok candidates per search, and cannot guarantee platform-wide top results.
