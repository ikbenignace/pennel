import copy
import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("decision_corpus", ROOT / "corpus.py")
corpus = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(corpus)


class CorpusTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = corpus.build_corpus()

    def test_ninety_independent_meetings_and_balanced_splits(self):
        meetings = self.data["meetings"]
        self.assertEqual(len(meetings), 90)
        self.assertEqual(len({m["group_id"] for m in meetings}), 90)
        for language in ("en", "nl", "mixed"):
            for split in ("development", "calibration", "heldout"):
                selected = [m for m in meetings if m["language"] == language and m["split"] == split]
                self.assertEqual(len(selected), 10)

    def test_all_annotations_validate(self):
        self.assertEqual(corpus.validate_corpus(self.data), [])

    def test_deterministic_bytes_and_hash(self):
        repeated = corpus.build_corpus()
        self.assertEqual(corpus.canonical_json(self.data), corpus.canonical_json(repeated))
        self.assertEqual(corpus.corpus_hash(self.data), corpus.corpus_hash(repeated))

    def test_every_task_language_and_split_has_cases(self):
        for task in ("answerability", "claim_support", "events", "templates", "speaker_names", "cleanup"):
            for language in ("en", "nl", "mixed"):
                for split in ("development", "calibration", "heldout"):
                    selected = [d for d in self.data["decisions"] if (d["task"], d["language"], d["split"]) == (task, language, split)]
                    self.assertGreaterEqual(len(selected), 10)

    def test_reordered_labels_do_not_change_the_expected_answer(self):
        cases = {case["id"]: case for case in self.data["decisions"]}
        for task in ("answerability", "claim_support", "events", "templates", "speaker_names", "cleanup"):
            for language in ("en", "nl", "mixed"):
                reordered = [d for d in cases.values() if d["task"] == task and d["language"] == language and "reordered_labels" in d["tags"]]
                self.assertGreaterEqual(len(reordered), 2)
                for variant in reordered:
                    original = cases[variant["variant_of"]]
                    self.assertEqual(variant["expected"], original["expected"])
                    self.assertEqual(variant["group_id"], original["group_id"])
                    self.assertEqual(variant["split"], original["split"])
                    self.assertEqual(list(variant["question"]["criteria"]), list(reversed(original["question"]["criteria"])))

    def test_requested_adversarial_slices_exist(self):
        tags = {tag for case in self.data["decisions"] for tag in case["tags"]}
        tags.update(tag for case in self.data["queries"] for tag in case["tags"])
        for tag in ("negation", "retraction", "tentative", "ambiguous_speaker", "addressed_speaker", "quoted_instruction", "prompt_injection", "reordered_labels", "paraphrase", "missing_answer", "missing_context", "long_input", "code_switching"):
            self.assertIn(tag, tags)

    def test_hash_and_serialization_preserve_label_order(self):
        original = self.data["decisions"][0]
        reordered = copy.deepcopy(original)
        reordered["question"]["criteria"] = dict(reversed(list(original["question"]["criteria"].items())))
        reordered["label_order"] = list(reordered["question"]["criteria"])
        self.assertNotEqual(corpus.corpus_hash(original), corpus.corpus_hash(reordered))
        self.assertEqual(list(__import__("json").loads(corpus.canonical_json(reordered))["question"]["criteria"]), reordered["label_order"])

    def test_long_inputs_cross_the_real_core_boundaries(self):
        for language in ("en", "nl", "mixed"):
            for split in ("development", "calibration", "heldout"):
                candidates = [m for m in self.data["meetings"] if m["language"] == language and m["split"] == split]
                self.assertTrue(any(sum(len(s["text"].encode()) for s in m["transcript"]) > 60_000 and sum(len(s["text"]) for s in m["transcript"]) > 40_000 for m in candidates))

    def test_binary_case_answers_are_allowed_and_template_ids_are_real(self):
        templates = set()
        for case in self.data["decisions"]:
            self.assertIn(case["expected"], case["question"]["criteria"])
            if case["task"] == "templates":
                templates.update(case["question"]["criteria"])
        self.assertEqual(templates, {"algemeen", "een-op-een", "klantgesprek", "standup", "abstain"})

    def test_unicode_evidence_is_a_code_point_slice(self):
        names = [d for d in self.data["decisions"] if d["task"] == "speaker_names" and d["expected"] != "unknown"]
        self.assertTrue(any("ë" in e["text"] or "é" in e["text"] for d in names for e in d["evidence"]))

    def test_validator_rejects_span_damage_and_split_leakage(self):
        damaged = copy.deepcopy(self.data)
        damaged["queries"][0]["evidence"][0]["start"] += 1
        self.assertTrue(corpus.validate_corpus(damaged))
        damaged = copy.deepcopy(self.data)
        damaged["meetings"][10]["group_id"] = damaged["meetings"][0]["group_id"]
        self.assertTrue(any("group" in issue for issue in corpus.validate_corpus(damaged)))

    def test_no_private_provenance_or_cross_split_library_dependencies(self):
        meetings = {m["id"]: m for m in self.data["meetings"]}
        for meeting in meetings.values():
            self.assertTrue(meeting["provenance"]["synthetic"])
            self.assertFalse(meeting["provenance"]["private_recordings"])
        for query in self.data["queries"]:
            for mid in query["candidate_meeting_ids"]:
                self.assertEqual(query["split"], meetings[mid]["split"])

    def test_cleanup_contains_safe_and_critical_mutations(self):
        for language in ("en", "nl", "mixed"):
            pairs = [p for p in self.data["cleanup_pairs"] if p["language"] == language]
            self.assertTrue(any(p["expected_safe"] for p in pairs))
            for tag in ("amount", "negation", "recipient", "date", "omission"):
                self.assertTrue(any(tag in p["tags"] and p["critical"] and not p["expected_safe"] for p in pairs))


if __name__ == "__main__":
    unittest.main()
