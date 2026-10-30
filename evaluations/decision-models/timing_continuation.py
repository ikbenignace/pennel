"""Continue unconsumed warm slots in a frozen hosted panel; failed slots stay consumed."""

import copy
from decimal import Decimal
import hashlib
import json

from hosted_measurements import _budget, _configs, _request_cases, panel_manifest
from measurements import _panel, _selected, _valid_number
from metrics import timing_metrics
from runners import DecisionRunner


def _route(config, preflight):
    tag = config.get("upstream_tag", config.get("provider"))
    endpoint = next((row for row in preflight.get("endpoints", []) if row.get("tag") == tag), None)
    if endpoint is None:
        raise ValueError("frozen_route_missing")
    return {"model": config["model"], "canonical": preflight.get("catalog", {}).get("canonical_slug"),
            "provider": endpoint.get("provider_name"), "tag": tag, "precision": endpoint.get("quantization"),
            "pricing": preflight.get("pricing", endpoint.get("pricing")),
            "base_url": config.get("base_url", "https://openrouter.ai"), "api_surface": config.get("api_surface", "decisions"),
            "privacy": {key: config.get("privacy", {}).get(key) for key in ("processing_region", "data_collection", "zdr")}}


def _index(records, case_ids, cold_ids, runner_id):
    counters, used, result = {}, set(), []
    for source in records:
        row = copy.deepcopy(source)
        case_id, phase = row.get("case_id"), row.get("phase")
        if (case_id not in case_ids or phase not in ("warm", "cold_client", "warmup", "continuation_warmup")
                or row.get("runner_id", runner_id) != runner_id or row.get("status") not in ("ok", "error", "unavailable")):
            raise ValueError("unexpected_timing_record")
        if phase in ("warm", "cold_client"):
            key = (case_id, phase)
            legacy = row.get("planned_sample_index")
            index = row.get("sample_index")
            if index is None:
                index = legacy - 1 if isinstance(legacy, int) and not isinstance(legacy, bool) else counters.get(key, 0)
            limit = 30 if phase == "warm" else 5
            if (isinstance(index, bool) or not isinstance(index, int) or not 0 <= index < limit
                    or (phase == "cold_client" and case_id not in cold_ids)
                    or (legacy is not None and (isinstance(legacy, bool) or not isinstance(legacy, int) or legacy != index + 1))):
                raise ValueError("timing_sample_index_out_of_bounds")
            if (*key, index) in used:
                raise ValueError("duplicate_timing_sample_index")
            used.add((*key, index))
            counters[key] = counters.get(key, 0) + 1
            row["sample_index"] = index
        result.append(row)
    return result


def plan_continuation(original, config, corpus, ranking_cases):
    fresh = panel_manifest(config, corpus, ranking_cases)
    frozen = original["panel"]
    if frozen.get("cases_sha256") != fresh["cases_sha256"] or frozen.get("warm_samples_per_case") != 30 or frozen.get("cold_client_samples_per_language") != 5:
        raise ValueError("frozen_timing_panel_mismatch")
    cases, models, items = _panel(corpus, ranking_cases), [], _configs(config)
    identifiers = [item.get("id", item["model"]) for item in items]
    if len(identifiers) != len(set(identifiers)):
        raise ValueError("duplicate_continuation_runner")
    for item in items:
        identifier = item.get("id", item["model"])
        prior = next((row for row in original["models"] if row["runner_id"] == identifier), None)
        entry = next((row for row in frozen["runners"] if row["runner_id"] == identifier), None)
        subset = _selected(item, cases)
        if prior is None or entry is None or prior.get("model") != item["model"] or entry["case_ids"] != [row["id"] for row in subset]:
            raise ValueError("frozen_timing_runner_mismatch")
        if prior.get("record_count", 0) and "records" not in prior:
            raise ValueError("full_timing_receipts_required")
        if "record_count" in prior and prior["record_count"] != len(prior.get("records", [])):
            raise ValueError("timing_receipt_count_mismatch")
        records = _index(prior.get("records", []), set(entry["case_ids"]), set(entry["cold_case_ids"]), identifier)
        expected = _route(item, prior["preflight"])
        for row in records:
            identity = row.get("identity", {})
            if row["status"] == "ok" and identity:
                route = identity.get("routing", {})
                privacy = {key: identity.get("privacy", {}).get(key) for key in expected["privacy"]}
                if (identity.get("provider") != expected["provider"] or identity.get("requested_model") != item["model"]
                        or identity.get("returned_model") not in (item["model"], expected["canonical"])
                        or route.get("only") != [expected["tag"]] or route.get("allow_fallbacks") is not False
                        or privacy != expected["privacy"]):
                    raise ValueError("frozen_receipt_route_mismatch")
        cells = [{"case_id": case["id"], "language": case["language"], "task": case["task"],
                  "missing_indices": sorted(set(range(30)) - {row["sample_index"] for row in records if row["case_id"] == case["id"] and row["phase"] == "warm"})} for case in subset]
        models.append({"runner_id": identifier, "model": item["model"], "expected_route": expected,
                       "records": records, "cases": cells, "cold_case_ids": entry["cold_case_ids"]})
    plan = {"panel_hash": frozen["panel_hash"], "models": models, "latency_concurrency": 1,
            "warm_slots_per_case": 30, "policy": "Failed slots are consumed; one excluded warmup per incomplete cell."}
    plan["plan_hash"] = hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest()
    return plan


