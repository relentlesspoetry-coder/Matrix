// Exercise the production external-launch UI with deterministic timers/IPC.
// These fixtures are not real Steam, GTK or game execution.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');

function fixture(invoke) {
  const timers = [], calls = [], toasts = [];
  const status = { innerHTML: '' }, overlay = {}, nodes = new Map();
  let html = '';
  Object.defineProperty(overlay, 'innerHTML', { get: () => html, set(value) {
    html = value;
    overlay.firstElementChild = value ? {} : null;
    nodes.clear();
    if (value.includes('launch-dialog')) {
      const close = { focus() {} };
      nodes.set('.launch-dialog', { querySelector: () => close });
      for (const selector of ['#launch-message', '.launch-output', '#launch-log-path']) nodes.set(selector, {});
    }
  } });
  overlay.innerHTML = '';
  const context = vm.createContext({
    consoles: [{ id: 'ps2', name: 'PlayStation 2' }],
    window: { __TAURI__: { core: { invoke: async (name, args) => {
      calls.push({ name, args });
      return invoke(name, args);
    } } } },
    document: { querySelector(selector) {
      if (selector === '#launch-status') return status;
      if (selector === '#overlay-root') return overlay;
      return nodes.get(selector) || null;
    } },
    setTimeout: (fn, delay) => { timers.push({ fn, delay }); },
    onToast: message => toasts.push(message),
  });
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(source.replace(startup, '') + `
    toast = onToast;
    globalThis.matrix = { state, launchExternal, launchConsole, showLaunchDetails, renderLaunchStatus, dismissOverlay };
  `, context);
  const api = context.matrix;
  api.state.page = 'windows';
  api.state.native = true;
  return { ...api, timers, calls, toasts, status, overlay, nodes };
}
const started = { phase: 'started', message: 'Windows launcher process started.', logPath: '/tmp/fixture.log', log: '' };
const failed = { phase: 'error', message: 'Windows launcher exited with exit status 17.', logPath: '/tmp/fixture.log', log: 'gio: missing <launcher> <script>bad()</script>' };

test('a post-spawn Windows failure is polled, visible and safely escaped in details', async () => {
  const f = fixture(name => name === 'launch_windows_game' ? started : failed);
  f.state.overlay = 'external';
  await f.launchExternal('windows', '/games/Fixture.desktop');
  assert.equal(f.state.overlay, null);
  assert.match(f.status.innerHTML, /Windows launcher process started/);
  assert.equal(f.timers.length, 1);
  await f.timers.shift().fn();
  assert.equal(f.state.launches.windows.phase, 'error');
  assert.match(f.status.innerHTML, /launch-error/);
  assert.match(f.status.innerHTML, /Launch details/);
  assert.doesNotMatch(f.status.innerHTML, /Open emulator/);
  assert.equal(f.toasts.at(-1), failed.message);
  await f.showLaunchDetails('windows');
  assert.match(f.overlay.innerHTML, /&lt;script&gt;bad\(\)&lt;\/script&gt;/);
  assert.doesNotMatch(f.overlay.innerHTML, /<script>/);
  assert.doesNotMatch(f.overlay.innerHTML, /Open emulator/);
});

test('immediate Steam errors get launch details instead of a false success toast', async () => {
  const f = fixture(() => ({ ...failed, message: 'Steam launcher exited with exit status 3.' }));
  f.state.page = 'steam';
  await f.launchExternal('steam', '123');
  assert.equal(f.calls[0].name, 'launch_steam_game');
  assert.equal(f.calls[0].args.uid, '123');
  assert.match(f.status.innerHTML, /Steam launcher exited/);
  assert.equal(f.timers.length, 0);
  assert.doesNotMatch(f.toasts.join('\n'), /requested/);
});

test('external launches ignore repeated clicks until the current IPC returns', async () => {
  let release;
  const f = fixture(() => new Promise(resolve => { release = resolve; }));
  const first = f.launchExternal('windows', 'one');
  await f.launchExternal('windows', 'two');
  assert.equal(f.calls.length, 1);
  release({ ...started, phase: 'exited' });
  await first;
  assert.equal(f.state.launchBusy.windows, false);
});

test('a newer launch invalidates old queued polling', async () => {
  const f = fixture(() => started);
  await f.launchExternal('windows', 'one');
  const old = f.timers.shift();
  await f.launchExternal('windows', 'two');
  await old.fn();
  assert.equal(f.calls.length, 2);
});

test('late launcher failures remain monitored after the startup window', async () => {
  let polls = 0;
  const f = fixture(name => name === 'launch_windows_game' ? started : ++polls > 32 ? failed : started);
  await f.launchExternal('windows', 'one');
  for (let n = 0; n < 31; n++) await f.timers.shift().fn();
  assert.equal(f.timers[0].delay, 5000);
  while (f.timers.length) await f.timers.shift().fn();
  assert.equal(f.state.launches.windows.phase, 'error');
});

test('spawn rejection is a visible error and unrelated open dialogs are retained', async () => {
  const f = fixture(() => { throw new Error('Desktop launcher is missing'); });
  f.state.overlay = 'windows-add';
  await f.launchExternal('windows', 'one');
  assert.equal(f.state.launches.windows.phase, 'error');
  assert.equal(f.state.overlay, 'windows-add');
  assert.match(f.status.innerHTML, /Desktop launcher is missing/);
  assert.doesNotMatch(f.status.innerHTML, /Open emulator/);
});

test('a delayed launch does not dismiss a newer game dialog', async () => {
  let resolve;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  f.state.overlay = 'external'; f.overlay.innerHTML = 'Original game';
  const launch = f.launchExternal('windows', 'one');
  f.overlay.innerHTML = 'Another game';
  resolve(started); await launch;
  assert.equal(f.state.overlay, 'external');
  assert.equal(f.overlay.innerHTML, 'Another game');
});

test('closing launch details while status is pending does not reopen it', async () => {
  let resolve;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  f.state.launches.windows = started;
  const details = f.showLaunchDetails('windows');
  assert.equal(f.state.overlay, 'launch-details');
  f.dismissOverlay(); resolve(failed); await details;
  assert.equal(f.state.overlay, null); assert.equal(f.overlay.innerHTML, '');
});

test('an older launch-details response cannot replace a newer details dialog', async () => {
  const replies = [];
  const f = fixture(() => new Promise(done => { replies.push(done); }));
  f.state.launches.windows = started;
  f.state.launches.steam = { ...started, message: 'Steam started' };
  const older = f.showLaunchDetails('windows');
  const newer = f.showLaunchDetails('steam');
  replies[1]({ ...failed, message: 'Steam failed' }); await newer;
  const dialog = f.nodes.get('.launch-dialog');
  replies[0](failed); await older;
  assert.equal(f.nodes.get('.launch-dialog'), dialog);
  assert.equal(f.nodes.get('#launch-message').textContent, 'Steam failed');
});

test('launch-details refresh preserves its controls and cannot roll back a newer launch', async () => {
  let resolve;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  f.state.launches.windows = started;
  const details = f.showLaunchDetails('windows');
  const dialog = f.nodes.get('.launch-dialog');
  resolve(failed); await details;
  assert.equal(f.nodes.get('.launch-dialog'), dialog);
  assert.equal(f.nodes.get('.launch-output').textContent, failed.log);
  const stale = f.showLaunchDetails('windows');
  f.state.launchGeneration.windows = 1; f.state.launches.windows = started;
  resolve(failed); await stale;
  assert.equal(f.state.launches.windows, started);
});
