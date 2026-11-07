"""Headless checks for native lifecycle fixtures; not a substitute for WebKit CI."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import native_update_lifecycle as lifecycle


class NativeLifecycleFixtureTests(unittest.TestCase):
    def test_writer_runs_in_a_fresh_home_without_emulator_configuration(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryFile() as output:
            home = Path(directory)
            writer, game = lifecycle.prepare_writer(home)
            self.assertTrue(game.is_file())
            self.assertEqual(writer.name, 'pcsx2')
            process = subprocess.Popen([str(writer), str(game)], env={**os.environ, 'HOME':str(home)},
                                       stdout=output, stderr=output)
            try:
                info = {'phase':'started', 'logPath':str(home/'not-written.log')}
                lifecycle.wait_for_writer(home, lambda *_: info, info, timeout=5)
                self.assertEqual(int((home/'writer.ready').read_text()), process.pid)
                self.assertIsNone(process.poll())
            finally:
                (home/'writer.release').touch()
                process.wait(timeout=5)
            self.assertEqual(process.returncode, 0)
            self.assertEqual((home/'writer.done').read_text(), 'survived')
            output.seek(0)
            data = output.read()
            self.assertGreater(len(data), 800000)
            self.assertTrue(data.endswith(b'late native writer diagnostic\n'))

    def test_initial_launch_failure_reports_phase_and_log_without_polling(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            log = home/'launch.log'
            log.write_text('synthetic interpreter failed to load')
            info = {'phase':'error', 'message':'launch failed', 'logPath':str(log)}
            with self.assertRaisesRegex(AssertionError, 'synthetic interpreter failed to load'):
                lifecycle.wait_for_writer(home, lambda *_: self.fail('must fail immediately'), info)
            self.assertEqual(json.loads((home/'lifecycle-launch-status.json').read_text()), info)

    def test_later_exit_and_readiness_timeout_keep_current_status(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            initial = {'phase':'started', 'message':'initial launch'}
            ended = {'phase':'exited', 'message':'ended without readiness'}
            with self.assertRaisesRegex(AssertionError, 'ended without readiness'):
                lifecycle.wait_for_writer(home, lambda *_: ended, initial)
            with self.assertRaisesRegex(AssertionError, 'readiness timed out.*initial launch'):
                lifecycle.wait_for_writer(home, lambda *_: initial, initial, timeout=0)

    def test_failure_before_second_window_still_releases_only_fixture_writer(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            with mock.patch.object(lifecycle, '_exercise', side_effect=RuntimeError('fixture error')):
                with self.assertRaisesRegex(RuntimeError, 'fixture error'):
                    lifecycle.exercise(None, {}, home, None, None, None)
            self.assertTrue((home/'writer.release').is_file())

    def test_diagnostics_are_bounded_and_survive_temporary_home_cleanup(self):
        with tempfile.TemporaryDirectory() as destination, tempfile.TemporaryDirectory() as outside:
            destination = Path(destination)
            external = Path(outside)/'private.log'
            external.write_text('must not copy')
            with tempfile.TemporaryDirectory() as directory:
                home = Path(directory)
                (home/'driver.log').write_bytes(b'x'*70000+b'recent driver failure')
                launches = home/'.local/share/matrix/launches'
                launches.mkdir(parents=True)
                (launches/'ps2-fixture.log').write_text('launch failure retained')
                (home/'leaked.log').symlink_to(external)
                (home/'lifecycle-launch-status.json').write_text('{"phase":"error"}')
                lifecycle.preserve_diagnostics(home, destination)
            self.assertEqual((destination/'driver.log').stat().st_size, 65536)
            self.assertTrue((destination/'driver.log').read_bytes().endswith(b'recent driver failure'))
            self.assertEqual((destination/'launches/ps2-fixture.log').read_text(), 'launch failure retained')
            self.assertFalse((destination/'leaked.log').exists())
            self.assertEqual(json.loads((destination/'lifecycle-launch-status.json').read_text())['phase'], 'error')


if __name__ == '__main__':
    unittest.main()
