const SHEET_FIELDS = [
  { key: 'cols',             id: 'sheet-cols',        kind: 'int'    },
  { key: 'rows',             id: 'sheet-rows',        kind: 'int'    },
  { key: 'width',            id: 'sheet-width',       kind: 'int'    },
  { key: 'gap',              id: 'sheet-gap',         kind: 'int'    },
  { key: 'thumb_font_size',  id: 'sheet-thumb-font',  kind: 'int'    },
  { key: 'header_font_size', id: 'sheet-header-font', kind: 'int'    },
  { key: 'show_timestamps',  id: 'sheet-timestamps',  kind: 'bool'   },
  { key: 'show_header',      id: 'sheet-header',      kind: 'bool'   },
  { key: 'format',           id: 'sheet-format',      kind: 'select' },
  { key: 'theme',            id: 'sheet-theme',       kind: 'select' },
  { key: 'jpeg_quality',     id: 'sheet-quality',     kind: 'int'    },
  { key: 'suffix',           id: 'sheet-suffix',      kind: 'text'   },
];

const SHOTS_FIELDS = [
  { key: 'count',        id: 'shots-count',   kind: 'int'    },
  { key: 'format',       id: 'shots-format',  kind: 'select' },
  { key: 'jpeg_quality', id: 'shots-quality', kind: 'int'    },
  { key: 'suffix',       id: 'shots-suffix',  kind: 'text'   },
];

const PREVIEW_FIELDS = [
  { key: 'count',            id: 'preview-count',       kind: 'int'    },
  { key: 'clip_length_secs', id: 'preview-clip-length', kind: 'int'    },
  { key: 'height',           id: 'preview-height',      kind: 'int'    },
  { key: 'fps',              id: 'preview-fps',         kind: 'int'    },
  { key: 'format',           id: 'preview-format',      kind: 'select' },
  { key: 'quality',          id: 'preview-quality',     kind: 'int'    },
  { key: 'suffix',           id: 'preview-suffix',      kind: 'text'   },
];

const ASHEET_FIELDS = [
  { key: 'cols',             id: 'asheet-cols',         kind: 'int'  },
  { key: 'rows',             id: 'asheet-rows',         kind: 'int'  },
  { key: 'width',            id: 'asheet-width',        kind: 'int'  },
  { key: 'gap',              id: 'asheet-gap',          kind: 'int'  },
  { key: 'clip_length_secs', id: 'asheet-clip-length',  kind: 'int'  },
  { key: 'fps',              id: 'asheet-fps',          kind: 'int'  },
  { key: 'quality',          id: 'asheet-quality',      kind: 'int'  },
  { key: 'thumb_font_size',  id: 'asheet-thumb-font',   kind: 'int'    },
  { key: 'header_font_size', id: 'asheet-header-font',  kind: 'int'    },
  { key: 'show_timestamps',  id: 'asheet-timestamps',   kind: 'bool'   },
  { key: 'show_header',      id: 'asheet-header',       kind: 'bool'   },
  { key: 'theme',            id: 'asheet-theme',        kind: 'select' },
  { key: 'suffix',           id: 'asheet-suffix',       kind: 'text'   },
];

function readField({ id, kind }) {
  const el = document.getElementById(id);
  if (!el) return undefined;
  if (kind === 'int')    return parseInt(el.value, 10);
  if (kind === 'bool')   return el.checked;
  if (kind === 'select') return el.value;
  if (kind === 'text')   return el.value || '';
  throw new Error(`unknown kind: ${kind}`);
}

function writeField({ id, kind }, v) {
  const el = document.getElementById(id);
  if (!el) return;
  if (kind === 'bool') el.checked = !!v;
  else el.value = v;
}

function readAll(fields) {
  const out = {};
  for (const f of fields) out[f.key] = readField(f);
  return out;
}

function writeAll(fields, data) {
  if (!data) return;
  for (const f of fields) {
    if (Object.prototype.hasOwnProperty.call(data, f.key)) writeField(f, data[f.key]);
  }
}

export function readSheetOpts()   { return readAll(SHEET_FIELDS); }
export function readShotsOpts()   { return readAll(SHOTS_FIELDS); }
export function readPreviewOpts() { return readAll(PREVIEW_FIELDS); }
export function readASheetOpts()  { return readAll(ASHEET_FIELDS); }

export function readOutput() {
  const mode = document.querySelector('input[name="out"]:checked').value;
  const custom = document.getElementById('custom-folder-path').textContent;
  // If "custom" radio is selected but no folder was picked, fall back to NextToSource.
  // Preserves prior silent-fallback UX that the backend used to handle.
  if (mode === 'custom' && custom) return { mode: 'custom', custom };
  return { mode: 'next_to_source' };
}

export const PRODUCE_FIELDS = [
  { key: 'shots',   id: 'prod-shots',   kind: 'bool' },
  { key: 'sheet',   id: 'prod-sheet',   kind: 'bool' },
  { key: 'preview', id: 'prod-preview', kind: 'bool' },
  { key: 'asheet',  id: 'prod-asheet',  kind: 'bool' },
];

export function readProduce()         { return readAll(PRODUCE_FIELDS); }
export function applyProduce(produce) { writeAll(PRODUCE_FIELDS, produce); }

export function updateOutputModeUI() {
  const r = document.querySelector('input[name="out"]:checked');
  document.getElementById('custom-folder-row')
    .classList.toggle('hidden', r?.value !== 'custom');
}

export function applyOpts({ sheet, shots, preview, asheet, out } = {}) {
  writeAll(SHEET_FIELDS,   sheet);
  writeAll(SHOTS_FIELDS,   shots);
  writeAll(PREVIEW_FIELDS, preview);
  writeAll(ASHEET_FIELDS,  asheet);
  if (out) {
    const r = document.querySelector(`input[name="out"][value="${out.mode}"]`);
    if (r) r.checked = true;
    if (out.custom) document.getElementById('custom-folder-path').textContent = out.custom;
    updateOutputModeUI();
  }
}
