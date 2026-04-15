import { invoke } from '@tauri-apps/api/core';

// Resolved once on first access; backend owns the canonical list.
let videoExtsPromise = null;
export function getVideoExts() {
  if (!videoExtsPromise) videoExtsPromise = invoke('get_video_exts');
  return videoExtsPromise;
}

export async function isVideo(path) {
  const exts = await getVideoExts();
  const m = path.toLowerCase().match(/\.([^./\\]+)$/);
  return !!m && exts.includes(m[1]);
}

export function createQueue(root, { onReveal, onChange } = {}) {
  const items = new Map();
  const nodes = new Map();

  function reindex() {
    let n = 1;
    for (const id of items.keys()) {
      const node = nodes.get(id);
      if (node) node.idxEl.textContent = String(n).padStart(2, '0');
      n++;
    }
  }

  function formatMeta(info) {
    if (!info) return '';
    const s = Math.floor(info.duration_secs || 0);
    const hh = Math.floor(s / 3600);
    const mm = Math.floor((s % 3600) / 60);
    const ss = s % 60;
    const t = hh > 0
      ? `${hh}:${String(mm).padStart(2,'0')}:${String(ss).padStart(2,'0')}`
      : `${mm}:${String(ss).padStart(2,'0')}`;
    const v = info.video || {};
    const res = v.width && v.height ? `${v.width}×${v.height}` : '';
    return res ? `${t} · ${res}` : t;
  }

  function buildRow(it, index) {
    const el = document.createElement('li');

    const idxEl = document.createElement('span');
    idxEl.className = 'idx';
    idxEl.textContent = String(index).padStart(2, '0');

    const nameCell = document.createElement('div');
    nameCell.className = 'name-cell';
    const nameEl = document.createElement('span');
    nameEl.className = 'name';
    nameEl.textContent = basename(it.path);
    nameEl.title = it.path;
    const metaEl = document.createElement('span');
    metaEl.className = 'meta';
    metaEl.textContent = formatMeta(it.info);
    nameCell.append(nameEl, metaEl);

    const progEl = document.createElement('span');
    progEl.className = 'prog';
    progEl.textContent = it.progress || '—';

    const statusCell = document.createElement('span');
    statusCell.className = `status-cell ${it.status}`;
    const led = document.createElement('span');
    led.className = 'led';
    const statusLabel = document.createElement('span');
    statusLabel.className = 'status-label';
    statusLabel.textContent = it.status;
    statusCell.append(led, statusLabel);

    const removeBtn = document.createElement('button');
    removeBtn.className = 'row-remove';
    removeBtn.textContent = '×';
    removeBtn.title = 'Remove';
    removeBtn.disabled = it.status === 'Running';
    removeBtn.onclick = (e) => {
      e.stopPropagation();
      items.delete(it.id);
      const n = nodes.get(it.id);
      if (n) { n.el.remove(); nodes.delete(it.id); }
      reindex();
      onChange?.();
    };

    el.append(idxEl, nameCell, progEl, statusCell, removeBtn);

    el.onclick = () => {
      const cur = items.get(it.id);
      if (cur?.status === 'Done' && cur.outputPath) onReveal?.(cur.outputPath);
    };

    return { el, idxEl, nameEl, metaEl, progEl, statusCell, statusLabel, removeBtn, errorEl: null };
  }

  function add(paths) {
    const existing = new Set([...items.values()].map(i => i.path));
    const added = [];
    for (const p of paths) {
      if (existing.has(p)) continue;
      const id = crypto.randomUUID();
      const it = { id, path: p, status: 'Pending' };
      items.set(id, it);
      existing.add(p);
      const n = buildRow(it, items.size);
      nodes.set(id, n);
      root.append(n.el);
      added.push({ id, path: p });
    }
    if (added.length) onChange?.();
    return added;
  }

  function update(id, patch) {
    const it = items.get(id);
    if (!it) return;
    const before = { status: it.status, progress: it.progress, error: it.error, info: it.info };
    Object.assign(it, patch);
    const n = nodes.get(id);
    if (!n) return;
    if (it.status !== before.status) {
      n.statusCell.className = `status-cell ${it.status}`;
      n.statusLabel.textContent = it.status;
      n.removeBtn.disabled = it.status === 'Running';
      n.el.classList.toggle('revealable', it.status === 'Done' && !!it.outputPath);
    }
    if (it.outputPath && it.status === 'Done') {
      n.el.classList.add('revealable');
    }
    if ((it.progress || '') !== (before.progress || '')) {
      n.progEl.textContent = it.progress || '—';
    }
    if (it.info !== before.info) {
      n.metaEl.textContent = formatMeta(it.info);
    }
    if ((it.error || null) !== (before.error || null)) {
      if (it.error) {
        if (!n.errorEl) {
          const err = document.createElement('div');
          err.className = 'row-error';
          n.el.append(err);
          n.errorEl = err;
        }
        n.errorEl.textContent = it.error;
      } else if (n.errorEl) {
        n.errorEl.remove();
        n.errorEl = null;
      }
    }
  }

  function clear() {
    items.clear();
    nodes.clear();
    root.innerHTML = '';
    onChange?.();
  }
  function values() { return [...items.values()]; }
  function pending() { return values().filter(i => i.status !== 'Done'); }
  function size() { return items.size; }

  return { add, update, clear, values, pending, size };
}

function basename(p) {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(i + 1) : p;
}
