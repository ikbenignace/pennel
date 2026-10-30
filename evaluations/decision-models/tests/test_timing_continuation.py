import copy
from decimal import Decimal
from pathlib import Path
import sys
import threading
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hosted_measurements import panel_manifest
from timing_continuation import execute_continuation, plan_continuation


CONFIG = {"id": "mercury", "enabled": True, "transport": "openrouter", "model": "synthetic/model-v1",
          "upstream_tag": "provider", "privacy": {"processing_region": "global", "data_collection": "deny", "zdr": True}}
PREFLIGHT = {"status": "ok", "catalog": {"canonical_slug": "synthetic/model-v1-pinned"},
             "endpoints": [{"tag": "provider", "provider_name": "Provider", "quantization": "unknown"}],
             "pricing": {"prompt": "0.00001", "completion": "0"}}


def fixture(rank=False):
    cases = [{"id": "answer", "language": "en", "task": "answerability", "split": "heldout", "state": "Synthetic",
              "question": {"type": "noul", "instructions": "Supported?"}},
             {"id": "claim", "language": "en", "task": "claim_support", "split": "heldout", "state": "Synthetic",
              "question": {"type": "noul", "instructions": "Supported?"}}]
    ranking = [{"id": "rank", "language": "en", "task": "ranking", "split": "heldout", "query": "Synthetic?",
                "passages": [{"id": "a", "text": "First"}, {"id": "b", "text": "Second"}]}] if rank else []
    corpus = {"decisions": cases}
    panel = panel_manifest([CONFIG], corpus, ranking)
    records = [{"case_id": panel["runners"][0]["cold_case_ids"][0], "phase": "cold_client", "status": "ok", "latency_ms": 5} for _ in range(5)]
    records += [{"case_id": "answer", "phase": "warm", "status": "ok", "latency_ms": 2} for _ in range(30)]
    records += [{"case_id": "claim", "phase": "warm", "status": "ok", "latency_ms": 3} for _ in range(7)]
    records += [{"case_id": "claim", "phase": "warm", "status": "error", "reason": "http_502", "latency_ms": 100}]
    original = {"panel": panel, "models": [{"runner_id": "mercury", "model": CONFIG["model"], "preflight": PREFLIGHT,
                                            "record_count": len(records), "records": records}]}
    return original, corpus, ranking


class Ledger:
    def __init__(self, limit="2", unresolved=False):
        self.limit, self.spent, self.unresolved = Decimal(limit), Decimal(0), unresolved
    def snapshot(self):
        return {"limit_usd": str(self.limit), "known_billed_usd": str(self.spent), "reserved_usd": "0",
                "unresolved_charge": self.unresolved}


class Factory:
    def __init__(self, changed=False, unknown=False):
        self.calls, self.instances, self.changed, self.unknown = [], [], changed, unknown
    def __call__(self, config, ledger, cancel_event=None):
        owner = self
        class Runner:
            _price = Decimal("0.00001")
            def preflight(self):
                result = copy.deepcopy(PREFLIGHT)
                if owner.changed:
                    result["catalog"]["canonical_slug"] = "different-model"
                return result
            def _questions(self, case):
                return {"decision": case["question"]}
            def _payload(self, case, questions):
                return {}, 10
            def predict(self, case):
                owner.calls.append(case["id"])
                ledger.spent += Decimal("0.00001")
                ledger.unresolved = owner.unknown
                return {"status": "error" if owner.unknown else "ok", "latency_ms": 4,
                        "probabilities": {"true": 0.8}, "cost_usd": None if owner.unknown else "0.00001",
                        "identity": {"provider": "Provider", "requested_model": CONFIG["model"],
                                     "returned_model": PREFLIGHT["catalog"]["canonical_slug"],
                                     "routing": {"only": ["provider"], "allow_fallbacks": False}, "privacy": CONFIG["privacy"]},
                        "attempts": [{"billed_cost_usd": None if owner.unknown else "0.00001"}]}
        runner = Runner()
        self.instances.append(runner)
        return runner


