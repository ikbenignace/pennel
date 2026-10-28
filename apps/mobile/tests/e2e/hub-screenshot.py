#!/usr/bin/env python3
import argparse
import json
import urllib.parse
import urllib.request
from pathlib import Path


parser = argparse.ArgumentParser()
parser.add_argument("name")
parser.add_argument("--hub", default="http://127.0.0.1:8879")
args = parser.parse_args()
if urllib.parse.urlsplit(args.hub).hostname not in {"127.0.0.1", "localhost"}:
    raise SystemExit("QA captures require the task-local loopback Hub.")
if not args.name.replace("-", "").replace("_", "").isalnum():
    raise SystemExit("Use a simple capture name.")
device = "64C462E2-0904-4E38-98C0-F1F19690D2E4"
base = args.hub.rstrip("/") + "/vendor/serve-sim"
query = urllib.parse.urlencode({"device": device})
with urllib.request.urlopen(base + "/api?" + query, timeout=15) as response:
    descriptor = json.load(response)
selected = descriptor.get("device")
if isinstance(selected, dict):
    selected = selected.get("udid") or selected.get("id")
if selected != device:
    raise SystemExit("The Hub descriptor did not resolve the assigned QA device.")
headers = {"Origin": args.hub}
if descriptor.get("execToken"):
    headers["Authorization"] = "Bearer " + descriptor["execToken"]
request = urllib.request.Request(base + "/api/screenshot?" + query, method="POST", headers=headers)
with urllib.request.urlopen(request, timeout=30) as response:
    image = response.read()
if not image.startswith(b"\x89PNG\r\n\x1a\n"):
    raise SystemExit("The Hub did not return a PNG capture.")
output = Path(__file__).resolve().parents[4] / "build" / "mobile" / "qa" / "device-hub" / (args.name + ".png")
output.parent.mkdir(parents=True, exist_ok=True)
output.write_bytes(image)
print(json.dumps({"device": device, "path": str(output), "bytes": len(image)}))
