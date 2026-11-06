"""Check the actual painted X11 window without enabling WebDriver automation.

Run in its own virtual display: xvfb-run -a python3 tests/test_normal_startup.py BINARY
Requires Pillow with XCB support (Ubuntu package python3-pil).
"""
import argparse
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
from PIL import ImageGrab


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('binary',type=Path)
    parser.add_argument('--screenshot',type=Path)
    cfg=parser.parse_args()
    binary=cfg.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='matrix-painted-window-') as tmp:
        home=Path(tmp)
        env=os.environ.copy()
        env.update(HOME=tmp,XDG_CONFIG_HOME=tmp+'/config',XDG_DATA_HOME=tmp+'/data',XDG_CACHE_HOME=tmp+'/cache',GDK_BACKEND='x11')
        env.pop('WAYLAND_DISPLAY',None)
        env.pop('TAURI_WEBVIEW_AUTOMATION',None)
        # Do not inject graphics workarounds: test a normal executable launch.
        logfile=home/'process.log'
        with logfile.open('w') as log:
            proc=subprocess.Popen([str(binary)],env=env,cwd=home,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            try:
                deadline=time.monotonic()+25
                while time.monotonic()<deadline:
                    if proc.poll() is not None:
                        raise AssertionError('Matrix exited before startup: '+logfile.read_text())
                    if 'Interface ready' in logfile.read_text() and len(list((home/'Games/Matrix Play').glob('*/* Games')))==18:break
                    time.sleep(.1)
                else:raise AssertionError('Interface never became ready: '+logfile.read_text())
                assert len(list((home/'Games/Matrix Play').glob('*/* Games')))==18
                deadline=time.monotonic()+15
                while time.monotonic()<deadline:
                    screenshot=ImageGrab.grab(xdisplay=os.environ['DISPLAY'])
                    if cfg.screenshot:
                        cfg.screenshot.parent.mkdir(parents=True,exist_ok=True)
                        screenshot.save(cfg.screenshot)
                    # Check what the display actually painted, allowing for a busy renderer.
                    if screenshot.getcolors(1000) is None:break
                    time.sleep(.25)
                else:raise AssertionError('The actual desktop window appears blank: '+logfile.read_text())
                assert (home/'data/matrix/startup.log').read_text().count('Interface ready')>=1
                print('PASS: ordinary non-WebDriver startup, visible painted interface, startup diagnostics, 18 console libraries')
            finally:
                try:os.killpg(proc.pid,signal.SIGTERM)
                except ProcessLookupError:pass
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid,signal.SIGKILL);proc.wait()
                time.sleep(.5)


if __name__=='__main__':main()
