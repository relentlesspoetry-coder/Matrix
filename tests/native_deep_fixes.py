"""Save UI, scaled dialogs, volume identity and startup recovery regressions.

Only disposable files and synthetic restore responses are used. No game is run.
"""
import json


def exercise(home, library, invoke, js, click, wait_for, screenshot):
    prefs = js('return {...libraryPrefs()}')
    try:
        for theme in ['arctic-current', 'white-frost']:
            for scale in [85, 100, 115, 130, 150]:
                js('libraryPrefs().theme=arguments[0];libraryPrefs().uiScale=arguments[1];goHome()', theme, scale)
                click('[data-action="playtime-statistics"]')
                wait_for(lambda: js('return document.querySelector("#statistics-content").textContent.includes("Most played")'))
                assert_bounds(js)
                if scale == 150:
                    screenshot('matrix-scaled-statistics-' + theme + '.png')
                click('.statistics-dialog .close')
                click('[data-action="ra-dashboard"]')
                wait_for(lambda: js('return document.querySelector("#ra-username")&&!document.querySelector("#ra-username").disabled'))
                assert_bounds(js)
                click('.achievements-dialog .close')
    finally:
        js('dismissOverlay();state.libraryPrefs=arguments[0];goHome()', prefs)

    # Model an unavailable volume with an empty directory still at its mountpoint.
    mount = home / 'persistent-volume'
    mount.mkdir()
    game = mount / 'Volume-fixture.gb'
    game.write_bytes(b'Synthetic volume fixture')
    uid = str(game)
    invoke('add_library_folder', {'id':'gb', 'path':str(mount)})
    assert any(g['uid'] == uid for g in invoke('library_snapshot')['games']['gb'])
    disconnected = home / 'disconnected-volume'
    mount.rename(disconnected)
    mount.mkdir()
    try:
        snapshot = invoke('library_snapshot')
        assert not next(f for f in snapshot['folders']['gb'] if f['path'] == str(mount))['available']
        assert any(g['uid'] == uid for g in snapshot['games']['gb'])
        assert (disconnected / game.name).read_bytes() == b'Synthetic volume fixture'
    finally:
        mount.rmdir()
        disconnected.rename(mount)
    assert next(f for f in invoke('library_snapshot')['folders']['gb'] if f['path'] == str(mount))['available']
    game.unlink()
    assert not any(g['uid'] == uid for g in invoke('library_snapshot')['games']['gb'])
    invoke('remove_library_folder', {'id':'gb','path':str(mount)})

    js('''
      goHome();window.__deepInvoke=nativeInvoke;window.__deepRestoreCalls=0;
      nativeInvoke=(name,args)=>{
        if(name==='save_data_dialog')return Promise.resolve({token:'synthetic',root:'/synthetic/memcards',files:[]});
        if(name==='apply_save_restore'){window.__deepRestoreCalls++;return new Promise(resolve=>window.__deepRestore=resolve);}
        return window.__deepInvoke(name,args);
      };
      void saveDataFlow('psp',true);
    ''')
    try:
        wait_for(lambda: js('return state.overlay==="save-restore"'))
        click('[data-action="apply-save-restore"]')
        wait_for(lambda: js('return saveRestoreBusy'))
        assert js('return document.querySelector(".save-restore-dialog .close").disabled')
        js('dismissOverlay()')
        assert js('return state.overlay') == 'save-restore'
        assert js('return window.__deepRestoreCalls') == 1
        js('void openPlaytimeStatistics()')  # simulate another async UI owner
        wait_for(lambda: js('return state.overlay==="statistics"'))
        js('window.__deepRestore("Synthetic restore completed")')
        wait_for(lambda: js('return !saveRestoreBusy'))
        assert js('return state.overlay') == 'statistics'
        click('.statistics-dialog .close')
    finally:
        js('if(window.__deepRestore)window.__deepRestore("Synthetic cleanup");nativeInvoke=window.__deepInvoke;dismissOverlay();delete window.__deepInvoke')

    # A transient startup failure must recover without another application restart.
    wait_for(lambda: js('return !managedRefreshBusy&&!libraryPolling'))
    settings_file = home / 'config/matrix/frontend.json'
    previous = settings_file.read_bytes()
    try:
        settings_file.write_text('{invalid synthetic startup fixture')
        js('window.location.reload()')
        wait_for(lambda: js('return typeof state!=="undefined"&&!state.nativeInitialized&&document.querySelector("#toast-root").textContent.includes("initialization failed")'))
        settings_file.write_bytes(previous)
        click('[data-action="refresh"]')
        wait_for(lambda: js('return state.nativeInitialized&&state.nativeBootstrapped'))
        assert js('return document.querySelectorAll(".console-card").length') > 0
    finally:
        # The recovery scan may have added current volume identities. Preserve it
        # when valid; only replace the deliberately corrupted fixture on failure.
        try:
            json.loads(settings_file.read_text())
        except (ValueError, OSError):
            settings_file.write_bytes(previous)
    print('PASS: native scaled modal bounds in both themes/all sizes, cached persistent mountpoints and reconnect/deletion, guarded restore ownership and startup recovery (synthetic fixtures)')


def assert_bounds(js):
    bounds = js('''
      const dialog=document.querySelector('#overlay-root [role="dialog"]'),r=dialog.getBoundingClientRect(),c=dialog.querySelector('.close').getBoundingClientRect();
      return {left:r.left,top:r.top,right:r.right,bottom:r.bottom,closeLeft:c.left,closeTop:c.top,closeRight:c.right,closeBottom:c.bottom,width:innerWidth,height:innerHeight};
    ''')
    assert bounds['left'] >= -1 and bounds['top'] >= -1, bounds
    assert bounds['right'] <= bounds['width'] + 1 and bounds['bottom'] <= bounds['height'] + 1, bounds
    assert bounds['closeLeft'] >= 0 and bounds['closeTop'] >= 0, bounds
    assert bounds['closeRight'] <= bounds['width'] + 1 and bounds['closeBottom'] <= bounds['height'] + 1, bounds
