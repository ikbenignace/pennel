import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch


DIRECTORY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(DIRECTORY))
import local_models

SPEC = importlib.util.spec_from_file_location("local_inference_worker", DIRECTORY / "workers" / "inference.py")
worker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(worker)


def tiny_registry():
    data = b"synthetic artifact"
    return {"tiny": {"repo": "synthetic/tiny", "revision": "a" * 40,
                     "precision": "float32", "environment": "none",
                     "files": [{"path": "model.safetensors", "size": len(data),
                                "sha256": hashlib.sha256(data).hexdigest()}]}}


class LocalModelsTests(unittest.TestCase):
    def test_manifest_pins_primary_artifacts_within_budget(self):
        entries = local_models.registry()
        primary = ["gliclass-multilingual", "gliclass-v4-q8", "gliner-multilingual", "gte-reranker"]
        self.assertLess(sum(entries[key]["download_bytes"] for key in primary), 3 * 1024**3)
        self.assertGreater(sum(entries[key]["download_bytes"] for key in [*primary, "laya"]), 3 * 1024**3)
        for key, value in entries.items():
            self.assertRegex(value["revision"], r"^[a-f0-9]{40}$")
            self.assertEqual(value["device"], "cpu")
            self.assertEqual(value["threads"], 4)
            for item in value["files"]:
                self.assertNotIn("..", Path(item["path"]).parts)
                self.assertFalse(Path(item["path"]).is_absolute())
        self.assertNotIn("LiquidAI/d1-2B", entries)

    def test_gte_code_is_separately_pinned(self):
        entry = local_models.registry()["gte-reranker"]
        sources = [item for item in entry["files"] if item["path"].startswith("custom_code/")]
        self.assertEqual(len(sources), 2)
        for source in sources:
            self.assertEqual(source["revision"], entry["code_revision"])
            self.assertRegex(source["sha256"], r"^[a-f0-9]{64}$")

    def test_acquisition_is_explicit_verified_and_not_repeated(self):
        calls = []
        def opener(url, timeout):
            calls.append(url)
            return io.BytesIO(b"synthetic artifact")
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", tiny_registry):
            result = local_models.fetch_models(["tiny"], root, budget_bytes=100, opener=opener)
            self.assertEqual(result["tiny"]["status"], "available")
            local_models.fetch_models(["tiny"], root, budget_bytes=100, opener=opener)
            self.assertEqual(len(calls), 1)
            self.assertIn("/resolve/" + "a" * 40 + "/", calls[0])

    def test_oversized_download_fails_before_network(self):
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", tiny_registry):
            with self.assertRaisesRegex(local_models.LocalModelError, "budget_exhausted"):
                local_models.fetch_models(["tiny"], root, budget_bytes=1,
                                          opener=lambda *_: self.fail("network must not run"))

    def test_unauthorized_higher_download_ceiling_is_rejected(self):
        with self.assertRaisesRegex(local_models.LocalModelError, "authorization"):
            local_models.fetch_models([], "/unused", budget_bytes=3 * 1024**3 + 1)

    def test_changed_artifact_is_not_accepted_or_overwritten(self):
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", tiny_registry):
            local_models.fetch_models(["tiny"], root, budget_bytes=100,
                                      opener=lambda *_a, **_kw: io.BytesIO(b"synthetic artifact"))
            artifact = local_models.model_directory("tiny", root) / "model.safetensors"
            artifact.write_bytes(b"changed artifact!!")
            self.assertEqual(local_models.model_status("tiny", root)["status"], "unavailable")
            with self.assertRaisesRegex(local_models.LocalModelError, "refuse_overwrite"):
                local_models.fetch_models(["tiny"], root, budget_bytes=100,
                                          opener=lambda *_: self.fail("must not overwrite"))

    def test_failed_transfer_reservation_is_not_refunded(self):
        def fail(*_args, **_kwargs):
            raise OSError("synthetic network failure")
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", tiny_registry):
            with self.assertRaises(OSError):
                local_models.fetch_models(["tiny"], root, budget_bytes=20, opener=fail)
            with self.assertRaisesRegex(local_models.LocalModelError, "budget_exhausted"):
                local_models.fetch_models(["tiny"], root, budget_bytes=20, opener=fail)
            self.assertFalse((Path(root) / ".download.lock").exists())

    def test_resume_preserves_already_verified_files(self):
        entries = tiny_registry()
        entries["tiny"]["files"].append({**entries["tiny"]["files"][0], "path": "tokenizer.json"})
        calls = []
        def partial(url, timeout):
            calls.append(url)
            if url.endswith("tokenizer.json"):
                raise OSError("synthetic interruption")
            return io.BytesIO(b"synthetic artifact")
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", lambda: entries):
            with self.assertRaises(OSError):
                local_models.fetch_models(["tiny"], root, budget_bytes=100, opener=partial)
            def complete(url, timeout):
                calls.append(url)
                return io.BytesIO(b"synthetic artifact")
            result = local_models.fetch_models(["tiny"], root, budget_bytes=100, opener=complete)
            self.assertEqual(result["tiny"]["status"], "available")
            self.assertEqual(sum(url.endswith("model.safetensors") for url in calls), 1)

    def test_download_hash_mismatch_never_promotes_partial_file(self):
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", tiny_registry):
            with self.assertRaisesRegex(local_models.LocalModelError, "verification_failed"):
                local_models.fetch_models(["tiny"], root, budget_bytes=100,
                                          opener=lambda *_a, **_kw: io.BytesIO(b"invalid artifact!!"))
            self.assertFalse((local_models.model_directory("tiny", root) / "model.safetensors").exists())
            self.assertFalse(list(Path(root).rglob("*.part")))

    def test_concurrent_acquisition_cannot_share_budget_reservation(self):
        with tempfile.TemporaryDirectory() as root, patch.object(local_models, "registry", tiny_registry):
            (Path(root) / ".download.lock").touch()
            with self.assertRaisesRegex(local_models.LocalModelError, "already_running"):
                local_models.fetch_models(["tiny"], root, budget_bytes=100)

    def test_missing_environment_is_unavailable_without_launch(self):
        client = local_models.LocalWorker("gte-reranker", "/not/a/model", python="/not/an/interpreter")
        result = client.predict({"id": "synthetic", "task": "ranking"})
        self.assertEqual(result["status"], "unavailable")
        expected = ("persistent_worker_measurement_unavailable_on_windows"
                    if sys.platform == "win32" else "isolated_environment_not_installed")
        self.assertEqual(result["reason"], expected)
        self.assertIsNone(client.process)

    def test_unmeasured_windows_worker_does_not_launch(self):
        client = local_models.LocalWorker("gte-reranker", "/not/a/model", python="/not/an/interpreter")
        with patch.object(local_models, "os", types.SimpleNamespace(name="nt")):
            result = client.predict({"id": "synthetic", "task": "ranking"})
        self.assertEqual(result["reason"], "persistent_worker_measurement_unavailable_on_windows")
        self.assertIsNone(client.process)

    def test_missing_model_worker_startup_uses_no_optional_packages(self):
        result = subprocess.run(
            [sys.executable, str(DIRECTORY / "workers" / "inference.py"),
             "--model", "gte-reranker", "--model-dir", "/not/a/model"],
            input=json.dumps({"id": "synthetic", "task": "ranking"}) + "\n",
            capture_output=True, text=True, check=True, timeout=10)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(rows[0]["status"], "unavailable")
        self.assertEqual(rows[1]["status"], "unavailable")
        self.assertNotIn("optional_package_missing", result.stdout)

    def test_label_order_and_binary_ids_survive_preparation(self):
        ids, labels = worker.ordered_labels({"type": "choice", "criteria": {"z": "last", "a": "first"}})
        self.assertEqual(ids, ["z", "a"])
        self.assertEqual(labels, ["z — last", "a — first"])
        ids, _ = worker.ordered_labels({"type": "noul", "criteria": {"true": "yes", "false": "no"}})
        self.assertEqual(ids, ["true", "false"])

    def test_reranking_retains_logits_and_tie_order(self):
        rows = [{"id": "c"}, {"id": "a"}, {"id": "b"}]
        result = worker.checked_ranking(rows, [-5, 3.5, 3.5])
        self.assertEqual(result["ranked_ids"], ["a", "b", "c"])
        self.assertEqual(result["scores"]["c"], -5)
        with self.assertRaises(worker.Unavailable):
            worker.checked_ranking(rows, [0, float("nan"), 1])

    def test_reranker_cannot_be_used_as_decision_probability(self):
        backend = object.__new__(worker.Backend)
        backend.backend = "gte"
        with self.assertRaisesRegex(worker.Unavailable, "does_not_support"):
            backend.predict({"task": "answerability"})

    def test_unverified_onnx_does_not_import_or_infer(self):
        backend = object.__new__(worker.Backend)
        with tempfile.TemporaryDirectory() as root:
            backend.path = Path(root)
            with self.assertRaisesRegex(worker.Unavailable, "parity_unverified"):
                backend._load_onnx()

    def test_probability_normalization_rejects_nonfinite_scores(self):
        self.assertAlmostEqual(sum(worker.softmax([1000.0, 1001.0])), 1)
        with self.assertRaises(worker.Unavailable):
            worker.softmax([float("inf"), 1])

    @unittest.skipIf(sys.platform == "win32", "POSIX pipe reader")
    def test_partial_json_line_respects_deadline(self):
        client = local_models.LocalWorker("gte-reranker", "/not/a/model", timeout=0.1)
        client.process = subprocess.Popen(
            [sys.executable, "-c", "import os,time;os.write(1,b'{');time.sleep(2)"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=0)
        client.startup = {"status": "ready", "identity": {"model": "synthetic"}}
        before = time.monotonic()
        try:
            with self.assertRaisesRegex(local_models.LocalModelError, "worker_timeout"):
                client._read()
            self.assertLess(time.monotonic() - before, 0.8)
            self.assertEqual(client.predict({"id": "after-timeout"})["status"], "unavailable")
        finally:
            client.close()

    def test_gliner_token_preflight_uses_exact_untruncated_record(self):
        backend = object.__new__(worker.Backend)
        backend.backend = "gliner"
        backend.limit = 12
        calls = []
        def transform(text, schema, max_len):
            calls.append(max_len)
            return types.SimpleNamespace(input_ids=list(range(13)))
        backend.model = types.SimpleNamespace(
            _classification_schema=lambda schema: schema,
            processor=types.SimpleNamespace(transform_record=transform))
        with self.assertRaisesRegex(worker.Unavailable, "context_budget"):
            backend._check_tokens("short text", "prompt", ["false", "true"],
                                  {"type": "noul"})
        self.assertEqual(calls, [None])


if __name__ == "__main__":
    unittest.main()
