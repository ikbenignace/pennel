#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pkg="$root/apps/macos"
preview="${PENNEL_MAC_PREVIEW:-0}"
if [ "${1:-}" = "--preview" ]; then
  preview=1
elif [ "$#" -ne 0 ]; then
  echo "usage: scripts/bundle-app.sh [--preview]" >&2
  exit 2
fi
app="$root/build/Pennel.app"
if [ "$preview" = 1 ]; then
  app="$root/build/Pennel Native Preview.app"
fi
if [ "$preview" = 1 ]; then
  identity="$(python3 "$root/scripts/preview-signing-identity.py")"
else
  # Keep the canonical app's established signing selection.
  default_identity="-"
  for candidate in "Pennel Local Dev" "Musli Local Dev"; do
    if security find-identity -v -p codesigning 2>/dev/null | grep -q "\"$candidate\""; then
      default_identity="$candidate"
      break
    fi
  done
  identity="${PENNEL_SIGN_IDENTITY:-$default_identity}"
fi

swift build -c release --package-path "$pkg"
bin="$(swift build -c release --package-path "$pkg" --show-bin-path)/Pennel"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/Fonts" "$app/Contents/Resources/Icons"
cp "$bin" "$app/Contents/MacOS/Pennel"
strip -x "$app/Contents/MacOS/Pennel"
cp "$pkg/Resources/Info.plist" "$app/Contents/Info.plist"
if [ "$preview" = 1 ]; then
  /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier dev.pennel.Pennel.native.preview" "$app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleName Pennel Native Preview" "$app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName Pennel Native Preview" "$app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Set :SUEnableAutomaticChecks false" "$app/Contents/Info.plist"
  /usr/libexec/PlistBuddy -c "Delete :SUFeedURL" "$app/Contents/Info.plist"
fi
cp "$root/assets/brand/fonts/"* "$app/Contents/Resources/Fonts/"
cp "$root/assets/ui/icons/"*.svg "$app/Contents/Resources/Icons/"
cp "$pkg/Resources/Pennel.icns" "$pkg/Resources/MenubarTemplate.png" "$pkg/Resources/MenubarTemplate@2x.png" \
  "$pkg/Resources/Wordmark.png" "$pkg/Resources/Wordmark@2x.png" "$app/Contents/Resources/"

mkdir -p "$app/Contents/Frameworks"
ditto "$(dirname "$bin")/Sparkle.framework" "$app/Contents/Frameworks/Sparkle.framework"

# Sparkle's helpers must be signed inside-out with our identity (notarization rejects --deep).
sparkle="$app/Contents/Frameworks/Sparkle.framework/Versions/B"
# Notarization needs a secure timestamp; local builds skip the network round-trip.
timestamp="--timestamp=none"
[ "${PENNEL_RELEASE:-}" = 1 ] && timestamp="--timestamp"
sign() { codesign --force --options runtime "$timestamp" --sign "$identity" "$@"; }
sign "$sparkle/XPCServices/Installer.xpc"
sign --preserve-metadata=entitlements "$sparkle/XPCServices/Downloader.xpc"
sign "$sparkle/Autoupdate"
sign "$sparkle/Updater.app"
sign "$app/Contents/Frameworks/Sparkle.framework"
entitlements="$pkg/Resources/Pennel.entitlements"
if [ "${PENNEL_RELEASE:-}" != 1 ]; then
  # A local certificate has no Team ID, so the hardened runtime would refuse the embedded Sparkle
  # framework; Developer ID releases share one Team ID and keep library validation on.
  entitlements="$(mktemp -t pennel-entitlements).plist"
  cp "$pkg/Resources/Pennel.entitlements" "$entitlements"
  /usr/libexec/PlistBuddy -c "Add :com.apple.security.cs.disable-library-validation bool true" "$entitlements"
fi
sign --entitlements "$entitlements" "$app"

if [ "$identity" = "-" ] && [ "$preview" != 1 ]; then
  echo "warning: ad-hoc signed; macOS forgets Pennel's permissions after every build." >&2
  echo "         run scripts/create-dev-cert.sh once and set PENNEL_SIGN_IDENTITY=\"Pennel Local Dev\"." >&2
fi
echo "built $app"
