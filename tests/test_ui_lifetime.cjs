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
  const intervals = new Map(), cards = [];
  let nextBlob = 0, nextTimer = 0, overlayWrites = 0;
  const updaterSelectors = ['.update-dialog', '.update-dialog .close', '#update-current', '#update-latest', '#update-latest-label', '.update-message', '#update-progress', '.update-dialog progress', '#update-progress-text', '.update-notes', '#update-hint', '#update-action'];
  const document = { activeElement: null, querySelector: selector => nodes.get(selector) ?? null,
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
  for (const selector of ['#screen', '#breadcrumbs', '#nav-consoles', '#nav-steam', '#nav-windows', '#nav-settings', '#game-results', '#overlay-root', '.app-shell', '#auto-covers', '#cover-style']) node(selector);
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
  const context = vm.createContext({ consoles: [
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
      loadFlatpakChoices, refreshNative, fetchArtwork, renderUpdater, runUpdate, openUpdater };
  `, context);
  const api = context.api;
  api.state.artwork.enabled = false;
  api.state.imported.ps2 = [{ uid: 'test.iso', title: 'Test Game' }];
  return { ...api, nodes, node, root, document, responders, calls, messages, blobs, revoked, intervals, cards,
    overlayWrites: () => overlayWrites };
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
