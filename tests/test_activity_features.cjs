const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(__dirname+'/../web/app.js','utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
const tick=()=>new Promise(r=>setImmediate(r));
function fixture(invoke=async()=>({})){
 const nodes={},calls=[],messages=[],events=[];for(const s of ['#gaming-status','#ra-message','#statistics-content'])nodes[s]={hidden:true,textContent:'',innerHTML:'',dataset:{}};
 const doc={documentElement:{dataset:{}},querySelector:s=>nodes[s]||null,querySelectorAll:()=>[],activeElement:null,hidden:false};
 const ctx=vm.createContext({consoles:[{id:'gb',name:'Game Boy',games:[]},{id:'ps2',name:'PlayStation 2',games:[]}],window:{__TAURI__:{core:{invoke:async(n,a)=>{calls.push([n,a]);return invoke(n,a);}}}},document:doc,localStorage:{setItem(){}},setTimeout(){},clearTimeout(){},setInterval(){},clearInterval(){},Date});
 vm.runInContext(source+';toast=x=>globalThis.messages.push(x);globalThis.api={state,applyGamingState,gamingPaused,pollGamingMode,refreshManagedAndGames,pollLibraries,pollPlaytime,checkForUpdateNotice,scanArtwork,refreshNative,handleAction,visibleGames,libraryPrefs,saveGameProgress,progressBadge,activitySummary,loadStatistics,loadRAProgress,loadRAGame,openRADashboard,saveRAConnection,metadataControls,gameMetadata};',ctx);ctx.messages=messages;
 return {...ctx.api,nodes,calls,messages,doc,ctx,events};
}
test('gaming mode prevents scheduled scans, downloads, update checks and playtime UI polling',async()=>{
 const f=fixture();f.state.gaming={active:true,emulators:['PCSX2']};f.state.artwork.enabled=true;
 await Promise.all([f.refreshManagedAndGames(),f.pollLibraries(),f.scanArtwork(),f.checkForUpdateNotice(),f.pollPlaytime()]);assert.equal(f.calls.length,0);assert.equal(f.gamingPaused(),true);
});
test('detector changes pause animation state and show which emulator is open',async()=>{
 const f=fixture(async()=>({active:true,emulators:['Dolphin']}));await f.pollGamingMode();assert.equal(f.doc.documentElement.dataset.gaming,'active');assert.equal(f.nodes['#gaming-status'].hidden,false);assert.match(f.nodes['#gaming-status'].textContent,/Dolphin/);
});
test('resuming preserves the user-paused artwork queue',async()=>{
 const f=fixture();f.state.coverQueue.paused=true;f.state.coverQueue.items=[{status:'pending'}];f.state.page='settings';f.applyGamingState({active:true,emulators:['PCSX2']});f.applyGamingState({active:false,emulators:[]});await tick();assert.equal(f.doc.documentElement.dataset.gaming,'idle');assert.equal(f.state.coverQueue.paused,true);assert.ok(!f.calls.some(c=>c[0]==='download_cover'));
});
test('an in-flight cover finishes but no next cover starts while gaming',async()=>{
 let resolve;const f=fixture((n)=>n==='download_cover'?new Promise(r=>resolve=r):null);
 f.state.coverQueue.items=[{kind:'ps2',uid:'a',title:'A',status:'pending'},{kind:'ps2',uid:'b',title:'B',status:'pending'}];
 const work=f.scanArtwork(true);await tick();assert.equal(f.calls[0][1].background,true);f.applyGamingState({active:true,emulators:['PCSX2']});resolve({status:'local'});await work;
 assert.equal(f.calls.filter(c=>c[0]==='download_cover').length,1);assert.equal(f.state.coverQueue.items[1].status,'pending');assert.equal(f.state.coverQueue.paused,false);
});
test('repeated Resume and quick Pause/Resume share one artwork worker',async()=>{
 const requests=[];let active=0,maximum=0;
 const f=fixture(n=>n==='download_cover'?new Promise(resolve=>{active++;maximum=Math.max(maximum,active);requests.push(result=>{active--;resolve(result);});}):null);
 f.state.coverQueue.items=['a','b'].map(uid=>({kind:'ps2',uid,title:uid,status:'pending'}));
 const work=f.scanArtwork(true);await tick();
 for(let n=0;n<10;n++)assert.equal(f.scanArtwork(true),work);
 await f.handleAction({dataset:{action:'queue-pause'}});
 await f.handleAction({dataset:{action:'queue-resume'}});
 assert.equal(requests.length,1);
 requests[0]({status:'local'});await tick();assert.equal(requests.length,2);
 requests[1]({status:'local'});await work;
 assert.equal(maximum,1);assert.deepEqual(f.calls.filter(c=>c[0]==='download_cover').map(c=>c[1].uid),['a','b']);
});
test('fresh artwork scans wait for an in-flight download and use the latest library',async()=>{
 let release,active=0,maximum=0;const f=fixture(n=>{
  if(n!=='download_cover')return null;
  active++;maximum=Math.max(maximum,active);
  if(!release)return new Promise(resolve=>{release=()=>{active--;resolve({status:'local'});};});
  active--;return {status:'local'};
 });
 f.state.imported.ps2=[{uid:'old',title:'Old'}];const work=f.scanArtwork();await tick();
 f.state.imported.ps2=[{uid:'new',title:'New'}];assert.equal(f.scanArtwork(),work);assert.equal(f.scanArtwork(),work);
 assert.equal(f.calls.length,1);release();await work;
 assert.equal(maximum,1);assert.deepEqual(f.calls.map(c=>c[1].uid),['old','new']);assert.equal(f.state.coverQueue.items[0].uid,'new');
});
for(const manuallyPaused of [false,true])test(`manual refresh during gaming retains cover jobs (queue paused=${manuallyPaused})`,async()=>{
 const f=fixture(n=>n==='library_snapshot'?{games:{ps2:[{uid:'new',title:'New'}]},folders:{}}:n==='download_cover'?{status:'local'}:{});
 vm.runInContext('render=()=>{};',f.ctx);f.state.page='settings';f.state.librarySnapshotSupported=true;
 f.state.gaming.active=true;f.state.coverQueue.paused=manuallyPaused;f.state.coverQueue.items=[{kind:'ps2',uid:'old',status:'local'}];
 await f.refreshNative();assert.equal(f.calls.filter(c=>c[0]==='download_cover').length,0);
 f.applyGamingState({active:false,emulators:[]});await tick();
 if(manuallyPaused){assert.equal(f.calls.filter(c=>c[0]==='download_cover').length,0);f.state.coverQueue.paused=false;}
 await f.scanArtwork(true);
 assert.deepEqual(f.calls.filter(c=>c[0]==='download_cover').map(c=>c[1].uid),['new']);
});
test('progress filters compose with favorites and retain separate console identities',()=>{
 const f=fixture();f.state.imported.gb=[{uid:'one',title:'Red'},{uid:'two',title:'Blue'}];f.libraryPrefs().metadata['gb|one']={progress:'completed'};f.state.progressFilter='completed';assert.deepEqual(Array.from(f.visibleGames('gb'),g=>g.uid),['one']);f.state.gameFilter='Favorites';assert.equal(f.visibleGames('gb').length,0);f.state.favorites.add('gb|one');assert.equal(f.visibleGames('gb').length,1);assert.match(f.progressBadge('gb','one'),/Completed/);assert.equal(f.progressBadge('ps2','one'),'');
});
test('saving progress keeps display titles and grouping metadata',async()=>{
 const f=fixture();f.state.page='settings';f.libraryPrefs().metadata['gb|one']={display:'My title',coverTitle:'Catalog title',group:'Version set'};
 const field={disabled:false,value:'playing',dataset:{id:'gb',uid:'one'}};await f.saveGameProgress(field);assert.equal(f.gameMetadata('gb','one').progress,'playing');assert.equal(f.gameMetadata('gb','one').display,'My title');assert.equal(f.calls[0][1].value.group,'Version set');assert.equal(field.disabled,false);
});
test('failed progress save cannot roll back a newer metadata edit',async()=>{
 let reject;const f=fixture(()=>new Promise((_,r)=>reject=r));f.state.page='settings';f.libraryPrefs().metadata['gb|one']={display:'Original'};
 const field={disabled:false,value:'completed',dataset:{id:'gb',uid:'one'}};const work=f.saveGameProgress(field);await tick();f.libraryPrefs().metadata['gb|one']={display:'New title',progress:'playing'};reject(Error('disk full'));await work;assert.equal(f.gameMetadata('gb','one').display,'New title');assert.equal(field.value,'playing');
});
test('statistics retain legacy totals without inventing daily history and exclude retired PC libraries',()=>{
 const f=fixture();f.state.imported.gb=[{uid:'red',title:'Pokémon Red'}];const data={totals:{'gb|red':50*3600000,'ps2|game':3600000,'windows|old':999},daily:{'2026-11-12':{'gb|red':600000,'windows|old':999}}};
 const s=f.activitySummary(data,new Date(2026,10,12,12));assert.equal(s.total,51*3600000);assert.equal(s.week,600000);assert.equal(s.ranked[0].title,'Pokémon Red');assert.equal(s.days.length,7);assert.equal(s.days[0].day,'2026-11-06');assert.equal(s.consoles.length,2);
});
test('a late statistics response does not replace another dialog',async()=>{
 let resolve;const f=fixture(()=>new Promise(r=>resolve=r));f.state.overlay='statistics';const old=f.nodes['#statistics-content'];const work=f.loadStatistics();await tick();f.state.overlay='configure';f.nodes['#statistics-content']={innerHTML:'New dialog'};resolve({totals:{},daily:{}});await work;assert.equal(old.innerHTML,'');assert.equal(f.nodes['#statistics-content'].innerHTML,'New dialog');
});
test('an older statistics error cannot replace a newer successful refresh',async()=>{
 const requests=[];const f=fixture(()=>new Promise((resolve,reject)=>requests.push({resolve,reject})));f.state.overlay='statistics';
 const first=f.loadStatistics(),second=f.loadStatistics();requests[1].resolve({totals:{'gb|red':50*3600000},daily:{}});await second;
 const current=f.nodes['#statistics-content'].innerHTML;assert.match(current,/50 h played/);
 requests[0].reject(Error('old failure'));await first;
 assert.equal(f.nodes['#statistics-content'].innerHTML,current);assert.equal(f.nodes['#statistics-content'].textContent,'');
});
test('the current statistics failure remains visible',async()=>{
 const f=fixture(async()=>{throw Error('cannot read settings');});f.state.overlay='statistics';await f.loadStatistics();
 assert.match(f.nodes['#statistics-content'].textContent,/cannot read settings/);
});
function accountFixture(invoke){
 const f=fixture(invoke),username={value:'',disabled:false},key={value:'',disabled:false},controls=[username,key,{disabled:false}];
 f.nodes['#ra-username']=username;f.nodes['#ra-key']=key;f.nodes['#ra-content']={innerHTML:''};f.nodes['#overlay-root']={innerHTML:''};
 f.nodes['.ra-account']={open:true};f.nodes['.achievements-dialog']={dataset:{},querySelectorAll:()=>controls};f.nodes['.achievements-dialog .close']={focus(){}};
 return {...f,username,key,controls};
}
for(const outcome of ['resolve','reject'])test(`old achievement load ${outcome} cannot unlock a newer account save`,async()=>{
 let progress,save;let progressCalls=0;
 const f=accountFixture(n=>n==='ra_status'?{connected:true,username:'User'}:n==='ra_save_account'?new Promise(resolve=>save=resolve):n==='ra_progress'?(++progressCalls===1?new Promise((resolve,reject)=>progress={resolve,reject}):{games:[],total:0}):null);
 const opening=f.openRADashboard();await tick();f.key.value='submitted-fixture-key';const saving=f.saveRAConnection();await tick();
 assert.ok(f.controls.every(c=>c.disabled));progress[outcome](outcome==='resolve'?{games:[],total:0}:Error('stale network error'));await opening;
 assert.equal(f.nodes['.achievements-dialog'].dataset.saving,'true');assert.ok(f.controls.every(c=>c.disabled));
 save({connected:true,username:'User'});await saving;assert.ok(f.controls.every(c=>!c.disabled));assert.equal(f.key.value,'');
});
test('failed account save retains its draft and error without starting another refresh',async()=>{
 const f=accountFixture(n=>n==='ra_status'?{connected:true,username:'User'}:n==='ra_progress'?{games:[],total:0}:Promise.reject(Error('invalid fixture key')));
 await f.openRADashboard();f.key.value='draft';const before=f.calls.length;await f.saveRAConnection();
 assert.equal(f.key.value,'draft');assert.match(f.nodes['#ra-message'].textContent,/invalid fixture key/);assert.deepEqual(f.calls.slice(before).map(c=>c[0]),['ra_save_account']);assert.ok(f.controls.every(c=>!c.disabled));
});
test('achievement pages and details render escaped titles and correct unlock counts',async()=>{
 const f=fixture(n=>n==='ra_progress'?{total:1,games:[{gameId:1,title:'<Game>',consoleName:'Game Boy',earned:4,possible:10,hardcore:2}],cached:false,warning:''}:{gameId:1,title:'<Game>',consoleName:'Game Boy',earned:1,possible:2,hardcore:0,achievements:[{title:'<First>',description:'Do <thing>',points:5,earned:true,hardcore:false}],warning:''});
 f.nodes['#ra-content']={innerHTML:''};f.state.overlay='ra-dashboard';f.state.ra={connected:true,username:'TestUser',offset:0};await f.loadRAProgress();assert.match(f.nodes['#ra-content'].innerHTML,/&lt;Game&gt;/);assert.match(f.nodes['#ra-content'].innerHTML,/4 \/ 10/);await f.loadRAGame(1);assert.match(f.nodes['#ra-content'].innerHTML,/&lt;First&gt;/);assert.match(f.nodes['#ra-content'].innerHTML,/Unlocked/);
});
test('gaming mode defers achievement network requests',async()=>{
 const f=fixture();f.nodes['#ra-content']={innerHTML:''};f.state.ra={connected:true};f.state.gaming.active=true;await f.loadRAProgress(0);await f.loadRAGame(1);assert.equal(f.calls.length,0);assert.match(f.nodes['#ra-message'].textContent,/Gaming mode/);
});
test('late achievement responses cannot overwrite a newly opened view',async()=>{
 let resolve;const f=fixture(()=>new Promise(r=>resolve=r));f.nodes['#ra-content']={innerHTML:'Original'};f.state.overlay='ra-dashboard';f.state.ra={connected:true};const work=f.loadRAProgress();await tick();f.state.overlay='statistics';f.nodes['#ra-content']={innerHTML:'New'};resolve({total:0,games:[]});await work;assert.equal(f.nodes['#ra-content'].innerHTML,'New');
});
