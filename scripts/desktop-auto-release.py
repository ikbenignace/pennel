#!/usr/bin/env python3
"""Decide the next automatic desktop release and render its Homebrew cask."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
TAURI_CONFIG = ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
# Paths that change what ends up in an installer; docs alone never cut a release.
APP_PATHS = ("core", "apps/desktop", "apps/macos-platform", "assets")
VERSION = re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)")


def parse(version):
    match = VERSION.fullmatch(version)
    if not match:
        raise ValueError(f"Not a plain semantic version: {version!r}")
    return tuple(int(part) for part in match.groups())


def next_version(configured, latest_tag):
    """Patch-bump the newest tag; a higher version committed in tauri.conf.json wins."""
    if latest_tag is None:
        return configured
    major, minor, patch = parse(latest_tag.removeprefix("v"))
    bumped = (major, minor, patch + 1)
    return configured if parse(configured) > bumped else "{}.{}.{}".format(*bumped)


def latest_release_tag(tags):
    versions = [tag for tag in tags if tag.startswith("v") and VERSION.fullmatch(tag[1:])]
    return max(versions, key=lambda tag: parse(tag[1:]), default=None)


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True)


def app_changed_since(tag):
    if tag is None:
        return True
    return subprocess.run(["git", "diff", "--quiet", tag, "HEAD", "--", *APP_PATHS], cwd=ROOT).returncode != 0


def render_cask(version, sha256, repository):
    if not re.fullmatch(r"[0-9a-f]{64}", sha256):
        raise ValueError("sha256 must be 64 lowercase hex characters")
    parse(version)
    return f'''# typed: strict
# frozen_string_literal: true

cask "pennel" do
  version "{version}"
  sha256 "{sha256}"

  url "https://github.com/{repository}/releases/download/v#{{version}}/Pennel_#{{version}}_aarch64.dmg"
  name "Pennel"
  desc "Dictation and meeting notes"
  homepage "https://pennel.app/"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Pennel.app"

  # Builds are not notarized yet; without this Gatekeeper refuses to open the app.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{{{appdir}}}}/Pennel.app"]
  end

  # Meetings, history and models stay; zap only removes regenerable data.
  zap trash: [
    "~/Library/Caches/app.pennel.desktop",
    "~/Library/WebKit/app.pennel.desktop",
  ]
end
'''


def plan():
    configured = json.loads(TAURI_CONFIG.read_text(encoding="utf-8"))["version"]
    tag = latest_release_tag(git("tag", "--list", "v*").split())
    return {
        "release": app_changed_since(tag),
        "version": next_version(configured, tag),
        "previous": tag or "",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("plan", help="Print release, version and previous as KEY=value lines")
    cask = commands.add_parser("cask", help="Print the Homebrew cask for a published release")
    cask.add_argument("version")
    cask.add_argument("sha256")
    cask.add_argument("--repository", default="pennel-app/pennel")
    args = parser.parse_args()
    try:
        if args.command == "plan":
            for key, value in plan().items():
                print(f"{key}={str(value).lower() if isinstance(value, bool) else value}")
        else:
            sys.stdout.write(render_cask(args.version, args.sha256, args.repository))
    except (ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error))


if __name__ == "__main__":
    main()
