#!/usr/bin/env bash
# Install (or update) rustup's stable toolchain for this runner process.
#
# The box runs several GitHub runner processes as the same user. They used to
# share ~/.rustup, so one job's `toolchain install` / wipe removed rustc from
# under another's cargo (`never executed` / ENOENT on rustc). Isolate self-hosted
# toolchains by runner; hosted VMs need stable paths for Rust cache reuse.
#
# Extra arguments are rustup components (clippy, rustfmt, ...).
set -euo pipefail

export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
SHARED_CARGO_HOME="$CARGO_HOME"
# Same workaround dtolnay/rust-toolchain uses: copy when rename hits
# "Directory not empty" on a toolchain dir.
export RUSTUP_PERMIT_COPY_RENAME=1

runner_id="${RUNNER_NAME:-unknown}"
if [ -n "${GITHUB_WORKSPACE:-}" ]; then
  # GITHUB_WORKSPACE is <runner-root>/_work/<repo>/<repo>
  prefix=$(cd "$GITHUB_WORKSPACE/../../.." && pwd)
  runner_id="${runner_id}-$(basename "$prefix")"
fi
runner_id=$(printf '%s' "$runner_id" | tr -c 'A-Za-z0-9._-' '-')
if [ "${RUNNER_ENVIRONMENT:-}" = "self-hosted" ]; then
  export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup/runners/$runner_id}"
  # The registry sources in a shared CARGO_HOME are extracted and pruned by
  # whichever runner finishes first, which removed crate files (and
  # `rustc` runs) from under a concurrent job. Each runner process gets its
  # own CARGO_HOME; the rustup binary is only linked in as the proxies.
  export CARGO_HOME="$HOME/.cargo-runners/$runner_id"
  mkdir -p "$CARGO_HOME/bin"
  if [ ! -x "$SHARED_CARGO_HOME/bin/rustup" ]; then
    mkdir -p "$SHARED_CARGO_HOME/bin"
    curl --proto '=https' --tlsv1.2 --retry 10 --retry-connrefused --location --silent --show-error --fail https://sh.rustup.rs \
      | CARGO_HOME="$SHARED_CARGO_HOME" sh -s -- --default-toolchain none -y --no-modify-path
  fi
  for proxy in rustup cargo rustc rustdoc rustfmt cargo-fmt cargo-clippy clippy-driver rust-gdb rust-lldb cargo-miri; do
    ln -sf "$SHARED_CARGO_HOME/bin/rustup" "$CARGO_HOME/bin/$proxy"
  done
else
  export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
fi

mkdir -p "$CARGO_HOME/bin" "$RUSTUP_HOME"
# Actions does not source .bashrc, so rustup at ~/.cargo/bin is invisible
# unless we put it on PATH ourselves (and for later steps, on GITHUB_PATH).
export PATH="$CARGO_HOME/bin:$PATH"
if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$CARGO_HOME/bin" >> "$GITHUB_PATH"
fi
if [ -n "${GITHUB_ENV:-}" ]; then
  echo "CARGO_HOME=$CARGO_HOME" >> "$GITHUB_ENV"
  echo "RUSTUP_HOME=$RUSTUP_HOME" >> "$GITHUB_ENV"
  echo "RUSTUP_TOOLCHAIN=stable" >> "$GITHUB_ENV"
fi

if ! command -v rustup >/dev/null; then
  curl --proto '=https' --tlsv1.2 --retry 10 --retry-connrefused --location --silent --show-error --fail https://sh.rustup.rs \
    | sh -s -- --default-toolchain none -y --no-modify-path
fi

lock="$RUSTUP_HOME/pennel-ci.lock"

run_install() {
  if (($#)); then
    local args=() c
    for c in "$@"; do
      args+=(--component "$c")
    done
    rustup toolchain install stable --profile minimal --no-self-update "${args[@]}"
  else
    rustup toolchain install stable --profile minimal --no-self-update
  fi
}

exec 9>"$lock"
if ! flock -w 300 9; then
  echo "timed out waiting for rustup lock ($lock)" >&2
  exit 1
fi

set +e
run_install "$@"
st=$?
set -e
if [ "$st" -ne 0 ]; then
  echo "rustup install failed; wiping this runner's stable toolchain and retrying" >&2
  rustup toolchain uninstall stable || true
  rm -rf "$RUSTUP_HOME"/tmp/* "$RUSTUP_HOME"/toolchains/stable-*
  run_install "$@"
fi
rustup default stable

rustc_path=$(rustup which rustc)
if [ ! -x "$rustc_path" ]; then
  echo "rustc missing after install: $rustc_path" >&2
  exit 1
fi
rustc --version
