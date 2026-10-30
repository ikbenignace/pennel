import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

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
                    "rubric_sha256": RUBRIC_SHA256, "config_sha256": run.digest(config), "workloads": {"heldout": []},
                    "protocol_sha256": run.digest(run.read_json(run.DIRECTORY / "protocol-generative.json"))}
        generate.validate_inputs(manifest, inputs, config)
        for field in ("inputs_sha256", "corpus_hash", "rubric_sha256", "config_sha256", "protocol_sha256"):
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

    def test_live_binary_hardware_and_shared_budget_cannot_silently_change(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "synthetic-binary"
            binary.write_bytes(b"fixture")
            config = {"ledger_path": "build/decision-model-evaluation/spend-ledger.json"}
            manifest = {"binary_sha256": generate.hashlib.sha256(binary.read_bytes()).hexdigest(),
                        "hardware": {"system": "fixture"}, "source_fingerprint": "fixture-source"}
            with patch.object(run, "hardware", return_value=manifest["hardware"]), patch.object(run, "source_fingerprint", return_value="fixture-source"):
                generate.validate_live(manifest, config, binary)
                with self.assertRaisesRegex(ValueError, "shared"):
                    generate.validate_live(manifest, {"ledger_path": "build/other-ledger.json"}, binary)
                with self.assertRaisesRegex(ValueError, "hardware"):
                    generate.validate_live({**manifest, "hardware": {"system": "other"}}, config, binary)
                binary.write_bytes(b"different-core-behavior")
                with self.assertRaisesRegex(ValueError, "binary"):
                    generate.validate_live(manifest, config, binary)

    def test_prediction_gold_metadata_must_match_frozen_inputs(self):
        manifest = {"workloads": {"heldout": [["meeting_qa", "q"], ["cleanup", "c"]]}}
        inputs = {"corpus": {"queries": [{"id": "q", "expected_answer": "50", "evidence": ["synthetic-span"]}],
                             "cleanup_pairs": [{"id": "c", "raw": "do not send fifty"}]}}
        rows = [{"id": "q", "task": "meeting_qa", "split": "heldout", "status": "ok", "run_manifest_hash": run.digest(manifest),
                 "adjudication": {"expected_answer": "50", "gold_evidence": ["synthetic-span"]}},
                {"id": "c", "task": "cleanup", "split": "heldout", "status": "ok", "run_manifest_hash": run.digest(manifest),
                 "adjudication": {"raw": "do not send fifty"}}]
        generate.validate_rows(rows, manifest, inputs)
        for index, field in ((0, "expected_answer"), (0, "gold_evidence"), (1, "raw")):
            changed = copy.deepcopy(rows)
            changed[index]["adjudication"][field] = "changed"
            with self.assertRaisesRegex(ValueError, "frozen inputs"):
                generate.validate_rows(changed, manifest, inputs)


if __name__ == "__main__":
    unittest.main()
