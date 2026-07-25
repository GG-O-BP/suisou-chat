#!/usr/bin/env bash

# Source this file before running Tauri Android commands:
#   source scripts/android-env.sh

export JAVA_HOME="${JAVA_HOME_ANDROID:-/usr/lib/jvm/java-21-openjdk}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/29.0.13846066}"

android_toolchain="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/bin:$android_toolchain:$PATH"

# Trunk 0.21 interprets NO_COLOR as a boolean and rejects the common value "1".
if [[ "${NO_COLOR:-}" == "1" ]]; then
  export NO_COLOR=true
fi

for required in \
  "$JAVA_HOME/bin/java" \
  "$ANDROID_HOME/platforms/android-36/android.jar" \
  "$ANDROID_HOME/platform-tools/adb" \
  "$NDK_HOME/source.properties"; do
  if [[ ! -e "$required" ]]; then
    printf 'Android build dependency not found: %s\n' "$required" >&2
    return 1 2>/dev/null || exit 1
  fi
done

unset android_toolchain required
