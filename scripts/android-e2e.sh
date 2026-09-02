#!/usr/bin/env bash

# Orchestrate the Android APK E2E layer end to end:
#   1. build the x86_64 debug APK (unless SUISOU_SKIP_APK_BUILD=1)
#   2. make sure the Appium UiAutomator2 driver is installed locally
#   3. boot the headless emulator and wait for it
#   4. run the WebdriverIO Android suite
#   5. always collect logcat into e2e/artifacts
#
# This layer is part of the required `npm run e2e` suite. It never uses a real
# API key: the app is exercised in its deterministic "no API key" state.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
# shellcheck source=scripts/android-env.sh
source scripts/android-env.sh

export APPIUM_HOME="${APPIUM_HOME:-$repo_root/e2e/.appium}"
artifact_dir="$repo_root/e2e/artifacts"
appium_bin="$repo_root/node_modules/.bin/appium"
mkdir -p "$artifact_dir" "$APPIUM_HOME"

log() { printf '[android-e2e] %s\n' "$*" >&2; }

if [[ ! -x "$appium_bin" ]]; then
  log "appium is not installed. Run: npm install"
  exit 1
fi

# The UiAutomator2 driver version is pinned to the last release that supports
# the Appium 2.x server line (peer: ^2.4.1 || ^3.0.0-beta.0).
UIAUTOMATOR2_VERSION="${SUISOU_UIAUTOMATOR2_VERSION:-4.2.3}"

# Install the UiAutomator2 driver into the project-local APPIUM_HOME once.
if ! "$appium_bin" driver list --installed 2>&1 | grep -q 'uiautomator2'; then
  log "installing appium uiautomator2@${UIAUTOMATOR2_VERSION} driver into $APPIUM_HOME"
  "$appium_bin" driver install "uiautomator2@${UIAUTOMATOR2_VERSION}" >&2
fi

# 1. Build the APK unless the caller reuses an existing one.
if [[ "${SUISOU_SKIP_APK_BUILD:-0}" != "1" ]]; then
  # Trunk/Gradle write informational output to stdout, so do not capture the
  # build command's stdout as the APK path.
  scripts/android-build-e2e-apk.sh >&2
  SUISOU_ANDROID_APK="$repo_root/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
  export SUISOU_ANDROID_APK
  log "using freshly built APK: $SUISOU_ANDROID_APK"
fi

# 2. Boot the emulator (idempotent) unless the caller manages it.
if [[ "${SUISOU_SKIP_EMULATOR:-0}" != "1" ]]; then
  scripts/android-emulator.sh start
fi

app_package="com.ggobp.suisou_chat"
log "uninstalling $app_package before the deterministic smoke run"
# The AVD retains installed APKs across cold boots. Appium treats an equal
# versionCode as "no upgrade needed", so an old package would otherwise survive
# a fresh build. Removing it guarantees the newly built APK is installed.
if adb shell pm path "$app_package" >/dev/null 2>&1; then
  adb uninstall "$app_package" >/dev/null
fi

collect_logcat() {
  if adb get-state >/dev/null 2>&1; then
    adb logcat -d > "$artifact_dir/android-logcat.txt" 2>/dev/null || true
    log "saved logcat to $artifact_dir/android-logcat.txt"
  fi
}
trap collect_logcat EXIT

# 3. Run the WebdriverIO Android suite.
log "running WebdriverIO Android suite"
npx wdio run e2e/wdio.android.conf.mjs
