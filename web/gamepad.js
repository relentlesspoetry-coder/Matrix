/* Standard-mapped controllers: D-pad/left stick move, A selects, B goes back,
 * bumpers move sequentially. Text entry and operating-system pickers still use
 * their normal keyboard/mouse controls. No controller input reaches emulators. */
(function (root) {
  'use strict';
  const selector='button,input,select,textarea,summary,a[href],[tabindex]';
  function promptLabels(style,pads=[]) {
    const playstation=style==='playstation'||style!=='xbox'&&pads.some(p=>p?.connected!==false&&p?.mapping==='standard'&&/sony|playstation|dualsense|dualshock|054c|wireless controller/i.test(p.id||''));
    return playstation?'D-pad: move · ×: select · ○: back · L1 / R1: next control':'D-pad: move · A: select · B: back · LB / RB: next control';
  }
  function commands(pad) {
    const down=index=>!!pad.buttons?.[index]?.pressed;
    const x=pad.axes?.[0] || 0, y=pad.axes?.[1] || 0;
    return {left:down(14)||x < -.55,right:down(15)||x > .55,
      up:down(12)||y < -.55,down:down(13)||y > .55,
      activate:down(0),back:down(1),previous:down(4),next:down(5)};
  }
  function createInput(dispatch) {
    const pads=new Map();
    function poll(devices,time,enabled=true) {
      const seen=new Set();
      for (const pad of devices || []) {
        if (!pad || pad.connected===false || pad.mapping!=='standard') continue;
        const key=`${pad.index}:${pad.id}`;seen.add(key);
        const current=commands(pad), previous=pads.get(key);
        // A newly connected pad must release any held buttons before acting.
        if (!previous || !enabled) {
          pads.set(key,{buttons:current,next:time+400,blocked:new Set(Object.keys(current).filter(action=>current[action]))});
          continue;
        }
        for (const action of previous.blocked) if (!current[action]) previous.blocked.delete(action);
        let fired=false;
        for (const action of ['back','activate','previous','next','up','down','left','right']) {
          if (!current[action] || previous.blocked.has(action)) continue;
          const repeat=['up','down','left','right','previous','next'].includes(action);
          if (!previous.buttons[action] || (repeat && time>=previous.next)) {
            dispatch(action);previous.next=time+(previous.buttons[action]?140:400);fired=true;break;
          }
        }
        previous.buttons=current;
        if (!fired && !Object.values(current).some(Boolean)) previous.next=time+400;
      }
      for (const key of pads.keys()) if (!seen.has(key)) pads.delete(key);
    }
    poll.reset=()=>pads.clear();
    return poll;
  }
  function nearest(current,candidates,direction) {
    const origin=current.getBoundingClientRect(), ox=origin.left+origin.width/2, oy=origin.top+origin.height/2;
    let best=null,score=Infinity;
    for (const node of candidates) {
      if (node===current) continue;
      const rect=node.getBoundingClientRect(), dx=rect.left+rect.width/2-ox, dy=rect.top+rect.height/2-oy;
      const forward=direction==='left'?-dx:direction==='right'?dx:direction==='up'?-dy:dy;
      if (forward<=1) continue;
      const across=Math.abs(direction==='left'||direction==='right'?dy:dx);
      const distance=forward+across*3+across*across/Math.max(forward,1);
      if (distance<score) {best=node;score=distance;}
    }
    return best;
  }
  function install(window) {
    const document=window.document, controllerKeyEvents=new WeakSet();
    function controls() {
      const scope=document.querySelector('#overlay-root [role="dialog"]') || document;
      return [...scope.querySelectorAll(selector)].filter(node=>node.tabIndex>=0 && !node.matches(':disabled') && !node.closest('[hidden],[inert]')
        && (!node.closest('details:not([open])') || node.closest('details:not([open])').querySelector('summary')===node)
        && node.getClientRects().length && window.getComputedStyle(node).visibility!=='hidden');
    }
    function focus(node) {
      if (!node) return;
      document.documentElement.classList.add('gamepad-navigation');
      node.focus();node.scrollIntoView({block:'nearest',inline:'nearest'});
    }
    function dispatch(action) {
      document.documentElement.classList.add('gamepad-navigation');
      if (action==='back') {
        const event=new window.KeyboardEvent('keydown',{key:'Escape',bubbles:true});
        controllerKeyEvents.add(event);
        document.dispatchEvent(event);return;
      }
      const options=controls();if (!options.length) return;
      const active=document.activeElement;
      if (!options.includes(active)) {focus(options[0]);return;}
      if (action==='activate') {
        if (active.tagName==='SELECT') {active.focus();return;}
        active.click();return;
      }
      // Selects are changed in place; bumpers always let the user leave them.
      if (active.tagName==='SELECT' && ['up','down','left','right'].includes(action)) {
        const choices=[...active.options].filter(option=>!option.disabled && !option.hidden && !option.parentElement?.disabled);
        const index=choices.indexOf(active.selectedOptions[0]);
        const next=choices[Math.max(0,Math.min(choices.length-1,index+(['up','left'].includes(action)?-1:1)))];
        if (next && !next.selected) {next.selected=true;active.dispatchEvent(new window.Event('change',{bubbles:true}));}
        return;
      }
      const index=options.indexOf(active);
      if (action==='next'||action==='previous') focus(options[(index+(action==='next'?1:-1)+options.length)%options.length]);
      else focus(nearest(active,options,action));
    }
    const poll=createInput(dispatch);
    let running=false;
    function frame(time) {
      let pads=[];
      try {pads=Array.from(window.navigator.getGamepads?.() || []);} catch (_) { /* Restricted web preview. */ }
      poll(pads,time,document.visibilityState!=='hidden' && document.hasFocus());
      running=pads.some(pad=>pad?.connected!==false && pad?.mapping==='standard');
      const help=document.getElementById?.('controller-help');if (help) {help.hidden=!running;help.textContent=promptLabels(window.matrixPromptStyle,pads);}
      if (running) window.requestAnimationFrame(frame);
    }
    function start() {if (!running) {running=true;window.requestAnimationFrame(frame);}}
    // Animation frames may stop completely while hidden or unfocused, so the
    // next sample must not turn a background press into a foreground action.
    const rearm=()=>{poll.reset();start();};
    window.addEventListener('gamepadconnected',start);
    window.addEventListener('blur',()=>poll.reset());
    window.addEventListener('focus',rearm);
    document.addEventListener('visibilitychange',rearm);
    document.addEventListener('pointerdown',()=>document.documentElement.classList.remove('gamepad-navigation'));
    document.addEventListener('keydown',event=>{if (!controllerKeyEvents.has(event)) document.documentElement.classList.remove('gamepad-navigation');});
    start();
  }
  if (typeof module!=='undefined' && module.exports) module.exports={commands,createInput,nearest,install,promptLabels};
  else if (root.document) install(root);
})(typeof window!=='undefined'?window:globalThis);
