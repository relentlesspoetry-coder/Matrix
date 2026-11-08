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
    globalThis.api={state,renderConsole,renderGameResults,renderSettings,renderExternal,dismissOverlay,toggleFavorite,handleAction};`,context);
  const api=context.api;api.state.page='console';api.state.consoleId='ps2';api.state.imported.ps2=[{uid:'game-1',title:'First'},{uid:'game-2',title:'Second'}];
  api.renderConsole();
  const find=(action,value,uid)=>[...groups.screen,...groups.results].find(n=>n.dataset.action===action&&(value===undefined||n.dataset.value===value)&&(uid===undefined||n.dataset.uid===uid));
  return {...api,document,find,groups,modal(){const n=node({},'overlay');groups.overlay.push(n);n.focus();return n;}};
}
const tick=()=>new Promise(resolve=>setImmediate(resolve));
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
for(const kind of ['steam','windows']) test(`${kind} refresh remaps details opener without stealing modal focus`,()=>{
 const f=fixture();f.state.page=kind;f.state.consoleId=null;f.state[kind]=[{uid:'external-1',title:'External'}];f.renderExternal(kind);
 const opener=f.find('external-game',undefined,'external-1');opener.focus();f.state.lastFocus=opener;f.state.overlay='external';const modal=f.modal();
 f.renderExternal(kind);assert.equal(f.document.activeElement,modal);
 f.dismissOverlay();assert.equal(f.document.activeElement,f.find('external-game',undefined,'external-1'));
});

for(const kind of ['steam','windows']) test(`${kind} missing details target returns to search`,()=>{
 const f=fixture();f.state.page=kind;f.state.consoleId=null;f.state[kind]=[{uid:'external-1',title:'External'}];f.renderExternal(kind);
 const opener=f.find('external-game',undefined,'external-1');opener.focus();f.state.lastFocus=opener;f.state.overlay='external';const modal=f.modal();
 f.state[kind]=[];f.renderExternal(kind);assert.equal(f.document.activeElement,modal);
 f.dismissOverlay();assert.equal(f.document.activeElement,f.groups.screen.find(n=>n.id==='external-search'));
});
test('switching external pages does not transfer focus to a coincidentally matching UID',()=>{
 const f=fixture();f.state.page='steam';f.state.steam=[{uid:'shared',title:'Steam'}];f.renderExternal('steam');f.find('external-game',undefined,'shared').focus();
 f.state.page='windows';f.state.windows=[{uid:'shared',title:'Windows'}];f.renderExternal('windows');
 assert.equal(f.document.activeElement,f.document.body);
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
test('unrelated navigation control keeps focus during same-page external refresh',()=>{
 const f=fixture();f.state.page='steam';f.renderExternal('steam');
 const navigation={isConnected:true};f.document.activeElement=navigation;f.renderExternal('steam');
 assert.equal(f.document.activeElement,navigation);
});
