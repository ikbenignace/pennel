import copy
import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import calibration
import corpus
import supplement


class SupplementTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = supplement.build_supplement()

    def test_independent_bounded_corpus_and_deterministic_serialization(self):
        self.assertEqual(supplement.validate_supplement(self.data), [])
        self.assertEqual(len(self.data["meetings"]), 108)
        self.assertEqual(len(self.data["decisions"]), 648)
        self.assertEqual(len({meeting["group_id"] for meeting in self.data["meetings"]}), 36)
        self.assertEqual(corpus.corpus_hash(self.data), corpus.corpus_hash(supplement.build_supplement()))
        self.assertTrue(all(len(corpus.canonical_json(case["state"]).encode("utf-8")) <= 2000 for case in self.data["decisions"]))

    def test_translations_and_label_perturbations_cannot_cross_splits(self):
        groups = {}
        for meeting in self.data["meetings"]:
            groups.setdefault(meeting["group_id"], []).append(meeting)
        for meetings in groups.values():
            self.assertEqual({meeting["language"] for meeting in meetings}, {"en", "nl", "mixed"})
            self.assertEqual(len({meeting["split"] for meeting in meetings}), 1)
            self.assertTrue(all(meeting["provenance"]["translation_group"] == meeting["group_id"] for meeting in meetings))
        cases = {case["id"]: case for case in self.data["decisions"]}
        for variant in cases.values():
            if "variant_of" not in variant:
                continue
            original = cases[variant["variant_of"]]
            self.assertEqual(variant["expected"], original["expected"])
            self.assertEqual(variant["group_id"], original["group_id"])
            self.assertEqual(variant["split"], original["split"])
            self.assertEqual(variant["label_order"], list(reversed(original["label_order"])))

    def test_every_calibration_context_has_sufficient_and_diverse_targets(self):
        report = supplement.feasibility_report(self.data)
        self.assertEqual(len(report["calibration_contexts"]), 18)
        for context in report["calibration_contexts"]:
            self.assertEqual(context["samples"], 12)
            self.assertEqual(context["meeting_groups"], 12)
            self.assertGreaterEqual(context["classes"], 3)

    def test_inference_state_and_target_evidence_are_not_reused_across_splits(self):
        state_splits = {}
        evidence_splits = {}
        for case in self.data["decisions"]:
            state_splits.setdefault(case["state"], set()).add(case["split"])
            # Ignore the topic-containing purpose line: changed event facts and
            # wording must separate the bodies as well as their headers.
            body = "\n".join(case["state"].splitlines()[1:])
            state_splits.setdefault(body, set()).add(case["split"])
            target_text = case["evidence"][1 if case["task"] == "events" else 0]["text"]
            evidence_splits.setdefault((case["language"], case["task"], target_text), set()).add(case["split"])
        self.assertTrue(all(len(splits) == 1 for splits in state_splits.values()))
        self.assertTrue(all(len(splits) == 1 for splits in evidence_splits.values()))
        for meeting in self.data["meetings"]:
            topic = supplement.TOPICS[meeting["split"]][1 if meeting["language"] == "nl" else 0]
            self.assertIn(topic, meeting["transcript"][0]["text"])
            expected_deadline = {
                "development": ("Monday", "maandag"), "calibration": ("Wednesday", "woensdag"), "heldout": ("Friday", "vrijdag"),
            }[meeting["split"]][meeting["language"] == "nl"]
            self.assertIn(expected_deadline, meeting["transcript"][4]["text"])

    def test_validator_rejects_verbatim_state_reuse_even_when_group_ids_differ(self):
        damaged = copy.deepcopy(self.data)
        by_split = {split: next(case for case in damaged["decisions"] if case["split"] == split) for split in corpus.SPLITS}
        by_split["heldout"]["state"] = by_split["development"]["state"]
        self.assertIn("supplement inference state crosses splits", supplement.validate_supplement(damaged))

    def test_template_and_name_suggestion_coverage_are_feasible_in_every_language_and_split(self):
        for row in supplement.feasibility_report(self.data)["rows"]:
            if row["task"] == "templates":
                self.assertEqual(row["cases"], 24)
                self.assertAlmostEqual(row["maximum_correct_suggestion_coverage"], 2 / 3)
                self.assertEqual(row["expected_counts"]["abstain"], 8)
            elif row["task"] == "speaker_names":
                self.assertEqual(row["cases"], 24)
                self.assertEqual(row["maximum_correct_suggestion_coverage"], .75)
                self.assertEqual(row["expected_counts"]["unknown"], 6)
            else:
                self.assertEqual(row["expected_counts"], {"speaker_1": 8, "speaker_2": 8, "unknown": 8})

    def test_original_calibration_pipeline_can_fit_every_context_without_heldout_labels(self):
        identity = {"transport": "offline_test_fixture", "returned_model": "test_fixture_v1", "provider": "mock", "precision": "fixture"}
        rows = []
        for case in self.data["decisions"]:
            labels = case["label_order"]
            rows.append({**case, "runner_id": "mock_fixture", "identity": identity,
                         "prediction": case["expected"], "raw_score_type": "probabilities",
                         "probabilities": {label: .9 if label == case["expected"] else .1 / (len(labels) - 1) for label in labels}})
        artifact = calibration.fit_predictions(rows, run_manifest_hash="mock_supplement_manifest")
        self.assertEqual(len(artifact["fits"]), 18)
        self.assertTrue(all(fit["status"] == "fitted" for fit in artifact["fits"].values()))
        self.assertEqual(artifact["ignored_splits"], {"development": 216, "heldout": 216})
        mutated = copy.deepcopy(rows)
        for row in mutated:
            if row["split"] == "heldout":
                row["expected"] = "not_a_real_label"
        self.assertEqual(artifact, calibration.fit_predictions(mutated, run_manifest_hash="mock_supplement_manifest"))

    def test_unicode_source_evidence_and_required_adversarial_slices(self):
        self.assertEqual(corpus.validate_corpus(self.data), [])
        tags = {tag for case in self.data["decisions"] for tag in case["tags"]}
        for tag in ("negation", "retraction", "tentative", "ambiguous_speaker", "addressed_speaker", "paraphrase", "missing_context", "quoted_instruction", "prompt_injection", "reordered_labels", "code_switching"):
            self.assertIn(tag, tags)
        self.assertTrue(any("Noël" in span["text"] for case in self.data["decisions"] for span in case["evidence"]))
        damaged = copy.deepcopy(self.data)
        damaged["decisions"][0]["evidence"][0]["start"] += 1
        self.assertTrue(supplement.validate_supplement(damaged))

    def test_protocol_retains_frozen_minima_and_thresholds(self):
        original = json.loads((ROOT / "protocol.json").read_text())
        extra = json.loads((ROOT / "protocol-supplement-v2.json").read_text())
        for task in ("templates", "speaker_names"):
            self.assertEqual(extra["acceptance"][task], original["acceptance"][task])
        for field in ("minimum_observations", "minimum_observed_classes", "minimum_meeting_groups"):
            self.assertEqual(extra["calibration"][field], original["calibration"][field])
        self.assertEqual(extra["budgets"]["hosted_usd_total_across_all_experiments"], original["budgets"]["hosted_usd"])

    def test_supplement_does_not_mutate_or_reuse_original_experiment(self):
        original = corpus.build_corpus()
        before = corpus.corpus_hash(original)
        supplement.build_supplement()
        self.assertEqual(corpus.corpus_hash(original), before)
        self.assertFalse({meeting["id"] for meeting in original["meetings"]} & {meeting["id"] for meeting in self.data["meetings"]})


if __name__ == "__main__":
    unittest.main()
