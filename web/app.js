/* Matrix-Play v0.6.9 — native Tauri library, launcher, and local artwork bridge. */
'use strict';

const TITLE_COVER_KINDS = ['ps3','ps4','psp','vita','n64','dreamcast','wiiu','gb','gba','nds','3ds'];
const AUTO_COVER_KINDS = ['ps1','ps2','gamecube','wii','switch',...TITLE_COVER_KINDS];
const byId = Object.fromEntries(consoles.map(consoleInfo => [consoleInfo.id, consoleInfo]));
const state = {
  page: 'home', consoleId: null, vendor: 'All', query: '',
  gameFilter: 'All', sort: 'default', view: 'list', previewGame: null,
  overlay: null, selectedGame: null, lastFocus: null,
  configs: {}, imported: {}, favorites: new Set(), art: new Map(),
  native: Boolean(window.__TAURI__?.core?.invoke), root: '', folders: {},
  steam: [], windows: [],
  discovery:{candidates:[],linked:[],ambiguous:[],notices:[]}, launches:{},launchBusy:{},launchGeneration:{},
  missingArt: new Set(), pendingArt: new Map(),
  readyProfiles: {}, recent: [], coverQueue: {paused:false,items:[]},
  artwork: {enabled:true, style:"default", titleEnabled:false}, artworkSaving: false, coverStatus: new Map(), coverGeneration: 0, artRevision: 0,
};
const $ = selector => document.querySelector(selector);
const nativeInvoke = (name, args = {}) => window.__TAURI__.core.invoke(name, args);
const escapeHtml = value => String(value ?? '').replace(/[&<>"']/g, char => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
})[char]);
const glyph = name => {
  const symbols = {
    back: '<path d="m15 18-6-6 6-6"/>',
    arrow: '<path d="M5 12h14m-6-6 6 6-6 6"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/>',
    close: '<path d="M5 5l14 14M19 5 5 19"/>',
    play: '<path d="m8 5 11 7-11 7z"/>',
    heart: '<path d="M20.5 5.2a5.4 5.4 0 0 0-7.7 0L12 6l-.8-.8a5.4 5.4 0 0 0-7.7 7.7L12 21l8.5-8.1a5.4 5.4 0 0 0 0-7.7Z"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8" cy="8" r="1.4"/><path d="m21 16-5-5-9 10"/>',
    grid: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
    list: '<path d="M4 6h16M4 12h16M4 18h16"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5m0-8v.1"/>',
    trash: '<path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13"/>',
  };
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${symbols[name] || symbols.info}</svg>`;
};

const consoleImages = {
  "switch": "./assets/consoles/switch.webp",
  "ps1": "./assets/consoles/ps1.webp",
  "ps2": "./assets/consoles/ps2.webp",
  "ps3": "./assets/consoles/ps3.webp",
  "psp": "./assets/consoles/psp.webp",
  "n64": "./assets/consoles/n64.webp",
  "gamecube": "./assets/consoles/gamecube.webp",
  "dreamcast": "./assets/consoles/dreamcast.webp",
  "wii": "./assets/consoles/wii.webp",
  "wiiu": "./assets/consoles/wiiu.webp",
  "gb": "./assets/consoles/gb.webp",
  "gba": "./assets/consoles/gba.webp",
  "nds": "./assets/consoles/nds.webp",
  "3ds": "./assets/consoles/3ds.webp",
  "ps4": "./assets/consoles/ps4.webp",
  "ps5": "./assets/consoles/ps5.webp",
  "vita": "./assets/consoles/vita.webp",
  "switch2": "./assets/consoles/switch2.webp",
  "genesis": "./assets/consoles/genesis.webp",
  "saturn": "./assets/consoles/saturn.webp",
  "nes": "./assets/consoles/nes.webp",
  "snes": "./assets/consoles/snes.webp",
  "xbox": "./assets/consoles/xbox.webp",
  "xbox360": "./assets/consoles/xbox360.webp",
};
// Supplied console artwork; original silhouettes remain a fallback.
function deviceArt(id) {
  if (consoleImages[id]) return `<img class="device-illustration console-image" src="${consoleImages[id]}" alt="" decoding="async">`;
  const hardware = {
    nes: '<ellipse cx="71" cy="112" rx="53" ry="7" fill="#020815" stroke="none"/><path d="m15 61 20-29h88l3 29Z" fill="#b9c1d0" stroke="#77869f"/><path d="M15 61h111v38l-8 9H23l-8-9Z" fill="#919bad" stroke="#66758c"/><path d="M23 63h95v18H23Z" fill="#29364b" stroke="none"/><path d="M56 33h51v26H39Z" fill="#d7dce4" stroke="#8c98ac"/><path d="M76 37h25m-27 5h27m-28 5h28m-29 5h29" stroke="#65748c"/><path d="M24 89h13v7H24Z" fill="#ea5e6c" stroke="none"/><path d="M44 89h13v7H44Z" fill="#cbd3e0" stroke="none"/><path d="M76 90h15v9H76Zm22 0h15v9H98Z" fill="#29364b" stroke="none"/>',
    snes: '<ellipse cx="70" cy="112" rx="52" ry="7" fill="#020815" stroke="none"/><path d="M17 66q0-12 12-24l9-9h66l13 12q9 9 9 21v27q0 12-13 12H30q-13 0-13-12Z" fill="#a5aec3" stroke="#71819b"/><path d="M18 66h107v25q0 12-13 12H31q-13 0-13-12Z" fill="#808da6" stroke="#71819b"/><path d="M39 35h61l8 28H30Z" fill="#c8cfdf" stroke="#9aa6be"/><path d="M42 44h55v8H42Z" fill="#34415e" stroke="none"/><path d="m30 66 15-5v12l-15 4Z" fill="#a18adc" stroke="#7465b0"/><path d="m96 61 16 5v11l-16-4Z" fill="#a18adc" stroke="#7465b0"/><path d="M41 86h22v10H41Zm36 0h22v10H77Z" fill="#26334c" stroke="none"/><circle cx="26" cy="87" r="2" fill="#e75e6e" stroke="none"/>',
    genesis: '<ellipse cx="70" cy="111" rx="56" ry="7" fill="#020815" stroke="none"/><path d="M16 50h109l5 45q0 12-15 12H25Q10 107 10 95Z" fill="#171e2d" stroke="#5a6c8c"/><ellipse cx="70" cy="54" rx="56" ry="29" fill="#303c52" stroke="#72809b"/><ellipse cx="70" cy="51" rx="32" ry="21" fill="#151c2b" stroke="#bda976"/><ellipse cx="70" cy="51" rx="28" ry="17" fill="#253147" stroke="#67758f"/><path d="M42 43h57v10H42Z" fill="#090f1c" stroke="#4c5e7a"/><path d="M20 87h100" stroke="#627493"/><path d="M32 94h24v9H32Zm50 0h24v9H82Z" fill="#070d18" stroke="#566783"/><circle cx="116" cy="77" r="3" fill="#e56575" stroke="none"/><path d="M20 74h15" stroke="#c6a768"/>',
    saturn: '<ellipse cx="70" cy="111" rx="54" ry="7" fill="#020815" stroke="none"/><rect x="15" y="49" width="111" height="58" rx="17" fill="#29354b" stroke="#6c7d9c"/><path d="M16 63q2-30 30-34h48q29 4 31 34Z" fill="#3e4d66" stroke="#8492ac"/><ellipse cx="70" cy="52" rx="33" ry="22" fill="#303e56" stroke="#7e8fac"/><ellipse cx="70" cy="52" rx="28" ry="18" fill="#253147" stroke="#526582"/><path d="M19 81h104" stroke="#7687a1"/><path d="M31 91h25v9H31Zm52 0h25v9H83Z" fill="#0d1728" stroke="#5c6c88"/><circle cx="27" cy="72" r="4" fill="#748cad"/><circle cx="114" cy="72" r="4" fill="#748cad"/><circle cx="41" cy="73" r="2" fill="#67c2e3" stroke="none"/>',
    xbox: '<ellipse cx="70" cy="111" rx="55" ry="7" fill="#020815" stroke="none"/><rect x="14" y="46" width="112" height="61" rx="10" fill="#1f302f" stroke="#637d78"/><path d="m15 49 16-19h78l16 19v29H15Z" fill="#334c42" stroke="#7b9680"/><path d="m33 34 37 20 36-20M21 74l49-20 48 20" stroke="#83b85b" stroke-width="9"/><path d="m33 34 37 20 36-20M21 74l49-20 48 20" stroke="#abc977" stroke-width="2"/><ellipse cx="70" cy="54" rx="14" ry="10" fill="#9fc969" stroke="#314d2a"/><path d="M26 87h51v7H26Z" fill="#111e24" stroke="#56766b"/><circle cx="98" cy="91" r="5" fill="#9bd365" stroke="#446e3c"/><circle cx="114" cy="92" r="2" fill="#8abc69" stroke="none"/>',
    xbox360: '<ellipse cx="70" cy="118" rx="32" ry="6" fill="#020815" stroke="none"/><path d="m44 15 43 1 13 10-3 86-12 7-42-5q8-50 1-99Z" fill="#d3dce3" stroke="#9caec0"/><path d="m87 16 13 10-3 86-12 7q-8-52 2-103Z" fill="#91a6b6" stroke="#708a9e"/><path d="M52 32h28v6H52Z" fill="#445d72" stroke="#748fa3"/><path d="M53 48h25m-24 6h23m-23 6h23m-23 6h23" stroke="#9cafbf"/><circle cx="65" cy="94" r="8" fill="#c2d0d9" stroke="#80c767" stroke-width="2.7"/><circle cx="65" cy="94" r="3" fill="#83b45f" stroke="none"/><path d="M91 39v55" stroke="#627c91"/>',
  };
  if (hardware[id]) return `<svg class="device-illustration hardware-art" viewBox="0 0 140 130" aria-hidden="true">${hardware[id]}</svg>`;
  const handheld = (twoScreens = false) => twoScreens
    ? '<rect x="27" y="11" width="85" height="99" rx="13"/><rect x="38" y="19" width="63" height="33" rx="3"/><path d="M30 61h79"/><rect x="38" y="69" width="63" height="30" rx="3"/><circle cx="20" cy="84" r="3"/><circle cx="118" cy="84" r="3"/>'
    : '<rect x="10" y="33" width="120" height="55" rx="25"/><rect x="37" y="40" width="66" height="41" rx="4"/><path d="M24 53v15m-7-7h14"/><circle cx="115" cy="54" r="3"/><circle cx="120" cy="66" r="3"/>';
  const designs = {

    switch: '<rect x="33" y="27" width="74" height="72" rx="6"/><rect x="42" y="36" width="56" height="54" rx="2"/><rect x="11" y="27" width="22" height="72" rx="11"/><rect x="107" y="27" width="22" height="72" rx="11"/><circle cx="22" cy="45" r="5"/><circle cx="118" cy="77" r="5"/>',
    ps1: '<rect x="19" y="47" width="105" height="45" rx="11"/><ellipse cx="71" cy="48" rx="38" ry="24"/><ellipse cx="71" cy="48" rx="24" ry="15"/><circle cx="35" cy="76" r="3"/><path d="M83 76h21"/>',
    ps2: '<path d="M25 35h94l-14 55H11z"/><path d="M38 35 24 91M48 40h58M37 51h67M28 65h69"/><path d="m69 66 26-1"/>',
    ps3: '<rect x="43" y="18" width="50" height="100" rx="12"/><path d="M51 25h34M51 106h34M59 18q-15 45 0 100"/>',
    psp: handheld(),
    n64: '<path d="M15 83 28 49h84l13 34-11 13H26z"/><path d="M40 49 51 36h39l11 13"/><ellipse cx="71" cy="64" rx="25" ry="12"/><circle cx="71" cy="63" r="4"/>',
    gamecube: '<path d="m34 36 72-13 17 64-76 15z"/><path d="m34 36-17 15 29 51M34 36l17-11 72-2"/><ellipse cx="74" cy="44" rx="20" ry="8" transform="rotate(-13 74 44)"/><path d="M52 85h23"/>',
    dreamcast: '<path d="m21 40 92-5 13 62-96 8z"/><ellipse cx="74" cy="64" rx="32" ry="21"/><ellipse cx="74" cy="64" rx="16" ry="11"/><circle cx="32" cy="92" r="3"/>',
    wii: '<path d="m50 14 26 2 15 99-23-2z"/><path d="M53 15 40 114l28-1M49 104l42 6"/><path d="M62 25h9"/>',
    wiiu: '<rect x="19" y="34" width="102" height="61" rx="18"/><rect x="40" y="42" width="62" height="43" rx="5"/><circle cx="28" cy="60" r="4"/><circle cx="112" cy="69" r="4"/><path d="M17 26h106"/>',
    gb: '<rect x="41" y="8" width="63" height="111" rx="9"/><rect x="49" y="17" width="47" height="51" rx="4"/><path d="M55 83v18m-8-9h16"/><circle cx="82" cy="88" r="3"/><circle cx="91" cy="81" r="3"/>',
    gba: handheld(),
    nds: handheld(true),
    '3ds': handheld(true),
  };
  return `<svg class="device-illustration" viewBox="0 0 140 130" aria-hidden="true">${designs[id] || handheld()}</svg>`;
}

const EXTS = {
  switch: ['nsp', 'xci'], ps1: ['cue','chd','iso','pbp','m3u'], ps2: ['iso','chd','cso'],
  ps3: ['iso'], psp: ['iso','cso','pbp','chd'], n64: ['z64','n64','v64'],
  gamecube: ['iso','gcm','rvz','gcz','wia','ciso'], dreamcast: ['gdi','cdi','chd'],
  wii: ['wbfs','iso','rvz','wia','gcz','ciso'], wiiu: ['wua','wud','wux','rpx'],
  gb: ['gb','gbc'], gba: ['gba'], nds: ['nds'], '3ds': ['3ds','cci','cxi'], ps4:['elf','bin'], ps5:['pkg','iso','elf','zar','bin'], vita:['vpk','zip'], switch2:['nsp','xci','nup','nsz','xcz','nx','nro','nso','nca','kip'],
  genesis:['md','gen','smd','bin','zip'],saturn:['cue','ccd','toc'],nes:['nes','unf','unif','fds','zip'],snes:['sfc','smc','zip'],xbox:['iso'],xbox360:['iso'],
};
const tones = { Sony: '#648aff', Nintendo: '#fa7687', Sega: '#53c9ec', Xbox: '#91d179' };
const consoleTone = consoleInfo => tones[consoleInfo.group] || '#657aa5';
const artKey = (consoleId, uid) => `${consoleId}|${uid}`;
const favoriteKey = (consoleId, uid) => `${consoleId}|${uid}`;
const importedFor = id => Array.isArray(state.imported[id]) ? state.imported[id] : [];
const isConfigured = id => !byId[id]?.libraryOnly && Boolean((state.configs[id]?.launcher === "flatpak" ? state.configs[id]?.flatpakId : state.configs[id]?.emulatorPath) && (state.native || state.configs[id]?.folderPath));
const consoleReady = id => isConfigured(id) && (!state.native || state.readyProfiles[id]===true) && importedFor(id).some(g=>!g.offline);
const configuredCount = () => consoles.filter(consoleInfo => isConfigured(consoleInfo.id)).length;

function demoGames(consoleInfo) {
  return consoleInfo.games.map(([title, year, a, b], index) => ({
    uid: `demo-${index}`, title, year, a, b, demo: true,
  }));
}
function gamesFor(id) {
  const saved = importedFor(id);
  const rows=state.native ? saved : (saved.length ? saved : demoGames(byId[id]));
  return rows.map(g=>({...g,originalTitle:g.originalTitle||g.title,title:gameMetadata(id,g.uid).display||g.title}));
}
function findGame(id, uid) {
  return gamesFor(id).find(game => game.uid === uid);
}
const preferenceMap = value => value && typeof value === 'object' && !Array.isArray(value) ? value : {};
function storedPrefs() {
  const stored = localStorage.getItem('matrix-ui-v03') || localStorage.getItem('console-hub-v02');
  if (stored) {
    // Invalid fields are isolated so valid neighboring preferences survive.
    let raw;
    try { raw = JSON.parse(stored); } catch { /* Try legacy recovery below. */ }
    if (raw && typeof raw === 'object' && !Array.isArray(raw)) return {
      configs: preferenceMap(raw.configs), imported: preferenceMap(raw.imported),
      favorites: Array.isArray(raw.favorites) ? raw.favorites.filter(x => typeof x === 'string') : [],
    };
  }
  let previous;
  try { previous = preferenceMap(JSON.parse(localStorage.getItem('console-hub-ui-v1') || '{}')); } catch { previous = {}; }
  return {
    configs: preferenceMap(previous.configs), imported: {},
    favorites: Array.isArray(previous.favorites) ? previous.favorites.filter(key => typeof key === 'string').map(key => {
      const [consoleId, index] = key.split(':');
      return favoriteKey(consoleId, `demo-${index}`);
    }) : [],
  };
}
function readPrefs() {
  try {
    const saved = storedPrefs();
    state.configs = saved.configs;
    state.imported = saved.imported;
    state.favorites = new Set(saved.favorites);
    try{state.libraryPrefs=normalizeLibraryPrefs(JSON.parse(localStorage.getItem('matrix-library-preferences')));}catch{}
  } catch { /* Browser storage can be disabled for local files. */ }
}
function savePrefs(change) {
  try {
    // Persist only this action, never the rest of this tab's possibly stale state.
    const saved = storedPrefs();
    if (change.type === 'favorite') {
      const favorites = new Set(saved.favorites);
      if (change.favorite) favorites.add(change.key); else favorites.delete(change.key);
      saved.favorites = [...favorites];
    } else if (change.type === 'profile') {
      saved.configs[change.id] = change.value;
    } else if (change.type === 'import') {
      if (change.value === undefined) delete saved.imported[change.id];
      else saved.imported[change.id] = change.value;
    } else throw new Error('Unknown preference change');
    localStorage.setItem('matrix-ui-v03', JSON.stringify(saved));
    return true;
  } catch {
    toast('The browser could not persist these UI preview settings.');
    return false;
  }
}
function evictCover(key) {
  const uri = state.art.get(key);
  state.art.delete(key);
  if (uri?.startsWith('blob:')) URL.revokeObjectURL(uri);
}
function cacheCover(key, uri) {
  if (state.art.get(key) === uri) return;
  evictCover(key);
  state.art.set(key, uri);
}
function clearCoverCache() {
  for (const key of state.art.keys()) evictCover(key);
}
let artDb = null;
async function initializeArt() {
  // Native Matrix-Play artwork lives in XDG data; browser preview uses IndexedDB.
  if (state.native || !('indexedDB' in window)) return;
  try {
    artDb = await new Promise((resolve, reject) => {
      const req = indexedDB.open('console-hub-art-v02', 1);
      req.onupgradeneeded = () => { req.result.createObjectStore('covers', { keyPath: 'key' }); };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    const all = await new Promise((resolve, reject) => {
      const req = artDb.transaction('covers', 'readonly').objectStore('covers').getAll();
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    for (const item of all) {
      // Startup storage may finish after a local choice. Merge only untouched
      // keys so the stale snapshot cannot replace or revoke that newer cover.
      if (item?.blob instanceof Blob && !state.art.has(item.key)) cacheCover(item.key, URL.createObjectURL(item.blob));
    }
    render();
  } catch { artDb = null; /* In-memory artwork still works. */ }
}
async function storeArt(key, blob) {
  if (!artDb) return false;
  try {
    await new Promise((resolve, reject) => {
      const tx = artDb.transaction('covers', 'readwrite');
      tx.objectStore('covers').put({ key, blob });
      tx.oncomplete = resolve;
      tx.onerror = () => reject(tx.error);
    });
    return true;
  } catch { return false; }
}

function renderNav() {
  const names = ['consoles','steam','windows','settings'];
  const active = state.page === 'home' || state.page === 'console' ? 'consoles' : state.page;
  for (const name of names) {
    const node = $(`#nav-${name}`);
    node.classList.toggle('active', name === active);
    node.setAttribute('aria-current', name === active ? 'page' : 'false');
  }
  const name = state.page === 'console' ? byId[state.consoleId]?.abbrev : null;
  $('#breadcrumbs').innerHTML = state.page === 'settings'
    ? 'LIBRARY <span>/</span> SETTINGS'
    : state.page === 'steam' ? 'LIBRARY <span>/</span> STEAM GAMES'
    : state.page === 'windows' ? 'LIBRARY <span>/</span> WINDOWS GAMES'
    : name ? `LIBRARY <span>/</span> CONSOLES <span>/</span> ${escapeHtml(name)}`
    : 'LIBRARY <span>/</span> CONSOLES';
}
function consoleCard(consoleInfo) {
  const count=gamesFor(consoleInfo.id).length;
  return `<button class="console-card" style="--tint:${consoleTone(consoleInfo)}" type="button" data-action="console" data-id="${consoleInfo.id}" aria-label="Browse ${escapeHtml(consoleInfo.name)} library">
    <span class="console-card__top"><span class="console-brand">${escapeHtml(consoleInfo.group==='Sony'?'PlayStation':consoleInfo.group)}</span><span class="connection-dot ${consoleReady(consoleInfo.id)?'ready':''}" title="${consoleReady(consoleInfo.id)?'Emulator and games ready':!isConfigured(consoleInfo.id)?'Set up emulator':state.native&&state.readyProfiles[consoleInfo.id]!==true?'Emulator unavailable':'Add games'}"></span></span>
    <span class="console-card__art">${deviceArt(consoleInfo.id)}</span><span class="console-card__bottom"><strong>${escapeHtml(consoleInfo.name)}</strong><small>${count} ${count===1?'game':'games'}</small></span></button>`;
}
function renderHome() {
  const focus=captureLibraryFocus($('#screen'),'home');
  const visible=consoleChoices();
  $('#screen').innerHTML=`<div class="title-row"><div><h1 class="page-title">Consoles</h1><p class="page-subtitle">${consoles.length} libraries</p></div></div>
    <div class="section-toolbar"><div class="segmented brand-filters" role="group" aria-label="Filter console brands">${['All','Sony','Nintendo','Sega','Xbox'].map(v=>`<button type="button" class="${state.vendor===v?'active':''}" data-action="vendor" data-value="${v}" aria-pressed="${state.vendor===v}">${v==='All'?'All systems':v==='Sony'?'PlayStation':v}</button>`).join('')}</div></div>
    ${state.recent.length?`<details class="library-notes recent-games"><summary>Recently played</summary><div class="tag-row">${state.recent.slice(0,8).map(g=>`<button class="button button-ghost" type="button" data-action="recent-game" data-id="${escapeHtml(g.kind)}" data-uid="${escapeHtml(g.uid)}">${escapeHtml(g.title)}</button>`).join('')}</div></details>`:''}
    <div class="console-browser"><div class="console-grid">${visible.map(consoleCard).join('')||'<p>No consoles match. Adjust Console visibility in Settings.</p>'}</div><aside class="console-preview" aria-label="Console preview">${consolePreview(visible.find(c=>c.id==='switch2') || visible[0])}</aside></div>`;
  restoreLibraryFocus(focus);
}
function consolePreview(consoleInfo) {
  if (!consoleInfo) return '';
  const count=gamesFor(consoleInfo.id).length;
  return `<div class="preview-device" aria-hidden="true">${deviceArt(consoleInfo.id)}</div><h2>${escapeHtml(consoleInfo.name)}</h2><p>${escapeHtml(consoleInfo.group)}</p><p>${count} ${count===1?'game':'games'}</p><button type="button" class="button button-primary" data-action="console" data-id="${consoleInfo.id}">Open library</button>`;
}
function gameCover(consoleInfo, game, index) {
  const key = artKey(consoleInfo.id, game.uid);
  const actual = state.art.get(key);
  const colors = [game.a || ['#ba6164','#5b879d','#6299bb','#9d7998'][index % 4], game.b || ['#2f2035','#253047','#222c52','#392941'][index % 4]];
  return `<div class="game-cover style-${index % 4}" data-art-kind="${escapeHtml(consoleInfo.id)}" data-art-uid="${escapeHtml(game.uid)}" data-art-key="${escapeHtml(key)}" style="--art-a:${escapeHtml(colors[0])};--art-b:${escapeHtml(colors[1])}">
    <span class="cover-system">${escapeHtml(consoleInfo.abbrev)}</span><span class="cover-faux-label">${escapeHtml(game.title)}</span><span class="cover-line"></span>
    ${actual ? `<img src="${escapeHtml(actual)}" alt="Custom cover for ${escapeHtml(game.title)}">` : ''}</div>`;
}
let coverObserver = null;

