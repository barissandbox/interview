# Agent Instructions

## Project Overview

Interview is a Tauri 2 desktop app for live interview assistance. The Rust backend captures speaker and microphone audio, converts audio to 16 kHz mono PCM, streams it to Deepgram, persists settings/transcripts/questions/answers as JSON files, authenticates with ChatGPT through OAuth, extracts optional PDF CV text, generates interview answers with ChatGPT, and emits live events to a TypeScript frontend.

The UI is a compact desktop control surface with:

- Deepgram API key validation and saved-key gating.
- ChatGPT sign-in/sign-out below the Deepgram API row.
- Speaker and microphone source toggles/selects.
- Source language, ChatGPT model, thinking variant, answer type, and target position controls.
- PDF CV upload/removal.
- Transcript navigation, create/delete, copy, start/stop, and Answer controls.
- A transcript pane with a resizable Answer pane below it.
- Footer links for developer and source repository.

## Repository Layout

- `.github/workflows/release-desktop.yml`: tagged-release workflow. Builds Windows, Linux, and macOS packages with Node 22, stable Rust, Tauri CLI v2, `npm ci`, and `cargo tauri build`, then publishes release assets.
- `.gitignore`: ignores build outputs, generated schemas, IDE files, logs, local DBs, `.env`, and `frontend/dist`.
- `package-lock.json`: root placeholder lockfile; the active frontend dependency lockfile is `frontend/package-lock.json`.
- `README.md`: user-facing Interview overview and development commands.
- `Cargo.toml`: Rust package manifest. Main crate is `interview`, edition 2024, Tauri 2 desktop app. Uses `vendor/typeid` through `[patch.crates-io]`.
- `tauri.conf.json`: Interview product metadata, frontend build hooks, app window size, icons, and bundle targets.
- `build.rs`: ensures `frontend/dist` exists for `cargo run`, copies `icons/icon.png` for the frontend header, then calls `tauri_build::build()`.
- `capabilities/default.json`: Tauri capability for the `main` window with `core:default`.
- `gen/schemas/*`: generated Tauri schema/ACL metadata. These are ignored generated files; avoid manual edits.
- `icons/*`: app icon assets. Keep the primary bundle icon names as `icons/icon.png` and `icons/icon.ico`.
- `images/interface.png` and `images/interface2.png`: README/interface screenshots.
- `CLAUDE.md`: intentionally contains only `read AGENTS.md`.

## Rust Backend

- `src/main.rs`: app startup, path/logger setup, `AppState` creation, event forwarder setup, Tauri command registration.
- `src/domain.rs`: domain constants, audio source enum, audio device DTO, language options, transcript/question/answer records, app settings, ChatGPT auth/profile/catalog models, Deepgram account status, language normalization/model selection.
- `src/app/commands.rs`: thin Tauri command wrappers. Keep command handlers small and delegate behavior to `AppState`.
- `src/app/state.rs`: central mutable app state behind `Arc<Mutex<_>>`; owns storage, settings, auth/profile/catalog data, transcript list, active transcript id, audio devices, capture session, Tokio runtime, Deepgram event sender, question detection, and ChatGPT answer orchestration.
- `src/app/capture.rs`: starts/stops/reconciles speaker and microphone capture streams and spawns one Deepgram streaming worker per active source.
- `src/app/events.rs`: bridges Deepgram worker events to Tauri frontend events, persists final transcript segments, detects speaker questions, and starts answer generation.
- `src/app/questions.rs`: speaker question detection and normalization helpers used by event handling and answer generation.
- `src/app/transcripts.rs`: helpers for default device selection, active transcript resolution, transcript text formatting, and recent transcript tail selection.
- `src/app/view.rs`: frontend-facing DTOs. Rust serializes these as camelCase; keep them synchronized with `frontend/src/types.d.ts`.
- `src/infra/audio.rs`: CPAL device discovery and capture. Speaker loopback is Windows-only. Captured samples are converted to mono 16 kHz PCM16.
- `src/infra/deepgram.rs`: Deepgram API key validation, balance lookup, realtime WebSocket streaming, auth fallback, keepalive/silence handling, and response parsing.
- `src/infra/chatgpt.rs`: ChatGPT OAuth PKCE login, localhost callback handling, token refresh, model catalog lookup, streaming response parsing, answer prompt construction, and PDF CV text extraction.
- `src/infra/logging.rs`: file logger setup for `Interview/app.log`.
- `src/infra/paths.rs`: resolves app data paths under `dirs::data_dir()/Interview`.
- `src/infra/shell.rs`: platform-specific external URL opening helpers.
- `src/infra/storage.rs`: JSON persistence for `settings.json`, `auth.json`, `profile.json`, `catalog.json`, transcript files, question records, and answer text.

