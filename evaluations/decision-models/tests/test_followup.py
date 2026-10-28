import copy
import gzip
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


DIRECTORY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(DIRECTORY))
import followup
import supplement


def configuration():
    return {"experiment": "mock-supplement", "ledger_path": "build/decision-model-evaluation/spend-ledger.json",
            "runners": [{"id": "mock-jev", "enabled": True, "transport": "openrouter",
                         "model": "typesafe/jev-1.14", "expected_returned_model": "typesafe/jev-1.14-20261014",
                         "upstream_tag": "typesafe", "question_grouping": "one_case_per_request",
                         "privacy": {"processing_region": "global", "policy_verified": False, "data_collection": "deny", "zdr": True}}]}


def predictions(corpus, manifest, split):
    runner = manifest["config"]["runners"][0]
    rows = []
    for case in corpus["decisions"]:
        if case["split"] != split:
            continue
        labels = case["label_order"]
        rows.append({**case, "case_id": case["id"], "runner_id": runner["id"], "status": "ok",
                     "run_manifest_hash": followup.run.digest(manifest), "prediction": case["expected"],
                     "probabilities": {label: .9 if label == case["expected"] else .1 / (len(labels) - 1) for label in labels},
                     "score_type": "categorical_probabilities", "phase": "warm", "latency_ms": 1,
                     "question_grouping": "one_case_per_request",
                     "identity": {"transport": "openrouter", "requested_model": runner["model"],
                                  "returned_model": runner["expected_returned_model"], "provider": "TypeSafe",
                                  "routing": {"only": ["typesafe"], "allow_fallbacks": False, "data_collection": "deny", "zdr": True},
                                  "privacy": runner["privacy"], "question_grouping": "one_case_per_request"}})
    return rows


