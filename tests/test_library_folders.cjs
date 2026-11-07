const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const vm=require('node:vm');
const {test}=require('node:test');
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
function fixture() {
  let owner=null,html='',refreshes=0;
  const calls=[],messages=[],responders={},document={activeElement:null,querySelector(selector){
    if(selector==='#overlay-root')return root;
    if(selector==='#library-folder-dialog')return owner;
    return null;
  },querySelectorAll(){return [];}};
  function node() {return {value:'',disabled:false,textContent:'',innerHTML:'',dataset:{},focus(){document.activeElement=this;}};}
  const root={};Object.defineProperty(root,'innerHTML',{get:()=>html,set(value){html=value;
    const input=node(),list=node(),error=node(),select=node();select.value='native';
    owner={dataset:{id:value.match(/data-id="([^"]+)"/)[1]},input,list,error,select,
      querySelector(selector){return {'input':input,'.library-folder-list':list,'.library-folder-error':error,'select':select}[selector];}};
  }});
  const context=vm.createContext({consoles:['ps1','ps2','gamecube','wii'].map(id=>({id,name:id,games:[]})),document,
    window:{__TAURI__:{core:{invoke(command,args){calls.push({command,args});return responders[command]?responders[command](args):Promise.resolve([]);}}}},
    onRefresh(){refreshes++;},onToast(message){messages.push(message);}});
  const source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8');
  const startup='readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();';
  assert.ok(source.includes(startup));
  vm.runInContext(source.replace(startup,'')+`\nrefreshNative=async()=>onRefresh();toast=onToast;
    globalThis.api={EXTS,state,openLibraryFolders,browseLibraryFolder,changeLibraryFolder,showLibraryFolderRows,scanArtwork,coverMessage};`,context);
  return {...context.api,calls,messages,responders,document,owner:()=>owner,html:()=>html,refreshes:()=>refreshes,
    button(action,folderPath){return {dataset:{action,path:folderPath},disabled:false};}};
}
for(const outcome of ['resolve','reject']) test(`old folder list ${outcome} cannot change replacement dialog`,async()=>{
  const f=fixture(),pending=deferred();f.responders.list_library_folders=()=>pending.promise;
  const first=f.openLibraryFolders('ps2');delete f.responders.list_library_folders;
  await f.openLibraryFolders('wii');const owner=f.owner(),rows=owner.list.innerHTML;
  pending[outcome](outcome==='resolve'?[{path:'/old'}]:Error('Old list failure'));await first;
  assert.equal(f.owner(),owner);assert.equal(owner.list.innerHTML,rows);assert.equal(owner.error.textContent,'');
});
test('folder modal uses the existing styled dialog class',async()=>{
  const f=fixture();await f.openLibraryFolders('ps2');assert.ok(f.html().match(/class="([^"]*)" id="library-folder-dialog"/)[1].split(/\s+/).includes('dialog'));
});
test('folder row content and attribute paths are escaped, managed folders cannot be removed',async()=>{
  const f=fixture();await f.openLibraryFolders('ps2');
  f.showLibraryFolderRows(f.owner(),[{path:'/games/"<img>&',managed:false,available:false},{path:'/managed',managed:true,available:true}]);
  const rows=f.owner().list.innerHTML;assert.ok(!rows.includes('<img>'));assert.match(rows,/&quot;&lt;img&gt;&amp;/);
  assert.equal((rows.match(/data-action="remove-library-folder"/g)||[]).length,1);assert.match(rows,/Unavailable or disconnected/);
});
for(const newer of ['dialog','draft']) test(`folder browse preserves newer ${newer}`,async()=>{
  const f=fixture(),pending=deferred();await f.openLibraryFolders('ps2');f.responders.browse_library_folder=()=>pending.promise;
  const button=f.button('browse-library-folder'),request=f.browseLibraryFolder(button);
  if(newer==='dialog')await f.openLibraryFolders('wii');
  f.owner().input.value='/new-draft';pending.resolve('/old-choice');await request;
  assert.equal(f.owner().input.value,'/new-draft');assert.equal(button.disabled,false);
});
for(const newer of ['dialog','draft']) test(`folder addition preserves newer ${newer}`,async()=>{
  const f=fixture(),pending=deferred();await f.openLibraryFolders('ps2');f.owner().input.value='/added';
  f.responders.add_library_folder=()=>pending.promise;
  const button=f.button('add-library-folder'),request=f.changeLibraryFolder(button);
  if(newer==='dialog')await f.openLibraryFolders('wii');
  f.owner().input.value='/next-draft';const rows=f.owner().list.innerHTML;pending.resolve([{path:'/added'}]);await request;
  assert.equal(f.owner().input.value,'/next-draft');if(newer==='dialog')assert.equal(f.owner().list.innerHTML,rows);
  assert.equal(button.disabled,false);assert.equal(f.refreshes(),1);
});
test('initial list cannot roll back completed folder mutation',async()=>{
  const f=fixture(),pending=deferred();f.responders.list_library_folders=()=>pending.promise;
  const opened=f.openLibraryFolders('ps2');f.owner().input.value='/added';
  f.responders.add_library_folder=()=>Promise.resolve([{path:'/added'}]);
  await f.changeLibraryFolder(f.button('add-library-folder'));const rows=f.owner().list.innerHTML;
  pending.resolve([{path:'/old-list'}]);await opened;assert.equal(f.owner().list.innerHTML,rows);
});
test('duplicate mutations are blocked and current errors stay in their dialog',async()=>{
  const f=fixture(),pending=deferred();await f.openLibraryFolders('ps2');f.owner().input.value='/added';
  f.responders.add_library_folder=()=>pending.promise;const button=f.button('add-library-folder');
  const request=f.changeLibraryFolder(button);await f.changeLibraryFolder(button);
  pending.reject(Error('Unavailable'));await request;
  assert.equal(f.calls.filter(call=>call.command==='add_library_folder').length,1);assert.match(f.owner().error.textContent,/Unavailable/);assert.equal(button.disabled,false);
});
test('old mutation failure does not write in replacement dialog',async()=>{
  const f=fixture(),pending=deferred();await f.openLibraryFolders('ps2');f.owner().input.value='/added';f.responders.add_library_folder=()=>pending.promise;
  const request=f.changeLibraryFolder(f.button('add-library-folder'));await f.openLibraryFolders('wii');pending.reject(Error('Old failure'));await request;
  assert.equal(f.owner().error.textContent,'');assert.match(f.messages.at(-1),/Old failure/);
});
for(const launcher of ['native','flatpak','snap']) test(`custom Steam folder records ${launcher} routing and removal omits launcher`,async()=>{
  const f=fixture();await f.openLibraryFolders('steam');f.owner().input.value=' /steamapps ';f.owner().select.value=launcher;
  await f.changeLibraryFolder(f.button('add-library-folder'));await f.changeLibraryFolder(f.button('remove-library-folder','/steamapps'));
  const calls=f.calls.filter(call=>/^(add|remove)_steam_library$/.test(call.command));
  assert.deepEqual(JSON.parse(JSON.stringify(calls)),[{command:'add_steam_library',args:{path:'/steamapps',launcher}},{command:'remove_steam_library',args:{path:'/steamapps'}}]);
});
test('automatic covers include GameCube and Wii with correct provider messages',async()=>{
  const f=fixture();for(const kind of ['ps1','ps2','gamecube','wii'])f.state.imported[kind]=[{uid:`${kind}.iso`}];
  f.responders.download_cover=()=>Promise.resolve({status:'not-found',message:'Exact lookup',url:'https://example.test/cover'});
  await f.scanArtwork();assert.deepEqual(f.calls.filter(call=>call.command==='download_cover').map(call=>call.args.kind),['ps1','ps2','gamecube','wii']);
  for(const kind of ['gamecube','wii'])assert.match(f.coverMessage(kind,`${kind}.iso`),/GameTDB/);
  assert.match(f.coverMessage('ps2','ps2.iso'),/xlenore/);
});
test('disabled automatic cover preference sends no provider requests',async()=>{
  const f=fixture();f.state.artwork.enabled=false;f.state.imported.wii=[{uid:'test.iso'}];await f.scanArtwork();assert.equal(f.calls.length,0);
});
test('initial list failure cannot overwrite a completed mutation',async()=>{
  const f=fixture(),pending=deferred();f.responders.list_library_folders=()=>pending.promise;
  const opened=f.openLibraryFolders('ps2');f.owner().input.value='/added';
  f.responders.add_library_folder=()=>Promise.resolve([{path:'/added'}]);await f.changeLibraryFolder(f.button('add-library-folder'));
  pending.reject(Error('Stale list error'));await opened;assert.equal(f.owner().error.textContent,'');
});
test('browse cancellation and stale errors preserve path and replacement error text',async()=>{
  const f=fixture();await f.openLibraryFolders('ps2');f.owner().input.value='/draft';
  f.responders.browse_library_folder=()=>Promise.resolve(null);await f.browseLibraryFolder(f.button('browse-library-folder'));
  assert.equal(f.owner().input.value,'/draft');
  const pending=deferred();f.responders.browse_library_folder=()=>pending.promise;
  const request=f.browseLibraryFolder(f.button('browse-library-folder'));await f.openLibraryFolders('wii');
  pending.reject(Error('Old picker failure'));await request;assert.equal(f.owner().error.textContent,'');
});
test('cover scan cancellation does not send remaining new-console provider requests',async()=>{
  const f=fixture(),pending=deferred();f.state.imported.gamecube=[{uid:'gc.iso'}];f.state.imported.wii=[{uid:'wii.iso'}];
  f.responders.download_cover=()=>pending.promise;const request=f.scanArtwork();f.state.artwork.enabled=false;++f.state.coverGeneration;
  pending.resolve({status:'not-found'});await request;assert.equal(f.calls.length,1);assert.equal(f.state.coverStatus.size,0);
});
for(const moved of [false,true]) test(`removal restores focus only when Remove still owns it (moved=${moved})`,async()=>{
  const f=fixture(),pending=deferred();await f.openLibraryFolders('ps2');f.responders.remove_library_folder=()=>pending.promise;
  const button=f.button('remove-library-folder','/old');f.document.activeElement=button;
  const request=f.changeLibraryFolder(button);const other={};if(moved)f.document.activeElement=other;
  pending.resolve([]);await request;assert.equal(f.document.activeElement,moved?other:f.owner().input);
});
test('failed mutation reloads rows while stale initial list cannot overwrite recovered list or error',async()=>{
  const f=fixture(),initial=deferred();let reads=0;
  f.responders.list_library_folders=()=>++reads===1?initial.promise:Promise.resolve([{path:'/recovered'}]);
  const opened=f.openLibraryFolders('ps2');f.owner().input.value='/draft';
  f.responders.add_library_folder=()=>Promise.reject(Error('Mutation failed'));
  await f.changeLibraryFolder(f.button('add-library-folder'));
  assert.match(f.owner().list.innerHTML,/recovered/);assert.equal(f.owner().input.value,'/draft');assert.match(f.owner().error.textContent,/Mutation failed/);
  initial.resolve([{path:'/stale'}]);await opened;assert.match(f.owner().list.innerHTML,/recovered/);
});
test('failed recovery replaces loading placeholder without discarding mutation error',async()=>{
  const f=fixture();await f.openLibraryFolders('ps2');f.owner().input.value='/draft';f.owner().list.innerHTML='Loading folders…';
  f.responders.add_library_folder=()=>Promise.reject(Error('Mutation failed'));f.responders.list_library_folders=()=>Promise.reject(Error('List failed'));
  await f.changeLibraryFolder(f.button('add-library-folder'));
  assert.match(f.owner().list.innerHTML,/unavailable/);assert.match(f.owner().error.textContent,/Mutation failed/);assert.equal(f.owner().input.value,'/draft');
});
test('recovery response after closing cannot replace newer dialog rows',async()=>{
  const f=fixture(),pending=deferred();await f.openLibraryFolders('ps2');f.owner().input.value='/draft';
  f.responders.add_library_folder=()=>Promise.reject(Error('Mutation failed'));f.responders.list_library_folders=()=>pending.promise;
  const request=f.changeLibraryFolder(f.button('add-library-folder'));await new Promise(resolve=>setImmediate(resolve));
  delete f.responders.list_library_folders;await f.openLibraryFolders('wii');const rows=f.owner().list.innerHTML;
  pending.resolve([{path:'/old'}]);await request;assert.equal(f.owner().list.innerHTML,rows);assert.equal(f.owner().error.textContent,'');
});

test('preview format filters include only the verified new compressed formats',()=>{
  const {EXTS}=fixture();
  for(const [id,exts] of Object.entries({psp:['chd'],gamecube:['wia','ciso'],wii:['gcz','ciso']}))
    for(const ext of exts)assert.ok(EXTS[id].includes(ext),`${id}: ${ext}`);
  assert.ok(!EXTS.psp.includes('zso'));assert.ok(!EXTS.gamecube.includes('chd'));assert.ok(!EXTS.wiiu.includes('gcz'));
});
