const {test}=require('node:test');
const assert=require('node:assert/strict');
const {install}=require('../web/gamepad.js');
function pad(buttons=[]){return {index:0,id:'standard fixture',mapping:'standard',connected:true,axes:[0,0],buttons:Array.from({length:16},(_,i)=>({pressed:buttons.includes(i)}))};}
test('controller Back retains styling while ordinary keyboard and pointer input clear it',()=>{
 let currentPad=pad(),frame,time=0;
 const handlers={},classes=new Set();
 const active={tagName:'BUTTON',tabIndex:0,matches:()=>false,closest:()=>null,getClientRects:()=>[{}],getBoundingClientRect:()=>({left:0,top:0,width:30,height:30}),focus(){document.activeElement=this},scrollIntoView(){}};
 const document={activeElement:active,visibilityState:'visible',hasFocus:()=>true,
  documentElement:{classList:{add(name){classes.add(name)},remove(name){classes.delete(name)}}},
  querySelector:()=>null,querySelectorAll:()=>[active],
  addEventListener(name,fn){(handlers[name]||=[]).push(fn)},
  dispatchEvent(event){for(const fn of handlers[event.type]||[])fn(event)}};
 const window={document,navigator:{getGamepads:()=>[currentPad]},requestAnimationFrame(fn){frame=fn},getComputedStyle:()=>({visibility:'visible'}),addEventListener(){},KeyboardEvent:class{constructor(type,args){this.type=type;Object.assign(this,args)}},Event:class{constructor(type){this.type=type}}};
 install(window);
 const step=buttons=>{currentPad=pad(buttons);frame(time+=20)};
 step([]);step([5]);assert.ok(classes.has('gamepad-navigation'));
 step([]);step([1]);
 assert.ok(classes.has('gamepad-navigation'),'Synthetic Escape from gamepad must not switch focus styling to keyboard/pointer mode');
 document.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Tab'}));
 assert.equal(classes.has('gamepad-navigation'),false,'Ordinary keyboard input still clears controller styling');
 step([]);step([1]);
 assert.ok(classes.has('gamepad-navigation'),'Back as first controller input restores controller styling');
 document.dispatchEvent(new window.Event('pointerdown'));
 assert.equal(classes.has('gamepad-navigation'),false,'Pointer input still clears controller styling');
 step([]);step([1]);assert.ok(classes.has('gamepad-navigation'));
});
