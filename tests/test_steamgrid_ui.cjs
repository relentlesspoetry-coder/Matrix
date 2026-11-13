const test=require('node:test');const assert=require('node:assert/strict');const fs=require('node:fs');const path=require('node:path');const vm=require('node:vm');
function fixture(invoke){
 const nodes={'.steamgrid-dialog':{_selection:{id:'ps2',uid:'/mock/Game.iso'},dataset:{}},'#steamgrid-query':{value:'Game'},'#steamgrid-message':{textContent:''},'#steamgrid-results':{innerHTML:''}};
 const calls=[];
 const context=vm.createContext({consoles:[{id:'ps2'}],document:{querySelector:s=>nodes[s]},window:{__TAURI__:{core:{invoke:(name,args)=>{calls.push([name,args]);return invoke(name,args);}}}}});
 const source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
 vm.runInContext(source+';render=()=>{};dismissOverlay=()=>{};clearCoverCache=()=>{};toast=()=>{};globalThis.api={state,steamGridAction,scanArtwork,handleAction,stubFetch(fn){fetchArtwork=fn;updateCoverStatus=()=>{};}};',context);
 return {nodes,calls,...context.api};
}
test('SteamGridDB picker escapes titles and saves the chosen cover for the original game',async()=>{
 const f=fixture(async name=>name==='steamgrid_search'?[{id:7,name:'Game <Edition>'}]:name==='steamgrid_grids'?[{id:9,url:'https://cdn2.steamgriddb.com/grid/test.png'}]:null);
 const button=action=>({dataset:{action,gameId:'7',gridId:'9'},disabled:false});
 await f.steamGridAction(button('steamgrid-search'));
 assert.match(f.nodes['#steamgrid-results'].innerHTML,/Game &lt;Edition&gt;/);
 await f.steamGridAction(button('steamgrid-game'));
 assert.match(f.nodes['#steamgrid-results'].innerHTML,/data-grid-id="9"/);
 await f.steamGridAction(button('steamgrid-cover'));
 assert.equal(JSON.stringify(f.calls.at(-1)),JSON.stringify(['steamgrid_save',{kind:'ps2',uid:'/mock/Game.iso',gameId:7,gridId:9}]));
});
test('late SteamGridDB responses cannot replace a newer dialog',async()=>{
 let resolve;const f=fixture(()=>new Promise(r=>{resolve=r;}));
 const pending=f.steamGridAction({dataset:{action:'steamgrid-search'},disabled:false});
 f.nodes['.steamgrid-dialog']={dataset:{}};
 resolve([{id:7,name:'Old response'}]);await pending;
 assert.equal(f.nodes['#steamgrid-results'].innerHTML,'');
});

test('automatic scans stop after a rejected key or rate limit',async()=>{
 for(const message of ['SteamGridDB API key was rejected.','SteamGridDB rate limit reached.']){
  const f=fixture(()=>{});let calls=0;f.state.native=true;f.state.steamgridEnabled=true;
  f.state.imported.ps2=[{uid:'one'},{uid:'two'}];
  f.stubFetch(async()=>{calls++;throw Error(message);});
  await f.scanArtwork();assert.equal(calls,1);
 }
});

test('key save survives navigation and preserves a newer draft',async()=>{
 for(const navigate of [false,true]){
  let resolve;const f=fixture(()=>new Promise(r=>{resolve=r;}));
  const owner={value:'original-key'};f.nodes['#steamgrid-key']=owner;
  const pending=f.handleAction({dataset:{action:'steamgrid-key-save'},disabled:false});
  if(navigate)delete f.nodes['#steamgrid-key'];else owner.value='new draft';
  resolve(true);await pending;
  assert.equal(owner.value,navigate?'original-key':'new draft');
  assert.equal(f.state.steamgridEnabled,true);
 }
});
test('cover queue resumes interrupted work and retries failures without repeating completed jobs',async()=>{
 const f=fixture(()=>{});f.state.native=true;f.state.steamgridEnabled=true;
 f.state.imported.ps2=[{uid:'one'},{uid:'two'},{uid:'three'}];
 const calls=[];let first=true;
 f.stubFetch(async(kind,uid)=>{calls.push(uid);if(uid==='two'&&first){first=false;throw Error('temporary error');}return {status:'saved'};});
 await f.scanArtwork();
 assert.equal(f.state.coverQueue.items[1].status,'failed');
 await f.scanArtwork(true);
 assert.deepEqual(calls,['one','two','three','two']);
 let resolve;f.state.coverQueue.items=[];calls.length=0;
 f.stubFetch((kind,uid)=>{calls.push(uid);return new Promise(r=>{resolve=r;});});
 const pending=f.scanArtwork();
 await f.handleAction({dataset:{action:'queue-pause'}});
 resolve({status:'saved'});await pending;
 assert.deepEqual(calls,['one']);assert.equal(f.state.coverQueue.paused,true);
 f.stubFetch(async(kind,uid)=>{calls.push(uid);return {status:'saved'};});
 f.state.coverQueue.paused=false;await f.scanArtwork(true);
 assert.deepEqual(calls,['one','two','three']);
});
