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
