#!/usr/bin/env bash

# Build the x86_64 debug APK used by the Android emulator E2E layer.
#
# The Android E2E layer drives the *normal* debug APK as a black box through
# Appium/UiAutomator2. It intentionally does NOT enable the `e2e` Cargo feature
# or the embedded WebDriver server, so no test-only automation server or
# capability ships inside the APK. Debug APKs are already WebView-debuggable,
# which is what lets Appium expose the WEBVIEW_* context.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
# shellcheck source=scripts/android-env.sh
source scripts/android-env.sh

TARGET_ABI="${SUISOU_ANDROID_ABI:-x86_64}"
APK_PATH="src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"

printf '[android-build-e2e-apk] building %s debug APK\n' "$TARGET_ABI" >&2
cargo tauri android build --debug --apk --target "$TARGET_ABI" --ci

if [[ ! -f "$APK_PATH" ]]; then
  printf '[android-build-e2e-apk] expected APK not found at %s\n' "$APK_PATH" >&2
  exit 1
fi

printf '%s\n' "$repo_root/$APK_PATH"
