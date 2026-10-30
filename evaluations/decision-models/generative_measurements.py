"""Fixed current-core generative timing panel; repetitions do not add quality evidence."""

from decimal import Decimal
import copy
import hashlib
import json
import time

from generative import OpenRouterGenerativeRunner
from hosted_measurements import _budget
from measurements import _valid_number
from metrics import timing_metrics


def _cases(inputs):
    corpus, bridge = inputs["corpus"], inputs["bridges"]["heldout"]
    meetings = {row["id"]: row for row in corpus["meetings"] if row["split"] == "heldout"}
    queries = {row["id"]: row for row in corpus["queries"]}
    pairs = {row["id"]: row for row in corpus["cleanup_pairs"]}
    selected = {}
    for task, rows in (("meeting_qa", bridge["queries"]), ("cleanup", bridge["cleanup"]), ("summary", bridge["summary_prompts"])):
        for source in sorted(rows, key=lambda row: row.get("id", row.get("meeting_id", ""))):
            if task == "meeting_qa":
                case = queries[source["id"]]
                meeting_id = case.get("origin_meeting_id") or case.get("meeting_id")
                if not source["id"].endswith("-q0"):
                    continue
            else:
                meeting_id = pairs[source["id"]]["meeting_id"] if task == "cleanup" else source["meeting_id"]
            meeting = meetings.get(meeting_id)
            if not meeting or meeting.get("long_input") or task == "summary" and source.get("map_reduce"):
                continue
            identifier = source["id"] if task != "summary" else meeting_id + "-summary"
            selected.setdefault((meeting["language"], task), {"id": identifier, "language": meeting["language"], "task": task,
                                                            "messages": source["messages"], "reasoning": "off" if task == "cleanup" else "low"})
    expected = {(language, task) for language in ("en", "nl", "mixed") for task in ("meeting_qa", "cleanup", "summary")}
    if set(selected) != expected:
        raise ValueError("complete_short_generative_timing_panel_required")
    return [selected[key] for key in sorted(selected)]


def panel_manifest(inputs):
    cases = _cases(inputs)
    manifest = {"version": 1, "split": "heldout", "latency_concurrency": 1,
                "cold_client_samples_per_language": 5, "warm_samples_per_case": 30,
                "excluded_warmups_per_case": 1, "planned_posts": 294,
                "cases": [{key: case[key] for key in ("id", "language", "task")} for case in cases],
                "cases_sha256": hashlib.sha256(json.dumps(cases, sort_keys=True, ensure_ascii=False).encode()).hexdigest()}
    manifest["panel_hash"] = hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()
    return manifest


def _route(availability):
    endpoint = availability.get("endpoint", {})
    return (availability.get("catalog", {}).get("canonical_slug"), endpoint.get("tag"),
            endpoint.get("provider_name"), endpoint.get("quantization"), endpoint.get("pricing"))


