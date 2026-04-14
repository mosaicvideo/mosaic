import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Store } from '@tauri-apps/plugin-store';
import { createQueue, isVideo, VIDEO_EXTS } from './queue.js';
import { readSheetOpts, readShotsOpts, readOutput, readProduce, applyOpts, applyProduce } from './options.js';
import { wireDropzone } from './dropzone.js';

const queue = createQueue(document.getElementById('queue'), {
  onReveal: (path) => invoke('reveal_in_finder', { path }).catch(console.error),
  onChange: () => { refreshActionBar(); saveSettings(); },
});
let store;
let saveTimer = null;
let running = false;
let userCancelled = false;

function init() {
  window.addEventListener('error', (e) => showBanner(`JS error: ${e.message}`));
  window.addEventListener('unhandledrejection', (e) => showBanner(`Promise rejection: ${e.reason?.message || e.reason}`));
  wireButtons();
  wireDropzone(document.getElementById('dropzone'), addPaths);
  wireEvents();
  updateQualityVisibility();
  refreshActionBar();
  loadSettings();
  checkTools();
}

function guard(fn) {
  return async (...args) => {
    try { return await fn(...args); }
    catch (e) { console.error(e); showBanner(`${e?.message || e}`); }
  };
}

async function loadSettings() {
  try {
    store = await Store.load('settings.json');
    const saved = {
      sheet: await store.get('sheet'),
      shots: await store.get('shots'),
      out: await store.get('out'),
      produce: await store.get('produce'),
    };
    applyOpts(saved.sheet, saved.shots, saved.out);
    applyProduce(saved.produce);
    updateQualityVisibility();
    refreshActionBar();
  } catch (e) {
    console.error('settings load failed:', e);
  }
}

let toolsOk = false;

async function checkTools() {
  const app = document.getElementById('app');
  const te = document.getElementById('tools-error');
  const q = document.getElementById('queue');
  const qh = document.querySelector('.queue-head');
  try {
    await invoke('check_tools');
    toolsOk = true;
    te.classList.add('hidden');
    q.classList.remove('hidden');
    qh.classList.remove('hidden');
    app.classList.remove('tools-missing');
  } catch (_) {
    toolsOk = false;
    te.classList.remove('hidden');
    q.classList.add('hidden');
    qh.classList.add('hidden');
    app.classList.add('tools-missing');
  }
  refreshActionBar();
}

function wireButtons() {
  document.getElementById('btn-add-files').onclick = guard(async () => {
    const picked = await open({ multiple: true, filters: [{ name: 'Videos', extensions: VIDEO_EXTS }] });
    if (!picked) return;
    addPaths(Array.isArray(picked) ? picked : [picked]);
  });
  document.getElementById('btn-add-folder').onclick = guard(async () => {
    const dir = await open({ directory: true, multiple: false });
    if (!dir) return;
    const paths = await invoke('scan_folder', { path: dir, recursive: true });
    if (!paths.length) { showBanner(`No videos found in ${dir}`); return; }
    addPaths(paths);
  });
  document.getElementById('btn-clear').onclick = () => queue.clear();
  document.getElementById('btn-generate').onclick = guard(onGenerate);
  document.getElementById('btn-cancel').onclick = guard(() => {
    userCancelled = true;
    return invoke('cancel_job');
  });
  document.getElementById('btn-pick-folder').onclick = guard(async () => {
    const dir = await open({ directory: true, multiple: false });
    if (dir) {
      document.getElementById('custom-folder-path').textContent = dir;
      refreshActionBar();
      saveSettings();
    }
  });
  document.getElementById('btn-retry-tools').onclick = checkTools;
  document.getElementById('btn-settings').onclick = openSettings;
  document.getElementById('btn-settings-close').onclick = closeSettings;
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && isSettingsOpen()) closeSettings();
  });

  document.querySelectorAll('input[name="out"]').forEach(r => r.onchange = () => {
    document.getElementById('custom-folder-row').classList.toggle('hidden', r.value !== 'custom' || !r.checked);
    refreshActionBar();
  });

  // Any option input (main "run options" + settings view) triggers save + UI refresh
  const selectors = '#run-options input, #settings-view input, #settings-view select';
  document.querySelectorAll(selectors).forEach(el => {
    el.addEventListener('change', () => {
      updateQualityVisibility();
      enforceProduceAtLeastOne();
      refreshActionBar();
      saveSettings();
    });
  });

  const suffixDefaults = { 'shots-suffix': '_screenshot_', 'sheet-suffix': '_contact_sheet' };
  for (const [id, def] of Object.entries(suffixDefaults)) {
    const el = document.getElementById(id);
    if (!el) continue;
    el.addEventListener('blur', () => {
      if (!el.value) {
        el.value = def;
        refreshActionBar();
        saveSettings();
      }
    });
  }
}

