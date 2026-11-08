// Exercise production preference functions in independent tabs sharing storage.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {test} = require('node:test');
const source = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
const preferenceSource = source.slice(source.indexOf('const preferenceMap ='), source.indexOf('function evictCover('));
function fixture(initial = {}) {
  const values = new Map(Object.entries(initial)), messages = [];
  let failing = false;
  const storage = {
    getItem(key) { if (failing) throw Error('Disabled storage'); return values.get(key) || null; },
    setItem(key, value) { if (failing) throw Error('Quota exceeded'); values.set(key, value); },
  };
  return {
    values, messages, fail() { failing = true; },
    saved: () => JSON.parse(values.get('matrix-ui-v03')),
    tab() {
      const ctx = vm.createContext({state:{configs:{}, imported:{}, favorites:new Set()}, localStorage:storage,
        favoriteKey:(id, uid)=>`${id}|${uid}`, toast:message=>messages.push(message)});
      vm.runInContext(preferenceSource + ';readPrefs();', ctx);
      return {ctx, state:ctx.state, save:change=>ctx.savePrefs(change)};
    },
  };
}
test('stale tabs preserve independent profiles, imports and favorite intents', () => {
  const f=fixture(), a=f.tab(), b=f.tab();
  a.save({type:'profile',id:'ps2',value:{emulatorName:'A'}});
  b.save({type:'favorite',key:'ps2|one',favorite:true});
  b.save({type:'profile',id:'n64',value:{emulatorName:'B'}});
  a.save({type:'import',id:'ps2',value:[{uid:'one'}]});
  b.save({type:'import',id:'n64',value:[{uid:'two'}]});
  a.save({type:'favorite',key:'ps2|two',favorite:true});
  b.save({type:'favorite',key:'ps2|one',favorite:false});
  assert.deepEqual(f.saved(), {configs:{ps2:{emulatorName:'A'},n64:{emulatorName:'B'}},
    imported:{ps2:[{uid:'one'}],n64:[{uid:'two'}]}, favorites:['ps2|two']});
});
test('same-console replacements and removals follow the latest action without resurrection', () => {
  const f=fixture(), a=f.tab(), b=f.tab();
  a.save({type:'profile',id:'ps2',value:{emulatorName:'old'}});
  b.save({type:'profile',id:'ps2',value:{emulatorName:'new'}});
  a.save({type:'import',id:'ps2',value:[{uid:'old'}]});
  b.save({type:'import',id:'ps2',value:[{uid:'new'}]});
  a.save({type:'import',id:'ps2'});
  b.save({type:'favorite',key:'ps2|new',favorite:true});
  assert.deepEqual(f.saved(), {configs:{ps2:{emulatorName:'new'}},imported:{},favorites:['ps2|new']});
});
test('malformed map fields recover without discarding valid neighboring fields', () => {
  for (const field of ['configs','imported']) {
    const valid={configs:{ps2:{emulatorName:'keep'}}, imported:{n64:[{uid:'keep'}]}, favorites:['ps2|keep']};
    valid[field]=[];
    const f=fixture({'matrix-ui-v03':JSON.stringify(valid)}), a=f.tab();
    assert.equal(Array.isArray(a.state[field]),false);
    a.save({type:field==='configs'?'profile':'import',id:'ps2',value:field==='configs'?{emulatorName:'new'}:[{uid:'new'}]});
    const saved=f.saved();
    assert.deepEqual(saved.favorites,['ps2|keep']);
    assert.deepEqual(saved[field==='configs'?'imported':'configs'],valid[field==='configs'?'imported':'configs']);
    assert.ok(f.tab().state[field].ps2);
  }
});
test('legacy migration rejects array maps and non-string favorites independently', () => {
  const f=fixture({'console-hub-ui-v1':JSON.stringify({configs:[],favorites:[null,'ps2:2',9]})}), a=f.tab();
  assert.equal(Array.isArray(a.state.configs),false);
  assert.deepEqual([...a.state.favorites],['ps2|demo-2']);
  a.save({type:'profile',id:'ps2',value:{emulatorName:'new'}});
  assert.deepEqual(f.saved().favorites,['ps2|demo-2']);
});
test('malformed JSON recovers; unavailable storage reports failure without success', () => {
  const f=fixture({'matrix-ui-v03':'{broken'}), a=f.tab();
  assert.equal(a.save({type:'favorite',key:'ps2|new',favorite:true}),true);
  const before=f.values.get('matrix-ui-v03');
  f.fail();
  assert.equal(a.save({type:'import',id:'ps2',value:[]}),false);
  assert.equal(f.values.get('matrix-ui-v03'),before);
  assert.match(f.messages.at(-1),/could not persist/);
  assert.doesNotThrow(()=>f.tab());
});
test('native favorite saves do not touch browser storage or report quota failures', async () => {
  const f=fixture({'matrix-ui-v03':JSON.stringify({configs:{},imported:{ps2:[{uid:'preview'}]},favorites:[]})}), a=f.tab();
  const before=f.values.get('matrix-ui-v03'); f.fail();
  Object.assign(a.ctx,{nativeInvoke:async(name,args)=>{assert.equal(name,'set_favorite');return [args.uid];},
    byId:{ps2:{name:'PlayStation 2'}}, renderConsole(){}, $(){return null;}});
  a.state.native=true;
  vm.runInContext('let favoriteWriteRevision=0;'+source.slice(source.indexOf('let favoriteQueue='),source.indexOf('function gameCard(')),a.ctx);
  const button={}; a.ctx.toggleFavorite('ps2','native.iso',button);
  await vm.runInContext('favoriteQueue',a.ctx);
  assert.equal(f.values.get('matrix-ui-v03'),before);
  assert.equal(button.disabled,false);
  assert.ok(a.state.favorites.has('ps2|native.iso'));
  assert.equal(f.messages.some(message=>message.includes('could not persist')),false);
});
test('malformed current document retains compatible legacy recovery', () => {
  const f=fixture({'matrix-ui-v03':'{broken','console-hub-ui-v1':JSON.stringify({configs:{ps2:{emulatorName:'legacy'}},favorites:['ps2:3']})});
  const a=f.tab();
  assert.equal(a.state.configs.ps2.emulatorName,'legacy');
  a.save({type:'favorite',key:'ps2|demo-4',favorite:true});
  assert.equal(f.saved().configs.ps2.emulatorName,'legacy');
  assert.deepEqual(f.saved().favorites,['ps2|demo-3','ps2|demo-4']);
});
