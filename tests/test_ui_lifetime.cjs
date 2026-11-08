// Deterministic async-lifetime checks against the real app functions. This small
// DOM model verifies state and node ownership, not browser layout/accessibility.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
const tick = () => new Promise(resolve => setImmediate(resolve));
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function fixture() {
  const nodes = new Map(), responders = {}, calls = [], messages = [], blobs = new Set(), revoked = [];
  const intervals = new Map(), cards = [], listeners = {};
  let nextBlob = 0, nextTimer = 0, overlayWrites = 0;
  const updaterSelectors = ['.update-dialog', '.update-dialog .close', '#update-current', '#update-latest', '#update-latest-label', '.update-message', '#update-progress', '.update-dialog progress', '#update-progress-text', '.update-notes', '#update-hint', '#update-close-confirm', '#update-safety', '#update-games-closed', '#update-action'];
  const document = { addEventListener(name, handler) { (listeners[name] ||= []).push(handler); }, activeElement: null, querySelector: selector => nodes.get(selector) ?? null,
    querySelectorAll: () => cards, createElement: () => makeNode() };
  function makeNode() {
    return { dataset: {}, value: '', checked: true, disabled: false, hidden: false, isConnected: true,
      textContent: '', classList: { toggle() {} }, setAttribute() {},
      cloneNode() { return { ...makeNode(), dataset: { ...this.dataset } }; },
      replaceWith(replacement) {
        this.isConnected = false;
        if (document.activeElement === this) document.activeElement = null;
        for (const [selector, current] of nodes) if (current === this) nodes.set(selector, replacement);
      },
      focus() { document.activeElement = this; }, querySelectorAll() { return []; } };
  }
  const node = selector => { if (!nodes.has(selector)) nodes.set(selector, makeNode()); return nodes.get(selector); };
  for (const selector of ['#screen', '#breadcrumbs', '#nav-consoles', '#nav-steam', '#nav-windows', '#nav-settings', '#game-results', '#overlay-root', '.app-shell', '#auto-covers', '#auto-title-covers', '#cover-style']) node(selector);
  node('#auto-title-covers').checked=false;
  const root = node('#overlay-root');
  let html = '';
  Object.defineProperty(root, 'innerHTML', { get: () => html, set(value) {
    overlayWrites++;
    for (const selector of updaterSelectors) {
      const old = nodes.get(selector);
      if (old) { old.isConnected = false; if (document.activeElement === old) document.activeElement = null; }
      nodes.delete(selector);
    }
    html = value;
    if (value.includes('update-dialog')) for (const selector of updaterSelectors) node(selector);
  } });
  const context = vm.createContext({ FormData: class { constructor(node) {this.values=node.values;} get(key) {return this.values[key];} has(key) {return key in this.values;} }, consoles: [
    { id: 'ps2', name: 'PlayStation 2', abbrev: 'PS2', games: [], group: 'Sony' },
    { id: 'n64', name: 'Nintendo 64', abbrev: 'N64', games: [], group: 'Nintendo' },
  ], window: { __TAURI__: { core: { invoke(name, args) {
    calls.push({ name, args });
    if (responders[name]) return responders[name](args);
    if (name === 'save_cover') return Promise.resolve();
    if (name === 'save_artwork_settings') return Promise.resolve(args);
    if (name === 'download_cover') return Promise.resolve({ status: 'downloaded' });
    if (name === 'get_cover') return Promise.resolve(null);
    if (['scan_console', 'scan_steam', 'list_windows_games'].includes(name)) return Promise.resolve([]);
    throw Error(`Unexpected command ${name}`);
  } } }, scrollTo() {} }, document,
  URL: { createObjectURL() { const uri = `blob:${++nextBlob}`; blobs.add(uri); return uri; },
    revokeObjectURL(uri) { blobs.delete(uri); revoked.push(uri); } },
  setTimeout() {}, clearTimeout() {},
  setInterval(callback) { const id = ++nextTimer; intervals.set(id, callback); return id; },
  clearInterval(id) { intervals.delete(id); }, localStorage: { setItem() {} },
  onToast: message => messages.push(message) });
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(source.replace(startup, '') + `
    toast = onToast;
    globalThis.api = { state, loadCover, goConsole, goSettings, dismissOverlay, saveArtworkOptions,
      loadDetectedEmulators, loadFlatpakChoices, refreshNative, fetchArtwork, renderUpdater, runUpdate, openUpdater, handleAction, toggleFavorite, visibleGames, renderConsole, setArtDb: db => { artDb=db; } };
    setupEvents();
  `, context);
  const api = context.api;
  api.state.artwork.enabled = false;
  api.state.imported.ps2 = [{ uid: 'test.iso', title: 'Test Game' }];
  return { ...api, nodes, node, root, document, responders, calls, messages, blobs, revoked, intervals, cards,
    listeners, overlayWrites: () => overlayWrites };
}
const cover = { type: 'image/png', size: 3, arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer };

