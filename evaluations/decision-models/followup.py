#!/usr/bin/env python3
"""Freeze, evaluate or replay the independent #53 suggestion supplement.

Prepare with --prepare --config CONFIG --frozen MANIFEST --output DIRECTORY.
Live calibration writes calibration.json; held-out execution additionally requires
--calibration and a committed freeze. --replay DIRECTORY needs no credentials.
"""

import argparse
from collections import Counter
import gzip
import json
from pathlib import Path

from calibration import fit_predictions, score_predictions
import metrics
import run
import supplement


DIRECTORY = Path(__file__).resolve().parent
ROOT = DIRECTORY.parents[1]
SHARED_LEDGER = ROOT / "build/decision-model-evaluation/spend-ledger.json"
BINDING_FIELDS = (
    "experiment_version", "corpus_hash", "protocol_hash", "supplement_protocol_hash",
    "source_fingerprint", "config_hash", "models_hash", "planned_cases", "prompts_hash",
)


def normalise_config(config):
    if "transport" in config:
        config = {"ledger_path": config.get("ledger_path"), "runners": [{"id": "configured-runner", **config}]}
    if not isinstance(config.get("runners"), list) or not config["runners"]:
        raise ValueError("The supplement requires an explicit runner configuration")
    identifiers = [runner.get("id") for runner in config["runners"]]
    if any(not isinstance(value, str) or not value for value in identifiers) or len(set(identifiers)) != len(identifiers):
        raise ValueError("Runner IDs must be present and unique")
    ledger = config.get("ledger_path")
    if not isinstance(ledger, str) or (ROOT / ledger).resolve() != SHARED_LEDGER.resolve():
        raise ValueError("The supplement must preserve the existing shared #53 spend ledger")
    return config


def _planned(corpus, config):
    return {split: {runner["id"]: [case["id"] for case in corpus["decisions"]
                                  if case["split"] == split and (not runner.get("tasks") or case["task"] in runner["tasks"])]
                    for runner in config["runners"] if runner.get("enabled") is True}
            for split in ("development", "calibration", "heldout")}


def prepare_manifest(config, corpus=None):
    corpus = supplement.build_supplement() if corpus is None else corpus
    errors = supplement.validate_supplement(corpus)
    if errors:
        raise ValueError("Supplement corpus validation failed")
    config = normalise_config(config)
    return {
        "schema_version": 1, "experiment_version": supplement.VERSION,
        "corpus_hash": supplement.corpus_hash(corpus), "corpus_hash_method": "ordered_json_utf8",
        "protocol_hash": run.digest(run.read_json(DIRECTORY / "protocol.json")),
        "supplement_protocol_hash": run.digest(run.read_json(DIRECTORY / "protocol-supplement-v2.json")),
        "source_fingerprint": run.source_fingerprint(), "source_commit": run.git("rev-parse", "HEAD"),
        "config_hash": run.digest(config), "config": config,
        "models_hash": run.digest(run.read_json(DIRECTORY / "models.json")),
        "prompts_hash": run.digest([{key: case[key] for key in ("id", "state", "question", "label_order")}
                                     for case in corpus["decisions"]]),
        "planned_cases": _planned(corpus, config), "hardware": run.hardware(),
        "hosted_spend_ceiling_usd_total": "2.00", "shared_ledger_path": config["ledger_path"],
        "separation": "Independent supplement; original corpus, calibration and results remain unchanged.",
    }


def verify_manifest(recorded, current, check_hardware=True, check_source=True):
    for field in BINDING_FIELDS:
        if field == "source_fingerprint" and not check_source:
            continue
        if recorded.get(field) != current.get(field):
            raise ValueError(f"Frozen supplement {field} differs; preregister a new experiment")
    if check_hardware and recorded.get("hardware") != current.get("hardware"):
        raise ValueError("Live supplement hardware differs from the frozen experiment")


def require_committed(path):
    try:
        relative = path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        raise ValueError("Held-out supplement freeze must be committed in this repository") from None
    if not run.git("ls-files", "--", relative) or run.git("status", "--porcelain", "--", relative):
        raise ValueError("Held-out supplement requires a committed, unchanged frozen manifest")
    if run.git("status", "--porcelain", "--", "evaluations/decision-models", "core/src", "core/Cargo.toml"):
        raise ValueError("Commit the evaluation implementation before held-out supplement inference")


