const {test}=require('node:test');
const assert=require('node:assert/strict');
const {commands,createInput,nearest,install}=require('../web/gamepad.js');
function pad(buttons=[],axes=[0,0],id=0) {return {index:id,id:`pad${id}`,mapping:'standard',connected:true,axes,buttons:Array.from({length:16},(_,index)=>({pressed:buttons.includes(index)}))};}
test('standard controller mapping and stick deadzone',()=>{
  assert.equal(commands(pad([],[.54,-.54])).right,false);
  assert.equal(commands(pad([],[.56,-.56])).right,true);
  assert.equal(commands(pad([],[.56,-.56])).up,true);
  assert.equal(commands(pad([0,1,4,5,12,13,14,15])).activate,true);
});
test('connect with held launch does not activate; holding A never repeats',()=>{
  const actions=[],poll=createInput(action=>actions.push(action));
  poll([pad([0])],0);poll([pad([0])],500);poll([pad()],600);poll([pad([0])],700);poll([pad([0])],2000);
  assert.deepEqual(actions,['activate']);
});
test('navigation repeats after delay at bounded cadence',()=>{
  const actions=[],poll=createInput(action=>actions.push(action));
  poll([pad()],0);poll([pad([13])],10);poll([pad([13])],409);poll([pad([13])],410);poll([pad([13])],549);poll([pad([13])],550);
  assert.deepEqual(actions,['down','down','down']);
});
test('background and reconnect require release before activation',()=>{
  const actions=[],poll=createInput(action=>actions.push(action));
  poll([pad()],0);poll([pad([0])],1,false);poll([pad([0])],2);poll([],3);poll([pad([0])],4);poll([pad([0])],500);
  assert.deepEqual(actions,[]);
});
test('nonstandard pads ignored and cancel takes priority over launch',()=>{
  const actions=[],poll=createInput(action=>actions.push(action));
  poll([{...pad(),mapping:''}],0);poll([{...pad([0]),mapping:''}],1);
  poll([pad()],2);poll([pad([0,1])],3);poll([pad([0])],4);
  assert.deepEqual(actions,['back']);
});
function node(x,y) {return {getBoundingClientRect(){return {left:x,top:y,width:30,height:30};}};}
test('spatial movement chooses aligned neighbor and never wraps across modal boundary',()=>{
  const current=node(0,0),right=node(50,0),diagonal=node(30,100),below=node(0,70);
  assert.equal(nearest(current,[current,right,diagonal,below],'right'),right);
  assert.equal(nearest(current,[current,right,diagonal,below],'down'),below);
  assert.equal(nearest(current,[current,right,below],'left'),null);
});
function domFixture() {
  let currentPad=pad(),frame,dialog=null;const clicked=[],events=[],all=[],local=[];
  const document={activeElement:null,visibilityState:'visible',hasFocus:()=>true,
    documentElement:{classList:{add(){},remove(){}}},addEventListener(){},dispatchEvent(event){events.push(event.key);},
    querySelector(){return dialog;},querySelectorAll(){return all;}};
  const window={document,navigator:{getGamepads:()=>[currentPad]},requestAnimationFrame(callback){frame=callback;},
    getComputedStyle:()=>({visibility:'visible'}),addEventListener(){},KeyboardEvent:class {constructor(name,args){Object.assign(this,args);}},Event:class {constructor(name){this.type=name;}}};
  function add(name,x,y,modal=false,props={}) {
    const control={...node(x,y),name,tagName:'BUTTON',tabIndex:0,matches(){return !!this.disabled;},closest(){return null;},getClientRects:()=>[{}],
      focus(){document.activeElement=this;},scrollIntoView(){},click(){clicked.push(name);},...props};
    (modal?local:all).push(control);return control;
  }
  install(window);
  let time=0;
  function step(buttons=[]) {currentPad=pad(buttons);frame(time+=20);}
  function press(button) {step();step([button]);}
  return {document,add,step,press,clicked,events,open(){dialog={querySelectorAll:()=>local};}};
}
test('controller navigation and activation stay inside live modal',()=>{
  const f=domFixture(),outside=f.add('outside',0,0),close=f.add('close',0,0,true),save=f.add('save',50,0,true);
  outside.focus();f.open();f.step();f.press(0);
  assert.equal(f.document.activeElement,close);assert.deepEqual(f.clicked,[]);
  f.press(15);assert.equal(f.document.activeElement,save);f.press(0);assert.deepEqual(f.clicked,['save']);
  f.press(5);assert.equal(f.document.activeElement,close);f.press(1);assert.deepEqual(f.events,['Escape']);
});
test('select navigation changes enabled choices, and bumpers leave the select',()=>{
  const f=domFixture(),choices=[{selected:true},{disabled:true},{selected:false}],changes=[];
  const select=f.add('choice',0,0,false,{tagName:'SELECT',options:choices,selectedOptions:[choices[0]],dispatchEvent(event){changes.push(event.type);}});
  const next=f.add('next',50,0);select.focus();f.step();f.press(13);
  assert.equal(choices[2].selected,true);assert.deepEqual(changes,['change']);
  f.press(5);assert.equal(f.document.activeElement,next);
});
test('disabled controls are skipped in sequential controller navigation',()=>{
  const f=domFixture(),first=f.add('first',0,0);f.add('disabled',20,0,false,{disabled:true});const last=f.add('last',40,0);
  first.focus();f.step();f.press(5);assert.equal(f.document.activeElement,last);
});

