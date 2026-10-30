import copy
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import measurements


def corpus():
    cases = []
    for language in ("en", "nl", "mixed"):
        for task in ("cleanup", "answerability"):
            case = {"id": f"b-{language}-{task}", "task": task, "language": language,
                    "split": "heldout", "state": "Synthetic content", "tags": [],
                    "question": {"type": "noul", "instructions": "Supported?"}, "label_order": ["false", "true"]}
            cases.extend([case, {**case, "id": "a-long-" + case["id"], "tags": ["long_input"]},
                          {**case, "id": "a-variant-" + case["id"], "variant_of": case["id"]},
                          {**case, "id": "a-development-" + case["id"], "split": "development"}])
    return {"decisions": cases}


def ranking():
    return [{"id": f"ranking-{language}", "task": "ranking", "language": language,
             "split": "heldout", "query": "Synthetic query", "passages": [{"id": "source", "text": "Evidence"}]}
            for language in ("en", "nl", "mixed")]


def config(model="classifier", tasks=None):
    result = {"id": model, "model": model, "transport": "local_worker", "enabled": True,
              "model_dir": "/synthetic/model"}
    if tasks is not None:
        result["tasks"] = tasks
    return result


class Factory:
    def __init__(self, unavailable=False, missing_latency=False, prediction_error=False):
        self.instances = []
        self.unavailable = unavailable
        self.missing_latency = missing_latency
        self.prediction_error = prediction_error

    def __call__(self, model, model_dir, python=None):
        owner = self

        class Worker:
            def __init__(self):
                self.calls = []
                self.closed = False
                self.observed_peak_rss_bytes = 0

            def start(self):
                if owner.unavailable:
                    return {"status": "unavailable", "reason": "artifact_not_downloaded"}
                return {"status": "ready", "load_ms": 10, "process_start_ms": 12,
                        "peak_rss_bytes": 100, "identity": {"model": model, "device": "cpu",
                                                            "packages": {"synthetic-runtime": "1.0"}}}

            def predict(self, case):
                self.calls.append(case["id"])
                if owner.prediction_error:
                    raise RuntimeError("private content must not be recorded")
                self.observed_peak_rss_bytes = 150
                result = {"status": "ok", "latency_ms": 100 if len(self.calls) == 1 else 2,
                          "peak_rss_bytes": 120, "memory_guard": {"observed_peak_rss_bytes": 140}}
                if owner.missing_latency:
                    del result["latency_ms"]
                return result

            def close(self):
                self.closed = True

        instance = Worker()
        self.instances.append(instance)
        return instance