class FollowupTests(unittest.TestCase):
    def setUp(self):
        self.corpus = supplement.build_supplement()
        self.patches = [patch.object(followup.run, "source_fingerprint", return_value="mock-source"),
                        patch.object(followup.run, "hardware", return_value={"system": "fixture"}),
                        patch.object(followup.run, "git", return_value="mock-commit")]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)
        self.manifest = followup.prepare_manifest(configuration(), self.corpus)

    def test_manifest_binds_corpus_both_protocols_config_and_source(self):
        self.assertEqual(self.manifest["experiment_version"], supplement.VERSION)
        self.assertEqual(self.manifest["corpus_hash"], supplement.corpus_hash(self.corpus))
        self.assertEqual(self.manifest["source_fingerprint"], "mock-source")
        self.assertIn("supplement_protocol_hash", self.manifest)
        self.assertIn("protocol_hash", self.manifest)
        self.assertEqual(len(self.manifest["planned_cases"]["heldout"]["mock-jev"]), 216)
        for field in ("corpus_hash", "protocol_hash", "supplement_protocol_hash", "source_fingerprint", "config_hash"):
            changed = copy.deepcopy(self.manifest)
            changed[field] = "different"
            with self.assertRaises(ValueError):
                followup.verify_manifest(self.manifest, changed)

    def test_live_hardware_changes_fail_but_offline_replay_is_portable(self):
        changed = copy.deepcopy(self.manifest)
        changed["hardware"] = {"system": "other"}
        with self.assertRaises(ValueError):
            followup.verify_manifest(self.manifest, changed)
        followup.verify_manifest(self.manifest, changed, check_hardware=False)
        changed["source_fingerprint"] = "new-compatible-source"
        with self.assertRaises(ValueError):
            followup.verify_manifest(self.manifest, changed, check_hardware=False)
        followup.verify_manifest(self.manifest, changed, check_hardware=False, check_source=False)

    def test_shared_ledger_cannot_be_reset_with_a_new_path(self):
        config = configuration()
        config["ledger_path"] = "build/new-independent-ledger.json"
        with self.assertRaises(ValueError):
            followup.prepare_manifest(config, self.corpus)

    def test_prediction_binding_rejects_foreign_ids_gold_split_and_routes(self):
        rows = predictions(self.corpus, self.manifest, "heldout")
        followup.validate_predictions(rows, self.manifest, self.corpus, "heldout")
        mutations = [
            lambda row: row.update(run_manifest_hash="foreign"),
            lambda row: row.update(case_id="foreign-case"),
            lambda row: row.update(split="calibration"),
            lambda row: row.update(expected="foreign-target"),
            lambda row: row["identity"].update(returned_model="foreign-model"),
            lambda row: row["identity"].update(provider="Other"),
            lambda row: row["identity"]["routing"].update(allow_fallbacks=True),
            lambda row: row["identity"]["routing"].pop("data_collection"),
            lambda row: row["identity"]["routing"].pop("zdr"),
        ]
        for mutate in mutations:
            changed = copy.deepcopy(rows)
            mutate(changed[0])
            with self.assertRaises(ValueError):
                followup.validate_predictions(changed, self.manifest, self.corpus, "heldout")

    def test_missing_or_duplicate_cases_do_not_disappear_from_denominator(self):
        rows = predictions(self.corpus, self.manifest, "heldout")
        for changed in (rows[:-1], rows + [rows[0]]):
            with self.assertRaises(ValueError):
                followup.validate_predictions(changed, self.manifest, self.corpus, "heldout")

    def test_fitting_uses_only_calibration_predictions(self):
        calibration_rows = predictions(self.corpus, self.manifest, "calibration")
        artifact = followup.fit_recorded(calibration_rows, self.manifest, self.corpus)
        self.assertEqual(len(artifact["fits"]), 18)
        self.assertEqual(artifact["supplement_protocol_hash"], self.manifest["supplement_protocol_hash"])
        self.assertEqual(artifact["calibration_rows"], 216)
        with self.assertRaises(ValueError):
            followup.fit_recorded(predictions(self.corpus, self.manifest, "heldout"), self.manifest, self.corpus)

    def test_calibration_artifact_is_bound_to_its_supplement_and_calibration_ids(self):
        artifact = followup.fit_recorded(predictions(self.corpus, self.manifest, "calibration"), self.manifest, self.corpus)
        test = predictions(self.corpus, self.manifest, "heldout")
        for field in ("supplement_protocol_hash", "supplement_corpus_hash", "run_manifest_hash"):
            changed = copy.deepcopy(artifact)
            changed[field] = "different"
            with self.assertRaises(ValueError):
                followup.score_recorded(test, changed, self.manifest, self.corpus)
        changed = copy.deepcopy(artifact)
        next(iter(changed["fits"].values()))["fit"]["calibration_case_ids"][0] = test[0]["case_id"]
        with self.assertRaises(ValueError):
            followup.score_recorded(test, changed, self.manifest, self.corpus)

    def test_json_and_gzip_replay_match_without_live_runner(self):
        artifact = followup.fit_recorded(predictions(self.corpus, self.manifest, "calibration"), self.manifest, self.corpus)
        rows = predictions(self.corpus, self.manifest, "heldout")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for mode in ("json", "gzip"):
                target = root / mode
                target.mkdir()
                followup.run.save_json(target / "manifest.json", self.manifest)
                followup.run.save_json(target / "calibration.json", artifact)
                followup.run.save_json(target / "execution.json", {
                    "split": "heldout", "run_manifest_hash": followup.run.digest(self.manifest),
                    "calibration_sha256": followup.run.digest(artifact), "predictions_sha256": followup.run.digest(rows),
                })
                if mode == "json":
                    followup.run.save_json(target / "predictions.json", {"frozen": followup.run.digest(self.manifest), "rows": rows})
                else:
                    with gzip.open(target / "predictions.jsonl.gz", "wt", encoding="utf-8") as stream:
                        for row in rows:
                            stream.write(json.dumps(row, ensure_ascii=False) + "\n")
            with patch.object(followup.run, "live_predictions", side_effect=AssertionError("offline replay attempted inference")):
                first = followup.replay(root / "json")
                second = followup.replay(root / "gzip")
            self.assertEqual(first, second)
            with patch.object(followup.run, "source_fingerprint", return_value="new-compatible-source"):
                historical = followup.replay(root / "json")
            self.assertEqual(historical["manifest"]["source_fingerprint"], "mock-source")
            self.assertEqual(historical["replay_source_fingerprint"], "new-compatible-source")
            self.assertEqual(historical["calibrated"], first["calibrated"])
            altered = copy.deepcopy(artifact)
            altered["gates"].append("changed-after-execution")
            followup.run.save_json(root / "json/calibration.json", altered)
            with self.assertRaises(ValueError):
                followup.replay(root / "json")

    def test_output_reuse_is_rejected_before_live_inference(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            followup.run.save_json(root / "config.json", configuration())
            followup.run.save_json(root / "frozen.json", self.manifest)
            output = root / "output"
            output.mkdir()
            (output / "user-file.txt").write_text("preserve me")
            with patch.object(followup.run, "live_predictions", side_effect=AssertionError("must not infer")):
                with self.assertRaises(ValueError):
                    followup.main(["--live", "--split", "calibration", "--config", str(root / "config.json"),
                                   "--frozen", str(root / "frozen.json"), "--output", str(output)])
            self.assertEqual((output / "user-file.txt").read_text(), "preserve me")

    def test_heldout_requires_committed_freeze_before_inference(self):
        artifact = followup.fit_recorded(predictions(self.corpus, self.manifest, "calibration"), self.manifest, self.corpus)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, value in (("config", configuration()), ("frozen", self.manifest), ("calibration", artifact)):
                followup.run.save_json(root / f"{name}.json", value)
            with patch.object(followup, "require_committed", side_effect=ValueError("uncommitted freeze")) as committed:
                with patch.object(followup.run, "live_predictions", side_effect=AssertionError("must not infer")):
                    with self.assertRaisesRegex(ValueError, "uncommitted"):
                        followup.main(["--live", "--split", "heldout", "--config", str(root / "config.json"),
                            "--frozen", str(root / "frozen.json"), "--calibration", str(root / "calibration.json"), "--output", str(root / "heldout")])
            committed.assert_called_once()

    def test_prepare_preserves_order_sensitive_corpus_hash(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            followup.run.save_json(root / "config.json", configuration())
            followup.main(["--prepare", "--config", str(root / "config.json"), "--frozen", str(root / "freeze/manifest.json"),
                           "--output", str(root / "prepared")])
            corpus = followup.run.read_json(root / "prepared/corpus.json")
            manifest = followup.run.read_json(root / "prepared/manifest.json")
            self.assertEqual(supplement.corpus_hash(corpus), manifest["corpus_hash"])

    def test_live_calibration_can_be_mocked_without_accessing_credentials(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            followup.run.save_json(root / "config.json", configuration())
            followup.run.save_json(root / "frozen.json", self.manifest)
            output = root / "calibration"
            rows = predictions(self.corpus, self.manifest, "calibration")
            def mock_inference(corpus, bridge, config, split, destination, manifest):
                self.assertEqual(bridge["queries"], [])
                self.assertTrue((destination / "execution.json").is_file())
                self.assertEqual(split, "calibration")
                return rows, []
            with patch.object(followup.run, "live_predictions", side_effect=mock_inference):
                followup.main(["--live", "--split", "calibration", "--config", str(root / "config.json"),
                               "--frozen", str(root / "frozen.json"), "--output", str(output)])
            self.assertEqual(len(followup.run.read_json(output / "calibration.json")["fits"]), 18)


if __name__ == "__main__":
    unittest.main()
