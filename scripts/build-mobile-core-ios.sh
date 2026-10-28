#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
artifacts="$root/apps/mobile/artifacts/ios"

PENNEL_SKIP_MAC_BINDINGS_CHECK=1 "$root/scripts/build-core-ios.sh"
mkdir -p "$artifacts/PennelCore"
cp "$root/apps/ios/PennelCore/pennel_core.swift" "$artifacts/PennelCore/"
rm -rf "$artifacts/PennelCoreFFI.xcframework"
cp -R "$root/apps/ios/Frameworks/PennelCoreFFI.xcframework" "$artifacts/"
python3 - "$root" "$artifacts" <<'PY'
import hashlib,json,pathlib,subprocess,sys
root,artifacts=map(pathlib.Path,sys.argv[1:])
digest=hashlib.sha256()
for p in sorted((root/'core').rglob('*')):
 if p.is_file() and 'target' not in p.parts and (p.suffix in {'.rs','.toml'} or p.name=='Cargo.lock'):
  digest.update(str(p.relative_to(root)).encode())
  digest.update(p.read_bytes())
manifest={'schema':1,'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'core_sha256':digest.hexdigest(),'bindings_sha256':hashlib.sha256((artifacts/'PennelCore/pennel_core.swift').read_bytes()).hexdigest(),'features':[],'targets':['aarch64-apple-ios','aarch64-apple-ios-sim']}
(artifacts/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
PY
echo "Prepared matching mobile Rust libraries and Swift bindings in $artifacts"
node "$root/apps/mobile/plugins/native-sources.cjs"
