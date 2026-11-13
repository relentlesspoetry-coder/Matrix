"""Verify native notice layout/navigation with a synthetic available release.

Network/signature selection uses the backend suite. Never download or install.
"""
def exercise(invoke, js, click, wait_for, screenshot):
    wait_for(lambda: js('return backgroundUpdateTask===null'), timeout=180)
    js('goHome()')
    assert js('return document.querySelector("#main-update-button")?.getClientRects().length>0')
    js('goSettings()')
    assert not js('return Boolean(document.querySelector("#screen [data-action=updates]"))')
    js('goHome();window.__hourlyOriginal=nativeInvoke;window.__hourlyCommands=[];window.__hourlyInfo={currentVersion:"0.0.0 fixture",latestVersion:"99.0.0",phase:"available",installMode:"download",message:"Synthetic update available; no installer downloaded.",notes:"Synthetic update notification test."};nativeInvoke=(name,args)=>{window.__hourlyCommands.push(name);return ["check_updates_background","update_status"].includes(name)?Promise.resolve(window.__hourlyInfo):window.__hourlyOriginal(name,args)};nextUpdateCheck=0;void checkForUpdateNotice()')
    try:
        wait_for(lambda: js('return !document.querySelector("#update-notice-root").hidden'))
        assert js('return state.overlay===null')
        assert js('return document.querySelector("#main-update-button").textContent')=='Updates'
        assert js('return document.documentElement.scrollWidth<=window.innerWidth')
        screenshot('matrix-native-update-notice.png')
        click('#update-notice-root [data-action="updates"]')
        wait_for(lambda: js('return state.overlay==="updates"&&document.querySelector("#update-latest").textContent==="99.0.0"'))
        click('.update-dialog .close')
        click('#update-notice-root [data-action="later-update"]')
        assert js('return document.querySelector("#update-notice-root").hidden')
        commands=js('return window.__hourlyCommands')
        assert commands.count('check_updates_background')==1,commands
        assert not any(c in commands for c in ['download_update','install_update']),commands
    finally:
        js('dismissOverlay();nativeInvoke=window.__hourlyOriginal;state.updateNotice=null;state.update=null;renderUpdateNotice();delete window.__hourlyOriginal')
    # The automatic endpoint must preserve its current protected state.
    assert invoke('update_status')['phase'] not in ['downloading','installing']
    print('PASS: native home update button, visible available-update prompt, explicit review and Later without download/installation (synthetic release)')
