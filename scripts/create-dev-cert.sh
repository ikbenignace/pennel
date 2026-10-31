#!/usr/bin/env bash
# Creates a self-signed code-signing certificate so local builds keep a stable identity
# and macOS remembers microphone/accessibility permissions across rebuilds.
set -euo pipefail

name="Pennel Local Dev"
if security find-identity -v -p codesigning | grep -q "$name"; then
  echo "\"$name\" already exists"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $name
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
CNF

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$tmp/key.pem" -out "$tmp/cert.pem" -config "$tmp/cert.cnf"
openssl pkcs12 -export -legacy -inkey "$tmp/key.pem" -in "$tmp/cert.pem" \
  -out "$tmp/cert.p12" -passout pass:pennel

keychain="$HOME/Library/Keychains/login.keychain-db"
security import "$tmp/cert.p12" -k "$keychain" -P pennel -T /usr/bin/codesign
security add-trusted-cert -r trustRoot -p codeSign -k "$keychain" "$tmp/cert.pem"

echo "created \"$name\"; build with: APPLE_SIGNING_IDENTITY=\"$name\" make desktop"
