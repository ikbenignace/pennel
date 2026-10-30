import importlib.util
import json
import math
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("decision_metrics", ROOT / "metrics.py")
metrics = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(metrics)


def context(**updates):
    result = {
        "model": "synthetic/decision-v1",
        "transport": "mock",
        "upstream": "fixture",
        "precision": "fp32",
        "prompt_hash": "fixture-prompt",
        "labels": ["false", "true"],
        "grouping": "one-question",
    }
    result.update(updates)
    return result


def row(expected, prediction, probability=None, **updates):
    value = {"expected": expected, "prediction": prediction, "split": "calibration"}
    if probability is not None:
        value["probabilities"] = {"false": 1 - probability, "true": probability}
        value["score_type"] = "probabilities"
    value.update(updates)
    return value


class RankingTests(unittest.TestCase):
    def test_hand_computed_ndcg_and_candidate_recall(self):
        result = metrics.ranking_metrics(["b", "c", "a"], {"a": 3, "b": 1, "d": 2}, ["a", "b", "c"], k=2)
        expected = 1 / (7 + 3 / math.log2(3))
        self.assertAlmostEqual(result["ndcg_at_k"], expected)
        self.assertAlmostEqual(result["recall_at_k"], 1 / 3)
        self.assertAlmostEqual(result["candidate_recall"], 2 / 3)

    def test_perfect_ranking(self):
        result = metrics.ranking_metrics(["a", "b"], {"a": 3, "b": 2}, k=5)
        self.assertEqual(result["ndcg_at_k"], 1)
        self.assertEqual(result["recall_at_k"], 1)

    def test_unanswerable_is_not_perfect_or_zero_quality(self):
        result = metrics.ranking_metrics(["a"], {}, k=5)
        self.assertIsNone(result["ndcg_at_k"])
        self.assertIsNone(result["recall_at_k"])
        self.assertIsNone(result["candidate_recall"])
        self.assertFalse(result["answerable"])
        self.assertEqual(result["returned_count"], 1)

    def test_duplicate_passages_cannot_inflate_recall(self):
        with self.assertRaises(ValueError):
            metrics.ranking_metrics(["a", "a"], {"a": 1})

    def test_reranker_cannot_add_unretrieved_evidence(self):
        with self.assertRaises(ValueError):
            metrics.ranking_metrics(["a"], {"a": 1}, candidate_ids=["b"])

    def test_invalid_grades_and_k(self):
        for grades in ({"a": -1}, {"a": math.nan}, {"a": math.inf}):
            with self.assertRaises(ValueError):
                metrics.ranking_metrics(["a"], grades)
        for k in (0, -1, 1.5, True):
            with self.assertRaises(ValueError):
                metrics.ranking_metrics([], {}, k=k)


class ClassificationTests(unittest.TestCase):
    def test_abstention_counts_as_uncovered_and_missed(self):
        result = metrics.classification_metrics([
            row("true", "true", .9),
            row("false", "true", .8),
            row("false", None, .2),
        ])
        self.assertAlmostEqual(result["accuracy"], 1 / 3)
        self.assertAlmostEqual(result["coverage"], 2 / 3)
        self.assertEqual(result["selective_accuracy"], .5)
        self.assertEqual(result["per_class"]["false"]["recall"], 0)
        self.assertEqual(result["per_class"]["true"]["precision"], .5)

    def test_accepted_false_is_abstention_even_with_prediction(self):
        result = metrics.classification_metrics([row("true", "true", .9, accepted=False)])
        self.assertEqual(result["coverage"], 0)
        self.assertEqual(result["accuracy"], 0)
        self.assertIsNone(result["selective_accuracy"])

    def test_brier_and_ece_hand_calculated(self):
        result = metrics.classification_metrics([row("true", "true", .8), row("false", "true", .6)])
        self.assertAlmostEqual(result["brier"], .4)
        self.assertAlmostEqual(result["binary_brier"], .2)
        self.assertAlmostEqual(result["ece"], .4)
        self.assertEqual(result["probability_count"], 2)

    def test_logits_and_confidence_are_not_probabilities(self):
        for score_type in ("logits", "rerank_logits", "confidence", "relevance"):
            result = metrics.classification_metrics([row("true", "true", .8, score_type=score_type)])
            self.assertIsNone(result["brier"])
            self.assertIsNone(result["ece"])
            self.assertEqual(result["probability_count"], 0)

    def test_invalid_probability_distributions_fail(self):
        for value in ({"true": 1.1, "false": -.1}, {"true": .4, "false": .4}, {"true": math.nan, "false": 0}, {}):
            with self.assertRaises(ValueError):
                metrics.classification_metrics([row("true", "true", probabilities=value)])

    def test_missing_gold_label_fails(self):
        with self.assertRaises(ValueError):
            metrics.classification_metrics([row("missing", "true", .8)])

    def test_empty_results_are_unmeasured(self):
        result = metrics.classification_metrics([])
        self.assertEqual(result["count"], 0)
        for key in ("accuracy", "coverage", "selective_accuracy", "macro_f1", "brier", "ece"):
            self.assertIsNone(result[key])

    def test_no_probabilities_does_not_fabricate_scores(self):
        result = metrics.classification_metrics([row("true", "true")])
        self.assertEqual(result["accuracy"], 1)
        self.assertIsNone(result["brier"])
        self.assertIsNone(result["ece"])