for (const destination of ['closed', 'n64', 'another-game', 'external']) {
  test(`cover import preserves ${destination} after async save`, async () => {
    const f = fixture(), saving = deferred();
    f.responders.save_cover = () => saving.promise;
    f.state.page = 'console'; f.state.consoleId = 'ps2'; f.state.overlay = 'game';
    f.state.selectedGame = { id: 'ps2', uid: 'test.iso' };
    const pending = f.loadCover(cover);
    await tick();
    f.dismissOverlay();
    if (destination === 'n64') f.goConsole('n64');
    if (destination === 'another-game') { f.state.overlay = 'game'; f.state.selectedGame = { id: 'ps2', uid: 'other.iso' }; }
    if (destination === 'external') { f.state.page = 'steam'; f.state.overlay = 'external'; f.state.selectedGame = { id: 'steam', uid: '1234' }; }
    const { page, overlay, selectedGame } = f.state;
    const writes = f.overlayWrites(), screen = f.node('#screen').innerHTML;
    saving.resolve(); await pending;
    assert.equal(f.state.page, page); assert.equal(f.state.overlay, overlay);
    assert.equal(f.state.selectedGame, selectedGame);
    assert.equal(f.overlayWrites(), writes); assert.equal(f.node('#screen').innerHTML, screen);
    assert.equal(f.state.art.get('ps2|test.iso'), 'blob:1');
  });
}

test('cover import updates current images without replacing details or focus', async () => {
  const f = fixture(), focused = f.node('cover-picker');
  f.state.overlay = 'game'; f.state.selectedGame = { id: 'ps2', uid: 'test.iso' };
  const image = {};
  f.cards.push({ dataset: { artKey: 'ps2|test.iso' }, querySelector: () => image });
  focused.focus(); const writes = f.overlayWrites();
  await f.loadCover(cover);
  assert.equal(image.src, 'blob:1'); assert.equal(f.document.activeElement, focused);
  assert.equal(f.overlayWrites(), writes);
});

for (const invalidation of ['refresh', 'preferences', 'download', 'replacement']) {
  test(`${invalidation} revokes discarded blob URLs`, async () => {
    const f = fixture(); f.state.selectedGame = { id: 'ps2', uid: 'test.iso' };
    await f.loadCover(cover); assert.equal(f.blobs.size, 1);
    if (invalidation === 'refresh') await f.refreshNative();
    if (invalidation === 'preferences') { f.node('#auto-covers').checked = false; f.node('#cover-style').value = '3d'; await f.saveArtworkOptions(); }
    if (invalidation === 'download') await f.fetchArtwork('ps2', 'test.iso');
    if (invalidation === 'replacement') await f.loadCover(cover);
    assert.deepEqual(f.revoked, ['blob:1']);
    assert.equal(f.blobs.size, invalidation === 'replacement' ? 1 : 0);
  });
}

test('failed artwork preference save does not replace the newly active page', async () => {
  const f = fixture(), saving = deferred();
  f.responders.save_artwork_settings = () => saving.promise;
  f.goSettings(); f.node('#cover-style').value = '3d';
  const pending = f.saveArtworkOptions();
  f.goConsole('n64'); const screen = f.node('#screen').innerHTML;
  saving.reject('disk full'); await pending;
  assert.equal(f.state.page, 'console'); assert.equal(f.state.consoleId, 'n64');
  assert.equal(f.node('#screen').innerHTML, screen); assert.equal(f.state.artworkSaving, false);
  assert.ok(f.messages.at(-1).includes('disk full'));
});

for (const outcome of ['resolve', 'reject']) {
  test(`title-provider opt-in restores replacement controls after ${outcome}`, async () => {
    const f=fixture(), saving=deferred();
    f.responders.save_artwork_settings=()=>saving.promise;
    f.goSettings(); f.node('#auto-covers').checked=true;
    f.node('#auto-title-covers').checked=true; f.node('#cover-style').value='default';
    // Exercise the real change-event route, not only a direct save call.
    f.listeners.change[0]({target:{id:'auto-title-covers'}});
    assert.equal(f.calls[0].name,'save_artwork_settings');
    assert.equal(f.calls[0].args.titleEnabled,true);
    assert.equal(f.node('#auto-title-covers').disabled,true);
    const old=f.node('#auto-title-covers');
    old.replaceWith({...old,checked:false});
    saving[outcome](outcome==='resolve' ? {enabled:true,style:'default',titleEnabled:true} : 'disk full');
    await tick();
    assert.equal(f.state.artwork.titleEnabled,outcome==='resolve');
    assert.equal(f.node('#auto-title-covers').checked,outcome==='resolve');
    assert.equal(f.node('#auto-title-covers').disabled,false);
  });

  test(`artwork preferences restore current Settings controls after ${outcome}`, async () => {
    const f = fixture(), saving = deferred();
    f.responders.save_artwork_settings = () => saving.promise;
    f.goSettings(); f.node('#cover-style').value = '3d';
    const pending = f.saveArtworkOptions();
    f.goConsole('n64'); f.goSettings();
    assert.match(f.node('#screen').innerHTML, /id="cover-style" disabled/);
    await f.saveArtworkOptions();
    assert.equal(f.calls.filter(x => x.name === 'save_artwork_settings').length, 1);
    saving[outcome](outcome === 'resolve' ? { enabled: true, style: '3d' } : 'disk full');
    await pending;
    assert.equal(f.node('#cover-style').value, outcome === 'resolve' ? '3d' : 'default');
    assert.equal(f.node('#auto-covers').checked, outcome === 'resolve');
    assert.equal(f.node('#cover-style').disabled, false);
  });
}

