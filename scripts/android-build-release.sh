#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
source scripts/android-env.sh

cargo tauri android build --apk --aab --target aarch64 --ci
