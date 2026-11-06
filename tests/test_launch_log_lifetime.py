"""Compile the production sink module and exercise synthetic process lifetimes.

Requires rustc; missing compiler is reported as skipped, never a successful check.
This does not build the GUI, exercise launch IPC, or run an actual emulator.
"""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

MODULE = Path(__file__).resolve().parents[1] / 'src-tauri/src/launch_log.rs'
EMITTER = r'''
import os, pathlib, sys, time
ready, done = map(pathlib.Path, sys.argv[1:])
end = time.monotonic() + 5
while not ready.exists():
    if time.monotonic() > end:
        sys.exit(2)
    time.sleep(0.01)
for fd in (1, 2):
    data = b'x' * 400000
    while data:
        data = data[os.write(fd, data):]
os.write(2, b'late failure diagnostic\n')
done.write_text('survived')
'''
LAUNCHER = r'''
import subprocess, sys
helper, log, ready, done, emitter = sys.argv[1:]
with open(log, 'wb') as output:
    sink = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=output,
                            stderr=subprocess.DEVNULL, process_group=0)
    subprocess.Popen([sys.executable, '-c', emitter, ready, done],
                     stdout=sink.stdin, stderr=sink.stdin,
                     stdin=subprocess.DEVNULL)
    sink.stdin.close()
'''

@unittest.skipUnless(sys.platform == 'linux', 'Linux launch implementation')
class LaunchLogLifetimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = shutil.which('rustc')
        if not compiler:
            raise unittest.SkipTest('rustc unavailable; production sink not tested')
        cls.build = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.build.cleanup)
        root = Path(cls.build.name)
        source = root / 'main.rs'
        source.write_text('#[path = ' + json.dumps(str(MODULE)) + ']\n'
                          'mod launch_log;\n'
                          'fn main() { launch_log::run().unwrap(); }\n')
        cls.helper = str(root / 'log-sink')
        subprocess.run([compiler, '--edition=2021', str(source), '-o', cls.helper],
                       check=True, timeout=60, capture_output=True)

    def test_writers_survive_launcher_exit_with_recent_tail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log, ready, done = (root / name for name in ('launch.log', 'ready', 'done'))
            subprocess.run([sys.executable, '-c', LAUNCHER, self.helper, str(log),
                            str(ready), str(done), EMITTER], check=True, timeout=5,
                           stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            ready.touch()  # Launcher is already gone before writers can proceed.
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                data = log.read_bytes()
                if done.exists() and data.endswith(b'late failure diagnostic\n'):
                    self.assertEqual(len(data), 65536)
                    self.assertEqual(done.read_text(), 'survived')
                    return
                time.sleep(0.01)
            self.fail('writer blocked/failed or recent diagnostics were lost')

    def test_short_output_is_flushed_at_eof(self):
        with tempfile.TemporaryFile() as output:
            sink = subprocess.Popen([self.helper], stdin=subprocess.PIPE, stdout=output)
            sink.communicate(b'short failure\n', timeout=5)
            self.assertEqual(sink.returncode, 0)
            output.seek(0)
            self.assertEqual(output.read(), b'short failure\n')

    def test_log_write_failure_still_drains(self):
        with open('/dev/full', 'wb', buffering=0) as full:
            sink = subprocess.Popen([self.helper], stdin=subprocess.PIPE, stdout=full,
                                    stderr=subprocess.DEVNULL)
            sink.communicate(b'x' * 800000, timeout=5)
            self.assertEqual(sink.returncode, 0)

if __name__ == '__main__':
    unittest.main()
