const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(__dirname+'/../web/app.js','utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
function fixture(invoke=async()=>({})){
 const nodes={},calls=[],messages=[];
 const doc={documentElement:{dataset:{},style:{setProperty(){}}},querySelector:s=>nodes[s]||null,querySelectorAll:()=>[]};
 const ctx=vm.createContext({consoles:[{id:'gb',name:'Game Boy',abbrev:'GB',games:[]},{id:'ps1',name:'PlayStation 1',games:[]}],window:{__TAURI__:{core:{invoke:async(n,a)=>{calls.push([n,a]);return invoke(n,a);}}}},document:doc,localStorage:{setItem(){}},setTimeout,clearTimeout});
 vm.runInContext(source+';toast=x=>globalThis.messages.push(x);globalThis.api={state,formatPlaytime,playtimeMarkup,gameCard,pollPlaytime,libraryPrefs,displayControls,applyDisplayPreferences,changeDisplayPreference,previewNativeDrop,loadDiscSet};',ctx);ctx.messages=messages;
 return {...ctx.api,nodes,calls,messages,doc};
}
test('playtime formatting handles zero, short sessions and 50-hour totals',()=>{
 const f=fixture();assert.equal(f.formatPlaytime(0),'');assert.equal(f.formatPlaytime(1000),'Less than a minute played');assert.equal(f.formatPlaytime(60000),'1 min played');assert.equal(f.formatPlaytime(50*3600000),'50 h played');assert.equal(f.formatPlaytime(50*3600000+120000),'50 h 2 min played');assert.equal(f.formatPlaytime(NaN),'');
});
test('playtime appears below the title and remains scoped to the game and console',()=>{
 const f=fixture();f.state.playtime['gb|/Pokemon Red.gb']=50*3600000;
 const html=f.gameCard({id:'gb',name:'Game Boy',abbrev:'GB'},{uid:'/Pokemon Red.gb',title:'Pokémon Red'},0);
 assert.match(html,/Pokémon Red<span class="game-playtime"[^>]*>50 h played<\/span>/);
 assert.match(f.playtimeMarkup('ps1','/Pokemon Red.gb'),/hidden/);
});
test('clock refresh updates existing text without replacing the focused interface',async()=>{
 const f=fixture(async()=>({'gb|game':3600000}));const node={dataset:{playtimeKey:'gb|game'},textContent:'',hidden:true};f.doc.querySelectorAll=()=>[node];
 await f.pollPlaytime();assert.equal(node.textContent,'1 h played');assert.equal(node.hidden,false);assert.equal(f.calls[0][0],'get_playtime');
});
test('display settings preserve default typography and allow independent size and prompt choices',()=>{
 const f=fixture();f.applyDisplayPreferences();assert.equal(f.doc.documentElement.style.zoom,'1');assert.equal(f.doc.documentElement.dataset.coverSize,'medium');
 Object.assign(f.libraryPrefs(),{uiScale:130,coverSize:'large',controllerPrompts:'playstation'});f.applyDisplayPreferences();assert.equal(f.doc.documentElement.style.zoom,'1.3');assert.equal(f.doc.documentElement.dataset.coverSize,'large');assert.match(f.displayControls(),/value="130" selected/);assert.match(f.displayControls(),/value="playstation" selected/);
});
test('failed display save rolls back the visible setting and re-enables its control',async()=>{
 const f=fixture(async()=>{throw Error('disk full');});f.libraryPrefs().uiScale=100;const field={dataset:{displayPref:'uiScale'},value:'150',disabled:false};
 await f.changeDisplayPreference(field);assert.equal(f.libraryPrefs().uiScale,100);assert.equal(f.doc.documentElement.style.zoom,'1');assert.equal(field.disabled,false);assert.match(f.messages[0],/not saved/);
});
test('drop inspection never bypasses an existing dialog',async()=>{
 const f=fixture();f.state.overlay='configure';await f.previewNativeDrop(['/tmp/Emulator.AppImage']);assert.equal(f.calls.length,0);assert.match(f.messages[0],/Close the current dialog/);
});
test('drop response cannot replace a newer dialog',async()=>{
 let resolve;const f=fixture(()=>new Promise(r=>resolve=r));f.nodes['#overlay-root']={firstElementChild:null};const pending=f.previewNativeDrop(['/games']);f.state.overlay='configure';resolve([{kind:'folder',path:'/games'}]);await pending;assert.equal(f.state.overlay,'configure');assert.equal(f.calls.length,1);
});
test('unsupported files and untargeted covers never mutate the library',async()=>{
 const f=fixture(async()=>[{kind:'cover',path:'/tmp/cover.png'}]);f.nodes['#overlay-root']={firstElementChild:null};await f.previewNativeDrop(['/tmp/cover.png']);assert.match(f.messages[0],/onto a game/);assert.equal(f.calls.length,1);assert.equal(f.state.overlay,null);
});
const {promptLabels}=require('../web/gamepad.js');
test('controller prompt overrides and automatic DualSense detection do not change input mapping',()=>{
 const pad={id:'Sony DualSense Wireless Controller',mapping:'standard',connected:true};assert.match(promptLabels('auto',[pad]),/×: select.*L1/);assert.match(promptLabels('xbox',[pad]),/A: select.*LB/);assert.match(promptLabels('playstation',[]),/○: back/);assert.match(promptLabels('auto',[]),/A: select/);
});
