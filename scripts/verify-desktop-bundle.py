#!/usr/bin/env python3
"""Inspect a packaged Mac app without opening windows, prompting, or recording."""

import argparse
import json
from pathlib import Path
import plistlib
import subprocess


USAGE_KEYS = (
    "NSMicrophoneUsageDescription", "NSAudioCaptureUsageDescription",
    "NSSpeechRecognitionUsageDescription", "NSCalendarsFullAccessUsageDescription",
)


def inspect_bundle(app, expected_identifier):
    with (app / "Contents" / "Info.plist").open("rb") as source:
        plist = plistlib.load(source)
    if plist.get("CFBundleIdentifier") != expected_identifier:
        raise ValueError(f"Unexpected bundle identifier: {plist.get('CFBundleIdentifier')}")
    if plist.get("LSMinimumSystemVersion") != "14.4":
        raise ValueError("Minimum macOS version must remain 14.4")
    if plist.get("LSUIElement") is not True:
        raise ValueError("Pennel must remain a menu-bar/background application")
    for key in USAGE_KEYS:
        if not isinstance(plist.get(key), str) or not plist[key].strip():
            raise ValueError(f"Missing permission explanation: {key}")
    executable = plist.get("CFBundleExecutable")
    if not isinstance(executable, str) or Path(executable).name != executable or not executable:
        raise ValueError("Invalid bundle executable")
    binary = app / "Contents" / "MacOS" / executable
    if not binary.is_file():
        raise ValueError("Packaged executable is missing")
    return plist, binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path)
    parser.add_argument("--identifier", default="dev.pennel.Pennel")
    parser.add_argument("--require-developer-id", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        plist, binary = inspect_bundle(args.app, args.identifier)
        architectures = subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip().split()
        if "arm64" not in architectures:
            raise ValueError("The initial Mac release must contain Apple Silicon code")
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(args.app)], check=True, capture_output=True, text=True)
        signature = subprocess.run(["codesign", "-dv", "--verbose=4", str(args.app)], check=True, capture_output=True, text=True).stderr
        developer_id = "Authority=Developer ID Application:" in signature
        if args.require_developer_id and not developer_id:
            raise ValueError("Release must be signed with Developer ID; an ad-hoc/development signature is not sufficient")
        receipt = {
            "bundle": str(args.app.resolve()), "identifier": args.identifier,
            "version": plist.get("CFBundleShortVersionString"), "build_number": plist.get("CFBundleVersion"),
            "architectures": architectures, "signature_valid": True,
            "developer_id": developer_id, "app_launched": False, "device_tests_performed": False,
        }
        rendered = json.dumps(receipt, indent=2) + "\n"
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered, encoding="utf-8")
        print(rendered, end="")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Packaged-app inspection failed: {error}\n")


if __name__ == "__main__":
    main()