def _runner_lookup(manifest):
    return {runner["id"]: runner for runner in manifest["config"]["runners"] if runner.get("enabled") is True}


def _provider_slug(value):
    return "".join(character for character in str(value).casefold() if character.isalnum())


def validate_predictions(rows, manifest, corpus, split):
    gold = {case["id"]: case for case in corpus["decisions"] if case["split"] == split}
    runners = _runner_lookup(manifest)
    planned = {(runner, identifier) for runner, identifiers in manifest["planned_cases"][split].items() for identifier in identifiers}
    seen = set()
    for row in rows:
        key = (row.get("runner_id"), row.get("case_id", row.get("id")))
        if key not in planned or key in seen:
            raise ValueError("Recorded supplement contains an unknown or duplicate prediction")
        seen.add(key)
        case = gold[key[1]]
        if row.get("run_manifest_hash") != run.digest(manifest):
            raise ValueError("Recorded supplement prediction belongs to another frozen experiment")
        if row.get("id") is not None and row["id"] != case["id"]:
            raise ValueError("Supplement prediction has inconsistent case identifiers")
        for field in ("split", "expected", "language", "group_id", "task", "subtask", "question", "label_order"):
            if row.get(field) != case.get(field):
                raise ValueError(f"Recorded supplement {field} differs from its frozen case")
        if row.get("status") != "ok":
            continue
        runner = runners[key[0]]
        identity = row.get("identity", {})
        transport = runner.get("transport")
        if transport == "local_worker":
            if identity.get("transport") != "local" or identity.get("model_id") != runner.get("model"):
                raise ValueError("Recorded supplement local model identity differs")
        else:
            if identity.get("transport") != transport or identity.get("requested_model") != runner.get("model"):
                raise ValueError("Recorded supplement requested model identity differs")
            allowed = {runner.get("model"), runner.get("expected_returned_model")}
            if not identity.get("returned_model") or identity["returned_model"] not in allowed:
                raise ValueError("Recorded supplement returned model identity differs")
            if transport == "openrouter":
                tag = runner["upstream_tag"]
                routing = identity.get("routing") or {}
                if routing.get("only") != [tag] or routing.get("allow_fallbacks") is not False:
                    raise ValueError("Recorded supplement routing differs")
                if _provider_slug(identity.get("provider")) != _provider_slug(tag.split("/")[0]):
                    raise ValueError("Recorded supplement actual provider differs")
                if identity.get("privacy") != runner.get("privacy"):
                    raise ValueError("Recorded supplement privacy route differs")
                privacy = runner.get("privacy") or {}
                if privacy.get("data_collection") == "deny" and routing.get("data_collection") != "deny":
                    raise ValueError("Recorded supplement outbound data-collection restriction is missing")
                if privacy.get("zdr") is True and routing.get("zdr") is not True:
                    raise ValueError("Recorded supplement outbound ZDR restriction is missing")
    if seen != planned:
        raise ValueError("Recorded supplement is incomplete; preserve it and record a separate continuation")


def _validate_calibration(artifact, manifest, corpus):
    bindings = {"run_manifest_hash": run.digest(manifest), "supplement_version": supplement.VERSION,
                "supplement_protocol_hash": manifest["supplement_protocol_hash"],
                "supplement_corpus_hash": manifest["corpus_hash"],
                "supplement_source_fingerprint": manifest["source_fingerprint"]}
    if any(artifact.get(key) != value for key, value in bindings.items()) or artifact.get("fit_split") != "calibration":
        raise ValueError("Calibration belongs to a different supplement, source or split")
    calibration_ids = {case["id"] for case in corpus["decisions"] if case["split"] == "calibration"}
    for entry in artifact["fits"].values():
        fit = entry.get("fit")
        if fit and (fit.get("calibration_split") != "calibration" or not set(fit["calibration_case_ids"]).issubset(calibration_ids)):
            raise ValueError("Calibration contains cases outside the calibration split")


def _metric_rows(rows):
    values = [row if row.get("status") == "ok" else {**row, "prediction": None, "probabilities": None} for row in rows]
    # Stable mapping iteration avoids last-bit sum differences between JSON and JSONL receipts.
    # Explicit label-order lists and every returned probability value remain unchanged.
    return json.loads(metrics.canonical_json(values))


