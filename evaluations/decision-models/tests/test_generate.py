import copy
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import generate
import run
from corpus import build_corpus, corpus_hash
from grounding import RUBRIC_SHA256


class GenerateTests(unittest.TestCase):
    def test_frozen_selection_is_language_balanced_and_keeps_missing_answers(self):
        corpus = build_corpus()
        bridge = {"queries": corpus["queries"], "cleanup": corpus["cleanup_pairs"],
                  "summary_prompts": [{"meeting_id": m["id"]} for m in corpus["meetings"]]}
        development = generate.selected_bridge(corpus, bridge, "development")
        heldout = generate.selected_bridge(corpus, bridge, "heldout")
        self.assertEqual(len(generate.workload_ids(development)), 9)
        self.assertEqual(len(generate.workload_ids(heldout)), 54)
        for language in ("en", "nl", "mixed"):
            queries = [q for q in heldout["queries"] if q["language"] == language]
            self.assertEqual(len(queries), 14)
            self.assertTrue(any(q["expected_answer"] is None for q in queries))
        dev = set(generate.workload_ids(development))
        self.assertFalse(dev & set(generate.workload_ids(heldout)))

    def test_frozen_input_config_rubric_and_workload_drift_rejected(self):
        inputs = {"corpus": {"meetings": []}, "bridges": {"heldout": {"queries": [], "cleanup": [], "summary_prompts": []}}}
        config = {"explicit": "synthetic"}
        manifest = {"inputs_sha256": run.digest(inputs), "corpus_hash": corpus_hash(inputs["corpus"]),
                    "rubric_sha256": RUBRIC_SHA256, "config_sha256": run.digest(config), "workloads": {"heldout": []}}
        generate.validate_inputs(manifest, inputs, config)
        for field in ("inputs_sha256", "corpus_hash", "rubric_sha256", "config_sha256"):
            changed = copy.deepcopy(manifest)
            changed[field] = "changed"
            with self.assertRaises(ValueError):
                generate.validate_inputs(changed, inputs, config)
        changed = copy.deepcopy(manifest)
        changed["workloads"]["heldout"] = [["meeting_qa", "extra"]]
        with self.assertRaisesRegex(ValueError, "workload"):
            generate.validate_inputs(changed, inputs)

    def test_archive_is_reproducible_and_preserves_unicode(self):
        with tempfile.TemporaryDirectory() as directory:
            first, second = (Path(directory) / name for name in ("first.gz", "second.gz"))
            data = {"source": "Noël: geen €50 overmaken"}
            for path in (first, second):
                generate.archive(path, data)
                self.assertEqual(generate.read_archive(path), data)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            data = build_corpus()
            generate.archive(first, data)
            self.assertEqual(corpus_hash(data), corpus_hash(generate.read_archive(first)))


if __name__ == "__main__":
    unittest.main()
