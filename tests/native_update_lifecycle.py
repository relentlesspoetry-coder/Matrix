"""Native shutdown integration, using only disposable windows/writer processes.

The coordinator entry point is cfg(test)-only and writes a marker in place of
installation. Release binaries retain mandatory signed update verification.
"""
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def exercise(binary, env, home, invoke, js, wait_for, coordinator_tests=None):
    registry = Path(f'/tmp/matrix-play-lifecycle-{os.geteuid()}')
    appimage = binary.name.endswith('.AppImage')

    def live_pids():
        owned = []
        for path in registry.glob('instance-*'):
            if path.suffix:
                continue
            with path.open('rb') as record:
                try:
                    fcntl.flock(record, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    continue  # Stale records are never process authority.
                except BlockingIOError:
                    pass
                pid = json.load(record)['pid']
            # Refuse to send any shutdown request if an unrelated real app is
            # registered. Both its executable and disposable HOME must match.
            actual_exe = Path(f'/proc/{pid}/exe').resolve()
            environment = Path(f'/proc/{pid}/environ').read_bytes().split(b'\0')
            if appimage:
                variables = dict(pair.split(b'=',1) for pair in environment if b'=' in pair)
                assert Path(os.fsdecode(variables[b'APPIMAGE'])).resolve() == binary
                assert actual_exe.is_relative_to(Path(os.fsdecode(variables[b'APPDIR'])).resolve())
                assert actual_exe.name == 'matrix-frontend'
            else:
                assert actual_exe == binary
            assert f'HOME={home}'.encode() in environment, 'Unrelated Matrix instance; native fixture refuses shutdown'
            owned.append(pid)
        return sorted(owned)

    def coordinator(name, expected, busy=False, runtime_pids=()):
        output = home / f'coordinator-{name}.result'
        config = home / f'coordinator-{name}.json'
        frontend = Path(f'/proc/{expected[0]}/exe').resolve()
        config.write_text(json.dumps({'binary': str(frontend), 'appimage': str(binary) if appimage else None, 'output': str(output), 'pids': expected, 'runtime_pids': list(runtime_pids), 'busy': busy}))
        helper_env = os.environ.copy()
        helper_env['MATRIX_NATIVE_COORDINATOR_CONFIG'] = str(config)
        # The same production Transaction is exercised through a test-only Rust
        # entry point. This command never downloads or installs a package.
        if coordinator_tests is not None:
            # Built under the runner, copied privately, then executed as this
            # isolated UID. No toolchain/cache permission changes are needed.
            command = [str(coordinator_tests.resolve(strict=True)), '--exact',
                       'backend::update_lifecycle::tests::native_coordinator_fixture',
                       '--nocapture', '--test-threads=1']
        else:
            command = ['cargo', 'test', '--locked', '--manifest-path', str(ROOT/'src-tauri/Cargo.toml'),
                       '--no-default-features', '--lib', 'native_coordinator_fixture', '--', '--nocapture', '--test-threads=1']
        log = (home/f'coordinator-{name}.log').open('w+')
        process = subprocess.Popen(command, env=helper_env, stdout=log, stderr=subprocess.STDOUT)
        return process, output, log

    def finish(process, logfile):
        status = process.wait(timeout=45)
        logfile.flush()
        logfile.seek(0)
        output = logfile.read()
        logfile.close()
        assert status == 0, output

    pids = live_pids()
    assert len(pids) == 1, pids
    writer = home/'bin/pcsx2'
    writer.write_text('''#!/usr/bin/env python3
import os, pathlib, time
root=pathlib.Path.home()
(root/'writer.ready').write_text(str(os.getpid()))
end=time.monotonic()+30
while not (root/'writer.release').exists():
    if time.monotonic()>end: raise RuntimeError('fixture deadline')
    time.sleep(.01)
for fd in [1,2]:
    data=b'x'*400000
    while data: data=data[os.write(fd,data):]
os.write(2,b'late native writer diagnostic\\n')
(root/'writer.done').write_text('survived')
''')
    writer.chmod(0o755)
    invoke('save_emulator', {'id':'ps2','emulatorName':'PCSX2 fixture','emulatorPath':str(writer),
                             'launcher':'executable','launchArgs':['{game}']})
    game = home/'Games/Matrix/PS2/PS2 Games/Test Game [SLUS-20312].chd'
    info = invoke('launch_console_game', {'id':'ps2','uid':str(game)})
    wait_for(lambda: (home/'writer.ready').exists())
    writer_pid = int((home/'writer.ready').read_text())
    app = coordinator('busy', pids, True)
    finish(app[0], app[2])
    assert app[1].read_text() == 'blocked-without-shutdown'
    assert live_pids() == pids and Path(f'/proc/{writer_pid}').exists()
    assert js('return Boolean(document.querySelector(".app-shell"))')
    (home/'writer.release').touch()
    wait_for(lambda: (home/'writer.done').exists())
    wait_for(lambda: Path(info['logPath']).read_bytes().endswith(b'late native writer diagnostic\n'))
    assert Path(info['logPath']).stat().st_size == 65536
    # Actual launcher and sink must release their guards before coordination.
    def activity_finished():
        with (registry/'activity.lock').open('rb') as guard:
            try:
                fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return True
            except BlockingIOError:
                return False
    wait_for(activity_finished)
    saved = (home/'config/matrix/frontend.json').read_bytes()
    game_bytes = game.read_bytes()
    covers = {p.relative_to(home):p.read_bytes() for p in (home/'.local/share/matrix/covers').rglob('*') if p.is_file()}
    second_log = (home/'second-matrix.log').open('w+')
    second = subprocess.Popen([str(binary)], env=env, cwd=home, stdout=second_log, stderr=subprocess.STDOUT)
    # Unrelated harmless writer process must remain unaffected by coordination.
    unrelated = subprocess.Popen([sys.executable, '-c', 'import sys; sys.stdin.buffer.read(1)'], stdin=subprocess.PIPE)
    try:
        wait_for(lambda: 'Interface ready' in (home/'second-matrix.log').read_text())
        expected = live_pids()
        assert len(expected) == 2
        assert appimage or second.pid in expected
        process, output, logfile = coordinator('shutdown', expected, runtime_pids=[second.pid] if appimage else [])
        wait_for(lambda: output.with_suffix('.ready').exists())
        with (home/'gated-start.log').open('w+') as startup_log:
            newcomer = subprocess.Popen([str(binary)], env=env, cwd=home, stdout=startup_log, stderr=subprocess.STDOUT)
            assert newcomer.wait(timeout=30) == 0
        assert 'update is in progress' in (home/'gated-start.log').read_text()
        assert live_pids() == expected
        assert not output.exists(), 'Replacement boundary must not be reached while either window runs'
        output.with_suffix('.proceed').touch()
        finish(process, logfile)
        assert output.read_text() == 'replacement-allowed-after-actual-exit'
        assert not live_pids()
        assert second.wait(timeout=10) == 0
        assert unrelated.poll() is None, 'Unrelated processes must not be terminated'
        assert (home/'config/matrix/frontend.json').read_bytes() == saved
        assert game.read_bytes() == game_bytes
        assert all((home/path).read_bytes() == data for path,data in covers.items())
        print('PASS: real Tauri shutdown of two windows before simulated replacement, blocked new startup, active writer/log protection, unrelated process and data preservation; no installation performed')
    finally:
        (home/'writer.release').touch()
        unrelated.stdin.close()
        unrelated.wait(timeout=5)
        if second.poll() is None:
            # Only this test's exact spawned app, never names/groups/user apps.
            second.terminate()
            second.wait(timeout=10)
        second_log.close()
