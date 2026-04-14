export const VIDEO_EXTS = ['mp4','mkv','mov','avi','webm','wmv','flv','m4v','mpg','mpeg','ts','m2ts'];

export function isVideo(path) {
  const m = path.toLowerCase().match(/\.([^./\\]+)$/);
  return !!m && VIDEO_EXTS.includes(m[1]);
}

export function createQueue(root) {
  const items = new Map(); // id -> { id, path, status, error, progress, probed?, info? }
  const nodes = new Map(); // id -> { el, nameEl, progEl, badgeEl, removeBtn, errorEl }

  function buildRow(it) {
    const el = document.createElement('li');
    const nameEl = document.createElement('span');
    nameEl.textContent = shorten(it.path, 60);
    nameEl.title = it.path;
    const progEl = document.createElement('span');
    progEl.className = 'progress-label';
    progEl.textContent = it.progress || '';
    const badgeEl = document.createElement('span');
    badgeEl.className = `status ${it.status}`;
    badgeEl.textContent = it.status;
    const removeBtn = document.createElement('button');
    removeBtn.className = 'subtle';
    removeBtn.textContent = '×';
    removeBtn.disabled = it.status === 'Running';
    removeBtn.onclick = () => {
      items.delete(it.id);
      const n = nodes.get(it.id);
      if (n) { n.el.remove(); nodes.delete(it.id); }
    };
    el.append(nameEl, progEl, badgeEl, removeBtn);
    return { el, nameEl, progEl, badgeEl, removeBtn, errorEl: null };
  }

  function add(paths) {
    const existing = new Set([...items.values()].map(i => i.path));
    const added = [];
    for (const p of paths) {
      if (!isVideo(p)) continue;
      if (existing.has(p)) continue;
      const id = crypto.randomUUID();
      const it = { id, path: p, status: 'Pending' };
      items.set(id, it);
      existing.add(p);
      const n = buildRow(it);
      nodes.set(id, n);
      root.append(n.el);
      added.push({ id, path: p });
    }
    return added;
  }

  function update(id, patch) {
    const it = items.get(id);
    if (!it) return;
    const before = { status: it.status, progress: it.progress, error: it.error };
    Object.assign(it, patch);
    const n = nodes.get(id);
    if (!n) return;
    if (it.status !== before.status) {
      n.badgeEl.className = `status ${it.status}`;
      n.badgeEl.textContent = it.status;
      n.removeBtn.disabled = it.status === 'Running';
    }
    if ((it.progress || '') !== (before.progress || '')) {
      n.progEl.textContent = it.progress || '';
    }
    if ((it.error || null) !== (before.error || null)) {
      if (it.error) {
        if (!n.errorEl) {
          const err = document.createElement('div');
          err.className = 'error';
          err.style.gridColumn = '1 / -1';
          err.style.color = '#ffa0a0';
          err.style.fontSize = '11px';
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
  }
  function values() { return [...items.values()]; }
  function pending() { return values().filter(i => i.status !== 'Done'); }

  return { add, update, clear, values, pending };
}

function shorten(s, max) {
  if (s.length <= max) return s;
  const half = Math.floor((max - 1) / 2);
  return s.slice(0, half) + '…' + s.slice(-half);
}
