const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(__dirname+'/../web/app.js','utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
const tick=()=>new Promise(r=>setImmediate(r));
function fixture(invoke=async()=>({phase:'current'})){
 let now=0,next=0;const calls=[],timers=new Map(),events={},nodes={};
 for(const name of ['#screen','#update-notice-root','#main-update-button','.update-dialog .close'])nodes[name]={innerHTML:'',hidden:true,dataset:{},classList:{toggle(){}},focus(){}};
 const doc={hidden:false,activeElement:{id:'draft'},querySelector:s=>nodes[s]||null,addEventListener:(n,f)=>events[n]=f};
 const ctx=vm.createContext({consoles:[],Date:class extends Date{static now(){return now;}},window:{__TAURI__:{core:{invoke:async(n,a)=>{calls.push([n,a]);return invoke(n,a);}}},addEventListener:(n,f)=>events[n]=f},document:doc,setTimeout:(f,ms)=>{const id=++next;timers.set(id,{f,ms});return id;},clearTimeout:id=>timers.delete(id),setInterval:()=>++next,clearInterval(){},localStorage:{setItem(){}}});
 vm.runInContext(source+';captureLibraryFocus=()=>({});restoreLibraryFocus=()=>{};renderUpdater=()=>{};globalThis.api={state,startUpdateChecks,checkForUpdateNotice,renderUpdateNotice,syncUpdateNotice,openUpdater,runUpdate,renderHome,renderSettings,handleAction,pending:()=>backgroundUpdateTask};',ctx);
 return {...ctx.api,nodes,calls,timers,events,doc,advance:ms=>{now+=ms;}};
}
const available={phase:'available',currentVersion:'0.6.17',latestVersion:'0.6.18',notes:'Verified release'};
test('main console menu exposes updates beside Add emulator; Settings no longer hides the button',()=>{
 const f=fixture();f.renderHome();assert.match(f.nodes['#screen'].innerHTML,/id="main-update-button"[^>]*data-action="updates"/);assert.match(f.nodes['#screen'].innerHTML,/Add emulator/);f.renderSettings();assert.doesNotMatch(f.nodes['#screen'].innerHTML,/data-action="updates"/);
});
test('checks at startup and exactly one hour later with a single scheduler',async()=>{
 const f=fixture();f.startUpdateChecks();f.startUpdateChecks();await tick();assert.equal(f.calls.length,1);assert.equal(f.calls[0][0],'check_updates_background');assert.equal(f.timers.size,1);const timer=[...f.timers.values()][0];assert.equal(timer.ms,3600000);
 f.advance(3599999);await f.checkForUpdateNotice();assert.equal(f.calls.length,1);f.advance(1);timer.f();await tick();assert.equal(f.calls.length,2);assert.equal(f.timers.size,1);
});
test('wake after suspend checks once and schedules a fresh hour from that request',async()=>{
 const f=fixture();f.startUpdateChecks();await tick();f.advance(600000);f.events.focus();await tick();assert.equal(f.calls.length,1);assert.equal([...f.timers.values()][0].ms,3000000);
 f.advance(7200000);f.events.focus();f.events.visibilitychange();await tick();assert.equal(f.calls.length,2);assert.equal([...f.timers.values()][0].ms,3600000);
});
test('available update creates a visible prompt without replacing a dialog, focus or starting a download',async()=>{
 const f=fixture(async()=>available);f.state.overlay='configure';const focus=f.doc.activeElement;await f.checkForUpdateNotice();assert.equal(f.nodes['#update-notice-root'].hidden,false);assert.match(f.nodes['#update-notice-root'].innerHTML,/Review update/);assert.equal(f.nodes['#main-update-button'].textContent,'Update available');assert.equal(f.state.overlay,'configure');assert.equal(f.doc.activeElement,focus);assert.deepEqual(f.calls.map(c=>c[0]),['check_updates_background']);
});
test('Later snoozes the same release while a newer version prompts immediately',async()=>{
 const f=fixture(async()=>available);await f.checkForUpdateNotice();await f.handleAction({dataset:{action:'later-update'}});assert.equal(f.nodes['#update-notice-root'].hidden,true);f.syncUpdateNotice(available);assert.equal(f.nodes['#update-notice-root'].hidden,true);f.syncUpdateNotice({...available,latestVersion:'0.6.19'});assert.equal(f.nodes['#update-notice-root'].hidden,false);
});
test('busy updater defers an overdue check without losing the hourly deadline',async()=>{
 const f=fixture();f.state.updateBusy=true;f.startUpdateChecks();await tick();assert.equal(f.calls.length,0);assert.equal([...f.timers.values()][0].ms,60000);f.state.updateBusy=false;[...f.timers.values()][0].f();await tick();assert.equal(f.calls.length,1);
});
test('network failure is quiet and does not produce repeated requests on focus',async()=>{
 const f=fixture(async()=>{throw Error('offline');});f.startUpdateChecks();await tick();assert.equal(f.pending(),null);assert.equal(f.state.update,undefined);assert.equal(f.nodes['#update-notice-root'].hidden,true);f.events.focus();await tick();assert.equal(f.calls.length,1);f.advance(3600000);f.events.focus();await tick();assert.equal(f.calls.length,2);
});
test('opening the updater waits for an in-flight hourly check and keeps its verified result',async()=>{
 let resolve;const f=fixture((name)=>name==='check_updates_background'?new Promise(r=>resolve=r):available);
 const checking=f.checkForUpdateNotice();await tick();const opening=f.openUpdater();assert.equal(f.state.overlay,'updates');assert.equal(f.calls.length,1);resolve(available);await checking;await opening;assert.equal(f.state.update.phase,'available');assert.deepEqual(f.calls.map(c=>c[0]),['check_updates_background','update_status']);
});
test('manual download waits for background verification and starts only after user action',async()=>{
 let resolve;const f=fixture(name=>name==='check_updates_background'?new Promise(r=>resolve=r):{phase:'downloaded'});
 const checking=f.checkForUpdateNotice();await tick();const downloading=f.runUpdate('download_update');assert.equal(f.calls.length,1);resolve(available);await checking;await downloading;assert.deepEqual(f.calls.map(c=>c[0]),['check_updates_background','download_update']);assert.equal(f.state.update.phase,'downloaded');assert.equal(f.nodes['#update-notice-root'].hidden,true);
});

test('the next hour starts after verification completes, avoiding the backend cache boundary',async()=>{
 let resolve;const f=fixture(()=>new Promise(r=>resolve=r));f.startUpdateChecks();await tick();f.advance(20000);resolve({phase:'current'});await tick();assert.equal([...f.timers.values()][0].ms,3600000);
});
