const test=require('node:test');const assert=require('node:assert/strict');const fs=require('node:fs');const path=require('node:path');const vm=require('node:vm');
function fixture(invoke){
 const nodes={'.steamgrid-dialog':{_selection:{id:'ps2',uid:'/mock/Game.iso'},dataset:{}},'#steamgrid-query':{value:'Game'},'#steamgrid-message':{textContent:''},'#steamgrid-results':{innerHTML:''}};
 const calls=[];
 const context=vm.createContext({consoles:[{id:'ps2'}],document:{querySelector:s=>nodes[s]},window:{__TAURI__:{core:{invoke:(name,args)=>{calls.push([name,args]);return invoke(name,args);}}}}});
 const source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8').replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
 vm.runInContext(source+';render=()=>{};dismissOverlay=()=>{};clearCoverCache=()=>{};toast=()=>{};globalThis.api={state,steamGridAction};',context);
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
