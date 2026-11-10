// Real UI functions and console catalog, with a small DOM/IPC fixture.
// This does not emulate games or certify an external emulator build.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
const tick = () => new Promise(resolve => setImmediate(resolve));
function fixture() {
  const nodes = new Map(), listeners = {}, calls = [], messages = [];
  const document = { activeElement: null, addEventListener(name, fn) { (listeners[name] ||= []).push(fn); },
    querySelector(selector) { return nodes.get(selector) || null; } };
  const node = selector => {
    if (!nodes.has(selector)) nodes.set(selector, { dataset: {}, value: '', textContent: '', hidden: false,
      get elements() { return Object.entries(this.values || {}).map(([name,value])=>({name,value,type:'text'})); },
      focus() { document.activeElement = this; }, querySelectorAll() { return []; } });
    return nodes.get(selector);
  };
  for (const selector of ['#overlay-root', '#emulator-launcher', '#file-fields', '#flatpak-fields', '#appimage-options', '#emulator-path', '#flatpak-app', '#emulator-file-label', '#emulator-file-hint', '#config-error', '#config-save-status']) node(selector);
  node('#emulator-launcher').value = 'executable';
  const context = vm.createContext({ document,
    FormData: class { constructor(form) { this.values = form.values; } get(key) { return this.values[key]; } has(key) { return key in this.values; } },
    window: { __TAURI__: { core: { invoke(name, args) {
      calls.push({ name, args });
      return Promise.resolve(name === 'save_emulator' ? undefined : { phase: 'exited', message: 'Fixture completed.' });
    } } } }, setTimeout() {}, clearTimeout() {}, localStorage: { setItem() {} },
    onToast: message => messages.push(message),
  });
  const data = fs.readFileSync(path.join(__dirname, '../web/data.js'), 'utf8');
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(data + '\n' + source.replace(startup, '') + `
    toast=onToast;render=()=>{};setupEvents();
    globalThis.api={state,byId,isConfigured,openConfigure,launchConsole,renderConsole,renderSettings,importFolder};
  `, context);
  context.api.state.native = true;
  return { ...context.api, node, nodes, calls, listeners, messages };
}

test('PS5 folder preview matches exact eboot scanning and uses the title folder', () => {
  const f=fixture();f.state.consoleId='ps5';f.state.native=false;
  f.importFolder([
    {name:'EBOOT.BIN',webkitRelativePath:'PS5/An.Extracted_Title/EBOOT.BIN'},
    {name:'data.bin',webkitRelativePath:'PS5/An.Extracted_Title/data.bin'},
    {name:'Archive.zar',webkitRelativePath:'PS5/Archive.zar'},
    {name:'Legacy.pkg',webkitRelativePath:'PS5/Legacy.pkg'},
  ]);
  assert.deepEqual(Array.from(f.state.imported.ps5,g=>g.title),['An Extracted Title','Archive','Legacy']);
  assert.equal(f.state.imported.ps5[0].uid,'file:PS5/An.Extracted_Title/EBOOT.BIN');
});
test('Switch 2 preview recognizes NUP case variants without adding them to Switch', () => {
  const f=fixture();f.state.consoleId='switch2';f.state.native=false;
  const files=[
    {name:'Lower.nup',webkitRelativePath:'Switch 2/Nested/Lower.nup'},
    {name:'Upper.NUP',webkitRelativePath:'Switch 2/Nested/Upper.NUP'},
    {name:'Mixed.NuP',webkitRelativePath:'Switch 2/Nested/Mixed.NuP'},
    {name:'Existing.nsp',webkitRelativePath:'Switch 2/Existing.nsp'},
    {name:'Existing.xci',webkitRelativePath:'Switch 2/Existing.xci'},
    {name:'Not a package.nup.txt',webkitRelativePath:'Switch 2/Not a package.nup.txt'},
  ];
  f.importFolder(files);
  assert.deepEqual(Array.from(f.state.imported.switch2,g=>g.title),['Lower','Upper','Mixed','Existing','Existing']);
  assert.equal(f.state.imported.switch2[1].uid,'file:Switch 2/Nested/Upper.NUP');
  assert.match(f.byId.switch2.supportNote,/NUP\/NSP\/XCI/);
  assert.match(f.byId.switch2.supportNote,/does not verify their contents or playability/);
  f.state.consoleId='switch';f.importFolder(files);
  assert.deepEqual(Array.from(f.state.imported.switch,g=>g.title),['Existing','Existing']);
});

