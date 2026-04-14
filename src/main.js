import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Store } from '@tauri-apps/plugin-store';
import { createQueue, isVideo, VIDEO_EXTS } from './queue.js';
import { readSheetOpts, readShotsOpts, readOutput, applyOpts } from './options.js';
import { wireDropzone } from './dropzone.js';

const queue = createQueue(document.getElementById('queue'));
let activeTab = 'sheet';
let store;
let saveTimer = null;

async function init() {
  store = await Store.load('settings.json');
  const saved = {
    sheet: await store.get('sheet'),
    shots: await store.get('shots'),
    out: await store.get('out'),
    activeTab: await store.get('activeTab') || 'sheet',
  };
  applyOpts(saved.sheet, saved.shots, saved.out);
  switchTab(saved.activeTab);

  try { await invoke('check_tools'); }
  catch (e) { showBanner(`ffmpeg/ffprobe not found on PATH. Install with: brew install ffmpeg  (macOS) / winget install ffmpeg (Windows) / apt install ffmpeg (Linux). ${e}`); }

  wireButtons();
  wireDropzone(document.getElementById('dropzone'), addPaths);
  wireEvents();
}

function wireButtons() {
  document.getElementById('btn-add-files').onclick = async () => {
    const picked = await open({ multiple: true, filters: [{ name: 'Videos', extensions: VIDEO_EXTS }] });
    if (!picked) return;
    addPaths(Array.isArray(picked) ? picked : [picked]);
  };
  document.getElementById('btn-add-folder').onclick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (!dir) return;
    // Walk recursively by asking the backend? For v1 keep it simple: only accept files;
    // users can drag-drop folders and Tauri's drop event lists file paths only.
    // Add a single-level scan using Tauri's FS plugin later if needed.
    addPaths([dir]); // just attempt — isVideo filter drops non-videos
  };
  document.getElementById('btn-clear').onclick = () => queue.clear();
  document.getElementById('btn-generate').onclick = onGenerate;
  document.getElementById('btn-cancel').onclick = () => invoke('cancel_job');
  document.getElementById('btn-pick-folder').onclick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (dir) document.getElementById('custom-folder-path').textContent = dir;
  };
  document.querySelectorAll('.tab').forEach(b => b.onclick = () => switchTab(b.dataset.tab));
  document.querySelectorAll('input[name="out"]').forEach(r => r.onchange = () => {
    document.getElementById('custom-folder-row').classList.toggle('hidden', r.value !== 'custom' || !r.checked);
  });
  document.querySelectorAll('#options input, #options select').forEach(el => el.onchange = saveSettings);
}

function switchTab(name) {
  activeTab = name;
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === name));
  document.querySelectorAll('.tab-panel').forEach(p => p.classList.toggle('active', p.dataset.panel === name));
  document.getElementById('btn-generate').textContent =
    name === 'sheet' ? 'Generate Contact Sheets' : 'Generate Screenshots';
  saveSettings();
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
  await store.set('activeTab', activeTab);
  await store.save();
}

async function addPaths(paths) {
  const vids = paths.filter(isVideo);
  const added = queue.add(vids);
  if (!added.length) return;
  // Probe only the newly-added items to fill in duration/resolution
  // (not strictly needed for generation).
  for (const it of added) {
    try {
      const info = await invoke('probe_video', { path: it.path });
      queue.update(it.id, { probed: true, info });
    } catch (_) { /* keep Pending; errors will surface at generation */ }
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
    queue.update(payload.fileId, { status: 'Done', progress: 'Done' });
  });
  listen('job:file-failed', ({ payload }) => {
    queue.update(payload.fileId, { status: 'Failed', error: payload.error });
  });
  listen('job:finished', ({ payload }) => {
    document.getElementById('btn-generate').disabled = false;
    document.getElementById('btn-cancel').disabled = true;
    document.getElementById('status').textContent =
      `Done: ${payload.completed} ok, ${payload.failed} failed, ${payload.cancelled} cancelled.`;
  });
}

function updateOverall(done, total) {
  const p = document.getElementById('progress');
  p.max = total; p.value = done;
}

async function onGenerate() {
  const items = queue.values()
    .filter(i => i.status === 'Pending' || i.status === 'Failed' || i.status === 'Cancelled')
    .map(i => ({ id: i.id, path: i.path }));
  if (!items.length) { showBanner('No files in queue.'); return; }
  document.getElementById('btn-generate').disabled = true;
  document.getElementById('btn-cancel').disabled = false;
  document.getElementById('status').textContent = '';
  const out = readOutput();
  if (activeTab === 'sheet') {
    await invoke('generate_contact_sheets', { items, opts: readSheetOpts(), output: out });
  } else {
    await invoke('generate_screenshots', { items, opts: readShotsOpts(), output: out });
  }
}

function showBanner(msg) {
  const b = document.getElementById('banner');
  b.textContent = msg;
  b.classList.remove('hidden');
}

init();
