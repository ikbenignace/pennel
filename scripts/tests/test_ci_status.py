import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / 'ci-status.py'


class StatusTests(unittest.TestCase):
    def check(self, legacy=False, platform=False, build='skipped', standalone='skipped', changes='success', core=False):
        spec = importlib.util.spec_from_file_location('ci_status', SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module.failures({
            'changes': {'result': changes, 'outputs': {
                'core': str(core).lower(), 'legacy': str(legacy).lower(), 'platform': str(platform).lower(),
            }},
            'build-and-test': {'result': build},
            'platform': {'result': standalone},
        })

    def test_intentionally_skipped_jobs_pass(self):
        self.assertEqual(self.check(), [])

    def test_legacy_runs_platform_tests_when_both_selected(self):
        self.assertEqual(self.check(legacy=True, platform=True, build='success'), [])

    def test_platform_only_requires_standalone_job(self):
        self.assertEqual(self.check(platform=True, standalone='success'), [])
        self.assertTrue(self.check(platform=True))

    def test_selected_legacy_cannot_be_silently_skipped(self):
        self.assertTrue(self.check(legacy=True))

    def test_core_checks_cannot_escape_the_legacy_job(self):
        self.assertTrue(self.check(core=True))
        self.assertEqual(self.check(core=True, legacy=True, build='success'), [])

    def test_failures_and_cancellations_never_pass(self):
        for result in ('failure', 'cancelled'):
            with self.subTest(result=result):
                self.assertTrue(self.check(legacy=True, build=result))
                self.assertTrue(self.check(platform=True, standalone=result))
                self.assertTrue(self.check(changes=result))
                self.assertTrue(self.check(build=result))

    def test_unknown_or_incomplete_status_fails(self):
        self.assertTrue(self.check(legacy=True, build='pending'))
        self.assertTrue(self.check(changes='skipped'))


if __name__ == '__main__':
    unittest.main()
