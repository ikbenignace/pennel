#!/usr/bin/env bash
# Builds a signed, notarized release and the Sparkle appcast. See docs/releasing.md.
#   scripts/release.sh 0.2.0             full release (Developer ID + notarization + GitHub draft release)
#   scripts/release.sh 0.2.0 --dry-run   local signing only; skips notarization and GitHub
set -euo pipefail

version="${1:?usage: scripts/release.sh <version> [--dry-run]}"
dry_run=0
[ "${2:-}" = "--dry-run" ] && dry_run=1

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/build/release/$version"
app="$root/build/Pennel.app"
sparkle_bin="$root/apps/macos/.build/artifacts/sparkle/Sparkle/bin"
download_prefix="https://github.com/pennel-app/pennel/releases/download/v$version/"

if [ "$dry_run" = 0 ]; then
  : "${PENNEL_SIGN_IDENTITY:?set PENNEL_SIGN_IDENTITY to your \"Developer ID Application: …\" identity}"
  case "$PENNEL_SIGN_IDENTITY" in
    "Developer ID Application:"*) ;;
    *) echo "PENNEL_SIGN_IDENTITY must be a Developer ID Application identity for notarization" >&2; exit 1 ;;
  esac
  export PENNEL_RELEASE=1
fi

plist="$root/apps/macos/Resources/Info.plist"
build_number="$(git -C "$root" rev-list --count HEAD)"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build_number" "$plist"

make -C "$root" core app

rm -rf "$out" && mkdir -p "$out"
zip="$out/Pennel-$version.zip"
ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"

if [ "$dry_run" = 0 ]; then
  xcrun notarytool submit "$zip" --keychain-profile pennel-notary --wait
  xcrun stapler staple "$app"
  rm "$zip"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"
fi

dmg="$out/Pennel-$version.dmg"
staging="$(mktemp -d)"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -volname "Pennel" -srcfolder "$staging" -ov -format UDZO "$dmg"
rm -rf "$staging"
if [ "$dry_run" = 0 ]; then
  codesign --force --timestamp --sign "$PENNEL_SIGN_IDENTITY" "$dmg"
  xcrun notarytool submit "$dmg" --keychain-profile pennel-notary --wait
  xcrun stapler staple "$dmg"
fi

# Sparkle updates from the zip; the dmg is only for first downloads, so it stays out of the appcast.
updates="$out/updates"
mkdir -p "$updates"
cp "$zip" "$updates/"
"$sparkle_bin/generate_appcast" --account pennel --download-url-prefix "$download_prefix" \
  --link https://pennel.app -o "$out/appcast.xml" "$updates"

if [ "$dry_run" = 0 ]; then
  gh release create "v$version" "$zip" "$dmg" --repo pennel-app/pennel --draft \
    --title "Pennel $version" --notes "See CHANGELOG"
fi

echo "release files in $out"
echo "next: publish the GitHub draft, then copy $out/appcast.xml to pennel-cloud apps/web/public/appcast.xml"
