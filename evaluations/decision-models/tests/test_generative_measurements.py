import copy
from decimal import Decimal
from pathlib import Path
import sys
import threading
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import generative_measurements


def inputs():
    corpus = {"meetings": [], "queries": [], "cleanup_pairs": []}
    bridge = {"queries": [], "cleanup": [], "summary_prompts": []}
    for language in ("en", "nl", "mixed"):
        for index in (0, 1, 2):
            identifier = f"{language}-{index}"
            messages = [{"role": "user", "content": "Synthetic Zoë 🙂 " + identifier}]
            corpus["meetings"].append({"id": identifier, "language": language, "split": "heldout", "long_input": index == 0})
            for question in (0, 1):
                case_id = identifier + "-q" + str(question)
                corpus["queries"].append({"id": case_id, "meeting_id": identifier})
                bridge["queries"].append({"id": case_id, "messages": messages})
            pair = identifier + "-cleanup"
            corpus["cleanup_pairs"].append({"id": pair, "meeting_id": identifier})
            bridge["cleanup"].append({"id": pair, "messages": messages})
            bridge["summary_prompts"].append({"meeting_id": identifier, "messages": messages, "map_reduce": index < 2})
    return {"corpus": corpus, "bridges": {"heldout": bridge}}


def config(**changes):
    settings = {"enabled": True, "provider": "openrouter", "max_completion_tokens": 10,
                "max_input_tokens": 10000, "max_request_cost_usd": "0.01", "model": "synthetic/model-v1",
                "expected_returned_model": "synthetic/model-v1-pinned", "upstream_tag": "synthetic"}
    settings.update(changes)
    return settings


class Ledger:
    def __init__(self, limit="2", unresolved=False):
        self.limit, self.spent, self.unresolved = Decimal(limit), Decimal(0), unresolved
    def snapshot(self):
        return {"limit_usd": str(self.limit), "known_billed_usd": str(self.spent),
                "reserved_usd": "0", "unresolved_charge": self.unresolved}


class Factory:
    def __init__(self, unknown=False, changed=False):
        self.calls, self.instances, self.unknown, self.changed = [], [], unknown, changed
    def __call__(self, settings, ledger):
        owner = self
        class Runner:
            _input_price, _output_price = Decimal("0.00000001"), Decimal("0.00000002")
            _endpoint, _catalog = {"context_length": 10000}, {"context_length": 10000}
            def __init__(self):
                self.count, self.bridge_call = 0, None
            def preflight(self):
                return {"status": "ok", "catalog": {"canonical_slug": "changed" if owner.changed and len(owner.instances) > 1 else "pinned"},
                        "endpoint": {"tag": "synthetic", "provider_name": "Provider", "pricing": {"prompt": str(self._input_price)}}}
            def complete(self, messages, reasoning):
                self.count += 1
                owner.calls.append({"messages": messages, "reasoning": reasoning, "bridge": self.bridge_call})
                ledger.spent += Decimal("0.000001")
                ledger.unresolved = owner.unknown
                return {"status": "error" if owner.unknown else "ok", "text": "Synthetic generated output",
                        "latency_ms": 7 if self.count == 1 else 2,
                        "cost_usd": None if owner.unknown else "0.000001", "usage": {"prompt_tokens": 10},
                        "identity": {"request_id": "synthetic-" + str(len(owner.calls)), "provider": "Provider",
                                     "requested_model": settings["model"], "returned_model": settings["expected_returned_model"],
                                     "routing": {"only": [settings["upstream_tag"]], "allow_fallbacks": False}},
                        "attempts": [{"billed_cost_usd": None if owner.unknown else "0.000001"}]}
        runner = Runner()
        self.instances.append(runner)
        return runner


