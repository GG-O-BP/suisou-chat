# End-to-end testing

Suisou uses WebdriverIO for renderer, performance, native Tauri, and Android
APK tests.
The normal production binary never contains an automation server.

## Test layers

| Layer | Command | Coverage |
| --- | --- | --- |
| Browser functional | `npm run e2e:browser` | Deterministic UI states, modes, settings, API-key UX, history, streaming success/failure/cancellation, panels, shortcuts, mobile layout |
| Browser performance | `npm run e2e:performance` | Large workspace startup/search budgets, long streamed answer budget, blank-frame regression, long tasks, layout shift |
| Native Tauri | `npm run e2e:native` | Embedded WebDriver, real WebKit/WebView2/WKWebView, Rust IPC, atomic workspace persistence, HTTPS URL rejection, Markdown export, secret exposure checks |
| Android APK | `npm run e2e:android` | API 36 Emulator, real x86_64 APK, MainActivity, System WebView, Rust IPC bootstrap, notification permission, settings/composer interaction, process restart, logcat secret checks |
| Live smoke | `npm run e2e:live` | Provider-specific real requests using an API key already stored in the platform credential store |

`npm run e2e` runs every deterministic layer, including Android. The Android
SDK, JDK, emulator image, and KVM-capable Linux host are therefore required
E2E prerequisites. The live test alone is deliberately opt-in because it uses a
real account, network, quota, and nondeterministic remote output.

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

The doctor checks Cargo, the Tauri CLI, Trunk, Node, npm, Chrome/Chromium, and
the Android emulator toolchain. Install the pinned Rust tools used in CI when
they are missing:

```bash
cargo install trunk --version 0.21.14 --locked
cargo install tauri-cli --version 2.11.4 --locked
```

Linux native tests require an X11 display, and the full suite requires a
KVM-capable Android emulator host. In headless CI:

```bash
xvfb-run -a -s "-screen 0 1440x900x24" npm run e2e:native
```

Because `npm run e2e` launches both desktop native and Android tests, CI runs
those layers as separate required jobs while retaining the same test commands.

Chrome/Chromium is used for deterministic browser and performance tests. The
native suite uses the embedded `tauri-plugin-wdio-webdriver` server and does
not require an external `tauri-driver`.

## Android APK E2E

The Android suite is a required black-box layer. It drives the normal x86_64
debug APK through Appium/UiAutomator2 instead of enabling the desktop-only
embedded WebDriver feature. This preserves the generated package namespace
`com.ggobp.suisou_chat`, crosses the actual APK/System WebView/Rust IPC
boundaries, and ensures the release application never receives a test
automation server or permission.

### Prerequisites

- Linux with writable `/dev/kvm`
- Android SDK 36, Build Tools 35.0.0, NDK 29.0.13846066, JDK 21
- Android Emulator and `system-images;android-36;google_apis;x86_64`
- all Rust Android targets documented in `AGENTS.md`
- project dependencies from `npm install`

Source the checked-in environment helper and run the preflight:

```bash
source scripts/android-env.sh
npm run e2e:android:doctor
```

`scripts/android-env.sh` adds `platform-tools`, `cmdline-tools/latest/bin`, the
emulator, and the NDK toolchain to `PATH`. `ANDROID_USER_HOME` is pinned so
`avdmanager` and `emulator` agree on where AVD definitions live.

At runtime the frontend marks Android System WebView with
`data-platform="android"`. The Android-only stylesheet keeps the same
composition and color hierarchy while removing expensive blur,
large shadow, mask, and idle-animation raster work that can stall first paint
on SwiftShader emulators and lower-end devices. Desktop and iOS rendering are
unchanged. The Android window theme and lightweight HTML boot surface also keep
a stable themed loading state visible until the first WebView frame,
instead of exposing a blank white window during cold startup.

### Run

The one-command path builds the APK, creates/boots the AVD if needed, installs
the pinned project-local UiAutomator2 driver, installs the APK, runs the WDIO
suite, and saves logcat:

```bash
npm run e2e:android
```

Useful individual operations:

```bash
npm run e2e:android:build
scripts/android-emulator.sh ensure-avd
scripts/android-emulator.sh start
scripts/android-emulator.sh status
scripts/android-emulator.sh stop
```

Environment overrides:

- `SUISOU_AVD_NAME` (default `suisou_api36`)
- `SUISOU_ANDROID_API` (default `36`)
- `SUISOU_ANDROID_APK` to test a specific APK
- `SUISOU_SKIP_APK_BUILD=1` to reuse an APK
- `SUISOU_SKIP_EMULATOR=1` to use an already-running device/emulator
- `SUISOU_EMULATOR_BOOT_TIMEOUT` (default 300 seconds)
- `SUISOU_EMULATOR_SIZE` (default `390x844`, use `physical` to keep AVD size)
- `SUISOU_EMULATOR_DENSITY` (default `160`, use `physical` to keep AVD density)
- `APPIUM_HOME` (default `e2e/.appium`)

The default APK output is:

```text
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

### Coverage and responsibility

The browser suite remains responsible for exhaustive deterministic fixtures,
streaming permutations, long Korean answers, responsive layouts, themes,
cancel/failure states, and performance budgets. The Android suite intentionally
does not duplicate all of those tests. It targets Android-specific boundaries:

- APK installation and `.MainActivity` foreground launch
- `POST_NOTIFICATIONS` permission handling
- System WebView discovery and CSS-selector interaction
- Korean interface text rendering without Unicode replacement glyphs
- frontend bootstrap through real Tauri/Rust IPC
- composer mode/input and settings/key-field behavior
- background/foreground transitions and orientation configuration changes
- app process restart and persisted workspace-setting recovery
- screenshot, Appium log, and `adb logcat` diagnostics
- obvious credential-marker absence in logcat

The smoke test does not enter or store an API key. Android Keystore behavior
with a real secret and foreground-research notification longevity remain
release/device checks; they should not run in untrusted CI.

### CI

`.github/workflows/android-e2e.yml` runs on every pull request and `main` push,
and can also be dispatched manually. It enables KVM access, installs the API 36
image, builds the x86_64 APK, runs the Android suite, and uploads screenshots,
Appium output, emulator logs, and logcat even when the test fails.

Before release, repeat the lifecycle/Keystore/notification checks on at least
one arm64 physical device. An emulator cannot represent vendor background
restrictions, hardware-backed Keystore differences, OEM WebView behavior, or
real memory/GPU pressure.

## Fixtures and artifacts

- `index.e2e.html` is a test-only Trunk entry point.
- `e2e/browser-bridge.js` implements deterministic Tauri command and event
  fixtures before the WASM application starts.
- `dist-e2e/` and `e2e/artifacts/` are ignored build/test output.
- `e2e/.appium/` is ignored project-local Appium driver state.
- A screenshot is stored in `e2e/artifacts/` when a test fails.
- Native backend output is stored as `e2e/artifacts/native-backend.log`.
- Android logcat is stored as `e2e/artifacts/android-logcat.txt`.

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

### Provider-specific live smoke

Sakana and Z.ai live checks remain opt-in and never accept an API key through a
file or command argument. To exercise only the Z.ai GLM Coding Plan Chat
Completions path after storing a Coding Plan API key in the native credential
store:

```bash
SUISOU_E2E_LIVE=1 npm run e2e:live:glm
```

The full `npm run e2e:live` suite still includes the legacy Sakana smoke. When
Sakana quota is unavailable, run the GLM-specific command above instead of the
full live suite.
