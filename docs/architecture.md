# Suisou source architecture

The repository keeps the WebView, native runtime, persistence, and secure
credential boundaries explicit. Refactors should preserve the Tauri command
names, `research-event` payload contract, workspace JSON schema, and the public
frontend `App` entry point.

## Frontend (`src/`)

- `app/mod.rs` — application composition only; creates the shared state context
  and assembles top-level surfaces.
- `app/runtime.rs` — startup bootstrap and the native research-event listener.
- `app/browser.rs` — browser/DOM adapters such as shortcuts, theme attributes,
  scrolling, external links, clipboard access, and research-stage presentation.
- `app/state/` — state transitions and use-case orchestration:
  - `mod.rs`: signals, internal IPC DTOs, and read-only selectors.
  - `stream.rs`: animation-frame batching for streamed text.
  - `persistence.rs`: serialized workspace saves and delete rollback.
  - `conversations.rs`: selection, creation, deletion, pinning, and retry.
  - `research.rs`: request construction, execution, cancellation, and results.
  - `credentials.rs`: API-key connection and removal flows.
  - `export.rs`: current-conversation export flow.
- `app/components/` — view-only feature surfaces. Components consume `AppState`
  but do not own native secrets or call Sakana directly.
- `ipc.rs` — the typed Tauri invoke/listen boundary.
- `models.rs` — frontend workspace/request/event data and pure helpers.
- `markdown.rs` — safe answer rendering.
- `icons.rs` — repository-native SVG icons.

## Native runtime (`src-tauri/src/`)

- `lib.rs` — Tauri builder, window construction, managed state, and command
  registration only.
- `app_state.rs` — paths, save lock, and the shared `FuguRuntime` handle.
- `commands/` — thin IPC adapters grouped by workspace, credentials, research,
  and operating-system operations. Existing command names remain stable.
- `fugu/mod.rs` — runtime type, invariants, limits, and internal module boundary.
- `fugu/runtime_credentials.rs` — secure in-memory key lifecycle and verification.
- `fugu/runtime_research.rs` — active request registry and request orchestration.
- `fugu/stream.rs` — bounded SSE consumption and frame parsing.
- `fugu/response.rs` — answer, HTTPS source, and usage extraction.
- `fugu/policy.rs` — mode instructions and output budgets.
- `fugu/transport.rs` — event emission, cancellation, HTTP/network errors, and
  key/string validation helpers.
- `credentials.rs` — platform-native secure credential-store abstraction.
- `storage.rs` — atomic workspace persistence, recovery, and Markdown export.
- `models.rs` — persisted/native IPC data contracts and validation.

## Styles (`styles/`)

`styles.css` is the stable Trunk entry point. It imports ordered local layers:

1. `foundation-shell.css` — global tokens, primitives, navigation, and shell.
2. `foundation-content.css` — welcome, transcript, Markdown, and progress.
3. `foundation-controls.css` — composer, panels, settings, and overlays.
4. `foundation-responsive.css` — base responsive, theme, and reduced-motion rules.
5. `observatory-environment.css` — bathymetric tokens, shell, and welcome tank.
6. `observatory-transcript.css` — illuminated answers and dive telemetry.
7. `observatory-controls.css` — capsule composer and evidence/control panels.
8. `observatory-responsive.css` — observatory responsive/theme/motion rules.
9. `welcome-observatory.css` — focused welcome-tank composition refinements.

Import order is part of the visual contract; do not reorder these files without
checking light/dark, desktop/mobile, and reduced-motion states.
