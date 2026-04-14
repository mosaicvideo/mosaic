export function readSheetOpts() {
  return {
    cols: int('sheet-cols'),
    rows: int('sheet-rows'),
    width: int('sheet-width'),
    gap: int('sheet-gap'),
    thumb_font_size: int('sheet-thumb-font'),
    header_font_size: int('sheet-header-font'),
    show_timestamps: checked('sheet-timestamps'),
    show_header: checked('sheet-header'),
    format: select('sheet-format'),
    jpeg_quality: int('sheet-quality'),
  };
}
export function readShotsOpts() {
  return {
    count: int('shots-count'),
    width: int('shots-width'),
    format: select('shots-format'),
    jpeg_quality: int('shots-quality'),
  };
}
export function readOutput() {
  const mode = document.querySelector('input[name="out"]:checked').value;
  const custom = document.getElementById('custom-folder-path').textContent || null;
  return { mode, custom };
}
export function applyOpts(sheet, shots, out) {
  if (sheet) for (const [k, v] of Object.entries(sheet)) setField(`sheet-${mapKey(k)}`, v);
  if (shots) for (const [k, v] of Object.entries(shots)) setField(`shots-${mapKey(k)}`, v);
  if (out) {
    document.querySelector(`input[name="out"][value="${out.mode}"]`)?.click();
    if (out.custom) document.getElementById('custom-folder-path').textContent = out.custom;
  }
}

function mapKey(k) {
  return { thumb_font_size: 'thumb-font', header_font_size: 'header-font', show_timestamps: 'timestamps', show_header: 'header', jpeg_quality: 'quality' }[k] || k;
}
function int(id) { return parseInt(document.getElementById(id).value, 10); }
function checked(id) { return document.getElementById(id).checked; }
function select(id) { return document.getElementById(id).value; }
function setField(id, v) {
  const el = document.getElementById(id);
  if (!el) return;
  if (el.type === 'checkbox') el.checked = !!v;
  else el.value = v;
}
