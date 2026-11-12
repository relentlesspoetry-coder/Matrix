const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const vm=require('node:vm');
function fixture(){
 const nodes={'#game-results':{innerHTML:''},'#game-preview':{innerHTML:''}};
 const source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
 const context=vm.createContext({consoles:[{id:'switch2',name:'Nintendo Switch 2',abbrev:'Switch 2',games:[]}],window:{},document:{querySelector:s=>nodes[s],querySelectorAll:()=>[]}});
 vm.runInContext(source+';captureLibraryFocus=()=>({});restoreLibraryFocus=()=>{};hydrateNativeCovers=()=>{};globalThis.api={state,renderGameResults,previewLibraryGame};',context);
 const api=context.api;api.state.native=true;api.state.page='console';api.state.consoleId='switch2';api.state.imported.switch2=[{uid:'one',title:'First game'},{uid:'two',title:'Second <game>'}];
 return {...api,nodes};
}
test('console rows select a safely escaped preview without opening or launching a game',()=>{
 const f=fixture();f.renderGameResults();
 assert.match(f.nodes['#game-results'].innerHTML,/class="game-list"/);
 assert.match(f.nodes['#game-preview'].innerHTML,/<h2>First game<\/h2>/);
 f.previewLibraryGame('switch2','two');
 assert.match(f.nodes['#game-preview'].innerHTML,/<h2>Second &lt;game&gt;<\/h2>/);
 assert.equal(f.state.previewGame.uid,'two');assert.equal(f.state.overlay,null);
 f.previewLibraryGame('ps2','one');assert.equal(f.state.previewGame.uid,'two');
});
test('preview survives sorting and clears or falls back when filters remove its game',()=>{
 const f=fixture();f.renderGameResults();f.previewLibraryGame('switch2','two');
 f.state.sort='az';f.renderGameResults();assert.equal(f.state.previewGame.uid,'two');
 f.state.query='First';f.renderGameResults();assert.equal(f.state.previewGame.uid,'one');
 f.state.query='No match';f.renderGameResults();assert.equal(f.state.previewGame,null);
 assert.match(f.nodes['#game-preview'].innerHTML,/Select a game/);
 assert.doesNotMatch(f.nodes['#game-preview'].innerHTML,/data-art-uid/);
});
