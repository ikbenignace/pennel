from decimal import Decimal
import copy
import json
from pathlib import Path
import sys
import threading
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import hosted_measurements
from runners import EvaluationError


def corpus():
    cases = []
    for language in ("en", "nl", "mixed"):
        for task in ("cleanup", "answerability"):
            case = {"id": f"b-{language}-{task}", "task": task, "language": language,
                    "split": "heldout", "state": "Synthetic", "tags": [],
                    "question": {"type": "noul", "instructions": "Supported?"}}
            cases.extend([case, {**case, "id": "a-long-" + case["id"], "tags": ["long_input"]},
                          {**case, "id": "a-variant-" + case["id"], "variant_of": case["id"]}])
    return {"decisions": cases}


def ranking():
    return [{"id": "ranking-" + language, "task": "ranking", "language": language, "split": "heldout",
             "query": "Synthetic?", "passages": [{"id": "a", "text": "First"}, {"id": "b", "text": "Second"}]}
            for language in ("en", "nl", "mixed")]


def config(count=1):
    return {"runners": [{"id": "hosted-" + str(index), "model": "synthetic/version-1",
                          "transport": "openrouter", "enabled": True} for index in range(count)]}


class Ledger:
    def __init__(self, limit="2", spent="0", unresolved=False):
        self.limit, self.spent, self.unresolved = Decimal(limit), Decimal(spent), unresolved
    def snapshot(self):
        return {"limit_usd": str(self.limit), "known_billed_usd": str(self.spent),
                "reserved_usd": "0", "unresolved_charge": self.unresolved}


class Factory:
    def __init__(self, unavailable=False, unsupported=False, fail_after=None, cancel_event=None):
        self.instances, self.calls = [], []
        self.unavailable, self.unsupported, self.fail_after = unavailable, unsupported, fail_after
        self.cancel_event = cancel_event
        self.active, self.maximum_active = 0, 0

    def __call__(self, config, ledger, cancel_event=None):
        owner = self
        class Runner:
            _price = Decimal("0.00001")
            def __init__(self):
                self.count = 0
            def preflight(self):
                return {"status": "unavailable", "reason": "synthetic_unavailable"} if owner.unavailable else {"status": "ok"}
            def _questions(self, case):
                return {"decision": case["question"]}
            def _payload(self, case, questions):
                if owner.unsupported and case.get("task") == "answerability":
                    raise EvaluationError("state_byte_limit_exceeded_no_truncation")
                return {}, 10
            def predict(self, case):
                if cancel_event is not None and cancel_event.is_set():
                    return {"status": "unavailable", "reason": "cancelled", "attempts": []}
                self.count += 1
                owner.calls.append(case["id"])
                owner.active += 1
                owner.maximum_active = max(owner.maximum_active, owner.active)
                ledger.spent += Decimal("0.00001")
                failed = owner.fail_after is not None and len(owner.calls) >= owner.fail_after
                if failed:
                    ledger.unresolved = True
                identity = {"returned_model": config["model"], "provider": "SyntheticProvider",
                            "request_id": "synthetic-" + str(len(owner.calls)), "routing": {"allow_fallbacks": False}}
                result = {"status": "error" if failed else "ok", "latency_ms": 7 if self.count == 1 else 2,
                          "probabilities": {"true": 0.8}, "identity": identity,
                          "usage": {"input_tokens": 10, "output_tokens": 0},
                          "cost_usd": None if failed else "0.00001",
                          "attempts": [{"identity": identity, "usage": {"input_tokens": 10},
                                        "billed_cost_usd": None if failed else "0.00001"}]}
                if owner.cancel_event:
                    owner.cancel_event.set()
                owner.active -= 1
                return result
        instance = Runner()
        self.instances.append(instance)
        return instance


