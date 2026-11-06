/* Matrix Play v0.5.4 — native Tauri library, launcher, and local artwork bridge. */
'use strict';

const byId = Object.fromEntries(consoles.map(consoleInfo => [consoleInfo.id, consoleInfo]));
const state = {
  page: 'home', consoleId: null, vendor: 'All', query: '',
  gameFilter: 'All', sort: 'default', view: 'grid',
  overlay: null, selectedGame: null, lastFocus: null,
  configs: {}, imported: {}, favorites: new Set(), art: new Map(),
  native: Boolean(window.__TAURI__?.core?.invoke), root: '', folders: {},
  steam: [], windows: [],
  missingArt: new Set(), pendingArt: new Set(),
  artwork: {enabled:true, style:"default"}, coverStatus: new Map(), coverGeneration: 0, artRevision: 0,
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

const consoleImages = {"switch": "./assets/consoles/switch.webp", "ps1": "./assets/consoles/ps1.webp", "ps2": "./assets/consoles/ps2.webp", "ps3": "./assets/consoles/ps3.webp", "psp": "./assets/consoles/psp.webp", "n64": "./assets/consoles/n64.webp", "gamecube": "./assets/consoles/gamecube.webp", "dreamcast": "./assets/consoles/dreamcast.webp", "wii": "./assets/consoles/wii.webp", "wiiu": "./assets/consoles/wiiu.webp", "gb": "./assets/consoles/gb.webp", "gba": "./assets/consoles/gba.webp", "nds": "./assets/consoles/nds.webp", "3ds": "./assets/consoles/3ds.webp"};
// Supplied console artwork; original silhouettes remain a fallback.
function deviceArt(id) {
  if (consoleImages[id]) return `<img class="device-illustration console-image" src="${consoleImages[id]}" alt="" decoding="async">`;
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
  switch: ['nsp', 'xci'], ps1: ['cue','chd','iso','pbp'], ps2: ['iso','chd','cso'],
  ps3: ['iso'], psp: ['iso','cso','pbp'], n64: ['z64','n64','v64'],
  gamecube: ['iso','gcm','rvz','gcz'], dreamcast: ['gdi','cdi','chd'],
  wii: ['wbfs','iso','rvz','wia'], wiiu: ['wua','wud','wux','rpx'],
  gb: ['gb','gbc'], gba: ['gba'], nds: ['nds'], '3ds': ['3ds','cci','cxi'],
};
const tones = { Sony: '#536cae', Nintendo: '#66547d', Sega: '#8d5e55' };
const consoleTone = consoleInfo => tones[consoleInfo.group] || '#657aa5';
const artKey = (consoleId, uid) => `${consoleId}|${uid}`;
const favoriteKey = (consoleId, uid) => `${consoleId}|${uid}`;
const importedFor = id => Array.isArray(state.imported[id]) ? state.imported[id] : [];
const isConfigured = id => Boolean((state.configs[id]?.launcher === "flatpak" ? state.configs[id]?.flatpakId : state.configs[id]?.emulatorPath) && (state.native || state.configs[id]?.folderPath));
const configuredCount = () => consoles.filter(consoleInfo => isConfigured(consoleInfo.id)).length;

function demoGames(consoleInfo) {
  return consoleInfo.games.map(([title, year, a, b], index) => ({
    uid: `demo-${index}`, title, year, a, b, demo: true,
  }));
}
function gamesFor(id) {
  const saved = importedFor(id);
  return state.native ? saved : (saved.length ? saved : demoGames(byId[id]));
}
function findGame(id, uid) {
  return gamesFor(id).find(game => game.uid === uid);
}
function readPrefs() {
  try {
    const stored = localStorage.getItem('matrix-ui-v03') || localStorage.getItem('console-hub-v02');
    const raw = stored ? JSON.parse(stored) : null;
    if (raw && typeof raw === 'object') {
      if (raw.configs && typeof raw.configs === 'object') state.configs = raw.configs;
      if (raw.imported && typeof raw.imported === 'object') state.imported = raw.imported;
      if (Array.isArray(raw.favorites)) state.favorites = new Set(raw.favorites.filter(x => typeof x === 'string'));
      return;
    }
  } catch { /* Browser storage can be disabled for local files. */ }
  // Migrate compatible emulator fields and favorites from the v0.1 preview.
  try {
    const previous = JSON.parse(localStorage.getItem('console-hub-ui-v1') || '{}');
    if (previous.configs) state.configs = previous.configs;
    if (Array.isArray(previous.favorites)) {
      state.favorites = new Set(previous.favorites.map(key => {
        const [consoleId, index] = key.split(':');
        return favoriteKey(consoleId, `demo-${index}`);
      }));
    }
  } catch { /* Optional migration only. */ }
}
function savePrefs() {
  try {
    localStorage.setItem('matrix-ui-v03', JSON.stringify({
      configs: state.configs, imported: state.imported, favorites: [...state.favorites],
    }));
  } catch { toast('The browser could not persist these UI preview settings.'); }
}
let artDb = null;
async function initializeArt() {
  // Native Matrix Play artwork lives in XDG data; browser preview uses IndexedDB.
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
      if (item?.blob instanceof Blob) state.art.set(item.key, URL.createObjectURL(item.blob));
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
    ? 'LIBRARY <span>/</span> EMULATORS &amp; FOLDERS'
    : state.page === 'steam' ? 'LIBRARY <span>/</span> STEAM GAMES'
    : state.page === 'windows' ? 'LIBRARY <span>/</span> WINDOWS GAMES'
    : name ? `LIBRARY <span>/</span> CONSOLES <span>/</span> ${escapeHtml(name)}`
    : 'LIBRARY <span>/</span> CONSOLES';
}
function consoleCard(consoleInfo) {
  const count = importedFor(consoleInfo.id).length;
  return `<button class="console-card" type="button" data-action="console" data-id="${consoleInfo.id}" style="--tint:${consoleTone(consoleInfo)}" aria-label="Browse ${escapeHtml(consoleInfo.name)} library">
    <span class="console-card__top"><span class="console-brand">${escapeHtml(consoleInfo.brand)}</span><span class="card-arrow">${glyph('arrow')}</span></span>
    ${deviceArt(consoleInfo.id)}
    <span class="console-card__bottom"><strong>${escapeHtml(consoleInfo.name)}</strong><small>${count ? `${count} ${state.native ? 'games' : 'folder preview titles'}` : state.native ? 'No games yet' : `${consoleInfo.games.length} sample titles`} <span class="card-status ${isConfigured(consoleInfo.id) ? 'ready' : ''}"></span> ${isConfigured(consoleInfo.id) ? 'Profile saved' : 'Not linked'}</small></span>
  </button>`;
}
function renderHome() {
  const visible = consoles.filter(consoleInfo => state.vendor === 'All' || consoleInfo.group === state.vendor);
  $('#screen').innerHTML = `<div class="title-row"><div><div class="eyebrow">YOUR LIBRARY</div><h1 class="page-title">Choose a console.</h1><p class="page-subtitle">Each console opens its own game collection.</p></div>
    <div class="stats"><div class="stat"><strong>${consoles.length}</strong><span>Consoles</span></div><div class="stat-separator"></div><div class="stat"><strong>${configuredCount()}</strong><span>Profiles saved</span></div></div></div>
    <div class="section-toolbar"><h2 class="section-title">All systems</h2><div class="segmented" role="group" aria-label="Filter console brands">${['All', 'Sony', 'Nintendo', 'Sega'].map(vendor => `<button type="button" class="${state.vendor === vendor ? 'active' : ''}" data-action="vendor" data-value="${vendor}" aria-pressed="${state.vendor === vendor}">${vendor === 'All' ? 'All systems' : vendor === 'Sony' ? 'PlayStation' : vendor}</button>`).join('')}</div></div>
    <div class="console-grid">${visible.map(consoleCard).join('')}</div>
    <div class="notice">${glyph('info')}<span>${state.native
      ? `Matrix Play reads each console’s own Games directory under ${escapeHtml(state.root)}. Steam and Windows have separate tabs.`
      : 'Browser-only preview: sample games are for design review. A native Matrix Play build creates the folders and scans installed games.'}</span></div>`;
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
  if (state.missingArt.has(key) || state.pendingArt.has(key)) return;
  state.pendingArt.add(key);
  const revision = state.artRevision;
  try {
    const uri = await nativeInvoke('get_cover', { kind, uid });
    if (revision !== state.artRevision) return;
    if (uri) {
      state.art.set(key, uri);
      updateVisibleCover(key, uri);
    } else state.missingArt.add(key);
  } catch { state.missingArt.add(key); /* Missing/corrupt image keeps a useful placeholder. */ }
  finally { state.pendingArt.delete(key); if (revision !== state.artRevision) void requestNativeCover(element); }
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
async function loadNativeFavorites(config) {
  let entries=config.favorites || {};
  if (!config.favoritesMigrated) {
    const legacy={};
    for (const key of state.favorites) {
      const split=key.indexOf('|'), id=key.slice(0,split), uid=key.slice(split+1);
      if (byId[id] && uid.startsWith('/')) (legacy[id] ||= []).push(uid);
    }
    entries=await nativeInvoke('import_favorites',{entries:legacy});
  }
  applyNativeFavorites(entries);
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
    savePrefs();
    if (state.page==='console' && state.consoleId===id) renderConsole();
    if (state.overlay==='game' && state.selectedGame?.id===id && state.selectedGame?.uid===uid) openGame(id,uid);
    toast(favorite ? `Added to ${byId[id].name} favorites.` : `Removed from ${byId[id].name} favorites.`);
  }).catch(error=>toast(`Could not save favorite: ${error}`)).finally(()=>{button.disabled=false;});
}

