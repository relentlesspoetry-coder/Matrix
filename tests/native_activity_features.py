"""Native gaming-mode, statistics, labels and read-only achievement UI fixtures.

Uses only synthetic game files and a disposable external shell process. The
RetroAchievements web responses are injected; no account key or gameplay is used.
"""
import os
import signal
import subprocess


def exercise(home, library, invoke, js, click, wait_for, screenshot):
    wait_for(lambda: not js('return gamingPaused()'))
    original=invoke('bootstrap')['profiles']['ps2']
    external=home/'External-emulator-fixture.sh'
    external.write_text('#!/bin/sh\nsleep 30\n')
    external.chmod(0o700)
    invoke('save_emulator',{'id':'ps2','emulatorName':'External fixture','emulatorPath':str(external),'launchArgs':['{game}']})
    process=subprocess.Popen([str(external)],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
    previous_pause=js('return state.coverQueue.paused')
    try:
        wait_for(lambda: 'PS2' in invoke('gaming_status')['emulators'])
        wait_for(lambda: js('return gamingPaused()'))
        assert js('return document.documentElement.dataset.gaming')=='active'
        assert js('return getComputedStyle(document.body,"::before").animationPlayState')=='paused'
        js('window.__activityInvoke=nativeInvoke;window.__pausedCalls=[];nativeInvoke=(name,args)=>{window.__pausedCalls.push(name);return window.__activityInvoke(name,args)};state.coverQueue.paused=true;nextUpdateCheck=0;window.__pausedDone=false;Promise.all([refreshManagedAndGames(),pollLibraries(),scanArtwork(true),checkForUpdateNotice(),pollPlaytime()]).then(()=>window.__pausedDone=true)')
        wait_for(lambda: js('return window.__pausedDone'))
        commands=js('return window.__pausedCalls')
        assert not any(c in commands for c in ['refresh_managed_emulators','library_snapshot','download_cover','check_updates_background','get_playtime']),commands
        screenshot('matrix-native-gaming-mode.png')
    finally:
        js('if(window.__activityInvoke){nativeInvoke=window.__activityInvoke;delete window.__activityInvoke}')
        try: os.killpg(process.pid,signal.SIGTERM)
        except ProcessLookupError: pass
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            process.wait()
        invoke('save_emulator',{'id':'ps2',**original})
    wait_for(lambda: not js('return gamingPaused()'))
    assert js('return state.coverQueue.paused') is True
    js('state.coverQueue.paused=arguments[0]',previous_pause)
    key='gb|'+str(library/'Game Boy/Game Boy Games/Pokemon Red.gb')
    stats=invoke('playtime_statistics')
    assert stats['totals'][key]>=50*3600000
    assert sum(day.get(key,0) for day in stats['daily'].values())>2000
    js('goHome()')
    click('[data-action="playtime-statistics"]')
    wait_for(lambda: js('return document.querySelector("#statistics-content").textContent.includes("Most played")'))
    assert '50 h' in js('return document.querySelector("#statistics-content").textContent')
    screenshot('matrix-native-playtime-statistics.png')
    click('.statistics-dialog .close')
    game=library/'Game Boy/Game Boy Games/Pokemon Red.gb'
    js('goConsole("gb");openGame("gb",arguments[0])',str(game))
    js('const input=document.querySelector("#game-progress");input.value="completed";input.dispatchEvent(new Event("change",{bubbles:true}))')
    wait_for(lambda: js('return !document.querySelector("#game-progress").disabled'))
    assert invoke('bootstrap')['libraryPreferences']['metadata'][key]['progress']=='completed'
    click('.detail-panel .close')
    js('const select=document.querySelector("#progress-filter");select.value="completed";select.dispatchEvent(new Event("change",{bubbles:true}))')
    assert js('return document.querySelectorAll(".game-card").length')==1
    assert 'Completed' in js('return document.querySelector(".game-title").textContent')
    screenshot('matrix-native-progress-label.png')
    js('const select=document.querySelector("#progress-filter");select.value="all";select.dispatchEvent(new Event("change",{bubbles:true}));goHome()')
    # Keep the real account/status/save commands, inject read-only network results.
    js('window.__raOriginal=nativeInvoke;nativeInvoke=(name,args)=>{if(name==="ra_progress")return Promise.resolve({total:1,offset:0,games:[{gameId:1,title:"Pokemon Red (synthetic fixture)",consoleName:"Game Boy",possible:10,earned:4,hardcore:2}],cached:false,warning:"Synthetic account data; no network request."});if(name==="ra_game")return Promise.resolve({gameId:1,title:"Pokemon Red (synthetic fixture)",consoleName:"Game Boy",possible:2,earned:1,hardcore:1,achievements:[{title:"First <achievement>",description:"Synthetic unlock",points:5,earned:true,hardcore:true},{title:"Second",description:"Synthetic locked achievement",points:10,earned:false,hardcore:false}],cached:false,warning:"Synthetic account data."});return window.__raOriginal(name,args)}')
    try:
        click('[data-action="ra-dashboard"]')
        wait_for(lambda: js('return document.querySelector("#ra-username")&&!document.querySelector("#ra-username").disabled'))
        js('document.querySelector("#ra-username").value="FixtureUser";document.querySelector("#ra-key").value="fixture-web-key-0000000000000000"')
        click('[data-action="ra-connect"]')
        wait_for(lambda: js('return document.querySelectorAll(".achievement-game").length===1'))
        assert invoke('ra_status')=={'username':'FixtureUser','connected':True}
        assert js('return document.querySelector("#ra-key").value')==''
        assert '4 / 10' in js('return document.querySelector(".achievement-game").textContent')
        click('[data-action="ra-game"]')
        wait_for(lambda: js('return document.querySelectorAll(".achievement").length===2'))
        assert js('return document.querySelectorAll(".achievement.earned").length')==1
        assert 'First <achievement>' in js('return document.querySelector(".achievement").textContent')
        screenshot('matrix-native-achievements.png')
        js('document.querySelector(".ra-account").open=true')
        click('[data-action="ra-disconnect"]')
        wait_for(lambda: not invoke('ra_status')['connected'])
        click('.achievements-dialog .close')
    finally:
        js('nativeInvoke=window.__raOriginal;delete window.__raOriginal;dismissOverlay()')
        invoke('ra_save_account',{'username':'','key':''})
    assert game.read_bytes()==b'Synthetic GB game'
    print('PASS: native external-emulator detection, background pause/resume, retained playtime statistics, saved progress filters and RetroAchievements connection/dashboard (synthetic responses; no real credentials/gameplay)')
