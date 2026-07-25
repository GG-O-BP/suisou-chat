#!/usr/bin/env bash
set -euo pipefail

# Debug builds should stay readable and fast. Release builds use Bun because
# Trunk 0.21.14's minify-js 0.5.6 does not accept wasm-bindgen 0.2.126's ESM.
if [[ "${TRUNK_PROFILE:-debug}" != "release" ]]; then
  exit 0
fi

if ! command -v bun >/dev/null 2>&1; then
  printf '%s\n' "Bun is required to minify wasm-bindgen JavaScript in release builds." >&2
  exit 1
fi

shopt -s nullglob
modules=("${TRUNK_STAGING_DIR}"/*.js)
if (( ${#modules[@]} != 1 )); then
  printf 'Expected exactly one generated JavaScript module in %s, found %d.\n' \
    "${TRUNK_STAGING_DIR}" "${#modules[@]}" >&2
  exit 1
fi

module="${modules[0]}"
temporary="${module}.min"

bun build "$module" \
  --target=browser \
  --format=esm \
  --minify \
  --external='*' \
  --outfile="$temporary" \
  >/dev/null 2>&1

mv "$temporary" "$module"
