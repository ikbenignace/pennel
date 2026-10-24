#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pkg="$root/apps/macos"
app="$root/build/Pennel.app"
# Prefer the stable local certificate so macOS keeps Pennel's permissions across rebuilds.
# "Musli Local Dev" is the certificate made before the rename; keep using it so granted permissions survive.
default_identity="-"
for candidate in "Pennel Local Dev" "Musli Local Dev"; do
  if security find-identity -v -p codesigning 2>/dev/null | grep -q "\"$candidate\""; then
    default_identity="$candidate"
    break
  fi
done
identity="${PENNEL_SIGN_IDENTITY:-$default_identity}"

swift build -c release --package-path "$pkg"
bin="$(swift build -c release --package-path "$pkg" --show-bin-path)/Pennel"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/Fonts"
cp "$bin" "$app/Contents/MacOS/Pennel"
strip -x "$app/Contents/MacOS/Pennel"
cp "$pkg/Resources/Info.plist" "$app/Contents/Info.plist"
cp "$pkg/Resources/Fonts/"* "$app/Contents/Resources/Fonts/"

codesign --force --options runtime \
  --entitlements "$pkg/Resources/Pennel.entitlements" \
  --sign "$identity" "$app"

if [ "$identity" = "-" ]; then
  echo "warning: ad-hoc signed; macOS forgets Pennel's permissions after every build." >&2
  echo "         run scripts/create-dev-cert.sh once and set PENNEL_SIGN_IDENTITY=\"Pennel Local Dev\"." >&2
fi
echo "built $app"
