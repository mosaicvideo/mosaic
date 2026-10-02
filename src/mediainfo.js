import { invoke } from '@tauri-apps/api/core';
import { basename } from './queue.js';

let modal, title, pre, copyBtn, closeBtn, copyTimer;
let requestSeq = 0;

export function createMediaInfoModal() {
  modal = document.createElement('div');
  modal.id = 'mediainfo-modal';
  modal.className = 'hidden';

  const backdrop = document.createElement('div');
  backdrop.className = 'modal-backdrop';
  backdrop.onclick = closeMediaInfo;

  const container = document.createElement('div');
  container.className = 'modal-container';
  container.setAttribute('role', 'dialog');
  container.setAttribute('aria-modal', 'true');
  container.setAttribute('aria-labelledby', 'mediainfo-title');

  const header = document.createElement('div');
  header.className = 'modal-header';

  title = document.createElement('span');
  title.className = 'modal-title';
  title.id = 'mediainfo-title';

  copyBtn = document.createElement('button');
  copyBtn.className = 'secondary small';
  copyBtn.textContent = 'Copy';
  copyBtn.onclick = onCopy;

  closeBtn = document.createElement('button');
  closeBtn.className = 'modal-close';
  closeBtn.textContent = '\u00d7';
  closeBtn.title = 'Close';
  closeBtn.onclick = closeMediaInfo;

  header.append(title, copyBtn, closeBtn);

  const body = document.createElement('div');
  body.className = 'modal-body';

  pre = document.createElement('pre');
  body.append(pre);

  container.append(header, body);
  modal.append(backdrop, container);
  document.getElementById('app').append(modal);
}

export async function openMediaInfo(path) {
  if (!modal) return;
  const name = basename(path);
  title.textContent = name;
  pre.textContent = 'Loading\u2026';
  copyBtn.disabled = true;
  copyBtn.textContent = 'Copy';
  clearTimeout(copyTimer);
  modal.classList.remove('hidden');
  closeBtn.focus();

  const seq = ++requestSeq;
  try {
    const text = await invoke('run_mediainfo', { path });
    if (seq !== requestSeq) return;
    pre.textContent = text;
    copyBtn.disabled = false;
  } catch (err) {
    if (seq !== requestSeq) return;
    pre.textContent = typeof err === 'string' ? err : err?.message || 'Unknown error';
    copyBtn.disabled = true;
  }
}

export function closeMediaInfo() {
  if (modal) modal.classList.add('hidden');
}

export function isMediaInfoOpen() {
  return modal && !modal.classList.contains('hidden');
}

function onCopy() {
  const text = pre?.textContent;
  if (!text) return;
  navigator.clipboard.writeText(text).then(() => {
    copyBtn.textContent = 'Copied!';
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => { copyBtn.textContent = 'Copy'; }, 1500);
  });
}
