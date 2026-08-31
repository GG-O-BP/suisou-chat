#!/usr/bin/env bash

# Manage the Android emulator used by the Android APK E2E layer.
#
#   scripts/android-emulator.sh ensure-avd   # create the AVD if it is missing
#   scripts/android-emulator.sh start        # boot the AVD headless and wait
#   scripts/android-emulator.sh wait         # wait until sys.boot_completed=1
#   scripts/android-emulator.sh stop         # shut the AVD down
#   scripts/android-emulator.sh status       # print adb state / boot status
#
# The script is deliberately idempotent so it can run locally and in CI.
# It never provisions a real API key and never touches the desktop E2E suite.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
# shellcheck source=scripts/android-env.sh
source scripts/android-env.sh

AVD_NAME="${SUISOU_AVD_NAME:-suisou_api36}"
ANDROID_API="${SUISOU_ANDROID_API:-36}"
SYSTEM_IMAGE="${SUISOU_SYSTEM_IMAGE:-system-images;android-${ANDROID_API};google_apis;x86_64}"
AVD_DEVICE="${SUISOU_AVD_DEVICE:-pixel_5}"
EMULATOR="$ANDROID_HOME/emulator/emulator"
BOOT_TIMEOUT="${SUISOU_EMULATOR_BOOT_TIMEOUT:-300}"
EMULATOR_LOG="${SUISOU_EMULATOR_LOG:-$repo_root/logs/android-emulator.log}"

log() { printf '[android-emulator] %s\n' "$*" >&2; }

require_tool() {
  if [[ ! -x "$1" ]]; then
    log "required tool not found: $1"
    exit 1
  fi
}

ensure_avd() {
  require_tool "$EMULATOR"
  if "$EMULATOR" -list-avds 2>/dev/null | grep -qx "$AVD_NAME"; then
    log "AVD '$AVD_NAME' already exists"
    return 0
  fi
  log "creating AVD '$AVD_NAME' from '$SYSTEM_IMAGE'"
  "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --install "$SYSTEM_IMAGE" >/dev/null
  echo "no" | "$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" create avd \
    --name "$AVD_NAME" \
    --package "$SYSTEM_IMAGE" \
    --device "$AVD_DEVICE" \
    --force
}

is_running() {
  adb devices 2>/dev/null | grep -qE '^emulator-[0-9]+\s+device$'
}

wait_for_boot() {
  log "waiting up to ${BOOT_TIMEOUT}s for boot to complete"
  adb start-server >/dev/null 2>&1 || true
  local deadline=$(( $(date +%s) + BOOT_TIMEOUT ))
  if ! timeout "$BOOT_TIMEOUT" adb wait-for-device; then
    log "emulator did not appear in adb within ${BOOT_TIMEOUT}s"
    return 1
  fi
  while (( $(date +%s) < deadline )); do
    local booted
    booted="$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || true)"
    if [[ "$booted" == "1" ]]; then
      adb shell input keyevent 82 >/dev/null 2>&1 || true
      log "boot completed (API $(adb shell getprop ro.build.version.sdk | tr -d '\r'))"
      return 0
    fi
    sleep 3
  done
  log "emulator did not finish booting within ${BOOT_TIMEOUT}s"
  return 1
}

start_emulator() {
  ensure_avd
  if is_running; then
    log "emulator already running"
    wait_for_boot
    return 0
  fi
  mkdir -p "$(dirname "$EMULATOR_LOG")"
  log "starting AVD '$AVD_NAME' headless (log: $EMULATOR_LOG)"
  local -a extra_args=()
  if [[ -n "${SUISOU_EMULATOR_EXTRA_ARGS:-}" ]]; then
    # Intended for simple flag/value additions in CI, not shell expressions.
    read -r -a extra_args <<<"$SUISOU_EMULATOR_EXTRA_ARGS"
  fi
  # setsid + no controlling terminal keeps the emulator alive after this
  # script returns so the WDIO run can attach to it.
  setsid "$EMULATOR" -avd "$AVD_NAME" \
    -no-window -no-audio -no-boot-anim -no-snapshot \
    -gpu swiftshader_indirect -no-metrics \
    "${extra_args[@]}" \
    >"$EMULATOR_LOG" 2>&1 &
  disown || true
  wait_for_boot
}

stop_emulator() {
  if ! is_running; then
    log "no emulator running"
    return 0
  fi
  log "stopping emulator"
  adb emu kill >/dev/null 2>&1 || adb -e emu kill >/dev/null 2>&1 || true
  for _ in $(seq 1 20); do
    is_running || { log "emulator stopped"; return 0; }
    sleep 1
  done
  log "emulator still running after stop request"
  return 1
}

status() {
  adb start-server >/dev/null 2>&1 || true
  printf 'devices:\n'
  adb devices
  printf 'boot_completed=%s\n' "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || echo '')"
}

case "${1:-start}" in
  ensure-avd) ensure_avd ;;
  start) start_emulator ;;
  wait) wait_for_boot ;;
  stop) stop_emulator ;;
  status) status ;;
  *)
    log "usage: $0 {ensure-avd|start|wait|stop|status}"
    exit 2
    ;;
esac