class MeasurementTests(unittest.TestCase):
    def measure(self, configs=None, factory=None, **kwargs):
        with patch.object(measurements, "registry", return_value={"classifier": {"download_bytes": 123},
                                                                  "gte": {"download_bytes": 456}}):
            return measurements.measure_local(configs or [config(tasks=["cleanup", "answerability"])],
                                               corpus(), ranking(), worker_factory=factory or Factory(), **kwargs)

    def test_panel_is_stable_canonical_short_and_heldout(self):
        original = corpus()
        first = measurements.panel_manifest([config()], original, ranking())
        reordered = {"decisions": list(reversed(original["decisions"]))}
        second = measurements.panel_manifest([config()], reordered, list(reversed(ranking())))
        self.assertEqual(first, second)
        self.assertEqual(len(first["case_ids"]), 9)
        self.assertFalse(any("development" in value or "long" in value or "variant" in value for value in first["case_ids"]))
        self.assertEqual(first["split"], "heldout")
        self.assertEqual(first["cold_samples_per_language"], 5)
        self.assertEqual(first["warm_samples_per_case"], 30)
        changed = copy.deepcopy(original)
        next(case for case in changed["decisions"] if case["id"] == "b-en-cleanup")["state"] = "Changed content"
        self.assertNotEqual(first["panel_hash"], measurements.panel_manifest([config()], changed, ranking())["panel_hash"])

    def test_default_cold_is_five_fresh_starts_per_language_and_warmups_excluded(self):
        factory = Factory()
        report = self.measure(factory=factory)
        model = report["models"][0]
        self.assertEqual(model["measurement"], "mocked_worker")
        self.assertEqual(model["status"], "ok")
        self.assertEqual(model["startup_attempts"], 16)
        self.assertEqual(len(factory.instances), 16)
        self.assertTrue(all(worker.closed for worker in factory.instances))
        self.assertTrue(all(len(worker.calls) == 1 for worker in factory.instances[:15]))
        self.assertEqual(len(factory.instances[-1].calls), 6 * 31)
        self.assertEqual(model["load"]["n"], 15)
        self.assertEqual(model["process_start"]["n"], 15)
        self.assertEqual(model["peak_rss_bytes"], 150)
        self.assertEqual(model["artifact_manifest_bytes"], 123)
        for cell in model["cases"]:
            self.assertEqual(cell["warm"]["n"], 30)
            self.assertEqual(cell["warm"]["p95_ms"], 2)
            if cell["task"] == "cleanup":
                self.assertEqual(cell["cold"]["n"], 5)
                self.assertEqual(cell["cold"]["p50_ms"], 100)
                self.assertEqual(cell["load"]["n"], 5)
            else:
                self.assertFalse(cell["cold_measured"])
                self.assertEqual(cell["cold"]["n"], 0)
                self.assertIsNone(cell["cold"]["p50_ms"])

    def test_gte_runs_ranking_only_and_supported_tasks_are_respected(self):
        factory = Factory()
        report = self.measure([config("gte", ["ranking"])], factory, cold_samples=1, warm_samples=2)
        self.assertEqual(report["models"][0]["startup_attempts"], 4)
        self.assertEqual({cell["task"] for cell in report["models"][0]["cases"]}, {"ranking"})
        self.assertTrue(all(case.startswith("ranking-") for worker in factory.instances for case in worker.calls))

    def test_disabled_and_hosted_configs_never_start_workers(self):
        factory = Factory()
        report = self.measure([dict(config(), enabled=False), {"transport": "openrouter", "enabled": True}], factory)
        self.assertEqual(report["models"], [])
        self.assertEqual(factory.instances, [])
        self.assertEqual(report["panel"]["runners"], [])

    def test_unavailable_startup_stops_repetition_and_does_not_fabricate_measurements(self):
        factory = Factory(unavailable=True)
        model = self.measure(factory=factory)["models"][0]
        self.assertEqual(len(factory.instances), 1)
        self.assertTrue(factory.instances[0].closed)
        self.assertEqual(model["startup_attempts"], 1)
        self.assertEqual(model["load"]["n"], 0)
        self.assertIsNone(model["peak_rss_bytes"])
        self.assertIsNone(model["native_binary_size_bytes"])
        self.assertEqual(model["failures"][0]["reason"], "artifact_not_downloaded")
        self.assertTrue(all(cell["warm"]["n"] == 0 for cell in model["cases"]))

    def test_failed_or_missing_latency_samples_are_counted_but_not_zero(self):
        for factory in (Factory(missing_latency=True), Factory(prediction_error=True)):
            model = self.measure(factory=factory, cold_samples=1, warm_samples=2)["models"][0]
            for cell in model["cases"]:
                self.assertEqual(cell["cold"]["n"], 0)
                self.assertEqual(cell["warm"]["n"], 0)
                self.assertIsNone(cell["cold"]["p95_ms"])
                self.assertIsNone(cell["warm"]["p95_ms"])
            self.assertTrue(all(worker.closed for worker in factory.instances))
            self.assertEqual(model["status"], "unavailable_or_partial")

    def test_non_positive_or_unbounded_counts_rejected(self):
        for value in (0, -1, 1001, True, 1.5):
            with self.assertRaises(ValueError):
                measurements.panel_manifest([config()], corpus(), ranking(), cold_samples=value)


if __name__ == "__main__":
    unittest.main()
