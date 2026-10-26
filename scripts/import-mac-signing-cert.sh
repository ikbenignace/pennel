#!/usr/bin/env bash
# Imports the self-signed release certificate into a throwaway keychain so codesign can use it.
# Tauri's own APPLE_CERTIFICATE import only accepts Apple-issued identities, so CI does it here.
# Usage: APPLE_CERTIFICATE=<base64 p12> APPLE_CERTIFICATE_PASSWORD=... import-mac-signing-cert.sh <identity> <keychain>
set -euo pipefail

identity="$1"
keychain="$2"
: "${APPLE_CERTIFICATE:?}" "${APPLE_CERTIFICATE_PASSWORD:?}"

keychain_password="$(openssl rand -hex 24)"
p12="$(mktemp)"
trap 'rm -f "$p12"' EXIT
printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$p12"

security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -t 3600 -u "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$p12" -k "$keychain" -f pkcs12 -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keychain_password" "$keychain" >/dev/null

existing=()
while IFS= read -r line; do
  line="${line#"${line%%[![:space:]]*}"}"
  line="${line//\"/}"
  [ -n "$line" ] && existing+=("$line")
done < <(security list-keychains -d user)
security list-keychains -d user -s "$keychain" "${existing[@]}"

# Self-signed, so not "valid" for find-identity -v; codesign signs with it regardless.
if ! security find-identity -p codesigning "$keychain" | grep -qF "\"$identity\""; then
  echo "certificate does not contain the code-signing identity \"$identity\"" >&2
  exit 1
fi
