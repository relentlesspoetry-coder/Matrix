import base64
import os
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / 'web'
HTML = (WEB / 'index.html').read_text().replace('<link rel="stylesheet" href="./styles.css">', '<style>' + (WEB / 'styles.css').read_text() + '</style>')
HTML = HTML.replace('<script src="./data.js" defer></script>', '').replace('<script src="./app.js" defer></script>', '')
HTML = HTML.replace('<script src="./startup.js" defer></script>', '').replace('<script src="./gamepad.js" defer></script>', '')
for name in ['matrix-play-logo.png','matrix-play-icon.png']:
    uri='data:image/png;base64,'+base64.b64encode((WEB/'assets'/name).read_bytes()).decode()
    HTML=HTML.replace('./assets/'+name,uri)
APP = (WEB / 'app.js').read_text()
for asset in (WEB/'assets/consoles').glob('*.webp'):
    APP = APP.replace('./assets/consoles/'+asset.name, 'data:image/webp;base64,'+base64.b64encode(asset.read_bytes()).decode())
HTML = HTML.replace('<link rel="stylesheet" href="./themes.css">','<style>'+(WEB/'themes.css').read_text()+'</style>')
typography = (WEB/'typography.css').read_text().replace('./assets/fonts/SoraVariable.ttf', 'data:font/ttf;base64,'+base64.b64encode((WEB/'assets/fonts/SoraVariable.ttf').read_bytes()).decode())
HTML = HTML.replace('<link rel="stylesheet" href="./typography.css">', '<style>'+typography+'</style>')
HTML = HTML.replace('<link rel="preload" href="./assets/fonts/SoraVariable.ttf" as="font" type="font/ttf" crossorigin>', '')
HTML = HTML.replace('</body>', '<script>' + (WEB / 'startup.js').read_text() + '</script>' + '<script>' + (WEB / 'data.js').read_text() + '</script><script>' + APP + '</script><script>' + (WEB / 'gamepad.js').read_text() + '</script></body>')
SCREENS = ROOT / 'screenshots'
SCREENS.mkdir(exist_ok=True)
MOCK = '''
window.__calls = [];
window.__update={currentVersion:'0.5.2',latestVersion:null,phase:'idle',message:'Check for updates',notes:'',received:0,total:0,installMode:'system'};
window.__favorites={};window.__migrated=false;
window.__failLaunch=false;window.__lastLaunch=null;
window.__discovery={linked:[],ambiguous:[],notices:[],candidates:[{id:'flatpak|net.pcsx2.PCSX2|system',consoleIds:['ps2'],ready:true,source:'Flatpak (system)',profile:{emulatorName:'PCSX2',launcher:'flatpak',flatpakId:'net.pcsx2.PCSX2',flatpakInstallation:'system',launchArgs:['--','{game}']}}]};
window.__artSaved = false;
window.__artOptions={enabled:true,style:"default"};
window.__TAURI__ = { core: { invoke: async (name,args={}) => {
  window.__calls.push([name,args]);
  const ids = ['switch','ps1','ps2','ps3','psp','n64','gamecube','dreamcast','wii','wiiu','gb','gba','nds','3ds','ps4','ps5','vita','switch2'];
  if(name==='update_status') return window.__update;
  if(name==='check_updates') return (window.__update={...window.__update,latestVersion:'0.6.0',phase:'available',message:'A newer Matrix Play release is available.',notes:'Console artwork and updates.'});
  if(name==='download_update') return (window.__update={...window.__update,phase:'downloaded',message:'Verified update downloaded.'});
  if(name==='install_update') return (window.__update={...window.__update,phase:'installing',message:'Closing Matrix Play to install the update…'});
  if(name==='discover_emulators') return window.__discovery;
  if(name==='bootstrap') return {emulatorDiscovery:window.__discovery,favorites:window.__favorites,favoritesMigrated:window.__migrated,artwork:window.__artOptions,root:'/home/example/Games/Matrix',folders:Object.fromEntries(ids.map(id=>[id,`/home/example/Games/Matrix/${id}/${id} Games`])),profiles:{ps2:{emulatorName:'PCSX2',emulatorPath:'/usr/bin/pcsx2',launchArgs:['{game}']}}};
  if(name==='scan_console' && args.id==='ps5') return [{uid:'/collection/PS5/Collection.pkg',title:'PS5 Collection',path:'/collection/PS5/Collection.pkg',kind:'ps5'}];
  if(name==='scan_console') return args.id==='ps2' ? [{uid:'/home/example/Games/Matrix/PS2/PS2 Games/Test.iso',title:'Test Game',path:'/home/example/Games/Matrix/PS2/PS2 Games/Test.iso',kind:'ps2'}] : [];
  if(name==='save_artwork_settings') {window.__artOptions={enabled:args.enabled,style:args.style};return window.__artOptions;}
  if(name==='download_cover') return args.serial ? {status:'downloaded',message:'Cover saved for offline use',serial:args.serial,source:'user-confirmed',url:'https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default/SLUS-20312.jpg'} : {status:'needs-serial',message:'No serial found; confirm a serial in game details'};
  if(name==='get_cover') return args.kind==='ps2' ? 'data:image/png;base64,__COVER__' : null;
  if(name==='save_cover') {window.__artSaved=true;return;}
  if(name==='import_favorites') {window.__migrated=true;return window.__favorites;}
  if(name==='set_favorite') {const set=new Set(window.__favorites[args.id] || []);if(args.favorite)set.add(args.uid);else set.delete(args.uid);return window.__favorites[args.id]=[...set];}
  if(name==='list_flatpaks') return [{appId:'org.example.Emulator',name:'Example Emulator',installation:'user'},{appId:'net.pcsx2.PCSX2',name:'PCSX2',installation:'system'}];
  if(name==='get_launch_status') return window.__lastLaunch;
  if(name==='launch_console_game' || name==='open_emulator') return window.__lastLaunch={consoleId:args.id,phase:window.__failLaunch && name==='launch_console_game'?'error':'exited',message:window.__failLaunch && name==='launch_console_game'?'Emulator exited with exit status 17.':'Emulator launcher completed.',log:window.__failLaunch && name==='launch_console_game'?'bwrap: missing runtime <script>bad()</script>':'',logPath:'/tmp/test-launch.log'};
  if(name==='browse_emulator') return '/home/example/Apps/Example.AppImage';
  if(name==='save_emulator') return;
  return;
}}};
'''

