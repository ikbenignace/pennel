#!/usr/bin/env python3
import argparse
import json
import plistlib
import subprocess
import tempfile
import uuid
import wave
from pathlib import Path


parser = argparse.ArgumentParser()
parser.add_argument("container", type=Path)
args = parser.parse_args()
metadata = args.container / ".com.apple.mobile_container_manager.metadata.plist"
with metadata.open("rb") as stream:
    identity = plistlib.load(stream).get("MCMMetadataIdentifier")
if identity != "app.pennel.ios.preview":
    raise SystemExit("Only the isolated Pennel preview container may be seeded.")

meeting_id = "2026-10-27 1420 Pennel QA controlled meeting"
folder = args.container / "Documents" / "Meetings" / meeting_id
if folder.exists():
    raise SystemExit("The controlled fixture already exists; refusing to overwrite it.")
folder.mkdir(parents=True)
uid = str(uuid.uuid4())
(folder / "meeting.md").write_text(
    "---\n"
    "title: Pennel QA controlled meeting\n"
    "start: 2026-10-27T14:20:00+01:00\n"
    "duration_min: 1\n"
    "template: algemeen\n"
    f"pennel_id: {uid}\n"
    "speakers: 1=Alex; 5=Bo; 12=Chris\n"
    "---\n\n"
    "## Summary\n\n"
    "This is synthetic interface-test content.\n\n"
    "- Check local note editing.\n"
    "- Keep the next step on this device.\n"
)
(folder / "notes.md").write_text("A synthetic personal note.\n")
(folder / "transcript.md").write_text(
    "[00:00] **Anderen 1:** This is a synthetic meeting for interface checks.\n"
    "[00:02] **Anderen 5:** The next step is a local note.\n"
    "[00:04] **Anderen 12:** No private audio was recorded.\n"
)
with tempfile.TemporaryDirectory(prefix="pennel-silent-qa-") as temporary:
    wav_path = Path(temporary) / "silence.wav"
    with wave.open(str(wav_path), "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(16_000)
        audio.writeframes(bytes(60 * 16_000 * 2))
    subprocess.run(
        ["afconvert", "-f", "m4af", "-d", "aac", "-b", "32000", str(wav_path), str(folder / "anderen.m4a")],
        check=True,
    )
print(json.dumps({"meeting_id": meeting_id, "uid": uid, "audio": "60 seconds of synthetic digital silence"}))
