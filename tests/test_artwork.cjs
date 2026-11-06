// Execute the real artwork functions with a minimal DOM/native bridge, so request
// ordering is deterministic and does not require a desktop or artwork assets.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');

function fixture() {
  let cards = [];
  const reads = [];
  const calls = [];
  const context = vm.createContext({
    consoles: [{ id: 'ps2' }],
    window: { __TAURI__: { core: { invoke(name, args) {
      calls.push({ name, args });
      if (name === 'get_cover') return new Promise((resolve, reject) => reads.push({ resolve, reject }));
      if (name === 'download_cover') return Promise.resolve({ status: 'downloaded' });
      if (name === 'save_cover') return Promise.resolve();
      if (['scan_console', 'scan_steam', 'list_windows_games'].includes(name)) return Promise.resolve([]);
      throw new Error(`Unexpected native command: ${name}`);
    } } } },
    document: {
      querySelectorAll: () => cards.filter(card => card.isConnected),
      createElement: () => ({}),
    },
    URL: { createObjectURL: () => 'blob:chosen-cover', revokeObjectURL() {} },
    onRender: () => replaceCard(),
  });
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup), 'Update the harness when startup changes');
  vm.runInContext(source.replace(startup, '') + `
    render = () => { onRender(); hydrateNativeCovers(); };
    refreshCoverUI = () => {};
    toast = () => {};
    globalThis.matrix = { state, requestNativeCover, refreshNative, loadCover, fetchArtwork };
  `, context);
  const api = context.matrix;
  api.state.artwork.enabled = false;
  const key = 'ps2|test.iso';
  function replaceCard() {
    for (const card of cards) card.isConnected = false;
    const card = {
      dataset: { artKey: key, artKind: 'ps2', artUid: 'test.iso' },
      isConnected: true,
      image: null,
      querySelector() { return this.image; },
      appendChild(image) { this.image = image; },
    };
    cards = [card];
    return card;
  }
  return { ...api, key, reads, calls, replaceCard, card: () => cards[0] };
}

for (const staleOutcome of ['resolve', 'reject']) {
  test(`refresh starts a new read and ignores a stale ${staleOutcome}`, async () => {
    const f = fixture();
    const oldCard = f.replaceCard();
    const oldRequest = f.requestNativeCover(oldCard);
    await f.refreshNative();
    assert.equal(oldCard.isConnected, false);
    assert.equal(f.reads.length, 2, 'A stale pending read must not block new cards');
    f.reads[0][staleOutcome]('old cover');
    await oldRequest;
    assert.equal(f.state.art.has(f.key), false);
    assert.equal(f.state.missingArt.has(f.key), false);
    assert.equal(f.state.pendingArt.get(f.key), f.state.artRevision);
    await f.requestNativeCover(f.card());
    assert.equal(f.reads.length, 2, 'Old cleanup must not remove a newer pending read');
    f.reads[1].resolve('data:new-cover');
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(f.state.art.get(f.key), 'data:new-cover');
    assert.equal(f.card().image.src, 'data:new-cover');
    assert.equal(f.state.pendingArt.size, 0);
  });
}

test('invalidation retries the current card instead of a detached original', async () => {
  const f = fixture();
  const oldRequest = f.requestNativeCover(f.replaceCard());
  f.state.artRevision++;
  const newCard = f.replaceCard();
  f.reads[0].resolve('data:old-cover');
  await oldRequest;
  assert.equal(f.reads.length, 2);
  f.reads[1].resolve('data:current-cover');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(newCard.image.src, 'data:current-cover');
});

for (const staleOutcome of ['resolve', 'reject']) {
  test(`a chosen local cover survives a stale ${staleOutcome}`, async () => {
    const f = fixture();
    const oldRequest = f.requestNativeCover(f.replaceCard());
    f.state.selectedGame = { id: 'ps2', uid: 'test.iso' };
    await f.loadCover({ type: 'image/png', size: 3, arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer });
    assert.ok(f.calls.some(call => call.name === 'save_cover'));
    assert.equal(f.state.art.get(f.key), 'blob:chosen-cover');
    f.reads[0][staleOutcome]('data:old-cover');
    await oldRequest;
    assert.equal(f.state.art.get(f.key), 'blob:chosen-cover');
    assert.equal(f.state.missingArt.has(f.key), false);
    assert.equal(f.card().image.src, 'blob:chosen-cover');
    assert.equal(f.reads.length, 1);
  });
}

test('an automatic download replaces a pending read without stale cache writes', async () => {
  const f = fixture();
  const oldRequest = f.requestNativeCover(f.replaceCard());
  await f.fetchArtwork('ps2', 'test.iso');
  assert.equal(f.reads.length, 2);
  f.reads[1].resolve('data:downloaded-cover');
  await new Promise(resolve => setImmediate(resolve));
  f.reads[0].resolve('data:old-cover');
  await oldRequest;
  assert.equal(f.state.art.get(f.key), 'data:downloaded-cover');
  assert.equal(f.card().image.src, 'data:downloaded-cover');
  assert.equal(f.reads.length, 2);
});

test('current missing artwork stays deduplicated until the next refresh', async () => {
  const f = fixture();
  const request = f.requestNativeCover(f.replaceCard());
  await f.requestNativeCover(f.card());
  assert.equal(f.reads.length, 1);
  f.reads[0].resolve(null);
  await request;
  assert.equal(f.state.missingArt.has(f.key), true);
  await f.requestNativeCover(f.card());
  assert.equal(f.reads.length, 1);
  await f.refreshNative();
  assert.equal(f.reads.length, 2);
  f.reads[1].resolve('data:found-after-refresh');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(f.state.art.get(f.key), 'data:found-after-refresh');
});