## Frontend

- `frontend/package.json`: active frontend package. Scripts: `build` and `watch`.
- `frontend/tsconfig.json`: strict TypeScript, `outFile` is `frontend/dist/app.js`, source root is `frontend/src`.
- `frontend/scripts/prepare-dist.mjs`: creates `frontend/dist`, copies `index.html`, `styles.css`, and `icons/icon.png`.
- `frontend/index.html`: source document shell. DOM ids must match `frontend/src/dom.ts`.
- `frontend/styles.css`: source UI styles. Compact dark desktop theme for controls, transcript body, answer splitter, answer panel, and buttons.
- `frontend/src/types.d.ts`: TypeScript mirrors of Rust DTOs and UI event payloads.
- `frontend/src/dom.ts`: typed DOM lookup. Any new DOM id used by TS must be registered here.
- `frontend/src/backend.ts`: wrapper around `window.__TAURI__.core.invoke` and `window.__TAURI__.event.listen`.
- `frontend/src/render.ts`: rendering, select population, button state, transcript text, answer text, question navigation, copy feedback, toggle state, and device-name cleanup.
- `frontend/src/app.ts`: UI event binding, command invocation, backend event handling, app state refresh/navigation/settings save, CV upload, answer generation, and answer splitter behavior.
- `frontend/app.js`: legacy/fallback compiled JavaScript. It can be copied by `build.rs` only when `frontend/dist/app.js` is missing. Prefer rebuilding from TypeScript with `npm run build` in `frontend`; do not treat this file as the source of truth.

## Build And Verification

Run commands from the repository root unless noted.

- Install frontend deps: `cd frontend; npm install`
- Frontend build: `cd frontend; npm run build`
- Frontend watch: `cd frontend; npm run watch`
- Rust format: `cargo fmt`
- Rust build: `cargo build`
- Run locally: `cargo run`
- Release package build: `cargo tauri build`

On Windows PowerShell, `npm.cmd` is also acceptable for the frontend commands, for example `cd frontend; npm.cmd run build`.

There is no dedicated automated test suite. For most changes:

1. Run `npm run build` from `frontend` after frontend source, HTML, CSS, or icon-copy changes.
2. Run `cargo fmt` after Rust changes.
3. Run `cargo build` after Rust, Tauri config, build script, or command-boundary changes.

## Runtime Architecture

1. `src/main.rs` resolves app paths, installs file logging, creates `AppState`, spawns the event forwarder, and registers commands.
2. The frontend calls Rust commands with `window.__TAURI__.core.invoke`.
3. `AppState` loads settings, ChatGPT auth/profile/catalog state, and transcripts from the app data directory, resolves default devices, and creates an initial transcript if needed.
4. `start_capture` validates saved Deepgram/source state, starts CPAL streams through `CaptureSession`, and spawns Deepgram stream workers on the Tokio runtime.
5. CPAL callbacks convert incoming audio to mono 16 kHz PCM16 and push chunks into Tokio channels.
6. Deepgram workers emit status, interim, final, or error events over `crossbeam_channel`.
7. `spawn_event_forwarder` updates state, persists final segments, emits `transcript-event` payloads, and detects speaker questions.
8. Detected speaker questions are stored on the active transcript and sent to ChatGPT when signed in.
9. ChatGPT answer jobs stream partial answer text to the frontend, persist final answer text, and emit updated app state.
10. Frontend render helpers update status, transcript text, question navigation, answer text, controls, and navigation from returned `AppViewState` or live events.

## Command Boundary Rules

- If adding a Tauri command, update all of:
  - `src/app/commands.rs`
  - `src/app/mod.rs`
  - `src/main.rs` `tauri::generate_handler!`
  - frontend caller in `frontend/src/app.ts` or another TS module
