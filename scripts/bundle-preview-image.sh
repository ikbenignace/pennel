#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if [ "$(uname -s)" != Darwin ]; then
  echo "Preview disk images require macOS." >&2
  exit 1
fi
app="$root/apps/desktop/src-tauri/target/release/bundle/macos/Pennel Preview.app"
if [ ! -d "$app" ]; then
  echo "Build the shared preview app before creating its disk image." >&2
  exit 1
fi
identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")"
if [ "$identifier" != dev.pennel.Pennel.preview ]; then
  echo "Refusing to sign a non-preview application." >&2
  exit 1
fi
pennel_preview_identity="$(python3 "$root/scripts/preview-signing-identity.py")"
# The linker's ad-hoc signature does not seal the bundle resources or bind its identity.
codesign --force --options runtime --timestamp=none --sign "$pennel_preview_identity" \
  --entitlements "$root/apps/desktop/src-tauri/Pennel.entitlements" "$app"
codesign --verify --deep --strict "$app"
mkdir -p "$root/build"
stage="$(mktemp -d "$root/build/preview-image-XXXXXX")"
trap 'rm -rf "$stage"' EXIT
ditto "$app" "$stage/Pennel Preview.app"
ln -s /Applications "$stage/Applications"
# Finder layout automation requires an interactive desktop; the preview image does not.
image="$root/build/Pennel Preview.dmg"
hdiutil create -quiet -volname "Pennel Preview" -srcfolder "$stage" -ov -format UDZO "$image"
hdiutil verify -quiet "$image"
echo "built $image"
