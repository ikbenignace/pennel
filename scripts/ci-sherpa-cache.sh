#!/usr/bin/env bash
# sherpa-onnx-sys downloads its native libraries into <target>/sherpa-onnx-prebuilt and only reruns its
# build script when an environment variable changes. A restored Rust cache can carry the build script's
# output while the libraries it points at are missing or unusable, and the link step then fails. Always
# drop that output so the build script runs again; it reuses a complete download and replaces a broken one.
set -euo pipefail
for target in "$@"; do
  [ -d "$target" ] || continue
  find "$target" -maxdepth 4 -type d \( -path "*/build/sherpa-onnx-sys-*" -o -path "*/.fingerprint/sherpa-onnx-sys-*" \) -prune -exec rm -rf {} + 2>/dev/null || true
  if [ -d "$target/sherpa-onnx-prebuilt" ] && ! find "$target/sherpa-onnx-prebuilt" \( -name 'libsherpa-onnx-c-api.a' -o -name 'sherpa-onnx-c-api.lib' -o -name 'libsherpa-onnx-c-api.so' \) | grep -q .; then
    rm -rf "$target/sherpa-onnx-prebuilt"
  fi
done
