import contextlib
import io
import json
from pathlib import Path
import sys
import unittest
import tempfile
from unittest.mock import patch
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import run
from corpus import build_corpus


class OrchestrationTests(unittest.TestCase):
    def test_live_requires_explicit_configuration_and_freeze(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            run.main(["--live"])

    def test_latency_panel_cannot_bypass_heldout_guard(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            run.main(["--live", "--split", "development", "--measure", "--config", "unused", "--frozen", "unused"])

    def test_experiment_ledger_cannot_be_owned_twice(self):
        with tempfile.TemporaryDirectory() as directory:
            ledger = Path(directory) / "ledger.json"
            with run.experiment_lock(ledger):
                with self.assertRaises(ValueError):
                    with run.experiment_lock(ledger):
                        pass
            self.assertFalse(ledger.with_suffix(".lock").exists())

    def test_budget_stop_preserves_full_planned_coverage_denominator(self):
        import runners
        class Runner:
            calls = 0
            def __init__(self, *args, **kwargs):
                pass
            def preflight(self):
                return {"status": "ok"}
            def predict(self, case):
                type(self).calls += 1
                if self.calls > 1:
                    return {"status": "unavailable", "reason": "budget_exhausted"}
                return {"status": "ok", "prediction": "true"}
        cases = [{"id": str(i), "meeting_id": "m", "task": "answerability", "expected": "true",
                  "language": "en", "group_id": "g", "split": "calibration",
                  "question": {"type": "noul", "instructions": "Supported?"}} for i in range(5)]
        with tempfile.TemporaryDirectory() as directory, patch.object(runners, "DecisionRunner", Runner):
            config = {"ledger_path": str(Path(directory) / "ledger.json"), "runners": [
                {"id": "fixture", "enabled": True, "transport": "openrouter", "tasks": ["answerability"]}]}
            with contextlib.redirect_stderr(io.StringIO()):
                rows, _ = run.live_predictions({"decisions": cases}, {}, config, "calibration",
                                               Path(directory), {"corpus_hash": "fixed"})
        self.assertEqual(Runner.calls, 2)
        self.assertEqual(len(rows), 5)
        self.assertEqual(run.predictions_report(rows)[0]["quality"]["coverage"], .2)

    def test_bridge_libraries_include_same_split_cross_meeting_candidates(self):
        corpus = build_corpus()
        observed = []
        def execute(command, **kwargs):
            requests = [json.loads(line) for line in kwargs["input"].splitlines()]
            observed.extend(requests)
            return SimpleNamespace(returncode=0, stdout="\n".join("{}" for _ in requests))
        with patch.object(run.subprocess, "run", side_effect=execute):
            run.collect_bridge(corpus, Path("owned-eval"))
        self.assertEqual(len(observed), 9)
        for request in observed:
            self.assertEqual(len(request["meetings"]), 10)
            self.assertEqual(len({m["language"] for m in request["meetings"]}), 1)
            self.assertEqual(len({m["split"] for m in request["meetings"]}), 1)
            ids = {m["id"] for m in request["meetings"]}
            for query in request["queries"]:
                self.assertTrue(set(query["candidate_meeting_ids"]).issubset(ids))

    def test_relevance_uses_exact_kind_and_meeting_span(self):
        query = {"evidence": [{"meeting_id": "a", "kind": "transcript", "text": "€50", "relevance": 3}]}
        passages = [{"id": "a1", "meeting_id": "a", "kind": "transcript", "text": "Keep €50."},
                    {"id": "b1", "meeting_id": "b", "kind": "transcript", "text": "Keep €50."},
                    {"id": "a2", "meeting_id": "a", "kind": "summary", "text": "Keep €50."}]
        self.assertEqual(run.relevance_for(query, passages), {"a1": 3})

    def test_frozen_configuration_and_source_drift_are_rejected(self):
        manifest = dict.fromkeys(("corpus_hash", "protocol_hash", "models_hash", "source_fingerprint",
                                  "config_hash", "candidates_prompts_hash"), "fixed")
        run.verify_frozen(manifest, manifest)
        with self.assertRaises(ValueError):
            run.verify_frozen(manifest, dict(manifest, config_hash="changed"))

    def test_failed_cases_stay_in_coverage_denominator(self):
        base = {"runner_id": "candidate", "language": "nl", "task": "answerability", "expected": "true"}
        rows = [dict(base, status="ok", prediction="true", latency_ms=1),
                dict(base, status="unavailable", prediction=None)]
        report = run.predictions_report(rows)[0]
        self.assertEqual(report["quality"]["coverage"], .5)
        self.assertEqual(report["quality"]["count"], 2)
        self.assertIsNone(report["quality"]["brier"])

    def test_decision_ranking_preserves_scores_and_each_billed_attempt(self):
        class Runner:
            def predict(self, case):
                value = .2 if case["state"] == "first" else .8
                return {"status": "ok", "probabilities": {"false": 1-value, "true": value},
                        "cost_usd": "0.01", "attempts": [{"id": case["id"]}],
                        "identity": {"transport": "openrouter"}}
        case = {"id": "rank", "query": "budget", "passages": [{"id": "a", "text": "first"},
                                                                    {"id": "b", "text": "second"}]}
        result = run.decision_rank(Runner(), case)
        self.assertEqual(result["ranked_ids"], ["b", "a"])
        self.assertEqual(result["cost_usd"], "0.02")
        self.assertEqual(len(result["attempts"]), 2)

    def test_ranking_excludes_whole_context_and_freezes_unique_candidates(self):
        corpus = {"queries": [{"id": "q", "split": "heldout", "group_id": "g", "language": "en",
                               "question": "budget", "evidence": []}]}
        passage = {"id": "p", "meeting_id": "m", "kind": "transcript", "text": "Budget."}
        bridge = {"passages": [passage], "queries": [{"id": "q", "ask_mode": "retrieved_library", "ask": [passage, passage]}]}
        self.assertEqual(run.ranking_cases(corpus, bridge, "heldout")[0]["baseline_ranked_ids"], ["p"])
        bridge["queries"][0]["ask_mode"] = "whole_meeting"
        self.assertEqual(run.ranking_cases(corpus, bridge, "heldout"), [])


if __name__ == "__main__":
    unittest.main()
