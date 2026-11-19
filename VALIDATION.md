# Verification — 2026-11-19

Host: Apple M1 Pro, macOS, native arm64 build.

Passed:

- TypeScript and Vite production build; Rust check and formatting.
- Ten Rust unit tests covering URL/range validation, contiguous completed replay segments, GPU flags, caption escaping/timing, bounded speech candidates, and platform upload metadata validation.
- Eleven browser tests covering both AI modes, save failures, caption draft persistence across polling, editor/export commands, replay controls, all eleven screens at 320/768/1024/1440 px, one-click generation, stage/download percentages, cancellation, advanced input validation, and exactly-once folder-opening commands for packages/manual exports.
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
- TikTok keyword search relies on a public video index and verifies counts with yt-dlp. An initial domain-operator query returned unrelated results; the working implementation uses a topic query plus a second phrasing when no TikTok links are found. That initial 12-candidate limit was subsequently replaced by the paginated search described below; results still cannot guarantee platform-wide top rankings.


Clone relevance update:

- Topics remain editable during active work and are saved across navigation and reloads. The default is now GTA 6 alone; separate `gaming` searches previously admitted unrelated games. All ten browser checks passed, including editable/persisted topics, topic bounds, evidence cards, and existing flows.
- Nine Rust unit checks passed. Added regression cases for GTA VI / Grand Theft Auto VI aliases, GTA V and GTA 60 exclusions, misleading hashtags, missing/negative/low-confidence/mismatched-topic AI responses, and current Shorts search metadata parsing.
- A real public YouTube keyword search returned 25 Shorts candidates through the Rust backend. Full YouTube preview downloads were blocked by the platform's sign-in/bot check during validation. These candidates are excluded with an error rather than approved from hashtags. No credentials or bypass were used.
- The actual FFmpeg → six frames → ChatGPT Codex path accepted a public TikTok GTA VI trailer discussion with concrete visible evidence and rejected a generated test-pattern video despite misleading GTA 6 title/hashtags. Disposable previews were removed. The same AI transport still returned the expected text-only highlight schema.
- Partial original-download recovery still passed and retained the completed file byte-for-byte. Maximum-quality export arguments remain unchanged.
- Verification checks metadata and six sampled frames, not the complete audiovisual sequence. Model scores are self-reported and relevance/authenticity are not guaranteed. At most 12 candidates undergo deeper verification; videos longer than three minutes and inconclusive results are excluded. Speech-only relevance is not evaluated.
- Local Ollama vision transport is implemented with an explicit capability check and a gemma3:4b setup option. A full local vision inference was not run on this host; the existing small text model cannot evaluate images. Native UI click-through remains unverified because of the existing Accessibility permission limitation.

- The complete live TikTok discovery → metadata/topic filtering → frame verification → original-download workflow passed in 269 seconds. Two candidates passed the visual check. The leading result reported 4.4 million views and retained 1080×1920, 7.9-second H.265 source quality (`bytevc1_1080p_2284499-1`).
- The updated macOS release bundle built, passed strict signature verification, and replaced the Desktop app with the previous bundle retained as a local backup. Cutline relaunched successfully; existing app data and downloaded files were preserved.


YouTube session update:

- Added an opt-in browser session setting shared by YouTube lookups, imports, live resolution, Clone metadata checks, previews and original downloads. Existing preferences deserialize to public/no-session access; invalid browser values are rejected. TikTok, Kick, non-HTTPS and lookalike domains do not receive the browser-session argument.
- Ten Rust unit tests and all eleven browser tests passed. The new UI check covers opt-in defaults, saved browser selection across navigation and visible save failures. Error checks cover sign-in renewal, Keychain/cookie failures and preservation of unrelated errors.
- The exact reported video (`VzEQkOCy-EA`) loaded through the signed-in Chrome session. An initial playback check was still denied; after opening that video normally in Chrome, the real application backend passed metadata lookup, preview download, six-frame AI analysis and original download. The downloaded original measured 1080×1920 and 52.5 seconds. The content classifier rejected relevance, which is separate from authentication success.
- Public metadata is explicitly projected instead of printing full yt-dlp information, which can include session cookies in format entries. Original export metadata was checked for absence of cookies, headers and formats. Browser sessions are not exported into files or passed to AI.
- YouTube may still require the user to renew their browser login or complete a confirmation. Browser reopening was exercised, but native page inspection remains blocked by macOS Accessibility permission. No CAPTCHA solving, alternate-identity rotation or access-control bypass was used. The end-to-end check covered Chrome on this Mac; other browsers/platforms were not exercised.

- The rebuilt macOS app passed strict signature verification, was installed on the Desktop and relaunched. The local YouTube browser setting was set to Chrome after the successful real-session test; other preferences and media were preserved.

## Clone quantity and exact view counts (2026-11-19)

- Raised search and batch-download limits to 10,000; the UI remembers the desired count. The target applies to search-only and automatic downloads. No 10,000-file download was performed.
- Replaced the fixed 12-candidate cap with a target-scaled finite budget and ordinary YouTube result continuations; TikTok uses four distinct topic-preserving queries because an experimental Bing page offset repeated the same URLs. Duplicate URLs are checked once.
- Deterministic regression checks cover the inclusive 100,000 minimum, 13 consecutive below-minimum candidates followed by valid candidates, 10,000/10,001 bounds, old cached results, continuation parsing, and avoiding a misleading Keychain message for unrelated errors containing cookie advice.
- The screenshot's 4.4 million and 2 million views both exceed 100,000. Cards now show full counts; the summary separates rejection/failure totals and actual versus requested results.
- A normal signed-in Chrome metadata request to the reported YouTube video returned a sign-in confirmation requirement again. Authentication remains dependent on YouTube; Clone now stops repeated YouTube checks on a session failure and continues TikTok.
- The full 12-test browser suite passed, then all five Clone tests passed after adding 100-results-per-page rendering and selecting a synthetic 10,000-result batch. Eleven deterministic Rust tests passed; the opt-in real YouTube continuation test also passed and confirmed new URLs on the second page.
- A real TikTok search → six-frame Codex verification → original download passed in 144 seconds: one requested/approved result with 4,400,000 views, 1080×1920 output, 7.9 seconds, source format `bytevc1_1080p_2284499-1`. Native click inspection still reports Accessibility permission unavailable.
- The final arm64 app rebuilt, passed strict signature verification, replaced the Desktop bundle with a retained backup, and relaunched. Existing application data was left unchanged.