function isSettingsOpen() {
  return !document.getElementById('settings-view').classList.contains('hidden');
}
function openSettings() {
  document.getElementById('main-view').classList.add('hidden');
  document.getElementById('settings-view').classList.remove('hidden');
}
function closeSettings() {
  document.getElementById('settings-view').classList.add('hidden');
  document.getElementById('main-view').classList.remove('hidden');
}

function updateQualityVisibility() {
  const pairs = [['sheet-format', 'sheet-quality'], ['shots-format', 'shots-quality']];
  for (const [fmtId, qualId] of pairs) {
    const fmt = document.getElementById(fmtId);
    const qual = document.getElementById(qualId);
    if (!fmt || !qual) continue;
    const label = qual.closest('label');
    if (!label) continue;
    const isJpeg = fmt.value === 'Jpeg';
    label.classList.toggle('hidden', !isJpeg);
  }
}

function saveSettings() {
  if (!store) return;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(doSave, 250);
}

async function doSave() {
  if (!store) return;
  await store.set('sheet', readSheetOpts());
  await store.set('shots', readShotsOpts());
  await store.set('out', readOutput());
  await store.set('produce', readProduce());
  await store.save();
}

async function addPaths(paths) {
  const vids = paths.filter(isVideo);
  const added = queue.add(vids);
  if (!added.length) return;
  for (const it of added) {
    try {
      const info = await invoke('probe_video', { path: it.path });
      queue.update(it.id, { probed: true, info });
    } catch (_) { /* ignore; errors surface at generation */ }
  }
}

function wireEvents() {
  listen('job:file-start', ({ payload }) => {
    queue.update(payload.fileId, { status: 'Running', progress: 'Starting…' });
    updateOverall(payload.index - 1, payload.total);
  });
  listen('job:step', ({ payload }) => {
    queue.update(payload.fileId, { progress: payload.label });
  });
  listen('job:file-done', ({ payload }) => {
    queue.update(payload.fileId, {
      status: 'Done',
      progress: 'Done',
      outputPath: payload.outputPath,
    });
  });
  listen('job:file-failed', ({ payload }) => {
    queue.update(payload.fileId, { status: 'Failed', error: payload.error });
  });
  listen('job:finished', () => {
    /* totals surfaced by onGenerate once all passes complete */
  });
}

function enforceProduceAtLeastOne() {
  const shots = document.getElementById('prod-shots');
  const sheet = document.getElementById('prod-sheet');
  if (!shots.checked && !sheet.checked) shots.checked = true;
}

function updateOverall(done, total) {
  const p = document.getElementById('progress');
  p.max = total; p.value = done;
}

