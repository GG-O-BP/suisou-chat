#!/usr/bin/env bash

# Source this file before running Tauri Android commands:
#   source scripts/android-env.sh

export JAVA_HOME="${JAVA_HOME_ANDROID:-/usr/lib/jvm/java-21-openjdk}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/29.0.13846066}"

# Keep avdmanager and the emulator in agreement on the AVD/user location.
# Without this, XDG_CONFIG_HOME makes avdmanager write AVDs to
# "$XDG_CONFIG_HOME/.android/avd" while the emulator only searches
# "$HOME/.android/avd", so freshly created AVDs report "Unknown AVD name".
export ANDROID_USER_HOME="${ANDROID_USER_HOME:-$HOME/.android}"

android_toolchain="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
# Command-line tools must live under cmdline-tools/latest/ so sdkmanager and
# avdmanager resolve the SDK root as "$ANDROID_HOME" instead of its parent.
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$ANDROID_HOME/cmdline-tools/latest/bin:$android_toolchain:$PATH"

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