def fit_recorded(rows, manifest, corpus=None):
    corpus = supplement.build_supplement() if corpus is None else corpus
    validate_predictions(rows, manifest, corpus, "calibration")
    artifact = fit_predictions(_metric_rows(rows), run_manifest_hash=run.digest(manifest))
    artifact.update(supplement_version=supplement.VERSION, supplement_protocol_hash=manifest["supplement_protocol_hash"],
                    supplement_corpus_hash=manifest["corpus_hash"], supplement_source_fingerprint=manifest["source_fingerprint"],
                    calibration_predictions_sha256=run.digest(rows))
    return artifact


def score_recorded(rows, artifact, manifest, corpus=None):
    corpus = supplement.build_supplement() if corpus is None else corpus
    validate_predictions(rows, manifest, corpus, "heldout")
    _validate_calibration(artifact, manifest, corpus)
    return score_predictions(_metric_rows(rows), artifact, run_manifest_hash=run.digest(manifest))


def _read_rows(directory):
    for name in ("predictions.jsonl.gz", "predictions.json", "predictions.jsonl"):
        path = directory / name
        if not path.is_file():
            continue
        if name.endswith(".gz"):
            with gzip.open(path, "rt", encoding="utf-8") as stream:
                return [json.loads(line) for line in stream if line.strip()]
        if name.endswith(".json"):
            payload = run.read_json(path)
            return payload["rows"] if isinstance(payload, dict) else payload
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    raise ValueError("Recorded supplement predictions are missing")


def _report(rows, artifact, manifest, corpus, split, availability=(), replayed=False):
    report = {"schema_version": 1, "experiment_version": supplement.VERSION,
              "evidence_kind": "offline_replay_of_recorded_supplement" if replayed else "measured_supplement_inference",
              "manifest": manifest, "split": split, "status_counts": dict(sorted(Counter(row["status"] for row in rows).items())),
              "feasibility": supplement.feasibility_report(corpus), "live": run.predictions_report(_metric_rows(rows)),
              "availability": list(availability), "calibration_sha256": run.digest(artifact),
              "limitations": corpus["limitations"]}
    if split == "heldout":
        report["calibrated"] = score_recorded(rows, artifact, manifest, corpus)
    else:
        report["calibration_contexts"] = dict(sorted(Counter(entry["status"] for entry in artifact["fits"].values()).items()))
    return report


def replay(directory, frozen_path=None, calibration_path=None):
    directory = Path(directory)
    manifest = run.read_json(directory / "manifest.json")
    if frozen_path and run.digest(run.read_json(frozen_path)) != run.digest(manifest):
        raise ValueError("Replay manifest differs from the supplied freeze")
    corpus = supplement.build_supplement()
    current = prepare_manifest(manifest["config"], corpus)
    verify_manifest(manifest, current, check_hardware=False, check_source=False)
    receipt = run.read_json(directory / "execution.json")
    artifact = run.read_json(calibration_path or directory / "calibration.json")
    rows = _read_rows(directory)
    if receipt.get("run_manifest_hash") != run.digest(manifest) or receipt.get("calibration_sha256") != run.digest(artifact):
        raise ValueError("Replay calibration or manifest differs from its execution receipt")
    if receipt.get("predictions_sha256") != run.digest(rows):
        raise ValueError("Replay predictions differ from their completed execution receipt")
    split = receipt.get("split")
    if split not in ("calibration", "heldout"):
        raise ValueError("Replay receipt does not identify a supported split")
    validate_predictions(rows, manifest, corpus, split)
    _validate_calibration(artifact, manifest, corpus)
    availability = run.read_json(directory / "availability.json") if (directory / "availability.json").is_file() else []
    report = _report(rows, artifact, manifest, corpus, split, availability, replayed=True)
    report["replay_source_fingerprint"] = current["source_fingerprint"]
    return report


def _new_output(path):
    path = Path(path)
    if path.exists() and (not path.is_dir() or any(path.iterdir())):
        raise ValueError("Use a new output directory; existing evaluation evidence will not be overwritten")
    return path