class HostedMeasurementTests(unittest.TestCase):
    def test_manifest_is_side_effect_free_stable_and_excludes_other_transports(self):
        settings = config()
        settings["runners"].extend([dict(settings["runners"][0], enabled=False),
                                    {"enabled": True, "transport": "direct"}])
        first = hosted_measurements.panel_manifest(settings, corpus(), ranking())
        second = hosted_measurements.panel_manifest(settings, {"decisions": list(reversed(corpus()["decisions"]))}, list(reversed(ranking())))
        self.assertEqual(first, second)
        self.assertEqual(len(first["runners"]), 1)
        self.assertEqual(len(first["runners"][0]["case_ids"]), 9)
        self.assertEqual(len(first["runners"][0]["cold_case_ids"]), 3)
        self.assertEqual(first["latency_concurrency"], 1)
        self.assertEqual(first["warm_samples_per_case"], 30)

    def test_entire_panel_bound_includes_ranking_fanout_and_prevents_unfair_partial_execution(self):
        factory, ledger = Factory(), Ledger(limit="0.05")
        report = hosted_measurements.measure_hosted(config(2), corpus(), ranking(), ledger, factory)
        self.assertEqual(Decimal(report["total_bound_usd"]), Decimal("0.0774"))
        self.assertEqual(report["reason"], "panel_bound_exceeds_remaining_budget")
        self.assertEqual(factory.calls, [])
        self.assertEqual(len(factory.instances), 2)
        self.assertTrue(all(not row["records"] for row in report["models"]))
        self.assertEqual(report["planned_paid_requests"], 774)

    def test_admission_uses_remaining_experiment_budget_not_initial_limit(self):
        factory, ledger = Factory(), Ledger(limit="0.05", spent="0.02")
        report = hosted_measurements.measure_hosted(config(), corpus(), ranking(), ledger, factory)
        self.assertEqual(report["remaining_before_usd"], "0.03")
        self.assertEqual(report["reason"], "panel_bound_exceeds_remaining_budget")
        self.assertEqual(factory.calls, [])

    def test_changed_fresh_client_route_does_not_start_inference(self):
        class ChangingFactory(Factory):
            def __call__(self, *args, **kwargs):
                runner = super().__call__(*args, **kwargs)
                runner._catalog = {"canonical_slug": "synthetic/model-" + str(len(self.instances))}
                return runner
        factory = ChangingFactory()
        report = hosted_measurements.measure_hosted(config(), corpus(), ranking(), Ledger(), factory)
        self.assertEqual(factory.calls, [])
        self.assertEqual(report["models"][0]["reason"], "fresh_client_unavailable_or_route_changed")

    def test_cold_client_and_warm_counts_receipts_and_serial_execution(self):
        factory, ledger = Factory(), Ledger()
        checkpointed = []
        report = hosted_measurements.measure_hosted(config(), corpus(), ranking(), ledger, factory,
                                                     checkpoint=checkpointed.append)
        model = report["models"][0]
        self.assertEqual(report["status"], "ok")
        self.assertEqual(model["measurement"], "mocked_runner")
        self.assertEqual(len(factory.instances), 16)
        self.assertEqual(len(factory.calls), 387)
        self.assertEqual(len(checkpointed), len(model["records"]))
        self.assertTrue(all(row["runner_id"] == model["runner_id"] for row in checkpointed))
        self.assertEqual(factory.maximum_active, 1)
        self.assertIsNone(model["server_load_ms"])
        self.assertIsNone(model["server_memory_bytes"])
        self.assertIsNone(model["download_bytes"])
        self.assertEqual(model["client_setup"]["n"], 15)
        self.assertEqual(sum(len(row.get("attempts", [])) for row in model["records"]), 387)
        self.assertTrue(all(row["attempts"][0]["identity"]["request_id"] for row in model["records"]))
        for cell in model["cases"]:
            self.assertEqual(cell["warm"]["n"], 30)
            if cell["task"] == "cleanup":
                self.assertEqual(cell["cold_client"]["n"], 5)
                self.assertEqual(cell["client_setup"]["n"], 5)
                self.assertEqual(cell["cold_client"]["p50_ms"], 7)
                self.assertEqual(cell["warm"]["p95_ms"], 2)
            else:
                self.assertEqual(cell["cold_client"]["n"], 0)
                self.assertIsNone(cell["cold_client"]["p50_ms"])
        self.assertIn("not server/model-cold", " ".join(report["limitations"]))

    def test_unknown_charges_and_precancel_prevent_inference(self):
        for ledger, cancelled in ((Ledger(unresolved=True), None), (Ledger(), threading.Event())):
            if cancelled is not None:
                cancelled.set()
            factory = Factory()
            report = hosted_measurements.measure_hosted(config(), corpus(), ranking(), ledger, factory, cancel_event=cancelled)
            self.assertEqual(factory.instances, [])
            self.assertEqual(factory.calls, [])
            self.assertIn(report["reason"], ("unreconciled_budget", "cancelled"))

    def test_unknown_charge_during_panel_stops_further_paid_requests(self):
        factory, ledger = Factory(fail_after=1), Ledger()
        report = hosted_measurements.measure_hosted(config(2), corpus(), ranking(), ledger, factory)
        self.assertEqual(len(factory.calls), 1)
        self.assertEqual(report["status"], "unmeasured_or_partial")
        self.assertTrue(report["ledger"]["unresolved_charge"])

    def test_unavailable_and_unsupported_cases_remain_unmeasured(self):
        factory = Factory(unavailable=True)
        report = hosted_measurements.measure_hosted(config(), corpus(), ranking(), Ledger(), factory)
        self.assertEqual(factory.calls, [])
        self.assertEqual(report["models"][0]["reason"], "synthetic_unavailable")
        factory = Factory(unsupported=True)
        report = hosted_measurements.measure_hosted(config(), corpus(), ranking(), Ledger(), factory)
        for row in report["models"][0]["cases"]:
            if row["task"] == "answerability":
                self.assertEqual(row["warm"]["n"], 0)
                self.assertEqual(row["unmeasured_reason"], "state_byte_limit_exceeded_no_truncation")


if __name__ == "__main__":
    unittest.main()
