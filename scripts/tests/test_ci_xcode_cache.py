import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / 'ci-xcode-cache.py'


def load_module():
    spec = importlib.util.spec_from_file_location('ci_xcode_cache', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class XcodeCacheTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cache = load_module()

    def metadata(self, **changes):
        values = {
            'revision': 'a' * 40,
            'xcode': 'Xcode 27.0\nBuild version 27A266a',
            'device_sdk': '27.0',
            'simulator_sdk': '27.0',
            'rustc': 'rustc 1.99.1\ncommit-hash: abc123',
            'architecture': 'arm64',
            'variant': 'preview',
            'dependency_sha256': 'b' * 64,
        }
        return values | changes

    def test_key_is_stable_and_partitions_native_toolchains_and_dependencies(self):
        expected = self.cache.cache_identity(self.metadata(), 'pilot-1')
        self.assertEqual(expected, self.cache.cache_identity(dict(reversed(list(self.metadata().items()))), 'pilot-1'))
        for name, value in (
            ('xcode', 'Xcode 27.0\nBuild version 27A267b'),
            ('device_sdk', '27.1'),
            ('simulator_sdk', '27.1'),
            ('rustc', 'rustc 1.99.2\ncommit-hash: def456'),
            ('architecture', 'x86_64'),
            ('variant', 'production'),
            ('dependency_sha256', 'c' * 64),
        ):
            with self.subTest(name=name):
                self.assertNotEqual(expected, self.cache.cache_identity(self.metadata(**{name: value}), 'pilot-1'))

    def test_commit_and_generation_rotate_snapshot_without_cross_context_fallback(self):
        first = self.cache.cache_identity(self.metadata(), 'pilot-1')
        new_commit = self.cache.cache_identity(self.metadata(revision='d' * 40), 'pilot-1')
        self.assertNotEqual(first['key'], new_commit['key'])
        self.assertEqual(first['restore_prefix'], new_commit['restore_prefix'])
        self.assertNotEqual(first['key'], self.cache.cache_identity(self.metadata(), 'pilot-2')['key'])

    def test_warm_requires_the_exact_seed_and_cold_rejects_existing_seed(self):
        self.cache.validate_restore('warm', 'true')
        self.cache.validate_restore('cold', '')
        self.cache.validate_restore('off', '')
        for mode, hit in [('warm', ''), ('warm', 'false'), ('cold', 'true')]:
            with self.subTest(mode=mode, hit=hit), self.assertRaises(ValueError):
                self.cache.validate_restore(mode, hit)

    def test_only_one_cold_producer_can_seed_a_snapshot(self):
        self.cache.validate_inputs('cold', 1, 'pilot-1')
        self.cache.validate_inputs('warm', 3, 'pilot-1')
        self.cache.validate_inputs('off', 3, 'pilot-1')
        for mode, repetitions, generation in [('cold', 3, 'v1'), ('warm', 2, 'v1'), ('invalid', 1, 'v1'), ('cold', 1, '../bad')]:
            with self.subTest(mode=mode, repetitions=repetitions, generation=generation), self.assertRaises(ValueError):
                self.cache.validate_inputs(mode, repetitions, generation)

    def test_cache_budget_counts_current_usage_and_conservative_uncompressed_size(self):
        limit = self.cache.CACHE_ALLOWANCE_BYTES
        reserve = self.cache.CACHE_RESERVE_BYTES
        self.assertTrue(self.cache.fits_budget(limit - reserve - 100, 100))
        self.assertFalse(self.cache.fits_budget(limit - reserve - 100, 101))
        self.assertFalse(self.cache.fits_budget(limit, 0))
        with self.assertRaises(ValueError):
            self.cache.fits_budget(-1, 100)

    def test_dependency_digest_includes_file_names_and_generated_pod_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = root / 'package-lock.json'
            second = root / 'Podfile.lock'
            first.write_text('locked npm')
            second.write_text('locked pods')
            digest = self.cache.digest_files(root, [first, second])
            self.assertEqual(digest, self.cache.digest_files(root, [second, first]))
            second.write_text('changed pods')
            self.assertNotEqual(digest, self.cache.digest_files(root, [first, second]))
            second.write_text('locked pods')
            second.rename(root / 'different.lock')
            self.assertNotEqual(digest, self.cache.digest_files(root, [first, root / 'different.lock']))

    def test_cold_preflight_refuses_a_full_repository_before_building(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / 'receipt.json'
            with mock.patch.object(self.cache, 'REPORT', report), mock.patch.object(self.cache, 'github_json', return_value={'active_caches_size_in_bytes': self.cache.CACHE_ALLOWANCE_BYTES}):
                with self.assertRaisesRegex(ValueError, 'insufficient headroom'):
                    self.cache.preflight()
                self.assertTrue(report.exists())

    def test_disabled_wrapper_preserves_original_arguments(self):
        arguments = ['test', '-workspace', '/tmp/My App/Pennel.xcworkspace', 'CODE_SIGNING_ALLOWED=YES']
        self.assertEqual(self.cache.xcode_arguments(arguments, 'off', Path('/tmp/cache')), arguments)

    def test_all_enabled_configurations_use_the_same_explicit_cas(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory) / 'shared cas'
            for action in ('build', 'test'):
                for mode in ('cold', 'warm'):
                    arguments = self.cache.xcode_arguments([action, '-configuration', 'Debug' if action == 'test' else 'Release'], mode, cache)
                    self.assertIn('COMPILATION_CACHE_ENABLE_CACHING=YES', arguments)
                    self.assertIn('COMPILATION_CACHE_ENABLE_DIAGNOSTIC_REMARKS=YES', arguments)
                    self.assertIn(f'COMPILATION_CACHE_CAS_PATH={cache}', arguments)
                    self.assertEqual(arguments[0], action)

    def test_relative_cas_path_is_rejected(self):
        with self.assertRaises(ValueError):
            self.cache.xcode_arguments(['build'], 'warm', Path('relative/cache'))


if __name__ == '__main__':
    unittest.main()