def _summarize(model):
    model["record_count"] = len(model["records"])
    for cell in model["cases"]:
        rows = [row for row in model["records"] if row["case_id"] == cell["case_id"] and row["phase"] == "warm"]
        cell.update(consumed_slots=len(rows), complete_coverage=len(rows) == 30,
                    all_passed=len(rows) == 30 and all(row["status"] == "ok" and _valid_number(row.get("latency_ms")) for row in rows),
                    status_counts={status: sum(row["status"] == status for row in rows) for status in sorted({row["status"] for row in rows})},
                    warm=timing_metrics([row["latency_ms"] for row in rows if row["status"] == "ok" and _valid_number(row.get("latency_ms"))]),
                    failed=timing_metrics([row["latency_ms"] for row in rows if row["status"] != "ok" and _valid_number(row.get("latency_ms"))]))
    model["cold_complete"] = all(sum(row["phase"] == "cold_client" and row["case_id"] == identifier for row in model["records"]) == 5 for identifier in model["cold_case_ids"])
    model["complete_coverage"] = model["cold_complete"] and all(cell["complete_coverage"] for cell in model["cases"])


def execute_continuation(original, config, corpus, ranking_cases, ledger, runner_factory=DecisionRunner, cancel_event=None, checkpoint=None):
    from run import decision_rank
    plan = plan_continuation(original, config, corpus, ranking_cases)
    output = {"plan": plan, "new_records": [], "models": copy.deepcopy(plan["models"]), "status": "unmeasured"}
    cases = {row["id"]: row for row in _panel(corpus, ranking_cases)}
    runners, total = {}, Decimal(0)
    blocked = _budget(ledger) is None or (cancel_event is not None and cancel_event.is_set())
    for model in output["models"]:
        pending = [cell for cell in model["cases"] if cell["missing_indices"]]
        if not pending or blocked:
            continue
        item = next(row for row in _configs(config) if row.get("id", row["model"]) == model["runner_id"])
        runner = runner_factory(item, ledger=ledger, cancel_event=cancel_event)
        available = runner.preflight()
        if available.get("status") != "ok" or _route(item, available) != model["expected_route"]:
            raise ValueError("continuation_route_changed")
        for cell in pending:
            bounds = [runner._payload(request, runner._questions(request))[1] * runner._price for request in _request_cases(cases[cell["case_id"]], decision_rank)]
            if any(bound > Decimal(str(item.get("max_request_cost_usd", "0.10"))) for bound in bounds):
                raise ValueError("continuation_request_bound_exceeds_cap")
            total += sum(bounds, Decimal(0)) * (len(cell["missing_indices"]) + 1)
        runners[model["runner_id"]] = runner
    remaining = _budget(ledger)
    output.update(total_bound_usd=str(total), remaining_before_usd=None if remaining is None else str(remaining))
    stopped = blocked or remaining is None or total > remaining or (cancel_event is not None and cancel_event.is_set())
    output["reason"] = "remaining_panel_fits_budget" if not stopped else "unreconciled_or_insufficient_budget_or_cancelled"
    for model in output["models"]:
        runner = runners.get(model["runner_id"])
        for cell in model["cases"]:
            if stopped or not cell["missing_indices"]:
                continue
            case = cases[cell["case_id"]]
            for index in [None, *cell["missing_indices"]]:
                if _budget(ledger) is None or (cancel_event is not None and cancel_event.is_set()):
                    stopped = True
                    break
                try:
                    result = decision_rank(runner, case) if case["task"] == "ranking" else runner.predict(case)
                except Exception as error:
                    result = {"status": "error", "reason": type(error).__name__}
                record = {**result, "runner_id": model["runner_id"], "case_id": case["id"], "task": case["task"], "language": case["language"],
                          "phase": "continuation_warmup" if index is None else "warm"}
                if index is not None:
                    record.update(sample_index=index, planned_sample_index=index + 1)
                model["records"].append(record)
                output["new_records"].append(record)
                if checkpoint is not None:
                    checkpoint(record)
                if _budget(ledger) is None or (index is None and result.get("status") != "ok"):
                    stopped = True
                    break
        _summarize(model)
    output["complete_coverage"] = all(model["complete_coverage"] for model in output["models"])
    output["all_passed"] = output["complete_coverage"] and all(cell["all_passed"] for model in output["models"] for cell in model["cases"])
    output["status"] = "complete_coverage" if output["complete_coverage"] else "partial"
    output["ledger"] = ledger.snapshot()
    return output
