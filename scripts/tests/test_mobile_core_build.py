"""Exercise mobile artifact preparation without an installed native toolchain."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "build-mobile-core-ios.sh"
TOOL = '''#!/usr/bin/env python3
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
root = pathlib.Path(os.environ["PENNEL_BUILD_TEST_ROOT"])
with pathlib.Path(os.environ["PENNEL_BUILD_TEST_LOG"]).open("a") as log:
    log.write(json.dumps([name, *args]) + "\\n")
if name == "cargo" and args[0] == "build":
    target = args[args.index("--target") + 1]
    output = root / "core/target" / target / "release"
    output.mkdir(parents=True, exist_ok=True)
    (output / "libpennel_core.a").write_bytes(target.encode())
    (output / "libpennel_core.dylib").write_bytes(target.encode())
elif name == "cargo" and args[0] == "run":
    output = pathlib.Path(args[args.index("--out-dir") + 1])
    output.mkdir(parents=True, exist_ok=True)
    for file in ("pennel_core.swift", "pennel_coreFFI.h", "pennel_coreFFI.modulemap"):
        (output / file).write_text("matching " + file)
elif name == "xcodebuild":
    output = pathlib.Path(args[args.index("-output") + 1])
    output.mkdir(parents=True)
    (output / "Info.plist").write_text("matched device and simulator slices")
elif name == "git":
    print("a" * 40)
elif name == "node":
    assert (root / "apps/mobile/artifacts/ios/manifest.json").is_file()
    assert not (root / "apps/ios").exists()
    assert not (root / "apps/macos").exists()
else:
    raise SystemExit("Unexpected native build command: " + name)
'''


class MobileCoreBuildTests(unittest.TestCase):
    def test_preparation_owns_matched_artifacts_without_legacy_trees(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            scripts = root / "scripts"
            scripts.mkdir()
            shutil.copy2(SCRIPT, scripts / SCRIPT.name)
            core = root / "core"
            core.mkdir()
            source = core / "Cargo.toml"
            source.write_text("[package]\nname = 'fixture'\n")
            tools = root / "tools"
            tools.mkdir()
            for name in ("cargo", "xcodebuild", "git", "node"):
                executable = tools / name
                executable.write_text(TOOL)
                executable.chmod(0o755)
            log = root / "commands.jsonl"
            env = {**os.environ, "PATH": str(tools) + os.pathsep + os.environ["PATH"],
                   "PENNEL_BUILD_TEST_ROOT": str(root), "PENNEL_BUILD_TEST_LOG": str(log)}
            subprocess.run(["bash", str(scripts / SCRIPT.name)], env=env, check=True, capture_output=True, text=True)
            artifacts = root / "apps/mobile/artifacts/ios"
            self.assertTrue((artifacts / "PennelCoreFFI.xcframework/Info.plist").is_file())
            bindings = artifacts / "PennelCore/pennel_core.swift"
            manifest = json.loads((artifacts / "manifest.json").read_text())
            self.assertEqual(manifest["revision"], "a" * 40)
            self.assertEqual(manifest["features"], [])
            self.assertEqual(manifest["targets"], ["aarch64-apple-ios", "aarch64-apple-ios-sim"])
            self.assertEqual(manifest["bindings_sha256"], hashlib.sha256(bindings.read_bytes()).hexdigest())
            digest = hashlib.sha256(b"core/Cargo.toml" + source.read_bytes()).hexdigest()
            self.assertEqual(manifest["core_sha256"], digest)
            commands = [json.loads(line) for line in log.read_text().splitlines()]
            builds = [args for name, *args in commands if name == "cargo" and args[0] == "build"]
            self.assertEqual(len(builds), 2)
            self.assertTrue(all("--no-default-features" in args for args in builds))
            generator = next(args for name, *args in commands if name == "cargo" and args[0] == "run")
            self.assertIn("--no-default-features", generator)
            self.assertEqual(commands[-1], ["node", str(root / "apps/mobile/plugins/native-sources.cjs")])
            self.assertFalse((root / "apps/ios").exists())
            self.assertFalse((root / "apps/macos").exists())


if __name__ == "__main__":
    unittest.main()