for (const outcome of ['resolve', 'reject']) {
  test(`closed Flatpak dialog safely ignores delayed ${outcome}`, async () => {
    const f = fixture(), loading = deferred();
    f.node('#flatpak-app'); f.node('#flatpak-help');
    f.responders.list_flatpaks = () => loading.promise;
    const pending = f.loadFlatpakChoices();
    f.nodes.delete('#flatpak-app'); f.nodes.delete('#flatpak-help');
    loading[outcome](outcome === 'resolve' ? [] : 'not installed');
    await assert.doesNotReject(pending);
  });
}

test('new Flatpak reload owns its controls even if an earlier request fails', async () => {
  const f = fixture(), first = deferred(), second = deferred();
  const select = f.node('#flatpak-app'), help = f.node('#flatpak-help');
  let calls = 0; f.responders.list_flatpaks = () => (++calls === 1 ? first : second).promise;
  const a = f.loadFlatpakChoices(), b = f.loadFlatpakChoices();
  first.reject('old failure'); await a;
  assert.equal(select.disabled, true); assert.match(help.textContent, /Reading installed/);
  second.resolve([]); await b; assert.equal(select.disabled, false);
  assert.match(help.textContent, /No Flatpak apps/);
});

test('updater polling keeps Cancel and Close nodes and keyboard focus alive', () => {
  const f = fixture(); f.state.overlay = 'updates';
  f.state.update = { phase: 'downloading', message: 'Downloading', total: 100, received: 1 };
  f.renderUpdater(); const action = f.node('#update-action'), close = f.node('.update-dialog .close');
  action.focus(); const writes = f.overlayWrites();
  for (let received = 2; received <= 20; received++) { f.state.update.received = received; f.renderUpdater(); }
  assert.equal(f.overlayWrites(), writes); assert.equal(f.node('#update-action'), action);
  assert.equal(f.node('.update-dialog .close'), close); assert.equal(f.document.activeElement, action);
  assert.equal(action.dataset.action, 'cancel-update'); assert.equal(f.node('.app-shell').inert, true);
  assert.equal(f.node('.update-dialog progress').value, 20);
  f.state.update.phase = 'installing'; f.renderUpdater();
  assert.equal(f.document.activeElement, close, 'Disabled action moves focus to an enabled dialog button');
  f.dismissOverlay(); assert.equal(f.node('.app-shell').inert, false);
});

test('stale updater poll cannot overwrite a later operation', async () => {
  const f = fixture(), first = deferred(), second = deferred(), poll = deferred();
  f.state.update = { phase: 'available' };
  f.responders.download_update = () => first.promise;
  f.responders.install_update = () => second.promise;
  f.responders.update_status = () => poll.promise;
  const downloading = f.runUpdate('download_update');
  const pendingPoll = [...f.intervals.values()][0]();
  first.resolve({ phase: 'downloaded' }); await downloading;
  f.node('#update-games-closed').checked = true;
  const installing = f.runUpdate('install_update');
  poll.resolve({ phase: 'downloading' }); await pendingPoll;
  assert.equal(f.state.update.phase, 'installing');
  second.resolve({ phase: 'installed' }); await installing;
});

test('a Cancel press cannot become Install when downloading finishes', () => {
  const f = fixture(); f.state.overlay = 'updates';
  f.state.update = { phase: 'downloading', message: 'Downloading', total: 100, received: 99 };
  f.renderUpdater();
  const cancel = f.node('#update-action'), close = f.node('.update-dialog .close');
  cancel.focus();
  f.state.update = { phase: 'downloaded', message: 'Verified installer downloaded' };
  f.renderUpdater();
  assert.equal(cancel.isConnected, false, 'The pending pointer/key gesture loses its old button');
  assert.equal(cancel.dataset.action, 'cancel-update', 'The pressed node never changes its action');
  assert.notEqual(f.node('#update-action'), cancel);
  assert.equal(f.node('#update-action').dataset.action, 'install-update');
  assert.equal(f.document.activeElement, close, 'Focus moves to the safe Close control');
  const install = f.node('#update-action');
  f.renderUpdater(); assert.equal(f.node('#update-action'), install, 'Same-action renders still preserve controls');
});

test('closing updater before status arrives does not reopen or start work', async () => {
  const f = fixture(), status = deferred(); f.responders.update_status = () => status.promise;
  const opening = f.openUpdater(); f.dismissOverlay(); const writes = f.overlayWrites();
  status.resolve({ phase: 'idle' }); await opening;
  assert.equal(f.state.overlay, null); assert.equal(f.overlayWrites(), writes);
  assert.deepEqual(f.calls.map(x => x.name), ['update_status']);
});


