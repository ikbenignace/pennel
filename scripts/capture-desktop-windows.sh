#!/usr/bin/env bash
# Captures only windows of an isolated synthetic-data capture application.
# Build: cd apps/desktop && npx tauri build --bundles app --config src-tauri/tauri.capture.conf.json
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
exec python3 "$root/scripts/capture-desktop-windows.py" "$@"
