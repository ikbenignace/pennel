#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
runner="$root/build/native-reference-runner"
output="${1:-$root/build/visual-parity/native}"
temporary="${TMPDIR:-/tmp}"
fixture="$(mktemp -d "$temporary/pennel-native-reference-XXXXXX")"
trap 'rm -rf "$fixture"' EXIT

if [ ! -d "$root/apps/macos/Frameworks/PennelCoreFFI.xcframework" ] || [ ! -f "$root/apps/macos/Sources/PennelCore/pennel_core.swift" ]; then
  "$root/scripts/with-toolchain.sh" make -C "$root" core
fi

mkdir -p "$runner/Sources/PennelSnapshot/Windows" "$runner/Sources/PennelSnapshot/Meetings" "$runner/Frameworks" "$output"
cp -R "$root/apps/macos/Sources/PennelKit" "$runner/Sources/"
cp -R "$root/apps/macos/Sources/PennelCore" "$runner/Sources/"
ditto "$root/apps/macos/Frameworks/PennelCoreFFI.xcframework" "$runner/Frameworks/PennelCoreFFI.xcframework"
cp "$root/apps/macos/Sources/Pennel/AppState.swift" "$root/apps/macos/Sources/Pennel/ModelDownloads.swift" "$root/apps/macos/Sources/Pennel/WindowSnapshot.swift" "$runner/Sources/PennelSnapshot/"
cp -L "$root/apps/macos/Sources/Pennel/FoundationLanguageModel.swift" "$runner/Sources/PennelSnapshot/"
cp "$root/apps/macos/Sources/Pennel/Windows/"*.swift "$runner/Sources/PennelSnapshot/Windows/"
for source in LibraryWindow MeetingViews MeetingChat MeetingChatView; do
  cp "$root/apps/macos/Sources/Pennel/Meetings/$source.swift" "$runner/Sources/PennelSnapshot/Meetings/"
done
cat > "$runner/Package.swift" <<'SWIFT'
// swift-tools-version: 6.0
import PackageDescription
let package = Package(
    name: "PennelReferenceSnapshot",
    platforms: [.macOS("14.4")],
    targets: [
        .binaryTarget(name: "PennelCoreFFI", path: "Frameworks/PennelCoreFFI.xcframework"),
        .target(name: "PennelCore", dependencies: ["PennelCoreFFI"], swiftSettings: [.swiftLanguageMode(.v5)], linkerSettings: [.linkedFramework("Metal"), .linkedFramework("MetalKit"), .linkedFramework("Accelerate"), .linkedFramework("Foundation"), .linkedLibrary("c++")]),
        .target(name: "PennelKit"),
        .executableTarget(name: "PennelSnapshot", dependencies: ["PennelCore", "PennelKit"], swiftSettings: [.define("PENNEL_REFERENCE_SNAPSHOT")])
    ]
)
SWIFT

swift build --package-path "$runner" --configuration debug --jobs 4 > "$root/build/native-reference-snapshot-build.log" 2>&1
bin="$(swift build --package-path "$runner" --configuration debug --show-bin-path)/PennelSnapshot"
app="$runner/PennelSnapshot.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/Fonts"
cp "$bin" "$app/Contents/MacOS/PennelSnapshot"
cp "$root/apps/macos/Resources/Fonts/"*.ttf "$app/Contents/Resources/Fonts/"
cp "$root/apps/macos/Resources/Wordmark"*.png "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.pennel.reference-snapshot</string>
<key>CFBundleExecutable</key><string>PennelSnapshot</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSUIElement</key><true/>
<key>ATSApplicationFontsPath</key><string>Fonts</string>
</dict></plist>
PLIST
codesign --force --sign - "$app" > "$root/build/native-reference-snapshot-sign.log" 2>&1
python3 - "$runner/sandbox.sb" <<'PY'
import json
from pathlib import Path
import sys

user = Path.home()
protected = [
    user / "Library/Application Support/Pennel",
    user / "Library/Application Support/Pennel Desktop",
    user / "Library/Application Support/Pennel Desktop Preview",
    user / "Library/Logs/Pennel",
    user / "Library/Keychains",
    user / "Pennel",
]
rules = ["(version 1)", "(allow default)", "(deny network*)"]
rules += [f"(deny file-read* file-write* (subpath {json.dumps(str(path))}))" for path in protected]
rules += [f"(deny file-write* (literal {json.dumps(str(user / 'Library/Preferences' / name))}))"
          for name in ["dev.pennel.Pennel.plist", "dev.pennel.Pennel.preview.plist"]]
Path(sys.argv[1]).write_text("\n".join(rules) + "\n")
PY
/usr/bin/sandbox-exec -f "$runner/sandbox.sb" "$app/Contents/MacOS/PennelSnapshot" "$output" "$fixture" > "$root/build/native-reference-snapshot-run.log" 2>&1
printf 'Native reference snapshots: %s\n' "$output"