test('late opening status cannot roll back an operation completed while it waited', async () => {
  const f = fixture(), download = deferred(), status = deferred();
  f.state.update = { phase: 'available' };
  f.responders.download_update = () => download.promise;
  f.responders.update_status = () => status.promise;
  const downloading = f.runUpdate('download_update');
  const opening = f.openUpdater();
  download.resolve({ phase: 'downloaded' }); await downloading;
  status.resolve({ phase: 'downloading' }); await opening;
  assert.equal(f.state.update.phase, 'downloaded');
  assert.equal(f.node('#update-action').dataset.action, 'install-update');
});

for (const outcome of ['resolve', 'reject']) test(`old emulator save ${outcome} leaves newer same-console form intact`, async () => {
  const f=fixture(), saving=deferred(); f.responders.save_emulator=()=>saving.promise;
  const old=f.node('#config-form'); old.id='config-form';old.dataset.id='ps2';
  old.values={emulatorName:'PCSX2', emulatorPath:'/test/pcsx2', folderPath:'/games', launcher:'executable'};
  const submit=f.node('#submit');f.node('#config-error');f.state.overlay='configure';
  const event={target:old,submitter:submit,preventDefault(){}};
  f.listeners.submit[0](event);
  const replacement={dataset:{id:'ps2'}};f.nodes.set('#config-form',replacement);
  f.node('#config-error').textContent='New form';const writes=f.overlayWrites();
  if (outcome==='resolve') saving.resolve();else saving.reject(Error('Old failure'));
  await tick();
  assert.equal(f.state.overlay,'configure');assert.equal(f.overlayWrites(),writes);
  assert.equal(f.node('#config-error').textContent,'New form');
  assert.equal(submit.disabled,false);assert.equal(old.dataset.saving,undefined);
});
for (const newestSucceeds of [true, false]) test(`reopened emulator saves retain the newest successful choice (${newestSucceeds})`, async () => {
  const f=fixture(), older=deferred(), newer=deferred();let writes=0;
  f.responders.save_emulator=()=>++writes===1?older.promise:newer.promise;
  const makeForm=name=>({id:'config-form',dataset:{id:'ps2'},values:{emulatorName:name,emulatorPath:'/test/'+name,folderPath:'/games',launcher:'executable'}});
  const first=makeForm('older'), second=makeForm('newer');
  f.node('#config-error');f.state.overlay='configure';
  f.nodes.set('#config-form',first);
  f.listeners.submit[0]({target:first,submitter:{dataset:{}},preventDefault(){}});
  // Closing and reopening creates a different form, so its submit is permitted.
  f.nodes.set('#config-form',second);
  f.listeners.submit[0]({target:second,submitter:{dataset:{}},preventDefault(){}});
  assert.equal(writes,2);
  if(newestSucceeds)newer.resolve();else newer.reject(Error('Invalid newer choice'));
  await tick();
  // Even successful IPC responses can arrive after a newer commit's response.
  older.resolve();await tick();
  assert.equal(f.state.configs.ps2.emulatorName,newestSucceeds?'newer':'older');
  assert.equal(first.dataset.saving,undefined);assert.equal(second.dataset.saving,undefined);
});

test('completed Windows registration cannot close replacement registration dialog', async () => {
  const f=fixture(), saving=deferred();f.responders.add_windows_game=()=>saving.promise;
  const old=f.node('#windows-form');old.id='windows-form';old.values={title:'Game',target:'/test.desktop'};
  f.state.overlay='windows-add';const event={target:old,preventDefault(){}};
  f.listeners.submit[0](event);
  f.nodes.set('#windows-form',{});const writes=f.overlayWrites();saving.resolve();await tick();
  assert.equal(f.calls.filter(c=>c.name==='add_windows_game').length,1);
  assert.equal(f.state.overlay,'windows-add');assert.equal(f.overlayWrites(),writes);
});
test('completed removal cannot close newer game details', async () => {
  const f=fixture(), saving=deferred();f.responders.remove_windows_game=()=>saving.promise;
  f.state.overlay='external';f.root.firstElementChild={};
  const button={dataset:{action:'remove-windows',uid:'old'},disabled:false};
  const pending=f.handleAction(button), duplicate=f.handleAction(button);
  f.root.firstElementChild={};f.state.selectedGame={id:'windows',uid:'new'};
  const writes=f.overlayWrites();saving.resolve();await Promise.all([pending,duplicate]);
  assert.equal(f.calls.filter(c=>c.name==='remove_windows_game').length,1);
  assert.equal(f.state.overlay,'external');assert.equal(f.state.selectedGame.uid,'new');assert.equal(f.overlayWrites(),writes);
});
test('favoriting updates details in place without replacing draft serial or focus', async () => {
  const f=fixture();f.responders.set_favorite=()=>Promise.resolve(['test.iso']);
  f.state.page='console';f.state.consoleId='ps2';f.state.overlay='game';f.state.selectedGame={id:'ps2',uid:'test.iso'};
  const serial=f.node('#cover-serial');serial.value='SLUS-20999';serial.focus();
  const control=f.node('#overlay-root [data-action="favorite"]');const writes=f.overlayWrites();
  f.toggleFavorite('ps2','test.iso',control);await tick();
  assert.equal(serial.value,'SLUS-20999');assert.equal(f.document.activeElement,serial);
  assert.equal(f.overlayWrites(),writes);assert.match(control.innerHTML,/Unfavorite/);
});
test('collapsed navigation and topbar buttons have explicit accessible names', () => {
  const html=fs.readFileSync(path.join(__dirname,'../web/index.html'),'utf8');
  for (const id of ['nav-consoles','nav-steam','nav-windows','nav-settings']) {
    const button=html.match(new RegExp(`<button[^>]*id="${id}"[^>]*>`))[0];
    assert.match(button,/aria-label="[^\"]+"/);
  }
  for (const action of ['refresh','configure-first']) assert.match(html.match(new RegExp(`<button[^>]*data-action="${action}"[^>]*>`))[0],/aria-label="[^\"]+"/);
});