def _complete_unavailable(rows, availability, corpus, config, split, manifest):
    planned = {(runner, identifier) for runner, identifiers in manifest["planned_cases"][split].items() for identifier in identifiers}
    indexed = {}
    for row in rows:
        key = (row["runner_id"], row.get("case_id", row.get("id")))
        if key not in planned or key in indexed:
            raise ValueError("Live supplement returned an unknown or duplicate prediction")
        indexed[key] = row
    reasons = {item["id"]: item.get("reason", "runner_unavailable") for item in availability}
    cases = {case["id"]: case for case in corpus["decisions"]}
    completed = []
    for runner, identifiers in manifest["planned_cases"][split].items():
        for identifier in identifiers:
            completed.append(indexed.get((runner, identifier)) or {
                **cases[identifier], "case_id": identifier, "runner_id": runner,
                "run_manifest_hash": run.digest(manifest), "status": "unavailable", "prediction": None,
                "probabilities": None, "reason": reasons.get(runner, "not_submitted"),
            })
    return completed


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--prepare", action="store_true")
    mode.add_argument("--live", action="store_true")
    mode.add_argument("--replay", type=Path)
    parser.add_argument("--offline", action="store_true", help="Explicit offline replay alias; never enables inference")
    parser.add_argument("--config", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--calibration", type=Path)
    parser.add_argument("--split", choices=("calibration", "heldout"), default="calibration")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.offline and not args.replay:
        parser.error("--offline is only valid with --replay")
    output = _new_output(args.output)
    if args.replay:
        report = replay(args.replay, args.frozen, args.calibration)
        output.mkdir(parents=True, exist_ok=True)
        run.save_json(output / "report.json", report)
        (output / "report.md").write_text(metrics.render_report(report), encoding="utf-8")
        return 0
    if not args.config or not args.frozen:
        parser.error("--prepare and --live require --config and --frozen")
    config = normalise_config(run.read_json(args.config))
    corpus = supplement.build_supplement()
    current = prepare_manifest(config, corpus)
    if args.prepare:
        if args.frozen.exists():
            raise ValueError("The frozen manifest already exists; preserve it and choose a new experiment")
        if args.frozen.resolve().parent == DIRECTORY:
            raise ValueError("Place the freeze in a nested experiment directory to avoid source self-reference")
        args.frozen.parent.mkdir(parents=True, exist_ok=True)
        with args.frozen.open("x", encoding="utf-8") as stream:
            json.dump(current, stream, ensure_ascii=False, sort_keys=True, indent=2)
            stream.write("\n")
        output.mkdir(parents=True, exist_ok=True)
        run.save_json(output / "manifest.json", current)
        (output / "corpus.json").write_text(json.dumps(corpus, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        run.save_json(output / "feasibility.json", supplement.feasibility_report(corpus))
        print(json.dumps({"status": "prepared", "manifest_hash": run.digest(current), "version": supplement.VERSION}))
        return 0
    frozen = run.read_json(args.frozen)
    verify_manifest(frozen, current)
    artifact = None
    if args.split == "heldout":
        if not args.calibration:
            parser.error("Held-out supplement inference requires --calibration")
        require_committed(args.frozen)
        artifact = run.read_json(args.calibration)
        _validate_calibration(artifact, frozen, corpus)
    output.mkdir(parents=True, exist_ok=True)
    run.save_json(output / "manifest.json", frozen)
    receipt = {"split": args.split, "run_manifest_hash": run.digest(frozen),
               "calibration_sha256": run.digest(artifact) if artifact else None, "status": "started"}
    run.save_json(output / "execution.json", receipt)
    rows, availability = run.live_predictions(corpus, {"queries": [], "passages": []}, config, args.split, output, frozen)
    rows = _complete_unavailable(rows, availability, corpus, config, args.split, frozen)
    validate_predictions(rows, frozen, corpus, args.split)
    if args.split == "calibration":
        artifact = fit_recorded(rows, frozen, corpus)
    run.save_json(output / "predictions.json", {"frozen": run.digest(frozen), "rows": rows})
    run.save_json(output / "calibration.json", artifact)
    receipt.update(status="complete", calibration_sha256=run.digest(artifact), predictions_sha256=run.digest(rows))
    run.save_json(output / "execution.json", receipt)
    report = _report(rows, artifact, frozen, corpus, args.split, availability)
    run.save_json(output / "report.json", report)
    (output / "report.md").write_text(metrics.render_report(report), encoding="utf-8")
    print(json.dumps({"status": "complete", "version": supplement.VERSION, "split": args.split, "rows": len(rows)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