async function onGenerate() {
  const produce = readProduce();
  const types = [];
  if (produce.shots) types.push('shots');
  if (produce.sheet) types.push('sheet');
  if (!types.length) { showBanner('Pick at least one output type.'); return; }

  // Sweep any rows stuck in Running from a previous cancel before building the candidate list
  for (const it of queue.values()) {
    if (it.status === 'Running') queue.update(it.id, { status: 'Cancelled' });
  }
  const candidates = queue.values();
  if (!candidates.length) { showBanner('No files to process.'); return; }

  running = true;
  userCancelled = false;
  refreshActionBar();
  document.getElementById('btn-cancel').disabled = false;
  document.getElementById('status').textContent = '';
  const out = readOutput();
  const statusEl = document.getElementById('status');

  try {
    for (let i = 0; i < types.length; i++) {
      if (userCancelled) break;
      const type = types[i];
      const pretty = type === 'shots' ? 'Screenshots' : 'Contact Sheets';

      for (const it of candidates) {
        queue.update(it.id, { status: 'Pending', progress: null, error: null, outputPath: null });
      }

      statusEl.textContent = types.length > 1
        ? `Pass ${i + 1} of ${types.length} · ${pretty}`
        : `Generating ${pretty.toLowerCase()}`;

      const items = candidates.map(c => ({ id: c.id, path: c.path }));
      if (type === 'shots') {
        await invoke('generate_screenshots', { items, opts: readShotsOpts(), output: out });
      } else {
        await invoke('generate_contact_sheets', { items, opts: readSheetOpts(), output: out });
      }
    }
    if (userCancelled) {
      statusEl.textContent = 'Cancelled';
    } else {
      statusEl.textContent = types.length > 1 ? 'All passes complete' : 'Done';
    }
  } finally {
    // Any row still marked Running must have been interrupted; mark it Cancelled
    for (const it of queue.values()) {
      if (it.status === 'Running') queue.update(it.id, { status: 'Cancelled', progress: null });
    }
    running = false;
    document.getElementById('btn-cancel').disabled = true;
    refreshActionBar();
  }
}

function showBanner(msg) {
  const b = document.getElementById('banner');
  b.textContent = msg;
  b.classList.remove('hidden');
}

function refreshActionBar() {
  const runnable = queue.values().filter(i => i.status !== 'Running');
  const produce = readProduce();
  const gen = document.getElementById('btn-generate');
  const label = gen.querySelector('.gen-label');
  let base;
  if (produce.shots && produce.sheet) base = 'Generate';
  else if (produce.shots) base = 'Generate Screenshots';
  else if (produce.sheet) base = 'Generate Contact Sheets';
  else base = 'Generate';
  label.textContent = runnable.length > 0 ? `${base} (${runnable.length})` : base;
  gen.disabled = running || runnable.length === 0 || (!produce.shots && !produce.sheet) || !toolsOk;
  renderOutputPreview();
}

function renderOutputPreview() {
  const preview = document.getElementById('output-preview');
  if (!preview) return;
  const first = queue.values()[0];
  const produce = readProduce();
  if (!first || (!produce.shots && !produce.sheet)) {
    preview.classList.add('hidden');
    preview.textContent = '';
    return;
  }
  const out = readOutput();
  const dir = out.mode === 'custom' && out.custom ? out.custom : dirname(first.path);
  const stem = basename(first.path).replace(/\.[^./\\]+$/, '');
  const parts = [];
  if (produce.shots) {
    const s = readShotsOpts();
    const ext = s.format === 'Jpeg' ? 'jpg' : 'png';
    const suffix = s.suffix || '_screenshot_';
    parts.push(`${stem}${suffix}01.${ext}`);
  }
  if (produce.sheet) {
    const s = readSheetOpts();
    const ext = s.format === 'Jpeg' ? 'jpg' : 'png';
    const suffix = s.suffix || '_contact_sheet';
    parts.push(`${stem}${suffix}.${ext}`);
  }
  const count = queue.size();
  const firstPath = joinPath(dir, parts[0]);
  const alsoSheet = parts.length > 1 ? ' +' : '';
  const moreFiles = count > 1 ? ` (+${count - 1} more)` : '';
  preview.textContent = `→ ${firstPath}${alsoSheet}${moreFiles}`;
  preview.classList.remove('hidden');
}

function dirname(p) {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(0, i) : '';
}
function basename(p) {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(i + 1) : p;
}
function joinPath(dir, name) {
  if (!dir) return name;
  const sep = dir.includes('\\') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : `${dir}${sep}${name}`;
}

init();
