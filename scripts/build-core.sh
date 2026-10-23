#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target=aarch64-apple-darwin
lib_dir="$root/core/target/$target/release"
bindings="$root/build/bindings"
framework="$root/apps/macos/Frameworks/MusliCoreFFI.xcframework"

cargo build --manifest-path "$root/core/Cargo.toml" --release --target "$target" --lib

rm -rf "$bindings"
# Library mode reads crate metadata via `cargo metadata`, which needs the crate directory as cwd.
(cd "$root/core" && cargo run --quiet --bin uniffi-bindgen -- \
  generate --library "$lib_dir/libmusli_core.dylib" --language swift --out-dir "$bindings")

mkdir -p "$bindings/headers"
mv "$bindings/musli_coreFFI.h" "$bindings/headers/"
mv "$bindings/musli_coreFFI.modulemap" "$bindings/headers/module.modulemap"

rm -rf "$framework"
xcodebuild -create-xcframework \
  -library "$lib_dir/libmusli_core.a" \
  -headers "$bindings/headers" \
  -output "$framework" >/dev/null

mkdir -p "$root/apps/macos/Sources/MusliCore"
cp "$bindings/musli_core.swift" "$root/apps/macos/Sources/MusliCore/"
echo "built $framework"
