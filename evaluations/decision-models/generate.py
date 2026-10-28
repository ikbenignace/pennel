#!/usr/bin/env python3
"""Freeze, run and replay the current core's synthetic generative workload."""

import argparse
import gzip
import hashlib
import json
from decimal import Decimal
from pathlib import Path

import run
from corpus import build_corpus, corpus_hash
from generative import OpenRouterGenerativeRunner, run_baselines
from grounding import RUBRIC_SHA256, grounding_report
from runners import SpendLedger

SHARED_LEDGER = run.ROOT / "build/decision-model-evaluation/spend-ledger.json"


def selected_bridge(corpus, bridge, split):
    indices = (1,) if split == "development" else (1, 2)
    selected = {m["id"] for m in corpus["meetings"]
                if m["split"] == split and int(m["id"].rsplit("-", 1)[1]) in indices}
    queries = [q for q in corpus["queries"]
               if q["split"] == split and (q.get("origin_meeting_id") or q.get("meeting_id")) in selected]
    if split == "development":
        queries = [q for q in queries if q["id"].endswith("-q0")]
    ids = {q["id"] for q in queries}
    pairs = []
    seen = set()
    for pair in corpus["cleanup_pairs"]:
        if pair["meeting_id"] in selected and pair["meeting_id"] not in seen:
            pairs.append(pair)
            seen.add(pair["meeting_id"])
    pair_ids = {p["id"] for p in pairs}
    return {"queries": [q for q in bridge["queries"] if q["id"] in ids],
            "cleanup": [p for p in bridge["cleanup"] if p["id"] in pair_ids],
            "summary_prompts": [s for s in bridge["summary_prompts"] if s["meeting_id"] in selected]}


def workload_ids(bridge):
    return ([('meeting_qa', q['id']) for q in bridge["queries"]]
            + [('cleanup', p['id']) for p in bridge["cleanup"]]
            + [('summary', s['meeting_id'] + '-summary') for s in bridge["summary_prompts"]])


def archive(path, value):
    with Path(path).open("wb") as stream:
        with gzip.GzipFile(fileobj=stream, mode="wb", filename="", mtime=0) as output:
            output.write((json.dumps(value, ensure_ascii=False) + "\n").encode())


def read_archive(path):
    with gzip.open(path, "rt", encoding="utf-8") as stream:
        return json.load(stream)


def prepare(config, binary):
    corpus = build_corpus()
    bridge = run.collect_bridge(corpus, binary)
    inputs = {"corpus": corpus, "bridges": {split: selected_bridge(corpus, bridge, split)
                                            for split in ("development", "heldout")}}
    manifest = {"experiment": "current-core-generative-v1", "corpus_hash": corpus_hash(corpus),
                "inputs_sha256": run.digest(inputs), "config_sha256": run.digest(config),
                "protocol_sha256": run.digest(run.read_json(run.DIRECTORY / "protocol-generative.json")),
                "rubric_sha256": RUBRIC_SHA256, "source_fingerprint": run.source_fingerprint(),
                "source_commit": run.git("rev-parse", "HEAD"), "hardware": run.hardware(),
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "workloads": {split: workload_ids(value) for split, value in inputs["bridges"].items()},
                "heldout_before_inference": True, "hosted_total_ceiling_usd": "2.00"}
    return manifest, inputs


def validate_inputs(manifest, inputs, config=None):
    if manifest["inputs_sha256"] != run.digest(inputs) or manifest["corpus_hash"] != corpus_hash(inputs["corpus"]):
        raise ValueError("Frozen generative inputs changed")
    if manifest["rubric_sha256"] != RUBRIC_SHA256:
        raise ValueError("Frozen grounding rubric changed")
    if manifest.get("protocol_sha256") != run.digest(run.read_json(run.DIRECTORY / "protocol-generative.json")):
        raise ValueError("Frozen generative protocol changed")
    for split, bridge in inputs["bridges"].items():
        if manifest["workloads"][split] != [list(x) for x in workload_ids(bridge)]:
            raise ValueError("Frozen generative workload changed")
    if config is not None and manifest["config_sha256"] != run.digest(config):
        raise ValueError("Frozen generative configuration changed")


def validate_live(manifest, config, binary):
    if (run.ROOT / config.get("ledger_path", "")).resolve() != SHARED_LEDGER.resolve():
        raise ValueError("Preserve the canonical shared evaluation spend ledger")
    if manifest["binary_sha256"] != hashlib.sha256(binary.read_bytes()).hexdigest():
        raise ValueError("Frozen core evaluation binary changed")
    if manifest["hardware"] != run.hardware():
        raise ValueError("Frozen generative evaluation hardware changed")
    if manifest["source_fingerprint"] != run.source_fingerprint():
        raise ValueError("Generative source changed after freeze")