class SuggestionTests(unittest.TestCase):
    def test_all_declines_have_zero_suggestion_coverage(self):
        rows = [row(label, label) for label in ("unknown", "abstain", "none")]
        rows.append(row("unknown", None))
        self.assertEqual(metrics.classification_metrics(rows)["accuracy"], .75)
        result = metrics.suggestion_metrics(rows)
        self.assertEqual(result["coverage"], 0)
        self.assertEqual(result["suggestion_count"], 0)
        self.assertIsNone(result["precision"])

    def test_false_ambiguous_names_are_counted(self):
        result = metrics.suggestion_metrics([
            row("unknown", "speaker_1"), row("speaker_1", "speaker_1"),
            row("unknown", "speaker_1", accepted=False),
        ])
        self.assertEqual(result["suggestion_count"], 2)
        self.assertEqual(result["precision"], .5)
        self.assertAlmostEqual(result["coverage"], 2 / 3)
        self.assertEqual(result["forced_ambiguous_names"], 1)

    def test_rejected_low_confidence_label_is_not_a_suggestion(self):
        result = metrics.suggestion_metrics([row("speaker_1", "speaker_1", accepted=False)])
        self.assertEqual(result["coverage"], 0)
        self.assertEqual(result["correct_count"], 0)
        self.assertIsNone(result["precision"])


class CalibrationTests(unittest.TestCase):
    def test_threshold_maximizes_coverage_at_precision(self):
        rows = [row("true", "true", .95), row("false", "false", .1), row("false", "true", .8)]
        selected = metrics.choose_threshold(rows, target_precision=1)
        self.assertEqual(selected["threshold"], .9)
        self.assertAlmostEqual(selected["coverage"], 2 / 3)
        self.assertEqual(selected["selective_precision"], 1)

    def test_no_safe_threshold_abstains(self):
        selected = metrics.choose_threshold([row("false", "true", .9)], target_precision=.98)
        self.assertIsNone(selected["threshold"])
        self.assertEqual(selected["coverage"], 0)

    def test_confident_declines_cannot_make_suggestion_threshold_pass(self):
        rows = [{"expected": "unknown", "prediction": "unknown", "split": "calibration",
                 "probabilities": {"unknown": .99, "name": .01}} for _ in range(50)]
        rows.append({"expected": "unknown", "prediction": "name", "split": "calibration",
                     "probabilities": {"unknown": .2, "name": .8}})
        self.assertEqual(metrics.choose_threshold(rows)["coverage"], 1)
        suggestion = metrics.choose_threshold(rows, suggestions_only=True)
        self.assertIsNone(suggestion["threshold"])
        self.assertEqual(suggestion["coverage"], 0)

    def test_fit_and_threshold_reject_development_or_heldout(self):
        for split in ("development", "heldout", "test", None):
            rows = [row("true", "true", .8, split=split)]
            with self.assertRaises(ValueError):
                metrics.fit_calibration(rows, context())
            with self.assertRaises(ValueError):
                metrics.choose_threshold(rows)

    def test_temperature_does_not_increase_regularized_loss(self):
        rows = [row("true", "true", .99), row("false", "true", .95), row("false", "false", .05), row("true", "false", .1)]
        fit = metrics.fit_calibration(rows, context(), kind="temperature")
        self.assertLessEqual(fit["objective_after"], fit["objective_before"] + 1e-12)
        self.assertEqual(fit["calibration_split"], "calibration")
        output = metrics.apply_calibration({"false": .2, "true": .8}, fit, context())
        self.assertAlmostEqual(sum(output.values()), 1)

    def test_binary_fit_and_apply(self):
        rows = [row("false", "true", .8), row("true", "true", .9), row("false", "false", .3), row("true", "true", .6)]
        fit = metrics.fit_calibration(rows, context(), kind="binary")
        self.assertLessEqual(fit["objective_after"], fit["objective_before"] + 1e-12)
        output = metrics.apply_calibration({"false": .4, "true": .6}, fit, context())
        self.assertTrue(0 < output["true"] < 1)
        self.assertAlmostEqual(sum(output.values()), 1)

    def test_fit_rejects_logits(self):
        with self.assertRaises(ValueError):
            metrics.fit_calibration([row("true", "true", .9, score_type="rerank_logits")], context())

    def test_configuration_drift_invalidates_calibration(self):
        rows = [row("true", "true", .9), row("false", "false", .1)]
        fit = metrics.fit_calibration(rows, context())
        for changes in ({"model": "other"}, {"upstream": "other"}, {"transport": "direct"}, {"precision": "q8"}, {"prompt_hash": "other"}, {"labels": ["true", "false"]}, {"grouping": "batch"}):
            with self.assertRaises(ValueError):
                metrics.apply_calibration({"false": .2, "true": .8}, fit, context(**changes))

    def test_missing_identity_cannot_be_calibrated(self):
        incomplete = context()
        del incomplete["upstream"]
        with self.assertRaises(ValueError):
            metrics.fit_calibration([row("true", "true", .8)], incomplete)

    def test_label_set_drift_cannot_be_applied(self):
        fit = metrics.fit_calibration([row("true", "true", .9), row("false", "false", .1)], context())
        with self.assertRaises(ValueError):
            metrics.apply_calibration({"no": .2, "yes": .8}, fit, context())

    def test_single_observed_class_is_flagged(self):
        fit = metrics.fit_calibration([row("true", "true", .9)], context())
        self.assertEqual(fit["status"], "identity_insufficient_classes")
        self.assertTrue(fit["warnings"])


