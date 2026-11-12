// Exercise the production console-launch UI with deterministic timers/IPC.
// These fixtures are not real emulator or game execution.
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
    consoles: [{ id: 'ps2', name: 'PlayStation 2' },{id:'n64',name:'Nintendo 64'}],
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
    globalThis.matrix = { state, launchConsole, launchConsole, showLaunchDetails, renderLaunchStatus, dismissOverlay };
  `, context);
  const api = context.matrix;
  api.state.page = 'console';api.state.consoleId='ps2';
  api.state.native = true;
  return { ...api, timers, calls, toasts, status, overlay, nodes };
}
const started = { phase: 'started', message: 'Emulator process started.', logPath: '/tmp/fixture.log', log: '' };
const failed = { phase: 'error', message: 'Emulator exited with exit status 17.', logPath: '/tmp/fixture.log', log: 'gio: missing <launcher> <script>bad()</script>' };

test('a post-spawn emulator failure is polled, visible and safely escaped in details', async () => {
  const f = fixture(name => name === 'launch_console_game' ? started : failed);
  f.state.overlay = 'game';
  await f.launchConsole('ps2', '/games/Fixture.desktop');
  assert.equal(f.state.overlay, null);
  assert.equal(f.status.innerHTML, "");
  assert.match(f.toasts.at(-1), /Emulator process started/);
  assert.equal(f.timers.length, 1);
  await f.timers.shift().fn();
  assert.equal(f.state.launches.ps2.phase, 'error');
  assert.match(f.status.innerHTML, /launch-error/);
  assert.match(f.status.innerHTML, /Launch details/);
  assert.match(f.status.innerHTML, /Open emulator/);
  assert.equal(f.toasts.at(-1), failed.message);
  await f.showLaunchDetails('ps2');
  assert.match(f.overlay.innerHTML, /&lt;script&gt;bad\(\)&lt;\/script&gt;/);
  assert.doesNotMatch(f.overlay.innerHTML, /<script>/);
  assert.match(f.overlay.innerHTML, /Open emulator/);
});

test('immediate console errors get launch details instead of a false success toast', async () => {
  const f = fixture(() => ({ ...failed, message: 'Emulator exited with exit status 3.' }));
  f.state.page = 'console';f.state.consoleId='n64';
  await f.launchConsole('n64', '123');
  assert.equal(f.calls[0].name, 'launch_console_game');
  assert.equal(f.calls[0].args.uid, '123');
  assert.match(f.status.innerHTML, /Emulator exited/);
  assert.equal(f.timers.length, 0);
  assert.doesNotMatch(f.toasts.join('\n'), /requested/);
});

test('console launches ignore repeated clicks until the current IPC returns', async () => {
  let release;
  const f = fixture(() => new Promise(resolve => { release = resolve; }));
  const first = f.launchConsole('ps2', 'one');
  await f.launchConsole('ps2', 'two');
  assert.equal(f.calls.length, 1);
  release({ ...started, phase: 'exited' });
  await first;
  assert.equal(f.state.launchBusy.ps2, false);
});

test('a newer launch invalidates old queued polling', async () => {
  const f = fixture(() => started);
  await f.launchConsole('ps2', 'one');
  const old = f.timers.shift();
  await f.launchConsole('ps2', 'two');
  await old.fn();
  assert.equal(f.calls.length, 2);
});

test('late launcher failures remain monitored after the startup window', async () => {
  let polls = 0;
  const f = fixture(name => name === 'launch_console_game' ? started : ++polls > 32 ? failed : started);
  await f.launchConsole('ps2', 'one');
  for (let n = 0; n < 31; n++) await f.timers.shift().fn();
  assert.equal(f.timers[0].delay, 5000);
  while (f.timers.length) await f.timers.shift().fn();
  assert.equal(f.state.launches.ps2.phase, 'error');
});

test('spawn rejection is a visible error and unrelated open dialogs are retained', async () => {
  const f = fixture(() => { throw new Error('Desktop launcher is missing'); });
  f.state.overlay = 'ps2-add';
  await f.launchConsole('ps2', 'one');
  assert.equal(f.state.launches.ps2.phase, 'needs-setup');
  assert.equal(f.state.overlay, 'ps2-add');
  assert.match(f.status.innerHTML, /Desktop launcher is missing/);
  assert.match(f.status.innerHTML, /Open emulator/);
});

test('a delayed launch does not dismiss a newer game dialog', async () => {
  let resolve;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  f.state.overlay = 'game'; f.overlay.innerHTML = 'Original game';
  const launch = f.launchConsole('ps2', 'one');
  f.overlay.innerHTML = 'Another game';
  resolve(started); await launch;
  assert.equal(f.state.overlay, 'game');
  assert.equal(f.overlay.innerHTML, 'Another game');
});

test('closing launch details while status is pending does not reopen it', async () => {
  let resolve;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  f.state.launches.ps2 = started;
  const details = f.showLaunchDetails('ps2');
  assert.equal(f.state.overlay, 'launch-details');
  f.dismissOverlay(); resolve(failed); await details;
  assert.equal(f.state.overlay, null); assert.equal(f.overlay.innerHTML, '');
});

test('an older launch-details response cannot replace a newer details dialog', async () => {
  const replies = [];
  const f = fixture(() => new Promise(done => { replies.push(done); }));
  f.state.launches.ps2 = started;
  f.state.launches.n64 = { ...started, message: 'Nintendo 64 started' };
  const older = f.showLaunchDetails('ps2');
  const newer = f.showLaunchDetails('n64');
  replies[1]({ ...failed, message: 'Nintendo 64 failed' }); await newer;
  const dialog = f.nodes.get('.launch-dialog');
  replies[0](failed); await older;
  assert.equal(f.nodes.get('.launch-dialog'), dialog);
  assert.equal(f.nodes.get('#launch-message').textContent, 'Nintendo 64 failed');
});

test('launch-details refresh preserves its controls and cannot roll back a newer launch', async () => {
  let resolve;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  f.state.launches.ps2 = started;
  const details = f.showLaunchDetails('ps2');
  const dialog = f.nodes.get('.launch-dialog');
  resolve(failed); await details;
  assert.equal(f.nodes.get('.launch-dialog'), dialog);
  assert.equal(f.nodes.get('.launch-output').textContent, failed.log);
  const stale = f.showLaunchDetails('ps2');
  f.state.launchGeneration.ps2 = 1; f.state.launches.ps2 = started;
  resolve(failed); await stale;
  assert.equal(f.state.launches.ps2, started);
});


test('successful launches do not leave a persistent status panel',()=>{
  const f=fixture(async()=>null);
  f.state.page='console';f.state.consoleId='ps2';f.state.launches.ps2={phase:'exited',message:'Game launcher completed.',logPath:'/tmp/fixture.log'};
  f.renderLaunchStatus();
  assert.equal(f.status.innerHTML,'');
});