def validate_rows(rows, manifest, inputs):
    expected = {tuple(x) for x in manifest["workloads"]["heldout"]}
    observed = [(r["task"], r["id"]) for r in rows]
    if len(set(observed)) != len(observed) or set(observed) != expected:
        raise ValueError("Missing or duplicate generative workload rows")
    queries = {q["id"]: q for q in inputs["corpus"]["queries"]}
    pairs = {p["id"]: p for p in inputs["corpus"]["cleanup_pairs"]}
    for row in rows:
        if row.get("run_manifest_hash") != run.digest(manifest) or row.get("split") != "heldout":
            raise ValueError("Generative predictions belong to another freeze or split")
        if row.get("status") != "ok":
            continue
        annotation = row.get("adjudication", {})
        if row["task"] == "meeting_qa":
            case = queries[row["id"]]
            if annotation.get("expected_answer") != case.get("expected_answer") or annotation.get("gold_evidence") != case.get("evidence", []):
                raise ValueError("Generated answer gold differs from frozen inputs")
        elif row["task"] == "cleanup":
            case = pairs[row["id"]]
            if annotation.get("raw") != case["raw"]:
                raise ValueError("Generated cleanup raw differs from frozen inputs")


def report(directory):
    manifest = run.read_json(directory / "manifest.json")
    inputs = read_archive(directory / "inputs.json.gz")
    validate_inputs(manifest, inputs)
    with directory.joinpath("predictions.jsonl").open(encoding="utf-8") as stream:
        rows = [json.loads(line) for line in stream if line.strip()]
    validate_rows(rows, manifest, inputs)
    judgments = run.read_json(directory / "adjudications.json")
    result = grounding_report(rows, judgments, inputs["corpus"]["meetings"])
    from metrics import timing_metrics
    result["measured_latency"] = {language: {task: timing_metrics([
        row["latency_ms"] for row in rows if row["language"] == language and row["task"] == task
        and row.get("status") == "ok" and row.get("latency_ms") is not None])
        for task in ("meeting_qa", "cleanup", "summary")} for language in ("en", "nl", "mixed")}
    result["latency_scope"] = "Quality-workload OpenRouter round trips; these are not a repeated cold/warm panel or server-cold measurements."
    result["run_manifest_hash"] = run.digest(manifest)
    result["billable_cost_usd"] = str(sum((Decimal(r["cost_usd"])
        for r in rows if r.get("cost_usd") is not None), Decimal(0)))
    result["unknown_cost_rows"] = sum(r.get("cost_usd") is None and bool(r.get("attempts")) for r in rows)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--prepare", action="store_true")
    mode.add_argument("--live", action="store_true")
    mode.add_argument("--replay", type=Path)
    parser.add_argument("--config", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--split", choices=("development", "heldout"), default="development")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    args.output.mkdir(parents=True, exist_ok=True)
    if args.replay:
        run.save_json(args.output / "report.json", report(args.replay))
        print(json.dumps({"status": "offline_replay_complete", "report": str(args.output / "report.json")}))
        return
    if not args.config or not args.frozen:
        parser.error("--prepare and --live require --config and --frozen")
    config = run.read_json(args.config)
    binary = run.bridge_binary(args.binary)
    if args.prepare:
        manifest, inputs = prepare(config, binary)
        run.save_json(args.frozen, manifest)
        archive(args.output / "inputs.json.gz", inputs)
        print(json.dumps({"status": "frozen", "workload_sizes": {s: len(x) for s, x in manifest["workloads"].items()}}))
        return
    if not args.inputs:
        parser.error("--live requires frozen --inputs")
    manifest, inputs = run.read_json(args.frozen), read_archive(args.inputs)
    validate_inputs(manifest, inputs, config)
    validate_live(manifest, config, binary)
    if args.split == "heldout":
        relative = str(args.frozen.resolve().relative_to(run.ROOT))
        if not run.git("ls-files", "--", relative) or run.git("status", "--porcelain", "--", relative, "evaluations/decision-models", "core/src"):
            raise ValueError("Commit frozen source and manifest before heldout inference")
    path = args.output / "predictions.jsonl"
    if path.exists():
        raise ValueError("Existing predictions preserved; use another output directory")
    ledger_path = run.ROOT / config["ledger_path"]
    if not ledger_path.is_file():
        raise ValueError("Existing shared spend ledger required")
    def checkpoint(state):
        temporary = ledger_path.with_suffix(".tmp")
        run.save_json(temporary, state)
        temporary.replace(ledger_path)
    with run.experiment_lock(ledger_path):
        ledger = SpendLedger.from_dict(run.read_json(ledger_path), checkpoint=checkpoint)
        if ledger.unresolved or ledger.pending:
            raise ValueError("Reconcile the shared ledger before paid work")
        runner = OpenRouterGenerativeRunner(config["generative"], ledger=ledger)
        with path.open("x", encoding="utf-8") as stream:
            def capture(row):
                row["run_manifest_hash"] = run.digest(manifest)
                stream.write(json.dumps(row, ensure_ascii=False) + "\n")
                stream.flush()
                print(json.dumps({"task": row["task"], "id": row["id"], "status": row["status"], "error": row.get("error"), "billed_total_usd": str(ledger.spent)}), flush=True)
            rows = run_baselines(inputs["corpus"], inputs["bridges"][args.split], runner, args.split,
                                 lambda request: run.bridge_call(binary, request), checkpoint=capture)
        run.save_json(args.output / "availability.json", runner.preflight())
        run.save_json(args.output / "manifest.json", manifest)
        archive(args.output / "inputs.json.gz", inputs)
        run.save_json(args.output / "ledger-summary.json", {k: v for k, v in ledger.snapshot().items() if k != "attempts"})
        if len(rows) != len(manifest["workloads"][args.split]):
            raise ValueError("Generative workload incomplete; preserve completed receipts and reconcile before continuing")


if __name__ == "__main__":
    main()
