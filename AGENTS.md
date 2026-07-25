# Repository memory

## Architecture

- Tauri 2 native crate: `src-tauri`; Sycamore 0.9 WASM frontend: root crate.
- Native Rust owns Sakana HTTPS/SSE calls, cancellation, API-key session memory, URL opening, persistence, and export. Do not move secrets or direct Sakana calls into WebView JavaScript.
- Workspace data is local JSON with atomic temporary-file replacement and `.bak` recovery. Never silently overwrite an unrecoverable corrupt primary.
- API key is intentionally session-only and zeroized on replacement/removal; do not persist or log it.
- Supported research modes map to quick (no web tool), search (web search), and deep (web search plus higher reasoning).

## Verified commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
trunk build --release
cargo tauri build --no-bundle
```

As of 2026-07-25: strict Clippy passes, 12 workspace tests pass, Trunk release output is about 487 KiB, and the Linux native executable builds and smoke-launches. Trunk 0.21.14 is exposed at `~/.local/bin/trunk` in this environment.

## Constraints and release checks

- Fugu integration uses `https://api.sakana.ai/v1/models`, `/responses`, Responses API SSE events, and `web_search`. Verify exact models/tools with a real account before release because service access can change.
- All opened sources must remain validated HTTPS URLs without credentials; treat remote titles/snippets as untrusted text.
- Keep frontend assets offline-capable: no remote fonts, scripts, or CDN resources.
- Android SDK is not installed in this environment; generated Android package identity follows Tauri identifier `com.ggobp.suisou-chat` -> namespace `com.ggobp.suisou_chat`. Build on an SDK-equipped host before release.
- Cloud/device sync, attachments, voice, team sharing, and background research require a backend and explicit privacy/security design; do not imply these exist.
- Trunk currently emits a non-fatal minifier warning for wasm-bindgen JavaScript, but release builds succeed.
