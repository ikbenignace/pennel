#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pkg="$root/apps/macos"
app="$root/build/Musli.app"
identity="${MUSLI_SIGN_IDENTITY:--}"

swift build -c release --package-path "$pkg"
bin="$(swift build -c release --package-path "$pkg" --show-bin-path)/Musli"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/Fonts"
cp "$bin" "$app/Contents/MacOS/Musli"
cp "$pkg/Resources/Info.plist" "$app/Contents/Info.plist"
cp "$pkg/Resources/Fonts/"* "$app/Contents/Resources/Fonts/"

codesign --force --options runtime \
  --entitlements "$pkg/Resources/Musli.entitlements" \
  --sign "$identity" "$app"

if [ "$identity" = "-" ]; then
  echo "warning: ad-hoc signed; macOS forgets Musli's permissions after every build." >&2
  echo "         run scripts/create-dev-cert.sh once and set MUSLI_SIGN_IDENTITY=\"Musli Local Dev\"." >&2
fi
echo "built $app"
