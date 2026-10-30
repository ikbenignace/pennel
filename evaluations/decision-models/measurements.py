"""Fixed held-out latency panels for fresh, owned local CPU workers only."""

import hashlib
import json
import math

from local_models import LocalWorker, registry
from metrics import timing_metrics


def _panel(corpus, ranking_cases):
    selected = {}
    for case in sorted([*corpus["decisions"], *ranking_cases], key=lambda row: row["id"]):
        if case.get("split") != "heldout" or case.get("variant_of") or "long_input" in case.get("tags", []):
            continue
        selected.setdefault((case["language"], case["task"]), case)
    return [selected[key] for key in sorted(selected)]


def _selected(config, cases):
    return [case for case in cases if not config.get("tasks") or case["task"] in config["tasks"]]


def _cold_cases(cases):
    selected = {}
    for case in sorted(cases, key=lambda row: (row["task"] != "cleanup", row["task"] != "ranking", row["id"])):
        selected.setdefault(case["language"], case)
    return [selected[key] for key in sorted(selected)]


def panel_manifest(configs, corpus, ranking_cases, cold_samples=5, warm_samples=30):
    if any(isinstance(n, bool) or not isinstance(n, int) or not 1 <= n <= 1000 for n in (cold_samples, warm_samples)):
        raise ValueError("measurement sample counts must be positive integers no greater than 1000")
    cases = _panel(corpus, ranking_cases)
    manifest = {"version": 1, "split": "heldout", "cold_samples_per_language": cold_samples,
                "warm_samples_per_case": warm_samples, "excluded_warmups_per_case": 1,
                "cases_sha256": hashlib.sha256(json.dumps(cases, sort_keys=True, ensure_ascii=False).encode()).hexdigest(),
                "case_ids": [case["id"] for case in cases], "runners": []}
    for config in configs:
        if config.get("transport") == "local_worker" and config.get("enabled") is True:
            subset = _selected(config, cases)
            manifest["runners"].append({"runner_id": config.get("id", config["model"]),
                                        "case_ids": [case["id"] for case in subset],
                                        "cold_case_ids": [case["id"] for case in _cold_cases(subset)]})
    manifest["panel_hash"] = hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()
    return manifest


def _valid_number(value):
    return not isinstance(value, bool) and isinstance(value, (int, float)) and math.isfinite(value) and value >= 0


def _infer(worker, case):
    try:
        result = worker.predict(case)
        if not isinstance(result, dict):
            return {"status": "error", "reason": "invalid_worker_record"}
        if result.get("status") == "ok" and not _valid_number(result.get("latency_ms")):
            return {"status": "error", "reason": "missing_or_invalid_latency"}
        return result
    except Exception as error:
        return {"status": "error", "reason": type(error).__name__}


