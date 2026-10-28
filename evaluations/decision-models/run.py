#!/usr/bin/env python3
"""Evaluate synthetic Pennel fixtures; network inference always needs --live and a config."""

import argparse
from collections import defaultdict
from contextlib import contextmanager
import hashlib
import gzip
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import re

DIRECTORY = Path(__file__).resolve().parent
ROOT = DIRECTORY.parents[1]
sys.path.insert(0, str(DIRECTORY))


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                     separators=(",", ":")).encode()).hexdigest()


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def save_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2,
                               allow_nan=False) + "\n", encoding="utf-8")


@contextmanager
def experiment_lock(ledger_path):
    lock = Path(ledger_path).with_suffix(".lock")
    lock.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except FileExistsError:
        raise ValueError("An experiment ledger is already locked; verify its owner before retrying") from None
    try:
        os.write(descriptor, str(os.getpid()).encode())
        yield
    finally:
        os.close(descriptor)
        lock.unlink(missing_ok=True)


def git(*arguments):
    return subprocess.check_output(["git", *arguments], cwd=ROOT, text=True).strip()


def hardware():
    info = {"system": platform.system(), "release": platform.release(),
            "machine": platform.machine(), "python": platform.python_version(),
            "logical_cpus": os.cpu_count()}
    if sys.platform == "darwin":
        for key, field in [("hw.memsize", "memory_bytes"),
                           ("machdep.cpu.brand_string", "cpu")]:
            value = subprocess.check_output(["sysctl", "-n", key], text=True).strip()
            info[field] = int(value) if field == "memory_bytes" else value
    return info


def bridge_binary(explicit=None):
    configured = explicit or os.environ.get("PENNEL_EVAL_BINARY")
    if configured:
        binary = Path(configured).resolve()
        if not binary.is_file():
            raise ValueError("Configured evaluation executable does not exist")
        return binary
    target = ROOT / "build" / "evaluation-core-target"
    command = ["cargo", "build", "--offline", "--locked", "--manifest-path",
               str(ROOT / "core/Cargo.toml"), "--no-default-features", "--features",
               "evaluation", "--bin", "pennel-eval"]
    if os.name != "nt":
        command.insert(0, str(ROOT / "scripts/with-toolchain.sh"))
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    subprocess.run(command, cwd=ROOT, env=env, check=True, stdout=sys.stderr)
    return target / "debug" / ("pennel-eval.exe" if os.name == "nt" else "pennel-eval")


def collect_bridge(corpus, binary):
    groups = defaultdict(list)
    for meeting in corpus["meetings"]:
        groups[(meeting["language"], meeting["split"])].append(meeting)
    requests = []
    for (language, split), meetings in sorted(groups.items()):
        requests.append({"op": "evaluate", "meetings": meetings,
                         "queries": [q for q in corpus["queries"]
                                     if q["language"] == language and q["split"] == split],
                         "cleanup_pairs": [c for c in corpus["cleanup_pairs"]
                                           if c["language"] == language and c["split"] == split],
                         "citation_cases": [c for c in corpus.get("citation_cases", [])
                                            if c["language"] == language and c["split"] == split]})
    payload = "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in requests)
    started = time.perf_counter()
    process = subprocess.run([str(binary)], input=payload, text=True,
                             encoding="utf-8", capture_output=True, timeout=600)
    if process.returncode:
        raise ValueError("Core evaluation executable failed; no user data was opened")
    outputs = [json.loads(line) for line in process.stdout.splitlines() if line.strip()]
    if len(outputs) != len(requests) or any("error" in result for result in outputs):
        raise ValueError("Core evaluation returned incomplete or invalid results")
    combined = {"passages": [], "queries": [], "cleanup": [],
                "templates": [], "summary_prompts": [], "citation_cases": []}
    for output in outputs:
        for key in combined:
            combined[key].extend(output.get(key, []))
    combined["wall_ms"] = (time.perf_counter() - started) * 1000
    return combined