test('duplicate emulator and Windows submits issue only one pending write per form', async()=>{
  for (const kind of ['config','windows']) {
    const f=fixture(),saving=deferred(),command=kind==='config'?'save_emulator':'add_windows_game';
    f.responders[command]=()=>saving.promise;f.node('#config-error');
    const form=f.node(`#${kind}-form`);form.id=`${kind}-form`;form.dataset.id='ps2';
    form.values={title:'Game',target:'/test.desktop',emulatorName:'PCSX2',emulatorPath:'/test/pcsx2',folderPath:'/games',launcher:'executable'};
    const event={target:form,submitter:f.node('#submit'),preventDefault(){}};
    f.listeners.submit[0](event);f.listeners.submit[0](event);
    assert.equal(f.calls.filter(c=>c.name===command).length,1);
    saving.resolve();await tick();assert.equal(form.dataset.saving,undefined);
  }
});

test('an old emulator picker failure does not contaminate a replacement setup dialog', async () => {
 const f=fixture(), browse=deferred();
 f.node('#emulator-path');f.node('#emulator-launcher').value='executable';f.node('#config-error');
 f.responders.browse_emulator=()=>browse.promise;
 const pending=f.handleAction({dataset:{action:'browse-emulator'}});
 for(const selector of ['#emulator-path','#emulator-launcher','#config-error'])f.nodes.delete(selector);
 const error=f.node('#config-error');error.textContent='New dialog validation';
 browse.reject('Old picker failed');await pending;
 assert.equal(error.textContent,'New dialog validation');
});
test('an emulator picker result preserves edits typed while its response is pending',async()=>{
 const f=fixture(),browse=deferred();
 const input=f.node('#emulator-path');input.value='/initial';f.node('#emulator-launcher').value='executable';
 f.responders.browse_emulator=()=>browse.promise;
 const pending=f.handleAction({dataset:{action:'browse-emulator'}});
 input.value='/new-draft';browse.resolve('/old-picker-choice');await pending;
 assert.equal(input.value,'/new-draft');
});
test('late prior refresh cannot restore entries removed by the newer refresh',async()=>{
 const f=fixture(),old=deferred(),recent=deferred();
 let scans=0;f.responders.list_windows_games=()=>++scans===1?old.promise:recent.promise;
 const first=f.refreshNative();const second=f.refreshNative();
 recent.resolve([]);await second;
 old.resolve([{uid:'/old.desktop',title:'Removed title',target:'/old.desktop',runner:'desktop'}]);await first;
 assert.equal(f.state.windows.length,0);
});
test('a slow earlier cover-file read cannot overwrite a later selection',async()=>{
 const f=fixture(),old=deferred();f.state.selectedGame={id:'ps2',uid:'test.iso'};
 const first=f.loadCover({type:'image/png',size:1,arrayBuffer:()=>old.promise});
 await f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([2]).buffer});
 old.resolve(new Uint8Array([1]).buffer);await first;
 assert.deepEqual(f.calls.filter(c=>c.name==='save_cover').map(c=>c.args.bytes[0]),[2]);
});

