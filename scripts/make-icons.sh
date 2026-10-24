#!/bin/sh
# Renders the brand SVGs into the app icon and menubar images bundled with the app (needs librsvg: brew install librsvg).
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
brand="$root/assets/brand"
res="$root/apps/macos/Resources"
set_dir="$(mktemp -d)/Pennel.iconset"
mkdir -p "$set_dir"
for size in 16 32 128 256 512; do
  rsvg-convert -w "$size" -h "$size" "$brand/app-icon.svg" -o "$set_dir/icon_${size}x${size}.png"
  rsvg-convert -w $((size * 2)) -h $((size * 2)) "$brand/app-icon.svg" -o "$set_dir/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$set_dir" -o "$res/Pennel.icns"
rsvg-convert -w 18 -h 18 "$brand/menubar.svg" -o "$res/MenubarTemplate.png"
rsvg-convert -w 36 -h 36 "$brand/menubar.svg" -o "$res/MenubarTemplate@2x.png"
rsvg-convert -h 48 "$brand/wordmark.svg" -o "$res/Wordmark.png"
rsvg-convert -h 96 "$brand/wordmark.svg" -o "$res/Wordmark@2x.png"
echo "icons written to $res"
