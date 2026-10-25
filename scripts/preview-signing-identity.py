#!/usr/bin/env python3
"""Choose an existing stable identity for isolated Mac preview builds."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


@dataclass(frozen=True)
class Identity:
    sha1: str
    name: str


@dataclass(frozen=True)
class Selection:
    identity: str
    sha1: str | None


def parse_identities(output):
    identities = []
    for line in output.splitlines():
        match = re.fullmatch(r'\s*\d+\)\s+([A-Fa-f0-9]{40})\s+"(.+)"\s*', line)
        if match:
            identity = Identity(match[1].upper(), match[2])
            if identity not in identities:
                identities.append(identity)
    return identities


def select_identity(identities, cached_sha=None, apple_override=None, pennel_override=None):
    explicit = apple_override or pennel_override
    if explicit:
        matches = [item for item in identities if explicit.upper() == item.sha1 or explicit == item.name]
        return Selection(explicit, matches[0].sha1 if len(matches) == 1 else None)
    if cached_sha:
        for identity in identities:
            if identity.sha1 == cached_sha.upper():
                return Selection(identity.sha1, identity.sha1)
    for name in ("Pennel Local Dev", "Musli Local Dev"):
        for identity in identities:
            if identity.name == name:
                return Selection(identity.sha1, identity.sha1)
    for identity in identities:
        if identity.name.startswith("Apple Development:"):
            return Selection(identity.sha1, identity.sha1)
    return Selection("-", None)


def cache_identity(path, sha1):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, prefix=path.name + ".", delete=False) as file:
            temporary = Path(file.name)
            file.write(sha1 + "\n")
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, default=Path(__file__).resolve().parents[1] / "build/preview-signing-identity.sha1")
    args = parser.parse_args(argv)
    try:
        result = subprocess.run(["security", "find-identity", "-v", "-p", "codesigning"], capture_output=True, text=True, check=False)
        identities = parse_identities(result.stdout) if result.returncode == 0 else []
    except OSError:
        identities = []
    try:
        cached_sha = args.cache.read_text().strip()
    except OSError:
        cached_sha = None
    selection = select_identity(
        identities, cached_sha, os.environ.get("APPLE_SIGNING_IDENTITY"), os.environ.get("PENNEL_SIGN_IDENTITY")
    )
    if selection.sha1:
        cache_identity(args.cache, selection.sha1)
    if selection.identity == "-":
        print("warning: preview uses ad-hoc signing; macOS permissions may need approval again after a rebuild. "
              "Set APPLE_SIGNING_IDENTITY or PENNEL_SIGN_IDENTITY to an existing certificate to retain them.", file=sys.stderr)
    print(selection.identity)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
