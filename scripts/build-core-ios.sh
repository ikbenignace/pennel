#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
export IPHONEOS_DEPLOYMENT_TARGET=26.0
device_target=aarch64-apple-ios
sim_target=aarch64-apple-ios-sim
device_dir="$root/core/target/$device_target/release"
sim_dir="$root/core/target/$sim_target/release"
bindings="$root/build/bindings-ios"
mac_bindings="$root/build/bindings"
framework="$root/apps/ios/Frameworks/PennelCoreFFI.xcframework"

for target in "$device_target" "$sim_target"; do
  cargo build --manifest-path "$root/core/Cargo.toml" --release --target "$target" --lib --no-default-features
done

rm -rf "$bindings"
# Library mode reads crate metadata via `cargo metadata`, which needs the crate directory as cwd.
(cd "$root/core" && cargo run --quiet --bin uniffi-bindgen -- \
  generate --library "$sim_dir/libpennel_core.dylib" --language swift --out-dir "$bindings")

mkdir -p "$bindings/headers"
mv "$bindings/pennel_coreFFI.h" "$bindings/headers/"
mv "$bindings/pennel_coreFFI.modulemap" "$bindings/headers/module.modulemap"

rm -rf "$framework"
mkdir -p "$(dirname "$framework")"
xcodebuild -create-xcframework \
  -library "$device_dir/libpennel_core.a" -headers "$bindings/headers" \
  -library "$sim_dir/libpennel_core.a" -headers "$bindings/headers" \
  -output "$framework" >/dev/null

mkdir -p "$root/apps/ios/PennelCore"
cp "$bindings/pennel_core.swift" "$root/apps/ios/PennelCore/"

strip_dir="$(mktemp -d)"
trap 'rm -rf "$strip_dir"' EXIT
for target in "$device_target" "$sim_target"; do
  cp "$root/core/target/$target/release/libpennel_core.a" "$strip_dir/$target.a"
  strip -S -x "$strip_dir/$target.a" 2>/dev/null
  echo "stripped libpennel_core.a ($target): $(stat -f %z "$strip_dir/$target.a") bytes"
done

if [ -f "$mac_bindings/pennel_core.swift" ]; then
  if ! diff -q "$mac_bindings/pennel_core.swift" "$bindings/pennel_core.swift" >/dev/null; then
    echo "iOS and macOS uniffi bindings differ; the API surface must be identical" >&2
    diff "$mac_bindings/pennel_core.swift" "$bindings/pennel_core.swift" | head -40 >&2
    exit 1
  fi
  echo "bindings identical to the macOS build"
else
  echo "warning: build/bindings/pennel_core.swift missing; run 'make core' to check the bindings" >&2
fi
echo "built $framework"
