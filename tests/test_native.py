"""Exercise a compiled Tauri Linux window using its actual IPC and filesystem.

Run under xvfb-run; install tauri-driver and webkit2gtk-driver first.
The launcher fixtures are harmless scripts, not real emulators, Wine or Steam.
Use --online to include two real cover downloads; all game data is synthetic.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def wait_for(fn, timeout=30):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = fn()
            if value:
                return value
        except (OSError, AssertionError, RuntimeError) as e:
            last = e
        time.sleep(.15)
    raise AssertionError(f'Timed out: {last}')


def main():
    args = argparse.ArgumentParser()
    args.add_argument('binary', type=Path)
    args.add_argument('--driver', default='tauri-driver')
    args.add_argument('--webkit-driver', default='WebKitWebDriver')
    args.add_argument('--online', action='store_true')
    args.add_argument('--screenshots', type=Path)
    cfg = args.parse_args()
    binary = cfg.binary.resolve(strict=True)
    endpoint = f'http://127.0.0.1:{port()}'
    session = None
    with tempfile.TemporaryDirectory(prefix='matrix-native-test-') as tmp:
        home = Path(tmp)
        library = home / 'Games/Matrix'
        ps2 = library / 'PS2/PS2 Games'
        ps1 = library / 'PS1/PS1 Games'
        for path in [ps1, ps2, home/'config/matrix', home/'bin', home/'.local/share/Steam/steamapps']:
            path.mkdir(parents=True, exist_ok=True)
        ps2game = ps2 / 'Test Game [SLUS-20312].chd'
        ps1game = ps1 / 'Test Game [SLUS-00664].chd'
        ps2game.write_text('Synthetic fixture; not a playable ROM.')
        ps1game.write_text('Synthetic fixture; not a playable ROM.')
        (home/'config/matrix/frontend.json').write_text(json.dumps({'artwork': {'enabled': cfg.online, 'style': 'default'}}))
        manifest = home/'.local/share/Steam/steamapps/appmanifest_12345.acf'
        manifest.write_text('"AppState"\n{\n"appid" "12345"\n"name" "Native Steam Fixture"\n}\n')
        launcher = '#!/bin/sh\nprintf "%s\\n" "$@" > "$MATRIX_NATIVE_TEST_LAUNCH_LOG"\n'
        for target in [library/'PS2/TestEmulator.AppImage', home/'bin/steam', home/'bin/wine']:
            target.write_text(launcher)
            target.chmod(0o755)
        exe = home/'Windows Fixture.exe'
        exe.write_text('Synthetic registration fixture.')
        log = home/'launch.log'
        env = os.environ.copy()
        env.update(HOME=str(home), XDG_CONFIG_HOME=str(home/'config'), XDG_DATA_HOME=str(home/'.local/share'),
                   XDG_CACHE_HOME=str(home/'cache'), XDG_STATE_HOME=str(home/'state'),
                   PATH=str(home/'bin')+':'+env['PATH'], GDK_BACKEND='x11',
                   MATRIX_NATIVE_TEST_LAUNCH_LOG=str(log), WEBKIT_DISABLE_DMABUF_RENDERER='1')
        env.pop('WAYLAND_DISPLAY', None)
        driver_log = open(home/'driver.log', 'w+')
        driver = subprocess.Popen([cfg.driver, '--port', endpoint.rsplit(':',1)[1], '--native-port', str(port()),
                                   '--native-driver', cfg.webkit_driver], env=env, stdout=driver_log, stderr=subprocess.STDOUT)

        def request(method, path, data=None):
            req = urllib.request.Request(endpoint+path, data=None if data is None else json.dumps(data).encode(),
                                         method=method, headers={'Content-Type':'application/json'})
            try:
                with urllib.request.urlopen(req, timeout=75) as response:
                    body = json.load(response)
            except urllib.error.HTTPError as e:
                raise RuntimeError(e.read().decode()) from e
            value = body.get('value')
            if isinstance(value, dict) and value.get('error'):
                raise RuntimeError(value)
            return value

        def js(script, *values):
            return request('POST', f'/session/{session}/execute/sync', {'script':script, 'args':list(values)})

        def invoke(name, values=None):
            result = request('POST', f'/session/{session}/execute/async', {'script': '''
                const done=arguments[arguments.length-1];
                window.__TAURI__.core.invoke(arguments[0],arguments[1]).then(value=>done({value})).catch(error=>done({error:String(error)}));
            ''', 'args':[name, values or {}]})
            if 'error' in result:
                raise RuntimeError(result['error'])
            return result.get('value')

        def click(selector):
            js('document.querySelector(arguments[0]).click()', selector)

        def screenshot(name):
            if cfg.screenshots:
                cfg.screenshots.mkdir(parents=True, exist_ok=True)
                data=request('GET', f'/session/{session}/screenshot')
                (cfg.screenshots/name).write_bytes(base64.b64decode(data))

        try:
            wait_for(lambda: request('GET', '/status'))
            result=request('POST','/session',{'capabilities':{'alwaysMatch':{'tauri:options':{'application':str(binary)}}}})
            session=result['sessionId']
            request('POST', f'/session/{session}/timeouts', {'script':60000})
            wait_for(lambda: js('return document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX" && document.querySelectorAll(".console-card").length === 14'))
            assert len([p for p in library.iterdir() if p.is_dir()]) == 14
            assert len(invoke('scan_console', {'id':'ps1'})) == 1
            assert len(invoke('scan_console', {'id':'ps2'})) == 1
            assert len(invoke('scan_console', {'id':'ps3'})) == 0
            screenshot('matrix-linux-home.png')
            click('[data-action="console"][data-id="ps2"]')
            wait_for(lambda: js('return document.querySelectorAll(".game-item").length === 1'))
            if cfg.online:
                wait_for(lambda: invoke('get_cover', {'kind':'ps2','uid':str(ps2game)}), 60)
                wait_for(lambda: invoke('get_cover', {'kind':'ps1','uid':str(ps1game)}), 60)
                wait_for(lambda: js('return document.querySelector(".game-cover img")?.complete'))
            screenshot('matrix-linux-ps2.png')
            click('.game-card')
            wait_for(log.exists)
            assert log.read_text().splitlines() == [str(ps2game)]
            click('[data-action="game"]')
            wait_for(lambda: js('return Boolean(document.querySelector(".cover-tools"))'))
            # Native manual import round-trip and cross-console rejection.
            cover = Path(__file__).parent/'fixtures/demo-cover.png'
            invoke('save_cover', {'kind':'ps2','uid':str(ps2game),'bytes':list(cover.read_bytes())})
            assert invoke('get_cover', {'kind':'ps2','uid':str(ps2game)}).startswith('data:image/png;base64,')
            try:
                invoke('get_cover', {'kind':'ps1','uid':str(ps2game)})
                raise AssertionError('Cross-console path accepted')
            except RuntimeError as e:
                assert "outside" in str(e)
            click('[data-action="close"]')
            log.unlink()
            click('[data-action="steam"]')
            wait_for(lambda: js('return document.querySelectorAll(".game-item").length === 1'))
            click('.game-card')
            wait_for(log.exists)
            assert log.read_text().splitlines() == ['-applaunch', '12345']
            invoke('add_windows_game', {'title':'Native Windows Fixture','target':str(exe),'runner':'wine'})
            click('[data-action="refresh"]')
            click('[data-action="windows"]')
            wait_for(lambda: js('return document.querySelectorAll(".game-item").length === 1'))
            log.unlink()
            click('.game-card')
            wait_for(log.exists)
            assert log.read_text().splitlines() == [str(exe)]
            invoke('save_artwork_settings', {'enabled':False,'style':'3d'})
            boot=invoke('bootstrap')
            assert boot['artwork'] == {'enabled':False,'style':'3d'}
            assert boot['profiles']['ps2']['emulatorPath'].endswith('TestEmulator.AppImage')
            assert len(boot['windows']) == 1
            print('PASS: real Tauri/WebKit window, native IPC, 14 libraries, PS1/PS2 scanning, manual/cache artwork, console boundaries, persisted settings, three launcher argument paths')
            if cfg.online:
                print('PASS: real desktop automatic PS1 and PS2 cover downloads (synthetic serial-labelled game fixtures)')
        except Exception:
            driver_log.flush()
            print((home/'driver.log').read_text())
            raise
        finally:
            if session:
                try: request('DELETE', f'/session/{session}')
                except Exception: pass
            driver.terminate()
            try: driver.wait(timeout=10)
            except subprocess.TimeoutExpired: driver.kill();driver.wait()
            driver_log.close()

if __name__ == '__main__':
    main()