test('newest release-year sort is unavailable without metadata and resets stale selection',()=>{
  const f=fixture();f.state.page='console';f.state.consoleId='ps2';f.state.sort='recent';f.renderConsole();
  assert.equal(f.state.sort,'default');const html=f.node('#screen').innerHTML;
  assert.match(html,/<option value="recent"[^>]*disabled>Release year: newest first/);
  assert.match(html,/unavailable: these titles have no release-year metadata/);
});
test('release-year sort uses only positive integer years and keeps unknown titles last',()=>{
  const f=fixture();f.state.imported.ps2=[{uid:'a',title:'Unknown',year:0},{uid:'b',title:'Older',year:1999},{uid:'c',title:'Newest',year:2005},{uid:'d',title:'Invalid',year:Infinity}];
  f.state.sort='recent';assert.deepEqual(Array.from(f.visibleGames('ps2'),game=>game.title),['Newest','Older','Unknown','Invalid']);
  f.state.page='console';f.state.consoleId='ps2';f.renderConsole();assert.doesNotMatch(f.node('#screen').innerHTML,/<option value="recent"[^>]*disabled/);
});
test('refresh removing release-year metadata resets sort and selected control',async()=>{
  const f=fixture();f.state.page='console';f.state.consoleId='ps2';f.state.sort='recent';f.state.imported.ps2=[{uid:'a',title:'Known',year:2001}];
  f.node('#sort-games').value='recent';f.responders.scan_console=()=>Promise.resolve([{uid:'b',title:'No metadata'}]);
  await f.refreshNative();assert.equal(f.state.sort,'default');assert.equal(f.node('#sort-games').value,'default');
});
test('native cover writes serialize while newest selection owns the final cache',async()=>{
  const f=fixture(),writing=deferred();f.state.selectedGame={id:'ps2',uid:'test.iso'};
  f.responders.save_cover=args=>args.bytes[0]===1?writing.promise:Promise.resolve();
  const first=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([1]).buffer});await tick();
  const second=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([2]).buffer});await tick();
  assert.deepEqual(f.calls.filter(c=>c.name==='save_cover').map(c=>c.args.bytes[0]),[1]);
  writing.resolve();await Promise.all([first,second]);
  assert.deepEqual(f.calls.filter(c=>c.name==='save_cover').map(c=>c.args.bytes[0]),[1,2]);assert.equal(f.blobs.size,1);assert.equal(f.messages.filter(m=>m.includes('Cover saved')).length,1);
});
test('failed earlier cover write does not prevent newer selection saving',async()=>{
  const f=fixture(),writing=deferred();f.state.selectedGame={id:'ps2',uid:'test.iso'};
  f.responders.save_cover=args=>args.bytes[0]===1?writing.promise:Promise.resolve();
  const first=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([1]).buffer});await tick();
  const second=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([2]).buffer});await tick();
  writing.reject(Error('Old failed'));await Promise.all([first,second]);assert.equal(f.messages.some(m=>m.includes('Old failed')),false);assert.equal(f.blobs.size,1);
});
test('stale refresh errors do not obscure a newer successful refresh',async()=>{
  const f=fixture(),old=deferred();let reads=0;f.responders.list_windows_games=()=>++reads===1?old.promise:Promise.resolve([]);
  const first=f.refreshNative();await f.refreshNative();old.reject(Error('Stale scan'));await first;assert.equal(f.messages.some(m=>m.includes('Stale scan')),false);
});

test('preview cover transactions are issued synchronously in selection order',async()=>{
  const f=fixture(),transactions=[],puts=[];f.state.native=false;f.state.selectedGame={id:'ps2',uid:'test.iso'};
  f.setArtDb({transaction(store,mode){
    assert.equal(store,'covers');assert.equal(mode,'readwrite');
    const tx={objectStore(name){assert.equal(name,'covers');return {put(item){puts.push(item);}};}};
    transactions.push(tx);return tx;
  }});
  const old={type:'image/png',size:1,label:'old'},latest={type:'image/png',size:1,label:'latest'};
  const first=f.loadCover(old),second=f.loadCover(latest);
  // Both puts already happened before either completion/await. IndexedDB
  // readwrite transactions with overlapping scopes run in this creation order.
  assert.deepEqual(puts.map(item=>item.blob.label),['old','latest']);
  assert.equal(f.state.art.get('ps2|test.iso'),'blob:2');
  transactions[0].oncomplete();await first;assert.equal(f.messages.length,0);
  transactions[1].oncomplete();await second;
  assert.equal(f.state.art.get('ps2|test.iso'),'blob:2');assert.equal(f.blobs.size,1);
  assert.equal(f.messages.filter(message=>message.includes('saved to this browser')).length,1);
});
test('preview selection before database readiness is not queued for a later stale write',async()=>{
  const f=fixture(),transactions=[],puts=[];f.state.native=false;f.state.selectedGame={id:'ps2',uid:'test.iso'};
  await f.loadCover({type:'image/png',size:1,label:'before-open'});
  assert.match(f.messages.at(-1),/session; browser storage was unavailable/);
  f.setArtDb({transaction(){const tx={objectStore(){return {put(item){puts.push(item);}};}};transactions.push(tx);return tx;}});
  const latest=f.loadCover({type:'image/png',size:1,label:'after-open'});
  assert.deepEqual(puts.map(item=>item.blob.label),['after-open']);
  transactions[0].oncomplete();await latest;assert.equal(f.state.art.get('ps2|test.iso'),'blob:2');
});

