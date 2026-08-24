# End-to-end testing

Suisou uses WebdriverIO for renderer, performance, and native Tauri tests.
The normal production binary never contains an automation server.

## Test layers

| Layer | Command | Coverage |
| --- | --- | --- |
| Browser functional | `npm run e2e:browser` | Deterministic UI states, modes, settings, API-key UX, history, streaming success/failure/cancellation, panels, shortcuts, mobile layout |
| Browser performance | `npm run e2e:performance` | Large workspace startup/search budgets, long streamed answer budget, blank-frame regression, long tasks, layout shift |
| Native Tauri | `npm run e2e:native` | Embedded WebDriver, real WebKit/WebView2/WKWebView, Rust IPC, atomic workspace persistence, HTTPS URL rejection, Markdown export, secret exposure checks |
| Live Sakana smoke | `npm run e2e:live` | One real request using an API key already stored in the platform credential store |

`npm run e2e` runs every deterministic layer. The live test is deliberately
opt-in because it uses a real account, network, quota, and nondeterministic
remote output.

## Security boundary

The embedded WebDriver server is controlled by the Cargo feature `e2e`.
`src-tauri/src/lib.rs` rejects any release build that attempts to enable this
feature. The test build uses:

```bash
cargo tauri build --debug --no-bundle \
  --features e2e \
  --config src-tauri/tauri.e2e.conf.json
```

The test config uses a separate application identifier and data directory:
`com.ggobp.suisou-chat.e2e`.

## Prerequisites

```bash
npm ci
npm run e2e:doctor
```

The doctor checks Cargo, the Tauri CLI, Trunk, Node, npm, and
Chrome/Chromium. Install the pinned Rust tools used in CI when they are
missing:

```bash
cargo install trunk --version 0.21.14 --locked
cargo install tauri-cli --version 2.11.4 --locked
```

Linux native tests require an X11 display. In headless CI:

```bash
xvfb-run -a -s "-screen 0 1440x900x24" npm run e2e:native
```

Chrome/Chromium is used for deterministic browser and performance tests. The
native suite uses the embedded `tauri-plugin-wdio-webdriver` server and does
not require an external `tauri-driver`.

## Fixtures and artifacts

- `index.e2e.html` is a test-only Trunk entry point.
- `e2e/browser-bridge.js` implements deterministic Tauri command and event
  fixtures before the WASM application starts.
- `dist-e2e/` and `e2e/artifacts/` are ignored build/test output.
- A screenshot is stored in `e2e/artifacts/` when a test fails.
- Native backend output is stored as `e2e/artifacts/native-backend.log`.

The fixture bridge covers empty, existing, read-only, slow-bootstrap,
bootstrap-error, save-error, ready, and large-history states. Streaming
scenarios cover success, slow/cancel, failure, and performance output.

## Live test

The live smoke test never accepts an API key in a file or command argument. It
only uses a key already present in the platform-native credential store.

```bash
SUISOU_E2E_LIVE=1 npm run e2e:live
```

Do not run the live test in untrusted CI environments.