test('newly connected or reset held navigation waits for release before repeat',()=>{
  const actions=[],poll=createInput(action=>actions.push(action));
  poll([pad([13])],0);poll([pad([13])],1000);
  assert.deepEqual(actions,[]);
  poll([pad()],1100);poll([pad([13])],1200);poll([pad([13])],1600);
  assert.deepEqual(actions,['down','down']);
  poll.reset();poll([pad([0,1,5,13])],2000);poll([pad([0,1,5,13])],5000);
  assert.deepEqual(actions,['down','down']);
  poll([pad()],5100);poll([pad([0])],5200);
  assert.deepEqual(actions,['down','down','activate']);
});
for (const transition of ['visibility','focus']) test(`${transition} resume ignores held activation when background frames never ran`,()=>{
  let currentPad=pad(),queued=[],clicks=0,focused=true;
  const handlers={},windowHandlers={};
  const active={...node(0,0),tagName:'BUTTON',tabIndex:0,matches:()=>false,closest:()=>null,getClientRects:()=>[{}],focus(){doc.activeElement=this;},scrollIntoView(){},click(){clicks++;}};
  const doc={activeElement:active,visibilityState:'visible',hasFocus:()=>focused,
    documentElement:{classList:{add(){},remove(){}}},querySelector:()=>null,querySelectorAll:()=>[active],
    addEventListener(name,fn){(handlers[name]||=[]).push(fn);}};
  const win={document:doc,navigator:{getGamepads:()=>[currentPad]},requestAnimationFrame(fn){queued.push(fn);},
    getComputedStyle:()=>({visibility:'visible'}),addEventListener(name,fn){(windowHandlers[name]||=[]).push(fn);}};
  const fire=(target,name)=>{for(const fn of target[name]||[])fn();};
  const frame=time=>{assert.equal(queued.length,1,'Only one animation loop may be queued');queued.shift()(time);};
  install(win);frame(0);
  if(transition==='visibility'){doc.visibilityState='hidden';fire(handlers,'visibilitychange');}
  else{focused=false;fire(windowHandlers,'blur');}
  currentPad=pad([0]); // No requestAnimationFrame callbacks run while away.
  if(transition==='visibility'){doc.visibilityState='visible';fire(handlers,'visibilitychange');}
  else{focused=true;fire(windowHandlers,'focus');}
  frame(1000);frame(2000);assert.equal(clicks,0);
  currentPad=pad();frame(2100);currentPad=pad([0]);frame(2200);
  assert.equal(clicks,1,'A fresh press after release still activates');
});
