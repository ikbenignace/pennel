#!/usr/bin/env python3
"""Prepare a Tauri update manifest from accepted signed artifacts; never upload it."""

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from urllib.parse import quote, urlparse


REQUIRED_GATES = ("macos", "windows", "linux_x11", "linux_wayland", "migration", "ui_parity", "performance", "signed_update")
REQUIRED_TARGETS = {"darwin-aarch64", "windows-x86_64", "linux-x86_64"}


def accepted(receipt, revision):
    if not revision or receipt.get("revision") != revision:
        raise ValueError("Acceptance receipt must match the candidate source revision")
    if any(receipt.get(gate) != "passed" for gate in REQUIRED_GATES):
        raise ValueError("Mac, Windows, Linux X11, Linux Wayland, migration, UI, performance and signed-update acceptance must all pass")


def verify_artifacts(receipt, version, artifacts):
    if receipt.get("version") != version:
        raise ValueError("Acceptance receipt must match the candidate version")
    hashes = receipt.get("artifacts", {})
    if set(hashes) != REQUIRED_TARGETS or set(artifacts) != REQUIRED_TARGETS:
        raise ValueError("Acceptance receipt must identify the exact Mac, Windows and Linux updater artifacts by SHA-256")
    for target, artifact in artifacts.items():
        if hashes.get(target) != hashlib.sha256(artifact.read_bytes()).hexdigest():
            raise ValueError(f"Updater artifact does not match the accepted {target} candidate")


def manifest(version, base_url, artifacts, notes, timestamp):
    url = urlparse(base_url)
    if url.scheme != "https" or not url.netloc or url.query or url.fragment:
        raise ValueError("Download base must be an HTTPS directory URL")
    if set(artifacts) != REQUIRED_TARGETS:
        raise ValueError("Provide exactly one accepted artifact for darwin-aarch64, windows-x86_64 and linux-x86_64")
    platforms = {}
    for target, artifact in artifacts.items():
        if not artifact.is_file():
            raise ValueError(f"Missing updater artifact for {target}")
        if target == "darwin-aarch64" and not artifact.name.endswith(".app.tar.gz"):
            raise ValueError("Mac updater artifact must be the signed .app.tar.gz")
        if target == "windows-x86_64" and artifact.suffix != ".exe":
            raise ValueError("Windows updater artifact must be the signed NSIS .exe")
        if target == "linux-x86_64" and artifact.suffix != ".AppImage":
            raise ValueError("Linux updater artifact must be the signed AppImage")
        signature = artifact.with_name(artifact.name + ".sig").read_text(encoding="utf-8").strip()
        if not signature or not base64.b64decode(signature, validate=True):
            raise ValueError(f"Invalid updater signature for {target}")
        platforms[target] = {"signature": signature, "url": base_url.rstrip("/") + "/" + quote(artifact.name)}
    return {"version": version, "notes": notes, "pub_date": timestamp, "platforms": platforms}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("--revision", required=True)
    parser.add_argument("--acceptance", type=Path, required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--artifact", action="append", required=True, metavar="TARGET=PATH")
    parser.add_argument("--notes", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        receipt = json.loads(args.acceptance.read_text(encoding="utf-8"))
        accepted(receipt, args.revision)
        artifacts = {}
        for entry in args.artifact:
            target, path = entry.split("=", 1)
            if target in artifacts:
                raise ValueError(f"Duplicate updater target: {target}")
            artifacts[target] = Path(path)
        verify_artifacts(receipt, args.version, artifacts)
        result = manifest(args.version, args.base_url, artifacts, args.notes.read_text(encoding="utf-8"), datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"Prepared {args.output}; publication is a separate reviewed action")
    except (OSError, ValueError) as error:
        parser.exit(1, f"Update manifest rejected: {error}\n")


if __name__ == "__main__":
    main()
