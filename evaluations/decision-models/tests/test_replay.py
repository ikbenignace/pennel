import gzip
import json
import math
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

DIRECTORY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(DIRECTORY))
import run
import calibration
from corpus import build_corpus


class MeasuredReplayTests(unittest.TestCase):
    def curve_knots(self, points):
        # libm can split display-only confidence ties by ULPs across Python versions.
        knots = {}
        for point in points:
            key = round(point["threshold"], 12)
            if key not in knots or point["accepted_count"] > knots[key]["accepted_count"]:
                knots[key] = point
        return [knots[key] for key in sorted(knots, reverse=True)]

    def test_protocol_identity_ignores_checkout_line_endings(self):
        content = (DIRECTORY / "protocol.json").read_bytes().replace(b"\r\n", b"\n")
        expected = calibration._protocol_hash()
        with patch.object(Path, "read_bytes", return_value=content.replace(b"\n", b"\r\n")):
            self.assertEqual(calibration._protocol_hash(), expected)

    def assert_portable_equal(self, expected, actual):
        if isinstance(expected, float):
            self.assertTrue(math.isclose(expected, actual, rel_tol=1e-10, abs_tol=1e-10),
                            f"Numerical replay differs: {expected} != {actual}")
        elif isinstance(expected, dict):
            self.assertEqual(set(expected), set(actual))
            for key in expected:
                if key in ("raw_selective_curve", "calibrated_selective_curve"):
                    self.assert_portable_equal(self.curve_knots(expected[key]), self.curve_knots(actual[key]))
                else:
                    self.assert_portable_equal(expected[key], actual[key])
        elif isinstance(expected, list):
            self.assertEqual(len(expected), len(actual))
            for left, right in zip(expected, actual):
                self.assert_portable_equal(left, right)
        else:
            self.assertEqual(expected, actual)

    def test_measured_predictions_reproduce_quality_without_models_or_network(self):
        for name in ("2026-10-28", "openrouter-2026-10-28"):
            with self.subTest(recorded_run=name):
                directory = DIRECTORY / "results" / name
                with gzip.open(directory / "report.json.gz", "rt", encoding="utf-8") as stream:
                    original = json.load(stream)
                replayed = {}
                run.replay_recorded(replayed, directory, build_corpus())
                for key in ("live", "reranking", "calibrated"):
                    self.assert_portable_equal(original[key], replayed[key])
                self.assertEqual(replayed["recorded_inference"]["evidence_kind"],
                                 "replay_of_measured_openrouter_inference" if name.startswith("openrouter")
                                 else "replay_of_measured_local_inference")


if __name__ == "__main__":
    unittest.main()