def bridge_call(binary, request, measured=False):
    command = [str(binary)]
    if measured and sys.platform == "darwin":
        command = ["/usr/bin/time", "-l", *command]
    started = time.perf_counter()
    process = subprocess.run(command, input=json.dumps(request, ensure_ascii=False) + "\n",
                             text=True, encoding="utf-8", capture_output=True, timeout=180)
    if process.returncode:
        raise ValueError("Owned core evaluation process failed")
    result = json.loads(process.stdout)
    if "error" in result:
        raise ValueError("Core evaluation operation was rejected")
    if measured:
        result["process_wall_ms"] = (time.perf_counter() - started) * 1000
        match = re.search(r"(\d+)\s+maximum resident set size", process.stderr)
        result["peak_rss_bytes"] = int(match.group(1)) if match else None
    return result


def measure_core(corpus, binary):
    from metrics import timing_metrics
    results = []
    panel = []
    for language in ("en", "nl", "mixed"):
        meetings = [m for m in corpus["meetings"] if m["language"] == language and m["split"] == "heldout"]
        long_ids = {m["id"] for m in meetings if m.get("long_input")}
        choices = [q for q in corpus["queries"] if q["language"] == language and q["split"] == "heldout"]
        queries = []
        for mode, check in [("whole_meeting", lambda q: q["meeting_id"] and q["meeting_id"] not in long_ids),
                            ("long_meeting", lambda q: q["meeting_id"] in long_ids),
                            ("library", lambda q: q["meeting_id"] is None)]:
            query = next(q for q in choices if check(q))
            queries.append((mode, query))
            panel.append({"language": language, "mode": mode, "query_id": query["id"]})
        pairs = [c for c in corpus["cleanup_pairs"] if c["language"] == language and c["split"] == "heldout"][:2]
        request = {"op": "evaluate", "meetings": meetings, "include_timings": True,
                   "queries": [q for _, q in queries], "cleanup_pairs": pairs}
        cold = [bridge_call(binary, request, measured=True) for _ in range(5)]
        repeated = [dict(q, id=q["id"] + "-warm-" + str(index))
                    for index in range(31) for _, q in queries]
        warm = bridge_call(binary, dict(request, queries=repeated, cleanup_pairs=[
            dict(pair, id=pair["id"] + "-warm-" + str(index))
            for index in range(31) for pair in pairs]), measured=True)
        by_mode = {}
        for mode, query in queries:
            rows = [q for q in warm["queries"] if q["id"].startswith(query["id"] + "-warm-")][1:]
            by_mode[mode] = {method: timing_metrics([q[method + "_elapsed_ns"] / 1e6 for q in rows])
                             for method in ("ask", "search")}
        cleanup = warm["cleanup"][len(pairs):]
        results.append({"language": language, "transport": "local_core",
                        "cold_definition": "owned process startup plus fresh synthetic library/index",
                        "cold_process": timing_metrics([r["process_wall_ms"] for r in cold]),
                        "cold_index": timing_metrics([r["index_elapsed_ns"] / 1e6 for r in cold]),
                        "warm_queries": by_mode,
                        "warm_cleanup_guard": timing_metrics([r["elapsed_ns"] / 1e6 for r in cleanup]),
                        "peak_rss_bytes": max(r["peak_rss_bytes"] for r in [*cold, warm]
                                              if r.get("peak_rss_bytes") is not None) if any(
                                                  r.get("peak_rss_bytes") is not None for r in [*cold, warm]) else None,
                        "memory_scope": "OS peak resident memory of the owned core executable; not Pennel app memory"})
    return {"panel": panel, "panel_hash": digest(panel), "results": results,
            "binary_bytes": binary.stat().st_size,
            "production_native_binary_increment_bytes": None,
            "production_native_binary_note": "No decision runtime is embedded or packaged by this issue"}


def source_fingerprint():
    paths = [*DIRECTORY.rglob("*.py"), *DIRECTORY.glob("*.json"),
             ROOT / "core/Cargo.toml", *ROOT.joinpath("core/src").rglob("*.rs")]
    return digest({str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                   for path in sorted(paths) if "__pycache__" not in str(path)})