function gameCard(consoleInfo, game, index) {
  const liked = state.favorites.has(favoriteKey(consoleInfo.id, game.uid));
  return `<div class="game-item">
    <button class="game-card" type="button" data-action="quick-launch" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}" aria-label="${state.native ? 'Launch' : 'View'} ${escapeHtml(game.title)}${state.native ? '' : ' details'}">
      ${gameCover(consoleInfo, game, index)}<span class="game-title">${escapeHtml(game.title)}</span><span class="game-meta">${game.year || (game.demo ? 'Sample game' : state.native ? 'Installed game' : 'Folder preview')} · ${escapeHtml(consoleInfo.abbrev)}</span></button>
    <button class="game-info" type="button" data-action="game" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}" aria-label="Details for ${escapeHtml(game.title)}" title="Game details">${glyph('info')}</button>
    <button class="favorite ${liked ? 'active' : ''}" type="button" data-action="favorite" data-id="${consoleInfo.id}" data-uid="${escapeHtml(game.uid)}" aria-label="${liked ? 'Remove' : 'Add'} ${escapeHtml(game.title)} ${liked ? 'from' : 'to'} favorites" aria-pressed="${liked}">${glyph('heart')}</button>
  </div>`;
}
function visibleGames(id) {
  let result = gamesFor(id).filter(game => game.title.toLocaleLowerCase().includes(state.query.toLocaleLowerCase()));
  if (state.gameFilter === 'Favorites') result = result.filter(game => state.favorites.has(favoriteKey(id, game.uid)));
  if (state.sort === 'az') result.sort((a,b) => a.title.localeCompare(b.title));
  if (state.sort === 'recent') result.sort((a,b) => (b.year || 0) - (a.year || 0));
  return result;
}
function renderGameResults() {
  const consoleInfo = byId[state.consoleId];
  const games = visibleGames(consoleInfo.id);
  $('#game-results').innerHTML = games.length
    ? `<div class="${state.view === 'grid' ? 'game-grid' : 'game-list'}">${games.map((game,index) => gameCard(consoleInfo,game,index)).join('')}</div>`
    : state.gameFilter === 'Favorites' ? `<div class="empty empty-library"><strong>${favoriteCount(consoleInfo.id) ? 'No favorites match this search' : `No ${escapeHtml(consoleInfo.name)} favorites yet`}</strong><p>${favoriteCount(consoleInfo.id) ? 'Try another search, or switch to All games.' : 'Select the heart on a game in All games to add it to this console’s favorites list.'}</p><button type="button" class="button button-ghost" data-action="filter-games" data-value="All">Browse all games</button></div>` : `<div class="empty empty-library"><strong>${state.native && !importedFor(consoleInfo.id).length ? 'No games detected yet' : 'No matching games'}</strong><p>${state.native && !importedFor(consoleInfo.id).length ? 'Place your game files in this console’s Games folder and click Refresh.' : 'Try another search or switch to All games.'}</p></div>`;
  hydrateNativeCovers();
}
function renderConsole() {
  const consoleInfo = byId[state.consoleId];
  const imported = importedFor(consoleInfo.id);
  const hasImported = imported.length > 0;
  const subtitle = state.native ? `${imported.length} games · ${isConfigured(consoleInfo.id) ? 'Emulator linked' : 'Emulator not linked'}` : hasImported ? `${imported.length} folder preview titles · ${isConfigured(consoleInfo.id) ? 'Profile saved' : 'Emulator not linked'}` : `${consoleInfo.games.length} sample titles · ${isConfigured(consoleInfo.id) ? 'Profile saved' : 'Emulator not linked'}`;
  $('#screen').innerHTML = `<button type="button" class="back-link" data-action="home">${glyph('back')} All consoles</button>
    <section class="console-hero" style="--tint:${consoleTone(consoleInfo)}"><div class="console-hero-copy"><div class="eyebrow">${escapeHtml(consoleInfo.brand)} · DEDICATED LIBRARY</div><h1>${escapeHtml(consoleInfo.name)}</h1><p>${subtitle}</p><button type="button" class="button button-ghost" data-action="configure" data-id="${consoleInfo.id}">${glyph('plus')} ${isConfigured(consoleInfo.id) ? 'Edit emulator' : 'Set up emulator'}</button><button type="button" class="button button-subtle hero-favorites" data-action="console-favorites">${glyph('heart')} My favorites (${favoriteCount(consoleInfo.id)})</button></div>${deviceArt(consoleInfo.id)}</section>
    <div class="library-heading"><div><h2>Game library</h2><p>${state.native ? `Managed folder: ${escapeHtml(state.folders[consoleInfo.id] || '')}` : hasImported ? 'Showing filenames from your selected folder. Not playable from this browser.' : 'Sample games and placeholder covers for UI review.'}</p></div><div class="library-heading-actions"><button type="button" class="button button-subtle" data-action="${state.native ? 'open-folder' : 'preview-folder'}">${glyph('folder')} ${state.native ? 'Open games folder' : 'Preview game folder'}</button>${!state.native && hasImported ? `<button type="button" class="button button-ghost" data-action="clear-preview">${glyph('trash')} Clear preview</button>` : ''}</div></div>
    <div class="library-tools"><label class="search">${glyph('search')}<input type="search" id="game-search" autocomplete="off" placeholder="Search ${escapeHtml(consoleInfo.name)} games" aria-label="Search ${escapeHtml(consoleInfo.name)} games" value="${escapeHtml(state.query)}"></label>
      <div class="segmented" role="group" aria-label="Game filter">${['All','Favorites'].map(value => `<button type="button" data-action="filter-games" data-value="${value}" class="${state.gameFilter === value ? 'active' : ''}" aria-pressed="${state.gameFilter === value}">${value === 'All' ? 'All games' : `Favorites (${favoriteCount(consoleInfo.id)})`}</button>`).join('')}</div>
      <select id="sort-games" class="sort-select" aria-label="Sort games"><option value="default" ${state.sort === 'default' ? 'selected' : ''}>Default order</option><option value="az" ${state.sort === 'az' ? 'selected' : ''}>Title: A to Z</option><option value="recent" ${state.sort === 'recent' ? 'selected' : ''}>Newest first</option></select>
      <div class="segmented" role="group" aria-label="Library layout"><button type="button" data-action="view" data-value="grid" aria-label="Grid view" aria-pressed="${state.view === 'grid'}" class="${state.view === 'grid' ? 'active' : ''}">${glyph('grid')}</button><button type="button" data-action="view" data-value="list" aria-label="List view" aria-pressed="${state.view === 'list'}" class="${state.view === 'list' ? 'active' : ''}">${glyph('list')}</button></div>
    </div><div id="game-results"></div>
    <div class="notice">${glyph('info')}<span>${state.native ? 'Your ROM files are not moved or modified. Launch requires a configured emulator path; launch arguments vary by emulator.' : hasImported ? 'Only filenames and relative paths were read. Nothing was uploaded. Folder preview entries cannot launch games until the native app is implemented.' : 'This is a browser preview. The native app will create and scan console folders on first run.'}</span></div>`;
  renderGameResults();
}
function renderSettings() {
  $('#screen').innerHTML = `<div class="settings-header"><div class="eyebrow">YOUR SETUP</div><h1 class="page-title">Emulators &amp; folders.</h1><p class="page-subtitle">Each console uses its own emulator and dedicated Games directory.</p>${state.native ? `<div class="root-path">${glyph('folder')} Managed folder <strong>${escapeHtml(state.root)}</strong></div>` : ''}</div>
    <div class="notice" style="margin:0 0 25px">${glyph('info')}<span>${state.native ? 'Choose Set up for a console, then select AppImage, Flatpak, or another executable. Browse for an AppImage anywhere, or choose an already-installed Flatpak from the list.' : 'Browser preview settings are not connected to your filesystem. Native folder creation and launching require the Matrix Play Linux application.'}</span></div>
    <section class="artwork-settings"><h2>Cover artwork</h2>
    <p>PS1 and PS2 covers from xlenore, matched by serial within each console. Unknown or conflicting serials need your review. Local covers take priority.</p>
    <label><input type="checkbox" id="auto-covers" ${state.artwork.enabled ? 'checked' : ''} ${state.native ? '' : 'disabled'}> Download missing PS1/PS2 covers after library scans</label>
    <label for="cover-style">Cover style</label><select id="cover-style" ${state.native ? '' : 'disabled'}><option value="default" ${state.artwork.style==='default'?'selected':''}>Default</option><option value="3d" ${state.artwork.style==='3d'?'selected':''}>3D</option></select>
    <p>Downloaded covers stay on this computer for offline use. Requests send the serial and style to GitHub; your game files and full paths are not uploaded.</p>
    <p>TheCoverProject: manual only. Automated access permission has not been established. Download in your browser, then use Choose cover in game details.</p>
    <button type="button" class="button button-ghost" data-action="cover-project">Open TheCoverProject</button><p id="cover-scan-status" role="status"></p></section>
    <div class="section-toolbar"><h2 class="section-title">Console profiles</h2><span class="status">${configuredCount()} / ${consoles.length} saved</span></div>
    <div class="settings-list">${consoles.map(consoleInfo => `<div class="settings-row"><div class="settings-row__left"><div class="settings-glyph" style="--tint:${consoleTone(consoleInfo)}">${deviceArt(consoleInfo.id)}</div><div class="settings-name"><strong>${escapeHtml(consoleInfo.name)}</strong><small>Suggested: ${escapeHtml(consoleInfo.emulator)}${state.native ? ` · ${escapeHtml(state.folders[consoleInfo.id] || '')}` : state.configs[consoleInfo.id]?.folderPath ? ` · ${escapeHtml(state.configs[consoleInfo.id].folderPath)}` : ''}</small></div></div><div class="settings-row__right"><span class="status ${isConfigured(consoleInfo.id) ? 'ready' : ''}">${isConfigured(consoleInfo.id) ? 'Profile saved' : 'Not set up'}</span><button type="button" class="button button-ghost" data-action="configure" data-id="${consoleInfo.id}">${isConfigured(consoleInfo.id) ? 'Edit' : 'Set up'}</button></div></div>`).join('')}</div>`;
}
function render() {
  renderNav();
  if (state.page === 'home') renderHome();
  if (state.page === 'console') renderConsole();
  if (state.page === 'settings') renderSettings();
  if (state.page === 'steam' || state.page === 'windows') renderExternal(state.page);
}
function goHome() {
  dismissOverlay(); state.page = 'home'; state.consoleId = null; state.query = ''; render(); window.scrollTo(0,0);
}
function goConsole(id) {
  if (!byId[id]) return;
  dismissOverlay(); state.page = 'console'; state.consoleId = id; state.query = ''; state.gameFilter = 'All'; state.sort = 'default'; render(); window.scrollTo(0,0);
}
function goSettings() {
  dismissOverlay(); state.page = 'settings'; state.consoleId = null; render(); window.scrollTo(0,0);
}
function goExternal(page) {
  dismissOverlay(); state.page = page; state.consoleId = null;
  state.query = ''; state.gameFilter = 'All'; state.sort = 'default';
  render(); window.scrollTo(0,0);
}
function externalList(page) { return page === 'steam' ? state.steam : state.windows; }
function renderExternal(page) {
  const steam = page === 'steam';
  const title = steam ? 'Steam Games' : 'Windows Games';
  const games = externalList(page).filter(g => g.title.toLowerCase().includes(state.query.toLowerCase()));
  $('#screen').innerHTML = `<div class="title-row"><div><div class="eyebrow">${steam ? 'STEAM LIBRARY' : 'WINDOWS LIBRARY'}</div><h1 class="page-title">${title}.</h1>
      <p class="page-subtitle">${steam ? 'Detected from existing Steam installations. Matrix Play does not copy or reinstall your games.' : 'Launch your existing Windows installs with Wine or their desktop shortcuts. Nothing is moved.'}</p></div>
      <div class="stats"><div class="stat"><strong>${externalList(page).length}</strong><span>Games</span></div></div></div>
    <div class="library-tools"><label class="search">${glyph('search')}<input type="search" id="external-search" placeholder="Search ${title}" value="${escapeHtml(state.query)}"></label>
    ${!steam ? `<button type="button" class="button button-primary" data-action="add-windows">${glyph('plus')} Add Windows game</button>` : ''}</div>
    ${games.length ? `<div class="game-grid">${games.map((game,index) => {
      const cover = gameCover({ id: page, abbrev: steam ? 'STEAM' : 'WINDOWS' }, game, index);
      return `<div class="game-item"><button class="game-card" type="button" data-action="quick-external" data-kind="${page}" data-uid="${escapeHtml(game.uid)}" aria-label="${state.native ? 'Launch' : 'View'} ${escapeHtml(game.title)}">${cover}<span class="game-title">${escapeHtml(game.title)}</span><span class="game-meta">${steam ? 'Steam · App ' + escapeHtml(game.appId || game.uid) : 'Windows · ' + escapeHtml(game.runner || 'launcher')}</span></button><button class="game-info" type="button" data-action="external-game" data-kind="${page}" data-uid="${escapeHtml(game.uid)}" aria-label="Details for ${escapeHtml(game.title)}" title="Game details">${glyph('info')}</button></div>`;
    }).join('')}</div>` : `<div class="empty external-empty"><strong>${state.query ? 'No matching games' : `No ${title.toLowerCase()} detected`}</strong><p>${state.query ? 'Try a different search term.' : steam ? 'Install and sign into Steam, then click Refresh.' : 'Choose Add Windows game to register an existing .exe or .desktop launcher.'}</p></div>`}
    <div class="notice">${glyph('info')}<span>${state.native ? steam ? 'Steam titles are discovered from appmanifest files and launched through Steam.' : 'Use existing Wine installations or desktop launchers; advanced per-game Proton/prefix handling will be added later.' : 'Browser preview only. Native discovery and launching are available in the Matrix Play Linux build.'}</span></div>`;
  hydrateNativeCovers();
}
function openExternalGame(kind, uid) {
  const game = externalList(kind).find(g => g.uid === uid); if (!game) return;
  state.lastFocus = document.activeElement; state.overlay = 'external'; state.selectedGame = {id:kind,uid};
  $('#overlay-root').innerHTML = `<div class="overlay detail-overlay" data-backdrop="true"><aside class="detail-panel" role="dialog" aria-modal="true" aria-labelledby="detail-title"><button class="close" data-action="close" aria-label="Close details">${glyph('close')}</button>
    <div class="detail-cover">${gameCover({id:kind,abbrev:kind.toUpperCase()},game,0)}</div><div class="eyebrow">${kind.toUpperCase()} LIBRARY</div><h2 id="detail-title">${escapeHtml(game.title)}</h2>
    <p class="detail-sub">${escapeHtml(game.target || `Steam app ID ${game.appId || game.uid}`)}</p><p class="detail-description">${kind === 'steam' ? 'Matrix Play asks your existing Steam client to start this game.' : 'Matrix Play starts the configured existing Windows launcher without copying the game files.'}</p>
    <button class="button button-primary launch-button" data-action="launch-external" data-kind="${kind}" data-uid="${escapeHtml(uid)}">${glyph('play')} Play game</button>
    <div class="detail-actions"><button class="button button-ghost" type="button" data-action="pick-cover">${glyph('image')} Choose cover</button></div>
    ${kind === 'windows' ? `<div class="detail-actions"><button class="button button-ghost" data-action="remove-windows" data-uid="${escapeHtml(uid)}">${glyph('trash')} Remove from Matrix Play</button></div>` : ''}
    <p class="detail-hint">${state.native ? 'Launching uses the installed Steam client or chosen Windows runner.' : 'Browser preview cannot launch games.'}</p></aside></div>`;
  $('.detail-panel .close').focus();
  hydrateNativeCovers();
}
function openAddWindows() {
  state.lastFocus = document.activeElement; state.overlay = 'windows-add';
  $('#overlay-root').innerHTML = `<div class="overlay" data-backdrop="true"><section class="dialog" role="dialog" aria-modal="true" aria-labelledby="windows-title"><button class="close" data-action="close">${glyph('close')}</button>
   <div class="eyebrow">WINDOWS LIBRARY</div><h2 id="windows-title">Add existing game.</h2><p class="muted">Register a game already installed on Linux. No files are moved or copied.</p>
   <form id="windows-form"><div class="field"><label for="windows-name">Game title</label><input id="windows-name" name="title" required maxlength="150" placeholder="Game title"></div>
   <div class="field"><label for="windows-target">Installed executable or launcher</label><input id="windows-target" name="target" required placeholder="/home/you/Games/My Game/game.exe"></div>
   <div class="field"><label for="windows-runner">Launch using</label><select id="windows-runner" name="runner"><option value="desktop">Existing .desktop launcher (recommended)</option><option value="wine">Wine (.exe file)</option></select><small>Games installed through other managers may require their own .desktop launcher and existing Wine/Proton prefix.</small></div>
   <div class="dialog-actions"><button class="button button-ghost" type="button" data-action="close">Cancel</button><button class="button button-primary" type="submit">Add game</button></div></form></section></div>`;
  $('#windows-name').focus();
}
function dismissOverlay() {
  if (state.overlay === null) return;
  state.overlay = null;
  $('#overlay-root').innerHTML = '';
  if (state.lastFocus?.isConnected) state.lastFocus.focus();
  state.lastFocus = null;
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
  $('#emulator-file-hint').textContent=launcher==='appimage' ? 'Choose your downloaded .AppImage wherever you keep it. Matrix Play leaves it in place.' : 'Choose an installed emulator file, for example one in /usr/bin.';
  if (launcher==='flatpak') void loadFlatpakChoices();
}
async function loadFlatpakChoices() {
  const select=$('#flatpak-app'); if (!select) return;
  if (!state.native) { $('#flatpak-help').textContent='Installed apps are available in the Linux application.';return; }
  const previous=select.value || select.dataset.saved || '';
  select.disabled=true;$('#flatpak-help').textContent='Reading installed Flatpak apps…';
  try {
    const apps=await nativeInvoke('list_flatpaks');
    if (select!==$('#flatpak-app')) return;
    select.innerHTML='<option value="">Choose an installed app…</option>'+apps.map(app=>`<option value="${escapeHtml(app.appId+'|'+app.installation)}" data-name="${escapeHtml(app.name)}">${escapeHtml(app.name)} — ${escapeHtml(app.appId)} (${escapeHtml(app.installation)})</option>`).join('');
    select.value=previous;
    $('#flatpak-help').textContent=apps.length ? 'Choose your installed emulator. Matrix Play gives it read access to this console’s Games folder for each launch.' : 'No Flatpak apps found. Install an emulator through Software, then click Reload apps.';
  } catch(error) { $('#flatpak-help').textContent=String(error); }
  finally { select.disabled=false; }
}
function openConfigure(id) {
  const consoleInfo=byId[id];if (!consoleInfo) return;
  state.lastFocus=document.activeElement;state.overlay='configure';state.selectedGame=null;
  const current=state.configs[id] || {};
  const launcher=current.launcher==='flatpak' ? 'flatpak' : current.launcher==='appimage' || /\.appimage$/i.test(current.emulatorPath || '') || !current.emulatorPath ? 'appimage' : 'executable';
  $('#overlay-root').innerHTML=`<div class="overlay" data-backdrop="true"><section class="dialog emulator-dialog" role="dialog" aria-modal="true" aria-labelledby="config-title"><button type="button" class="close" data-action="close" aria-label="Close dialog">${glyph('close')}</button><div class="eyebrow">EMULATOR SETUP</div><h2 id="config-title">${escapeHtml(consoleInfo.name)}</h2><p class="muted">Link an emulator you already installed or downloaded.</p>
    <form id="config-form" data-id="${id}"><div class="field"><label for="emulator-launcher">How is your emulator installed?</label><select id="emulator-launcher" name="launcher">${[['appimage','AppImage file'],['flatpak','Flatpak app'],['executable','Other executable']].map(([value,label])=>`<option value="${value}" ${launcher===value?'selected':''}>${label}</option>`).join('')}</select></div>
    <div id="file-fields"><div class="field"><label id="emulator-file-label" for="emulator-path">AppImage file</label><div class="file-choice"><input id="emulator-path" name="emulatorPath" placeholder="Choose a file or paste its full path" value="${escapeHtml(current.emulatorPath || '')}"><button type="button" class="button button-ghost" data-action="browse-emulator">Browse…</button></div><small id="emulator-file-hint"></small></div><div id="appimage-options" class="field"><label class="check-row"><input type="checkbox" id="allow-execute" name="allowExecute" checked> Allow this AppImage to run</label><small>Adds execute permission to the chosen AppImage if needed. Saving does not run it.</small><label class="check-row"><input type="checkbox" name="extractAppimage" ${current.extractAppimage?'checked':''}> Run without FUSE (extraction mode)</label><small>Use this if your AppImage reports that FUSE is unavailable.</small></div></div>
    <div id="flatpak-fields" hidden><div class="field"><label for="flatpak-app">Installed Flatpak emulator</label><select id="flatpak-app" name="flatpakApp" data-saved="${escapeHtml(current.flatpakId ? current.flatpakId+'|'+current.flatpakInstallation : '')}"><option value="">Choose an installed app…</option></select><small id="flatpak-help"></small></div><button type="button" class="button button-ghost" data-action="reload-flatpaks">Reload apps</button><p class="muted">Flatpak apps are installed through Software. Downloaded .flatpakref files are not executable files to browse for.</p></div>
    <div class="field"><label for="emulator-name">Emulator name</label><input id="emulator-name" name="emulatorName" maxlength="120" value="${escapeHtml(current.emulatorName || consoleInfo.emulator)}" required></div>
    <details class="advanced-setup"><summary>Advanced launch options</summary><div class="field"><label for="launch-args">Launch arguments (one per line)</label><textarea id="launch-args" name="launchArgs" rows="3">${escapeHtml((current.launchArgs?.length ? current.launchArgs : defaultLaunchArgs(id)).join('\n'))}</textarea><small>{game} is the game file. Defaults vary by console; some emulators require different options. Each line is passed as one argument.</small></div></details>
    <div class="field"><label for="library-path">Game library folder</label><input id="library-path" name="folderPath" value="${escapeHtml(state.native ? state.folders[id] || '' : current.folderPath || '')}" ${state.native?'readonly':'required'}><small>Place this console’s games here. Your emulator can stay wherever it is installed.</small></div><p id="config-error" class="config-error" role="alert"></p><div class="dialog-actions"><button class="button button-ghost" type="button" data-action="close">Cancel</button><button class="button button-primary" type="submit">Save emulator</button></div></form></section></div>`;
  updateLauncherFields();$('#emulator-launcher').focus();
}
function openGame(id, uid) {
  const consoleInfo = byId[id], game = consoleInfo && findGame(id, uid); if (!game) return;
  if (state.overlay !== 'game') state.lastFocus = document.activeElement;
  state.overlay = 'game'; state.selectedGame = { id, uid };
  const liked = state.favorites.has(favoriteKey(id,uid));
  const index = gamesFor(id).indexOf(game);
  $('#overlay-root').innerHTML = `<div class="overlay detail-overlay" data-backdrop="true"><aside class="detail-panel" role="dialog" aria-modal="true" aria-labelledby="detail-title">
    <button type="button" class="close" data-action="close" aria-label="Close game details">${glyph('close')}</button>
    <div class="detail-cover">${gameCover(consoleInfo,game,index)}</div><div class="eyebrow">GAME DETAILS · ${state.native ? 'LOCAL GAME' : game.demo ? 'DEMO TITLE' : 'FOLDER PREVIEW'}</div><h2 id="detail-title">${escapeHtml(game.title)}</h2>
    <p class="detail-sub">${escapeHtml(consoleInfo.name)}${game.year ? ` · ${game.year}` : ''}</p>
    <div class="tag-row"><span class="tag">${escapeHtml(consoleInfo.abbrev)}</span><span class="tag">${escapeHtml(state.configs[id]?.emulatorName || consoleInfo.emulator)}</span><span class="tag">${isConfigured(id) ? 'Profile saved' : 'Not connected'}</span></div>
    <p class="detail-description">${state.native ? 'The Play button launches this file with the emulator configured for this console.' : `The native application will launch this game using ${escapeHtml(state.configs[id]?.emulatorName || consoleInfo.emulator)}. This browser preview cannot execute games.`}</p>
    <button type="button" class="button button-primary launch-button" data-action="play-console">${glyph('play')} Play game</button>
    <div class="detail-actions"><button type="button" class="button button-ghost" data-action="pick-cover">${glyph('image')} Choose cover</button><button type="button" class="button button-ghost" data-action="favorite" data-id="${id}" data-uid="${escapeHtml(uid)}">${glyph('heart')} ${liked ? 'Unfavorite' : 'Favorite'}</button></div>
    ${state.native ? `<section class="cover-tools"><h3>Cover artwork</h3><p class="cover-result" role="status">${escapeHtml(coverMessage(id,uid))}</p>${['ps1','ps2'].includes(id) ? `<label for="cover-serial">Confirm game serial</label><input id="cover-serial" placeholder="SLUS-20312" value="${escapeHtml(state.coverStatus.get(artKey(id,uid))?.serial || '')}" maxlength="16"><small>Use the serial for this exact game, region and disc. Compressed images may need a serial entered here.</small><button type="button" class="button button-ghost" data-action="download-cover">Find cover by serial</button>` : '<p>Automatic artwork is available for PS1 and PS2. Choose a local image for this console.</p>'}<button type="button" class="button button-ghost" data-action="cover-project">Open TheCoverProject</button></section>` : ''}
    <p class="detail-hint">${state.native ? 'The configured emulator is required to launch this title. Cover artwork is stored under ~/.local/share/matrix/covers.' : 'Preview only. Play does not execute an emulator. Custom artwork is saved in this browser.'}</p>
  </aside></div>`;
  $('.detail-panel .close').focus();
  hydrateNativeCovers();
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
function importFolder(files) {
  const id = state.consoleId;
  if (!id) return;
  const allowed = new Set(EXTS[id] || []);
  const matched = Array.from(files).filter(file => allowed.has(file.name.split('.').at(-1).toLocaleLowerCase()));
  const seen = new Set();
  const imported = matched.slice(0,2000).map((file,index) => {
    const path = file.webkitRelativePath || file.name;
    if (seen.has(path)) return null;
    seen.add(path);
    return { uid: `file:${path}`, title: cleanTitle(file.name), relativePath: path, year: null, demo: false,
      a: ['#5877b7','#a16570','#6e94b2','#8a7bad'][index % 4],
      b: ['#213459','#352b49','#20394f','#2e2c4a'][index % 4] };
  }).filter(Boolean);
  if (!imported.length) {
    toast(`No recognized ${byId[id].name} game filenames were found. Supported extensions: ${[...allowed].map(ext => '.'+ext).join(', ')}.`);
    return;
  }
  state.imported[id] = imported; state.query = ''; state.gameFilter = 'All';
  savePrefs(); render();
  toast(`Previewed ${imported.length} ${byId[id].name} filename${imported.length === 1 ? '' : 's'}. No game files were copied or uploaded.`);
}
function refreshCoverUI(selected) {
  if (state.page === 'console') { renderGameResults(); openGame(selected.id, selected.uid); }
  else if (state.page === 'steam' || state.page === 'windows') {
    renderExternal(state.page);
    openExternalGame(state.page, selected.uid);
  }
}

async function loadCover(file) {
  const selected = state.selectedGame;
  if (!selected || !file) return;
  const sizeLimit = state.native ? 4 * 1024 * 1024 : 8 * 1024 * 1024;
  if (!['image/png','image/jpeg','image/webp'].includes(file.type) || file.size > sizeLimit) {
    toast(`Choose a PNG, JPG, or WebP cover under ${sizeLimit / (1024 * 1024)} MB.`); return;
  }
  const key = artKey(selected.id, selected.uid);
  if (state.native) {
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      await nativeInvoke('save_cover', { kind: selected.id, uid: selected.uid, bytes });
      const previous = state.art.get(key);
      if (previous?.startsWith('blob:')) URL.revokeObjectURL(previous);
      state.art.set(key, URL.createObjectURL(file));
      state.missingArt.delete(key);
      refreshCoverUI(selected);
      toast('Cover saved to Matrix Play’s Linux artwork folder.');
    } catch (error) { toast(`Could not save cover: ${error}`); }
    return;
  }
  const old = state.art.get(key);
  const next = URL.createObjectURL(file);
  state.art.set(key, next);
  if (old?.startsWith('blob:')) URL.revokeObjectURL(old);
  const saved = await storeArt(key,file);
  refreshCoverUI(selected);
  toast(saved ? 'Custom cover saved to this browser.' : 'Custom cover updated for this session; browser storage was unavailable.');
}
function coverMessage(kind,uid) {
  const status = state.coverStatus.get(artKey(kind,uid));
  if (!status) return ['ps1','ps2'].includes(kind) ? (state.artwork.enabled ? 'Waiting for automatic artwork scan.' : 'Automatic artwork is off.') : 'Choose a local cover for this console.';
  return [status.message, status.serial, status.source ? `Identity: ${status.source}` : '', status.url ? 'Provider: xlenore on GitHub' : ''].filter(Boolean).join(' · ');
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
    state.artRevision++;state.missingArt.delete(key); state.art.delete(key);
    // Load only currently visible cards; other games use the persistent native cache later.
    document.querySelectorAll('.game-cover[data-art-key]').forEach(node => {
      if (node.dataset.artKey === key) void requestNativeCover(node);
    });
  }
  return outcome;
}
async function scanArtwork() {
  const generation=++state.coverGeneration;
  if (!state.native || !state.artwork.enabled) return;
  let complete=0;
  const games=['ps1','ps2'].flatMap(kind => (state.imported[kind] || []).map(g=>({kind,uid:g.uid})));
  for (const {kind,uid} of games) {
    if (generation!==state.coverGeneration || !state.artwork.enabled) return;
    try { await fetchArtwork(kind,uid,null,generation); }
    catch(error) {
      if (generation!==state.coverGeneration) return;
      state.coverStatus.set(artKey(kind,uid),{message:`Artwork unavailable: ${error}`});updateCoverStatus(kind,uid);
    }
    complete++;
    const progress=$('#cover-scan-status');
    if (progress) progress.textContent=`Artwork scan: ${complete} / ${games.length} checked`;
  }
}
async function saveArtworkOptions() {
  if (!state.native) return;
  ++state.coverGeneration;
  const controls=[$('#auto-covers'),$('#cover-style')];
  const enabled=controls[0].checked, style=controls[1].value;
  controls.forEach(node => node.disabled=true);
  try {
    state.artwork=await nativeInvoke('save_artwork_settings',{enabled,style});
    state.artRevision++;state.art.clear();state.missingArt.clear();state.coverStatus.clear();
    toast('Artwork preferences saved.');void scanArtwork();
  } catch(error) {toast(`Could not save artwork preferences: ${error}`);renderSettings();}
  finally {controls.forEach(node=>node.disabled=false);}
}
async function refreshNative() {
  if (!state.native) { toast('Preview mode — launch the native Matrix Play app to scan your filesystem.'); return; }
  try {
    const results = await Promise.all([
      ...consoles.map(c => nativeInvoke('scan_console', {id:c.id})),
      nativeInvoke('scan_steam'), nativeInvoke('list_windows_games'),
    ]);
    consoles.forEach((c,index) => { state.imported[c.id] = results[index].map(g => ({...g, demo:false})); });
    state.steam = results[consoles.length];
    state.windows = results[consoles.length+1];
    state.missingArt.clear(); // Rescan can introduce matching .jpg files next to ROMs.
    state.artRevision++;state.art.clear();
    render();
    void scanArtwork();
  } catch (error) { toast(`Library scan failed: ${error}`); }
}
async function initializeNative() {
  if (!state.native) return;
  $('#mode-indicator').textContent = 'NATIVE LINUX';
  $('#native-status').textContent = 'Native backend · folders managed automatically';
  try {
    const config = await nativeInvoke('bootstrap');
    state.root = config.root; state.folders = config.folders;
    state.artwork = config.artwork || {enabled:true,style:'default'};
    state.configs = config.profiles;
    await loadNativeFavorites(config);
    await refreshNative();
  } catch(error) { toast(`Matrix Play initialization failed: ${error}`); }
}
function renderUpdater() {
  if (state.overlay !== 'updates') return;
  const info = state.update || {phase:'checking',message:'Checking GitHub…'};
  const busy = ['checking','downloading','installing'].includes(info.phase);
  const action = {available:['download-update','Download update'], downloaded:['install-update',info.installMode === 'download' ? 'Open installer' : 'Install update'],installed:['restart-update','Restart Matrix Play']}[info.phase];
  const percent = info.total ? Math.min(100,Math.round(info.received / info.total * 100)) : 0;
  const hint = info.installMode === 'system' ? 'Installation uses your Linux system installer and may ask for your password.' : info.installMode === 'appimage' ? 'Your AppImage will be replaced in its current folder. A .previous backup is kept.' : 'The downloaded Linux installer opens in your system installer.';
  $('#overlay-root').innerHTML = `<div class="overlay" data-backdrop="true"><section class="dialog update-dialog" role="dialog" aria-modal="true" aria-labelledby="update-title"><button type="button" class="close" data-action="close" aria-label="Close updates">${glyph('close')}</button><div class="eyebrow">MATRIX PLAY</div><h2 id="update-title">App updates</h2><p class="muted">Installed <strong>${escapeHtml(info.currentVersion || '—')}</strong>${info.latestVersion ? ` · Latest <strong>${escapeHtml(info.latestVersion)}</strong>` : ''}</p><p class="update-message" role="status">${escapeHtml(info.message)}</p>${info.phase === 'downloading' ? `<progress max="100" value="${percent}" aria-label="Update download"></progress><p class="muted">${percent}% · ${(info.received/1048576).toFixed(1)} / ${(info.total/1048576).toFixed(1)} MB</p>` : ''}${info.notes ? `<div class="update-notes">${escapeHtml(info.notes)}</div>` : ''}<p class="muted">${escapeHtml(hint)} Downloads are checked against a signed release before installation.</p><div class="dialog-actions"><button type="button" class="button button-ghost" data-action="update-releases">GitHub releases</button>${info.phase === 'downloading' ? '<button type="button" class="button button-ghost" data-action="cancel-update">Cancel download</button>' : action ? `<button type="button" class="button button-primary" data-action="${action[0]}">${action[1]}</button>` : `<button type="button" class="button button-primary" data-action="check-updates" ${busy ? 'disabled' : ''}>${busy ? 'Please wait…' : 'Check again'}</button>`}</div></section></div>`;
}
async function runUpdate(command) {
  if (!state.native) { toast('App updates are available in the Linux application.'); return; }
  if (state.updateBusy) return;
  state.updateBusy = true;
  if (state.update) {
    state.update.phase = {check_updates:'checking',download_update:'downloading',install_update:'installing'}[command] || state.update.phase;
    state.update.message = 'Please wait…';
  }
  renderUpdater();
  let polling = false;
  const timer = setInterval(async () => {
    if (polling) return;
    polling = true;
    try { const info = await nativeInvoke('update_status'); if (state.updateBusy) { state.update = info; renderUpdater(); } } catch (_) { /* The operation reports its own error. */ }
    finally { polling = false; }
  }, 500);
  try { state.update = await nativeInvoke(command); }
  catch(error) { state.update = {...state.update, phase:'error',message:String(error)}; }
  finally { clearInterval(timer); state.updateBusy = false; renderUpdater(); }
}
async function openUpdater() {
  if (!state.native) { toast('App updates are available in the Linux application.'); return; }
  state.lastFocus = document.activeElement; state.overlay = 'updates';
  try { state.update = await nativeInvoke('update_status'); } catch(error) { state.update = {phase:'error',message:String(error)}; }
  renderUpdater();
  $('#update-title')?.closest('.dialog').querySelector('button')?.focus();
  if (!state.updateBusy && ['idle','current','error'].includes(state.update?.phase)) void runUpdate('check_updates');
}