- Command names in frontend `invoke` calls must match Rust command function names exactly.
- Command handlers should return `Result<T, String>` at the IPC boundary and delegate actual logic to `AppState`.
- External URLs should be opened through Rust shell helpers or approved Tauri APIs, not ad hoc frontend navigation.

## DTO And Event Synchronization

- Keep `src/app/view.rs` and `frontend/src/types.d.ts` synchronized.
- Keep `src/app/events.rs` `UiEvent` payloads and `UiEventPayload` in `frontend/src/types.d.ts` synchronized.
- Keep DOM ids in `frontend/index.html` and refs in `frontend/src/dom.ts` synchronized.
- Rust structs crossing the frontend boundary use `#[serde(rename_all = "camelCase")]`.
- Preserve useful `AppSettings` serde aliases unless intentionally migrating older stored settings.

## Data, Storage, And Security

- Deepgram API keys are saved only through `test_deepgram_key` after Deepgram validation.
- ChatGPT access and refresh tokens are stored only in `Interview/auth.json`; do not expose them in frontend DTOs.
- `FrontendSettings` intentionally excludes API keys and ChatGPT tokens; do not add secrets there casually.
- Settings are stored as JSON at the platform app data directory under `Interview/settings.json`.
- Transcript/question/answer files are stored as JSON under `Interview/data/`.
- CV text is extracted locally from PDF files and stored in `Interview/profile.json`.
- Logs are written to `Interview/app.log`.
- Do not commit real API keys, local settings, auth tokens, transcript data, CV text, logs, `.env`, DB files, or build outputs.

## Audio, Deepgram, And ChatGPT Notes

- Speaker loopback currently works only on Windows; microphone capture uses the default CPAL host.
- Audio stream handles must own and drop CPAL streams on their owner threads.
- Keep audio callbacks lightweight; do not block in CPAL callbacks.
- `send_pcm` uses simple nearest-neighbor resampling; improve carefully if changing audio quality behavior.
- Deepgram realtime uses `wss://api.deepgram.com/v1/listen`, linear16, 16000 Hz, mono, interim results, VAD events, punctuation, and the model selected by `model_for_language`.
- Keepalive sends both silence PCM and a Deepgram KeepAlive JSON message while the stream is open.
- Speaker final transcript segments are treated as interviewer text for question detection; microphone final transcript segments are treated as candidate text.
- ChatGPT OAuth callback uses `http://localhost:1455/auth/callback`; avoid changing this without updating the ChatGPT auth constants.
- ChatGPT answers should be generated for detected or manually selected questions, not interim transcript text.

## Frontend Guidelines

- Source of truth is `frontend/src/*`, `frontend/index.html`, and `frontend/styles.css`.
- Run `npm run build` from `frontend` to regenerate `frontend/dist`.
- Do not manually edit `frontend/dist`.
- Avoid relying on `frontend/app.js`; it is a fallback and may lag behind TypeScript source.
- Preserve the compact desktop layout. This is a utility app, not a landing page.
- Keep UI labels short and stable; ensure text fits in the small resizable window.
- Add new controls with explicit disabled/running states in `InterviewRender.updateButtons`.

## Release Workflow Notes

- Releases are triggered by tags matching `v*`.
- Linux CI installs WebKit/GTK, ALSA, appindicator, OpenSSL, xdo, librsvg, and pkg-config dependencies.
- The workflow uploads per-platform artifacts and publishes a GitHub Release with generated notes.
- If changing bundle targets, app identifiers, icons, or Linux system dependencies, update both `tauri.conf.json` and `.github/workflows/release-desktop.yml` as needed.

## Common Change Checklist

1. Identify whether the change touches frontend, Rust state/commands, audio/Deepgram/ChatGPT infrastructure, persistence, config, or release packaging.
2. Update all synchronized boundaries: command registration, DTOs, event payloads, DOM refs, and HTML ids.
3. Keep generated outputs generated; do not manually edit `frontend/dist` or `gen/schemas`.
4. For frontend changes, run `cd frontend; npm run build`.
5. For Rust changes, run `cargo fmt` and `cargo build`.
6. For release/config changes, review `.github/workflows/release-desktop.yml`, `tauri.conf.json`, `capabilities/default.json`, and `build.rs`.
