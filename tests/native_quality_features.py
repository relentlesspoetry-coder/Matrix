"""Actual WebKit/IPC conveniences, using synthetic games and harmless launchers."""
import json
from pathlib import Path


def exercise(home, library, invoke, js, click, wait_for, screenshot):
    wait_for(lambda: js('return state.qualityDropListening===true'))
    # A native path is inspected and confirmed in the real UI. No files move.
    extra=home/'Dropped PS1 games'
    extra.mkdir()
    (extra/'Drop fixture.chd').write_bytes(b'Synthetic disc')
    js('goHome();void previewNativeDrop(arguments[0])',[str(extra)])
    wait_for(lambda: js('return state.overlay==="drop-preview"'))
    assert js('return document.querySelector("#drop-destination").textContent').startswith('Choose')
    js('document.querySelector("#drop-console").value="ps1";document.querySelector("#drop-console").dispatchEvent(new Event("change",{bubbles:true}))')
    click('[data-action="confirm-drop"]')
    wait_for(lambda: js('return state.page==="console"&&state.consoleId==="ps1"&&!state.overlay'))
    assert any(g['path']==str(extra/'Drop fixture.chd') for g in invoke('scan_console',{'id':'ps1'}))
    assert (extra/'Drop fixture.chd').read_bytes()==b'Synthetic disc'
    # Ordered playlist creation through the actual controls.
    discs=[extra/'Adventure (Disc 1).chd',extra/'Adventure (Disc 2).chd']
    for disc in discs: disc.write_bytes(b'Synthetic disc')
    js('void refreshNative()')
    wait_for(lambda: js('return gamesFor("ps1").some(g=>g.path===arguments[0])',str(discs[1])))
    js('openDiscCreator("ps1")')
    js('document.querySelector("#disc-title").value="Native Adventure"')
    for disc in discs:
        js('const box=[...document.querySelectorAll("[data-disc-uid]")].find(n=>n.dataset.discUid===arguments[0]);box.checked=true;box.dispatchEvent(new Event("change",{bubbles:true}))',str(disc))
    click('[data-action="save-disc-set"]')
    playlist=extra/'Native Adventure.m3u'
    wait_for(lambda: playlist.exists() and js('return !state.overlay'))
    assert playlist.read_text()=='#EXTM3U\nAdventure (Disc 1).chd\nAdventure (Disc 2).chd\n'
    assert invoke('disc_set',{'id':'ps1','uid':str(playlist)})['discs'][1]['path']==str(discs[1])
    js('openGame("ps1",arguments[0])',str(playlist))
    wait_for(lambda: js('return document.querySelectorAll("#starting-disc option").length===2'))
    js('dismissOverlay()')
    # Cover path validation and explicit replacement use the same normal writer.
    cover=Path(__file__).parent/'fixtures/demo-cover.png'
    # Fixtures are copied into the isolated test account along with this module.
    js('void previewNativeDrop(arguments[0],{id:"ps1",uid:arguments[1]})',[str(cover)],str(playlist))
    wait_for(lambda: js('return state.overlay==="drop-preview"'))
    click('[data-action="confirm-drop"]')
    wait_for(lambda: js('return !state.overlay'))
    assert invoke('get_cover',{'kind':'ps1','uid':str(playlist)}).startswith('data:image/png;base64,')
    # Seed a labelled 50-hour fixture, then accumulate a real short process session.
    gb=library/'Game Boy/Game Boy Games'
    gb.mkdir(parents=True,exist_ok=True)
    game=gb/'Pokemon Red.gb';game.write_bytes(b'Synthetic GB game')
    launcher=home/'Clock-fixture.sh';launcher.write_text('#!/bin/sh\nsleep 3\n');launcher.chmod(0o700)
    invoke('save_emulator',{'id':'gb','emulatorName':'Clock fixture','emulatorPath':str(launcher),'launchArgs':['{game}']})
    key='gb|'+str(game)
    settings_file=home/'config/matrix/frontend.json'
    settings=json.loads(settings_file.read_text());settings.setdefault('playtime',{})[key]=50*3600000;settings_file.write_text(json.dumps(settings))
    invoke('launch_console_game',{'id':'gb','uid':str(game)})
    wait_for(lambda: invoke('get_playtime').get(key,0)>50*3600000+2000)
    total=invoke('get_playtime')[key]
    assert 50*3600000+2000<=total<50*3600000+6000,total
    js('void refreshNative().then(()=>{goConsole("gb");return pollPlaytime()})')
    wait_for(lambda: js('return document.querySelector(".game-title .game-playtime")?.textContent==="50 h played"'))
    screenshot('matrix-native-playtime.png')
    js('goSettings();document.querySelector(".display-options").open=true')
    for section,value in [('uiScale','130'),('coverSize','large'),('controllerPrompts','playstation')]:
        js('const field=document.querySelector(`[data-display-pref="${arguments[0]}"]`);field.value=arguments[1];field.dispatchEvent(new Event("change",{bubbles:true}))',section,value)
        wait_for(lambda: js('return !document.querySelector(`[data-display-pref="${arguments[0]}"]`).disabled',section))
    prefs=invoke('bootstrap')['libraryPreferences']
    assert prefs['uiScale']==130 and prefs['coverSize']=='large' and prefs['controllerPrompts']=='playstation',prefs
    assert js('return document.documentElement.style.zoom')=='1.3'
    assert invoke('set_app_fullscreen',{'fullscreen':True}) is True
    assert invoke('set_app_fullscreen',{'fullscreen':False}) is False
    click('[data-action="reset-display"]')
    wait_for(lambda: js('return document.documentElement.style.zoom==="1"'))
    assert invoke('get_playtime')[key]==total
    print('PASS: native drop confirmation, unchanged game files, ordered disc creation/selection, dropped cover import, persistent session playtime and display/fullscreen preferences (synthetic fixtures)')