def prepare_manifest(corpus, bridge, binary, config):
    from corpus import corpus_hash
    candidate_data = [{"id": q["id"], "ask": q.get("ask", []),
                       "search": q.get("search", []), "messages": q.get("messages", [])}
                      for q in bridge["queries"]]
    return {"schema_version": 1, "corpus_hash": corpus_hash(corpus),
            "protocol_hash": digest(read_json(DIRECTORY / "protocol.json")),
            "models_hash": digest(read_json(DIRECTORY / "models.json")),
            "source_fingerprint": source_fingerprint(), "config_hash": digest(config),
            "candidates_prompts_hash": digest(candidate_data),
            "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "binary_bytes": binary.stat().st_size, "hardware": hardware(),
            "source_commit": git("rev-parse", "HEAD"),
            "split_policy": "meeting-family-disjoint; calibration only fits calibration",
            "model_download_ceiling_bytes": 3 * 1024 ** 3,
            "hosted_spend_ceiling_usd": "2.00"}


def verify_frozen(frozen, manifest):
    for key in ("corpus_hash", "protocol_hash", "models_hash", "source_fingerprint",
                "config_hash", "candidates_prompts_hash"):
        if frozen.get(key) != manifest.get(key):
            raise ValueError(f"Frozen {key} differs; preregister a new experiment")


def relevance_for(query, passages):
    result = {}
    for passage in passages:
        for evidence in query.get("evidence", []):
            if (passage["meeting_id"] == evidence["meeting_id"]
                    and passage["kind"] == evidence["kind"]
                    and evidence["text"] in passage["text"]):
                result[passage["id"]] = max(result.get(passage["id"], 0),
                                            evidence.get("relevance", 3))
    return result


def mean_defined(rows, key):
    values = [r[key] for r in rows if r.get(key) is not None]
    return sum(values) / len(values) if values else None


def ranking_cases(corpus, bridge, split):
    gold = {q["id"]: q for q in corpus["queries"]}
    cases = []
    for result in bridge["queries"]:
        query = gold[result["id"]]
        if query["split"] != split or result["ask_mode"] == "whole_meeting":
            continue
        candidates = {p["id"]: p for p in result["ask"]}
        cases.append({"id": query["id"] + "-ranking", "task": "ranking",
                      "group_id": query["group_id"], "language": query["language"],
                      "split": split, "query": query["question"],
                      "passages": list(candidates.values()),
                      "baseline_ranked_ids": list(candidates),
                      "relevance": relevance_for(query, bridge["passages"])})
    return cases


def decision_rank(runner, case):
    from decimal import Decimal
    started = time.perf_counter()
    scores, attempts = [], []
    cost = Decimal(0)
    cost_known = True
    identity = None
    for passage in case["passages"]:
        result = runner.predict({"id": case["id"] + "-" + passage["id"],
                                 "state": passage["text"], "question": {
                                     "type": "noul", "instructions": "Does this passage provide relevant evidence for this query? " + case["query"],
                                     "criteria": {"false": "The passage is irrelevant.", "true": "The passage is relevant."}}})
        attempts.extend(result.get("attempts", []))
        identity = result.get("identity", identity)
        if result.get("cost_usd") is None:
            cost_known = False
        else:
            cost += Decimal(result["cost_usd"])
        if result.get("status") != "ok":
            return {"status": result.get("status", "error"), "reason": result.get("reason"),
                    "attempts": attempts, "identity": identity, "cost_usd": str(cost) if cost_known else None}
        scores.append(result["probabilities"]["true"])
    order = sorted(range(len(scores)), key=lambda index: (-scores[index], index))
    return {"status": "ok", "ranked_ids": [case["passages"][i]["id"] for i in order],
            "scores": {p["id"]: score for p, score in zip(case["passages"], scores)},
            "score_type": "decision_relevance_probability", "identity": identity,
            "latency_ms": (time.perf_counter() - started) * 1000,
            "attempts": attempts, "cost_usd": str(cost) if cost_known else None}