test('Switch 2 previews extracted base and update boot files using game names and exact paths', () => {
  const f=fixture();f.state.consoleId='switch2';f.state.native=false;
  const paths=[
    'Switch 2 Games/Donkey Kong Bananza/updates/458752/exefs/main',
    'Switch 2 Games/Pokémon™ Pokopia/updates/655360/exefs/main',
    'Switch 2 Games/Pokémon™ Pokopia/base/exefs/main',
    'Switch 2 Games/Loose.Title/EXEFS/MAIN',
    'Switch 2 Games/Pokémon™ Pokopia/updates/655360/exefs/main',
    'Switch 2 Games/Unrelated/main',
    'Switch 2 Games/Loose.Title/exefs/main.npdm',
    'Switch 2 Games/Loose.Title/exefs/subsdk0',
    'Switch 2 Games/Loose.Title/romfs/main',
    'Switch 2 Games/Loose.Title/romfs/Data.nro',
    'Switch 2 Games/.Hidden/exefs/main',
  ];
  const files=paths.map(p=>({name:p.split('/').at(-1),webkitRelativePath:p}));
  f.importFolder(files);
  assert.deepEqual(Array.from(f.state.imported.switch2,g=>g.title),[
    'Donkey Kong Bananza — Update 458752','Pokémon™ Pokopia — Update 655360',
    'Pokémon™ Pokopia — Base','Loose Title',
  ]);
  assert.equal(f.state.imported.switch2[0].uid,'file:'+paths[0]);
  for (const id of ['switch','ps5','ps4']) {
    f.state.consoleId=id;f.importFolder(files);
    assert.equal(f.state.imported[id]?.length || 0,0,'ExeFS detection is Switch 2 only');
  }
});

test('Switch 2 previews Lichii file types case insensitively and keeps existing package identities', () => {
  const f=fixture();f.state.consoleId='switch2';f.state.native=false;
  const extensions=['nup','nsp','xci','nsz','xcz','nx','nro','nso','nca','kip'];
  f.importFolder(extensions.flatMap(ext=>[
    {name:'A.'+ext,webkitRelativePath:'Games/A.'+ext},
    {name:'B.'+ext.toUpperCase(),webkitRelativePath:'Games/B.'+ext.toUpperCase()},
  ]).concat([{name:'Not.nup.txt',webkitRelativePath:'Games/Not.nup.txt'}]));
  assert.equal(f.state.imported.switch2.length,20);
  assert.equal(f.state.imported.switch2[0].uid,'file:Games/A.nup');
  assert.equal(f.state.imported.switch2[1].uid,'file:Games/B.NUP');
  assert.match(f.byId.switch2.supportNote,/exefs\/main/);
  assert.match(f.byId.switch2.supportNote,/separate, labelled entries/);
});

for (const id of ['ps5', 'switch2']) {
  test(`${id} exposes custom setup and saves literal launch arguments`, async () => {
    const f = fixture();
    assert.equal(f.byId[id].libraryOnly, undefined);
    assert.equal(f.byId[id].customEmulator, true);
    f.state.folders[id] = '/games';
    f.openConfigure(id);
    const html = f.node('#overlay-root').innerHTML;
    assert.match(html, /advanced-setup" open/);
    assert.match(html, /\{gameDir\}/);
    assert.match(html, /without arguments/);
    assert.match(html, /not a game-compatibility guarantee/);
    const form = f.node('#config-form');form.id='config-form';form.dataset.id=id;
    form.values={emulatorName:'My Emulator', emulatorPath:'/apps/My Emulator', launcher:'executable', folderPath:'/games', launchArgs:'--game\n{gameDir}\n--literal=$(never)'};
    const event={target:form,submitter:f.node('#submit'),preventDefault(){}};
    f.listeners.submit[0](event);f.listeners.submit[0](event);
    await tick();
    const saves=f.calls.filter(c=>c.name==='save_emulator');
    assert.equal(saves.length,1);
    assert.equal(saves[0].args.id,id);
    assert.deepEqual(Array.from(saves[0].args.launchArgs),['--game','{gameDir}','--literal=$(never)']);
    assert.equal(f.isConfigured(id),true);
    assert.equal(f.calls.some(c=>c.name==='open_emulator'),false,'Save must not execute the file');
    assert.equal(f.state.overlay,null);
    await f.launchConsole(id);
    await f.launchConsole(id,'/games/Game file.xci');
    assert.deepEqual(f.calls.slice(-2).map(c=>c.name),['open_emulator','launch_console_game']);
    assert.equal(f.calls.at(-1).args.uid,'/games/Game file.xci');
  });
}


test('six added console libraries preview their own formats and exact Xbox 360 boot files', () => {
  const f=fixture();f.state.native=false;
  for (const [id,file,title] of [
    ['nes','Mario.NES','Mario'],['snes','Mario.SFC','Mario'],['genesis','Sonic.MD','Sonic'],
    ['saturn','Disc.CUE','Disc'],['xbox','Halo.ISO','Halo'],['xbox360','default.xex','Extracted Game'],
  ]) {
    f.state.consoleId=id;
    f.importFolder([{name:file,webkitRelativePath:'Games/Extracted Game/'+file}]);
    assert.equal(f.state.imported[id].length,1,id);
    assert.equal(f.state.imported[id][0].title,title,id);
    assert.equal(f.state.imported[id][0].relativePath,'Games/Extracted Game/'+file);
  }
  f.state.consoleId='xbox360';
  f.importFolder([{name:'module.xex',webkitRelativePath:'Games/Extracted Game/module.xex'}]);
  assert.match(f.messages.at(-1),/No recognized/);
  assert.equal(f.byId.xbox360.customEmulator,true);
  assert.equal(f.byId.xbox.customEmulator,undefined);
});
