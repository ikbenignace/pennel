#!/usr/bin/env bash
# sherpa-onnx-sys downloads its native libraries into <target>/sherpa-onnx-prebuilt and only reruns its
# build script when an environment variable changes. A restored Rust cache can hold the build script's
# output without that directory, so the link step then fails. Drop the stale output so Cargo reruns it.
set -euo pipefail
for target in "$@"; do
  [ -d "$target" ] || continue
  if [ ! -d "$target/sherpa-onnx-prebuilt" ] || [ -z "$(ls -A "$target/sherpa-onnx-prebuilt" 2>/dev/null)" ]; then
    find "$target" -maxdepth 4 -type d \( -path "*/build/sherpa-onnx-sys-*" -o -path "*/.fingerprint/sherpa-onnx-sys-*" \) -prune -exec rm -rf {} + 2>/dev/null || true
    echo "sherpa-onnx prebuilt libraries missing in $target; its build script will run again"
  fi
done
