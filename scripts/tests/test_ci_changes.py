import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / 'ci-changes.py'


class ChangesTests(unittest.TestCase):
    def classify(self, *paths):
        spec = importlib.util.spec_from_file_location('ci_changes', SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return {key for key, enabled in module.classify(paths).items() if enabled}

    def test_non_pr_runs_everything(self):
        output = subprocess.check_output(['python3', str(SCRIPT)], text=True)
        self.assertEqual(set(output.splitlines()), {
            'core=true', 'legacy=true', 'platform=true', 'app=true', 'packaging=true',
        })

    def test_empty_diff_skips_builds(self):
        self.assertEqual(self.classify(), set())

    def test_docs_do_not_build(self):
        self.assertEqual(self.classify('README.md', 'docs/desktop.md', 'AGENTS.md'), set())

    def test_frontend_only_builds_desktop(self):
        self.assertEqual(self.classify('apps/desktop/src/main.ts'), {'app'})

    def test_bundle_config_runs_packaging_checks(self):
        self.assertEqual(self.classify('apps/desktop/src-tauri/tauri.conf.json'),
                         {'app', 'packaging'})

    def test_core_builds_all_consumers(self):
        self.assertEqual(self.classify('core/src/lib.rs'), {'core', 'legacy', 'app'})

    def test_speaker_changes_build_all_consumers(self):
        self.assertEqual(self.classify('core/speakers/Cargo.lock'), {'core', 'legacy', 'app'})

    def test_platform_library_builds_swift_tests_and_desktop(self):
        self.assertEqual(self.classify('apps/macos-platform/Package.swift'), {'platform', 'app'})

    def test_legacy_only_builds_legacy(self):
        self.assertEqual(self.classify('apps/macos/Sources/Pennel/App.swift'), {'legacy'})

    def test_shared_assets_build_both_apps(self):
        self.assertEqual(self.classify('assets/brand/tokens.json'), {'legacy', 'app'})

    def test_translation_catalogs_validate_core_and_both_apps(self):
        self.assertEqual(self.classify('assets/locales/onboarding.en.json'),
                         {'core', 'legacy', 'app'})

    def test_build_infrastructure_runs_everything(self):
        for path in ('Makefile', '.cargo/config.toml', '.github/workflows/ci.yml',
                     'scripts/generate-theme.py', 'scripts/tests/test_ci_changes.py'):
            with self.subTest(path=path):
                self.assertEqual(self.classify(path), {'core', 'legacy', 'platform', 'app', 'packaging'})

    def test_mixed_changes_union_jobs(self):
        self.assertEqual(self.classify('README.md', 'apps/desktop/package-lock.json',
                                       'core/src/lib.rs'), {'core', 'legacy', 'app'})

    def test_unknown_inputs_fail_open(self):
        self.assertEqual(self.classify('new-build-config.toml'),
                         {'core', 'legacy', 'platform', 'app', 'packaging'})

    def test_git_diff_handles_deletions_renames_and_branch_base(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            def git(*args):
                return subprocess.check_output(['git', '-C', directory, *args], text=True).strip()
            git('init', '-q')
            git('config', 'user.email', 'ci@example.com')
            git('config', 'user.name', 'CI test')
            (repo / 'core').mkdir()
            (repo / 'core/input.rs').write_text('input')
            git('add', '.')
            git('commit', '-qm', 'base')
            base = git('rev-parse', 'HEAD')
            git('checkout', '-qb', 'topic')
            git('mv', 'core/input.rs', 'README.md')
            git('commit', '-qm', 'rename code to docs')
            head = git('rev-parse', 'HEAD')
            git('checkout', '-qb', 'base-advanced', base)
            (repo / 'unrelated.toml').write_text('base-only change')
            git('add', '.')
            git('commit', '-qm', 'advance base')
            advanced_base = git('rev-parse', 'HEAD')
            output = subprocess.check_output([
                'python3', str(SCRIPT), '--base', advanced_base, '--head', head,
            ], cwd=repo, text=True)
            self.assertEqual(set(output.splitlines()),
                             {'core=true', 'legacy=true', 'app=true', 'platform=false', 'packaging=false'})


if __name__ == '__main__':
    unittest.main()