test('a failed latest cover selection retains consistency with the preceding successful disk write',async()=>{
 const f=fixture(),saving=deferred();f.state.selectedGame={id:'ps2',uid:'test.iso'};
 f.state.art.set('ps2|test.iso','data:original-cover');
 let count=0,persisted=0;
 f.responders.get_cover=()=>Promise.resolve(persisted ? 'data:saved-cover' : 'data:original-cover');
 f.responders.save_cover=async args=>{
  if(++count===1){await saving.promise;persisted=args.bytes[0];return;}
  throw Error('New cover save failed');
 };
 const old=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([1]).buffer});
 await tick();
 const latest=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([2]).buffer});
 await tick();saving.resolve();await old;await latest;
 assert.equal(persisted,1);
 assert.ok(f.calls.some(call=>call.name==='get_cover'),'Read actual retained disk artwork after failure');
 assert.equal(f.state.art.get('ps2|test.iso'),'data:saved-cover','Reconcile the actual persisted image after the newest save fails');
});

test('failed latest cover read reconciles only after an earlier active write settles',async()=>{
  const f=fixture(),writing=deferred();f.state.selectedGame={id:'ps2',uid:'test.iso'};let persisted='data:original';
  f.responders.save_cover=async()=>{await writing.promise;persisted='data:saved';};f.responders.get_cover=()=>Promise.resolve(persisted);
  const first=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([1]).buffer});await tick();
  const latest=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>{throw Error('Read failed');}});await tick();
  assert.equal(f.calls.filter(call=>call.name==='get_cover').length,0);
  writing.resolve();await Promise.all([first,latest]);assert.equal(f.state.art.get('ps2|test.iso'),'data:saved');
  assert.match(f.messages.at(-1),/Read failed/);
});
test('failed cover reconciliation cannot overwrite a subsequent selection',async()=>{
  const f=fixture(),reading=deferred();f.state.selectedGame={id:'ps2',uid:'test.iso'};
  f.responders.save_cover=args=>args.bytes[0]===1?Promise.reject(Error('Failed')):Promise.resolve();
  f.responders.get_cover=()=>reading.promise;
  const failed=f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([1]).buffer});await tick();
  await f.loadCover({type:'image/png',size:1,arrayBuffer:async()=>new Uint8Array([2]).buffer});
  const newest=f.state.art.get('ps2|test.iso');reading.resolve('data:older-disk');await failed;
  assert.equal(f.state.art.get('ps2|test.iso'),newest);
});


test('update installation requires an explicit saved-and-closed games confirmation', async () => {
  const f = fixture(); f.state.overlay = 'updates';
  f.state.update = {phase: 'downloaded', installMode: 'appimage'};
  f.renderUpdater(); f.node('#update-games-closed').checked = false;
  await f.runUpdate('install_update');
  assert.equal(f.calls.length, 0);
  assert.equal(f.state.update.phase, 'downloaded');
  assert.match(f.messages.at(-1), /Save your progress/);
  assert.match(f.messages.at(-1), /installers\/updates to finish/);
  f.node('#update-games-closed').checked = true;
  f.responders.install_update = args => { assert.equal(args.gamesClosed, true); return {phase:'installing'}; };
  await f.runUpdate('install_update');
  assert.equal(f.calls.filter(c => c.name==='install_update').length, 1);
});


test('confirming closed work never overrides a busy backend activity guard', async () => {
  const f=fixture(); f.state.overlay='updates'; f.state.update={phase:'downloaded',installMode:'appimage'};
  f.renderUpdater(); f.node('#update-games-closed').checked=true;
  f.responders.install_update=args=>{ assert.equal(args.gamesClosed,true); return {phase:'error',message:'A Matrix launch or log helper is still active. Nothing will be force-stopped.'}; };
  await f.runUpdate('install_update');
  assert.equal(f.state.update.phase,'error');
  assert.match(f.state.update.message,/still active/);
  assert.equal(f.state.overlay,'updates');
});

test('update prompt tells users to save progress and let installers finish', () => {
  const source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8');
  assert.match(source,/Before updating, save your progress and close games, emulators and other apps launched through Matrix Play/);
  assert.match(source,/Let installers and updates finish before closing them/);
});


for (const phase of ['error','recovery']) test(`opening Updates preserves ${phase} feedback until explicit retry`, async () => {
  const f=fixture();
  const message='Installation was cancelled or failed. Check the system package manager before retrying.';
  f.responders.update_status=()=>Promise.resolve({phase,message,installMode:'system'});
  f.responders.check_updates=()=>Promise.resolve({phase:'available',message:'Update available'});
  await f.openUpdater();
  assert.equal(f.state.update.message,message);
  assert.equal(f.state.update.phase,phase);
  assert.equal(f.calls.filter(c=>c.name==='check_updates').length,0);
  assert.equal(f.node('#update-action').dataset.action,'check-updates');
  await f.runUpdate('check_updates');
  assert.equal(f.calls.filter(c=>c.name==='check_updates').length,1);
  assert.equal(f.state.update.phase,'available');
});


