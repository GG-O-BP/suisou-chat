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
trunk build --release
cargo tauri build --no-bundle
```

As of 2026-07-25: strict Clippy passes, 18 workspace tests pass, Trunk release output is about 500 KiB, the Linux native executable builds, and an arm64 Android debug APK builds successfully. Trunk 0.21.14 is exposed at `~/.local/bin/trunk` in this environment.

## Constraints and release checks

- Fugu integration uses `https://api.sakana.ai/v1/models`, `/responses`, Responses API SSE events, and `web_search`. Verify exact models/tools with a real account before release because service access can change.
- All opened sources must remain validated HTTPS URLs without credentials; treat remote titles/snippets as untrusted text.
- Keep frontend assets offline-capable: no remote fonts, scripts, or CDN resources.
- Android SDK 36, Build Tools 35.0.0, NDK 29.0.13846066, JDK 21, and all four Rust Android targets are installed. Use `source scripts/android-env.sh` or the Android build scripts so Tauri sees them. Generated package identity follows Tauri identifier `com.ggobp.suisou-chat` -> namespace `com.ggobp.suisou_chat`.
- Cloud/device sync, attachments, voice, team sharing, and background research require a backend and explicit privacy/security design; do not imply these exist.
- Trunk currently emits a non-fatal minifier warning for wasm-bindgen JavaScript, but release builds succeed.
- Android secure credential storage initializes its NDK context from the generated `MainActivity`. Do not rerun `cargo tauri android init` without preserving that customization.
