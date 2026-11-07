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
      focus() { document.activeElement = this; }, querySelectorAll() { return []; } });
    return nodes.get(selector);
  };
  for (const selector of ['#overlay-root', '#emulator-launcher', '#file-fields', '#flatpak-fields', '#appimage-options', '#emulator-path', '#flatpak-app', '#emulator-file-label', '#emulator-file-hint', '#config-error']) node(selector);
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
