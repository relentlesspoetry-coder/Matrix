"""Validate CI isolation wiring without creating accounts or invoking sudo."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('find_test_binary', ROOT/'scripts/find-test-binary.py')
FINDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FINDER)


class ProcessAccountTests(unittest.TestCase):
    def test_cargo_artifact_is_selected_by_source_kind_and_test_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root/'lib.rs'
            binary = root/'backend-tests'
            binary.write_bytes(b'disposable path-selection fixture')
            artifact = {'reason':'compiler-artifact', 'profile':{'test':True},
                        'target':{'kind':['lib'], 'src_path':str(source)}, 'executable':str(binary)}
            other = {**artifact, 'target':{'kind':['bin'], 'src_path':str(root/'main.rs')}}
            lines = [json.dumps(other), json.dumps(artifact)]
            self.assertEqual(FINDER.find_binary(lines, 'lib', source), binary)
            with self.assertRaisesRegex(ValueError, 'found 0'):
                FINDER.find_binary([json.dumps(other)], 'lib', source)
            second = root/'second-tests'
            second.write_bytes(b'not executed')
            with self.assertRaisesRegex(ValueError, 'found 2'):
                FINDER.find_binary(lines+[json.dumps({**artifact,'executable':str(second)})], 'lib', source)

    def run_wrapper(self, root, *, target_uid='20001', runner_uid='1000', command=('printf','literal value; not shell code')):
        bindir = root/'bin'
        bindir.mkdir(exist_ok=True)
        fake_id = bindir/'id'
        fake_id.write_text('#!/bin/sh\nif [ "$#" = 2 ]; then printf "%s\\n" "$FAKE_TARGET_UID"; else printf "%s\\n" "$FAKE_RUNNER_UID"; fi\n')
        fake_id.chmod(0o755)
        fake_sudo = bindir/'sudo'
        fake_sudo.write_text('#!/usr/bin/python3\nimport json,os,sys\nopen(os.environ["MOCK_SUDO_ARGS"],"w").write(json.dumps(sys.argv[1:]))\n')
        fake_sudo.chmod(0o755)
        log = root/'args.json'
        env = os.environ.copy()
        env.update(PATH=str(bindir)+':/usr/bin:/bin', FAKE_TARGET_UID=target_uid,
                   FAKE_RUNNER_UID=runner_uid, MOCK_SUDO_ARGS=str(log),
                   GH_TOKEN='synthetic-must-not-forward', CARGO_HOME='/not/shared',
                   APPIMAGE_EXTRACT_AND_RUN='1')
        result = subprocess.run([str(ROOT/'scripts/run-ci-as-test-user.sh'), *command],
                                env=env, capture_output=True, text=True)
        return result, json.loads(log.read_text()) if log.exists() else None

    def test_wrapper_clears_environment_and_preserves_literal_arguments(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, args = self.run_wrapper(Path(tmp))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(args[:5], ['--user=matrix-test','--group=matrix-test','--','env','-i'])
            self.assertIn('HOME=/home/matrix-test',args)
            self.assertIn('APPIMAGE_EXTRACT_AND_RUN=1',args)
            self.assertFalse(any(a.startswith(('GH_TOKEN=','CARGO_HOME=')) for a in args))
            self.assertEqual(args[-3:], ['sh','printf','literal value; not shell code'])
            self.assertIn('cd /home/matrix-test/work; exec "$@"',args)

    def test_wrapper_rejects_root_or_runner_identity(self):
        for target in ['0','1000']:
            with self.subTest(target_uid=target), tempfile.TemporaryDirectory() as tmp:
                result, args = self.run_wrapper(Path(tmp), target_uid=target)
                self.assertNotEqual(result.returncode,0)
                self.assertIn('separate from the runner',result.stderr)
                self.assertIsNone(args)

    def test_native_ci_uses_prebuilt_harness_without_shared_caches(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        self.assertIn('useradd --create-home --user-group',workflow)
        self.assertIn('--coordinator-tests /home/matrix-test/bin/backend-tests',workflow)
        self.assertNotIn('chmod -R',workflow)
        self.assertNotIn('--preserve-env',workflow)
        native = (ROOT/'tests/native_update_lifecycle.py').read_text()
        self.assertIn('if coordinator_tests is not None:',native)
        self.assertIn('backend::update_lifecycle::tests::native_coordinator_fixture',native)

    def test_packaged_helpers_are_built_before_direct_desktop_tests(self):
        config = json.loads((ROOT/'src-tauri/tauri.conf.json').read_text())
        prepare = config['build']['beforeBuildCommand']
        self.assertEqual(config['build']['beforeDevCommand'], prepare)
        self.assertEqual(prepare, 'cargo build --locked --release --manifest-path src-tauri/Cargo.toml '
                         '--no-default-features --bin matrix-install-update --bin matrix-update-coordinator')
        for path, desktop in [
            ('.github/workflows/linux.yml', 'cargo test --locked --manifest-path src-tauri/Cargo.toml --bin matrix-frontend'),
            ('scripts/build-native.sh', 'cargo test --locked --manifest-path src-tauri/Cargo.toml -- --test-threads=1'),
        ]:
            with self.subTest(path=path):
                source = (ROOT/path).read_text()
                self.assertLess(source.index(prepare), source.index(desktop))
        native_script = (ROOT/'scripts/build-native.sh').read_text()
        self.assertIn('cargo test --locked --manifest-path src-tauri/Cargo.toml '
                      '--no-default-features --lib -- --test-threads=1', native_script)

    def test_root_staging_gate_runs_only_the_disposable_exact_fixture(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        step = workflow.split('- name: Verify private root-owned update staging\n', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('lib update-core/src/lib.rs', step)
        self.assertIn('sudo mktemp -d /var/tmp/matrix-staging-regression.XXXXXX', step)
        self.assertIn('sudo install -o root -g root -m 0755', step)
        self.assertIn('sudo env -i PATH=/usr/bin:/bin HOME=/root TMPDIR=', step)
        self.assertIn('--exact deb::tests::staging_creation_is_private_under_common_umasks --nocapture', step)
        self.assertIn('trap ', step)
        self.assertNotIn('continue-on-error', step)
        self.assertNotIn('||', step)

    def test_native_lifecycle_checks_require_two_passes_with_distinct_diagnostics(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        for name, folder in [('Test actual desktop window', 'native-$iteration'),
                             ('Test extracted AppImage shutdown', 'native-appimage-$iteration')]:
            step = workflow.split(f'- name: {name}\n', 1)[1].split('\n      - ', 1)[0]
            self.assertIn('for iteration in 1 2; do', step)
            self.assertIn(f'--screenshots "/home/matrix-test/artifacts/{folder}"', step)
            self.assertNotIn('continue-on-error', step)
            self.assertNotIn('||', step)

    def test_signed_deb_gate_is_isolated_and_required(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        step = workflow.split('- name: Verify isolated signed DEB upgrades\n', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('--build-only --source-root', step)
        self.assertIn('for mask in 000 002 022 077', step)
        self.assertIn('for scenario in tamper downgrade', step)
        self.assertIn('--seed-deb', step)
        self.assertIn('sudo mktemp -d /var/tmp/matrix-deb-integration.XXXXXX', step)
        self.assertIn('output="$integration_root/valid-$mask"', step)
        self.assertIn("trap 'sudo rm -rf --", step)
        self.assertIn('sudo env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root', step)
        self.assertNotIn('continue-on-error', step)
        self.assertNotIn('||', step)
        harness = (ROOT/'tests/signed_deb_integration.py').read_text()
        self.assertIn("['unshare','--user','--map-root-user','--mount','--net'", harness)
        self.assertIn('Host package database changed', harness)
        self.assertIn('matrix_update_core::deb::install_from_stdin', harness)
        self.assertIn('Staging leaked after installer exit', harness)

    def test_new_cover_provider_has_a_required_live_gate(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        step = workflow.split('- name: Verify live Libretro cover templates\n', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('live_libretro_templates -- --ignored', step)
        self.assertNotIn('continue-on-error', step)
        self.assertNotIn('||', step)
        self.assertLess(workflow.index('- name: Verify live Libretro cover templates'), workflow.index('- name: Build Linux installers'))


    def test_switch_cover_provider_has_a_required_live_gate(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        step = workflow.split('- name: Verify live Switch GameTDB cover template\n', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('live_switch_gametdb_template -- --ignored', step)
        self.assertNotIn('continue-on-error', step)
        self.assertNotIn('||', step)
        self.assertLess(workflow.index('- name: Verify live Switch GameTDB cover template'), workflow.index('- name: Build Linux installers'))


    def test_native_cover_downloads_are_exercised_online(self):
        workflow = (ROOT/'.github/workflows/linux.yml').read_text()
        step = workflow.split('- name: Test actual desktop window\n', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('tests/test_native.py /home/matrix-test/bin/matrix-frontend --online --coordinator-tests', step)
        self.assertIn('for iteration in 1 2', step)
        self.assertNotIn('continue-on-error', step)
        self.assertNotIn('||', step)


if __name__ == '__main__':
    unittest.main()
