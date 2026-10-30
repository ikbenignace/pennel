#!/usr/bin/env python3
"""Fill unsent slots in a published timing panel without replacing failures."""

import argparse
import gzip
import json
from pathlib import Path

import run
from corpus import build_corpus
from runners import SpendLedger
from timing_continuation import execute_continuation, plan_continuation


def load_original(directory):
    original = run.read_json(directory / "resources.json")
    with gzip.open(directory / "latency-receipts.jsonl.gz", "rt", encoding="utf-8") as stream:
        records = [json.loads(line) for line in stream if line.strip()]
    for model in original["models"]:
        model["records"] = [r for r in records if r["runner_id"] == model["runner_id"]]
        if len(model["records"]) != model["record_count"]:
            raise ValueError("Original timing receipt count mismatch")
    return original


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--frozen", type=Path, required=True)
    parser.add_argument("--original", type=Path, required=True)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--core-artifacts", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    config = run.read_json(args.config)
    config["runners"] = [r for r in config["runners"] if r["id"] == "openrouter-mercury"]
    original = load_original(args.original)
    corpus = build_corpus()
    ranking = run.ranking_cases(corpus, run.read_json(args.core_artifacts), "heldout")
    plan = plan_continuation(original, config, corpus, ranking)
    if args.prepare:
        run.save_json(args.frozen, {"experiment": "mercury-timing-continuation-v1", "plan_sha256": plan["plan_hash"],
            "original_resources_sha256": run.digest(original), "config_sha256": run.digest(config),
            "parent_manifest_sha256": run.digest(run.read_json(args.original / "manifest.json")),
            "cases": [{k: v for k, v in cell.items()} for model in plan["models"] for cell in model["cases"]],
            "hosted_total_ceiling_usd": "2.00", "latency_concurrency": 1,
            "policy": "Complete only unsent slots, preserve failed samples; excluded continuation warmups; no automatic retries."})
        print(json.dumps({"status": "prepared", "plan_hash": plan["plan_hash"]}))
        return
    frozen = run.read_json(args.frozen)
    if frozen["plan_sha256"] != plan["plan_hash"] or frozen["config_sha256"] != run.digest(config) or frozen["original_resources_sha256"] != run.digest(original):
        raise ValueError("Frozen timing continuation changed")
    relative = str(args.frozen.resolve().relative_to(run.ROOT))
    if not run.git("ls-files", "--", relative) or run.git("status", "--porcelain", "--", relative):
        raise ValueError("Commit the timing continuation freeze before inference")
    args.output.mkdir(parents=True, exist_ok=True)
    receipts = args.output / "new-receipts.jsonl"
    if receipts.exists():
        raise ValueError("Existing timing receipts preserved")
    ledger_path = run.ROOT / config["ledger_path"]
    def checkpoint(state):
        temporary = ledger_path.with_suffix(".tmp")
        run.save_json(temporary, state)
        temporary.replace(ledger_path)
    with run.experiment_lock(ledger_path):
        ledger = SpendLedger.from_dict(run.read_json(ledger_path), checkpoint=checkpoint)
        with receipts.open("x", encoding="utf-8") as stream:
            count = 0
            def capture(record):
                nonlocal count
                stream.write(json.dumps(record, ensure_ascii=False) + "\n")
                stream.flush()
                count += 1
                if count % 50 == 0 or record.get("status") != "ok":
                    print(json.dumps({"records": count, "task": record["task"], "language": record["language"],
                                      "status": record["status"], "reason": record.get("reason"), "total_billed_usd": str(ledger.spent)}), flush=True)
            result = execute_continuation(original, config, corpus, ranking, ledger, checkpoint=capture)
        result["manifest_sha256"] = run.digest(frozen)
        result["generation_commit"] = run.git("rev-parse", "HEAD")
        run.save_json(args.output / "report.json", result)
        print(json.dumps({"status": result["status"], "all_passed": result["all_passed"],
                          "new_records": len(result["new_records"]), "total_billed_usd": str(ledger.spent)}), flush=True)


if __name__ == "__main__":
    main()
