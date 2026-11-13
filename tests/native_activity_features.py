"""Native gaming-mode, statistics, labels and read-only achievement UI fixtures.

Uses only synthetic game files and a disposable external shell process. The
RetroAchievements web responses are injected; no account key or gameplay is used.
"""
import os
import signal
import shutil
import subprocess
import time


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
    exercise_polish(home, invoke, js, click, wait_for, screenshot)


def exercise_polish(home, invoke, js, click, wait_for, screenshot):
    # A harmless native process with the dedicated-server filename must not
    # activate gaming mode, even after the detector's two-second cache expires.
    room = home / 'azahar-room'
    shutil.copyfile('/bin/sleep', room)
    room.chmod(0o700)
    process = subprocess.Popen([str(room), '30'], start_new_session=True)
    started = time.monotonic()
    try:
        wait_for(lambda: time.monotonic() - started > 3)
        assert process.poll() is None
        assert not invoke('gaming_status')['active']
    finally:
        process.terminate()
        process.wait(timeout=5)

    wait_for(lambda: js('return artworkTask===null'))
    js('''
      window.__polishInvoke=nativeInvoke;
      window.__polishQueue=state.coverQueue;
      window.__polishEnabled=state.artwork.enabled;
      window.__polishCovers=[];
      nativeInvoke=(name,args)=>name==='download_cover'&&args.uid.startsWith('polish-')
        ?new Promise(resolve=>window.__polishCovers.push({uid:args.uid,resolve}))
        :window.__polishInvoke(name,args);
      state.artwork.enabled=true;
      state.coverQueue={paused:false,items:['polish-a','polish-b'].map(uid=>({kind:'ps2',uid,title:uid,status:'pending'}))};
      window.__polishWork=scanArtwork(true);
      window.__polishSameWorker=Array.from({length:10},()=>scanArtwork(true)===window.__polishWork).every(Boolean);
      void handleAction({dataset:{action:'queue-pause'}});
      void handleAction({dataset:{action:'queue-resume'}});
    ''')
    try:
        assert js('return window.__polishSameWorker')
        assert js('return window.__polishCovers.length') == 1
        js('window.__polishCovers[0].resolve({status:"not-found"})')
        wait_for(lambda: js('return window.__polishCovers.length===2'))
        assert js('return window.__polishCovers.map(c=>c.uid)') == ['polish-a', 'polish-b']
        js('window.__polishCovers[1].resolve({status:"not-found"})')
        wait_for(lambda: js('return artworkTask===null'))
        assert js('return window.__polishCovers.length') == 2
    finally:
        js('for(const request of window.__polishCovers)request.resolve({status:"not-found"});state.coverQueue.paused=true')
        wait_for(lambda: js('return artworkTask===null'))
        js('nativeInvoke=window.__polishInvoke;state.coverQueue=window.__polishQueue;state.artwork.enabled=window.__polishEnabled')

    js('''
      window.__polishStats=[];
      nativeInvoke=(name,args)=>name==='playtime_statistics'
        ?new Promise((resolve,reject)=>window.__polishStats.push({resolve,reject}))
        :window.__polishInvoke(name,args);
      window.__polishOpeningDone=false;
      void openPlaytimeStatistics().then(()=>window.__polishOpeningDone=true);
      void loadStatistics();
    ''')
    try:
        wait_for(lambda: js('return window.__polishStats.length===2'))
        js('window.__polishStats[1].resolve({totals:{"gb|fixture":180000000},daily:{}})')
        wait_for(lambda: js('return document.querySelector("#statistics-content").textContent.includes("50 h played")'))
        js('window.__polishStats[0].reject(new Error("stale statistics failure"))')
        wait_for(lambda: js('return window.__polishOpeningDone'))
        assert '50 h played' in js('return document.querySelector("#statistics-content").textContent')
        screenshot('matrix-native-polished-statistics.png')
        click('.statistics-dialog .close')
    finally:
        js('nativeInvoke=window.__polishInvoke;dismissOverlay()')

    js('''
      window.__polishProgressCalls=0;window.__polishOpeningDone=false;
      nativeInvoke=(name,args)=>{
        if(name==='ra_status')return Promise.resolve({connected:true,username:'FixtureUser'});
        if(name==='ra_progress')return ++window.__polishProgressCalls===1
          ?new Promise((resolve,reject)=>window.__polishOldProgress=reject)
          :Promise.resolve({total:0,games:[],cached:false,warning:''});
        if(name==='ra_save_account')return new Promise(resolve=>window.__polishSave=resolve);
        return window.__polishInvoke(name,args);
      };
      void openRADashboard().then(()=>window.__polishOpeningDone=true);
    ''')
    try:
        wait_for(lambda: js('return window.__polishProgressCalls===1'))
        js('document.querySelector(".ra-account").open=true;document.querySelector("#ra-key").value="fixture-key-never-sent"')
        click('[data-action="ra-connect"]')
        wait_for(lambda: js('return typeof window.__polishSave==="function"'))
        js('window.__polishOldProgress(new Error("stale progress failure"))')
        wait_for(lambda: js('return window.__polishOpeningDone'))
        assert js('return document.querySelector(".achievements-dialog").dataset.saving') == 'true'
        assert js('return [...document.querySelectorAll(".ra-account input,.ra-account button")].every(c=>c.disabled)')
        js('window.__polishSave({connected:true,username:"FixtureUser"})')
        wait_for(lambda: js('return document.querySelector("#ra-content").textContent.includes("0 games on your account")'))
        assert js('return document.querySelector("#ra-key").value') == ''
        assert js('return [...document.querySelectorAll(".ra-account input,.ra-account button")].every(c=>!c.disabled)')
        click('.achievements-dialog .close')
    finally:
        js('nativeInvoke=window.__polishInvoke;dismissOverlay();delete window.__polishInvoke')
    print('PASS: native room-helper exclusion, single cover worker through repeated Resume and Pause/Resume, stale-statistics rejection and account-save control ownership (synthetic process/network fixtures)')