test('agree and update clearly closes Matrix instead of promising a restart', () => {
  const f=fixture();
  f.state.overlay='updates'; f.state.update={phase:'downloaded', installMode:'system'};
  f.renderUpdater();
  assert.equal(f.node('#update-action').textContent, 'Agree and update');
  assert.match(f.node('#update-hint').textContent, /stays closed when it succeeds/);
});


function setupDiscovery(f) {
  const form=f.node('#config-form');form.dataset.id='ps2';
  for(const selector of ['#detected-emulator','#refresh-detected','#detected-help','#emulator-path','#launch-args'])f.node(selector);
  return form;
}
const detected=(path)=>({id:'file|'+path,consoleIds:['ps2'],profile:{emulatorName:'PCSX2',emulatorPath:path},ready:true});

test('setup discovery refresh removes a moved choice without rewriting any draft or saved profile',async()=>{
  const f=fixture(),loading=deferred();setupDiscovery(f);
  const old=detected('/Downloads/pcsx2.AppImage'),recent=detected('/Games/pcsx2.AppImage');
  f.state.discovery.candidates=[old];f.state.configs.ps2={emulatorPath:'/saved/custom'};
  f.node('#detected-emulator').value=old.id;
  f.node('#emulator-path').value=old.profile.emulatorPath;f.node('#launch-args').value='--my-draft';
  f.responders.discover_emulators=()=>loading.promise;
  const pending=f.loadDetectedEmulators();
  assert.equal(f.node('#detected-emulator').disabled,true);
  f.node('#emulator-path').value='/typed/while-scanning';
  loading.resolve({candidates:[recent],notices:[]});await pending;
  assert.equal(f.node('#emulator-path').value,'/typed/while-scanning');
  assert.equal(f.node('#launch-args').value,'--my-draft');
  assert.equal(f.state.configs.ps2.emulatorPath,'/saved/custom');
  assert.equal(f.node('#detected-emulator').value,'');
  assert.match(f.node('#detected-emulator').innerHTML,/Games/);
  assert.doesNotMatch(f.node('#detected-emulator').innerHTML,/Downloads/);
  assert.deepEqual(f.calls.map(c=>c.name),['discover_emulators']);
  assert.equal(f.node('#refresh-detected').disabled,false);
});

test('a new discovery refresh wins over a delayed older result',async()=>{
  const f=fixture(),old=deferred(),recent=deferred();setupDiscovery(f);
  let count=0;f.responders.discover_emulators=()=>++count===1?old.promise:recent.promise;
  const first=f.loadDetectedEmulators(),second=f.loadDetectedEmulators();
  recent.resolve({candidates:[detected('/new/pcsx2.AppImage')],notices:[]});await second;
  old.resolve({candidates:[detected('/old/pcsx2.AppImage')],notices:[]});await first;
  assert.equal(f.state.discovery.candidates[0].profile.emulatorPath,'/new/pcsx2.AppImage');
  assert.equal(f.node('#refresh-detected').disabled,false);
});

for(const outcome of ['resolve','reject'])test(`closed discovery ignores delayed ${outcome}`,async()=>{
  const f=fixture(),loading=deferred();setupDiscovery(f);
  f.responders.discover_emulators=()=>loading.promise;
  const pending=f.loadDetectedEmulators();
  for(const selector of ['#config-form','#detected-emulator','#refresh-detected','#detected-help'])f.nodes.delete(selector);
  setupDiscovery(f);f.node('#detected-help').textContent='New dialog';
  loading[outcome](outcome==='resolve'?{candidates:[detected('/old/pcsx2')],notices:[]}:'old error');
  await pending;assert.equal(f.node('#detected-help').textContent,'New dialog');
  assert.equal(f.state.discovery.candidates.length,0);
});

test('discovery failure keeps drafts and offers Browse or retry without replacing the form',async()=>{
  const f=fixture();const owner=setupDiscovery(f);
  f.node('#emulator-path').value='/draft';
  f.responders.discover_emulators=()=>Promise.reject('scan failed');
  await f.loadDetectedEmulators();
  assert.equal(f.node('#config-form'),owner);
  assert.equal(f.node('#emulator-path').value,'/draft');
  assert.match(f.node('#detected-help').textContent,/Use Browse/);
  assert.equal(f.node('#refresh-detected').disabled,false);
});

test('discovery retains a still-current explicit choice and escapes filenames',async()=>{
  const f=fixture();setupDiscovery(f);
  const entry=detected('/apps/pcsx2-<tag>".AppImage');
  f.node('#detected-emulator').value=entry.id;
  f.responders.discover_emulators=()=>Promise.resolve({candidates:[entry],notices:[]});
  await f.loadDetectedEmulators();
  assert.equal(f.node('#detected-emulator').value,entry.id);
  assert.match(f.node('#detected-emulator').innerHTML,/&lt;tag&gt;&quot;/);
  f.responders.discover_emulators=()=>Promise.resolve({candidates:[],notices:[]});
  await f.loadDetectedEmulators();
  assert.match(f.node('#detected-help').textContent,/No recognized installations/);
  assert.equal(f.node('#detected-emulator').value,'');
});