def offline_report(corpus, bridge, split="heldout"):
    from metrics import classification_metrics, ranking_metrics
    queries = {q["id"]: q for q in corpus["queries"]}
    pairs = {c["id"]: c for c in corpus["cleanup_pairs"]}
    ranking = []
    for row in bridge["queries"]:
        query = queries[row["id"]]
        if query["split"] != split:
            continue
        gold = relevance_for(query, bridge["passages"])
        if query.get("evidence") and not gold:
            raise ValueError(f"Gold evidence cannot be mapped to indexed passages: {query['id']}")
        for method in ("ask", "search"):
            actual = [p["id"] for p in row[method]]
            ranked = list(dict.fromkeys(actual))
            score = ranking_metrics(ranked, gold, candidate_ids=ranked, k=5)
            score["recall_at_12"] = ranking_metrics(ranked, gold, k=12)["recall_at_k"]
            score["recall_at_25"] = ranking_metrics(ranked, gold, k=25)["recall_at_k"]
            whole = method == "ask" and row.get("ask_mode") == "whole_meeting"
            if whole:
                score["ndcg"] = score["ndcg_at_k"] = None
            ranking.append({"id": row["id"], "language": query["language"],
                            "split": query["split"], "group_id": query["group_id"],
                            "method": "ask_whole" if whole else method, "mode": row.get("ask_mode"),
                            "duplicate_source_count": len(actual) - len(ranked),
                            "answerable": query.get("expected_answer") is not None,
                            "ranked_ids": ranked, "relevance": gold, **score})
    cleanup = []
    for row in bridge["cleanup"]:
        case = pairs[row["id"]]
        if case["split"] != split:
            continue
        cleanup.append({"id": row["id"], "language": case["language"],
                        "split": case["split"], "group_id": case["group_id"],
                        "expected": str(case["expected_safe"]).lower(),
                        "prediction": str(row["accepted"]).lower(),
                        "accepted": True, "critical": case["critical"]})
    languages = {}
    for language in ("en", "nl", "mixed"):
        language_ranking = [r for r in ranking if r["language"] == language
                            and r["split"] == split]
        language_cleanup = [r for r in cleanup if r["language"] == language
                            and r["split"] == split]
        methods = {}
        for method in sorted({r["method"] for r in language_ranking}):
            rows = [r for r in language_ranking if r["method"] == method]
            metric_keys = sorted({key for r in rows for key, value in r.items()
                                  if isinstance(value, (int, float)) and not isinstance(value, bool)})
            methods[method] = {"queries": len(rows), "unanswerable": sum(
                not r["answerable"] for r in rows),
                **{key: mean_defined(rows, key) for key in metric_keys}}
        safe = [r for r in language_cleanup if r["expected"] == "true"]
        critical = [r for r in language_cleanup if r["critical"] and r["expected"] == "false"]
        languages[language] = {"ranking": methods,
                              "cleanup": {**classification_metrics(language_cleanup),
                                          "safe_acceptance": sum(r["prediction"] == "true" for r in safe) / len(safe) if safe else None,
                                          "critical_corruptions_accepted": sum(r["prediction"] == "true" for r in critical),
                                          "critical_cases": len(critical)},
                              "template_suggestions": {"behavior": "manual; no inference", "coverage": 0.0},
                              "speaker_name_suggestions": {"behavior": "manual/acoustic; no text inference", "coverage": 0.0}}
    citation_gold = {c["id"]: c for c in corpus.get("citation_cases", [])}
    citation_rows = [{"id": c["id"], "language": citation_gold[c["id"]]["language"],
                      "split": citation_gold[c["id"]]["split"],
                      "correct": c["numbers"] == citation_gold[c["id"]]["expected_citations"]}
                     for c in bridge.get("citation_cases", [])]
    return {"evidence_kind": "measured_deterministic_core", "split": split, "languages": languages,
            "ranking_rows": ranking, "cleanup_rows": cleanup,
            "citation_syntax": {language: {"count": len(values), "correct": sum(c["correct"] for c in values)}
                                for language in ("en", "nl", "mixed")
                                for values in [[c for c in citation_rows if c["language"] == language and c["split"] == split]]},
            "grounding": {"status": "not_measured", "reason": "Citation syntax is not factual support; no generative inference in offline mode"},
            "recommendations": {
                "ranking": "retain current retrieval; model comparison pending",
                "answerability": "defer model adoption; inference evidence required",
                "claim_support": "defer model adoption; inference evidence required",
                "events": "retain summaries/manual review; structured detector remains deferred",
                "templates": "retain manual selection; no text model adopted",
                "speaker_names": "retain explicit naming; no text model adopted",
                "cleanup": "retain raw fallback; current guard is not a semantic-fidelity guarantee"}}


