// Startup/selection ordering against production functions, with in-memory
// IndexedDB and blob-URL doubles. No browser storage or image I/O is used.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
const tick = () => new Promise(resolve => setImmediate(resolve));

class PreviewBlob {
  constructor(label) { this.label = label; this.type = 'image/png'; this.size = 1; }
}

function fixture() {
  const opened = {}, reading = {}, writes = [], created = [], revoked = [], messages = [], renders = [];
  const live = new Map();
  let opens = 0;
  const database = { transaction(store, mode) {
    assert.equal(store, 'covers');
    if (mode === 'readonly') return { objectStore: () => ({ getAll: () => reading }) };
    assert.equal(mode, 'readwrite');
    const tx = { objectStore: () => ({ put(item) { writes.push({ ...item, tx }); } }) };
    return tx;
  } };
  const context = vm.createContext({
    consoles: [{ id: 'ps2' }], window: { indexedDB: {} },
    document: { querySelectorAll: () => [] }, Blob: PreviewBlob,
    indexedDB: { open(name, version) {
      assert.equal(name, 'console-hub-art-v02'); assert.equal(version, 1);
      opens++; return opened;
    } },
    URL: {
      createObjectURL(blob) {
        assert.ok(blob instanceof PreviewBlob);
        const uri = `blob:preview-${created.length + 1}`;
        created.push(blob.label); live.set(uri, blob.label); return uri;
      },
      revokeObjectURL(uri) { revoked.push(uri); live.delete(uri); },
    },
    onToast: message => messages.push(message), onRender: art => renders.push(art),
  });
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup), 'Update the harness when startup changes');
  vm.runInContext(source.replace(startup, '') + `
    toast = onToast;
    render = () => onRender(new Map(state.art));
    globalThis.api = { state, initializeArt, loadCover };
  `, context);
  context.api.state.selectedGame = { id: 'ps2', uid: 'test.iso' };
  return { ...context.api, key: 'ps2|test.iso', reading, writes, created, revoked, live, messages, renders,
    opens: () => opens,
    async openDatabase() { opened.result = database; opened.onsuccess(); await tick(); },
    finishRead(items) { reading.result = items; reading.onsuccess(); },
  };
}

test('preview startup preserves a cover chosen before the database opens', async () => {
  const f = fixture(), startup = f.initializeArt();
  await f.loadCover(new PreviewBlob('latest'));
  const latest = f.state.art.get(f.key);
  assert.match(f.messages.at(-1), /updated for this session/);
  await f.openDatabase();
  f.finishRead([
    { key: f.key, blob: new PreviewBlob('old snapshot') },
    { key: 'ps2|other.iso', blob: new PreviewBlob('unchanged') },
  ]);
  await startup;
  assert.equal(f.state.art.get(f.key), latest);
  assert.equal(f.renders.at(-1).get(f.key), latest);
  assert.deepEqual(f.created, ['latest', 'unchanged']);
  assert.deepEqual(f.revoked, []);
  assert.equal(f.live.size, 2);
  assert.equal(f.writes.length, 0, 'A session-only selection must not be queued for a later write');
});

test('preview startup preserves a newer selection while its storage write is pending', async () => {
  const f = fixture(), startup = f.initializeArt();
  await f.openDatabase();
  const selection = f.loadCover(new PreviewBlob('latest'));
  const latest = f.state.art.get(f.key);
  f.finishRead([{ key: f.key, blob: new PreviewBlob('old snapshot') }]);
  await startup;
  assert.equal(f.state.art.get(f.key), latest);
  assert.equal(f.renders.at(-1).get(f.key), latest);
  assert.deepEqual(f.created, ['latest']);
  assert.deepEqual(f.revoked, []);
  assert.deepEqual(f.writes.map(item => [item.key, item.blob.label]), [[f.key, 'latest']]);
  f.writes[0].tx.oncomplete(); await selection;
  assert.equal(f.state.art.get(f.key), latest);
  assert.match(f.messages.at(-1), /saved to this browser/);
});

test('unchanged preview covers hydrate and later replacement revokes the old URL once', async () => {
  const f = fixture(), startup = f.initializeArt();
  await f.openDatabase();
  f.finishRead([{ key: f.key, blob: new PreviewBlob('stored') }, { key: 'invalid', blob: null }]);
  await startup;
  const previous = f.state.art.get(f.key);
  assert.equal(f.live.get(previous), 'stored');
  assert.equal(f.state.art.has('invalid'), false);
  const selection = f.loadCover(new PreviewBlob('replacement'));
  f.writes[0].tx.oncomplete(); await selection;
  assert.deepEqual(f.created, ['stored', 'replacement']);
  assert.deepEqual(f.revoked, [previous]);
  assert.equal(f.live.size, 1);
  assert.equal(f.live.get(f.state.art.get(f.key)), 'replacement');
});

test('preview initialization failure leaves the current in-memory choice intact', async () => {
  const f = fixture(), startup = f.initializeArt();
  await f.loadCover(new PreviewBlob('latest'));
  const latest = f.state.art.get(f.key);
  await f.openDatabase();
  f.reading.error = Error('Synthetic unavailable storage'); f.reading.onerror();
  await startup;
  assert.equal(f.state.art.get(f.key), latest);
  assert.deepEqual(f.revoked, []);
  await f.loadCover(new PreviewBlob('replacement'));
  assert.equal(f.writes.length, 0);
  assert.match(f.messages.at(-1), /updated for this session/);
});

test('native initialization never opens the preview artwork database', async () => {
  const f = fixture(); f.state.native = true;
  await f.initializeArt();
  assert.equal(f.opens(), 0);
  assert.deepEqual(f.created, []);
  assert.equal(f.renders.length, 0);
});