async function handleAction(button) {
  const { action, id, uid, value } = button.dataset;
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
    try { const config = await nativeInvoke('bootstrap'); state.configs = config.profiles; state.folders = config.folders; state.artwork = config.artwork || state.artwork; await loadNativeFavorites(config); await refreshNative(); }
    catch(error) { toast(String(error)); }
  }
  else if (action === 'open-folder') { try { await nativeInvoke('open_console_folder',{id:state.consoleId}); } catch (e) { toast(String(e)); } }
  else if (action === 'console') goConsole(id);
  else if (action === 'vendor') { state.vendor = value; renderHome(); }
  else if (action === 'configure') openConfigure(id);
  else if (action === 'configure-first') openConfigure(state.consoleId || 'switch');
  else if (action === 'close') dismissOverlay();
  else if (action === 'game') openGame(id,uid);
  else if (action === 'quick-launch') {
    if (!state.native) { openGame(id,uid); return; }
    try {
      await nativeInvoke('launch_console_game', {id,uid});
      toast('Game launch requested.');
    } catch(error) { toast(`Unable to launch: ${error}`); }
  }
  else if (action === 'play-console') {
    if (!state.native) { toast('Browser preview cannot launch emulators.'); return; }
    try {
      await nativeInvoke('launch_console_game', {id:state.selectedGame.id,uid:state.selectedGame.uid});
      dismissOverlay(); toast('Launch request sent to emulator.');
    } catch(error) { toast(String(error)); }
  }
  else if (action === 'external-game') openExternalGame(button.dataset.kind,uid);
  else if (action === 'quick-external') {
    const kind = button.dataset.kind;
    if (!state.native) { openExternalGame(kind,uid); return; }
    try {
      await nativeInvoke(kind === 'steam' ? 'launch_steam_game' : 'launch_windows_game', {uid});
      toast('Game launch requested.');
    } catch(error) { toast(`Unable to launch: ${error}`); }
  }
  else if (action === 'add-windows') openAddWindows();
  else if (action === 'launch-external') {
    if (!state.native) { toast('Browser preview cannot launch games.'); return; }
    try { await nativeInvoke(button.dataset.kind === 'steam' ? 'launch_steam_game' : 'launch_windows_game', {uid});
      dismissOverlay(); toast('Launch request sent.');
    } catch(error) { toast(String(error)); }
  }
  else if (action === 'remove-windows') {
    if (!state.native) { toast('Native Matrix Play required.'); return; }
    try { await nativeInvoke('remove_windows_game', {uid}); dismissOverlay(); await refreshNative(); toast('Removed library entry only. Installed files were preserved.'); }
    catch(error) { toast(String(error)); }
  }
  else if (action === 'console-favorites') { state.gameFilter='Favorites';state.query='';renderConsole(); }
  else if (action === 'reload-flatpaks') void loadFlatpakChoices();
  else if (action === 'browse-emulator') {
    if (!state.native) { toast('File browsing is available in the Linux app.');return; }
    button.disabled=true;
    try { const input=$('#emulator-path');const path=await nativeInvoke('browse_emulator',{appimage:$('#emulator-launcher').value==='appimage'});if (path && input===$('#emulator-path')) input.value=path; }
    catch(error) { if ($('#config-error')) $('#config-error').textContent=String(error); }
    finally {button.disabled=false;}
  }
  else if (action === 'filter-games') { state.gameFilter = value; renderConsole(); }
  else if (action === 'view') { state.view = value; renderConsole(); }
  else if (action === 'favorite') toggleFavorite(id,uid,button);
  else if (action === 'preview-folder') { $('#folder-input').value = ''; $('#folder-input').click(); }
  else if (action === 'clear-preview') {
    delete state.imported[state.consoleId]; state.query = ''; state.gameFilter = 'All';
    savePrefs(); render(); toast('Folder preview cleared. Your game files were not changed.');
  }
  else if (action === 'download-cover') {
    const selected = {...state.selectedGame};
    const serial = $('#cover-serial')?.value.trim();
    if (!serial) {toast('Enter the game serial first.');return;}
    button.disabled=true;
    try {const outcome=await fetchArtwork(selected.id,selected.uid,serial);toast(outcome?.message || 'Artwork scan complete.');}
    catch(error) {toast(`Cover lookup failed: ${error}`);}
    finally {button.disabled=false;}
  }
  else if (action === 'cover-project') {
    if (!state.native) {toast('Open thecoverproject.net in your browser.');return;}
    try {await nativeInvoke('open_cover_project');} catch(error) {toast(String(error));}
  }
  else if (action === 'pick-cover') { $('#art-input').value = ''; $('#art-input').click(); }
}
function setupEvents() {
  document.addEventListener('change', event => {
    if (event.target.id==='emulator-launcher') updateLauncherFields();
    if (event.target.id==='flatpak-app' && event.target.selectedOptions[0]?.dataset.name) $('#emulator-name').value=event.target.selectedOptions[0].dataset.name;
    if (['auto-covers','cover-style'].includes(event.target.id)) void saveArtworkOptions();
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
    if (event.target.id === 'sort-games') { state.sort = event.target.value; renderGameResults(); }
    if (event.target.id === 'folder-input') importFolder(event.target.files);
    if (event.target.id === 'art-input') void loadCover(event.target.files?.[0]);
  });
  document.addEventListener('submit', event => {
    if (event.target.id === 'windows-form') {
      event.preventDefault();
      if (!state.native) { toast('Windows game registration requires native Matrix Play.'); return; }
      const form = new FormData(event.target);
      void nativeInvoke('add_windows_game', {
        title:String(form.get('title') || '').trim(), target:String(form.get('target') || '').trim(), runner:String(form.get('runner') || '')
      }).then(async () => { dismissOverlay(); await refreshNative(); toast('Game added to Windows library.'); })
        .catch(error => toast(String(error)));
      return;
    }
    if (event.target.id !== 'config-form') return;
    event.preventDefault();
    const id = event.target.dataset.id, form = new FormData(event.target);
    const emulatorName = String(form.get('emulatorName') || '').trim();
    const emulatorPath = String(form.get('emulatorPath') || '').trim();
    const folderPath = String(form.get('folderPath') || '').trim();
    const launchArgs = String(form.get('launchArgs') || '').split('\n').map(s => s.trim()).filter(Boolean);
    const launcher=String(form.get('launcher') || 'executable');
    const [flatpakId='',flatpakInstallation='']=String(form.get('flatpakApp') || '').split('|');
    if (!emulatorName || !folderPath || (launcher==='flatpak' ? !flatpakId : !emulatorPath)) return;
    const profile={emulatorName,emulatorPath,launchArgs,launcher,flatpakId,flatpakInstallation,extractAppimage:form.has('extractAppimage')};
    if (state.native) {
      const submit=event.target.querySelector('[type=submit]');submit.disabled=true;$('#config-error').textContent='';
      void nativeInvoke('save_emulator', {id,...profile,allowExecute:form.has('allowExecute')}).then(() => {
        state.configs[id]=profile;
        if (state.overlay==='configure' && $('#config-form')?.dataset.id===id) dismissOverlay();
        render();toast(`${byId[id].name} emulator saved.`);
      }).catch(error => {if ($('#config-form')?.dataset.id===id) $('#config-error').textContent=String(error);else toast(String(error));})
        .finally(()=>{submit.disabled=false;});
    } else {
      state.configs[id]={...profile,folderPath};savePrefs();dismissOverlay();render();toast('Browser preview settings saved.');
    }
  });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
      if (state.overlay) dismissOverlay();
      else if (state.page === 'console') goHome();
    }
    if (state.overlay && event.key === 'Tab') {
      const options = [...$('#overlay-root').querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled])')];
      if (!options.length) return;
      const first = options[0], last = options.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
  });
}
readPrefs(); setupEvents(); render(); window.matrixStartup?.complete(); void initializeArt(); void initializeNative();
