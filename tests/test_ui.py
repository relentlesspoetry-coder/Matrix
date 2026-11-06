import base64
import os
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / 'web'
HTML = (WEB / 'index.html').read_text().replace('<link rel="stylesheet" href="./styles.css">', '<style>' + (WEB / 'styles.css').read_text() + '</style>')
HTML = HTML.replace('<script src="./data.js" defer></script>', '').replace('<script src="./app.js" defer></script>', '')
HTML = HTML.replace('<script src="./startup.js" defer></script>', '')
for name in ['matrix-play-logo.png','matrix-play-icon.png']:
    uri='data:image/png;base64,'+base64.b64encode((WEB/'assets'/name).read_bytes()).decode()
    HTML=HTML.replace('./assets/'+name,uri)
HTML = HTML.replace('</body>', '<script>' + (WEB / 'startup.js').read_text() + '</script>' + '<script>' + (WEB / 'data.js').read_text() + '</script><script>' + (WEB / 'app.js').read_text() + '</script></body>')
SCREENS = ROOT / 'screenshots'
SCREENS.mkdir(exist_ok=True)
MOCK = '''
window.__calls = [];
window.__artSaved = false;
window.__artOptions={enabled:true,style:"default"};
window.__windows = [{uid:'/games/My Windows Game.desktop',title:'My Windows Game',target:'/games/My Windows Game.desktop',runner:'desktop'}];
window.__TAURI__ = { core: { invoke: async (name,args={}) => {
  window.__calls.push([name,args]);
  const ids = ['switch','ps1','ps2','ps3','psp','n64','gamecube','dreamcast','wii','wiiu','gb','gba','nds','3ds'];
  if(name==='bootstrap') return {artwork:window.__artOptions,root:'/home/example/Games/Matrix',folders:Object.fromEntries(ids.map(id=>[id,`/home/example/Games/Matrix/${id}/${id} Games`])),profiles:{ps2:{emulatorName:'PCSX2',emulatorPath:'/usr/bin/pcsx2',launchArgs:['{game}']}},windows:window.__windows};
  if(name==='scan_console') return args.id==='ps2' ? [{uid:'/home/example/Games/Matrix/PS2/PS2 Games/Test.iso',title:'Test Game',path:'/home/example/Games/Matrix/PS2/PS2 Games/Test.iso',kind:'ps2'}] : [];
  if(name==='scan_steam') return [{uid:'12345',title:'Test Steam Game',kind:'steam',appId:'12345'}];
  if(name==='list_windows_games') return window.__windows;
  if(name==='add_windows_game') { const row={uid:args.target,title:args.title,target:args.target,runner:args.runner};window.__windows.push(row);return row;}
  if(name==='remove_windows_game'){window.__windows=window.__windows.filter(x=>x.uid!==args.uid);return;}
  if(name==='save_artwork_settings') {window.__artOptions={enabled:args.enabled,style:args.style};return window.__artOptions;}
  if(name==='download_cover') return args.serial ? {status:'downloaded',message:'Cover saved for offline use',serial:args.serial,source:'user-confirmed',url:'https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default/SLUS-20312.jpg'} : {status:'needs-serial',message:'No serial found; confirm a serial in game details'};
  if(name==='get_cover') return args.kind==='ps2' ? 'data:image/png;base64,__COVER__' : null;
  if(name==='save_cover') {window.__artSaved=true;return;}
  if(name==='save_emulator') return;
  return;
}}};
'''