def live_predictions(corpus, bridge, config, split, output, frozen):
    from local_models import LocalWorker
    from runners import DecisionRunner, SpendLedger
    availability = []
    ledger_path = ROOT / config.get("ledger_path", "build/decision-model-evaluation/spend-ledger.json")
    def checkpoint(state):
        temporary = ledger_path.with_suffix(".tmp")
        save_json(temporary, state)
        temporary.replace(ledger_path)
    predictions = []
    decisions = [case for case in corpus["decisions"] if case["split"] == split]
    output.mkdir(parents=True, exist_ok=True)
    with experiment_lock(ledger_path), output.joinpath("predictions.jsonl").open("w", encoding="utf-8") as stream:
        ledger = (SpendLedger.from_dict(read_json(ledger_path), checkpoint=checkpoint)
                  if ledger_path.exists() else SpendLedger("2.00", checkpoint=checkpoint))
        for item in config.get("runners", []):
            runner = None
            ready = False
            if item.get("enabled") is not True:
                availability.append({"id": item["id"], "status": "unavailable", "reason": "not_explicitly_enabled"})
                continue
            if item.get("transport") == "local_worker":
                try:
                    runner = LocalWorker(item["model"], item["model_dir"], python=item.get("python"))
                    status = runner.start()
                    ready = status.get("status") == "ready"
                    availability.append({"id": item["id"], "status": "available" if ready else "unavailable", "details": status})
                except (ValueError, RuntimeError, OSError) as error:
                    availability.append({"id": item["id"], "status": "unavailable", "reason": type(error).__name__})
            else:
                runner = DecisionRunner(item, ledger=ledger)
                status = runner.preflight()
                availability.append({"id": item["id"], **status})
                if status.get("status") in ("ok", "available") or status.get("available"):
                    ready = True
            if not ready:
                if runner is not None and hasattr(runner, "close"):
                    runner.close()
                continue
            selected_tasks = item.get("tasks")
            cases = [case for case in decisions if not selected_tasks or case["task"] in selected_tasks]
            if not selected_tasks or "ranking" in selected_tasks:
                cases.extend(ranking_cases(corpus, bridge, split))
            halt_reason = None
            for index, case in enumerate(cases):
                try:
                    result = ({"case_id": case["id"], "status": "unavailable", "prediction": None,
                               "reason": halt_reason} if halt_reason else
                              decision_rank(runner, case) if case["task"] == "ranking" and item.get("transport") != "local_worker"
                              else runner.predict(case))
                except (ValueError, RuntimeError, OSError) as error:
                    result = {"case_id": case["id"], "status": "error", "prediction": None,
                              "reason": type(error).__name__}
                result.update({"runner_id": item["id"], "expected": case.get("expected"),
                               "language": case["language"], "task": case["task"],
                               "subtask": case.get("subtask"), "tags": case.get("tags", []),
                               "group_id": case["group_id"], "split": case["split"],
                               "question": case.get("question"), "label_order": case.get("label_order"),
                               "prompt_hash": digest({"corpus_hash": frozen["corpus_hash"],
                                                      "task": case["task"], "subtask": case.get("subtask"),
                                                      "primitive": case.get("question", {}).get("type")}),
                               "run_manifest_hash": digest(frozen),
                               "question_grouping": item.get("question_grouping", "one_case_per_request"),
                               "phase": "client_first" if index == 0 else "warm"})
                if case["task"] == "ranking":
                    from metrics import ranking_metrics
                    result.update(baseline_ranked_ids=case["baseline_ranked_ids"], relevance=case["relevance"])
                    if result.get("status") == "ok":
                        result["ranking_metrics"] = ranking_metrics(result["ranked_ids"], case["relevance"],
                                                                    candidate_ids=case["baseline_ranked_ids"])
                    result["baseline_metrics"] = ranking_metrics(case["baseline_ranked_ids"], case["relevance"])
                predictions.append(result)
                stream.write(json.dumps(result, ensure_ascii=False) + "\n")
                stream.flush()
                if index % 100 == 0:
                    print(json.dumps({"runner": item["id"], "split": split,
                                      "completed": index + 1, "total": len(cases)}), file=sys.stderr)
                if result.get("reason") in ("budget_exhausted", "unresolved_charge"):
                    halt_reason = result["reason"]
            if hasattr(runner, "close"):
                runner.close()
    save_json(output / "predictions.json", {"frozen": digest(frozen), "rows": predictions})
    save_json(output / "availability.json", availability)
    return predictions, availability