// Only fetch artwork as a game card becomes visible. Avoid transferring a whole
// Steam library of multi-megabyte images across Tauri IPC on every refresh.
function updateVisibleCover(key, uri) {
  document.querySelectorAll('.game-cover[data-art-key]').forEach(container => {
    if (container.dataset.artKey !== key) return;
    let img = container.querySelector('img');
    if (!img) {
      img = document.createElement('img');
      container.appendChild(img);
    }
    img.src = uri;
    img.alt = 'Cover artwork';
  });
}

async function requestNativeCover(element) {
  if (!state.native || !element.isConnected) return;
  const { artKey: key, artKind: kind, artUid: uid } = element.dataset;
  if (!key) return;
  if (state.art.has(key)) { updateVisibleCover(key, state.art.get(key)); return; }
  const revision = state.artRevision;
  if (state.missingArt.has(key) || state.pendingArt.get(key) === revision) return;
  // An invalidated request must not block a new card rendered by a rescan.
  state.pendingArt.set(key, revision);
  try {
    const uri = await nativeInvoke('get_cover', { kind, uid });
    if (revision !== state.artRevision) return;
    if (uri) {
      cacheCover(key, uri);
      updateVisibleCover(key, uri);
    } else state.missingArt.add(key);
  } catch {
    // Errors from an old request must not mark the current artwork missing.
    if (revision === state.artRevision) state.missingArt.add(key);
  } finally {
    if (state.pendingArt.get(key) === revision) state.pendingArt.delete(key);
    if (revision !== state.artRevision) {
      // The original card may have been removed while this request was pending.
      document.querySelectorAll('.game-cover[data-art-key]').forEach(node => {
        if (node.dataset.artKey === key) void requestNativeCover(node);
      });
    }
  }
}

function hydrateNativeCovers() {
  if (!state.native) return;
  if (coverObserver) coverObserver.disconnect();
  const cards = [...document.querySelectorAll('.game-cover[data-art-key]')];
  if (!('IntersectionObserver' in window)) {
    cards.slice(0, 80).forEach(card => void requestNativeCover(card));
    return;
  }
  coverObserver = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      coverObserver.unobserve(entry.target);
      void requestNativeCover(entry.target);
    }
  }, { rootMargin: '300px 0px' });
  for (const card of cards) coverObserver.observe(card);
}

const favoriteCount=id=>gamesFor(id).filter(game=>state.favorites.has(favoriteKey(id,game.uid))).length;
function applyNativeFavorites(entries) {
  state.favorites=new Set(Object.entries(entries || {}).flatMap(([id,uids])=>byId[id] && Array.isArray(uids) ? uids.map(uid=>favoriteKey(id,uid)) : []));
}
async function loadNativeFavorites(config, canApply=()=>true) {
  let entries=config.favorites || {};
  if (!config.favoritesMigrated) {
    const legacy={};
    for (const key of state.favorites) {
      const split=key.indexOf('|'), id=key.slice(0,split), uid=key.slice(split+1);
      if (byId[id] && uid.startsWith('/')) (legacy[id] ||= []).push(uid);
    }
    entries=await nativeInvoke('import_favorites',{entries:legacy});
  }
  if (canApply()) applyNativeFavorites(entries);
}
let favoriteQueue=Promise.resolve();
function toggleFavorite(id,uid,button) {
  button.disabled=true;
  favoriteQueue=favoriteQueue.then(async()=>{
    const key=favoriteKey(id,uid), favorite=!state.favorites.has(key);
    if (state.native) {
      const list=await nativeInvoke('set_favorite',{id,uid,favorite});
      for (const old of state.favorites) if (old.startsWith(id+'|')) state.favorites.delete(old);
      for (const value of list) state.favorites.add(favoriteKey(id,value));
    } else if (favorite) state.favorites.add(key);else state.favorites.delete(key);
    favoriteWriteRevision++;
    const persisted = state.native || savePrefs({type:'favorite',key,favorite});
    if (state.page==='console' && state.consoleId===id) renderConsole();
    if (state.overlay==='game' && state.selectedGame?.id===id && state.selectedGame?.uid===uid) {
      // Keep the live details form, draft serial and focus intact.
      const control=$('#overlay-root [data-action="favorite"]');
      if (control) { control.innerHTML=`${glyph('heart')} ${favorite ? 'Unfavorite' : 'Favorite'}`;control.setAttribute('aria-pressed',String(favorite)); }
    }
    if (persisted) toast(favorite ? `Added to ${byId[id].name} favorites.` : `Removed from ${byId[id].name} favorites.`);
  }).catch(error=>toast(`Could not save favorite: ${error}`)).finally(()=>{button.disabled=false;});
}

