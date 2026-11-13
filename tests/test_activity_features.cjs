const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(__dirname+'/../web/app.js','utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
const tick=()=>new Promise(r=>setImmediate(r));
function fixture(invoke=async()=>({})){
 const nodes={},calls=[],messages=[],events=[];for(const s of ['#gaming-status','#ra-message','#statistics-content'])nodes[s]={hidden:true,textContent:'',innerHTML:'',dataset:{}};
 const doc={documentElement:{dataset:{}},querySelector:s=>nodes[s]||null,querySelectorAll:()=>[],activeElement:null,hidden:false};
 const ctx=vm.createContext({consoles:[{id:'gb',name:'Game Boy',games:[]},{id:'ps2',name:'PlayStation 2',games:[]}],window:{__TAURI__:{core:{invoke:async(n,a)=>{calls.push([n,a]);return invoke(n,a);}}}},document:doc,localStorage:{setItem(){}},setTimeout(){},clearTimeout(){},setInterval(){},clearInterval(){},Date});
 vm.runInContext(source+';toast=x=>globalThis.messages.push(x);globalThis.api={state,applyGamingState,gamingPaused,pollGamingMode,refreshManagedAndGames,pollLibraries,pollPlaytime,checkForUpdateNotice,scanArtwork,visibleGames,libraryPrefs,saveGameProgress,progressBadge,activitySummary,loadStatistics,loadRAProgress,loadRAGame,saveRAConnection,metadataControls,gameMetadata};',ctx);ctx.messages=messages;
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