def predictions_report(rows):
    from metrics import classification_metrics, timing_metrics
    groups = defaultdict(list)
    for row in rows:
        if row["task"] == "ranking":
            continue
        groups[(row["runner_id"], row["language"], row["task"], row.get("subtask"))].append(row)
    return [{"runner_id": runner, "language": language, "task": task,
             "subtask": subtask,
             "rows": len(values), "measured": sum(v.get("status") == "ok" for v in values),
             "quality": classification_metrics([dict(v, prediction=v.get("prediction") if v.get("status") == "ok" else None,
                                                       probabilities=v.get("probabilities") if v.get("status") == "ok" else None) for v in values]),
             "warm_latency": timing_metrics([v["latency_ms"] for v in values if v.get("phase") == "warm" and v.get("status") == "ok" and v.get("latency_ms") is not None]),
             "load_ms": next((v.get("load_ms") for v in values if v.get("load_ms") is not None), None),
             "cost_usd": [v.get("cost_usd") for v in values]}
            for (runner, language, task, subtask), values in sorted(groups.items(), key=lambda item: str(item[0]))]


def reranking_report(rows):
    from metrics import paired_bootstrap, timing_metrics
    groups = defaultdict(list)
    for row in rows:
        if row["task"] == "ranking":
            groups[(row["runner_id"], row["language"])].append(row)
    reports = []
    for (runner, language), values in sorted(groups.items()):
        measured = [r for r in values if r.get("status") == "ok"]
        pairs = [{"baseline": r["baseline_metrics"]["ndcg"], "candidate": r["ranking_metrics"]["ndcg"],
                  "group_id": r["group_id"], "language": language, "split": r["split"]}
                 for r in measured if r["baseline_metrics"]["ndcg"] is not None]
        reports.append({"runner_id": runner, "language": language, "cases": len(values),
                        "measured_cases": len(measured), "baseline_ndcg_at_5": mean_defined([r["baseline_metrics"] for r in measured], "ndcg"),
                        "candidate_ndcg_at_5": mean_defined([r["ranking_metrics"] for r in measured], "ndcg"),
                        "candidate_recall": mean_defined([r["ranking_metrics"] for r in measured], "candidate_recall"),
                        "paired_meeting_bootstrap": paired_bootstrap(pairs),
                        "warm_latency": timing_metrics([r["latency_ms"] for r in measured if r.get("latency_ms") is not None])})
    return reports


def replay_recorded(report, directory, corpus):
    from calibration import score_predictions
    recorded = read_json(directory / "manifest.json")
    from corpus import corpus_hash
    if recorded["corpus_hash"] != corpus_hash(corpus) or recorded["protocol_hash"] != digest(read_json(DIRECTORY / "protocol.json")):
        raise ValueError("Recorded inference belongs to a different corpus or protocol")
    with gzip.open(directory / "predictions.jsonl.gz", "rt", encoding="utf-8") as stream:
        rows = [json.loads(line) for line in stream if line.strip()]
    manifest_hash = digest(recorded)
    if any(row.get("run_manifest_hash") != manifest_hash for row in rows):
        raise ValueError("Recorded prediction identity does not match its manifest")
    report["recorded_inference"] = {"evidence_kind": "replay_of_measured_local_inference",
                                    "manifest": recorded, "source": str(directory)}
    report["live"] = predictions_report(rows)
    report["reranking"] = reranking_report(rows)
    report["calibrated"] = score_predictions([r for r in rows if r["task"] != "ranking"],
                                             read_json(directory / "calibration.json"), run_manifest_hash=manifest_hash)
    for key, filename in [("model_resources", "resources.json"), ("core_resources", "core-resources.json"),
                          ("availability", "availability.json"), ("generative", "generative.json")]:
        if (directory / filename).is_file():
            report[key] = read_json(directory / filename)