MOCK = MOCK.replace('__COVER__', base64.b64encode((ROOT/'tests/fixtures/demo-cover.png').read_bytes()).decode('ascii'))

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True, executable_path=os.environ.get('MATRIX_CHROMIUM'),args=['--no-sandbox','--disable-dev-shm-usage'])
    errors=[]
    preview = browser.new_page(viewport={'width':1440,'height':940})
    preview.on('pageerror',lambda e: errors.append(str(e)))
    preview.set_content(HTML)
    assert preview.locator('.console-card').count()==14
    assert preview.locator('#startup-status').is_hidden()
    assert preview.get_by_role('button',name='Matrix Play home').count()==1
    assert preview.locator('.brand-logo').get_attribute('alt')=='Matrix Play'
    preview.wait_for_function('document.querySelector(".brand-logo").naturalWidth > 0')
    preview.get_by_role('button', name='Steam Games').click()
    assert 'No steam games detected' in preview.locator('.external-empty').inner_text()
    preview.get_by_role('button', name='Windows Games').click()
    preview.get_by_role('button', name='Add Windows game').click()
    assert preview.locator('#windows-form').count()==1
    preview.keyboard.press('Escape')
    preview.get_by_role('button', name='Consoles').click()
    preview.screenshot(path=str(SCREENS/'matrix-console-preview.png'),full_page=True)

    failed = browser.new_page(viewport={'width':1000,'height':800})
    failed.set_content(HTML.replace((WEB/'app.js').read_text(), "throw new Error('Injected startup failure');"))
    failed.wait_for_function('document.querySelector("#startup-status strong").textContent.includes("could not finish")')
    assert 'Injected startup failure' in failed.locator('#startup-status').inner_text()
    assert failed.get_by_role('button',name='Try again').is_visible()
    failed.close()

    native = browser.new_page(viewport={'width':1440,'height':940})
    native.evaluate(MOCK)
    native.on('pageerror',lambda e: errors.append(str(e)))
    native.set_content(HTML)
    native.wait_for_function('document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX" && document.querySelector(".console-card__bottom small")?.textContent?.includes("games")')
    native.screenshot(path=str(SCREENS/'matrix-native-home.png'),full_page=True)
    assert native.locator('.console-card').count()==14
    native.get_by_role('button',name='Browse PlayStation 2 library').click()
    assert native.locator('.game-item').count()==1
    native.screenshot(path=str(SCREENS/'matrix-native-ps2.png'),full_page=True)
    native.wait_for_function('window.__calls.some(x=>x[0]==="get_cover" && x[1].kind==="ps2")')
    assert native.locator('.game-cover img').count()>=1
    native.locator('.game-card').first.click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="launch_console_game")')
    assert native.locator('.detail-panel').count()==0
    native.get_by_role('button',name='Details for Test Game').click()
    assert native.get_by_role('button',name='Play game').count()==1
    native.wait_for_function('window.__calls.some(x=>x[0]==="download_cover" && x[1].kind==="ps2")')
    assert 'No serial found' in native.locator('.cover-result').inner_text()
    native.locator('#cover-serial').fill('SLUS-20312')
    native.get_by_role('button',name='Find cover by serial').click()
    native.wait_for_function('document.querySelector(".cover-result").textContent.includes("user-confirmed")')
    native.screenshot(path=str(SCREENS/'matrix-cover-details.png'),full_page=True)
    native.get_by_role('button',name='Open TheCoverProject').click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="open_cover_project")')
    native.get_by_role('button', name='Choose cover').click()
    native.locator('#art-input').set_input_files(str(ROOT/'tests/fixtures/demo-cover.png'))
    native.wait_for_function('window.__artSaved === true')
    native.keyboard.press('Escape')
    native.get_by_role('button',name='Steam Games').click()
    assert native.locator('.game-item').count()==1
    native.screenshot(path=str(SCREENS/'matrix-native-steam.png'),full_page=True)
    native.locator('.game-card').first.click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="launch_steam_game")')
    native.get_by_role('button',name='Windows Games').click()
    assert native.locator('.game-item').count()==1
    native.locator('.game-card').first.click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="launch_windows_game")')
    native.screenshot(path=str(SCREENS/'matrix-native-windows.png'),full_page=True)
    native.get_by_role('button',name='Add Windows game').click()
    native.locator('#windows-name').fill('Second Windows Game')
    native.locator('#windows-target').fill('/games/Second Game.exe')
    native.locator('#windows-runner').select_option('wine')
    native.get_by_role('button',name='Add game').click()
    native.wait_for_function('window.__windows.length === 2')
    assert native.locator('.game-item').count()==2
    native.get_by_role('button',name='Emulators & folders').click()
    assert native.locator('.root-path').count()==1
    assert '/home/example/Games/Matrix' in native.locator('.root-path').inner_text()
    native.screenshot(path=str(SCREENS/'matrix-cover-settings.png'),full_page=True)
    native.locator('#cover-style').select_option('3d')
    native.wait_for_function('window.__artOptions.style === "3d"')
    native.locator('#auto-covers').uncheck()
    native.wait_for_function('window.__artOptions.enabled === false')
    before=native.evaluate('window.__calls.filter(x=>x[0]==="download_cover").length')
    native.get_by_role('button',name='Refresh',exact=True).click()
    native.wait_for_timeout(150)
    assert native.evaluate('window.__calls.filter(x=>x[0]==="download_cover").length')==before
    native.get_by_role('button', name='Edit').click()
    assert native.locator('#library-path').get_attribute('readonly') is not None
    assert native.locator('#launch-args').count() == 1
    native.keyboard.press('Escape')

    mobile = browser.new_page(viewport={'width':390,'height':844},device_scale_factor=1,is_mobile=True,has_touch=True)
    mobile.evaluate(MOCK)
    mobile.on('pageerror',lambda e: errors.append(str(e)))
    mobile.set_content(HTML)
    mobile.wait_for_function('document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX"')
    mobile.screenshot(path=str(SCREENS/'matrix-mobile.png'),full_page=True)
    assert mobile.evaluate('document.documentElement.scrollWidth <= window.innerWidth'), 'Mobile horizontal overflow'
    assert not errors, errors
    print('PASS: 14 consoles, 3 libraries, launch dispatch, artwork status/serial/styles/disable/manual import, Windows registration, mobile layout (mock native bridge)')
    browser.close()
