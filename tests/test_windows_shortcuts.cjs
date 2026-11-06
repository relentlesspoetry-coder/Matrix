// Exercise the production Windows-library UI without starting external software.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');

function fixture() {
  const calls = [], nodes = new Map();
  for (const selector of ['#screen', '#overlay-root', '#launch-status', '#windows-name', '.detail-panel .close']) {
    nodes.set(selector, { innerHTML: '', focus() {} });
  }
  const context = vm.createContext({
    consoles: [],
    document: { activeElement: null, querySelector: selector => nodes.get(selector) || null },
    window: { __TAURI__: { core: { invoke: async (name, args) => {
      calls.push({ name, args }); return { phase: 'exited', message: 'Fixture exited' };
    } } } },
  });
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(source.replace(startup, '') + `
    hydrateNativeCovers = () => {}; gameCover = () => ''; toast = () => {};
    globalThis.matrix = { state, windowsShortcutSupported, renderExternal, openExternalGame, openAddWindows, handleAction };
  `, context);
  return { ...context.matrix, calls, nodes };
}
const legacy = { uid: '/games/Old.exe', target: '/games/Old.exe', title: 'Legacy game', runner: 'retired-runtime' };
const shortcut = { uid: '/games/New.desktop', target: '/games/New.desktop', title: 'Shortcut game', runner: 'desktop' };

test('Windows registration offers only an existing desktop shortcut', () => {
  const f = fixture(); f.openAddWindows();
  const html = f.nodes.get('#overlay-root').innerHTML;
  assert.match(html, /Existing desktop shortcut/);
  assert.match(html, /Direct \.exe launching is unavailable/);
  assert.doesNotMatch(html, /<select|windows-runner/);
});

test('shortcut detection rejects legacy runners, executables and missing targets', () => {
  const f = fixture();
  assert.equal(f.windowsShortcutSupported(shortcut), true);
  assert.equal(f.windowsShortcutSupported({ ...shortcut, target: '/games/Upper.DESKTOP' }), true);
  for (const game of [legacy, { ...legacy, runner: 'desktop' }, { ...shortcut, runner: 'retired-runtime' }, { runner: 'desktop' }, null]) {
    assert.equal(f.windowsShortcutSupported(game), false);
  }
});

test('legacy cards stay visible without displaying retired runtime names', () => {
  const f = fixture(); f.state.windows = [legacy, shortcut]; f.renderExternal('windows');
  const html = f.nodes.get('#screen').innerHTML;
  assert.match(html, /View Legacy game/);
  assert.match(html, /Shortcut required/);
  assert.match(html, /Launch Shortcut game/);
  assert.doesNotMatch(html, /retired-runtime/);
  assert.equal(f.state.windows[0], legacy);
});

test('clicking a legacy card explains replacement and never invokes a launcher', async () => {
  const f = fixture(); f.state.windows = [legacy];
  await f.handleAction({ dataset: { action: 'quick-external', kind: 'windows', uid: legacy.uid } });
  assert.equal(f.calls.length, 0);
  const html = f.nodes.get('#overlay-root').innerHTML;
  assert.match(html, /Direct \.exe launching is unavailable/);
  assert.match(html, /data-action="launch-external"[^>]*disabled/);
  assert.match(html, /data-action="remove-windows"/);
  assert.match(html, /data-action="pick-cover"/);
});

test('desktop shortcuts and Steam retain their separate launch commands', async () => {
  for (const [kind, game, command] of [['windows', shortcut, 'launch_windows_game'], ['steam', { uid: '42', title: 'Steam game' }, 'launch_steam_game']]) {
    const f = fixture(); f.state.page = kind; f.state[kind] = [game];
    await f.handleAction({ dataset: { action: 'quick-external', kind, uid: game.uid } });
    assert.equal(f.calls.length, 1);
    assert.equal(f.calls[0].name, command);
    assert.equal(f.calls[0].args.uid, game.uid);
  }
});
