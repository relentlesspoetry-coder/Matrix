// Production render/event functions with a small replacement/focus DOM model.
// This verifies ownership and semantic targeting, not browser layout.
const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const vm=require('node:vm');
const path=require('node:path');
function fixture() {
  const handlers={}, groups={screen:[],results:[],overlay:[]};
  const document={body:{tagName:'BODY'},activeElement:null,addEventListener(name,fn){(handlers[name]||=[]).push(fn);}};
  document.activeElement=document.body;
  function node(attrs={},group='screen') {
    return {id:attrs.id||'',dataset:Object.fromEntries(Object.entries(attrs).filter(([k])=>k.startsWith('data-')).map(([k,v])=>[k.slice(5).replace(/-([a-z])/g,(_,c)=>c.toUpperCase()),v])),disabled:false,isConnected:true,
      focus(){document.activeElement=this;for(const fn of handlers.focusin||[])fn({target:this});},setAttribute(){},
      get innerHTML(){return ''},set innerHTML(value){},group};
  }
  const roots={};
  function root(name) {
    const result={contains:n=>!!n && (groups[name].includes(n)||(name==='screen'&&groups.results.includes(n))),querySelectorAll:()=>name==='screen'?[...groups.screen,...groups.results]:groups[name]};
    Object.defineProperty(result,'firstElementChild',{get:()=>groups[name][0]||null});
    Object.defineProperty(result,'innerHTML',{set(html){
      for(const group of name==='screen'?['screen','results']:[name]) {
        for(const old of groups[group]){old.isConnected=false;if(document.activeElement===old)document.activeElement=document.body;}
        groups[group]=[];
      }
      for(const match of html.matchAll(/<(button|input|select|textarea)\b([^>]*)>/g)) {
        const attrs=Object.fromEntries([...match[2].matchAll(/([\w-]+)="([^"]*)"/g)].map(m=>[m[1],m[2].replace(/&quot;/g,'"').replace(/&amp;/g,'&')]));
        groups[name].push(node(attrs,name));
      }
    }});return result;
  }
  roots['#screen']=root('screen');roots['#game-results']=root('results');roots['#overlay-root']=root('overlay');
  document.querySelector=selector=>roots[selector]|| (selector.startsWith('#')?[...groups.screen,...groups.results].find(n=>n.id===selector.slice(1)):null)||null;
  const context=vm.createContext({document,window:{},localStorage:{setItem(){}},setTimeout(){},clearTimeout(){}});
  vm.runInContext(fs.readFileSync(path.join(__dirname,'../web/data.js'),'utf8'),context);
  let source=fs.readFileSync(path.join(__dirname,'../web/app.js'),'utf8');
  source=source.replace('readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();','');
  vm.runInContext(source+`;hydrateNativeCovers=()=>{};renderLaunchStatus=()=>{};toast=()=>{};gameCover=()=>'';
    globalThis.api={state,renderHome,renderConsole,renderGameResults,renderSettings,dismissOverlay,toggleFavorite,handleAction};`,context);
  const api=context.api;api.state.page='console';api.state.consoleId='ps2';api.state.imported.ps2=[{uid:'game-1',title:'First'},{uid:'game-2',title:'Second'}];
  api.renderConsole();
  const find=(action,value,uid)=>[...groups.screen,...groups.results].find(n=>n.dataset.action===action&&(value===undefined||n.dataset.value===value)&&(uid===undefined||n.dataset.uid===uid));
  return {...api,document,find,groups,modal(){const n=node({},'overlay');groups.overlay.push(n);n.focus();return n;}};
}
const tick=()=>new Promise(resolve=>setImmediate(resolve));
function homeFixture() {
 const f=fixture();f.state.page='home';f.state.consoleId=null;f.renderHome();return f;
}
for(const vendor of ['Sony','Nintendo','Sega','All']) test(`Home ${vendor} filter retains focus across repeated activation`,async()=>{
 const f=homeFixture();
 for(let i=0;i<2;i++) {
  const before=f.find('vendor',vendor);before.focus();await f.handleAction(before);
  assert.equal(f.state.vendor,vendor);assert.equal(before.isConnected,false);
  assert.equal(f.document.activeElement,f.find('vendor',vendor));
 }
});
test('Home refresh retains the focused console card',()=>{
 const f=homeFixture();
 for(let i=0;i<2;i++) {
  const before=f.groups.screen.find(n=>n.dataset.action==='console'&&n.dataset.id==='ps2');before.focus();
  f.state.imported.ps2.push({uid:'refresh-'+i,title:'New game'});f.renderHome();
  assert.equal(before.isConnected,false);
  assert.equal(f.document.activeElement,f.groups.screen.find(n=>n.dataset.action==='console'&&n.dataset.id==='ps2'));
 }
});
test('Home disappearing console falls back to the selected brand',()=>{
 const f=homeFixture();f.groups.screen.find(n=>n.dataset.action==='console'&&n.dataset.id==='ps2').focus();
 f.state.vendor='Nintendo';f.renderHome();
 assert.equal(f.document.activeElement,f.find('vendor','Nintendo'));
});
for(const removeOpener of [false,true]) test(`Home refresh ${removeOpener?'falls back from':'remaps'} modal opener without stealing focus`,()=>{
 const f=homeFixture(),opener=f.groups.screen.find(n=>n.dataset.action==='console'&&n.dataset.id==='ps2');opener.focus();
 f.state.lastFocus=opener;f.state.overlay='updates';const modal=f.modal();
 if(removeOpener) f.state.vendor='Nintendo';
 f.renderHome();
 const target=removeOpener?f.find('vendor','Nintendo'):f.groups.screen.find(n=>n.dataset.action==='console'&&n.dataset.id==='ps2');
 assert.equal(opener.isConnected,false);assert.equal(f.document.activeElement,modal);assert.equal(f.state.lastFocus,target);
 f.dismissOverlay();assert.equal(f.document.activeElement,target);
});
test('Home refresh leaves unrelated navigation focus untouched',()=>{
 const f=homeFixture(),navigation={isConnected:true};f.document.activeElement=navigation;
 f.renderHome();assert.equal(f.document.activeElement,navigation);
});
test('Home and console navigation do not transfer focus into the next page',()=>{
 const f=fixture();f.find('game',undefined,'game-1').focus();
 f.state.page='home';f.state.consoleId=null;f.renderHome();assert.equal(f.document.activeElement,f.document.body);
 f.find('vendor','Sony').focus();f.state.page='console';f.state.consoleId='ps2';f.renderConsole();
 assert.equal(f.document.activeElement,f.document.body);
});
test('Home does not restore coincidentally matching controls from an unrelated screen',()=>{
 const f=homeFixture();
 f.document.querySelector('#screen').innerHTML='<button data-action="vendor" data-value="Sony">Unrelated control</button>';
 f.find('vendor','Sony').focus();f.renderHome();assert.equal(f.document.activeElement,f.document.body);
});
test('details favorite rerender remaps opener without moving modal focus',async()=>{
 const f=fixture(),opener=f.find('game',undefined,'game-1');opener.focus();
 f.state.lastFocus=opener;f.state.overlay='game';f.state.selectedGame={id:'ps2',uid:'game-1'};
 const modal=f.modal();f.toggleFavorite('ps2','game-1',modal);await tick();
 assert.equal(opener.isConnected,false);assert.equal(f.document.activeElement,modal);
 assert.equal(f.state.lastFocus,f.find('game',undefined,'game-1'));
 f.dismissOverlay();assert.equal(f.document.activeElement,f.find('game',undefined,'game-1'));
});
test('removing last favorite recovers focus after disabled-button blur',async()=>{
 const f=fixture();f.state.favorites.add('ps2|game-1');f.state.gameFilter='Favorites';f.renderConsole();
 const heart=f.find('favorite',undefined,'game-1');heart.focus();f.toggleFavorite('ps2','game-1',heart);
 f.document.activeElement=f.document.body; // Browser blur after disabling the focused button.
 await tick();assert.equal(f.find('favorite'),undefined);
 assert.equal(f.document.activeElement,f.find('filter-games','Favorites'));
});
test('details return falls back when last filtered favorite disappears',async()=>{
 const f=fixture();f.state.favorites.add('ps2|game-1');f.state.gameFilter='Favorites';f.renderConsole();
 const opener=f.find('game',undefined,'game-1');opener.focus();f.state.lastFocus=opener;f.state.overlay='game';
 const modal=f.modal();f.toggleFavorite('ps2','game-1',modal);await tick();assert.equal(f.document.activeElement,modal);
 f.dismissOverlay();assert.equal(f.document.activeElement,f.find('filter-games','Favorites'));
});
for(const [action,value] of [['filter-games','Favorites'],['filter-games','All'],['view','list'],['view','grid']]) {
 test(`${action} ${value} retains semantic focus`,async()=>{
  const f=fixture(),before=f.find(action,value);before.focus();await f.handleAction(before);
  assert.equal(before.isConnected,false);assert.equal(f.document.activeElement,f.find(action,value));
 });
}
test('results-only replacement retains game focus while search focus is untouched',()=>{
 const f=fixture();f.find('game',undefined,'game-2').focus();f.state.sort='az';f.renderGameResults();
 assert.equal(f.document.activeElement,f.find('game',undefined,'game-2'));
 const search=f.groups.screen.find(n=>n.id==='game-search');search.focus();f.state.query='First';f.renderGameResults();
 assert.equal(f.document.activeElement,search);
});

test('settings save dismissal followed by rerender retains configure opener',()=>{
 const f=fixture();f.state.page='settings';f.state.consoleId=null;f.renderSettings();
 const opener=f.groups.screen.find(n=>n.dataset.action==='configure'&&n.dataset.id==='ps2');opener.focus();
 f.state.lastFocus=opener;f.state.overlay='configure';f.modal();
 f.dismissOverlay();f.renderSettings();
 assert.equal(f.document.activeElement,f.groups.screen.find(n=>n.dataset.action==='configure'&&n.dataset.id==='ps2'));
});
test('returning from an unrelated screen does not restore its control into settings',()=>{
 const f=fixture();f.state.page='settings';f.renderSettings();
 // Model Home replacing the screen outside these focused render functions.
 f.document.querySelector('#screen').innerHTML='<button data-action="configure" data-id="ps2">Home control</button>';
 f.find('configure').focus();f.renderSettings();assert.equal(f.document.activeElement,f.document.body);
});
test('settings refresh remaps configure opener without stealing modal focus',()=>{
 const f=fixture();f.state.page='settings';f.renderSettings();
 const opener=f.groups.screen.find(n=>n.dataset.action==='configure'&&n.dataset.id==='ps2');opener.focus();
 f.state.lastFocus=opener;f.state.overlay='configure';const modal=f.modal();f.renderSettings();
 assert.equal(f.document.activeElement,modal);f.dismissOverlay();
 assert.equal(f.document.activeElement,f.groups.screen.find(n=>n.dataset.action==='configure'&&n.dataset.id==='ps2'));
});
