// Production cover controls with a deterministic DOM/IPC fixture. No network.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {test} = require('node:test');

function fixture() {
  const nodes = new Map(), calls = [];
  const node = key => {
    if (!nodes.has(key)) nodes.set(key, {innerHTML:'', textContent:'', focus(){}});
    return nodes.get(key);
  };
  const context = vm.createContext({
    document:{activeElement:null, querySelector:node, querySelectorAll:()=>[]},
    window:{__TAURI__:{core:{invoke:async(name,args)=>{
      calls.push({name,args}); return {status:'needs-title',message:'Exact title needed'};
    }}}},
  });
  const source = fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8');
  const data = fs.readFileSync(path.join(__dirname,'../web/data.js'),'utf8');
  const startup = 'readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(data+'\n'+source.replace(startup,'')+`
    hydrateNativeCovers=()=>{};
    globalThis.api={state,byId,openGame,renderSettings,coverMessage,scanArtwork,AUTO_COVER_KINDS,TITLE_COVER_KINDS};
  `,context);
  const api=context.api;
  for (const id of Object.keys(api.byId)) api.state.imported[id]=[{uid:id+'-game',title:'Title <fixture>',path:'/private/game'}];
  return {...api,node,calls};
}

test('all documented title catalogs have exact-title controls and flat-cover guidance',()=>{
  const f=fixture();
  assert.equal(f.TITLE_COVER_KINDS.length,11);
  assert.equal(f.AUTO_COVER_KINDS.length,16);
  for(const kind of f.TITLE_COVER_KINDS){
    f.openGame(kind,kind+'-game');
    const html=f.node('#overlay-root').innerHTML;
    assert.match(html,/Confirm exact catalog title/);
    assert.match(html,/maxlength="240"/);
    assert.match(html,/No fuzzy matching or region guessing/);
    assert.match(html,/Only flat covers/);
    assert.match(html,/Title &lt;fixture&gt;/);
  }
});

test('unverified platforms stay manual and are excluded from automatic scans',async()=>{
  const f=fixture();
  for(const kind of ['ps5','switch2']){
    f.openGame(kind,kind+'-game');
    const html=f.node('#overlay-root').innerHTML;
    assert.match(html,/No verified automatic provider/);
    assert.doesNotMatch(html,/data-action="download-cover"/);
    assert.match(html,/data-action="pick-cover"/);
  }
  f.state.artwork.titleEnabled=true;
  await f.scanArtwork();
  assert.equal(f.calls.length,16);
  assert.deepEqual(f.calls.map(c=>c.args.kind).sort(),Array.from(f.AUTO_COVER_KINDS).sort());
  assert.ok(f.calls.every(c=>c.name==='download_cover' && c.args.serial===null));
});

test('upgraded settings do not start filename-based requests until enabled',async()=>{
  const f=fixture();
  assert.equal(f.state.artwork.titleEnabled,false);
  await f.scanArtwork();
  assert.deepEqual(f.calls.map(c=>c.args.kind).sort(),['gamecube','ps1','ps2','switch','wii']);
  assert.match(f.coverMessage('gba','gba-game'),/Enable title-based covers/);
  f.state.artwork={enabled:true,style:'default'};
  f.calls.length=0;
  await f.scanArtwork();
  assert.equal(f.calls.length,5,'Missing legacy field is disabled, not implicitly opted in');
});

test('provider attribution and preferences accurately describe title disclosure and coverage limits',()=>{
  const f=fixture();
  for (const [kind,provider] of [['ps2','xlenore'],['wii','GameTDB'],['switch','GameTDB'],['vita','Libretro']]){
    f.state.coverStatus.set(kind+'|'+kind+'-game',{message:'Saved',url:'https://fixture',serial:'Exact Title',source:'user-confirmed'});
    assert.ok(f.coverMessage(kind,kind+'-game').includes(provider));
  }
  f.renderSettings();
  const html=f.node('#screen').innerHTML;
  assert.match(html,/filename-based catalog title/);
  assert.match(html,/full paths are not uploaded/);
  assert.match(html,/PS5 and Switch 2 still need local covers/);
  assert.match(html,/Local covers take priority/);
});

test('legacy providers retain serial controls and do not become title lookups',()=>{
  const f=fixture();
  for (const kind of ['ps1','ps2','gamecube','wii']){
    f.openGame(kind,kind+'-game');
    const html=f.node('#overlay-root').innerHTML;
    assert.match(html,/maxlength="16"/);
    assert.doesNotMatch(html,/Confirm exact catalog title/);
  }
});

test('Switch controls require exact product and artwork region, not title-ID guessing',()=>{
  const f=fixture();
  f.openGame('switch','switch-game');
  const html=f.node('#overlay-root').innerHTML;
  assert.match(html,/Confirm Switch product code and region/);
  assert.match(html,/placeholder="AAACA-US"/);
  assert.match(html,/not the 16-digit title ID/);
  assert.match(html,/Only flat covers/);
  assert.match(html,/no Switch 2 fallback/);
  assert.doesNotMatch(html,/Confirm exact catalog title/);
});
