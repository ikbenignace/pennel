import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / 'ci-rustup.sh'


class RustupEnvironmentTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.home = self.root / 'home'
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.shared_cargo = self.home / '.cargo'
        (self.shared_cargo / 'bin').mkdir(parents=True)
        tool = f'''#!{sys.executable}
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
with open(os.environ['TOOL_CALLS'], 'a') as log:
    log.write(json.dumps({{'tool': name, 'args': sys.argv[1:], 'cargo': os.environ.get('CARGO_HOME'), 'rustup': os.environ.get('RUSTUP_HOME')}}) + '\\n')
if name == 'rustup' and sys.argv[1:3] == ['which', 'rustc']:
    print(os.environ['FAKE_RUSTC'])
elif name == 'rustup' and sys.argv[1:3] == ['toolchain', 'install'] and os.environ.get('FAIL_FIRST_INSTALL') == '1':
    marker = pathlib.Path(os.environ['INSTALL_MARKER'])
    if not marker.exists():
        marker.touch()
        sys.exit(1)
elif name == 'rustc':
    print('rustc 1.99.1 (mock)')
'''
        for path in (self.bin / 'rustup', self.bin / 'rustc', self.bin / 'flock',
                     self.shared_cargo / 'bin/rustup', self.shared_cargo / 'bin/rustc'):
            path.write_text(tool)
            path.chmod(0o755)

    def run_helper(self, runner_name, runner_environment='github-hosted', **overrides):
        calls = self.root / f'calls-{runner_name}.jsonl'
        environment = dict(os.environ)
        for key in ('CARGO_HOME', 'RUSTUP_HOME', 'GITHUB_ENV', 'GITHUB_PATH', 'GITHUB_WORKSPACE',
                    'RUNNER_NAME', 'RUNNER_ENVIRONMENT', 'FAIL_FIRST_INSTALL'):
            environment.pop(key, None)
        environment.update({
            'HOME': str(self.home),
            'PATH': str(self.bin) + os.pathsep + environment['PATH'],
            'RUNNER_NAME': runner_name,
            'RUNNER_ENVIRONMENT': runner_environment,
            'GITHUB_ENV': str(self.root / f'env-{runner_name}'),
            'GITHUB_PATH': str(self.root / f'path-{runner_name}'),
            'TOOL_CALLS': str(calls),
            'FAKE_RUSTC': str(self.bin / 'rustc'),
            'INSTALL_MARKER': str(self.root / f'install-{runner_name}'),
        })
        environment.update(overrides)
        result = subprocess.run(['bash', str(SCRIPT), 'clippy', 'rustfmt'], env=environment,
                                text=True, capture_output=True, check=True)
        exported = dict(line.split('=', 1) for line in Path(environment['GITHUB_ENV']).read_text().splitlines())
        return exported, [json.loads(line) for line in calls.read_text().splitlines()], result

    def test_hosted_runner_names_share_stable_cache_paths(self):
        first, first_calls, _ = self.run_helper('GitHub Actions 1001')
        second, second_calls, _ = self.run_helper('GitHub Actions 1002')
        self.assertEqual(first, second)
        self.assertEqual(first['RUSTUP_HOME'], str(self.home / '.rustup'))
        self.assertEqual(first['CARGO_HOME'], str(self.shared_cargo))
        self.assertTrue((self.home / '.rustup/pennel-ci.lock').is_file())
        for calls in (first_calls, second_calls):
            self.assertIn(['-w', '300', '9'], [call['args'] for call in calls if call['tool'] == 'flock'])
            self.assertIn(['toolchain', 'install', 'stable', '--profile', 'minimal', '--no-self-update',
                           '--component', 'clippy', '--component', 'rustfmt'],
                          [call['args'] for call in calls if call['tool'] == 'rustup'])

    def test_self_hosted_runners_keep_separate_homes_and_locks(self):
        first, _, _ = self.run_helper('runner-1', 'self-hosted')
        second, _, _ = self.run_helper('runner-2', 'self-hosted')
        self.assertNotEqual(first['RUSTUP_HOME'], second['RUSTUP_HOME'])
        self.assertNotEqual(first['CARGO_HOME'], second['CARGO_HOME'])
        for exported in (first, second):
            self.assertTrue(Path(exported['RUSTUP_HOME'], 'pennel-ci.lock').is_file())
            self.assertTrue(Path(exported['CARGO_HOME'], 'bin/rustup').is_symlink())

    def test_explicit_hosted_homes_are_preserved(self):
        custom_cargo = str(self.root / 'configured-cargo')
        custom_rustup = str(self.root / 'configured-rustup')
        exported, _, _ = self.run_helper('hosted', CARGO_HOME=custom_cargo, RUSTUP_HOME=custom_rustup)
        self.assertEqual(exported['CARGO_HOME'], custom_cargo)
        self.assertEqual(exported['RUSTUP_HOME'], custom_rustup)
        self.assertTrue(Path(custom_rustup, 'pennel-ci.lock').is_file())

    def test_self_hosted_runner_install_directories_remain_isolated(self):
        exported = []
        for runner in ('actions-runner-1', 'actions-runner-2'):
            workspace = self.root / runner / '_work/pennel/pennel'
            workspace.mkdir(parents=True)
            homes, _, _ = self.run_helper('shared-name', 'self-hosted', GITHUB_WORKSPACE=str(workspace))
            exported.append(homes)
        self.assertNotEqual(exported[0]['RUSTUP_HOME'], exported[1]['RUSTUP_HOME'])
        self.assertNotEqual(exported[0]['CARGO_HOME'], exported[1]['CARGO_HOME'])

    def test_explicit_self_hosted_rustup_home_is_preserved(self):
        custom_rustup = str(self.root / 'configured-rustup')
        exported, _, _ = self.run_helper('runner-1', 'self-hosted', RUSTUP_HOME=custom_rustup)
        self.assertEqual(exported['RUSTUP_HOME'], custom_rustup)
        self.assertNotEqual(exported['CARGO_HOME'], str(self.shared_cargo))

    def test_failed_install_repairs_only_the_selected_toolchain_home(self):
        selected = self.home / '.rustup/runners/runner-1'
        other = self.home / '.rustup/runners/runner-2/toolchains/stable-safe'
        old = selected / 'toolchains/stable-broken'
        old.mkdir(parents=True)
        other.mkdir(parents=True)
        exported, calls, result = self.run_helper('runner-1', 'self-hosted', FAIL_FIRST_INSTALL='1')
        self.assertEqual(exported['RUSTUP_HOME'], str(selected))
        self.assertFalse(old.exists())
        self.assertTrue(other.is_dir())
        self.assertIn('wiping this runner', result.stderr)
        installs = [call for call in calls if call['tool'] == 'rustup' and call['args'][:2] == ['toolchain', 'install']]
        self.assertEqual(len(installs), 2)
        self.assertTrue(all(call['rustup'] == str(selected) for call in installs))


if __name__ == '__main__':
    unittest.main()
