# Repository memory

## Architecture

- Tauri 2 native crate: `src-tauri`; Sycamore 0.9 WASM frontend: root crate.
- Native Rust owns Sakana HTTPS/SSE calls, cancellation, API-key secure persistence/session memory, URL opening, workspace persistence, and export. Do not move secrets or direct Sakana calls into WebView JavaScript.
- Workspace data is local JSON with atomic temporary-file replacement and `.bak` recovery. Never silently overwrite an unrecoverable corrupt primary.
- API keys are persisted only in the platform-native secure credential store, restored at startup, and zeroized in process memory on replacement/removal. Never write or log them in workspace/browser/plaintext storage.
- Supported research modes map to quick (no web tool), search (web search), and deep (web search plus higher reasoning).

## Verified commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
env -u NO_COLOR trunk build --release
env -u NO_COLOR cargo tauri build --no-bundle
npm run e2e
```

As of 2026-09-02: strict Clippy passes, 53 workspace tests pass, `npm run e2e` must include deterministic browser, performance, native Tauri, and API 36 Android emulator tests, the opt-in GLM Coding Plan live smoke passes, and both x86_64 and arm64 Android debug APKs build successfully. Trunk 0.21.14 is exposed at `~/.local/bin/trunk` in this environment. This shell exports `NO_COLOR=1`, which breaks Trunk 0.21.14's `--no-color` parsing; run Trunk/Tauri builds with `env -u NO_COLOR` (the project `.codex/config.toml` already strips `NO_COLOR` for Codex-run subprocesses).

## Constraints and release checks

- Fugu integration uses `https://api.sakana.ai/v1/models`, `/responses`, Responses API SSE events, and `web_search`. Verify exact models/tools with a real account before release because service access can change.
- All opened sources must remain validated HTTPS URLs without credentials; treat remote titles/snippets as untrusted text.
- Keep frontend assets offline-capable: no remote fonts, scripts, or CDN resources.
- This repository does not use `OPENAI_API_KEY`. Never request it, store it, or
  propose the OpenAI Image API/CLI fallback. `SAKANA_API_KEY` is unrelated.
- Keep UI accessibility intact: visible focus states, semantic labels,
  reduced-motion support, and practical 44px touch targets.
- Android SDK 36, Build Tools 35.0.0, NDK 29.0.13846066, JDK 21, and all four Rust Android targets are installed. Use `source scripts/android-env.sh` or the Android build scripts so Tauri sees them. Generated package identity follows Tauri identifier `com.ggobp.suisou-chat` -> namespace `com.ggobp.suisou_chat`.
- Cloud/device sync, attachments, voice, team sharing, and background research require a backend and explicit privacy/security design; do not imply these exist.
- Keep Trunk's generated JavaScript unchanged after it calculates file hashes
  and SRI. A post-build minifier previously changed the module without updating
  `index.html`, causing Android WebView to reject it and show only the CSS
  background. JavaScript minification is disabled until it can run before
  Trunk's final hashing step.
- Android secure credential storage initializes its NDK context from the generated `MainActivity`. Do not rerun `cargo tauri android init` without preserving that customization.

## Frontend map

- `src/app.rs`: Sycamore components, state, events, panels and UI flow
- `src/icons.rs`: repository-native SVG icon system
- `src/models.rs`: frontend workspace and message models
- `styles.css`: current global visual system and responsive rules
- `index.html`: document metadata and stylesheet entry
- `public/`: offline-capable frontend assets

Do not perform an unrelated architecture refactor during visual work. Split
components or styles only when the requested design would otherwise become
difficult to maintain.