class BootstrapAndTimingTests(unittest.TestCase):
    def test_bootstrap_resamples_meetings_not_rows(self):
        rows = [{"group_id": "a", "baseline": 0, "candidate": 1}] * 10
        rows += [{"group_id": "b", "baseline": 1, "candidate": 0}]
        result = metrics.paired_bootstrap(rows, iterations=500)
        self.assertEqual(result["mean_difference"], 0)
        self.assertEqual(result["group_count"], 2)
        self.assertEqual(result, metrics.paired_bootstrap(rows, iterations=500))
        self.assertEqual(result["lower"], -1)
        self.assertEqual(result["upper"], 1)

    def test_bootstrap_empty_is_unmeasured(self):
        result = metrics.paired_bootstrap([])
        self.assertIsNone(result["lower"])
        self.assertIsNone(result["mean_difference"])

    def test_bootstrap_rejects_mixed_languages(self):
        with self.assertRaises(ValueError):
            metrics.paired_bootstrap([
                {"group_id": "a", "language": "en", "baseline": 0, "candidate": 1},
                {"group_id": "b", "language": "nl", "baseline": 0, "candidate": 1},
            ])

    def test_percentiles_are_linear_and_include_sample_count(self):
        result = metrics.timing_metrics([1, 2, 3, 4])
        self.assertEqual(result["n"], 4)
        self.assertEqual(result["p50_ms"], 2.5)
        self.assertAlmostEqual(result["p95_ms"], 3.85)
        self.assertIsNone(metrics.timing_metrics([])["p95_ms"])

    def test_invalid_timings_fail(self):
        for value in (-1, math.nan, math.inf):
            with self.assertRaises(ValueError):
                metrics.timing_metrics([value])


class ReproducibilityTests(unittest.TestCase):
    def test_json_digest_and_rendering_are_stable(self):
        first = {"title": "Synthetic EN/NL", "results": {"nl": {"f1": .5}, "en": {"f1": 1}}, "generated_at": "first"}
        second = {"generated_at": "second", "results": {"en": {"f1": 1}, "nl": {"f1": .5}}, "title": "Synthetic EN/NL"}
        self.assertEqual(metrics.semantic_digest(first), metrics.semantic_digest(second))
        self.assertEqual(metrics.render_report(first), metrics.render_report(second))
        self.assertEqual(json.loads(metrics.canonical_json(first)), first)

    def test_protocol_captures_frozen_acceptance_and_limits(self):
        protocol = json.loads((ROOT / "protocol.json").read_text())
        self.assertEqual(protocol["languages"], ["en", "nl", "mixed"])
        self.assertEqual(protocol["budgets"]["hosted_usd"], "2.00")
        self.assertEqual(protocol["budgets"]["model_download_bytes"], 3 * 1024 ** 3)
        self.assertEqual(protocol["budgets"]["owned_memory_bytes"], 24 * 1024 ** 3)
        self.assertEqual(protocol["calibration"]["fit_split"], "calibration")
        self.assertEqual(protocol["latency_sampling"]["cold_samples"], 5)
        self.assertEqual(protocol["latency_sampling"]["warm_samples"], 30)


if __name__ == "__main__":
    unittest.main()
