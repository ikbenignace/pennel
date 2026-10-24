#!/bin/sh
# Refreshes core/src/catalog.json from the @handy-computer/models npm package.
set -eu
version="${1:?usage: scripts/update-catalog.sh <version>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
(cd "$tmp" && npm pack "@handy-computer/models@$version" --silent >/dev/null && tar xzf ./*.tgz)
cp "$tmp/package/catalog.json" "$root/core/src/catalog.json"
echo "catalog.json updated to @handy-computer/models@$version"
