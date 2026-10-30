import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location("calibration_pipeline", ROOT / "calibration.py")
calibration = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(calibration)


def prediction(index, split="calibration", **updates):
    positive = index % 2 == 0
    label = "true" if positive else "false"
    result = {
        "id": f"{split}-{index}", "runner_id": "router-jev", "task": "answerability",
        "language": "nl", "split": split, "group_id": f"{split}-meeting-{index // 2}",
        "expected": label, "prediction": label,
        "probabilities": {"false": .1 if positive else .9, "true": .9 if positive else .1},
        "raw_score_type": "binary_probability", "label_order": ["false", "true"],
        "question": {"type": "noul", "instructions": "Is the answer supported?"},
        "identity": {
            "transport": "openrouter", "requested_model": "typesafe/jev-1.14",
            "returned_model": "typesafe/jev-1.14-20261014", "provider": "TypeSafe",
            "routing": {"only": ["typesafe"], "allow_fallbacks": False},
            "privacy": {"region": "global", "retention": "documented"},
        },
    }
    result.update(updates)
    return result


class CalibrationPipelineTests(unittest.TestCase):
    def test_only_calibration_rows_are_fitted(self):
        rows = [prediction(index) for index in range(10)]
        rows += [prediction(90, "development"), prediction(91, "heldout", expected="should-not-be-used")]
        artifact = calibration.fit_predictions(rows, run_manifest_hash="frozen-run")
        self.assertEqual(artifact["calibration_rows"], 10)
        self.assertEqual(artifact["ignored_splits"], {"development": 1, "heldout": 1})
        self.assertEqual(len(artifact["fits"]), 1)
        fit = next(iter(artifact["fits"].values()))
        self.assertEqual(fit["status"], "fitted")
        self.assertEqual(fit["fit"]["sample_count"], 10)
        self.assertEqual(fit["fit"]["kind"], "binary")
        self.assertEqual(artifact["run_manifest_hash"], "frozen-run")

    def test_heldout_apply_and_missing_predictions_keep_denominator(self):
        artifact = calibration.fit_predictions([prediction(index) for index in range(10)], run_manifest_hash="frozen-run")
        rows = [prediction(index, "heldout") for index in range(4)]
        rows.append(prediction(20, "heldout", prediction=None, probabilities=None, status="unavailable"))
        report = calibration.score_predictions(rows, artifact, run_manifest_hash="frozen-run")
        result = report["groups"][0]
        self.assertEqual(result["raw"]["count"], 5)
        self.assertEqual(result["calibrated"]["count"], 5)
        self.assertEqual(result["calibrated"]["coverage"], .8)
        self.assertEqual(result["calibrated"]["selective_accuracy"], 1)
        self.assertEqual(result["calibrated"]["probability_count"], 4)
        self.assertEqual(result["uncalibrated_count"], 1)

    def test_insufficient_samples_or_classes_do_not_fit(self):
        for rows in ([prediction(index) for index in range(9)], [prediction(index * 2) for index in range(10)]):
            artifact = calibration.fit_predictions(rows)
            result = next(iter(artifact["fits"].values()))
            self.assertNotEqual(result["status"], "fitted")
            self.assertIsNone(result["fit"])

    def test_one_meeting_cannot_fake_independent_sample_size(self):
        artifact = calibration.fit_predictions([prediction(index, group_id="same-meeting") for index in range(10)])
        self.assertEqual(next(iter(artifact["fits"].values()))["status"], "insufficient_meeting_groups")

    def test_each_language_and_subtask_has_its_own_fit(self):
        rows = [prediction(index) for index in range(10)]
        rows += [prediction(index, language="en", id=f"en-{index}") for index in range(10)]
        rows += [prediction(index, subtask="owner", id=f"owner-{index}") for index in range(10)]
        artifact = calibration.fit_predictions(rows)
        self.assertEqual(len(artifact["fits"]), 3)

    def test_config_identity_order_privacy_drift_abstains(self):
        artifact = calibration.fit_predictions([prediction(index) for index in range(10)], run_manifest_hash="frozen-run")
        for updates in (
            {"label_order": ["true", "false"]},
            {"question": {"type": "noul", "instructions": "Changed prompt"}},
            {"grouping": "many-questions"},
            {"identity": {**prediction(0)["identity"], "provider": "Other"}},
            {"identity": {**prediction(0)["identity"], "privacy": {"region": "eu"}}},
        ):
            report = calibration.score_predictions([prediction(0, "heldout", **updates)], artifact, run_manifest_hash="frozen-run")
            group = report["groups"][0]
            self.assertEqual(group["calibrated"]["coverage"], 0)
            self.assertEqual(group["uncalibrated_count"], 1)
            self.assertTrue(group["gates"])

    def test_volatile_request_metadata_does_not_split_context(self):
        rows = []
        for index in range(10):
            identity = {**prediction(index)["identity"], "request_id": f"request-{index}", "timestamp": str(index), "usage": {"input_tokens": index}}
            rows.append(prediction(index, identity=identity, latency_ms=index))
        artifact = calibration.fit_predictions(rows)
        self.assertEqual(len(artifact["fits"]), 1)
        self.assertEqual(next(iter(artifact["fits"].values()))["status"], "fitted")

    def test_changing_speaker_label_spaces_stays_uncalibrated(self):
        rows = []
        for index in range(10):
            name = f"Person-{index}"
            rows.append(prediction(index, task="speaker_names", expected=name, prediction=name,
                label_order=["none", name], probabilities={"none": .1, name: .9},
                raw_score_type="categorical_probabilities",
                question={"type": "choice", "instructions": "Name the speaker", "criteria": {"none": "Unknown", name: name}}))
        artifact = calibration.fit_predictions(rows)
        self.assertEqual(len(artifact["fits"]), 10)
        self.assertTrue(all(value["fit"] is None for value in artifact["fits"].values()))

    def test_relevance_logits_are_not_calibrated(self):
        artifact = calibration.fit_predictions([prediction(index, raw_score_type="rerank_logits") for index in range(10)])
        self.assertFalse(artifact["fits"])
        self.assertEqual(artifact["uncalibratable"][0]["reason"], "unsupported_score_type")

    def test_categorical_and_ordinal_distributions_use_temperature(self):
        for score_type, question_type in (("categorical_probabilities", "choice"), ("ordinal_probabilities", "score")):
            rows = [prediction(index, raw_score_type=score_type,
                label_order=["0", "1", "2"], expected=str(index % 3), prediction=str(index % 3),
                probabilities={str(value): .8 if value == index % 3 else .1 for value in range(3)},
                question={"type": question_type, "instructions": "Select level", "criteria": {"0": "low", "1": "mid", "2": "high"}}) for index in range(12)]
            artifact = calibration.fit_predictions(rows)
            self.assertEqual(next(iter(artifact["fits"].values()))["fit"]["kind"], "temperature")

    def test_manifest_drift_is_an_error(self):
        artifact = calibration.fit_predictions([prediction(index) for index in range(10)], run_manifest_hash="frozen-run")
        with self.assertRaises(ValueError):
            calibration.score_predictions([prediction(0, "heldout")], artifact, run_manifest_hash="different-run")

    def test_missing_manifest_link_is_explicit_gate(self):
        artifact = calibration.fit_predictions([prediction(index) for index in range(10)])
        report = calibration.score_predictions([prediction(0, "heldout")], artifact)
        self.assertFalse(report["manifest_verified"])
        self.assertIn("run_manifest_unlinked", report["gates"])

    def test_repeated_latency_cases_do_not_inflate_fit(self):
        rows = [prediction(index) for index in range(10)] + [prediction(0)]
        with self.assertRaises(ValueError):
            calibration.fit_predictions(rows)

    def test_missing_calibration_or_heldout_results_are_explicit_gates(self):
        artifact = calibration.fit_predictions([], run_manifest_hash="frozen-run")
        self.assertIn("no_fitted_calibration_contexts", artifact["gates"])
        report = calibration.score_predictions([], artifact, run_manifest_hash="frozen-run")
        self.assertIn("no_heldout_predictions", report["gates"])

    def test_minimum_sample_requirement_cannot_be_weakened(self):
        with self.assertRaises(ValueError):
            calibration.fit_predictions([prediction(index) for index in range(10)], min_samples=2)

    def test_declines_do_not_satisfy_template_coverage(self):
        def template(index, split="calibration"):
            return prediction(index, split, task="templates", expected="algemeen" if index % 2 else "abstain",
                prediction="abstain", probabilities={"algemeen": .01, "abstain": .99},
                raw_score_type="categorical_probabilities", label_order=["algemeen", "abstain"],
                question={"type": "choice", "instructions": "Suggest template", "criteria": {"algemeen": "General", "abstain": "Unknown"}})
        artifact = calibration.fit_predictions([template(index) for index in range(10)], run_manifest_hash="frozen-run")
        fit = next(iter(artifact["fits"].values()))["fit"]
        self.assertEqual(fit["threshold_scope"], "positive_suggestions")
        self.assertIsNone(fit["threshold"]["threshold"])
        result = calibration.score_predictions([template(1, "heldout"), template(2, "heldout")], artifact, run_manifest_hash="frozen-run")["groups"][0]
        self.assertEqual(result["raw"]["accuracy"], .5)
        self.assertEqual(result["raw_suggestions"]["coverage"], 0)
        self.assertEqual(result["calibrated_suggestions"]["coverage"], 0)
        self.assertIn("suggestion_coverage_below_minimum", result["gates"])
        self.assertIn("suggestion_precision_below_minimum", result["gates"])

    def test_forced_ambiguous_and_low_confidence_names(self):
        def name(index, split="calibration", **updates):
            positive = index % 2 == 0
            values = dict(task="speaker_names", expected="speaker_1" if positive else "unknown",
                prediction="speaker_1" if positive else "unknown",
                probabilities={"speaker_1": .9 if positive else .1, "unknown": .1 if positive else .9},
                raw_score_type="categorical_probabilities", label_order=["speaker_1", "unknown"],
                question={"type": "choice", "instructions": "Suggest name", "criteria": {"speaker_1": "Jan", "unknown": "Unknown"}})
            values.update(updates)
            return prediction(index, split, **values)
        artifact = calibration.fit_predictions([name(index) for index in range(10)], run_manifest_hash="frozen-run")
        rows = [name(0, "heldout", expected="unknown"), name(1, "heldout", expected="speaker_1", prediction="speaker_1", probabilities={"speaker_1": .51, "unknown": .49})]
        result = calibration.score_predictions(rows, artifact, run_manifest_hash="frozen-run")["groups"][0]
        self.assertEqual(result["calibrated_suggestions"]["suggestion_count"], 1)
        self.assertEqual(result["calibrated_suggestions"]["forced_ambiguous_names"], 1)
        self.assertEqual(result["calibrated_suggestions"]["precision"], 0)
        self.assertIn("forced_ambiguous_names", result["gates"])


if __name__ == "__main__":
    unittest.main()