function gameCard(consoleInfo, game, index) {
  const liked = state.favorites.has(favoriteKey(consoleInfo.id, game.uid));
  return `<div class="game-item ${state.previewGame?.id===consoleInfo.id && state.previewGame?.uid===game.uid ? 'is-previewed' : ''}" data-preview-uid="${escapeHtml(game.uid)}">
    <button class="game-card" type="button" data-action="quick-launch" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}" aria-label="${state.native && !consoleInfo.libraryOnly ? 'Launch' : 'View'} ${escapeHtml(game.title)}${state.native && !consoleInfo.libraryOnly ? '' : ' details'}">
      ${gameCover(consoleInfo, game, index)}<span class="game-title">${escapeHtml(game.title)}${game.versions?.length>1?` <small class="version-count">${game.versions.length} versions</small>`:''}${game.offline?' <small class="offline-badge">Offline</small>':''}</span><span class="game-meta">${game.year || (game.demo ? 'Sample game' : state.native ? consoleInfo.libraryOnly ? 'Collection file' : 'Installed game' : 'Folder preview')} · ${escapeHtml(consoleInfo.abbrev)}</span></button>
    <button class="game-info" type="button" data-action="game" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}" aria-label="Details for ${escapeHtml(game.title)}" title="Game details">${glyph('info')}</button>
    <button class="favorite ${liked ? 'active' : ''}" type="button" data-action="favorite" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}" aria-label="${liked ? 'Remove' : 'Add'} ${escapeHtml(game.title)} ${liked ? 'from' : 'to'} favorites" aria-pressed="${liked}">${glyph('heart')}</button>
  </div>`;
}
function releaseYear(game) { return Number.isInteger(game.year) && game.year>0 ? game.year : 0; }
function hasReleaseYears(id) { return gamesFor(id).some(game=>releaseYear(game)>0); }
function normalizeGameSort(id) { if (state.sort==='recent' && !hasReleaseYears(id)) state.sort='default'; }
function visibleGames(id) {
  normalizeGameSort(id);
  let result = gamesFor(id).filter(game => game.title.toLocaleLowerCase().includes(state.query.toLocaleLowerCase()));
  if (state.gameFilter === 'Favorites') result = result.filter(game => state.favorites.has(favoriteKey(id, game.uid)));
  if (state.sort === 'az') result.sort((a,b) => a.title.localeCompare(b.title));
  if (state.sort === 'recent') result.sort((a,b) => releaseYear(b) - releaseYear(a));
  return groupedGames(id,result);
}
// Keep keyboard/controller position when a library render replaces its controls.
// Disabling an async favorite button may blur it before the render begins.
let lastLibraryFocus = null, lastFocusRender = null;
document.addEventListener?.('focusin', event => {
  if ($('#screen')?.contains?.(event.target)) lastLibraryFocus = event.target;
});
function captureLibraryFocus(scope, page = null) {
  // A matching page name alone is insufficient after replacing a screen: the old
  // screen must still be the one rendered here, not a different screen's UI.
  if (page && (lastFocusRender?.page !== page || lastFocusRender.first !== scope?.firstElementChild)) return {page};
  const describe = node => scope?.contains?.(node) ? {
    id: node.id, action: node.dataset?.action, consoleId: node.dataset?.id,
    uid: node.dataset?.uid, kind: node.dataset?.kind, value: node.dataset?.value,
    start: node.selectionStart, end: node.selectionEnd, direction: node.selectionDirection,
  } : null;
  let active = document.activeElement;
  if (active === document.body && lastLibraryFocus?.disabled && lastLibraryFocus.isConnected) active = lastLibraryFocus;
  return { page, active: describe(active), opener: describe(state.lastFocus) };
}
function restoreLibraryFocus(saved) {
  const screen = $('#screen');
  if (saved.page) lastFocusRender = {page:saved.page, first:screen?.firstElementChild};
  const resolve = key => {
    if (!key) return null;
    const controls = [...(screen?.querySelectorAll('button,input,select,textarea,a[href],[tabindex]') || [])];
    return controls.find(node => !node.disabled && (key.id ? node.id === key.id : key.action
      && node.dataset?.action === key.action && node.dataset?.id === key.consoleId
      && node.dataset?.uid === key.uid && node.dataset?.kind === key.kind && node.dataset?.value === key.value))
      || (state.page === 'home' ? controls.find(node => !node.disabled && node.dataset?.action === 'vendor' && node.dataset?.value === state.vendor) : null)
      || controls.find(node => node.dataset?.action === 'filter-games' && node.dataset?.value === state.gameFilter)
      || (state.page === 'settings' ? controls.find(node => !node.disabled && node.dataset?.action === 'configure' && node.dataset?.id === key.consoleId)
        || controls.find(node => !node.disabled && node.dataset?.action === 'configure') : null)
      || $('#external-search') || $('#game-search');
  };
  if (saved.opener) state.lastFocus = resolve(saved.opener);
  if (saved.active && !state.overlay) {
    const target = resolve(saved.active);
    target?.focus();
    if (target?.id === saved.active.id && typeof saved.active.start === 'number' && target.setSelectionRange) {
      target.setSelectionRange(saved.active.start, saved.active.end, saved.active.direction);
    }
  }
}
function renderGamePreview(consoleInfo, game) {
  const preview=$('#game-preview');
  if (!preview) return;
  preview.innerHTML=game ? `${gameCover(consoleInfo,game,0)}<h2>${escapeHtml(game.title)}</h2><p>${escapeHtml(consoleInfo.name)}</p><button type="button" class="button button-ghost" data-action="game" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}">Game details</button>` : '<div class="preview-empty"><p>Select a game to preview its cover.</p></div>';
}
function previewLibraryGame(id, uid) {
  if(state.page!=='console' || state.consoleId!==id || state.previewGame?.id===id && state.previewGame?.uid===uid)return;
  const game=visibleGames(id).find(game=>game.uid===uid);
  if(!game)return;
  state.previewGame={id,uid};
  if(window.setTimeout){clearTimeout(state.viewSaveTimer);state.viewSaveTimer=window.setTimeout(rememberLibrary,500);}
  document.querySelectorAll('#game-results .game-item').forEach(item=>item.classList.toggle('is-previewed',item.dataset.previewUid===uid));
  renderGamePreview(byId[id],game);hydrateNativeCovers();
}
function renderGameResults() {
  const focus = captureLibraryFocus($('#game-results'));
  const consoleInfo = byId[state.consoleId];
  const games = visibleGames(consoleInfo.id);
  const preview=games.find(game=>state.previewGame?.id===consoleInfo.id && game.uid===state.previewGame.uid)||games[0];
  state.previewGame=preview ? {id:consoleInfo.id,uid:preview.uid} : null;
  const sortControl=$('#sort-games');if (sortControl) sortControl.value=state.sort;
  $('#game-results').innerHTML = games.length
    ? `<div class="${state.view === 'grid' ? 'game-grid' : 'game-list'}">${games.map((game,index) => gameCard(consoleInfo,game,index)).join('')}</div>`
    : state.gameFilter === 'Favorites' ? `<div class="empty empty-library"><strong>${favoriteCount(consoleInfo.id) ? 'No favorites match this search' : `No ${escapeHtml(consoleInfo.name)} favorites yet`}</strong><p>${favoriteCount(consoleInfo.id) ? 'Try another search, or switch to All games.' : 'Select the heart on a game in All games to add it to this console’s favorites list.'}</p><button type="button" class="button button-ghost" data-action="filter-games" data-value="All">Browse all games</button></div>` : `<div class="empty empty-library"><strong>${state.native && !importedFor(consoleInfo.id).length ? 'No games detected yet' : 'No matching games'}</strong><p>${state.native && !importedFor(consoleInfo.id).length ? 'Add a game folder to get started.' : 'Try another search or switch to All games.'}</p>${state.native && !importedFor(consoleInfo.id).length ? `<button type="button" class="button button-primary" data-action="library-folders" data-id="${consoleInfo.id}">Add games</button>` : ''}</div>`;
  renderGamePreview(consoleInfo,preview);
  hydrateNativeCovers();
  restoreLibraryFocus(focus);
}
function renderConsole() {
  const focus=captureLibraryFocus($('#screen'),'console:'+state.consoleId);
  const consoleInfo=byId[state.consoleId]; normalizeGameSort(consoleInfo.id);
  const yearSorting=hasReleaseYears(consoleInfo.id), imported=importedFor(consoleInfo.id), hasImported=imported.length>0;
  $('#screen').innerHTML=`<div class="console-library"><div class="console-library-main"><button type="button" class="back-link" data-action="home">${glyph('back')} Consoles</button>
    <div class="library-header" style="--tint:${consoleTone(consoleInfo)}"><div class="library-identity"><span class="library-emblem" aria-hidden="true">${deviceArt(consoleInfo.id)}</span><div><h1 class="page-title">${escapeHtml(consoleInfo.name)}</h1><p class="page-subtitle">${gamesFor(consoleInfo.id).length} ${gamesFor(consoleInfo.id).length===1?'game':'games'}</p></div></div>
    <div class="library-header-actions">${!isConfigured(consoleInfo.id)?`<button type="button" class="button button-primary" data-action="configure" data-id="${consoleInfo.id}">Set up emulator</button>`:''}
      <details class="library-menu"><summary aria-label="Library options" title="Library options">···</summary><div class="menu-items">
        ${state.native?`<button data-action="save-data-backup" data-id="${consoleInfo.id}">Back up saves…</button><button data-action="save-data-restore" data-id="${consoleInfo.id}">Restore saves…</button>`:''}
        ${isConfigured(consoleInfo.id)?`<button type="button" data-action="configure" data-id="${consoleInfo.id}">Edit emulator</button>`:''}
        ${state.native&&isConfigured(consoleInfo.id)?`<button type="button" data-action="open-emulator" data-id="${consoleInfo.id}">Open emulator</button>`:''}
        ${state.native?`<button type="button" data-action="library-folders" data-id="${consoleInfo.id}">Manage folders</button>`:''}
        ${state.native?`<button type="button" data-action="launch-details" data-id="${consoleInfo.id}" ${state.launches[consoleInfo.id]?'':'hidden'}>Launch details</button>`:''}
        <button type="button" data-action="${state.native?'open-folder':'preview-folder'}">${state.native?'Open games folder':'Preview game folder'}</button>
        ${!state.native&&hasImported?'<button type="button" data-action="clear-preview">Clear preview</button>':''}
      </div></details></div></div>
    <div id="launch-status"></div>
    <div class="library-tools"><label class="search">${glyph('search')}<input type="search" id="game-search" autocomplete="off" placeholder="Search games" aria-label="Search ${escapeHtml(consoleInfo.name)} games" value="${escapeHtml(state.query)}"></label>
      <div class="segmented" role="group" aria-label="Game filter">${['All','Favorites'].map(v=>`<button type="button" data-action="filter-games" data-value="${v}" class="${state.gameFilter===v?'active':''}" aria-pressed="${state.gameFilter===v}">${v==='All'?'All games':`Favorites (${favoriteCount(consoleInfo.id)})`}</button>`).join('')}</div>
      <select id="sort-games" class="sort-select" aria-label="Sort games" aria-describedby="sort-year-hint"><option value="default" ${state.sort==='default'?'selected':''}>Default</option><option value="az" ${state.sort==='az'?'selected':''}>Name</option><option value="recent" ${state.sort==='recent'?'selected':''} ${yearSorting?'':'disabled'}>Release year</option></select>
      <div class="segmented" role="group" aria-label="Library layout"><button type="button" data-action="view" data-value="grid" aria-label="Grid view" aria-pressed="${state.view==='grid'}" class="${state.view==='grid'?'active':''}">${glyph('grid')}</button><button type="button" data-action="view" data-value="list" aria-label="List view" aria-pressed="${state.view==='list'}" class="${state.view==='list'?'active':''}">${glyph('list')}</button></div></div>
    <p id="sort-year-hint" class="sr-only">${yearSorting?'Titles with unknown years appear last.':'Release-year sorting is unavailable: these titles have no release-year metadata.'}</p><div id="game-results"></div>
    ${consoleInfo.supportNote?`<details class="library-notes"><summary>Format &amp; emulator notes</summary><p class="platform-notice">${escapeHtml(consoleInfo.supportNote)}</p></details>`:''}</div><aside class="game-preview" id="game-preview" aria-label="Selected game preview"></aside></div>`;
  renderGameResults();renderLaunchStatus();restoreLibraryFocus(focus);
}
function renderSettings() {
  const focus=captureLibraryFocus($('#screen'),'settings');
  $('#screen').innerHTML=`<div class="settings-header"><h1 class="page-title">Settings</h1><div class="settings-app"><span>Matrix-Play</span><button type="button" class="button button-ghost" data-action="updates" aria-label="Update Matrix-Play">Check for updates</button></div></div>
    ${state.native?discoverySummary():''}
    ${organizationControls()}
    <details class="library-notes"><summary>Library behavior</summary><label><input type="checkbox" data-pref="autoRefresh" ${libraryPrefs().autoRefresh!==false?'checked':''}> Automatically refresh game libraries every 30 seconds</label><label><input type="checkbox" data-pref="grouping" ${libraryPrefs().grouping?'checked':''}> Group game versions (same console)</label></details>
    <details class="library-notes"><summary>Backup and restore</summary><p>Back up settings, favorites and covers (up to 64 MB). Game files and API keys are excluded. Restore only backups you trust; existing covers are kept, and a safety snapshot is saved before restoring settings.</p><div class="tag-row"><button type="button" class="button button-ghost" data-action="backup-library" ${state.native?'':'disabled'}>Create backup</button><button type="button" class="button button-ghost" data-action="restore-library" ${state.native?'':'disabled'}>Restore backup…</button></div><p id="backup-status" role="status"></p></details>
    <section class="artwork-settings"><h2>Artwork</h2>
      <div class="steamgrid-settings"><h3>SteamGridDB</h3><p>${state.steamgridEnabled?'Enabled for automatic missing covers.':'Add your personal API key to enable SteamGridDB covers for all consoles.'}</p><label class="field" for="steamgrid-key">API key<input type="password" id="steamgrid-key" autocomplete="new-password" placeholder="Paste your SteamGridDB API key" ${state.native?'':'disabled'}></label><div class="tag-row"><button type="button" class="button button-primary" data-action="steamgrid-key-save" ${state.native?'':'disabled'}>Save and enable</button><button type="button" class="button button-ghost" data-action="steamgrid-key-clear" ${state.native?'':'disabled'}>Disable SteamGridDB</button><button type="button" class="button button-ghost" data-action="steamgrid-key-open" ${state.native?'':'disabled'}>Get API key</button></div><p class="setting-hint">Automatic lookups send game titles to SteamGridDB. Unique exact title matches use static portrait covers (flat only). Ambiguous matches can be selected in game details. Local covers take priority; the key stays in your private local settings.</p></div>
      <label><input type="checkbox" id="auto-covers" ${state.artwork.enabled?'checked':''} ${state.native&&!state.artworkSaving?'':'disabled'}> Download missing covers</label>
      <label><input type="checkbox" id="auto-title-covers" ${state.artwork.titleEnabled?'checked':''} ${state.native&&!state.artworkSaving?'':'disabled'}> Match game titles with Libretro</label>
      <p class="setting-hint">Title matching sends game names to Libretro. Local covers take priority.</p>
      <label for="cover-style">Cover style</label><select id="cover-style" ${state.native&&!state.artworkSaving?'':'disabled'}><option value="default" ${state.artwork.style==='default'?'selected':''}>Default</option><option value="3d" ${state.artwork.style==='3d'?'selected':''}>3D</option></select>
      <details class="library-notes"><summary>Artwork details</summary><p>Requests send the game serial, disc ID or filename-based catalog title, console and style to GitHub, GameTDB or Libretro; game files and full paths are not uploaded. Local covers take priority. Libretro and Switch use flat covers.</p><p>PS5 and Switch 2 still need local covers. The six additional libraries also use local covers. Choose cover is available in game details.</p><button type="button" class="button button-ghost" data-action="cover-project">Open TheCoverProject</button></details><p id="cover-scan-status" role="status"></p><details class="library-notes"><summary>Cover download queue</summary><div class="tag-row"><button type="button" class="button button-ghost" data-action="queue-pause">Pause</button><button type="button" class="button button-ghost" data-action="queue-resume">Resume / retry failures</button></div><div id="cover-queue-results"></div></details></section>
    <div class="section-toolbar"><h2 class="section-title">Emulators &amp; folders</h2><span class="status">${configuredCount()} / ${consoles.length}</span></div>
    <div class="settings-list">${consoles.map(c=>`<div class="settings-row"><div class="settings-row__left"><div class="settings-name"><strong>${escapeHtml(c.name)}</strong><small>${escapeHtml(state.configs[c.id]?.emulatorName||c.emulator)}</small></div></div><div class="settings-row__right">${state.native?`<button type="button" class="button button-ghost" data-action="library-folders" data-id="${c.id}" aria-label="Manage ${escapeHtml(c.name)} folders">Folders</button>`:''}<button type="button" class="button button-ghost" data-action="configure" data-id="${c.id}">${isConfigured(c.id)?'Edit':'Set up'}</button></div></div>`).join('')}</div>`;
  renderCoverQueue();
  restoreLibraryFocus(focus);
}
function render() {
  renderNav();
  if (state.page === 'home') renderHome();
  if (state.page === 'console') renderConsole();
  if (state.page === 'settings') renderSettings();
  if (state.page === 'steam' || state.page === 'windows') renderExternal(state.page);
}
function goHome() {
  rememberLibrary();dismissOverlay(); state.page = 'home'; state.consoleId = null; state.query = ''; render(); window.scrollTo(0,0);
}
function goConsole(id) {
  if (!byId[id]) return;
  rememberLibrary();const remembered=libraryPrefs().views?.[id];state.view=remembered?.layout||'list';state.previewGame={id,uid:remembered?.selected||''};
  dismissOverlay(); state.page = 'console'; state.consoleId = id; state.query = ''; state.gameFilter = 'All'; state.sort = remembered?.sort||'default'; render(); window.scrollTo(0,0);
}
function goSettings() {
  rememberLibrary();dismissOverlay(); state.page = 'settings'; state.consoleId = null; render(); window.scrollTo(0,0);
}
function goExternal(page) {
  rememberLibrary();dismissOverlay(); state.page = page; state.consoleId = null;
  state.query = ''; state.gameFilter = 'All'; state.sort = 'default';
  render(); window.scrollTo(0,0);
}
function externalList(page) { return page === 'steam' ? state.steam : state.windows; }
function windowsShortcutSupported(game) { return game?.runner === 'desktop' && /\.desktop$/i.test(game.target || ''); }
function renderExternal(page) {
  const focus = captureLibraryFocus($('#screen'), page);
  const steam = page === 'steam';
  const title = steam ? 'Steam Games' : 'Windows Games';
  const games = externalList(page).filter(g => g.title.toLowerCase().includes(state.query.toLowerCase()));
  $('#screen').innerHTML = `<div class="title-row"><div><h1 class="page-title">${title}</h1><p class="page-subtitle">${externalList(page).length} games</p></div>
      <div class="stats"><div class="stat"><strong>${externalList(page).length}</strong><span>Games</span></div></div></div>
    <div class="library-tools"><label class="search">${glyph('search')}<input type="search" id="external-search" placeholder="Search ${title}" value="${escapeHtml(state.query)}"></label>
    ${!steam ? `<button type="button" class="button button-primary" data-action="add-windows">${glyph('plus')} Add Windows game</button>` : state.native ? '<button type="button" class="button button-ghost" data-action="library-folders" data-id="steam">Manage Steam libraries</button>' : ''}</div>
    <div id="launch-status"></div>
    ${games.length ? `<div class="game-grid">${games.map((game,index) => {
      const cover = gameCover({ id: page, abbrev: steam ? 'STEAM' : 'WINDOWS' }, game, index);
      return `<div class="game-item"><button class="game-card" type="button" data-action="quick-external" data-kind="${page}" data-uid="${escapeHtml(game.uid)}" aria-label="${state.native && (steam || windowsShortcutSupported(game)) ? 'Launch' : 'View'} ${escapeHtml(game.title)}">${cover}<span class="game-title">${escapeHtml(game.title)}</span><span class="game-meta">${steam ? 'Steam · App ' + escapeHtml(game.appId || game.uid) : windowsShortcutSupported(game) ? 'Windows · Desktop shortcut' : 'Windows · Shortcut required'}</span></button><button class="game-info" type="button" data-action="external-game" data-kind="${page}" data-uid="${escapeHtml(game.uid)}" aria-label="Details for ${escapeHtml(game.title)}" title="Game details">${glyph('info')}</button></div>`;
    }).join('')}</div>` : `<div class="empty external-empty"><strong>${state.query ? 'No matching games' : `No ${title.toLowerCase()} detected`}</strong><p>${state.query ? 'Try a different search term.' : steam ? 'Install and sign into Steam, then click Refresh.' : 'Choose Add Windows game to register an existing .desktop shortcut.'}</p></div>`}
    `;
  renderLaunchStatus();
  hydrateNativeCovers();
  restoreLibraryFocus(focus);
}
function openExternalGame(kind, uid) {
  const game = externalList(kind).find(g => g.uid === uid); if (!game) return;
  const supported = kind === 'steam' || windowsShortcutSupported(game);
  state.lastFocus = document.activeElement; state.overlay = 'external'; state.selectedGame = {id:kind,uid};
  $('#overlay-root').innerHTML = `<div class="overlay detail-overlay" data-backdrop="true"><aside class="detail-panel" role="dialog" aria-modal="true" aria-labelledby="detail-title"><button class="close" data-action="close" aria-label="Close details">${glyph('close')}</button>
    <div class="detail-cover">${gameCover({id:kind,abbrev:kind.toUpperCase()},game,0)}</div><div class="eyebrow">${kind.toUpperCase()} LIBRARY</div><h2 id="detail-title">${escapeHtml(game.title)}</h2>
    <p class="detail-sub">${escapeHtml(game.target || `Steam app ID ${game.appId || game.uid}`)}</p><p class="detail-description">${kind === 'steam' ? 'Matrix-Play asks your existing Steam client to start this game.' : supported ? 'Matrix-Play opens your existing desktop shortcut. Any external software it uses is managed outside Matrix-Play.' : 'Direct .exe launching is unavailable. This saved entry and its artwork are retained. Add a working .desktop shortcut, then remove this old entry if you no longer need it. Your installed game files are unchanged.'}</p>
    <button class="button button-primary launch-button" data-action="launch-external" data-kind="${kind}" data-uid="${escapeHtml(uid)}" ${supported ? '' : 'disabled'}>${glyph('play')} ${supported ? 'Play game' : 'Desktop shortcut required'}</button>
    <div class="detail-actions"><button class="button button-ghost" type="button" data-action="pick-cover">${glyph('image')} Choose cover</button></div>
    ${kind === 'windows' ? `<div class="detail-actions"><button class="button button-ghost" data-action="remove-windows" data-uid="${escapeHtml(uid)}">${glyph('trash')} Remove from Matrix-Play</button></div>` : ''}
    <p class="detail-hint">${state.native ? 'Launching uses your installed Steam client or existing desktop shortcut.' : 'Browser preview cannot launch games.'}</p></aside></div>`;
  $('.detail-panel .close').focus();
  hydrateNativeCovers();
}
function openAddWindows() {
  state.lastFocus = document.activeElement; state.overlay = 'windows-add';
  $('#overlay-root').innerHTML = `<div class="overlay" data-backdrop="true"><section class="dialog" role="dialog" aria-modal="true" aria-labelledby="windows-title"><button type="button" class="close" data-action="close" aria-label="Close add game">${glyph('close')}</button>
   <div class="eyebrow">WINDOWS LIBRARY</div><h2 id="windows-title">Add existing game.</h2><p class="muted">Register a .desktop shortcut that already works on Linux. No files are moved or copied.</p>
   <form id="windows-form"><div class="field"><label for="windows-name">Game title</label><input id="windows-name" name="title" required maxlength="150" placeholder="Game title"></div>
   <div class="field"><label for="windows-target">Existing desktop shortcut</label><input id="windows-target" name="target" required placeholder="/home/you/.local/share/applications/My Game.desktop"><small>Only .desktop shortcuts are supported. Direct .exe launching is unavailable. Matrix-Play does not provide or configure a Windows compatibility runtime.</small></div>
   <div class="dialog-actions"><button class="button button-ghost" type="button" data-action="close">Cancel</button><button class="button button-primary" type="submit">Add game</button></div></form></section></div>`;
  $('#windows-name').focus();
}
function dismissOverlay() {
  if (state.overlay === null) return;
  state.overlay = null;
  state.selectedGame = null;
  const shell=$('.app-shell');if (shell) shell.inert=false;
  $('#overlay-root').innerHTML = '';
  if (state.lastFocus?.isConnected) state.lastFocus.focus();
  state.lastFocus = null;
}
function activeLaunchId() { return state.page === 'console' ? state.consoleId : ['steam','windows'].includes(state.page) ? state.page : null; }
function renderLaunchStatus() {
  const target=$('#launch-status');if (!target) return;
  const id=activeLaunchId(), info=state.launches[id], emulator=!!byId[id];
  const detailsButton=$('.library-menu [data-action="launch-details"]');
  if (detailsButton) detailsButton.hidden=!info;
  // Successful launches use the transient toast. Keep actionable failures visible.
  target.innerHTML=info && ['error','needs-setup'].includes(info.phase) ? `<div class="launch-notice ${['error','needs-setup'].includes(info.phase)?'launch-error':''}" role="status"><p>${escapeHtml(info.message)}</p><div class="launch-actions">${emulator ? `<button type="button" class="button button-ghost" data-action="open-emulator" data-id="${id}">Open emulator</button>`:''}${info.log || info.logPath ? `<button type="button" class="button button-ghost" data-action="launch-details" data-id="${id}">Launch details</button>`:''}</div></div>` : '';
}
let launchDetailsRequest = 0;
async function showLaunchDetails(id) {
  const info=state.launches[id];if (!info) return;
  const request=++launchDetailsRequest, generation=state.launchGeneration[id];
  const emulator=!!byId[id], name=byId[id]?.name || (id==='steam' ? 'Steam Games' : 'Windows Games');
  state.lastFocus=document.activeElement;state.overlay='launch-details';
  $('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog launch-dialog" role="dialog" aria-modal="true" aria-labelledby="launch-title"><button class="close" type="button" data-action="close" aria-label="Close launch details">${glyph('close')}</button><div class="eyebrow">${escapeHtml(name)}</div><h2 id="launch-title">${emulator ? 'Emulator' : 'Game'} launch</h2><p class="muted" id="launch-message">${escapeHtml(info.message)}</p><pre class="launch-output">${escapeHtml(info.log || 'No output was captured.')}</pre><p class="muted" id="launch-log-path" ${info.logPath ? '' : 'hidden'}>${info.logPath ? `Saved log: ${escapeHtml(info.logPath)}` : ''}</p>${emulator ? `<div class="dialog-actions"><button type="button" class="button button-primary" data-action="open-emulator" data-id="${id}">Open emulator</button></div>`:''}</section></div>`;
  const dialog=$('.launch-dialog');
  dialog.querySelector('button').focus();
  try {
    const latest=state.native ? await nativeInvoke('get_launch_status',{id}) : null;
    if (!latest || request!==launchDetailsRequest || state.overlay!=='launch-details' || dialog!==$('.launch-dialog') || generation!==state.launchGeneration[id]) return;
    state.launches[id]=latest;
    // Refresh text without replacing focused controls or a newer dialog.
    $('#launch-message').textContent=latest.message || '';
    $('.launch-output').textContent=latest.log || 'No output was captured.';
    $('#launch-log-path').textContent=latest.logPath ? `Saved log: ${latest.logPath}` : '';
    $('#launch-log-path').hidden=!latest.logPath;
  } catch (_) { /* Retain the last available status. */ }
}
async function launchTracked(id,command,args) {
  if (state.launchBusy[id]) return;
  state.launchBusy[id]=true;
  const generation=(state.launchGeneration[id] || 0)+1;state.launchGeneration[id]=generation;
  const overlay=state.overlay, dialog=$('#overlay-root').firstElementChild;
  try {
    const info=await nativeInvoke(command,args);
    state.launches[id]=info || {phase:'started',message:'Game launch requested.'};
    if (['game','external','launch-details'].includes(overlay) && state.overlay===overlay && $('#overlay-root').firstElementChild===dialog) dismissOverlay();
    if (activeLaunchId()===id) renderLaunchStatus();
    if(args.uid&&['launch_console_game','launch_steam_game','launch_windows_game'].includes(command)&&info?.phase!=='error'){const game=(byId[id]?gamesFor(id):id==='steam'?state.steam:state.windows).find(g=>g.uid===args.uid);if(game){state.recent=[{kind:id,uid:args.uid,title:game.title},...state.recent.filter(g=>g.kind!==id||g.uid!==args.uid)].slice(0,20);}}
    toast(state.launches[id].message);
    let attempts=0;
    const poll=async()=>{
      if (state.launchGeneration[id]!==generation) return;
      try {
        const latest=await nativeInvoke('get_launch_status',{id});
        if (!latest || state.launchGeneration[id]!==generation) return;
        const changed=state.launches[id]?.phase!==latest.phase;state.launches[id]=latest;
        if (changed && activeLaunchId()===id) renderLaunchStatus();
        if (['error','exited'].includes(latest.phase)) {if (latest.phase==='error') toast(latest.message);return;}
        // Continue watching later failures too, at a lower cadence after startup.
        setTimeout(poll,++attempts<30 ? 700 : 5000);
      } catch (_) { /* Launch details can still be requested manually. */ }
    };
    if (!['error','exited'].includes(state.launches[id].phase)) setTimeout(poll,700);
  } catch(error) {
    state.launches[id]={phase:byId[id] ? 'needs-setup' : 'error',message:String(error)};
    if (activeLaunchId()===id) renderLaunchStatus();toast(String(error));
  } finally {state.launchBusy[id]=false;}
}
async function launchConsole(id,uid=null) {
  if(uid&&findGame(id,uid)?.offline){toast('Game drive is offline. Reconnect it before launching.');return;}
  if (byId[id]?.libraryOnly) {if (uid) openGame(id,uid);else toast(byId[id].supportNote);return;}
  if (!state.native) {toast('Open the Linux app to launch an emulator.');return;}
  await launchTracked(id,uid ? 'launch_console_game':'open_emulator',uid ? {id,uid}:{id});
}
async function launchExternal(kind,uid) {
  if (!state.native) {toast('Browser preview cannot launch games.');return;}
  await launchTracked(kind,kind==='steam' ? 'launch_steam_game' : 'launch_windows_game',{uid});
}
function discoverySummary() {
  const report=state.discovery, total=report.candidates.length;
  const auto=Object.values(state.configs).filter(p=>p.autoDetected).length;
  return `<section class="discovery-box"><div><h2>Emulator discovery</h2><p>${total} detected installation${total===1?'':'s'} · ${auto} console${auto===1?'':'s'} linked.</p>${report.ambiguous.length ? `<p>Choose between multiple matches for: ${report.ambiguous.map(id=>escapeHtml(byId[id]?.name || id)).join(', ')}.</p>`:''}${report.candidates.some(c=>!c.ready) ? '<p>Some downloaded AppImages need execute permission. Choose one in Set up, then Save emulator.</p>':''}</div><button type="button" class="button button-primary" data-action="find-emulators">Find emulators</button></section>`;
}
function detectedEmulatorOptions(id) {
  return '<option value="">Choose a detected installation…</option>'+state.discovery.candidates.filter(c=>c.consoleIds.includes(id)).map(c=>`<option value="${escapeHtml(c.id)}">${escapeHtml(c.profile.emulatorName)} — ${escapeHtml(c.profile.emulatorPath || c.source)}${c.ready?'':' (allow execution when saving)'}</option>`).join('');
}
async function loadDetectedEmulators() {
  const owner=$('#config-form'), select=$('#detected-emulator'), button=$('#refresh-detected'), help=$('#detected-help');
  if (!state.native || !owner || !select || !button || !help) return;
  const request=owner.discoveryRequest=(owner.discoveryRequest || 0)+1;
  const current=()=>owner===$('#config-form') && request===owner.discoveryRequest;
  select.disabled=true;button.disabled=true;help.textContent='Checking emulator files…';
  try {
    const report=await nativeInvoke('discover_emulators');
    if (!current()) return;
    const selected=select.value;
    state.discovery={...state.discovery,candidates:report.candidates,notices:report.notices};
    select.innerHTML=detectedEmulatorOptions(owner.dataset.id);
    select.value=report.candidates.some(c=>c.id===selected && c.consoleIds.includes(owner.dataset.id)) ? selected : '';
    const count=report.candidates.filter(c=>c.consoleIds.includes(owner.dataset.id)).length;
    help.textContent=count ? 'Current installations checked. Choosing an entry fills the setup fields. Save to use it for this console.' : 'No recognized installations found. Use Browse to choose your emulator file.';
    // Never overwrite the path, arguments or other unsaved fields on refresh.
  } catch(error) {
    if (current()) help.textContent='Could not refresh detected installations. Use Browse or try Refresh detected again. '+String(error);
  } finally { if (current()) {select.disabled=false;button.disabled=false;} }
}
function chooseDetectedEmulator() {
  const candidate=state.discovery.candidates.find(c=>c.id===$('#detected-emulator')?.value);if (!candidate) return;
  const p=candidate.profile;
  $('#emulator-launcher').value=p.launcher || 'executable';$('#emulator-path').value=p.emulatorPath || '';$('#emulator-name').value=p.emulatorName;
  $('#launch-args').value=(p.launchArgs || ['{game}']).join('\n');
  $('#flatpak-app').value='';$('#flatpak-app').dataset.saved=p.flatpakId ? p.flatpakId+'|'+p.flatpakInstallation:'';
  $('#runtime-fonts').checked=p.flatpakRuntimeFonts ?? p.flatpakId==='net.pcsx2.PCSX2';
  $('[name=extractAppimage]').checked=Boolean(p.extractAppimage);updateLauncherFields();
}
async function findEmulators(button) {
  if (!state.native) {toast('Automatic discovery is available in the Linux app.');return;}
  button.disabled=true;
  try {
    if (!await bootstrapNative()) return;
    // This load may supersede initialization, so it must also populate games.
    await refreshNative();
    toast(`${state.discovery.linked.length} console profiles linked; ${state.discovery.candidates.length} emulator installations detected.`);
  } catch(error) {toast(String(error));}finally {button.disabled=false;}
}

function defaultLaunchArgs(id) {
  return ['gamecube','wii'].includes(id) ? ['-b','-e','{game}'] : id === 'wiiu' ? ['-g','{game}'] : id === 'ps1' ? ['-batch','{game}'] : ['{game}'];
}
function updateLauncherFields() {
  const launcher=$('#emulator-launcher')?.value; if (!launcher) return;
  $('#file-fields').hidden=launcher==='flatpak';
  $('#flatpak-fields').hidden=launcher!=='flatpak';
  $('#appimage-options').hidden=launcher!=='appimage';
  $('#emulator-path').required=launcher!=='flatpak';
  $('#flatpak-app').required=launcher==='flatpak';
  $('#emulator-file-label').textContent=launcher==='appimage' ? 'AppImage file' : 'Emulator executable';
  $('#emulator-file-hint').textContent=launcher==='appimage' ? 'Choose your downloaded .AppImage wherever you keep it. Matrix-Play leaves it in place.' : 'Choose an installed emulator file, for example one in /usr/bin.';
  if (launcher==='flatpak') void loadFlatpakChoices();
}
let flatpakRequest = 0;
const FLATPAK_LOADING_SAVE_MESSAGE='Flatpak apps are still loading. Wait for the list to finish, then save again.';
async function loadFlatpakChoices() {
  const select=$('#flatpak-app'); if (!select) return;
  if (!state.native) { $('#flatpak-help').textContent='Installed apps are available in the Linux application.';return; }
  const request=++flatpakRequest;
  const current=()=>request===flatpakRequest && select===$('#flatpak-app');
  const previous=select.value || select.dataset.saved || '';
  select.disabled=true;$('#flatpak-help').textContent='Reading installed Flatpak apps…';
  try {
    const apps=await nativeInvoke('list_flatpaks');
    if (!current()) return;
    select.innerHTML='<option value="">Choose an installed app…</option>'+apps.map(app=>`<option value="${escapeHtml(app.appId+'|'+app.installation)}" data-name="${escapeHtml(app.name)}">${escapeHtml(app.name)} — ${escapeHtml(app.appId)} (${escapeHtml(app.installation)})</option>`).join('');
    select.value=previous;
    // Seed the initial choice once. Keep it for failed-load retries, but never
    // restore it over a later explicit blank choice on reload.
    delete select.dataset.saved;
    $('#flatpak-help').textContent=apps.length ? 'Choose your installed emulator. Matrix-Play gives it access to the selected game library, with scoped save access for supported emulators.' : 'No Flatpak apps found. Install an emulator through Software, then click Reload apps.';
  } catch(error) { if (current()) $('#flatpak-help').textContent=String(error); }
  finally {
    if (current()) {
      select.disabled=false;
      const status=$('#config-save-status');
      if (status?.textContent===FLATPAK_LOADING_SAVE_MESSAGE) status.textContent='';
    }
  }
}
function openConfigure(id) {
  const consoleInfo=byId[id];if (!consoleInfo) return;
  if (consoleInfo.libraryOnly) {toast(consoleInfo.supportNote);return;}
  state.lastFocus=document.activeElement;state.overlay='configure';state.selectedGame=null;
  const current=state.configs[id] || {};
  const launcher=current.launcher==='flatpak' ? 'flatpak' : current.launcher==='appimage' || /\.appimage$/i.test(current.emulatorPath || '') || !current.emulatorPath ? 'appimage' : 'executable';
  $('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog emulator-dialog" role="dialog" aria-modal="true" aria-labelledby="config-title"><button type="button" class="close" data-action="close" aria-label="Close dialog">${glyph('close')}</button><div class="eyebrow">EMULATOR SETUP</div><h2 id="config-title">${escapeHtml(consoleInfo.name)}</h2><p class="muted">${consoleInfo.customEmulator ? 'Link your own Linux emulator. This is experimental custom launch support, not a game-compatibility guarantee. Open emulator starts the selected program without arguments; a command-line core may not show a setup window.' : 'Link an emulator you already installed or downloaded. Game cards start recognized emulators fullscreen; Open emulator shows setup menus.'}</p>
    <form id="config-form" data-id="${id}">${state.native ? `<div class="field"><label for="detected-emulator">Detected emulators for ${escapeHtml(consoleInfo.name)}</label><select id="detected-emulator">${detectedEmulatorOptions(id)}</select><small id="detected-help" role="status">Choosing an entry fills the setup fields. Save to use it for this console.</small><button type="button" id="refresh-detected" class="button button-ghost" data-action="refresh-detected">Refresh detected</button></div>`:''}<div class="field"><label for="emulator-launcher">How is your emulator installed?</label><select id="emulator-launcher" name="launcher">${[['appimage','AppImage file'],['flatpak','Flatpak app'],['executable','Other executable']].map(([value,label])=>`<option value="${value}" ${launcher===value?'selected':''}>${label}</option>`).join('')}</select></div>
    <div id="file-fields"><div class="field"><label id="emulator-file-label" for="emulator-path">AppImage file</label><div class="file-choice"><input id="emulator-path" name="emulatorPath" placeholder="Choose a file or paste its full path" value="${escapeHtml(current.emulatorPath || '')}"><button type="button" class="button button-ghost" data-action="browse-emulator">Browse…</button></div><small id="emulator-file-hint"></small></div><div id="appimage-options" class="field"><label class="check-row"><input type="checkbox" id="allow-execute" name="allowExecute" checked> Allow this AppImage to run</label><small>Adds execute permission to the chosen AppImage if needed. Saving does not run it.</small><label class="check-row"><input type="checkbox" name="extractAppimage" ${current.extractAppimage?'checked':''}> Run without FUSE (extraction mode)</label><small>Use this if your AppImage reports that FUSE is unavailable.</small></div></div>
    <div id="flatpak-fields" hidden><div class="field"><label for="flatpak-app">Installed Flatpak emulator</label><select id="flatpak-app" name="flatpakApp" data-saved="${escapeHtml(current.flatpakId ? current.flatpakId+'|'+current.flatpakInstallation : '')}"><option value="">Choose an installed app…</option></select><small id="flatpak-help"></small></div><button type="button" class="button button-ghost" data-action="reload-flatpaks">Reload apps</button><label class="check-row runtime-font-choice"><input type="checkbox" id="runtime-fonts" name="flatpakRuntimeFonts" ${(current.flatpakRuntimeFonts ?? current.flatpakId==='net.pcsx2.PCSX2')?'checked':''}> Use compatible runtime fonts</label><small>Recommended for PCSX2. Avoids crashes from host font configuration; applies only to emulator launches.</small><p class="muted">Flatpak apps are installed through Software. Downloaded .flatpakref files are not executable files to browse for.</p></div>
    <div class="field"><label for="emulator-name">Emulator name</label><input id="emulator-name" name="emulatorName" maxlength="120" value="${escapeHtml(current.emulatorName || consoleInfo.emulator)}" required></div>
    <details class="advanced-setup" ${consoleInfo.customEmulator ? 'open' : ''}><summary>Advanced launch options</summary><div class="field"><label for="launch-args">Launch arguments (one per line)</label><textarea id="launch-args" name="launchArgs" rows="3">${escapeHtml((current.launchArgs?.length ? current.launchArgs : defaultLaunchArgs(id)).join('\n'))}</textarea><small>{game} is the game file. ${consoleInfo.customEmulator ? '{gameDir} is its containing folder. Put each placeholder on its own line. These arguments are used only for game launches; no fullscreen options are added.' : 'Defaults vary by console; some emulators require different options.'} Each line is passed as one argument, without a shell.</small>${consoleInfo.customEmulator ? `<p class="muted custom-launch-help">${id==='xbox360' ? 'Choose a Linux-compatible Xbox 360 emulator. Put {game} on its own line; add only the arguments documented for your installed build.' : id==='ps5' ? 'KytyPS5 core example: choose kyty_emulator, then enter --game and {gameDir} on separate lines for an extracted eboot.bin title. For ELF or ZAR, use --game and {game}. PKG/ISO collection files are not KytyPS5 boot targets.' : 'Lichii/Eglantine example: enter -f, -g and {game} on three separate lines. Check the documentation for your installed build; original Switch emulator settings are not reused.'}</p>` : ''}</div></details>
    <div class="field"><label for="library-path">Game library folder</label><input id="library-path" name="folderPath" value="${escapeHtml(state.native ? state.folders[id] || '' : current.folderPath || '')}" ${state.native?'readonly':'required'}><small>Place this console’s games here. Your emulator can stay wherever it is installed.</small></div><p id="config-error" class="config-error" role="alert"></p><p id="config-save-status" class="muted" role="status"></p><div class="dialog-actions"><button class="button button-ghost" type="button" data-action="close">Cancel</button><button class="button button-primary" type="submit">Save emulator</button><button class="button button-ghost" type="submit" data-open-after-save="true">Save and open emulator</button></div></form></section></div>`;
  updateLauncherFields();$('#emulator-launcher').focus();void loadDetectedEmulators();
}
let libraryFolderBusy = false;
function showLibraryFolderRows(owner, folders) {
  if ($('#library-folder-dialog') !== owner) return;
  const list=owner.querySelector('.library-folder-list');
  list.innerHTML=folders.length ? folders.map(folder=>`<li><div><strong>${escapeHtml(folder.path)}</strong><small>${folder.managed ? 'Managed folder' : owner.dataset.id==='steam' ? `${escapeHtml(folder.launcher)} Steam` : 'External folder'}${folder.available ? '' : ' · Unavailable or disconnected'}</small></div>${folder.managed ? '' : `<button type="button" class="button button-ghost" data-action="remove-library-folder" data-path="${escapeHtml(folder.path)}" aria-label="Remove folder ${escapeHtml(folder.path)}">Remove</button>`}</li>`).join('') : '<li>No custom libraries registered. Standard Steam installations are detected automatically.</li>';
}
async function openLibraryFolders(id) {
  if (!state.native || (id!=='steam' && !byId[id])) return;
  const steam=id==='steam';
  state.lastFocus=document.activeElement;state.overlay='library-folders';
  $('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog library-folder-dialog" id="library-folder-dialog" data-id="${escapeHtml(id)}" role="dialog" aria-modal="true" aria-labelledby="library-folder-title"><button type="button" class="close" data-action="close" aria-label="Close library folders">${glyph('close')}</button><div class="eyebrow">YOUR FILES STAY IN PLACE</div><h2 id="library-folder-title">${escapeHtml(steam ? 'Steam libraries' : byId[id].name+' folders')}</h2><p>${steam ? 'Standard native, Flatpak and Snap installations are detected automatically. Add another Steam folder or steamapps directory and choose the installation that owns it.' : 'Add a folder containing this console’s games. The managed folder remains available. Removing a folder only removes its registration; games, saves and artwork are not deleted.'}</p><ul class="library-folder-list" aria-label="Registered library folders"><li>Loading folders…</li></ul><div class="field"><label for="external-library-path">Existing library folder</label><div class="file-choice"><input id="external-library-path" placeholder="/media/drive/Games"><button type="button" class="button button-ghost" data-action="browse-library-folder">Browse…</button></div></div>${steam ? '<div class="field"><label for="steam-library-launcher">Owning Steam installation</label><select id="steam-library-launcher"><option value="native">Native Steam</option><option value="flatpak">Flatpak Steam</option><option value="snap">Snap Steam</option></select></div>' : ''}<p class="config-error library-folder-error" role="alert"></p><div class="dialog-actions"><button type="button" class="button button-ghost" data-action="close">Done</button><button type="button" class="button button-primary" data-action="add-library-folder">Add folder</button></div></section></div>`;
  const owner=$('#library-folder-dialog');owner.querySelector('input').focus();
  const request=owner.folderRequest=1;
  try {const folders=await nativeInvoke(steam ? 'list_steam_libraries' : 'list_library_folders',steam ? {} : {id});if (owner.folderRequest===request) showLibraryFolderRows(owner,folders);}
  catch(error) {if ($('#library-folder-dialog')===owner && owner.folderRequest===request) owner.querySelector('.library-folder-error').textContent=String(error);}
}
async function browseLibraryFolder(button) {
  const owner=$('#library-folder-dialog');if (!owner || button.disabled) return;
  const input=owner.querySelector('input'), draft=input.value;button.disabled=true;
  try {const path=await nativeInvoke('browse_library_folder');if (path && $('#library-folder-dialog')===owner && input.value===draft) {input.value=path;input.focus();}}
  catch(error) {if ($('#library-folder-dialog')===owner) owner.querySelector('.library-folder-error').textContent=String(error);}
  finally {button.disabled=false;}
}
async function changeLibraryFolder(button) {
  const owner=$('#library-folder-dialog');if (!owner || libraryFolderBusy) return;
  const id=owner.dataset.id, steam=id==='steam', remove=button.dataset.action==='remove-library-folder';
  const input=owner.querySelector('input'), draft=input.value, path=remove ? button.dataset.path : draft.trim();
  const errorNode=owner.querySelector('.library-folder-error');
  if (!path) {errorNode.textContent='Choose an existing folder first.';input.focus();return;}
  libraryFolderBusy=true;
  // Disabled buttons can lose focus before the reply arrives. Move it now so
  // replacing rows cannot strand it, without overriding later user navigation.
  if (remove && document.activeElement===button) input.focus();
  button.disabled=true;errorNode.textContent='';
  ++owner.folderRequest; // An older initial list must not roll back this mutation.
  try {
    const args=steam ? {path,...(remove ? {} : {launcher:owner.querySelector('select').value})} : {id,path};
    const command=steam ? (remove ? 'remove_steam_library' : 'add_steam_library') : (remove ? 'remove_library_folder' : 'add_library_folder');
    const folders=await nativeInvoke(command,args);
    if ($('#library-folder-dialog')===owner) {
      showLibraryFolderRows(owner,folders);
      if (!remove && input.value===draft) {input.value='';input.focus();}
    }
    await refreshNative();toast(remove ? 'Folder registration removed. Files and saves are unchanged.' : 'Library folder added.');
  } catch(error) {
    if ($('#library-folder-dialog')!==owner) {toast(String(error));return;}
    errorNode.textContent=String(error);
    // The initial read was invalidated when this mutation began. Recover the
    // current rows without clearing the failure or a path edited meanwhile.
    const request=++owner.folderRequest;
    try {
      const folders=await nativeInvoke(steam ? 'list_steam_libraries' : 'list_library_folders',steam ? {} : {id});
      if ($('#library-folder-dialog')===owner && owner.folderRequest===request) {
        showLibraryFolderRows(owner,folders);
      }
    } catch (_) {
      if ($('#library-folder-dialog')===owner && owner.folderRequest===request) owner.querySelector('.library-folder-list').innerHTML='<li>Folder list unavailable. Close and reopen this dialog to retry.</li>';
    }
  }
  finally {libraryFolderBusy=false;button.disabled=false;}
}

function openGame(id, uid) {
  const consoleInfo = byId[id], game = consoleInfo && findGame(id, uid); if (!game) return;
  if (state.overlay !== 'game') state.lastFocus = document.activeElement;
  state.overlay = 'game'; state.selectedGame = { id, uid };
  const liked = state.favorites.has(favoriteKey(id,uid));
  const index = gamesFor(id).findIndex(g=>g.uid===uid);
  $('#overlay-root').innerHTML = `<div class="overlay detail-overlay" data-backdrop="true"><aside class="detail-panel" role="dialog" aria-modal="true" aria-labelledby="detail-title">
    <button type="button" class="close" data-action="close" aria-label="Close game details">${glyph('close')}</button>
    <div class="detail-cover">${gameCover(consoleInfo,game,index)}</div><div class="eyebrow">GAME DETAILS · ${state.native ? consoleInfo.libraryOnly ? 'COLLECTION FILE' : 'LOCAL GAME' : game.demo ? 'DEMO TITLE' : 'FOLDER PREVIEW'}</div><h2 id="detail-title">${escapeHtml(game.title)}</h2>
    ${metadataControls(id,game)}
    <p class="detail-sub">${escapeHtml(consoleInfo.name)}${game.year ? ` · ${game.year}` : ''}</p>
    <div class="tag-row"><span class="tag">${escapeHtml(consoleInfo.abbrev)}</span><span class="tag">${escapeHtml(state.configs[id]?.emulatorName || consoleInfo.emulator)}</span><span class="tag">${consoleInfo.libraryOnly ? 'No launch adapter' : isConfigured(id) ? 'Profile saved' : 'Not connected'}</span></div>
    <details class="library-notes"><summary>Game information</summary><p class="detail-description">${consoleInfo.libraryOnly || consoleInfo.customEmulator ? escapeHtml(consoleInfo.supportNote) : state.native ? 'The Play button starts this file fullscreen with the recognized emulator’s game-only launch options. Custom launchers need their own fullscreen arguments.' : `The native application will launch this game using ${escapeHtml(state.configs[id]?.emulatorName || consoleInfo.emulator)}. This browser preview cannot execute games.`}</p></details>
    ${consoleInfo.libraryOnly ? '' : `<button type="button" class="button button-primary launch-button" data-action="play-console">${glyph('play')} Play game</button>`}
    <div class="detail-actions"><button type="button" class="button button-ghost" data-action="pick-cover">${glyph('image')} Choose cover</button><button type="button" class="button button-ghost" data-action="favorite" data-id="${id}" data-uid="${escapeHtml(uid)}" aria-pressed="${liked}">${glyph('heart')} ${liked ? 'Unfavorite' : 'Favorite'}</button></div>
    ${state.native ? `<button type="button" class="button button-ghost" data-action="game-options">Launch settings</button>` : ''}
    ${state.native ? `<section class="cover-tools"><h3>Cover artwork</h3><p class="cover-result" role="status">${escapeHtml(coverMessage(id,uid))}</p>${state.steamgridEnabled ? '<p>SteamGridDB is enabled. Find on SteamGridDB lets you select the game and cover.</p>' : AUTO_COVER_KINDS.includes(id) ? `<label for="cover-serial">${TITLE_COVER_KINDS.includes(id) ? 'Confirm exact catalog title' : id==='switch' ? 'Confirm Switch product code and region' : ['gamecube','wii'].includes(id) ? 'Confirm disc ID' : 'Confirm game serial'}</label><input id="cover-serial" placeholder="${TITLE_COVER_KINDS.includes(id) ? 'Exact title (USA) (Rev 1)' : id==='switch' ? 'AAACA-US' : id==='gamecube' ? 'GMSE01' : id==='wii' ? 'RMGE01' : 'SLUS-20312'}" value="${escapeHtml(state.coverStatus.get(artKey(id,uid))?.serial || '')}" maxlength="${TITLE_COVER_KINDS.includes(id) ? 240 : 16}"><small>${TITLE_COVER_KINDS.includes(id) ? 'Match the Libretro title exactly, including region, language and edition. No fuzzy matching or region guessing. Only flat covers are available.' : id==='switch' ? 'Use the five-character original Switch product code and artwork region, for example AAACA-US or HAC-P-AAACA-USA. This is not the 16-digit title ID. EUR selects English artwork (EN). Only flat covers are available; no Switch 2 fallback.' : 'Use the identity for this exact game, region and disc. Compressed images may need an ID entered here.'}</small><button type="button" class="button button-ghost" data-action="download-cover">Find exact cover</button>` : id==='ps5' ? '<p>No verified automatic provider for online PS5 downloads. Extracted eboot games in their own folders can use an existing sce_sys/icon0.png. Imported and sidecar covers take priority.</p>' : '<p>No verified automatic provider for this console yet. Choose a local image; existing covers are kept.</p>'}<button type="button" class="button button-primary" data-action="steamgrid-open">Find on SteamGridDB</button><button type="button" class="button button-ghost" data-action="cover-project">Open TheCoverProject</button></section>` : ''}
    <p class="detail-hint">${consoleInfo.libraryOnly ? 'Collection files can have local covers and favorites. Launching is unavailable for this system.' : state.native ? 'The configured emulator is required to launch this title. Covers are saved in this console’s Covers folder. To add an image there manually, match the game filename without its extension.' : 'Preview only. Play does not execute an emulator. Custom artwork is saved in this browser.'}</p>
  </aside></div>`;
  $('.detail-panel .close').focus();
  hydrateNativeCovers();
}
function openSteamGrid() {
  if (!state.native || !state.selectedGame) return;
  const selected={...state.selectedGame};
  const title=gameMetadata(selected.id,selected.uid).coverTitle||gamesFor(selected.id).find(g=>g.uid===selected.uid)?.originalTitle||gamesFor(selected.id).find(g=>g.uid===selected.uid)?.title || '';
  state.overlay='steamgrid';
  $('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog steamgrid-dialog" role="dialog" aria-modal="true" aria-labelledby="steamgrid-title"><button type="button" class="close" data-action="close" aria-label="Close SteamGridDB">${glyph('close')}</button><div class="eyebrow">COVER ART</div><h2 id="steamgrid-title">SteamGridDB</h2><p class="muted">Select the correct game, then choose a cover to replace its artwork. Requires an API key in Settings.</p><label class="field">Game title<input id="steamgrid-query" value="${escapeHtml(title)}" maxlength="240"></label><button type="button" class="button button-primary" data-action="steamgrid-search">Search</button><p id="steamgrid-message" role="status"></p><div id="steamgrid-results"></div></section></div>`;
  $('.steamgrid-dialog')._selection=selected;
  $('#steamgrid-query').focus();
}
async function steamGridAction(button) {
  const owner=$('.steamgrid-dialog');if(!owner || owner.dataset.busy) return;
  const selected=owner._selection;owner.dataset.busy='true';button.disabled=true;
  const current=()=>$('.steamgrid-dialog')===owner;
  try {
    $('#steamgrid-message').textContent='Loading SteamGridDB…';
    if(button.dataset.action==='steamgrid-search') {
      const games=await nativeInvoke('steamgrid_search',{query:$('#steamgrid-query').value});if(!current())return;
      $('#steamgrid-results').innerHTML=games.slice(0,30).map(g=>`<button type="button" class="button button-ghost" data-action="steamgrid-game" data-game-id="${Number(g.id)}">${escapeHtml(g.name)}</button>`).join('');
      $('#steamgrid-message').textContent=games.length?'Choose the matching title and edition.':'No matching games found.';
    } else if(button.dataset.action==='steamgrid-game') {
      const gameId=Number(button.dataset.gameId);const grids=await nativeInvoke('steamgrid_grids',{gameId});if(!current())return;
      $('#steamgrid-results').innerHTML=grids.map(g=>`<button type="button" class="steamgrid-cover" data-action="steamgrid-cover" data-game-id="${gameId}" data-grid-id="${Number(g.id)}" aria-label="Use cover ${Number(g.id)}"><img src="${escapeHtml(g.url)}" alt="Cover ${Number(g.id)}" loading="lazy"><span>Use this cover</span></button>`).join('');
      $('#steamgrid-message').textContent=grids.length?'Choose a cover. This replaces the current cover.':'No static portrait covers found.';
    } else {
      await nativeInvoke('steamgrid_save',{kind:selected.id,uid:selected.uid,gameId:Number(button.dataset.gameId),gridId:Number(button.dataset.gridId)});
      const key=artKey(selected.id,selected.uid);state.art.delete(key);state.missingArt.delete(key);state.artRevision++;clearCoverCache();
      if(current()){dismissOverlay();render();}toast('SteamGridDB cover saved.');
    }
  } catch(e){if(current())$('#steamgrid-message').textContent=String(e);}finally{delete owner.dataset.busy;button.disabled=false;}
}
let toastTimer;
function toast(message) {
  $('#toast-root').innerHTML = `<div class="toast">${escapeHtml(message)}</div>`;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { $('#toast-root').innerHTML = ''; }, 4800);
}
function cleanTitle(filename) {
  return filename.replace(/\.[^.]+$/, '').replace(/[._]+/g,' ').replace(/\s*\[[^\]]+\]/g,'').replace(/\s*\([^)]*\)/g,'').replace(/\s+/g,' ').trim() || filename;
}
function switch2ExtractedTitle(path) {
  const parts=path.split('/');
  if (parts.length<3 || parts.at(-1).toLowerCase()!=='main' || parts.at(-2).toLowerCase()!=='exefs') return null;
  const name=parts.at(-3), storage=parts.at(-4);
  if (storage?.toLowerCase()==='updates') {
    const title=parts.at(-5);
    return title ? cleanTitle(title+'.bin')+' — Update '+name : null;
  }
  if (name.toLowerCase()==='base') return storage ? cleanTitle(storage+'.bin')+' — Base' : null;
  return cleanTitle(name+'.bin');
}
function importFolder(files) {
  const id = state.consoleId;
  if (!id) return;
  const allowed = new Set(EXTS[id] || []);
  const matched = Array.from(files).filter(file => {
    const extension=file.name.split('.').at(-1).toLowerCase();
    const parts=(file.webkitRelativePath || file.name).split('/');
    if (id==='switch2' && parts.slice(1,-1).some(part=>part.startsWith('.') || part.toLowerCase()==='romfs')) return false;
    if (id==='switch2' && file.name.startsWith('.')) return false;
    return (allowed.has(extension) && (id!=='ps5' || extension!=='bin' || /^eboot\.bin$/i.test(file.name)))
      || (id==='switch2' && switch2ExtractedTitle(file.webkitRelativePath || file.name)!==null)
      || (id==='xbox360' && /^default\.xex$/i.test(file.name));
  });
  const seen = new Set();
  const imported = matched.slice(0,2000).map((file,index) => {
    const path = file.webkitRelativePath || file.name;
    if (seen.has(path)) return null;
    seen.add(path);
    const folderTitle=(id==='ps5' && /^eboot\.bin$/i.test(file.name) || id==='xbox360' && /^default\.xex$/i.test(file.name)) ? path.split('/').at(-2) : null;
    return { uid: `file:${path}`, title: (id==='switch2' ? switch2ExtractedTitle(path) : null) || cleanTitle(folderTitle ? folderTitle+'.bin' : file.name), relativePath: path, year: null, demo: false,
      a: ['#5877b7','#a16570','#6e94b2','#8a7bad'][index % 4],
      b: ['#213459','#352b49','#20394f','#2e2c4a'][index % 4] };
  }).filter(Boolean);
  if (!imported.length) {
    toast(`No recognized ${byId[id].name} game filenames were found. Supported extensions: ${[...allowed].map(ext => id==='ps5' && ext==='bin' ? 'eboot.bin only' : '.'+ext).join(', ')}${id==='switch2' ? ', extracted exefs/main' : ''}.`);
    return;
  }
  state.imported[id] = imported; state.query = ''; state.gameFilter = 'All';
  const persisted = savePrefs({type:'import',id,value:imported}); render();
  if (persisted) toast(`Previewed ${imported.length} ${byId[id].name} filename${imported.length === 1 ? '' : 's'}. No game files were copied or uploaded.`);
}
function refreshCoverUI(selected) {
  // Update matching images in place. A pending import must never reopen details
  // or replace a different library/dialog after the user has navigated away.
  const key = artKey(selected.id, selected.uid);
  const uri = state.art.get(key);
  if (uri) updateVisibleCover(key, uri);
}

