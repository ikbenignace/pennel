import importlib.util
import io
import json
from pathlib import Path
import tarfile
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

    def test_only_cold_packs_and_only_warm_unpacks(self):
        self.cache.validate_snapshot_mode('pack', 'cold')
        self.cache.validate_snapshot_mode('unpack', 'warm')
        for action, mode in [('pack', 'off'), ('pack', 'warm'), ('unpack', 'off'), ('unpack', 'cold')]:
            with self.subTest(action=action, mode=mode), self.assertRaises(ValueError):
                self.cache.validate_snapshot_mode(action, mode)

    def test_only_one_cold_producer_can_seed_a_snapshot(self):
        self.cache.validate_inputs('cold', 1, 'pilot-1')
        self.cache.validate_inputs('warm', 3, 'pilot-1')
        self.cache.validate_inputs('off', 3, 'pilot-1')
        for mode, repetitions, generation in [('cold', 3, 'v1'), ('warm', 2, 'v1'), ('invalid', 1, 'v1'), ('cold', 1, '../bad')]:
            with self.subTest(mode=mode, repetitions=repetitions, generation=generation), self.assertRaises(ValueError):
                self.cache.validate_inputs(mode, repetitions, generation)

    def test_cache_budget_counts_current_usage_and_archive_size(self):
        limit = self.cache.CACHE_ALLOWANCE_BYTES
        reserve = self.cache.CACHE_RESERVE_BYTES
        self.assertTrue(self.cache.fits_budget(limit - reserve - 100, 100))
        self.assertFalse(self.cache.fits_budget(limit - reserve - 100, 101))
        self.assertFalse(self.cache.fits_budget(limit, 0))
        with self.assertRaises(ValueError):
            self.cache.fits_budget(-1, 100)

    def test_budget_uses_compressed_payload_rather_than_raw_cas_size(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'cas'
            source.mkdir()
            (source / 'object').write_bytes(b'compressible' * 1000)
            archive = root / 'snapshot.tar.gz'
            raw_bytes = self.cache.pack_snapshot(source, archive)
            usage = self.cache.CACHE_ALLOWANCE_BYTES - self.cache.CACHE_RESERVE_BYTES - archive.stat().st_size
            report = root / 'receipt.json'
            report.write_text(json.dumps({'phases': [], 'cache': {'mode': 'cold', 'uncompressed_bytes': raw_bytes}}))
            with mock.patch.object(self.cache, 'REPORT', report), mock.patch.object(self.cache, 'CACHE_ARCHIVE', archive), mock.patch.object(self.cache, 'github_json', return_value={'active_caches_size_in_bytes': usage}), mock.patch.object(self.cache, 'output'):
                self.cache.budget()
            data = json.loads(report.read_text())['cache']
            self.assertTrue(data['save_allowed'])
            self.assertEqual(data['archive_bytes'], archive.stat().st_size)
            self.assertEqual(data['uncompressed_bytes'], raw_bytes)

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

    def test_compiler_cache_roundtrip_preserves_nested_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            (source / 'builtin/v1.1').mkdir(parents=True)
            payload = b'compiler object\0' * 1000
            (source / 'builtin/v1.1/object').write_bytes(payload)
            (source / 'index').write_bytes(b'index')
            archive = root / 'snapshot.tar.gz'
            original_bytes = self.cache.pack_snapshot(source, archive)
            destination = root / 'restored'
            restored_bytes = self.cache.unpack_snapshot(archive, destination)
            self.assertEqual(original_bytes, restored_bytes)
            self.assertEqual((destination / 'builtin/v1.1/object').read_bytes(), payload)
            self.assertEqual((destination / 'index').read_bytes(), b'index')
            self.assertLess(archive.stat().st_size, original_bytes)

    def test_packing_empty_or_linked_cas_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'cas'
            source.mkdir()
            with self.assertRaisesRegex(ValueError, 'empty'):
                self.cache.pack_snapshot(source, root / 'snapshot.tar.gz')
            (root / 'outside').write_text('outside')
            (source / 'linked').symlink_to(root / 'outside')
            with self.assertRaisesRegex(ValueError, 'regular files and directories'):
                self.cache.pack_snapshot(source, root / 'snapshot.tar.gz')

    def archive(self, path, member):
        with tarfile.open(path, 'w:gz') as archive:
            root = tarfile.TarInfo('cas')
            root.type = tarfile.DIRTYPE
            archive.addfile(root)
            archive.addfile(member, io.BytesIO(b'x' * member.size) if member.isfile() else None)

    def test_unsafe_archives_never_install_a_cas_or_escape_staging(self):
        names = ['outside/file', 'cas/../../escaped', '/absolute', 'cas/../cas/traversal', 'cas\\windows-path']
        for name in names:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                archive = root / 'snapshot.tar.gz'
                member = tarfile.TarInfo(name)
                member.size = 1
                self.archive(archive, member)
                with self.assertRaises(ValueError):
                    self.cache.unpack_snapshot(archive, root / 'restored')
                self.assertFalse((root / 'restored').exists())
                self.assertFalse((root / 'escaped').exists())

    def test_devices_fifos_and_links_are_refused(self):
        for member_type in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE, tarfile.BLKTYPE):
            with self.subTest(member_type=member_type), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                archive = root / 'snapshot.tar.gz'
                member = tarfile.TarInfo('cas/entry')
                member.type = member_type
                member.linkname = '../../outside'
                self.archive(archive, member)
                with self.assertRaisesRegex(ValueError, 'regular files and directories'):
                    self.cache.unpack_snapshot(archive, root / 'restored')
                self.assertFalse((root / 'restored').exists())

    def test_empty_or_wrong_root_archives_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'snapshot.tar.gz'
            for entries in ([], [tarfile.TarInfo('cas')], [tarfile.TarInfo('wrong-root')]):
                with self.subTest(entries=[entry.name for entry in entries]):
                    with tarfile.open(archive, 'w:gz') as file:
                        for entry in entries:
                            entry.type = tarfile.DIRTYPE
                            file.addfile(entry)
                    with self.assertRaises(ValueError):
                        self.cache.unpack_snapshot(archive, root / 'restored')

    def test_duplicate_archive_paths_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'snapshot.tar.gz'
            with tarfile.open(archive, 'w:gz') as file:
                folder = tarfile.TarInfo('cas')
                folder.type = tarfile.DIRTYPE
                file.addfile(folder)
                for _ in range(2):
                    member = tarfile.TarInfo('cas/object')
                    member.size = 1
                    file.addfile(member, io.BytesIO(b'x'))
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                self.cache.unpack_snapshot(archive, root / 'restored')

    def test_warm_restore_cannot_mix_with_existing_local_objects(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.mkdir()
            (source / 'object').write_bytes(b'compiled')
            archive = root / 'snapshot.tar.gz'
            self.cache.pack_snapshot(source, archive)
            destination = root / 'destination'
            destination.mkdir()
            (destination / 'existing').write_bytes(b'existing')
            with self.assertRaisesRegex(ValueError, 'empty'):
                self.cache.unpack_snapshot(archive, destination)
            self.assertEqual((destination / 'existing').read_bytes(), b'existing')


if __name__ == '__main__':
    unittest.main()