def measure_local(configs, corpus, ranking_cases, worker_factory=LocalWorker, cold_samples=5, warm_samples=30):
    manifest = panel_manifest(configs, corpus, ranking_cases, cold_samples, warm_samples)
    cases, entries, results = _panel(corpus, ranking_cases), registry(), []
    for config in configs:
        if config.get("transport") != "local_worker" or config.get("enabled") is not True:
            continue
        subset = _selected(config, cases)
        rows = {case["id"]: {"case_id": case["id"], "language": case["language"], "task": case["task"],
                            "cold_samples": [], "warm_samples": [], "cold_outcomes": [], "warm_outcomes": [],
                            "load_samples": [], "process_start_samples": []} for case in subset}
        row = {"runner_id": config.get("id", config["model"]), "model": config["model"], "status": "unavailable",
               "measurement": "measured_local_inference" if worker_factory is LocalWorker else "mocked_worker",
               "startup_attempts": 0, "load_samples": [], "process_start_samples": [], "peak_rss_bytes": None,
               "artifact_manifest_bytes": entries.get(config["model"], {}).get("download_bytes"),
               "artifact_size_source": "registry download metadata", "native_binary_size_bytes": None,
               "limitations": ["Cold means a fresh owned process; operating-system file caches are uncontrolled.",
                               "RSS covers the owned CPU worker; GPU, private user processes and server memory are unmeasured.",
                               "No native Pennel package is built by this resource panel."], "failures": []}

        def observe(record, worker):
            values = [record.get("peak_rss_bytes"), getattr(worker, "observed_peak_rss_bytes", None),
                      (record.get("memory_guard") or {}).get("observed_peak_rss_bytes")]
            measured = [value for value in values if _valid_number(value) and value > 0]
            if measured:
                row["peak_rss_bytes"] = max([row["peak_rss_bytes"] or 0, *measured])
            if record.get("identity"):
                row["identity"] = record["identity"]

        def start_worker():
            worker = None
            row["startup_attempts"] += 1
            try:
                worker = worker_factory(config["model"], config["model_dir"], python=config.get("python"))
                startup = worker.start()
                observe(startup, worker)
                if startup.get("status") != "ready":
                    row["failures"].append({"stage": "startup", "status": startup.get("status", "error"),
                                            "reason": startup.get("reason", "worker_not_ready")})
                    worker.close()
                    return None, startup
                return worker, startup
            except Exception as error:
                if worker is not None:
                    worker.close()
                row["failures"].append({"stage": "startup", "status": "error", "reason": type(error).__name__})
                return None, {}

        available = bool(subset)
        for case in _cold_cases(subset):
            for _ in range(cold_samples):
                worker, startup = start_worker()
                if worker is None:
                    available = False
                    break
                try:
                    for key in ("load_ms", "process_start_ms"):
                        if _valid_number(startup.get(key)):
                            row["load_samples" if key == "load_ms" else "process_start_samples"].append(startup[key])
                            rows[case["id"]]["load_samples" if key == "load_ms" else "process_start_samples"].append(startup[key])
                    result = _infer(worker, case)
                    observe(result, worker)
                    rows[case["id"]]["cold_outcomes"].append(result.get("status", "error"))
                    if result.get("status") == "ok":
                        rows[case["id"]]["cold_samples"].append(result["latency_ms"])
                finally:
                    worker.close()
            if not available:
                break
        if available:
            worker, startup = start_worker()
            if worker is not None:
                try:
                    row["warm_startup"] = startup
                    for case in subset:
                        warmup = _infer(worker, case)
                        observe(warmup, worker)
                        rows[case["id"]]["warmup_status"] = warmup.get("status", "error")
                        if warmup.get("status") != "ok":
                            continue
                        for _ in range(warm_samples):
                            result = _infer(worker, case)
                            observe(result, worker)
                            rows[case["id"]]["warm_outcomes"].append(result.get("status", "error"))
                            if result.get("status") == "ok":
                                rows[case["id"]]["warm_samples"].append(result["latency_ms"])
                finally:
                    worker.close()
        row["load"] = timing_metrics(row.pop("load_samples"))
        row["process_start"] = timing_metrics(row.pop("process_start_samples"))
        for cell in rows.values():
            for phase in ("cold", "warm"):
                cell[phase] = timing_metrics(cell.pop(phase + "_samples"))
                outcomes = cell.pop(phase + "_outcomes")
                cell[phase + "_status_counts"] = {status: outcomes.count(status) for status in sorted(set(outcomes))}
            cell["load"] = timing_metrics(cell.pop("load_samples"))
            cell["process_start"] = timing_metrics(cell.pop("process_start_samples"))
            cell["cold_measured"] = cell["cold"]["n"] > 0
        row["cases"] = list(rows.values())
        complete = rows and all(cell["warm"]["n"] == warm_samples for cell in rows.values())
        complete = complete and all(rows[case["id"]]["cold"]["n"] == cold_samples for case in _cold_cases(subset))
        row["status"] = "ok" if complete else "unavailable_or_partial"
        results.append(row)
    return {"panel": manifest, "models": results}