const coverSelections=new Map(), coverWrites=new Map();
// Successful responses may arrive out of order, even after their disk commits.
const emulatorSaveRequests=new Map(), emulatorSavedRequests=new Map();
async function reconcileSavedCover(selected,key,selection) {
  // A newer file read may fail while an earlier write is still in progress.
  // Wait for that write before reading the actual retained cover from disk.
  await (coverWrites.get(key)||Promise.resolve()).catch(()=>{});
  if (coverSelections.get(key)!==selection) return;
  const revision=++state.artRevision;
  evictCover(key);state.missingArt.delete(key);
  document.querySelectorAll('.game-cover[data-art-key]').forEach(node=>{
    if (node.dataset.artKey===key) node.querySelector('img')?.remove();
  });
  try {
    const uri=await nativeInvoke('get_cover',{kind:selected.id,uid:selected.uid});
    if (coverSelections.get(key)!==selection || revision!==state.artRevision) return;
    if (uri) {cacheCover(key,uri);refreshCoverUI(selected);}
    else state.missingArt.add(key);
  } catch (_) { /* Leave uncached so the next visible-card read can retry. */ }
  finally {
    if (coverSelections.get(key)===selection && revision!==state.artRevision) {
      // Unrelated cover changes can invalidate this recovery after its image
      // was removed. Retry visible cards without duplicating a newer read.
      document.querySelectorAll('.game-cover[data-art-key]').forEach(node=>{
        if (node.dataset.artKey===key) void requestNativeCover(node);
      });
    }
  }
}
async function loadCover(file) {
  const selected = state.selectedGame;
  if (!selected || !file) return;
  const sizeLimit = state.native ? 4 * 1024 * 1024 : 8 * 1024 * 1024;
  if (!['image/png','image/jpeg','image/webp'].includes(file.type) || file.size > sizeLimit) {
    toast(`Choose a PNG, JPG, or WebP cover under ${sizeLimit / (1024 * 1024)} MB.`); return;
  }
  const key = artKey(selected.id, selected.uid), selection={};
  coverSelections.set(key,selection);
  if (state.native) {
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      if (coverSelections.get(key)!==selection) return;
      // Reads can finish out of order. Once a write has started it cannot be
      // cancelled, so serialize writes per game and skip superseded selections.
      const writing=(coverWrites.get(key)||Promise.resolve()).catch(()=>{}).then(async()=>{
        if (coverSelections.get(key)!==selection) return;
        await nativeInvoke('save_cover', { kind: selected.id, uid: selected.uid, bytes });
        if (coverSelections.get(key)!==selection) return;
        state.artRevision++; // In-flight native reads must not replace this local cover.
        cacheCover(key, URL.createObjectURL(file));
        state.missingArt.delete(key);
        refreshCoverUI(selected);
        toast('Cover saved to Matrix-Play’s Linux artwork folder.');
      });
      coverWrites.set(key,writing);
      try {await writing;} finally {if (coverWrites.get(key)===writing) coverWrites.delete(key);}
    } catch (error) {
      if (coverSelections.get(key)===selection) {
        toast(`Could not save cover: ${error}`);
        await reconcileSavedCover(selected,key,selection);
      }
    }
    finally {if (coverSelections.get(key)===selection) coverSelections.delete(key);}
    return;
  }
  cacheCover(key, URL.createObjectURL(file));
  const saved = await storeArt(key,file);
  if (coverSelections.get(key)!==selection) return;
  coverSelections.delete(key);
  refreshCoverUI(selected);
  toast(saved ? 'Custom cover saved to this browser.' : 'Custom cover updated for this session; browser storage was unavailable.');
}
function coverMessage(kind,uid) {
  const status = state.coverStatus.get(artKey(kind,uid));
  if (!status && TITLE_COVER_KINDS.includes(kind) && !state.artwork.titleEnabled) return 'Enable title-based covers in Settings, or confirm an exact title here.';
  if (!status) return AUTO_COVER_KINDS.includes(kind) ? (state.artwork.enabled ? 'Waiting for automatic artwork scan.' : 'Automatic artwork is off.') : kind==='ps5' ? 'Local covers and extracted PS5 icons are supported. Online downloads are unavailable.' : 'Choose a local cover for this console.';
  return [status.message, status.serial, status.source ? `Identity: ${status.source}` : '', status.url ? (TITLE_COVER_KINDS.includes(kind) ? 'Provider: Libretro' : ['gamecube','wii','switch'].includes(kind) ? 'Provider: GameTDB' : 'Provider: xlenore on GitHub') : ''].filter(Boolean).join(' · ');
}
function updateCoverStatus(kind,uid) {
  if (state.selectedGame?.id === kind && state.selectedGame?.uid === uid) {
    const node = document.querySelector('.cover-result');
    if (node) node.textContent = coverMessage(kind,uid);
  }
}
async function fetchArtwork(kind,uid,serial=null,generation=state.coverGeneration) {
  const outcome = await nativeInvoke('download_cover',{kind,uid,serial});
  if (generation !== state.coverGeneration) return;
  const key=artKey(kind,uid);
  state.coverStatus.set(key,outcome);
  updateCoverStatus(kind,uid);
  if (['downloaded','cached','local'].includes(outcome?.status)) {
    state.artRevision++;state.missingArt.delete(key); evictCover(key);
    // Load only currently visible cards; other games use the persistent native cache later.
    document.querySelectorAll('.game-cover[data-art-key]').forEach(node => {
      if (node.dataset.artKey === key) void requestNativeCover(node);
    });
  }
  return outcome;
}
function renderCoverQueue() {
 const node=$('#cover-queue-results');if(node)node.innerHTML=state.coverQueue.items.filter(i=>i.status!=='pending').slice(-30).map(i=>`<p>${escapeHtml(byId[i.kind]?.name||i.kind)} · ${escapeHtml(i.title)} — ${escapeHtml(i.status)}${i.error?': '+escapeHtml(i.error):''}</p>`).join('');
}
async function scanArtwork(resume=false) {
 const generation=++state.coverGeneration;
 if(!state.native||!state.artwork.enabled||state.coverQueue.paused)return;
 const kinds=state.steamgridEnabled?consoles.map(c=>c.id):AUTO_COVER_KINDS.filter(k=>!TITLE_COVER_KINDS.includes(k)||state.artwork.titleEnabled);
 const items=resume&&state.coverQueue.items.length?state.coverQueue.items:kinds.flatMap(kind=>(state.imported[kind]||[]).filter(g=>!g.offline).map(g=>({kind,uid:g.uid,title:g.title||g.uid,status:'pending'})));
 state.coverQueue.items=items;renderCoverQueue();let complete=0;
 for(const item of items){
  if(resume&&!['pending','downloading','failed'].includes(item.status)){complete++;continue;}
  delete item.error;
  if(generation!==state.coverGeneration||!state.artwork.enabled||state.coverQueue.paused)return;
  item.status='downloading';renderCoverQueue();
  try {const result=await fetchArtwork(item.kind,item.uid,null,generation);if(generation!==state.coverGeneration)return;item.status=result?.status||'checked';}
  catch(error){if(generation!==state.coverGeneration)return;item.status='failed';item.error=String(error);state.coverStatus.set(artKey(item.kind,item.uid),{message:String(error)});updateCoverStatus(item.kind,item.uid);
   if(state.steamgridEnabled&&/API key was rejected|rate limit|cooling down|Cannot reach SteamGridDB/i.test(String(error))){state.coverQueue.paused=true;renderCoverQueue();const p=$('#cover-scan-status');if(p)p.textContent='Scan paused: '+String(error);return;}}
  complete++;renderCoverQueue();const p=$('#cover-scan-status');if(p)p.textContent=`Artwork scan: ${complete} / ${items.length} checked`;
 }
}
async function saveArtworkOptions() {
  if (!state.native || state.artworkSaving) return;
  const controls=[$('#auto-covers'),$('#cover-style')];
  const titleControl=$('#auto-title-covers');
  if (titleControl) controls.push(titleControl);
  if (controls.some(node=>!node)) return;
  ++state.coverGeneration;
  state.artworkSaving=true;
  const enabled=controls[0].checked, style=controls[1].value, titleEnabled=titleControl ? titleControl.checked : Boolean(state.artwork.titleEnabled);
  controls.forEach(node => node.disabled=true);
  try {
    state.artwork=await nativeInvoke('save_artwork_settings',{enabled,style,titleEnabled});
    artworkWriteRevision++;
    state.artRevision++;clearCoverCache();state.missingArt.clear();state.coverStatus.clear();
    hydrateNativeCovers();
    toast('Artwork preferences saved.');void scanArtwork();
  } catch(error) {toast(`Could not save artwork preferences: ${error}`);}
  finally {
    state.artworkSaving=false;
    // Controls may belong to an old Settings view. Never replace the current
    // page, and restore whichever Settings controls are actually visible now.
    if (state.page==='settings') {
      const enabledControl=$('#auto-covers'), styleControl=$('#cover-style');
      if (enabledControl) {enabledControl.checked=state.artwork.enabled;enabledControl.disabled=false;}
      if (styleControl) {styleControl.value=state.artwork.style;styleControl.disabled=false;}
      const titleControl=$('#auto-title-covers');
      if (titleControl) {titleControl.checked=Boolean(state.artwork.titleEnabled);titleControl.disabled=false;}
    }
  }
}
let nativeRefreshGeneration=0;
async function refreshNative() {
  if (!state.native) { toast('Preview mode — launch the native Matrix-Play app to scan your filesystem.'); return; }
  const generation=++nativeRefreshGeneration;
  try {
    const snapshot=state.librarySnapshotSupported?await nativeInvoke('library_snapshot'):null;
    const results = await Promise.all([
      ...consoles.map(c => snapshot?Promise.resolve(snapshot.games[c.id]||[]):nativeInvoke('scan_console', {id:c.id})),
      nativeInvoke('scan_steam'), nativeInvoke('list_windows_games'),
    ]);
    if (generation!==nativeRefreshGeneration) return;
    if(snapshot)state.libraryFolders=snapshot.folders||{};
    consoles.forEach((c,index) => { state.imported[c.id] = results[index].map(g => ({...g, demo:false,offline:snapshot?offlineGame(c.id,g):Boolean(g.offline)})); });
    state.steam = results[consoles.length];
    state.windows = results[consoles.length+1];
    state.missingArt.clear(); // Rescan can introduce matching .jpg files next to ROMs.
    state.artRevision++;clearCoverCache();
    render();
    void scanArtwork();
  } catch (error) { if (generation===nativeRefreshGeneration) toast(`Library scan failed: ${error}`); }
}
// Bootstrap includes slow inventory/folder work. A returned snapshot must not
// replace local writes that succeeded while it was in flight, or a newer load.
let nativeBootstrapGeneration=0, artworkWriteRevision=0, favoriteWriteRevision=0;
async function bootstrapNative() {
  const generation=++nativeBootstrapGeneration;
  const profileRevisions=new Map(emulatorSavedRequests);
  const artworkRevision=artworkWriteRevision, favoritesRevision=favoriteWriteRevision,libraryRevision=libraryPrefRevision;
  const current=()=>generation===nativeBootstrapGeneration;
  try {
    const config=await nativeInvoke('bootstrap');
    if (!current()) return null;
    const profiles={...(config.profiles || {})};
    const readiness={...(config.readyProfiles||{})};
    for (const [id,profile] of Object.entries(state.configs)) {
      if (emulatorSavedRequests.get(id)!==profileRevisions.get(id)) profiles[id]=profile;readiness[id]=state.readyProfiles[id];
    }
    state.librarySnapshotSupported=Boolean(config.libraryPreferences);if(config.libraryPreferences&&libraryRevision===libraryPrefRevision)state.libraryPrefs=normalizeLibraryPrefs(config.libraryPreferences);state.recent=config.recent||state.recent;state.configs=profiles;state.readyProfiles=readiness;
    state.root=config.root;state.folders=config.folders;state.steamgridEnabled=Boolean(config.steamgridEnabled);
    state.discovery=config.emulatorDiscovery || state.discovery;
    if (artworkWriteRevision===artworkRevision) state.artwork=config.artwork || state.artwork;
    await loadNativeFavorites(config,()=>current() && favoriteWriteRevision===favoritesRevision);
    return current()?config:null;
  } catch(error) {
    if (current()) throw error;
    return null;
  }
}
async function initializeNative() {
  if (!state.native) return;
  $('#mode-indicator').textContent = 'NATIVE LINUX';
  $('#native-status').textContent = 'Native backend · folders managed automatically';
  try {
    if (await bootstrapNative()) {await refreshNative();await pollLibraries(true);setInterval(()=>void pollLibraries(),30000);}
  } catch(error) { toast(`Matrix-Play initialization failed: ${error}`); }
}

