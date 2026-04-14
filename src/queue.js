const VIDEO_EXTS = ['mp4','mkv','mov','avi','webm','wmv','flv','m4v','mpg','mpeg','ts','m2ts'];

export function isVideo(path) {
  const m = path.toLowerCase().match(/\.([^./\\]+)$/);
  return !!m && VIDEO_EXTS.includes(m[1]);
}

export function createQueue(root) {
  const items = new Map(); // id -> { id, path, status, error, progress }

  function render() {
    root.innerHTML = '';
    for (const it of items.values()) {
      const li = document.createElement('li');
      const name = document.createElement('span');
      name.textContent = shorten(it.path, 60);
      name.title = it.path;
      const prog = document.createElement('span');
      prog.className = 'progress-label';
      prog.textContent = it.progress || '';
      const badge = document.createElement('span');
      badge.className = `status ${it.status}`;
      badge.textContent = it.status;
      const rm = document.createElement('button');
      rm.className = 'subtle';
      rm.textContent = '×';
      rm.disabled = it.status === 'Running';
      rm.onclick = () => { items.delete(it.id); render(); };
      li.append(name, prog, badge, rm);
      if (it.error) {
        const err = document.createElement('div');
        err.className = 'error';
        err.style.gridColumn = '1 / -1';
        err.style.color = '#ffa0a0';
        err.style.fontSize = '11px';
        err.textContent = it.error;
        li.append(err);
      }
      root.append(li);
    }
  }

  function add(paths) {
    let added = 0;
    for (const p of paths) {
      if (!isVideo(p)) continue;
      if ([...items.values()].some(i => i.path === p)) continue;
      const id = crypto.randomUUID();
      items.set(id, { id, path: p, status: 'Pending' });
      added++;
    }
    if (added) render();
    return added;
  }

  function update(id, patch) {
    const it = items.get(id);
    if (!it) return;
    Object.assign(it, patch);
    render();
  }

  function clear() { items.clear(); render(); }
  function values() { return [...items.values()]; }
  function pending() { return values().filter(i => i.status !== 'Done'); }

  return { add, update, clear, values, pending };
}

function shorten(s, max) {
  if (s.length <= max) return s;
  const half = Math.floor((max - 1) / 2);
  return s.slice(0, half) + '…' + s.slice(-half);
}