MOCK = MOCK.replace('__COVER__', base64.b64encode((ROOT/'tests/fixtures/demo-cover.png').read_bytes()).decode('ascii'))

def screenshot(page,name):
    page.evaluate('async()=>{await Promise.all([...document.images].map(i=>i.decode()));await new Promise(requestAnimationFrame);}');
    page.screenshot(path=str(SCREENS/name),full_page=True)

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True, executable_path=os.environ.get('MATRIX_CHROMIUM'),args=['--no-sandbox','--disable-dev-shm-usage'])
    errors=[]
    preview = browser.new_page(viewport={'width':1440,'height':940})
    preview.on('pageerror',lambda e: errors.append(str(e)))
    preview.set_content(HTML)
    assert preview.locator('.console-card').count()==24
    assert preview.locator('#startup-status').is_hidden()
    assert preview.get_by_role('button',name='Matrix-Play home').count()==1
    assert preview.locator('.brand-logo').get_attribute('alt')=='Matrix-Play'
    preview.wait_for_function('document.querySelector(".brand-logo").naturalWidth > 0')
    assert preview.locator('#nav-steam,#nav-windows').count()==0
    screenshot(preview,'matrix-console-preview.png')

    failed = browser.new_page(viewport={'width':1000,'height':800})
    failed.set_content(HTML.replace(APP, "throw new Error('Injected startup failure');"))
    failed.wait_for_function('document.querySelector("#startup-status strong").textContent.includes("could not finish")')
    assert 'Injected startup failure' in failed.locator('#startup-status').inner_text()
    assert failed.get_by_role('button',name='Try again').is_visible()
    failed.close()

    native = browser.new_page(viewport={'width':1440,'height':940})
    native.evaluate(MOCK)
    native.on('pageerror',lambda e: errors.append(str(e)))
    native.set_content(HTML)
    native.wait_for_function('document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX" && document.querySelector(".console-card__bottom small")?.textContent?.includes("games")')
    assert native.locator('.console-card').count()==24
    native.wait_for_function('document.querySelectorAll(".console-card").length === consoles.length && [...document.querySelectorAll(".console-card")].every(card => { const image = card.querySelector(".console-image"); return image?.complete && image.naturalWidth > 0; })')
    screenshot(native,'matrix-native-home.png')
    for platform, id in [('PlayStation 5', 'ps5'), ('Nintendo Switch 2', 'switch2')]:
        native.get_by_role('button',name=f'Browse {platform} library').click()
        native.locator('.library-notes > summary').click()
        assert 'Experimental custom emulator support' in native.locator('.platform-notice').inner_text()
        native.locator('.library-header [data-action="configure"]').click()
        assert native.locator('.advanced-setup').get_attribute('open') is not None
        assert '{gameDir}' in native.locator('.advanced-setup').inner_text()
        assert 'without arguments' in native.locator('.emulator-dialog > p').inner_text()
        native.locator('#emulator-launcher').select_option('executable')
        native.locator('#emulator-path').fill('/home/example/Apps/My Emulator')
        native.locator('#emulator-name').fill('My custom emulator')
        args='--game\n{gameDir}' if id=='ps5' else '-f\n-g\n{game}'
        native.locator('#launch-args').fill(args)
        native.get_by_role('button',name='Save emulator',exact=True).click()
        native.wait_for_function('document.querySelector("#config-form")===null')
        saved=native.evaluate('window.__calls.filter(c=>c[0]==="save_emulator").at(-1)[1]')
        assert saved['id']==id and saved['launchArgs']==args.split('\n')
        native.get_by_label('Library options',exact=True).click()
        native.get_by_role('button',name='Open emulator',exact=True).click()
        native.wait_for_function('(id)=>window.__calls.some(c=>c[0]==="open_emulator" && c[1].id===id)',arg=id)
        screenshot(native,f'matrix-custom-{id}.png')
        native.locator('#nav-consoles').click()
    native.get_by_role('button',name='Browse PlayStation Vita library').click()
    native.locator('.library-notes > summary').click()
    assert 'SDL frontend' in native.locator('.platform-notice').inner_text()
    native.locator('#nav-consoles').click()

    # Keep a real polling interval active while checking DOM/focus ownership.
    native.evaluate("""() => {
      window.__originalInvoke=window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke=(name,args)=>{
        if(name==='download_update') {
          window.__update={...window.__update,phase:'downloading',message:'Downloading',total:100,received:10};
          return new Promise(resolve=>window.__finishDownload=()=>resolve(window.__update={...window.__update,phase:'downloaded',message:'Verified update downloaded.'}));
        }
        return window.__originalInvoke(name,args);
      };
    }""")
    native.locator('#nav-settings').click()
    native.get_by_role('button',name='Update Matrix-Play').click()
    native.get_by_role('button',name='Download update').click()
    native.get_by_role('button',name='Cancel download').focus()
    native.evaluate("window.__focusedCancel=document.activeElement")
    native.wait_for_timeout(1200)
    assert native.evaluate("document.activeElement===window.__focusedCancel && document.activeElement.isConnected")
    assert native.evaluate("document.querySelector('.app-shell').inert")
    native.keyboard.press('Tab')
    assert native.get_by_role('button',name='Close updates').evaluate('(node)=>document.activeElement===node')
    native.keyboard.press('Escape')
    assert not native.evaluate("document.querySelector('.app-shell').inert")
    native.evaluate("window.__finishDownload(); window.__TAURI__.core.invoke=window.__originalInvoke")
    native.wait_for_timeout(100)
    assert native.locator('.update-dialog').count()==0
    native.locator('#nav-settings').click()
    native.get_by_role('button',name='Update Matrix-Play').click()
    native.locator('#update-games-closed').check()
    screenshot(native,'matrix-update-shutdown-confirmation.png')
    assert 'stays closed when it succeeds' in native.locator('#update-hint').inner_text()
    native.get_by_role('button',name='Agree and update').click()
    assert native.locator('.update-message').inner_text()=='Closing Matrix Play to install the update…'
    assert native.get_by_role('button',name='Restart Matrix Play').count()==0
    screenshot(native,'matrix-updates.png')
    native.keyboard.press('Escape')
    native.locator('#nav-consoles').click()
    native.get_by_role('button',name='Browse PlayStation 2 library').click()
    assert native.locator('.game-item').count()==1
    native.get_by_role('button',name='Add Test Game to favorites',exact=True).click()
    native.get_by_role('button',name='Favorites (1)').click()
    assert native.locator('.game-item').count()==1
    screenshot(native,'matrix-console-favorites.png')
    assert native.evaluate('window.__favorites.ps2.length')==1
    native.get_by_role('button',name='Remove Test Game from favorites',exact=True).click()
    native.wait_for_function('document.querySelector(".empty-library strong")?.textContent.includes("favorites yet")')
    native.get_by_role('button',name='Browse all games').click()
    screenshot(native,'matrix-native-ps2.png')
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
    native.get_by_role('button',name='Find exact cover').click()
    native.wait_for_function('document.querySelector(".cover-result").textContent.includes("user-confirmed")')
    screenshot(native,'matrix-cover-details.png')
    native.get_by_role('button',name='Open TheCoverProject').click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="open_cover_project")')
    native.evaluate("""() => {
      window.__originalInvoke=window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke=(name,args)=>name==='save_cover'
        ? new Promise(resolve=>window.__finishCover=()=>{window.__artSaved=true;resolve();})
        : window.__originalInvoke(name,args);
    }""")
    native.get_by_role('button', name='Choose cover').click()
    native.locator('#art-input').set_input_files(str(ROOT/'tests/fixtures/demo-cover.png'))
    native.wait_for_function("typeof window.__finishCover==='function'")
    native.keyboard.press('Escape')
    assert native.locator('#nav-steam,#nav-windows').count()==0
    native.locator('#nav-settings').click()
    assert native.locator('.settings-row [data-action="library-folders"]').count()==24
    screenshot(native,'matrix-cover-settings.png')
    native.locator('#cover-style').select_option('3d')
    native.wait_for_function('window.__artOptions.style === "3d"')
    native.locator('#auto-covers').uncheck()
    native.wait_for_function('window.__artOptions.enabled === false')
    before=native.evaluate('window.__calls.filter(x=>x[0]==="download_cover").length')
    native.get_by_role('button',name='Refresh library',exact=True).click()
    native.wait_for_timeout(150)
    assert native.evaluate('window.__calls.filter(x=>x[0]==="download_cover").length')==before
    native.get_by_role('button', name='Edit').click()
    assert native.locator('#library-path').get_attribute('readonly') is not None
    assert native.locator('#launch-args').count() == 1
    native.keyboard.press('Escape')
    native.get_by_role('button',name='Find emulators',exact=True).click()
    native.wait_for_function('window.__calls.filter(x=>x[0]==="bootstrap").length >= 3')
    native.get_by_role('button',name='Edit',exact=True).click()
    native.locator('#detected-emulator').select_option('flatpak|net.pcsx2.PCSX2|system')
    native.wait_for_function('document.querySelector("#flatpak-app").value==="net.pcsx2.PCSX2|system"')
    assert native.locator('#runtime-fonts').is_checked()
    assert native.locator('#launch-args').input_value()=='--\n{game}'
    native.locator('#emulator-launcher').select_option('appimage')
    native.get_by_role('button',name='Browse…').click()
    native.wait_for_function('document.querySelector("#emulator-path").value.includes("Example.AppImage")')
    assert native.locator('#allow-execute').is_checked()
    screenshot(native,'matrix-appimage-setup.png')
    native.locator('#emulator-launcher').select_option('flatpak')
    native.wait_for_function('document.querySelector("#flatpak-app").options.length===3')
    native.locator('#flatpak-app').select_option('org.example.Emulator|user')
    assert native.locator('#emulator-name').input_value()=='Example Emulator'
    assert native.locator('#emulator-path').is_hidden()
    screenshot(native,'matrix-flatpak-setup.png')
    native.get_by_role('button',name='Save emulator',exact=True).click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="save_emulator" && x[1].launcher==="flatpak" && x[1].flatpakId==="org.example.Emulator" && x[1].flatpakInstallation==="user")')
    native.locator('#nav-consoles').click()
    native.get_by_role('button',name='Browse PlayStation 2 library').click()
    native.get_by_label('Library options',exact=True).click()
    assert native.get_by_role('button',name='Open emulator',exact=True).is_visible()
    native.keyboard.press('Escape')
    native.evaluate('window.__failLaunch=true')
    native.locator('.game-card').first.click()
    native.wait_for_function('document.querySelector(".launch-error")?.textContent.includes("exit status 17")')
    native.get_by_role('button',name='Launch details',exact=True).click()
    assert '<script>bad()</script>' in native.locator('.launch-output').inner_text()
    screenshot(native,'matrix-launch-error.png')
    native.locator('.launch-dialog').get_by_role('button',name='Open emulator',exact=True).click()
    native.wait_for_function('window.__calls.some(x=>x[0]==="open_emulator" && x[1].id==="ps2")')
    native.wait_for_function('!document.querySelector(".launch-dialog")')
    native.get_by_label('Library options',exact=True).click()
    native.get_by_role('button',name='Edit emulator',exact=True).click()
    native.get_by_role('button',name='Save and open emulator',exact=True).click()
    native.wait_for_function('window.__calls.filter(x=>x[0]==="open_emulator").length>=2')


    mobile = browser.new_page(viewport={'width':390,'height':844},device_scale_factor=1,is_mobile=True,has_touch=True)
    mobile.evaluate(MOCK)
    mobile.on('pageerror',lambda e: errors.append(str(e)))
    mobile.set_content(HTML)
    mobile.wait_for_function('document.querySelector("#mode-indicator")?.textContent === "NATIVE LINUX"')
    screenshot(mobile,'matrix-mobile.png')
    assert mobile.evaluate('document.documentElement.scrollWidth <= window.innerWidth'), 'Mobile horizontal overflow'
    assert not errors, errors
    print('PASS: 24 consoles, 3 libraries, launch dispatch, artwork status/serial/styles/disable/manual import, Windows registration, mobile layout (mock native bridge)')
    browser.close()
