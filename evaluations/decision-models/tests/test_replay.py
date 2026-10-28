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
                self.assert_portable_equal(expected[key], actual[key])
        elif isinstance(expected, list):
            self.assertEqual(len(expected), len(actual))
            for left, right in zip(expected, actual):
                self.assert_portable_equal(left, right)
        else:
            self.assertEqual(expected, actual)

    def test_measured_predictions_reproduce_quality_without_models_or_network(self):
        directory = DIRECTORY / "results" / "2026-10-28"
        with gzip.open(directory / "report.json.gz", "rt", encoding="utf-8") as stream:
            original = json.load(stream)
        replayed = {}
        run.replay_recorded(replayed, directory, build_corpus())
        for key in ("live", "reranking", "calibrated"):
            self.assert_portable_equal(original[key], replayed[key])
        self.assertEqual(replayed["recorded_inference"]["evidence_kind"],
                         "replay_of_measured_local_inference")


if __name__ == "__main__":
    unittest.main()
