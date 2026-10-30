import gzip
import json
import math
from collections import Counter
from decimal import Decimal
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

DIRECTORY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(DIRECTORY))
import followup
import generate
import runners


def read_gzip(path):
    with gzip.open(path, 'rt', encoding='utf-8') as stream:
        return json.load(stream)


class FollowupReplayTests(unittest.TestCase):
    directory = DIRECTORY / 'results/followup-2026-10-28'

    def assert_portable(self, expected, actual):
        if isinstance(expected, float):
            self.assertTrue(math.isclose(expected, actual, rel_tol=1e-10, abs_tol=1e-10))
        elif isinstance(expected, dict):
            self.assertEqual(set(expected), set(actual))
            for key in expected:
                if key == 'replay_source_fingerprint':
                    continue
                if key in ('raw_selective_curve', 'calibrated_selective_curve'):
                    def knots(points):
                        result = {}
                        for point in points:
                            value = round(point['threshold'], 12)
                            if value not in result or point['accepted_count'] > result[value]['accepted_count']:
                                result[value] = point
                        return [result[value] for value in sorted(result, reverse=True)]
                    self.assert_portable(knots(expected[key]), knots(actual[key]))
                else:
                    self.assert_portable(expected[key], actual[key])
        elif isinstance(expected, list):
            self.assertEqual(len(expected), len(actual))
            for left, right in zip(expected, actual):
                self.assert_portable(left, right)
        else:
            self.assertEqual(expected, actual)

    def test_actual_quality_and_evidence_reports_replay_without_network(self):
        with patch.object(runners.HttpTransport, '__call__', side_effect=AssertionError('Replay attempted HTTP')):
            original = read_gzip(self.directory / 'supplement/report.json.gz')
            replayed = followup.replay(self.directory / 'supplement')
            for key in ('live', 'calibrated', 'feasibility', 'calibration_sha256'):
                self.assert_portable(original[key], replayed[key])
            self.assertEqual(replayed['calibrated']['heldout_rows'], 1080)
            self.assertEqual(replayed['status_counts'], {'error': 1, 'ok': 1079})
            original = read_gzip(self.directory / 'supplement-local/report.json.gz')
            local = followup.replay(self.directory / 'supplement-local')
            for key in ('live', 'calibrated', 'feasibility', 'calibration_sha256'):
                self.assert_portable(original[key], local[key])
            self.assertEqual(local['calibrated']['heldout_rows'], 432)
            self.assertEqual(local['status_counts'], {'ok': 432})
            original = read_gzip(self.directory / 'generative/report.json.gz')
            replayed = generate.report(self.directory / 'generative')
            self.assert_portable(original['languages'], replayed['languages'])
            self.assertEqual(sum(cell['adjudicated'] for language in replayed['languages'].values()
                                 for cell in language.values()), 54)
            self.assertEqual(sum(cell['pending'] for language in replayed['languages'].values()
                                 for cell in language.values()), 0)
            for language in ('en', 'nl', 'mixed'):
                self.assertTrue(replayed['languages'][language]['meeting_qa']['adoption_blocked'])

    def test_timing_keeps_errors_and_unavailable_slots_visible(self):
        with gzip.open(self.directory / 'supplement/timing-receipts.jsonl.gz', 'rt', encoding='utf-8') as stream:
            rows = [json.loads(line) for line in stream if line.strip()]
        warm = Counter((row['runner_id'], row['case_id']) for row in rows if row['phase'] == 'warm')
        cold = Counter(row['runner_id'] for row in rows if row['phase'] == 'cold_client')
        for model in ('openrouter-jev', 'openrouter-decider', 'openrouter-decider-flash', 'openrouter-mercury'):
            self.assertEqual(sorted(count for (runner, _), count in warm.items() if runner == model), [30] * 9)
            self.assertEqual(cold[model], 15)
        self.assertEqual(sum(count for (model, _), count in warm.items() if model == 'openrouter-clef-flash'), 35)
        errors = Counter(row.get('reason') for row in rows if row['status'] != 'ok')
        self.assertEqual(errors, {'probabilities_do_not_sum_to_one': 2, 'http_429': 2, 'http_502': 1})
        with gzip.open(self.directory / 'mercury/receipts.jsonl.gz', 'rt', encoding='utf-8') as stream:
            original = [json.loads(line) for line in stream if line.strip()]
        self.assertEqual(sum(row['phase'] == 'warm' for row in original), 630)
        self.assertEqual(sum(row['phase'] == 'warm' and row['status'] != 'ok' for row in original), 2)

    def test_cumulative_cost_contains_every_resolved_attempt(self):
        ledger = read_gzip(self.directory / 'billing-ledger.json.gz')
        self.assertFalse(ledger['unresolved_charge'])
        self.assertEqual(ledger['pending_reservations'], {})
        self.assertEqual(ledger['attempt_count'], len(ledger['attempts']))
        self.assertEqual(ledger['attempt_count'], 35555)
        actual = sum((Decimal(attempt['billed_cost_usd']) for attempt in ledger['attempts']), Decimal(0))
        self.assertEqual(actual, Decimal('0.532481940'))
        self.assertLessEqual(actual, Decimal(ledger['limit_usd']))


if __name__ == '__main__':
    unittest.main()
