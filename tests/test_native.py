"""Exercise a compiled Tauri Linux window using its actual IPC and filesystem.

Run under xvfb-run; install tauri-driver and webkit2gtk-driver first.
The launcher fixtures are harmless scripts, not real emulators or Steam.
Use --online to include two real cover downloads; all game data is synthetic.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import signal
import socket
import shutil
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
    args.add_argument('--check-updates', action='store_true', help='Check the real public GitHub updater (network required)')
    args.add_argument('--download-update', action='store_true', help='Download and verify a newer release through the actual UI; never install it')
    args.add_argument('--unset-parent-cache', action='store_true')
    args.add_argument('--backend', choices=['session','x11','wayland'], default='session')
    args.add_argument('--legacy-graphics-workaround', action='store_true', help='Reproduce the compatibility settings used in the original test')
    args.add_argument('--screenshots', type=Path)
    cfg = args.parse_args()
    binary = cfg.binary.resolve(strict=True)
    cfg.driver = shutil.which(cfg.driver) or str(Path(cfg.driver).resolve(strict=True))
    cfg.webkit_driver = shutil.which(cfg.webkit_driver) or str(Path(cfg.webkit_driver).resolve(strict=True))
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
        legacy = {'uid':str(home/'Windows Fixture.exe'),'title':'Legacy Windows Fixture','target':str(home/'Windows Fixture.exe'),'runner':'retired-runtime'}
        (home/'config/matrix/frontend.json').write_text(json.dumps({'artwork': {'enabled': cfg.online, 'style': 'default'}, 'windows':[legacy]}))
        manifest = home/'.local/share/Steam/steamapps/appmanifest_12345.acf'
        manifest.write_text('"AppState"\n{\n"appid" "12345"\n"name" "Native Steam Fixture"\n}\n')
        launcher = '#!/bin/sh\nprintf "%s\\n" "$@" > "$MATRIX_NATIVE_TEST_LAUNCH_LOG"\n'
        launcher += 'printf "%s" "$XDG_CACHE_HOME" > "$MATRIX_NATIVE_TEST_CACHE_LOG"\n'
        for target in [library/'PS2/TestEmulator.AppImage', home/'bin/steam', home/'bin/gio']:
            target.write_text(launcher)
            target.chmod(0o755)
        flatpak=home/'bin/flatpak'
        flatpak.write_text('#!/bin/sh\nif [ "$1" = "list" ]; then\n  printf "org.matrix.TestEmulator\\tMatrix Test Emulator\\tuser\\norg.matrix.TestEmulator\\tMatrix Test Emulator\\tsystem\\ncom.github.Rosalie241.RMG\\tRMG\\tsystem\\n"\n  exit 0\nfi\n'+launcher.split('\n',1)[1])
        flatpak.chmod(0o755)
        appimage=home/'Applications/duckstation-linux.AppImage'
        appimage.parent.mkdir();appimage.write_bytes(b'\x7fELF0000AI\x02synthetic non-playable AppImage');appimage.chmod(0o600)
        shortcuts=home/'.local/share/applications';shortcuts.mkdir(parents=True)
        (shortcuts/'duckstation.desktop').write_text('[Desktop Entry]\nExec="'+str(appimage)+'" %f\n')
        exe = home/'Windows Fixture.exe'
        exe.write_text('Synthetic registration fixture.')
        desktop = home/'Windows Fixture.desktop'
        desktop.write_text('[Desktop Entry]\nType=Application\nName=Fixture\nExec=true\n')
        log = home/'launch.log'
        env = os.environ.copy()
        env.update(HOME=str(home), XDG_CONFIG_HOME=str(home/'config'), XDG_DATA_HOME=str(home/'.local/share'),
                   XDG_CACHE_HOME=str(home/'cache'), XDG_STATE_HOME=str(home/'state'),
                   PATH=str(home/'bin')+':'+env['PATH'], MATRIX_NATIVE_TEST_LAUNCH_LOG=str(log), MATRIX_NATIVE_TEST_CACHE_LOG=str(home/'child-cache.log'))
        if cfg.unset_parent_cache:
            env.pop('XDG_CACHE_HOME',None)
        parent_cache = home/'.cache' if cfg.unset_parent_cache else home/'cache'
        expected_child_cache = '' if cfg.unset_parent_cache else str(home/'cache')
        if cfg.backend != 'session':
            env['GDK_BACKEND'] = cfg.backend
        if cfg.backend == 'x11':
            env.pop('WAYLAND_DISPLAY', None)
        if cfg.legacy_graphics_workaround:
            env['WEBKIT_DISABLE_DMABUF_RENDERER'] = '1'
        driver_log = open(home/'driver.log', 'w+')
        driver = subprocess.Popen([cfg.driver, '--port', endpoint.rsplit(':',1)[1], '--native-port', str(port()),
                                   '--native-driver', cfg.webkit_driver], env=env, cwd=home, stdout=driver_log, stderr=subprocess.STDOUT, start_new_session=True)

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

        def key(value, shift=False):
            actions = ([{'type':'keyDown','value':'\ue008'}] if shift else [])
            actions += [{'type':'keyDown','value':value}, {'type':'keyUp','value':value}]
            actions += ([{'type':'keyUp','value':'\ue008'}] if shift else [])
            request('POST', f'/session/{session}/actions', {'actions':[{'type':'key','id':'keyboard','actions':actions}]})

        def focused(selector):
            return js('return document.activeElement === document.querySelector(arguments[0])', selector)

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
            wait_for(lambda: js('return document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX" && document.querySelectorAll(".console-card").length === 18'))
            assert len([p for p in library.iterdir() if p.is_dir()]) == 18
            assert js('return document.title').startswith('Matrix Play')
            wait_for(lambda: js('return document.querySelector(".brand-logo")?.naturalWidth > 0'))
            discovered=invoke('bootstrap')
            assert discovered['profiles']['n64']['flatpakId']=='com.github.Rosalie241.RMG'
            assert discovered['profiles']['n64']['autoDetected'] is True
            n64game=library/'N64/N64 Games/Fullscreen Test.z64'
            n64game.write_text('Synthetic, non-playable ROM fixture')
            invoke('launch_console_game',{'id':'n64','uid':str(n64game)})
            n64base=['run','--system',f'--filesystem={n64game.parent}:ro','com.github.Rosalie241.RMG']
            wait_for(lambda: log.read_text().splitlines()==n64base+['--fullscreen','--nogui','--quit-after-emulation',str(n64game)])
            invoke('open_emulator',{'id':'n64'})
            wait_for(lambda: log.read_text().splitlines()==n64base)
            stored=library/'PS5/PS5 Games/Collection.pkg'
            stored.write_text('Stored collection fixture, never executable')
            for command,params in [('launch_console_game',{'id':'ps5','uid':str(stored)}),('open_emulator',{'id':'ps5'})]:
                try:
                    invoke(command,params)
                    raise AssertionError('Library-only console accepted launch')
                except RuntimeError as error: assert 'Library only' in str(error)
            assert log.read_text().splitlines()==n64base
            assert invoke('set_favorite',{'id':'ps5','uid':str(stored),'favorite':True})==[str(stored)]
            log.unlink()

            image_candidates=[c for c in discovered['emulatorDiscovery']['candidates'] if c['profile'].get('emulatorPath')==str(appimage)]
            assert len(image_candidates)==1 and image_candidates[0]['ready'] is False
            assert appimage.stat().st_mode & 0o777 == 0o600
            assert 'ps1' not in discovered['profiles']
            appimage.chmod(0o700)
            assert invoke('bootstrap')['profiles']['ps1']['launcher']=='appimage'
            diagnostics=invoke('graphics_diagnostics')
            assert diagnostics['XDG_CACHE_HOME'] == str(parent_cache/'matrix/runtime-v1')
            assert len(invoke('scan_console', {'id':'ps1'})) == 1
            assert len(invoke('scan_console', {'id':'ps2'})) == 1
            assert len(invoke('scan_console', {'id':'ps3'})) == 0
            # External PS1 folders: actual dialog/IPC, grouping, artwork and safe removal.
            # Launcher scripts below verify argv only, not real emulator gameplay.
            external_ps1 = home/'External PS1 Collection'
            external_ps1.mkdir()
            for name in ['Disc One.chd', 'Disc Two.chd', 'Disc Three.chd', 'Loose.chd']:
                (external_ps1/name).write_text('Synthetic external-disc fixture')
            playlist = external_ps1/'Adventure.m3u'
            (external_ps1/'Broken.m3u').write_text('Loose.chd\nMissing.chd\n')
            (external_ps1/'Ignored.m3u8').write_text('Loose.chd\n')
            click('[data-action="console"][data-id="ps1"]')
            click('[data-action="library-folders"][data-id="ps1"]')
            wait_for(lambda: js('return document.querySelector(".library-folder-list small")?.textContent === "Managed folder"'))
            js('document.querySelector("#external-library-path").value=arguments[0]', str(external_ps1))
            click('[data-action="add-library-folder"]')
            wait_for(lambda: js('return !libraryFolderBusy && document.querySelectorAll("[data-action=remove-library-folder]").length === 1'))
            folders = invoke('list_library_folders', {'id':'ps1'})
            assert folders == [{'path':str(ps1),'managed':True,'available':True}, {'path':str(external_ps1),'managed':False,'available':True}], folders
            screenshot('matrix-library-folders.png')
            # Existing individual-disc favorites and imported covers remain reachable.
            fixture_cover = Path(__file__).parent/'fixtures/demo-cover.png'
            invoke('set_favorite', {'id':'ps1','uid':str(external_ps1/'Disc One.chd'),'favorite':True})
            invoke('save_cover', {'kind':'ps1','uid':str(external_ps1/'Disc Two.chd'),'bytes':list(fixture_cover.read_bytes())})
            playlist.write_text('Disc One.chd\nDisc Two.chd\nDisc Three.chd\n')
            games = invoke('scan_console', {'id':'ps1'})
            assert {g['uid'] for g in games} == {str(ps1game),str(playlist),str(external_ps1/'Loose.chd'),str(external_ps1/'Disc One.chd'),str(external_ps1/'Disc Two.chd')}, games
            invoke('set_favorite', {'id':'ps1','uid':str(playlist),'favorite':True})
            fixture_cover = Path(__file__).parent/'fixtures/demo-cover.png'
            invoke('save_cover', {'kind':'ps1','uid':str(playlist),'bytes':list(fixture_cover.read_bytes())})
            assert invoke('get_cover', {'kind':'ps1','uid':str(playlist)}).startswith('data:image/png;base64,')
            duckstation_fixture = home/'bin/duckstation-qt'
            duckstation_fixture.write_text(launcher); duckstation_fixture.chmod(0o755)
            invoke('save_emulator', {'id':'ps1','emulatorName':'DuckStation fixture','emulatorPath':str(duckstation_fixture),'launchArgs':['-batch','{game}'],'launcher':'executable'})
            invoke('launch_console_game', {'id':'ps1','uid':str(playlist)})
            wait_for(lambda: log.read_text().splitlines() == ['-nogui','-fullscreen','-batch','--',str(playlist)])
            invoke('save_emulator', {'id':'ps1','emulatorName':'Flatpak fixture','emulatorPath':'','launchArgs':['{game}'],'launcher':'flatpak','flatpakId':'org.matrix.TestEmulator','flatpakInstallation':'user'})
            invoke('launch_console_game', {'id':'ps1','uid':str(playlist)})
            wait_for(lambda: log.read_text().splitlines() == ['run','--user',f'--filesystem={external_ps1}:ro','org.matrix.TestEmulator',str(playlist)])
            unplugged_ps1 = external_ps1.with_name('Temporarily disconnected PS1')
            external_ps1.rename(unplugged_ps1)
            assert not invoke('list_library_folders', {'id':'ps1'})[1]['available']
            click('[data-action="remove-library-folder"]')
            wait_for(lambda: js('return !libraryFolderBusy && document.querySelectorAll("[data-action=remove-library-folder]").length === 0'))
            assert len(invoke('scan_console', {'id':'ps1'})) == 1
            unplugged_ps1.rename(external_ps1)
            for command, params in [('get_cover',{'kind':'ps1','uid':str(playlist)}), ('launch_console_game',{'id':'ps1','uid':str(playlist)})]:
                try:
                    invoke(command, params)
                    raise AssertionError('Removed library remained authorized')
                except RuntimeError:
                    pass
            assert playlist.read_text() == 'Disc One.chd\nDisc Two.chd\nDisc Three.chd\n'
            assert (external_ps1/'Disc One.chd').read_text() == 'Synthetic external-disc fixture'
            assert str(playlist) in invoke('bootstrap')['favorites']['ps1']
            invoke('add_library_folder', {'id':'ps1','path':str(external_ps1)})
            assert invoke('get_cover', {'kind':'ps1','uid':str(playlist)}).startswith('data:image/png;base64,')
            assert invoke('get_cover', {'kind':'ps1','uid':str(external_ps1/'Disc Two.chd')}).startswith('data:image/png;base64,')
            assert str(external_ps1/'Disc One.chd') in invoke('bootstrap')['favorites']['ps1']
            assert len(invoke('scan_console', {'id':'ps1'})) == 5
            invoke('remove_library_folder', {'id':'ps1','path':str(external_ps1)})
            click('[data-action="close"]')
            click('[data-action="home"]')
            log.unlink()
            print('PASS: external-folder dialog, PS1 M3U grouping, malformed-list preservation, scoped Flatpak argv, offline removal and retained favorites/artwork')
            wait_for(lambda: js('return [...document.querySelectorAll(".console-image")].length === 18 && [...document.querySelectorAll(".console-image")].every(i=>i.naturalWidth > 0)'))
            update=invoke('update_status')
            assert update['phase'] == 'idle' and update['currentVersion'] == diagnostics['version']
            if cfg.check_updates or cfg.download_update:
                click('[data-action="updates"]')
                wait_for(lambda: invoke('update_status')['phase'] in ['current','available','error'], 75)
                status=invoke('update_status')
                assert status['phase'] in ['current','available'],status
                if cfg.download_update:
                    assert status['phase'] == 'available',status
                    click('[data-action="download-update"]')
                    wait_for(lambda: invoke('update_status')['phase'] in ['downloaded','error'], 180)
                    status=invoke('update_status')
                    assert status['phase'] == 'downloaded',status
                    wait_for(lambda: js('return Boolean(document.querySelector("[data-action=install-update]"))'))
                    assert list((home/'.local/share/matrix/updates').rglob('*.deb'))
                    print('PASS: real desktop UI checked public GitHub, downloaded and verified the signed newer DEB; installation not run')
                else:
                    print('PASS: real desktop UI checked the public GitHub update service')
                screenshot('matrix-linux-update-check.png')
                click('[data-action="close"]')
            screenshot('matrix-linux-home.png')
            ps1_favorite_baseline = set(invoke('bootstrap')['favorites'].get('ps1', []))
            assert ps1_favorite_baseline == {str(playlist), str(external_ps1/'Disc One.chd')}
            click('[data-action="console"][data-id="ps2"]')
            wait_for(lambda: js('return document.querySelectorAll(".game-item").length === 1'))
            if cfg.online:
                wait_for(lambda: invoke('get_cover', {'kind':'ps2','uid':str(ps2game)}), 60)
                wait_for(lambda: invoke('get_cover', {'kind':'ps1','uid':str(ps1game)}), 60)
                wait_for(lambda: js('return document.querySelector(".game-cover img")?.complete'))
            click('[data-action="favorite"]')
            wait_for(lambda: invoke('bootstrap')['favorites'].get('ps2') == [str(ps2game)])
            click('[data-action="console-favorites"]')
            assert js('return document.querySelectorAll(".game-item").length') == 1
            assert set(invoke('bootstrap')['favorites'].get('ps1', [])) == ps1_favorite_baseline
            try:
                invoke('set_favorite', {'id':'ps1','uid':str(ps2game),'favorite':True})
                raise AssertionError('Cross-console favorite accepted')
            except RuntimeError: pass
            request('POST',f'/session/{session}/refresh',{})
            wait_for(lambda: js('return document.querySelectorAll(".console-card").length === 18 && document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX"'))
            click('[data-action="console"][data-id="ps2"]')
            wait_for(lambda: js('return document.querySelector(".favorite")?.getAttribute("aria-pressed") === "true"'))
            screenshot('matrix-linux-ps2.png')
            click('.game-card')
            wait_for(log.exists)
            assert log.read_text().splitlines() == [str(ps2game)]
            wait_for(lambda: (home/'child-cache.log').read_text() == expected_child_cache)
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
            wait_for(lambda: (home/'child-cache.log').read_text() == expected_child_cache)
            # Direct executables and retired runner values cannot be registered or launched.
            for runner in ['desktop', 'retired-runtime']:
                try:
                    invoke('add_windows_game', {'title':'Rejected executable','target':str(exe),'runner':runner})
                    raise AssertionError('Direct executable registration was accepted')
                except RuntimeError as error:
                    assert 'Direct .exe launching is unavailable' in str(error)
            try:
                invoke('launch_windows_game', {'uid':legacy['uid']})
                raise AssertionError('Legacy direct executable was launched')
            except RuntimeError as error:
                assert 'Direct .exe launching is unavailable' in str(error)
            assert legacy in invoke('list_windows_games')
            assert exe.read_text() == 'Synthetic registration fixture.'
            invoke('add_windows_game', {'title':'Native Windows Fixture','target':str(desktop),'runner':'desktop'})
            click('[data-action="refresh"]')
            click('[data-action="windows"]')
            wait_for(lambda: js('return document.querySelectorAll(".game-item").length === 2'))
            click('.game-card[aria-label="View Legacy Windows Fixture"]')
            assert js('return document.querySelector(".launch-button").disabled')
            assert 'Direct .exe launching is unavailable' in js('return document.querySelector(".detail-description").textContent')
            click('[data-action="close"]')
            click('[data-action="add-windows"]')
            assert not js('return Boolean(document.querySelector("#windows-runner"))')
            assert '.desktop' in js('return document.querySelector("#windows-target").placeholder')
            click('[data-action="close"]')
            log.unlink()
            click('.game-card[aria-label="Launch Native Windows Fixture"]')
            wait_for(log.exists)
            assert log.read_text().splitlines() == ['launch', str(desktop)]
            wait_for(lambda: (home/'child-cache.log').read_text() == expected_child_cache)
            screenshot('matrix-linux-windows.png')
            # A native Steam binary is present, but this title belongs to Flatpak.
            flat_steam = home/'.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps'
            flat_steam.mkdir(parents=True)
            external_steam = home/'External Steam Library/steamapps'
            external_steam.mkdir(parents=True)
            (flat_steam/'libraryfolders.vdf').write_text('"path" "'+str(external_steam.parent)+'"\n')
            (external_steam/'appmanifest_23456.acf').write_text('"appid" "23456"\n"name" "Flatpak Steam Fixture"\n')
            invoke('launch_steam_game', {'uid':'23456'})
            wait_for(lambda: log.read_text().splitlines() == ['run','com.valvesoftware.Steam','-applaunch','23456'])
            # Synthetic Snap client and custom Steam root IPC; no real gameplay.
            snap = home/'bin/snap'
            snap.write_text((home/'bin/steam').read_text())
            snap.chmod(0o755)
            snap_steam = home/'snap/steam/common/.local/share/Steam/steamapps'
            snap_steam.mkdir(parents=True)
            (snap_steam/'appmanifest_34567.acf').write_text('"appid" "34567"\n"name" "Snap Steam Fixture"\n')
            invoke('launch_steam_game', {'uid':'34567'})
            wait_for(lambda: log.read_text().splitlines() == ['run','steam','-applaunch','34567'])
            custom_steam = home/'Custom Steam Library/steamapps'
            custom_steam.mkdir(parents=True)
            custom_manifest = custom_steam/'appmanifest_45678.acf'
            custom_manifest.write_text('"appid" "45678"\n"name" "Custom Steam Fixture"\n')
            roots = invoke('add_steam_library', {'path':str(custom_steam.parent),'launcher':'native'})
            assert roots == [{'path':str(custom_steam),'launcher':'native','available':True}], roots
            invoke('launch_steam_game', {'uid':'45678'})
            wait_for(lambda: log.read_text().splitlines() == ['-applaunch','45678'])
            roots = invoke('add_steam_library', {'path':str(custom_steam),'launcher':'snap'})
            assert len(roots)==1 and roots[0]['launcher']=='snap', roots
            invoke('launch_steam_game', {'uid':'45678'})
            wait_for(lambda: log.read_text().splitlines() == ['run','steam','-applaunch','45678'])
            offline = custom_steam.with_name('unmounted')
            custom_steam.rename(offline)
            roots = invoke('list_steam_libraries')
            assert len(roots)==1 and not roots[0]['available'] and roots[0]['launcher']=='snap', roots
            assert invoke('remove_steam_library', {'path':str(custom_steam)}) == []
            offline.rename(custom_steam)
            assert custom_manifest.is_file(), 'Removing a registration must preserve Steam files'
            # Delayed runner failures must remain visible after successful spawn.
            wait_for(lambda: js('return !state.launchBusy.windows'))
            gio = home/'bin/gio'
            gio.write_text('#!/bin/sh\nsleep 0.7\necho "Desktop shortcut fixture could not launch" >&2\nexit 17\n')
            click('.game-card[aria-label="Launch Native Windows Fixture"]')
            wait_for(lambda: js('return state.launches.windows?.phase === "error"'))
            failure = js('return state.launches.windows')
            assert '17' in failure['message'] and 'Windows launcher' in failure['message'], failure
            # Rust formats ExitStatus as "exit status: 17". Check the actual
            # backend message in the error UI instead of inventing its wording.
            assert js('return document.querySelector("#launch-status .launch-error p")?.textContent') == failure['message']
            click('[data-action="launch-details"][data-id="windows"]')
            assert 'Desktop shortcut fixture could not launch' in js('return document.querySelector(".launch-output").textContent')
            assert not js('return Boolean(document.querySelector(".launch-dialog [data-action=open-emulator]"))')
            click('[data-action="close"]')
            gio.write_text(launcher)
            # gio dispatch is also monitored; this is a shim, not a GTK desktop launch.
            desktop = home/'Failed Launcher.desktop'
            desktop.write_text('[Desktop Entry]\nType=Application\nName=Fixture\nExec=missing-fixture\n')
            gio = home/'bin/gio'
            gio.write_text('#!/bin/sh\nsleep 0.7\necho "gio fixture failed to launch" >&2\nexit 19\n')
            gio.chmod(0o755)
            registered = invoke('add_windows_game', {'title':'Desktop failure','target':str(desktop),'runner':'desktop'})
            info = invoke('launch_windows_game', {'uid':registered['uid']})
            assert info['phase'] == 'started', info
            wait_for(lambda: invoke('get_launch_status', {'id':'windows'})['phase'] == 'error')
            info = invoke('get_launch_status', {'id':'windows'})
            assert '19' in info['message'] and 'gio fixture failed to launch' in info['log'], info
            invoke('remove_windows_game', {'uid':registered['uid']})
            invoke('save_artwork_settings', {'enabled':False,'style':'3d'})
            boot=invoke('bootstrap')
            assert boot['artwork'] == {'enabled':False,'style':'3d'}
            assert boot['profiles']['ps2']['emulatorPath'].endswith('TestEmulator.AppImage')
            assert len(boot['windows']) == 2
            assert legacy in boot['windows']
            assert boot['favorites']['ps2']==[str(ps2game)]
            apps=invoke('list_flatpaks')
            assert {a['installation'] for a in apps}=={'user','system'}
            click('[data-action="settings"]')
            click('[data-action="configure"][data-id="ps2"]')
            js('document.querySelector("#emulator-launcher").value="flatpak";document.querySelector("#emulator-launcher").dispatchEvent(new Event("change",{bubbles:true}))')
            wait_for(lambda: js('return document.querySelector("#flatpak-app").options.length === 4'))
            js('window.__keyEvents=[];document.addEventListener("keydown",event=>{const entry={key:event.key,code:event.code,keyCode:event.keyCode,shift:event.shiftKey,target:event.target.outerHTML};queueMicrotask(()=>{entry.prevented=event.defaultPrevented;entry.active=document.activeElement.outerHTML;window.__keyEvents.push(entry)})})')
            # Native WebKit keyboard navigation, not simulated focus handling.
            # Hidden file/AppImage controls must not steal Flatpak tab order.
            js('document.querySelector("#emulator-launcher").focus()')
            key('\ue004')
            assert focused('#flatpak-app')
            js('document.querySelector("#emulator-name").focus()')
            key('\ue004')
            assert focused('.advanced-setup summary')
            key('\ue007')
            assert js('return document.querySelector(".advanced-setup").open')
            key('\ue004')
            assert focused('#launch-args')
            key('\ue004')
            assert focused('#library-path')
            key('\ue004', shift=True)
            assert focused('#launch-args')
            key('\ue004', shift=True)
            assert focused('.advanced-setup summary')
            key('\ue004', shift=True)
            assert focused('#emulator-name')
            js('document.querySelector(".advanced-setup").open=false;document.querySelector(".advanced-setup summary").focus()')
            key('\ue004')
            assert focused('#library-path')
            js('document.querySelector(".emulator-dialog .close").focus()')
            key('\ue004', shift=True)
            assert focused('[data-open-after-save]')
            key('\ue004')
            assert focused('.emulator-dialog .close')
            print('PASS: native keyboard forward/reverse Tab, expanded/collapsed advanced settings and dialog wrapping')
            # Exercise real WebKit focus/layout and the production polling loop,
            # with a synthetic standard Gamepad API device. This is NOT physical
            # controller, driver mapping, or platform Gamepad API verification.
            assert js('return ["nav-consoles","nav-steam","nav-windows","nav-settings"].every(id=>document.getElementById(id).getAttribute("aria-label"))')
            assert js('return document.hasFocus()'), 'Native window must own focus for controller checks'
            js('''window.__originalGamepads=Object.getOwnPropertyDescriptor(navigator,'getGamepads');
                window.__testPad={index:0,id:'Matrix synthetic controller',mapping:'standard',connected:true,
                  axes:[0,0],buttons:Array.from({length:16},()=>({pressed:false,value:0,touched:false}))};
                Object.defineProperty(navigator,'getGamepads',{configurable:true,value:()=>[window.__testPad]});
                window.dispatchEvent(new Event('gamepadconnected'));''')

            def gamepad_buttons(*buttons):
                request('POST', f'/session/{session}/execute/async', {'script': '''
                    const pressed=arguments[0],done=arguments[arguments.length-1];
                    window.__testPad.buttons.forEach((button,index)=>{button.pressed=pressed.includes(index);button.value=button.pressed?1:0;});
                    requestAnimationFrame(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));
                ''', 'args':[list(buttons)]})

            def gamepad_press(button):
                gamepad_buttons()
                gamepad_buttons(button)

            gamepad_buttons()
            js('document.querySelector(".emulator-dialog .close").focus()')
            gamepad_press(4)  # LB wraps only inside this modal.
            assert focused('[data-open-after-save]')
            gamepad_press(5)
            assert focused('.emulator-dialog .close')
            assert js('return document.documentElement.classList.contains("gamepad-navigation")')
            js('document.querySelector("#flatpak-app").selectedIndex=1;document.querySelector("#flatpak-app").focus()')
            gamepad_press(13)  # D-pad changes a native select without clicking behind it.
            assert js('return document.querySelector("#flatpak-app").selectedIndex===2')
            gamepad_press(5)
            assert focused('[data-action="reload-flatpaks"]')
            js('document.querySelector(".advanced-setup").open=false;document.querySelector(".advanced-setup summary").focus()')
            gamepad_press(0)
            assert js('return document.querySelector(".advanced-setup").open')
            time.sleep(.5)  # Holding A must not toggle the summary repeatedly.
            assert js('return document.querySelector(".advanced-setup").open')
            gamepad_press(1)
            assert js('return !document.querySelector("[role=dialog]")')
            gamepad_buttons()
            js('''window.__testPad.connected=false;
                if (window.__originalGamepads) Object.defineProperty(navigator,'getGamepads',window.__originalGamepads);
                else delete navigator.getGamepads;
                delete window.__testPad; delete window.__originalGamepads;''')
            print('PASS: simulated gamepad in native WebKit: modal traversal, select, activation, hold guard, back; physical hardware unverified')
            click('[data-action="configure"][data-id="ps2"]')
            js('document.querySelector("#emulator-launcher").value="flatpak";document.querySelector("#emulator-launcher").dispatchEvent(new Event("change",{bubbles:true}))')
            wait_for(lambda: js('return document.querySelector("#flatpak-app").options.length === 4'))
            js('document.querySelector("#flatpak-app").value="org.matrix.TestEmulator|user";document.querySelector("#flatpak-app").dispatchEvent(new Event("change",{bubbles:true}));document.querySelector("#config-form").requestSubmit()')
            wait_for(lambda: invoke('bootstrap')['profiles']['ps2'].get('launcher')=='flatpak')
            wait_for(lambda: js('return !document.querySelector("#config-form")'))
            click('[data-action="home"]')
            click('[data-action="console"][data-id="ps2"]')
            wait_for(lambda: js('return Boolean(document.querySelector(".game-card"))'))
            log.unlink()
            (home/'child-cache.log').unlink()
            click('.game-card')
            expected=['run','--user',f'--filesystem={ps2}:ro','org.matrix.TestEmulator',str(ps2game)]
            wait_for(lambda: log.read_text().splitlines()==expected)
            wait_for(lambda: (home/'child-cache.log').read_text()==expected_child_cache)
            assert invoke('bootstrap')['profiles']['ps2']['flatpakInstallation']=='user'
            assert invoke('bootstrap')['favorites']['ps2']==[str(ps2game)]
            # The child writes argv before the asynchronous UI launch request settles.
            # Wait for completion before testing a second click; busy launches reject duplicates.
            wait_for(lambda: js('return !state.launchBusy.ps2'))
            click('[data-action="open-emulator"][data-id="ps2"]')
            wait_for(lambda: log.read_text().splitlines()==expected[:-1])
            healthy_flatpak=flatpak.read_text()
            pcsx2_flatpak=healthy_flatpak.replace('com.github.Rosalie241.RMG\\tRMG\\tsystem\\n','com.github.Rosalie241.RMG\\tRMG\\tsystem\\nnet.pcsx2.PCSX2\\tPCSX2\\tsystem\\n')
            flatpak.write_text(pcsx2_flatpak)
            invoke('save_emulator',{'id':'ps2','emulatorName':'PCSX2','emulatorPath':'','launcher':'flatpak','flatpakId':'net.pcsx2.PCSX2','flatpakInstallation':'system','launchArgs':['--','{game}']})
            opened=invoke('open_emulator',{'id':'ps2'})
            assert opened['phase']!='error',opened
            fontfile=home/'.local/share/matrix/compat/runtime-fonts-v1.conf'
            assert fontfile.exists() and '/usr/share/fonts' in fontfile.read_text()
            fontargs=['run','--system',f'--filesystem={ps2}:ro',f'--filesystem={fontfile}:ro',f'--env=FONTCONFIG_FILE={fontfile}','net.pcsx2.PCSX2']
            wait_for(lambda: log.read_text().splitlines()==fontargs)
            try:
                invoke('launch_console_game',{'id':'ps2','uid':str(ps2game)})
                raise AssertionError('Unfinished PCSX2 setup should be explained')
            except RuntimeError as error: assert 'Open emulator' in str(error)
            ini=home/'.var/app/net.pcsx2.PCSX2/config/PCSX2/inis/PCSX2.ini'
            ini.parent.mkdir(parents=True)
            ini.write_text('[UI]\nSetupWizardIncomplete=false\n[Filenames]\nBIOS=synthetic-not-a-real-bios.bin\n')
            invoke('launch_console_game',{'id':'ps2','uid':str(ps2game)})
            wait_for(lambda: log.read_text().splitlines()==fontargs+['-nogui','-fullscreen','--',str(ps2game)])

            invoke('save_emulator',{'id':'ps2','emulatorName':'Matrix Test Emulator','emulatorPath':'','launcher':'flatpak','flatpakId':'org.matrix.TestEmulator','flatpakInstallation':'user','launchArgs':['{game}'],'flatpakRuntimeFonts':False})
            flatpak.write_text(healthy_flatpak)
            flatpak.write_text(healthy_flatpak.replace('printf "%s\\n" "$@" > "$MATRIX_NATIVE_TEST_LAUNCH_LOG"', 'echo "runtime failed to start" >&2; exit 17\n#'))
            failed=invoke('launch_console_game',{'id':'ps2','uid':str(ps2game)})
            assert failed['phase']=='error' and 'runtime failed to start' in failed['log'],failed
            assert Path(failed['logPath']).read_text().strip()=='runtime failed to start'
            assert invoke('get_launch_status',{'id':'ps2'})['phase']=='error'
            # Scoped save-write flags for known sidecar-saving Flatpaks.
            save_flatpak = healthy_flatpak.replace('com.github.Rosalie241.RMG\\tRMG\\tsystem\\n', 'com.github.Rosalie241.RMG\\tRMG\\tsystem\\nio.github.sameboy.SameBoy\\tSameBoy\\tuser\\nio.mgba.mGBA\\tmGBA\\tuser\\nnet.kuribo64.melonDS\\tmelonDS\\tuser\\n')
            flatpak.write_text(save_flatpak)
            for console, folder, engine, extension in [('gb','Game Boy','io.github.sameboy.SameBoy','gbc'),('gba','GBA','io.mgba.mGBA','gba'),('nds','Nintendo DS','net.kuribo64.melonDS','nds')]:
                game_root = library/folder/(folder+' Games')
                game = game_root/'Nested Game'/('Fixture.'+extension)
                game.parent.mkdir(parents=True)
                game.write_text('Synthetic fixture')
                invoke('save_emulator',{'id':console,'emulatorName':'Fixture','emulatorPath':'','launcher':'flatpak','flatpakId':engine,'flatpakInstallation':'user','launchArgs':['{game}']})
                invoke('launch_console_game',{'id':console,'uid':str(game)})
                argv = log.read_text().splitlines()
                assert f'--filesystem={game_root}:ro' in argv and f'--filesystem={game.parent}:rw' in argv, argv
                assert argv[-1] == str(game), argv
                invoke('open_emulator',{'id':console})
                argv = log.read_text().splitlines()
                assert f'--filesystem={game_root}:ro' in argv and not any(arg.endswith(':rw') for arg in argv), argv
            flatpak.write_text(healthy_flatpak)
            invoke('set_favorite',{'id':'ps2','uid':str(ps2game),'favorite':False})
            assert invoke('bootstrap')['favorites']['ps2']==[]
            invoke('set_favorite',{'id':'ps2','uid':str(ps2game),'favorite':True})
            request('DELETE',f'/session/{session}')
            session=None
            time.sleep(.5)
            session=request('POST','/session',{'capabilities':{'alwaysMatch':{'tauri:options':{'application':str(binary)}}}})['sessionId']
            request('POST',f'/session/{session}/timeouts',{'script':60000})
            wait_for(lambda: js('return document.querySelectorAll(".console-card").length === 18 && document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX"'))
            click('[data-action="console"][data-id="ps2"]')
            wait_for(lambda: js('return document.querySelector(".favorite")?.getAttribute("aria-pressed") === "true"'))
            assert invoke('bootstrap')['profiles']['ps2']['launcher']=='flatpak'
            assert set(invoke('bootstrap')['favorites'].get('ps1', [])) == ps1_favorite_baseline
            assert legacy in invoke('list_windows_games')
            assert exe.read_text() == 'Synthetic registration fixture.'
            print('PASS: desktop-only registration, direct executable rejection and legacy entry preservation')
            print('PASS: scoped sidecar-save Flatpak argv, native/Flatpak Steam provenance, and delayed Windows runner diagnostics (synthetic fixtures)')
            print('PASS: automatic Flatpak/AppImage linking, canonical duplicate removal, execute-permission preservation, Open emulator and captured startup errors')
            print('PASS: native favorites survive process restart and webview reload, remain console-scoped, and preserve profiles; Flatpak discovery/setup/launch argv use harmless fixtures')
            print('PASS: real Tauri/WebKit window, native IPC, 18 libraries, PS1/PS2 scanning, manual/cache artwork, console boundaries, persisted settings, three launcher argument paths')
            if cfg.online:
                print('PASS: real desktop automatic PS1 and PS2 cover downloads (synthetic serial-labelled game fixtures)')
        except Exception:
            try:
                print('Native UI failure state:', json.dumps(js('return {page:state.page, overlay:state.overlay, busy:state.launchBusy, launches:state.launches, active:document.activeElement.outerHTML, keyEvents:window.__keyEvents, tabStops:[...document.querySelectorAll("#overlay-root button,#overlay-root input,#overlay-root select,#overlay-root textarea,#overlay-root summary")].map(node=>({html:node.outerHTML,tabIndex:node.tabIndex,disabled:node.matches(":disabled"),hidden:!!node.closest("[hidden],[inert]"),rects:node.getClientRects().length,visibility:getComputedStyle(node).visibility})), launchStatus:document.querySelector("#launch-status")?.textContent}')))
            except Exception:
                pass
            driver_log.flush()
            print((home/'driver.log').read_text())
            raise
        finally:
            if session:
                try: request('DELETE', f'/session/{session}')
                except Exception: pass
            try: os.killpg(driver.pid, signal.SIGTERM)
            except ProcessLookupError: pass
            try: driver.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(driver.pid, signal.SIGKILL)
                driver.wait()
            driver_log.close()
            time.sleep(.5)  # Let WebKit finish releasing its temporary disk caches.

if __name__ == '__main__':
    main()