class TimingContinuationTests(unittest.TestCase):
    def test_failed_legacy_slot_is_consumed_and_inputs_are_preserved(self):
        original, corpus, ranking = fixture()
        unchanged = copy.deepcopy(original)
        plan = plan_continuation(original, [CONFIG], corpus, ranking)
        claim = next(cell for cell in plan["models"][0]["cases"] if cell["case_id"] == "claim")
        self.assertEqual(claim["missing_indices"], list(range(8, 30)))
        self.assertEqual(original, unchanged)
        self.assertEqual(plan_continuation(original, [CONFIG], corpus, ranking)["plan_hash"], plan["plan_hash"])

    def test_continuation_never_retries_failed_or_completed_slots(self):
        original, corpus, ranking = fixture()
        factory, saved = Factory(), []
        result = execute_continuation(original, [CONFIG], corpus, ranking, Ledger(), factory, checkpoint=saved.append)
        self.assertEqual(len(factory.calls), 23)
        self.assertEqual(len(result["new_records"]), 23)
        self.assertEqual(saved, result["new_records"])
        self.assertEqual(sum(row["phase"] == "continuation_warmup" for row in saved), 1)
        self.assertEqual([row["sample_index"] for row in saved if row["phase"] == "warm"], list(range(8, 30)))
        claim = next(cell for cell in result["models"][0]["cases"] if cell["case_id"] == "claim")
        self.assertEqual(claim["status_counts"], {"error": 1, "ok": 29})
        self.assertEqual(claim["warm"]["n"], 29)
        self.assertEqual(claim["failed"]["n"], 1)
        self.assertTrue(result["complete_coverage"])
        self.assertFalse(result["all_passed"])
        replay = copy.deepcopy(original)
        replay["models"][0].update(records=result["models"][0]["records"], record_count=result["models"][0]["record_count"])
        empty_factory = Factory()
        repeated = execute_continuation(replay, [CONFIG], corpus, ranking, Ledger(), empty_factory)
        self.assertTrue(repeated["complete_coverage"])
        self.assertEqual(empty_factory.calls, [])

    def test_legacy_one_based_and_null_indices_are_supported(self):
        original, corpus, ranking = fixture()
        for index, row in enumerate(row for row in original["models"][0]["records"] if row["phase"] == "warm" and row["case_id"] == "answer"):
            row.update(sample_index=None, planned_sample_index=index + 1)
        plan = plan_continuation(original, [CONFIG], corpus, ranking)
        self.assertEqual(next(cell for cell in plan["models"][0]["cases"] if cell["case_id"] == "answer")["missing_indices"], [])

    def test_duplicates_out_of_bounds_unknown_cases_and_compact_inputs_rejected(self):
        for change in (lambda row: row.update(sample_index=0), lambda row: row.update(sample_index=30),
                       lambda row: row.update(sample_index=True), lambda row: row.update(case_id="unknown")):
            original, corpus, ranking = fixture()
            change(original["models"][0]["records"][6])
            with self.assertRaises(ValueError):
                plan_continuation(original, [CONFIG], corpus, ranking)
        original, corpus, ranking = fixture()
        del original["models"][0]["records"]
        with self.assertRaisesRegex(ValueError, "full_timing_receipts_required"):
            plan_continuation(original, [CONFIG], corpus, ranking)

    def test_full_remaining_bound_includes_warmups_and_ranking_fanout(self):
        original, corpus, ranking = fixture(rank=True)
        factory = Factory()
        result = execute_continuation(original, [CONFIG], corpus, ranking, Ledger(limit="0.008"), factory)
        self.assertEqual(Decimal(result["total_bound_usd"]), Decimal("0.0085"))
        self.assertEqual(factory.calls, [])
        self.assertEqual(result["new_records"], [])

    def test_route_drift_unknown_cost_and_precancel_never_continue_paid_work(self):
        original, corpus, ranking = fixture()
        changed = Factory(changed=True)
        with self.assertRaisesRegex(ValueError, "continuation_route_changed"):
            execute_continuation(original, [CONFIG], corpus, ranking, Ledger(), changed)
        self.assertEqual(changed.calls, [])
        factory = Factory(unknown=True)
        result = execute_continuation(original, [CONFIG], corpus, ranking, Ledger(), factory)
        self.assertEqual(len(factory.calls), 1)
        self.assertEqual(result["new_records"][0]["phase"], "continuation_warmup")
        self.assertFalse(result["complete_coverage"])
        event, factory = threading.Event(), Factory()
        event.set()
        execute_continuation(original, [CONFIG], corpus, ranking, Ledger(), factory, cancel_event=event)
        self.assertEqual(factory.instances, [])


if __name__ == "__main__":
    unittest.main()
