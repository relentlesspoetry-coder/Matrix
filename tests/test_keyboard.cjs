// Event-handler fixtures use the production code; the native suite additionally
// checks these controls in real WebKit with layout and keyboard dispatch.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
function fixture() {
  const listeners = {}, controls = [];
  const document = { activeElement: null, addEventListener(name, handler) { (listeners[name] ||= []).push(handler); },
    querySelector(selector) { assert.equal(selector, '#overlay-root'); return { querySelectorAll(selector) {
      // Model the tag selectors used for native sequential-focus candidates.
      return controls.filter(node => selector.split(',').some(part => part.trim().split(/[:[]/, 1)[0] === node.tag || (part.trim() === '[tabindex]' && node.explicitTabIndex)));
    } }; } };
  const context = vm.createContext({ consoles: [], window: { getComputedStyle: node => ({ visibility: node.visibility }) }, document });
  const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(source.replace(startup, '') + '\nstate.overlay="configure"; setupEvents();', context);
  function add(tag, props = {}) {
    const node = { tag, tabIndex: 0, visibility: 'visible', focus() { document.activeElement = node; },
      closest(selector) { return selector === 'details:not([open])' ? node.closedDetails || null : node.hidden || node.inert ? {} : null; },
      getClientRects() { return node.noLayout ? [] : [{}]; }, matches() { return !!node.disabled; }, ...props };
    controls.push(node); return node;
  }
  function tab(node, shiftKey = false, key = {}) {
    document.activeElement = node; let prevented = false;
    for (const handler of listeners.keydown) handler({ key: 'Tab', shiftKey, ...key, preventDefault() { prevented = true; } });
    return { prevented, active: document.activeElement };
  }
  return { add, tab };
}
for (const tag of ['summary', 'textarea']) {
  for (const shift of [false, true]) test(`${tag} permits normal ${shift ? 'reverse' : 'forward'} Tab inside a dialog`, () => {
    const f = fixture(); f.add('button'); const node = f.add(tag); f.add('input'); f.add('button');
    assert.equal(f.tab(node, shift).prevented, false, 'Interior control must not jump back to Close');
  });
}
test('dialog boundaries wrap only to visible, enabled sequential controls', () => {
  const f = fixture(); f.add('input', { hidden: true }); f.add('button', { disabled: true });
  const first = f.add('button'); f.add('summary'); f.add('textarea'); const last = f.add('button');
  f.add('input', { noLayout: true }); f.add('button', { tabIndex: -1 }); f.add('button', { visibility: 'hidden' });
  assert.equal(f.tab(first, true).active, last);
  assert.equal(f.tab(last).active, first);
});
test('focus outside the dialog is recovered in the requested direction', () => {
  const f = fixture(); const first = f.add('button'), last = f.add('button');
  const outside = { focus() {} };
  assert.equal(f.tab(outside).active, first);
  assert.equal(f.tab(outside, true).active, last);
});

for (const key of [{key:'Unidentified',code:'Tab'}, {key:'Unidentified',keyCode:9}]) {
  test(`GTK shifted Tab wraps using ${key.code ? 'code' : 'legacy keyCode'}`, () => {
    const f = fixture(); const first = f.add('button'), last = f.add('button');
    const result = f.tab(first, true, key);
    assert.equal(result.prevented, true); assert.equal(result.active, last);
  });
}
test('unrelated unidentified keys do not move dialog focus', () => {
  const f = fixture(); const first = f.add('button'); f.add('button');
  const result = f.tab(first, true, {key:'Unidentified',code:'KeyQ',keyCode:0});
  assert.equal(result.prevented, false); assert.equal(result.active, first);
});
test('closed details descendants are skipped even when WebKit retains their layout rectangles', () => {
  const f = fixture(); const first = f.add('button'), summary = f.add('summary');
  const details = { querySelector() { return summary; } }; summary.closedDetails = details;
  f.add('textarea', { closedDetails: details });
  assert.equal(f.tab(first, true).active, summary);
});
