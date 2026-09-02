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
  but do not own native secrets or call a research provider directly.
- `ipc.rs` — the typed Tauri invoke/listen boundary.
- `models.rs` — frontend workspace/request/event data and pure helpers.
- `markdown.rs` — safe answer rendering.
- `icons.rs` — repository-native SVG icons.

## Native runtime (`src-tauri/src/`)

- `lib.rs` — Tauri builder, window construction, managed state, and command
  registration only.
- `app_state.rs` — paths, save lock, and the shared research-runtime handle.
- `commands/` — thin IPC adapters grouped by workspace, credentials, research,
  and operating-system operations. Existing command names remain stable.
- `fugu/mod.rs` — native research runtime type, invariants, provider limits,
  and internal module boundary. The historical module name is retained to keep
  this provider change focused.
- `fugu/runtime_credentials.rs` — provider-specific secure key lifecycle,
  restoration, and connection verification. Sakana uses the legacy
  `sakana-api-key` credential entry; Z.ai uses `zai-glm-api-key`.
- `fugu/runtime_research.rs` — provider routing and Sakana Responses requests.
- `fugu/zai.rs` — Z.ai GLM Coding Plan Chat Completions payload construction,
  bounded SSE consumption, metadata merging, and completion handling.
- `fugu/stream.rs` — bounded Sakana Responses SSE consumption and shared frame parsing.
- `fugu/response.rs` — provider answer extraction plus HTTPS/no-userinfo source
  and usage normalization.
- `fugu/policy.rs` — mode instructions and output budgets.
- `fugu/transport.rs` — event emission, cancellation, HTTP/network errors, and
  key/string validation helpers.
- `credentials.rs` — platform-native secure credential-store abstraction.
- `research_jobs.rs` — active/finalizing research jobs, durable journaling,
  workspace commits, event emission, and provider-aware credential locks.
- `storage.rs` — atomic workspace persistence, recovery, and Markdown export.
- `models.rs` — provider/model contracts, persisted/native IPC data, and validation.

## Styles (`styles/`)

`styles.css` is the stable Trunk entry point. It imports ordered local layers:

1. `foundation-shell.css` — global tokens, primitives, navigation, and shell.
2. `foundation-content.css` — welcome, transcript, Markdown, and progress.
3. `foundation-controls.css` — composer, panels, settings, and overlays.
4. `foundation-responsive.css` — base responsive, theme, and reduced-motion rules.
5. `observatory-environment.css` — environmental tokens, shell, and welcome composition.
6. `observatory-transcript.css` — answer rendering and progress telemetry.
7. `observatory-controls.css` — composer and source/settings panels.
8. `observatory-responsive.css` — responsive/theme/motion rules.
9. `welcome-observatory.css` — focused welcome composition refinements.

Import order is part of the visual contract; do not reorder these files without
checking light/dark, desktop/mobile, and reduced-motion states.
