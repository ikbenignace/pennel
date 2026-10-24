#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if ! command -v cargo >/dev/null && [ -x "$root/build/toolchain/cargo/bin/cargo" ]; then
  export CARGO_HOME="$root/build/toolchain/cargo"
  export RUSTUP_HOME="$root/build/toolchain/rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
if ! command -v cmake >/dev/null && [ -x "$root/build/toolchain/cmake/cmake/data/bin/cmake" ]; then
  export PATH="$root/build/toolchain/cmake/cmake/data/bin:$PATH"
fi
exec "$@"