function renderUpdater() {
  if (state.overlay !== 'updates') return;
  const info = state.update || {phase:'checking',message:'Checking GitHub…'};
  const busy = ['checking','downloading','installing'].includes(info.phase);
  const action = {available:['download-update','Download update'], downloaded:['install-update',info.installMode === 'download' ? 'Installation instructions' : 'Agree and update'],installed:['restart-update','Restart Matrix-Play'],downloading:['cancel-update','Cancel download']}[info.phase];
  const percent = info.total ? Math.min(100,Math.round(info.received / info.total * 100)) : 0;
  const hint = info.installMode === 'system' ? 'Installation uses your Linux system installer and may ask for your password.' : info.installMode === 'appimage' ? 'Your AppImage will be replaced in its current folder. A .previous backup is kept.' : 'The downloaded Linux installer opens in your system installer.';
  // Keep interactive elements alive across polling ticks so keyboard focus,
  // screen-reader position and a pointer press survive progress updates.
  if (!$('.update-dialog')) {
    $('#overlay-root').innerHTML = `<div class="overlay" data-backdrop="true"><section class="dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title"><button type="button" class="close" data-action="close" aria-label="Close updates">${glyph('close')}</button><div class="eyebrow">MATRIX PLAY</div><h2 id="update-title">App updates</h2><p class="muted">Installed <strong id="update-current"></strong><span id="update-latest-label"> · Latest <strong id="update-latest"></strong></span></p><p class="update-message" role="status"></p><div id="update-progress"><progress max="100" value="0" aria-label="Update download"></progress><p class="muted" id="update-progress-text"></p></div><div class="update-notes"></div><p class="muted" id="update-hint"></p><p class="muted" id="update-safety">Before updating, save your progress and close games, emulators and other apps launched through Matrix-Play. Let installers and updates finish before closing them.</p><label id="update-close-confirm"><input type="checkbox" id="update-games-closed"> I have saved my progress, closed running apps, and waited for installers and updates to finish. Close all Matrix-Play windows to install.</label><div class="dialog-actions"><button type="button" class="button button-ghost" data-action="update-releases">GitHub releases</button><button type="button" class="button button-primary" id="update-action"></button></div></section></div>`;
  }
  const shell=$('.app-shell');if (shell) shell.inert=true;
  $('#update-current').textContent=info.currentVersion || '—';
  $('#update-latest').textContent=info.latestVersion || '';
  $('#update-latest-label').hidden=!info.latestVersion;
  $('.update-message').textContent=info.message || '';
  $('#update-progress').hidden=info.phase!=='downloading';
  $('.update-dialog progress').value=percent;
  $('#update-progress-text').textContent=`${percent}% · ${((info.received || 0)/1048576).toFixed(1)} / ${((info.total || 0)/1048576).toFixed(1)} MB`;
  $('.update-notes').textContent=info.notes || '';
  $('.update-notes').hidden=!info.notes;
  $('#update-hint').textContent=`${hint} Matrix-Play closes to install the update and stays closed when it succeeds. Open it again when you are ready. Running games, apps and installers are never force-stopped. Downloads are checked against a signed release before installation.`;
  $('#update-close-confirm').hidden=info.phase!=='downloaded';
  $('#update-safety').hidden=info.phase!=='downloaded';
  const nextAction=action?.[0] || 'check-updates';
  let button=$('#update-action');
  if (button.dataset.action && button.dataset.action!==nextAction) {
    // A press on Cancel must not become Install if the download completes
    // before pointer/key release. Preserve nodes only for the same action.
    const focused=document.activeElement===button;
    const replacement=button.cloneNode(false);
    button.replaceWith(replacement);button=replacement;
    if (focused) $('.update-dialog .close').focus();
  }
  button.dataset.action=nextAction;
  button.textContent=action?.[1] || (busy ? 'Please wait…' : 'Check again');
  button.disabled=!action && busy;
  button.classList.toggle('button-ghost',info.phase==='downloading');
  button.classList.toggle('button-primary',info.phase!=='downloading');
  if (button.disabled && document.activeElement===button) $('.update-dialog .close').focus();
}
let updateOperation = 0;
let updaterOpening = 0;
async function runUpdate(command) {
  if (!state.native) { toast('App updates are available in the Linux application.'); return; }
  if (state.updateBusy) return;
  const gamesClosed=command==='install_update' && $('#update-games-closed')?.checked===true;
  if (command==='install_update' && !gamesClosed) { toast('Save your progress, close launched apps, and wait for installers/updates to finish. Then check the confirmation.'); return; }
  state.updateBusy = true;
  const operation=++updateOperation;
  if (state.update) {
    state.update.phase = {check_updates:'checking',download_update:'downloading',install_update:'installing'}[command] || state.update.phase;
    state.update.message = 'Please wait…';
  }
  renderUpdater();
  let polling = false;
  const timer = setInterval(async () => {
    if (polling) return;
    polling = true;
    try { const info = await nativeInvoke('update_status'); if (state.updateBusy && operation===updateOperation) { state.update = info; renderUpdater(); } } catch (_) { /* The operation reports its own error. */ }
    finally { polling = false; }
  }, 500);
  try { state.update = await nativeInvoke(command, command==='install_update' ? {gamesClosed} : undefined); }
  catch(error) { state.update = {...state.update, phase:'error',message:String(error)}; }
  finally { clearInterval(timer); state.updateBusy = false; renderUpdater(); }
}
async function openUpdater() {
  if (!state.native) { toast('App updates are available in the Linux application.'); return; }
  const opening=++updaterOpening, operationAtOpen=updateOperation, busyAtOpen=state.updateBusy;
  state.lastFocus = document.activeElement; state.overlay = 'updates';
  renderUpdater();
  $('.update-dialog .close')?.focus();
  try {
    const info=await nativeInvoke('update_status');
    if (opening!==updaterOpening || state.overlay!=='updates') return;
    if (!busyAtOpen && !state.updateBusy && operationAtOpen===updateOperation) state.update=info;
  } catch(error) {
    if (opening!==updaterOpening || state.overlay!=='updates') return;
    if (!busyAtOpen && !state.updateBusy && operationAtOpen===updateOperation) state.update={phase:'error',message:String(error)};
  }
  renderUpdater();
  if (!state.updateBusy && ['idle','current'].includes(state.update?.phase)) void runUpdate('check_updates');
}

