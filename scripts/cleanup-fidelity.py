#!/usr/bin/env python3
"""Reproduce #60's synthetic offline guard report without provider access."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import platform
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]


def run_report(test: str, marker: str) -> dict:
    command = [
        str(ROOT / "scripts/with-toolchain.sh"),
        "cargo", "test", "--manifest-path", str(ROOT / "core/Cargo.toml"),
        "--lib", "--no-default-features", f"cleanup_fidelity_tests::{test}",
        "--", "--exact", "--ignored", "--nocapture", "--test-threads=1",
    ]
    completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
    if completed.returncode:
        sys.stderr.write(completed.stdout)
        sys.stderr.write(completed.stderr)
        raise SystemExit(completed.returncode)
    records = re.findall(re.escape(marker) + r"(\{[^\n]*\})", completed.stdout)
    if len(records) != 1:
        raise SystemExit("Expected exactly one structured cleanup report.")
    return json.loads(records[0])


def write_json(path: pathlib.Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n")


def source_identity() -> dict:
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    sources = sorted([ROOT / "core/Cargo.toml", ROOT / "core/Cargo.lock", *(ROOT / "core/src").rglob("*.rs")])
    return {
        "commit": commit,
        "files_sha256": {
            path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sources if path.is_file()
        },
        "report_script_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
    }


def validate_quality(quality: dict, frozen: dict) -> None:
    if quality.get("schema") != 1 or quality.get("manifest") != frozen:
        raise SystemExit("Report fixtures differ from the frozen manifest. Preserve the prior report and freeze a new version explicitly.")
    for version in ("baseline", "candidate"):
        if len(quality[version]["cases"]) != 336:
            raise SystemExit("The report omitted scheduled cases.")
        for language in ("en", "nl", "mixed"):
            summary = quality[version]["languages"][language]
            selected = [row for row in quality[version]["cases"] if row["language"] == language and row["split"] == "held_out"]
            harmful = [row for row in selected if row["harmful"]]
            benign = [row for row in selected if not row["harmful"]]
            if len(harmful) != 48 or len(benign) != 36:
                raise SystemExit("Case records disagree with the declared held-out split.")
            numerators = {
                "harmful_acceptance": sum(row["accepted"] for row in harmful),
                "benign_rejection": sum(not row["accepted"] for row in benign),
                "abstention": sum(row["abstained"] for row in selected),
            }
            for metric, expected in (("harmful_acceptance", 48), ("benign_rejection", 36), ("abstention", 84)):
                result = summary[metric]
                if result["denominator"] != expected or result["numerator"] != numerators[metric] or result["rate"] != numerators[metric] / expected:
                    raise SystemExit(f"Invalid {language} {metric} reduction.")
    for version in ("baseline", "candidate"):
        if any({"raw", "candidate", "cleaned"}.intersection(row) for row in quality[version]["cases"]):
            raise SystemExit("Raw fixture text must not appear in report records.")


def fraction(value: dict) -> str:
    return f'{value["numerator"]}/{value["denominator"]} ({100 * value["rate"]:.1f}%)'


def markdown(quality: dict, timing: dict, mock_pipeline: dict) -> str:
    manifest = quality["manifest"]
    lines = [
        "# Cleanup fidelity: offline evidence", "",
        f'Panel `{manifest["panel_version"]}` · fixtures SHA-256 `{manifest["fixture_sha256"]}`.', "",
        "Synthetic fixtures compare the historical overlap guard with the actual Rust safeguard. "
        "These results establish fixture behavior; production semantic qualification and live provider latency remain unavailable.", "",
        "| Language | Guard | Harmful acceptance | Benign rejection | Abstention |",
        "| --- | --- | --- | --- | --- |",
    ]
    for language in ("en", "nl", "mixed"):
        for version in ("baseline", "candidate"):
            summary = quality[version]["languages"][language]
            lines.append(f'| {language} | {version} | {fraction(summary["harmful_acceptance"])} | {fraction(summary["benign_rejection"])} | {fraction(summary["abstention"])} |')
    lines.extend(["", "The five known issue/#53 regression seeds are excluded from held-out denominators. "
                  "Deterministic rejections are rejections, rather than semantic abstentions.", "",
                  "| Language | Added deterministic p50 (ms) | Added deterministic p95 (ms) | Paired samples |",
                  "| --- | --- | --- | --- |"])
    for language in ("en", "nl", "mixed"):
        added = timing["languages"][language]["added"]
        lines.append(f'| {language} | {added["p50_ms"]:.6f} | {added["p95_ms"]:.6f} | {added["samples"]} |')
    lines.extend(["", "Timings exclude one warmup per case, alternate paired execution order, and use linear type-7 percentiles. "
                  "They measure CPU overhead in this process, not live stop-to-result latency. "
                  "Raw samples are preserved in `timing.json`.", "", "## Mock pipeline controls", "",
                  "The following records come from actual offline mock requests. Programmed responses verify control flow and call counts; "
                  "they do not measure model accuracy or establish qualification.", "",
                  "| Language | Scenario | Outcome | Reason | Generation calls | Verification calls | Cleanup elapsed (ms) |",
                  "| --- | --- | --- | --- | --- | --- | --- |"])
    for row in mock_pipeline["scenarios"]:
        lines.append(f'| {row["language"]} | {row["id"]} | {row["outcome"]} | {row["reason_code"]} | {row["generation_calls"]} | {row["verification_calls"]} | {row["elapsed_ms"]:.3f} |')
    lines.extend(["", "| Annotated mock language | Added verifier p50 (ms) | Added verifier p95 (ms) | Paired samples |",
                  "| --- | --- | --- | --- |"])
    for language in ("en", "nl", "mixed"):
        added = mock_pipeline["verification_overhead"]["languages"][language]["added"]
        lines.append(f'| {language} | {added["p50_ms"]:.6f} | {added["p95_ms"]:.6f} | {added["samples"]} |')
    lines.extend(["", "Mock verifier timings pair generation-only cleanup with generation plus one verifier call, "
                  "using thirty repetitions per annotated language and alternating execution order. One excluded warmup pair "
                  "and its actual calls are retained in `timing.json`, alongside every measured dispatch. These are local mock cleanup timings; "
                  "live recognition, semantic quality and real provider latency remain unmeasured.", "",
                  "No hosted inference, model download, production dictation, or benchmark-budget reuse is required.", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "build/cleanup-fidelity")
    parser.add_argument("--freeze", action="store_true", help="Freeze fixtures/criteria without assessing held-out candidates.")
    arguments = parser.parse_args()
    output = arguments.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    frozen_path = output / "frozen-manifest.json"
    if arguments.freeze:
        frozen = run_report("freeze_manifest", "PENNEL_CLEANUP_MANIFEST=")
        if frozen_path.exists() and json.loads(frozen_path.read_text()) != frozen:
            raise SystemExit("A different frozen manifest already exists. Select a new output directory to preserve it.")
        write_json(frozen_path, frozen)
        print(f'Frozen {frozen["total_cases"]} synthetic proposals: {frozen["fixture_sha256"]}')
        return
    if not frozen_path.exists():
        raise SystemExit("Freeze the fixtures first with --freeze; that command performs no held-out assessment.")
    frozen = json.loads(frozen_path.read_text())
    identity = source_identity()
    result = run_report("report", "PENNEL_CLEANUP_REPORT=")
    if source_identity() != identity:
        raise SystemExit("Implementation sources changed during the report; rerun after concurrent edits finish.")
    validate_quality(result["quality"], frozen)
    result["quality"]["source"] = identity
    result["timing"]["source"] = identity
    result["timing"]["environment"] = {"system": platform.system(), "release": platform.release(), "machine": platform.machine(), "python": platform.python_version()}
    result["timing"]["mock_pipeline"] = result["mock_pipeline"]
    write_json(output / "quality.json", result["quality"])
    write_json(output / "timing.json", result["timing"])
    (output / "report.md").write_text(markdown(result["quality"], result["timing"], result["mock_pipeline"]))
    print(f"Wrote synthetic offline evidence to {output}")
    failed = [language for language, summary in result["quality"]["candidate"]["languages"].items() if summary["harmful_acceptance"]["numerator"]]
    if failed:
        raise SystemExit("Harmful proposals were accepted in: " + ", ".join(failed))


if __name__ == "__main__":
    main()