class GenerativeMeasurementsTests(unittest.TestCase):
    def test_manifest_selects_short_q0_cleanup_and_non_map_summary_per_language(self):
        data = inputs()
        first = generative_measurements.panel_manifest(data)
        reordered = copy.deepcopy(data)
        for key in reordered["bridges"]["heldout"]:
            reordered["bridges"]["heldout"][key].reverse()
        self.assertEqual(first, generative_measurements.panel_manifest(reordered))
        self.assertEqual(first["planned_posts"], 294)
        self.assertEqual(len(first["cases"]), 9)
        self.assertTrue(all(case["id"].endswith("1-q0") for case in first["cases"] if case["task"] == "meeting_qa"))
        self.assertTrue(all(case["id"].endswith("2-summary") for case in first["cases"] if case["task"] == "summary"))

    def test_fixed_counts_setup_receipts_and_no_semantic_quality_credit(self):
        factory, ledger, bridge = Factory(), Ledger(), lambda request: request
        result = generative_measurements.measure(inputs(), config(), ledger, bridge, factory)
        self.assertEqual(result["status"], "complete_coverage")
        self.assertTrue(result["all_passed"])
        self.assertEqual(len(factory.instances), 16)
        self.assertEqual(len(factory.calls), 294)
        self.assertTrue(all(call["bridge"] is bridge for call in factory.calls))
        self.assertEqual(len(result["records"]), 294)
        self.assertTrue(all(not row["quality_evaluation"] and "text" not in row for row in result["records"]))
        self.assertIsNone(result["server_memory_bytes"])
        for cell in result["cases"]:
            self.assertEqual(cell["warm"]["n"], 30)
            self.assertEqual(cell["warm"]["p95_ms"], 2)
            if cell["task"] == "meeting_qa":
                self.assertEqual(cell["cold_client"]["n"], 5)
                self.assertEqual(cell["cold_client"]["p50_ms"], 7)
                self.assertEqual(cell["client_setup"]["n"], 5)
            else:
                self.assertEqual(cell["cold_client"]["n"], 0)

    def test_whole_panel_budget_context_and_request_cap_checked_before_paid_work(self):
        for settings, ledger in ((config(), Ledger(limit="0.0001")),
                                  (config(max_input_tokens=1000), Ledger()),
                                  (config(max_request_cost_usd="0.000001"), Ledger())):
            factory = Factory()
            result = generative_measurements.measure(inputs(), settings, ledger, None, factory)
            self.assertEqual(result["status"], "unmeasured")
            self.assertEqual(factory.calls, [])
            self.assertEqual(ledger.spent, Decimal(0))

    def test_unknown_cost_cancellation_and_changed_route_stop_later_calls(self):
        factory = Factory(unknown=True)
        result = generative_measurements.measure(inputs(), config(), Ledger(), None, factory)
        self.assertEqual(len(factory.calls), 1)
        self.assertEqual(result["status"], "partial")
        self.assertTrue(result["ledger"]["unresolved_charge"])
        event, factory = threading.Event(), Factory()
        event.set()
        generative_measurements.measure(inputs(), config(), Ledger(), None, factory, cancel_event=event)
        self.assertEqual(factory.instances, [])
        factory = Factory(changed=True)
        result = generative_measurements.measure(inputs(), config(), Ledger(), None, factory)
        self.assertEqual(factory.calls, [])
        self.assertEqual(result["reason"], "fresh_generative_client_route_changed")

    def test_required_language_task_coverage_cannot_be_silently_dropped(self):
        data = inputs()
        data["bridges"]["heldout"]["summary_prompts"] = []
        with self.assertRaisesRegex(ValueError, "complete_short"):
            generative_measurements.panel_manifest(data)
        factory = Factory()
        generative_measurements.measure(inputs(), config(enabled=False), Ledger(), None, factory)
        self.assertEqual(factory.instances, [])

    def test_resume_preserves_failed_slots_and_only_fills_missing_samples(self):
        initial = generative_measurements.measure(inputs(), config(), Ledger(), None, Factory())
        records = copy.deepcopy(initial["records"])
        removed = next(row for row in records if row["phase"] == "warm" and row["sample_index"] == 29)
        records.remove(removed)
        failed = next(row for row in records if row["phase"] == "warm" and row["sample_index"] == 28 and row["case_id"] == removed["case_id"])
        failed.update(status="error", error="generative_http_502")
        factory = Factory()
        resumed = generative_measurements.measure(inputs(), config(), Ledger(), None, factory, prior_records=records)
        self.assertEqual(len(factory.calls), 2)
        self.assertEqual([row["phase"] for row in resumed["new_records"]], ["continuation_warmup", "warm"])
        self.assertEqual(resumed["new_records"][-1]["sample_index"], 29)
        self.assertTrue(resumed["complete_coverage"])
        self.assertFalse(resumed["all_passed"])
        again = Factory()
        finished = generative_measurements.measure(inputs(), config(), Ledger(), None, again, prior_records=resumed["records"])
        self.assertEqual(again.calls, [])
        self.assertTrue(finished["complete_coverage"])

    def test_resume_rejects_duplicate_out_of_bounds_and_changed_route_receipts(self):
        first = generative_measurements.measure(inputs(), config(), Ledger(), None, Factory())["records"][0]
        for changed in ([first, first], [dict(first, sample_index=5)],
                        [dict(first, identity={"requested_model": "different"})]):
            with self.assertRaises(ValueError):
                generative_measurements.measure(inputs(), config(), Ledger(), None, Factory(), prior_records=changed)


if __name__ == "__main__":
    unittest.main()