let steamgridKeyRevision=0;
async function handleAction(button) {
  const menu=button.closest?.('.library-menu');
  if (menu) { menu.open=false;menu.querySelector('summary')?.focus(); }
  const { action, id, uid, value } = button.dataset;
  if(action==='move-console'){const order=[...new Set([...(libraryPrefs().order||[]),...consoles.map(c=>c.id)])];const at=order.indexOf(id),next=at+Number(value);if(next>=0&&next<order.length){[order[at],order[next]]=[order[next],order[at]];await saveLibraryPref('order','',order);renderSettings();}return;}
  if(action==='save-game-metadata'){const selected={...state.selectedGame};const owner=$('.detail-panel');const metadata={display:$('#display-title').value.trim(),coverTitle:$('#cover-search-title').value.trim(),group:$('#version-group').value.trim()};if(!await saveLibraryPref('metadata',artKey(selected.id,selected.uid),metadata))return;if($('.detail-panel')===owner){dismissOverlay();render();}return;}
  if(action==='select-version'){state.previewGame={id,uid};rememberLibrary();renderConsole();openGame(id,uid);return;}
  if(action==='save-data-backup'||action==='save-data-restore'){button.disabled=true;try{await saveDataFlow(id,action==='save-data-restore');}catch(e){toast(String(e));}finally{button.disabled=false;}return;}
  if(action==='apply-save-restore'){button.disabled=true;try{toast(await nativeInvoke('apply_save_restore',{token:value}));dismissOverlay();}catch(e){toast(String(e));}finally{button.disabled=false;}return;}
  if (action === 'updates') void openUpdater();
  else if (action === 'check-updates') void runUpdate('check_updates');
  else if (action === 'download-update') void runUpdate('download_update');
  else if (action === 'install-update') void runUpdate('install_update');
  else if (action === 'cancel-update') { try { await nativeInvoke('cancel_update'); } catch(error) { toast(String(error)); } }
  else if (action === 'restart-update') { try { await nativeInvoke('restart_updated'); } catch(error) { toast(String(error)); } }
  else if (action === 'update-releases') { try { await nativeInvoke('open_update_releases'); } catch(error) { toast(String(error)); } }
  else if (action === 'home') goHome();
  else if (action === 'settings') goSettings();
  else if (action === 'steam' || action === 'windows') goExternal(action);
  else if (action === 'refresh') {
    if (!state.native) { toast('Browser preview — no filesystem access.'); return; }
    try { if (await bootstrapNative()) await refreshNative(); }
    catch(error) { toast(String(error)); }
  }
  else if (action === 'library-folders') void openLibraryFolders(id || state.consoleId);
  else if (action === 'browse-library-folder') void browseLibraryFolder(button);
  else if (action === 'add-library-folder' || action === 'remove-library-folder') void changeLibraryFolder(button);
  else if (action === 'open-folder') { try { await nativeInvoke('open_console_folder',{id:state.consoleId}); } catch (e) { toast(String(e)); } }
  else if (action === 'console') goConsole(id);
  else if (action === 'vendor') { state.vendor = value; renderHome(); }
  else if (action === 'configure') openConfigure(id);
  else if (action === 'configure-first') openConfigure(state.consoleId || 'switch');
  else if (action === 'close') dismissOverlay();
  else if (action === 'game') openGame(id,uid);
  else if (action === 'quick-launch') {
    if(findGame(id,uid)?.offline){toast('This game is on an offline drive. Reconnect it and refresh.');return;}
    if (!state.native) {openGame(id,uid);return;}
    rememberLibrary();void launchConsole(id,uid);
  }
  else if (action === 'play-console') {if (state.selectedGame) void launchConsole(state.selectedGame.id,state.selectedGame.uid);}
  else if (action === 'open-emulator') void launchConsole(id);
  else if (action === 'launch-details') void showLaunchDetails(id);
  else if (action === 'find-emulators') void findEmulators(button);
  else if (action === 'refresh-detected') void loadDetectedEmulators();
  else if (action === 'external-game') openExternalGame(button.dataset.kind,uid);
  else if (action === 'quick-external') {
    const kind = button.dataset.kind;
    if (!state.native || (kind === 'windows' && !windowsShortcutSupported(externalList(kind).find(game => game.uid === uid)))) { openExternalGame(kind,uid); return; }
    void launchExternal(kind,uid);
  }
  else if (action === 'add-windows') openAddWindows();
  else if (action === 'launch-external') {
    void launchExternal(button.dataset.kind,uid);
  }
  else if (action === 'remove-windows') {
    if (!state.native) { toast('Native Matrix-Play required.'); return; }
    const dialog=$('#overlay-root').firstElementChild;
    if (button.disabled) return;
    button.disabled=true;
    try { await nativeInvoke('remove_windows_game', {uid}); if (dialog===$('#overlay-root').firstElementChild) dismissOverlay(); await refreshNative(); toast('Removed library entry only. Installed files were preserved.'); }
    catch(error) { toast(String(error)); }
    finally {button.disabled=false;}
  }
  else if (action === 'console-favorites') { state.gameFilter='Favorites';state.query='';renderConsole(); }
  else if (action === 'reload-flatpaks') void loadFlatpakChoices();
  else if (action === 'browse-emulator') {
    if (!state.native) { toast('File browsing is available in the Linux app.');return; }
    if (button.disabled) return;
    const input=$('#emulator-path'), errorNode=$('#config-error'), draft=input?.value;
    if (!input) return;
    button.disabled=true;
    try { const path=await nativeInvoke('browse_emulator',{appimage:$('#emulator-launcher').value==='appimage'});if (path && input===$('#emulator-path') && input.value===draft) input.value=path; }
    catch(error) { if (input===$('#emulator-path') && errorNode===$('#config-error') && errorNode) errorNode.textContent=String(error); }
    finally {button.disabled=false;}
  }
  else if (action === 'filter-games') { state.gameFilter = value; renderConsole(); }
  else if (action === 'view') { state.view = value; rememberLibrary();renderConsole(); }
  else if (action === 'favorite') toggleFavorite(id,uid,button);
  else if (action === 'preview-folder') { $('#folder-input').value = ''; $('#folder-input').click(); }
  else if (action === 'clear-preview') {
    delete state.imported[state.consoleId]; state.query = ''; state.gameFilter = 'All';
    const persisted = savePrefs({type:'import',id:state.consoleId}); render();
    if (persisted) toast('Folder preview cleared. Your game files were not changed.');
  }
  else if (action === 'download-cover') {
    const selected = {...state.selectedGame};
    const serial = $('#cover-serial')?.value.trim();
    if (!serial) {toast('Enter the exact catalog title, game serial or disc ID first.');return;}
    button.disabled=true;
    try {const outcome=await fetchArtwork(selected.id,selected.uid,serial);toast(outcome?.message || 'Artwork scan complete.');}
    catch(error) {toast(`Cover lookup failed: ${error}`);}
    finally {button.disabled=false;}
  }
  else if (action === 'cover-project') {
    if (!state.native) {toast('Open thecoverproject.net in your browser.');return;}
    try {await nativeInvoke('open_cover_project');} catch(error) {toast(String(error));}
  }
  else if (action === 'queue-pause') {state.coverQueue.paused=true;++state.coverGeneration;renderCoverQueue();toast('Cover queue paused.');}
  else if (action === 'queue-resume') {state.coverQueue.paused=false;void scanArtwork(true);}
  else if (action === 'recent-game') {if(byId[id]){goConsole(id);if(state.native)await launchConsole(id,uid);}else{goExternal(id);if(state.native)await launchExternal(id,uid);}}
  else if (action === 'backup-library' || action === 'restore-library') {
    button.disabled=true;
    try {const result=await nativeInvoke('backup_library',{restoreBackup:action==='restore-library'});toast(result);if(action==='restore-library'&&result!=='Cancelled'){await bootstrapNative();await refreshNative();}}catch(e){toast(String(e));}finally{button.disabled=false;}
  }
  else if (action === 'game-options') {
    const selected={...state.selectedGame};state.overlay='game-options';
    $('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog game-options-dialog" role="dialog" aria-modal="true" aria-labelledby="game-options-title"><button type="button" class="close" data-action="close" aria-label="Close launch settings">${glyph('close')}</button><h2 id="game-options-title">Game launch settings</h2><p class="muted">Override this game's arguments only. One literal argument per line; use {game} for the file. No shell commands. Leave blank to use console defaults.</p><label class="field">Arguments<textarea id="game-options-args" rows="6" disabled></textarea></label><p id="game-options-status" role="status">Loading…</p><button type="button" class="button button-primary" data-action="save-game-options" disabled>Save</button></section></div>`;
    const owner=$('.game-options-dialog');owner._selection=selected;
    try {const options=await nativeInvoke('game_options',{kind:selected.id,uid:selected.uid});if($('.game-options-dialog')===owner){$('#game-options-args').value=(options.args||[]).join('\n');$('#game-options-args').disabled=false;owner.querySelector('[data-action=save-game-options]').disabled=false;$('#game-options-status').textContent='';$('#game-options-args').focus();}}catch(e){if($('.game-options-dialog')===owner)$('#game-options-status').textContent=String(e);}
  }
  else if (action === 'save-game-options') {
    const owner=$('.game-options-dialog');const selected=owner._selection;const args=$('#game-options-args').value.split('\n').map(a=>a.trim()).filter(Boolean);button.disabled=true;
    try {await nativeInvoke('save_game_options',{kind:selected.id,uid:selected.uid,args});if($('.game-options-dialog')===owner)dismissOverlay();toast('Game launch settings saved.');}catch(e){if($('.game-options-dialog')===owner)$('#game-options-status').textContent=String(e);}finally{button.disabled=false;}
  }
  else if (action === 'steamgrid-key-open') { try {await nativeInvoke('steamgrid_open_key');} catch(e){toast(String(e));} }
  else if (action === 'steamgrid-key-save' || action === 'steamgrid-key-clear') {
    const owner=$('#steamgrid-key');
    const key=action==='steamgrid-key-clear'?'':owner.value.trim();
    if (action==='steamgrid-key-save' && !key) {toast('Paste your SteamGridDB API key first.');return;}
    const request=++steamgridKeyRevision;button.disabled=true;
    try {const enabled=await nativeInvoke('steamgrid_save_key',{key});if(request!==steamgridKeyRevision)return;state.steamgridEnabled=enabled;if ($('#steamgrid-key')===owner && owner.value.trim()===key) {owner.value='';if(state.page==='settings')renderSettings();}void scanArtwork();toast(state.steamgridEnabled?'SteamGridDB enabled.':'SteamGridDB disabled.');}catch(e){toast(String(e));}finally{button.disabled=false;}
  }
  else if (action === 'steamgrid-open') openSteamGrid();
  else if (action === 'steamgrid-search' || action === 'steamgrid-game' || action === 'steamgrid-cover') void steamGridAction(button);
  else if (action === 'pick-cover') { $('#art-input').value = ''; $('#art-input').click(); }
}
function emulatorFormDraft(owner) {
  // Read controls directly: FormData omits the Flatpak choice while a reload
  // temporarily disables it. Preserve text and checkbox edits exactly.
  return JSON.stringify(Array.from(owner.elements).filter(field=>field.name)
    .map(field=>[field.name,field.type==='checkbox' ? field.checked : field.value]));
}
function setupEvents() {
  const previewConsole = event => {
    const card=event.target.closest?.('.console-card');
    const preview=$('.console-preview');
    if (state.page!=='home' || !card || !preview || preview.dataset.consoleId===card.dataset.id) return;
    preview.dataset.consoleId=card.dataset.id;
    preview.innerHTML=consolePreview(byId[card.dataset.id]);
  };
  const previewGame = event => {
    const item=event.target.closest?.('#game-results .game-item');
    const card=item?.querySelector('.game-card');
    if(card)previewLibraryGame(card.dataset.id,card.dataset.uid);
  };
  document.addEventListener('pointerover', previewGame);
  document.addEventListener('focusin', previewGame);
  document.addEventListener('pointerover', previewConsole);
  document.addEventListener('focusin', previewConsole);
  document.addEventListener('change', event => {
    const field=event.target;
    if(field.dataset?.pref){void saveLibraryPref(field.dataset.pref,'',field.checked);return;}
    for(const [attr,section]of [['consoleHide','hidden'],['consolePin','pinned']])if(field.dataset?.[attr]){const ids=new Set(libraryPrefs()[section]||[]);if(field.checked)ids.add(field.dataset[attr]);else ids.delete(field.dataset[attr]);void saveLibraryPref(section,'',[...ids]);return;}
    if (event.target.id==='detected-emulator') chooseDetectedEmulator();
    if (event.target.id==='flatpak-app') $('#runtime-fonts').checked=event.target.value.startsWith('net.pcsx2.PCSX2|');
    if (event.target.id==='emulator-launcher') updateLauncherFields();
    if (event.target.id==='flatpak-app' && event.target.selectedOptions[0]?.dataset.name) $('#emulator-name').value=event.target.selectedOptions[0].dataset.name;
    if (['auto-covers','auto-title-covers','cover-style'].includes(event.target.id)) void saveArtworkOptions();
  });
  document.addEventListener('click', event => {
    const button = event.target.closest('button[data-action]');
    if (button) return handleAction(button);
    if (event.target.matches('[data-backdrop="true"]')) dismissOverlay();
  });
  document.addEventListener('input', event => {
    if (event.target.id === 'game-search') {
      state.query = event.target.value;
      renderGameResults(); // Don't rerender the focused input on each keystroke.
    }
    if (event.target.id === 'external-search') {
      state.query = event.target.value;
      const selection = event.target.selectionStart;
      renderExternal(state.page);
      const input = $('#external-search'); input.focus(); input.setSelectionRange(selection,selection);
    }
  });
  document.addEventListener('change', event => {
    if (event.target.id === 'sort-games') { state.sort = event.target.value; rememberLibrary();renderGameResults(); }
    if (event.target.id === 'folder-input') importFolder(event.target.files);
    if (event.target.id === 'art-input') void loadCover(event.target.files?.[0]);
  });
  document.addEventListener('submit', event => {
    if (event.target.id === 'windows-form') {
      event.preventDefault();
      if (!state.native) { toast('Windows game registration requires native Matrix-Play.'); return; }
      const owner=event.target; if (owner.dataset.saving==='true') return;owner.dataset.saving='true';
      const form = new FormData(owner);
      void nativeInvoke('add_windows_game', {
        title:String(form.get('title') || '').trim(), target:String(form.get('target') || '').trim(), runner:'desktop'
      }).then(async () => { if (owner===$('#windows-form')) dismissOverlay(); await refreshNative(); toast('Game added to Windows library.'); })
        .catch(error => toast(String(error))).finally(()=>{delete owner.dataset.saving;});
      return;
    }
    if (event.target.id !== 'config-form') return;
    event.preventDefault();
    const owner=event.target; if (owner.dataset.saving==='true') return;
    const id = owner.dataset.id, form = new FormData(owner);
    const emulatorName = String(form.get('emulatorName') || '').trim();
    const emulatorPath = String(form.get('emulatorPath') || '').trim();
    const folderPath = String(form.get('folderPath') || '').trim();
    const launchArgs = String(form.get('launchArgs') || '').split('\n').map(s => s.trim()).filter(Boolean);
    const launcher=String(form.get('launcher') || 'executable');
    if (launcher==='flatpak' && Array.from(owner.elements).some(field=>field.name==='flatpakApp' && field.disabled)) {
      if ($('#config-form')===owner) $('#config-save-status').textContent=FLATPAK_LOADING_SAVE_MESSAGE;
      return;
    }
    const [flatpakId='',flatpakInstallation='']=String(form.get('flatpakApp') || '').split('|');
    if (!emulatorName || !folderPath || (launcher==='flatpak' ? !flatpakId : !emulatorPath)) return;
    const profile={emulatorName,emulatorPath,launchArgs,launcher,flatpakId,flatpakInstallation,extractAppimage:form.has('extractAppimage'),flatpakRuntimeFonts:form.has('flatpakRuntimeFonts')};
    if (state.native) {
      owner.dataset.saving='true';
      const submittedDraft=emulatorFormDraft(owner);
      $('#config-save-status').textContent='';
      const request=(emulatorSaveRequests.get(id)||0)+1;emulatorSaveRequests.set(id,request);
      const submit=event.submitter || event.target.querySelector('[type=submit]:not([data-open-after-save])');const openAfterSave=event.submitter?.dataset.openAfterSave==='true';submit.disabled=true;$('#config-error').textContent='';
      void nativeInvoke('save_emulator', {id,...profile,allowExecute:form.has('allowExecute')}).then(() => {
        if ((emulatorSavedRequests.get(id)||0)>request) return;
        emulatorSavedRequests.set(id,request);
        state.configs[id]=profile;state.readyProfiles[id]=true;
        const current=state.overlay==='configure' && $('#config-form')===owner;
        const edited=current && emulatorFormDraft(owner)!==submittedDraft;
        if (current && !edited) dismissOverlay();
        if (edited) $('#config-save-status').textContent='Your submitted settings were saved. Newer edits are not saved yet. Save again to use them.';
        render();toast(`${byId[id].name} emulator saved.${edited ? ' Newer edits are still unsaved.' : ''}`);
        if (openAfterSave && current && !edited) void launchConsole(id);
      }).catch(error => {if ($('#config-form')===owner) $('#config-error').textContent=String(error);else toast(String(error));})
        .finally(()=>{submit.disabled=false;delete owner.dataset.saving;});
    } else {
      state.configs[id]={...profile,folderPath};const persisted=savePrefs({type:'profile',id,value:state.configs[id]});dismissOverlay();render();if (persisted) toast('Browser preview settings saved.');
    }
  });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
      const menu=document.querySelector('details.library-menu[open]');
      if (menu?.open && !state.overlay) { event.preventDefault();menu.open=false;menu.querySelector('summary')?.focus();return; }
      if (state.overlay) dismissOverlay();
      else if (state.page === 'console') goHome();
    }
    // GTK/WebKit can label shifted Tab as Unidentified (ISO_Left_Tab).
    const isTab = event.key === 'Tab' || event.code === 'Tab' || event.keyCode === 9;
    if (state.overlay && isTab) {
      // Match real sequential focus: advanced summaries and multiline arguments
      // belong in the dialog, while hidden launcher fields and closed details do not.
      const options = [...$('#overlay-root').querySelectorAll('button,input,select,textarea,summary,a[href],[tabindex]')]
        .filter(node => node.tabIndex >= 0 && !node.matches(':disabled') && !node.closest('[hidden],[inert]')
          && (!node.closest('details:not([open])') || node.closest('details:not([open])').querySelector('summary') === node)
          && node.getClientRects().length && window.getComputedStyle(node).visibility !== 'hidden');
      if (!options.length) return;
      const first = options[0], last = options.at(-1);
      if (!options.includes(document.activeElement)) { event.preventDefault(); (event.shiftKey ? last : first).focus(); }
      else if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
  });
}
// Library organization is additive; identities and on-disk names stay unchanged.
let libraryWrites=Promise.resolve(), libraryPolling=false,libraryPrefRevision=0;
function normalizeLibraryPrefs(value){const p=value&&typeof value==='object'?value:{};for(const key of ['hidden','pinned','order'])p[key]=Array.isArray(p[key])?[...new Set(p[key].filter(id=>byId[id]))]:[];for(const key of ['views','metadata'])if(!p[key]||typeof p[key]!=='object'||Array.isArray(p[key]))p[key]={};return p;}
function libraryPrefs(){return state.libraryPrefs || (state.libraryPrefs={hidden:[],pinned:[],order:[],views:{},metadata:{},autoRefresh:true,grouping:false,readyOnly:false});}
function gameMetadata(id,uid){return libraryPrefs().metadata?.[artKey(id,uid)]||{};}
function saveLibraryPref(section,key,value){
 ++libraryPrefRevision;const p=libraryPrefs();if(section==='view')(p.views||={})[key]=value;else if(section==='metadata')(p.metadata||={})[key]=value;else p[section]=value;
 if(!state.native){try{localStorage.setItem('matrix-library-preferences',JSON.stringify(p));}catch{}return Promise.resolve(true);}
 libraryWrites=libraryWrites.catch(()=>{}).then(()=>nativeInvoke('set_library_preference',{section,key,value})).then(()=>true).catch(e=>{toast('Library preference was not saved: '+e);return false;});return libraryWrites;
}
function rememberLibrary(){if(state.page!=='console'||!state.consoleId)return;const v={layout:state.view,sort:state.sort,selected:state.previewGame?.id===state.consoleId?state.previewGame.uid:''};const old=libraryPrefs().views?.[state.consoleId];if(JSON.stringify(v)!==JSON.stringify(old))void saveLibraryPref('view',state.consoleId,v);}
function versionKey(id,g){const m=gameMetadata(id,g.uid);if(m.group==='-')return g.uid;if(m.group)return m.group.toLocaleLowerCase();return (g.originalTitle||g.title).replace(/\s+[—–-]\s+(?:Base|Update\s+\d+)$/i,'').replace(/\s*\((?:USA|Europe|Japan|World|Rev\s+[\w.]+|v\d[\w.]*)\)\s*$/i,'').trim().toLocaleLowerCase();}
function groupedGames(id,games){
 if(!libraryPrefs().grouping)return games;
 const groups=new Map();for(const g of games){const k=versionKey(id,g);if(!groups.has(k))groups.set(k,[]);groups.get(k).push(g);}
 return [...groups.values()].map(versions=>{const preferred=state.previewGame?.id===id?state.previewGame.uid:libraryPrefs().views?.[id]?.selected;const g=versions.find(g=>g.uid===preferred)||versions.find(g=>/\s[—–-]\sBase$/i.test(g.originalTitle||g.title))||versions[0];return {...g,versions};});
}
function consoleChoices(){const p=libraryPrefs();const order=[...(p.order||[]),...consoles.map(c=>c.id)];return consoles.filter(c=>!(p.hidden||[]).includes(c.id)&&(!p.readyOnly||consoleReady(c.id))&&(state.vendor==='All'||c.group===state.vendor)).sort((a,b)=>Number((p.pinned||[]).includes(b.id))-Number((p.pinned||[]).includes(a.id))||order.indexOf(a.id)-order.indexOf(b.id));}
function organizationControls(){const p=libraryPrefs();return `<details class="library-notes organization"><summary>Console visibility &amp; order</summary><label><input type="checkbox" data-pref="readyOnly" ${p.readyOnly?'checked':''}> Ready systems only</label><div class="console-order">${[...new Set([...(p.order||[]),...consoles.map(c=>c.id)])].map(id=>`<div><span>${escapeHtml(byId[id].name)}</span><label><input type="checkbox" data-console-hide="${id}" ${(p.hidden||[]).includes(id)?'checked':''}> Hide</label><label><input type="checkbox" data-console-pin="${id}" ${(p.pinned||[]).includes(id)?'checked':''}> Pin</label><button class="button button-ghost" data-action="move-console" data-id="${id}" data-value="-1" aria-label="Move ${escapeHtml(byId[id].name)} up">↑</button><button class="button button-ghost" data-action="move-console" data-id="${id}" data-value="1" aria-label="Move ${escapeHtml(byId[id].name)} down">↓</button></div>`).join('')}</div></details>`;}
function metadataControls(id,game){const m=gameMetadata(id,game.uid);const variants=gamesFor(id).filter(g=>versionKey(id,g)===versionKey(id,game));return `<details class="library-notes"><summary>Title &amp; versions${variants.length>1?' ('+variants.length+')':''}</summary><label class="field">Display title<input id="display-title" maxlength="240" value="${escapeHtml(m.display||'')}" placeholder="${escapeHtml(game.originalTitle||game.title)}"></label><label class="field">Cover search title<input id="cover-search-title" maxlength="240" value="${escapeHtml(m.coverTitle||'')}" placeholder="Original catalog title"></label><label class="field">Version group<input id="version-group" maxlength="240" value="${escapeHtml(m.group||'')}" placeholder="Automatic; use - to keep separate"></label><p class="muted">Changes affect the library only. Matching group names combine versions within this console when grouping is enabled.</p><button class="button button-ghost" data-action="save-game-metadata">Save title &amp; group</button>${variants.length>1?`<p>Choose version:</p><div class="tag-row">${variants.map(g=>`<button class="button button-ghost" data-action="select-version" data-id="${id}" data-uid="${escapeHtml(g.uid)}">${escapeHtml(g.originalTitle||g.title)}${g.offline?' · Offline':''}</button>`).join('')}</div>`:''}</details>`;}
function offlineGame(id,g){return (state.libraryFolders?.[id]||[]).some(f=>!f.available&&(g.path===f.path||g.path?.startsWith(f.path.replace(/\/$/,'')+'/')));}
async function pollLibraries(force=false){
 if(!state.native||libraryPolling||!force&&(libraryPrefs().autoRefresh===false||document.hidden||state.overlay))return;
 libraryPolling=true;const generation=nativeRefreshGeneration;try{await libraryWrites;const [result,steam,windows]=await Promise.all([nativeInvoke('library_snapshot'),nativeInvoke('scan_steam'),nativeInvoke('list_windows_games')]);if(!result?.games||!force&&state.overlay||generation!==nativeRefreshGeneration)return;const before=JSON.stringify([state.imported,state.libraryFolders,state.steam,state.windows]);if(Array.isArray(steam))state.steam=steam;if(Array.isArray(windows))state.windows=windows;state.libraryFolders=result.folders||{};for(const c of consoles)state.imported[c.id]=(result.games[c.id]||[]).map(g=>({...g,demo:false,offline:offlineGame(c.id,g)}));if(before!==JSON.stringify([state.imported,state.libraryFolders,state.steam,state.windows])){state.missingArt.clear();if(!state.overlay)render();}}catch(e){if(force)toast('Library refresh failed: '+e);}finally{libraryPolling=false;}
}
async function saveDataFlow(id,restore){
 const result=await nativeInvoke('save_data_dialog',{id,restore});if(result.cancelled)return;if(result.message){toast(result.message);return;}
 state.overlay='save-restore';$('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog" role="dialog" aria-modal="true" aria-labelledby="save-restore-title"><button class="close" data-action="close" aria-label="Cancel restore">${glyph('close')}</button><h2 id="save-restore-title">Review save restore</h2><p>Close the emulator before continuing. These files will be written inside ${escapeHtml(result.root)}. A safety backup is created first; files not listed are kept.</p><div class="save-file-list">${result.files.map(f=>`<p>${escapeHtml(f.name)} · ${f.bytes} bytes · ${f.replaces?'Replace':'Add'}</p>`).join('')}</div><button class="button button-primary" data-action="apply-save-restore" data-value="${escapeHtml(result.token)}">Restore these files</button></section></div>`;$('.dialog .close')?.focus();
}

readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();