def markdown_report(report):
    lines = ["# Decision-model evaluation", "", "Synthetic fixtures only. See the run manifest for identity and limitations.", "",
             "## Measured deterministic held-out baselines", "",
             "| Language | Method | nDCG@5 | Candidate recall |", "| --- | --- | ---: | ---: |"]
    for language, data in report["offline"]["languages"].items():
        for method, values in data["ranking"].items():
            ndcg = values.get("ndcg", values.get("ndcg@5"))
            recall = values.get("candidate_recall", values.get("recall"))
            lines.append(f"| {language} | {method} | {ndcg} | {recall} |")
    lines.extend(["", "| Language | Valid cleanups accepted | Critical corruptions accepted |", "| --- | ---: | ---: |"])
    for language, data in report["offline"]["languages"].items():
        lines.append(f"| {language} | {data['cleanup']['safe_acceptance']} | {data['cleanup']['critical_corruptions_accepted']}/{data['cleanup']['critical_cases']} |")
    lines.extend(["", "## Recommendations", ""])
    for task, recommendation in report["offline"]["recommendations"].items():
        lines.append(f"- {task}: {recommendation}.")
    lines.extend(["", "## Limits", "", "- Offline scores measure the real deterministic core; they are not model inference scores.",
                  "- No hosted/server memory, packaged native runtime, Windows/Linux inference or device acceptance is established.",
                  "- Unsupported, unavailable and unresolved judgments remain open gates.", ""])
    if report.get("live"):
        lines.extend(["## Optional measured inference", "", "| Runner | Language | Task | Subtask | Measured / total | Macro-F1 | Coverage | Brier | Warm p95 ms |",
                      "| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: |"])
        for row in report["live"]:
            quality = row["quality"]
            lines.append(f"| {row['runner_id']} | {row['language']} | {row['task']} | {row['subtask']} | {row['measured']}/{row['rows']} | {quality['macro_f1']} | {quality['coverage']} | {quality['brier']} | {row['warm_latency']['p95_ms']} |")
        lines.extend(["", "Brier/ECE require full class probabilities. Label-only outputs remain unmeasured for those metrics.", ""])
    if report.get("reranking"):
        lines.extend(["## Reranking frozen candidates", "", "| Runner | Language | Measured / total | Baseline nDCG@5 | Candidate nDCG@5 | Candidate recall |",
                      "| --- | --- | ---: | ---: | ---: | ---: |"])
        for row in report["reranking"]:
            lines.append(f"| {row['runner_id']} | {row['language']} | {row['measured_cases']}/{row['cases']} | {row['baseline_ndcg_at_5']} | {row['candidate_ndcg_at_5']} | {row['candidate_recall']} |")
    if report.get("availability"):
        lines.extend(["", "## Availability", ""])
        for item in report["availability"]:
            reason = item.get("reason", item.get("details", {}).get("reason", ""))
            lines.append(f"- {item['id']}: {item['status']} {reason}".rstrip())
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--offline", action="store_true")
    mode.add_argument("--prepare", action="store_true", help="Freeze inputs without scoring held-out outputs")
    mode.add_argument("--live", action="store_true")
    mode.add_argument("--calibrate", type=Path, help="Fit only calibration predictions, without inference")
    parser.add_argument("--config", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--split", choices=("development", "calibration", "heldout"), default="development")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--calibration", type=Path, help="Frozen calibration artifact for held-out predictions")
    parser.add_argument("--measure", action="store_true", help="Collect the fixed cold/warm core latency panel")
    parser.add_argument("--replay", type=Path, help="Offline replay of a published measured-inference receipt directory")
    parser.add_argument("--output", type=Path, default=ROOT / "build/decision-model-evaluation/offline")
    args = parser.parse_args(argv)
    if args.live and (not args.config or not args.frozen):
        parser.error("--live requires explicit --config and --frozen")
    if args.live and args.measure and args.split != "heldout":
        parser.error("The fixed local model latency panel requires --split heldout")
    if args.replay and not args.offline:
        parser.error("--replay is only valid with --offline")
    if args.calibrate:
        from calibration import fit_predictions
        if not args.frozen:
            parser.error("--calibrate requires --frozen")
        predictions = read_json(args.calibrate)
        frozen_hash = digest(read_json(args.frozen))
        if predictions.get("frozen") != frozen_hash:
            parser.error("Predictions belong to a different frozen experiment")
        rows = [r for r in predictions["rows"] if r["task"] != "ranking"]
        artifact = fit_predictions(rows, run_manifest_hash=frozen_hash)
        save_json(args.output / "calibration.json", artifact)
        print(json.dumps({"status": "calibrated", "contexts": len(artifact["fits"]),
                          "artifact": str(args.output / "calibration.json")}))
        return 0
    from corpus import build_corpus, validate_corpus
    corpus = build_corpus()
    validate_corpus(corpus)
    config = read_json(args.config) if args.config else {"runners": []}
    if "transport" in config:
        config = {"runners": [{"id": "configured-runner", **config}]}
    binary = bridge_binary(args.binary)
    bridge = collect_bridge(corpus, binary)
    manifest = prepare_manifest(corpus, bridge, binary, config)
    from measurements import panel_manifest
    manifest["local_latency_panel"] = panel_manifest(config.get("runners", []), corpus,
                                                     ranking_cases(corpus, bridge, "heldout"))
    if args.prepare:
        if not args.frozen:
            parser.error("--prepare requires --frozen output path")
        save_json(args.frozen, manifest)
        save_json(args.output / "corpus.json", corpus)
        save_json(args.output / "core-artifacts.json", bridge)
        print(json.dumps({"status": "prepared", "manifest_hash": digest(manifest), "meetings": len(corpus["meetings"])}))
        return 0
    frozen = read_json(args.frozen) if args.frozen else manifest
    verify_frozen(frozen, manifest)
    if args.live and args.split == "heldout":
        relative = str(args.frozen.resolve().relative_to(ROOT))
        if git("status", "--porcelain", "--", relative) or not git("ls-files", "--", relative):
            parser.error("Held-out inference requires a committed frozen manifest")
        if git("status", "--porcelain", "--", "evaluations/decision-models", "core/src", "core/Cargo.toml"):
            parser.error("Commit the evaluation implementation before held-out inference")
        if frozen["hardware"] != manifest["hardware"] or frozen["binary_sha256"] != manifest["binary_sha256"]:
            parser.error("Held-out hardware or evaluation executable differs from the frozen experiment")
    report = {"schema_version": 1, "manifest": manifest,
              "offline": offline_report(corpus, bridge, split=args.split if args.live else "heldout"), "live": []}
    if args.replay:
        replay_recorded(report, args.replay, corpus)
    if args.measure:
        report["core_resources"] = measure_core(corpus, binary)
    if args.live:
        rows, availability = live_predictions(corpus, bridge, config, args.split, args.output, frozen)
        report["live"] = predictions_report(rows)
        report["reranking"] = reranking_report(rows)
        report["availability"] = availability
        if args.split == "heldout" and args.calibration:
            from calibration import score_predictions
            report["calibrated"] = score_predictions([r for r in rows if r["task"] != "ranking"],
                                                     read_json(args.calibration), run_manifest_hash=digest(frozen))
        if config.get("generative"):
            from generative import GenerativeRunner, run_baselines
            generated = []
            for item in config["generative"]:
                generated.extend(run_baselines(corpus, bridge, GenerativeRunner(item), args.split,
                                               lambda request: bridge_call(binary, request)))
            save_json(args.output / "generative-predictions.json", generated)
            report["generative"] = {"rows": len(generated), "measured": sum(r["status"] == "ok" for r in generated),
                                    "pending_judgments": sum(bool(r.get("adjudication")) for r in generated),
                                    "availability": [r for r in generated if r["task"] == "generative_availability"]}
        if args.measure:
            from measurements import measure_local
            report["model_resources"] = measure_local(config.get("runners", []), corpus,
                                                       ranking_cases(corpus, bridge, "heldout"))
    save_json(args.output / "manifest.json", manifest)
    save_json(args.output / "report.json", report)
    args.output.joinpath("report.md").write_text(markdown_report(report), encoding="utf-8")
    save_json(args.output / "core-artifacts.json", bridge)
    print(json.dumps({"status": "complete", "evidence": "suite run; issue acceptance remains separate",
                      "report": str(args.output / "report.md"), "semantic_hash": digest(report["offline"])}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Evaluation stopped: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(1)
