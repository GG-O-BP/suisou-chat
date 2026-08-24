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

As of 2026-08-24: strict Clippy passes, 46 workspace tests pass, the deterministic E2E suite runs 21 browser/performance/native tests, the opt-in live Sakana smoke passes, the Linux native executable builds, and an arm64 Android debug APK has built successfully. Trunk 0.21.14 is exposed at `~/.local/bin/trunk` in this environment. This shell exports `NO_COLOR=1`, which breaks Trunk 0.21.14's `--no-color` parsing; run Trunk/Tauri builds with `env -u NO_COLOR` (the project `.codex/config.toml` already strips `NO_COLOR` for Codex-run subprocesses).

## Constraints and release checks

- Fugu integration uses `https://api.sakana.ai/v1/models`, `/responses`, Responses API SSE events, and `web_search`. Verify exact models/tools with a real account before release because service access can change.
- All opened sources must remain validated HTTPS URLs without credentials; treat remote titles/snippets as untrusted text.
- Keep frontend assets offline-capable: no remote fonts, scripts, or CDN resources.
- This repository does not use `OPENAI_API_KEY`. Never request it, store it, or
  propose the OpenAI Image API/CLI fallback. `SAKANA_API_KEY` is unrelated.
  Use the built-in `image_gen` tool only when it is actually exposed in the
  current session. If it is unavailable, continue with CSS/SVG or produce an
  asset brief for a user-supplied/local generator; do not block the design task.
- Android SDK 36, Build Tools 35.0.0, NDK 29.0.13846066, JDK 21, and all four Rust Android targets are installed. Use `source scripts/android-env.sh` or the Android build scripts so Tauri sees them. Generated package identity follows Tauri identifier `com.ggobp.suisou-chat` -> namespace `com.ggobp.suisou_chat`.
- Cloud/device sync, attachments, voice, team sharing, and background research require a backend and explicit privacy/security design; do not imply these exist.
- Keep Trunk's generated JavaScript unchanged after it calculates file hashes
  and SRI. A post-build minifier previously changed the module without updating
  `index.html`, causing Android WebView to reject it and show only the CSS
  background. JavaScript minification is disabled until it can run before
  Trunk's final hashing step.
- Android secure credential storage initializes its NDK context from the generated `MainActivity`. Do not rerun `cargo tauri android init` without preserving that customization.

## Aquarium visual direction

The product should feel like a quiet bioluminescent research observatory:
scientific, contemplative, alive, and unmistakably aquatic without becoming
childish or decorative. The core visual thesis is:

> A quiet bioluminescent deep-sea observatory where questions descend,
> evidence is discovered, and research returns as illuminated specimens.

Map product behavior to the aquarium world:

- Welcome screen: central observation tank
- Composer: pressure-resistant research capsule
- Quick answer: sunlit shallow water
- Web search: reef and sonar exploration
- Deep research: bioluminescent descent
- Research progress: depth gauge or dive telemetry
- Streaming answer: findings gradually illuminating
- Sources: numbered bioluminescent specimens
- Conversation history: prior dive logs
- Settings: life-support control room
- Failure: loss or instability of light, not decorative alarm effects
- Cancellation: controlled return to the surface

Light theme should feel like a sunlit aquarium gallery; dark theme should feel
like an abyssal observatory at night. Do not implement dark mode by simply
inverting light colors.

### Frontend map

- `src/app.rs`: Sycamore components, state, events, panels and UI flow
- `src/icons.rs`: repository-native SVG icon system
- `src/models.rs`: frontend workspace and message models
- `styles.css`: current global visual system and responsive rules
- `index.html`: document metadata and stylesheet entry
- `public/`: offline-capable frontend assets

Do not perform an unrelated architecture refactor during visual work. Split
components or styles only when the requested design would otherwise become
difficult to maintain.

### Anti-patterns

Avoid: generic AI SaaS dashboards; a mosaic of interchangeable rounded cards;
blue gradients used as the entire concept; excessive glassmorphism; cyberpunk
neon; random decorative bubbles; childish fish illustrations; decorative
animation unrelated to application state; generated UI screenshots embedded as
the actual interface; text baked into raster images; imitating a specific
product, brand, artist, or artwork. The interface must still have a
recognizable identity when animation, background imagery, and color are removed.

### Design workflow

For redesign, styling, animation, illustration, visual-polish, responsive UI,
or screenshot-review work, use `$suisou-aquarium-art-director`. Before a large
redesign: inspect the real components and states, produce three structurally
different art directions, evaluate them against the skill's visual scorecard,
recommend one, and implement Welcome + Composer + one research-progress state
first before extending to the rest of the app. For a request that explicitly
asks only for analysis, ideation, or methods, do not edit files. For a narrowly
scoped visual correction, skip the three-direction phase and make the smallest
consistent change.

### Visual implementation rules

- Preserve all existing product behavior unless the task explicitly changes it.
- Prefer CSS and repository-native SVG for interface graphics.
- Use raster generation (`$imagegen`) only for atmospheric backgrounds,
  textures, illustrations, or organic assets that cannot be represented well in
  CSS/SVG, and only when the current session actually exposes the built-in
  `image_gen` tool. Never ask for `OPENAI_API_KEY` or use the image-generation
  CLI/API fallback. When the built-in tool is absent, continue with CSS/SVG,
  procedural assets, or a precise asset brief. Never bake buttons, text, form
  controls, or `src/icons.rs` icons into generated images.
- Keep all final assets in the repository and available offline; store generated
  project assets under `public/assets/aquarium/` with descriptive kebab-case
  names.
- Do not add remote fonts, remote scripts, CDN dependencies, or runtime asset
  downloads. Do not add a production dependency without justifying it.
- Keep Korean text readable over stable, sufficiently opaque surfaces; maintain
  visible focus states and semantic labels; do not rely on color alone for
  state; keep touch targets at least 44 CSS pixels where practical.
- Preserve `prefers-reduced-motion`; the interface must remain understandable
  and polished with motion disabled. Animate primarily with `transform` and
  `opacity`, and keep atmospheric animation away from long-form text.

### Required visual states

When the affected surface can be rendered, check: 1440×900 desktop, 390×844
mobile, light and dark themes, reduced motion, new/empty workspace, existing
conversation, loading/bootstrap, API key missing, connecting/searching/
reasoning/writing, streamed partial answer, long Korean answer, sources panel,
settings panel, cancelled and failed answers, read-only/storage error, and
hover/focus-visible/disabled/active controls.