def measure(inputs, config, ledger, bridge_call, runner_factory=OpenRouterGenerativeRunner, checkpoint=None, cancel_event=None, prior_records=None):
    settings, cases = config.get("generative", config), _cases(inputs)
    prior, used = copy.deepcopy(prior_records or []), set()
    lookup = {case["id"]: case for case in cases}
    for record in prior:
        case, phase, index = lookup.get(record.get("case_id")), record.get("phase"), record.get("sample_index")
        if (case is None or phase not in ("cold_client", "warm", "warmup", "continuation_warmup")
                or record.get("task") != case["task"] or record.get("language") != case["language"]):
            raise ValueError("unexpected_generative_timing_receipt")
        if phase in ("cold_client", "warm"):
            limit = 5 if phase == "cold_client" else 30
            if (isinstance(index, bool) or not isinstance(index, int) or not 0 <= index < limit
                    or phase == "cold_client" and case["task"] != "meeting_qa"):
                raise ValueError("generative_timing_sample_index_out_of_bounds")
            key = (case["id"], phase, index)
            if key in used:
                raise ValueError("duplicate_generative_timing_sample")
            used.add(key)
        identity = record.get("identity", {})
        if (identity.get("requested_model") != settings["model"]
                or identity.get("returned_model") not in (None, settings["expected_returned_model"])
                or identity.get("routing", {}).get("only") != [settings["upstream_tag"]]
                or identity.get("routing", {}).get("allow_fallbacks") is not False):
            raise ValueError("generative_timing_receipt_route_mismatch")
    missing = lambda case, phase: [index for index in range(5 if phase == "cold_client" else 30)
                                    if (case["id"], phase, index) not in used]
    result = {"panel": panel_manifest(inputs), "status": "unmeasured", "records": prior, "new_records": [], "cases": [],
              "latency_concurrency": 1, "quality_evaluation": False,
              "measurement": "measured_openrouter_generative_end_to_end" if runner_factory is OpenRouterGenerativeRunner else "mocked_runner",
              "server_load_ms": None, "server_memory_bytes": None, "download_bytes": None, "native_binary_size_bytes": None,
              "limitations": ["Cold-client means first inference after a fresh client and metadata setup; server/model cold state is unknown.",
                              "Timing repetitions are excluded from semantic quality adjudication; server resources are unmeasured."]}
    if settings.get("enabled") is not True or settings.get("provider") != "openrouter":
        result["reason"] = "explicit_openrouter_generative_configuration_required"
        return result
    if _budget(ledger) is None or cancel_event is not None and cancel_event.is_set():
        result["reason"] = "unreconciled_budget_or_cancelled"
        return result
    setup_started = time.perf_counter()
    runner = runner_factory(settings, ledger=ledger)
    runner.bridge_call = bridge_call
    available = runner.preflight()
    result.update(preflight=available, planning_setup_ms=(time.perf_counter() - setup_started) * 1000)
    if available.get("status") != "ok":
        result["reason"] = available.get("error", "runner_unavailable")
        return result
    if any(record.get("identity", {}).get("provider") not in (None, available.get("endpoint", {}).get("provider_name")) for record in prior):
        raise ValueError("generative_timing_receipt_provider_mismatch")
    bounds = {}
    for case in cases:
        bound = len(json.dumps(case["messages"], ensure_ascii=False).encode("utf-8")) + settings.get("billing_overhead_tokens", 2048) + 1024
        output_limit = settings["max_completion_tokens"]
        context = min(settings["max_input_tokens"], runner._endpoint.get("context_length") or runner._catalog.get("context_length") or settings["max_input_tokens"])
        if bound + output_limit > context or sum(len(row["content"]) for row in case["messages"]) > settings.get("max_input_chars", 100000):
            result["reason"] = "generative_timing_context_bound_exceeded"
            return result
        cost = bound * runner._input_price + output_limit * runner._output_price
        if cost > Decimal(str(settings.get("max_request_cost_usd", "0.01"))):
            result["reason"] = "generative_timing_request_cost_bound_exceeded"
            return result
        bounds[case["id"]] = cost
    total = sum((bounds[case["id"]] * (len(missing(case, "warm")) + bool(missing(case, "warm"))
                                      + (len(missing(case, "cold_client")) if case["task"] == "meeting_qa" else 0))
                 for case in cases), Decimal(0))
    remaining = _budget(ledger)
    result.update(total_bound_usd=str(total), remaining_before_usd=None if remaining is None else str(remaining))
    if remaining is None or total > remaining:
        result["reason"] = "whole_generative_panel_exceeds_reconciled_budget"
        return result
    cells = {case["id"]: {"case_id": case["id"], "task": case["task"], "language": case["language"],
                          "client_setup_samples": [row["client_setup_ms"] for row in prior if row["case_id"] == case["id"] and _valid_number(row.get("client_setup_ms"))]} for case in cases}

    def invoke(client, case, phase, index=None, setup_ms=None):
        if _budget(ledger) is None or cancel_event is not None and cancel_event.is_set():
            return False
        prediction = client.complete(case["messages"], reasoning=case["reasoning"])
        record = {key: value for key, value in prediction.items() if key != "text"}
        record.update(case_id=case["id"], task=case["task"], language=case["language"], phase=phase,
                      sample_index=index, quality_evaluation=False, output_chars=len(prediction["text"]) if isinstance(prediction.get("text"), str) else None)
        if setup_ms is not None:
            record["client_setup_ms"] = setup_ms
        if record.get("status") == "ok" and not _valid_number(record.get("latency_ms")):
            record.update(status="error", error="latency_unreported")
        result["records"].append(record)
        result["new_records"].append(record)
        if checkpoint is not None:
            checkpoint(record)
        return _budget(ledger) is not None

    stopped = False
    for case in (case for case in cases if case["task"] == "meeting_qa"):
        for index in missing(case, "cold_client"):
            if stopped:
                break
            started = time.perf_counter()
            fresh = runner_factory(settings, ledger=ledger)
            fresh.bridge_call = bridge_call
            status = fresh.preflight()
            setup = (time.perf_counter() - started) * 1000
            cells[case["id"]]["client_setup_samples"].append(setup)
            if status.get("status") != "ok" or _route(status) != _route(available):
                result["reason"], stopped = "fresh_generative_client_route_changed", True
                break
            stopped = not invoke(fresh, case, "cold_client", index, setup)
    for case in cases:
        if stopped:
            break
        if not missing(case, "warm"):
            continue
        if not invoke(runner, case, "continuation_warmup" if prior_records else "warmup") or result["records"][-1].get("status") != "ok":
            stopped = True
            break
        for index in missing(case, "warm"):
            if not invoke(runner, case, "warm", index):
                stopped = True
                break
    for cell in cells.values():
        cell["client_setup"] = timing_metrics(cell.pop("client_setup_samples"))
        for phase in ("cold_client", "warm"):
            rows = [row for row in result["records"] if row["case_id"] == cell["case_id"] and row["phase"] == phase]
            cell[phase] = timing_metrics([row["latency_ms"] for row in rows if row.get("status") == "ok" and _valid_number(row.get("latency_ms"))])
            cell[phase + "_status_counts"] = {status: sum(row.get("status") == status for row in rows) for status in sorted({row.get("status", "error") for row in rows})}
            cell[phase + "_consumed_slots"] = len(rows)
        cell["complete_coverage"] = cell["warm_consumed_slots"] == 30 and (cell["task"] != "meeting_qa" or cell["cold_client_consumed_slots"] == 5)
    result["cases"] = list(cells.values())
    result["complete_coverage"] = all(cell["complete_coverage"] for cell in cells.values())
    result["all_passed"] = result["complete_coverage"] and all(row.get("status") == "ok" for row in result["records"])
    result["status"] = "complete_coverage" if result["complete_coverage"] else "partial"
    result["known_billed_usd"] = str(sum((Decimal(row["cost_usd"]) for row in result["records"] if row.get("cost_usd") is not None), Decimal(0)))
    result["cost_complete"] = all(row.get("cost_usd") is not None for row in result["records"] if row.get("attempts"))
    result["ledger"] = ledger.snapshot()
    return result
