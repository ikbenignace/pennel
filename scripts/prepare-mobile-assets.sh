#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
output="$root/apps/mobile/artifacts/brand"
if ! command -v rsvg-convert >/dev/null; then
  echo "Pennel icon preparation uses the repository's librsvg dependency (rsvg-convert)." >&2
  exit 1
fi
mkdir -p "$output"
rsvg-convert -w 1024 -h 1024 "$root/assets/brand/app-icon.svg" -o "$output/app-icon.png"
