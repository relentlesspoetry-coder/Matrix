const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const tick=()=>new Promise(resolve=>setImmediate(resolve));
function fixture(reply){
 const calls=[],controls=[{disabled:false},{disabled:false}],owner={querySelectorAll:()=>controls};
 const nodes={'.emulator-import-dialog':owner,'#import-console':{value:'gamecube'},'#import-path':{value:'/old/Dolphin.AppImage'},'#import-error':{textContent:''},'#import-status':{textContent:''},'#import-destination':{textContent:''},'#overlay-root':{innerHTML:''},'.app-shell':{inert:false}};
 const context=vm.createContext({consoles:[{id:'gamecube',name:'GameCube',games:[]},{id:'wii',name:'Wii',games:[]}],window:{__TAURI__:{core:{invoke:async(name,args)=>{calls.push([name,args]);if(name==='import_emulator')return reply();if(name==='set_library_preference')return {};throw Error('Inventory must not be awaited after import');}}}},document:{querySelector:s=>nodes[s]},localStorage:{getItem(){return null;},setItem(){}}});
 const source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
 vm.runInContext(source+';render=()=>{};toast=()=>{};state.overlay="emulator-import";state.configs={gamecube:{emulatorPath:"/old/Dolphin.AppImage"},wii:{emulatorPath:"/old/Dolphin.AppImage"}};globalThis.api={state,saveEmulatorImport,finishEmulatorSetup,dismissOverlay,libraryPrefs,emulatorSavedRequests};',context);
 return {...context.api,nodes,calls,controls};
}
const committed=()=>({path:'/managed/Dolphin.AppImage',moved:true,message:'Emulator added and configured.',profiles:{gamecube:{emulatorPath:'/managed/Dolphin.AppImage'},wii:{emulatorPath:'/managed/Dolphin.AppImage',launchArgs:['--user','/keep/Wii config','{game}']}}});
test('committed import immediately updates every shared profile without inventory dependency',async()=>{
 const f=fixture(committed);await f.saveEmulatorImport(f.controls[0]);assert.equal(f.calls.length,1);assert.equal(f.calls[0][0],'import_emulator');
 for(const id of ['gamecube','wii']){assert.equal(f.state.configs[id].emulatorPath,'/managed/Dolphin.AppImage');assert.equal(f.state.readyProfiles[id],true);assert.ok(f.emulatorSavedRequests.has(id));}
 assert.equal(f.nodes['#import-path'].value,'');assert.match(f.nodes['#import-status'].textContent,/configured/);assert.equal(f.nodes['#import-error'].textContent,'');assert.ok(f.controls.every(c=>!c.disabled));f.dismissOverlay();assert.equal(f.state.overlay,null);
});
test('failed import preserves selection and profiles and releases controls',async()=>{
 const f=fixture(()=>Promise.reject(Error('Destination already exists')));await f.saveEmulatorImport(f.controls[0]);assert.match(f.nodes['#import-error'].textContent,/already exists/);assert.equal(f.nodes['#import-path'].value,'/old/Dolphin.AppImage');assert.equal(f.state.configs.wii.emulatorPath,'/old/Dolphin.AppImage');assert.ok(f.controls.every(c=>!c.disabled));
});
test('repeated clicks cannot duplicate a move and Close unlocks after completion',async()=>{
 let resolve;const f=fixture(()=>new Promise(r=>resolve=r));const pending=f.saveEmulatorImport(f.controls[0]);await tick();await f.saveEmulatorImport(f.controls[1]);f.dismissOverlay();assert.equal(f.state.overlay,'emulator-import');assert.equal(f.calls.length,1);resolve(committed());await pending;f.dismissOverlay();assert.equal(f.state.overlay,null);
});
test('finishing onboarding preserves explicit show-all preference',async()=>{
 const f=fixture(committed);f.libraryPrefs().hideUnconfigured=false;await f.finishEmulatorSetup(f.controls[0]);assert.equal(f.libraryPrefs().hideUnconfigured,false);assert.deepEqual(f.calls.map(c=>c[1].section),['setupCompleted']);assert.equal(f.state.needsEmulatorSetup,false);
});
test('finishing onboarding defaults to hiding only when no visibility choice was saved',async()=>{
 const f=fixture(committed);await f.finishEmulatorSetup(f.controls[0]);assert.deepEqual(f.calls.map(c=>c[1].section),['hideUnconfigured','setupCompleted']);assert.equal(f.libraryPrefs().hideUnconfigured,true);
});
