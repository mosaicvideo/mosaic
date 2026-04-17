// Mosaic showcase — lightweight upgrade layer.
// The terminal content renders statically in HTML; this script fills
// in real release URLs, asset sizes, version strings, and highlights
// the row that matches the visitor's OS. Fails silently so the static
// fallback always works.

const REPO = 'mosaicvideo/mosaic';
const API = `https://api.github.com/repos/${REPO}/releases/latest`;

const PATTERNS = {
  macos:    /universal\.dmg$/,
  winX64:   /_x64-setup\.exe$/,
  winArm64: /_arm64-setup\.exe$/,
  linux:    /\.AppImage$/,
};

// ────────────────────────────────────────────────────────────────
// OS detection — adds .primary to the matching row.
// ────────────────────────────────────────────────────────────────

function detectOS() {
  const ua = (navigator.userAgent || '').toLowerCase();
  const plat = (navigator.userAgentData?.platform || navigator.platform || '').toLowerCase();
  if (/mac|darwin/.test(plat) || /mac os/.test(ua)) return 'macos';
  if (/win/.test(plat) || /windows/.test(ua)) {
    if (/arm64|aarch64/.test(ua)) return 'winArm64';
    return 'winX64';
  }
  if (/linux/.test(plat) || /linux/.test(ua)) return 'linux';
  return null;
}

function highlightOS() {
  const os = detectOS();
  if (!os) return;
  const key = {
    macos: 'macos',
    winX64: 'win-x64',
    winArm64: 'win-arm',
    linux: 'linux',
  }[os];
  const row = document.querySelector(`.dl-row[data-key="${key}"]`);
  if (row) row.classList.add('primary');
}

// ────────────────────────────────────────────────────────────────
// Release upgrade — real URLs, sizes, version string.
// ────────────────────────────────────────────────────────────────

function humanSize(bytes) {
  if (!bytes && bytes !== 0) return '--';
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(1)} MB`;
}

function setRow(id, asset) {
  const anchor = document.getElementById(id);
  if (!anchor || !asset) return;
  anchor.href = asset.browser_download_url;
  const pathCell = anchor.querySelector('.dl-path');
  const sizeCell = anchor.querySelector('.dl-size');
  if (pathCell) pathCell.textContent = asset.name;
  if (sizeCell) sizeCell.textContent = humanSize(asset.size);
}

async function upgrade() {
  let release;
  try {
    const res = await fetch(API, { headers: { 'Accept': 'application/vnd.github+json' } });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    release = await res.json();
  } catch (e) {
    console.warn('[mosaic] release API unavailable:', e);
    return;
  }
  if (!release.assets) return;

  const find = (p) => release.assets.find(a => p.test(a.name));
  setRow('btn-macos',   find(PATTERNS.macos));
  setRow('btn-win',     find(PATTERNS.winX64));
  setRow('btn-win-arm', find(PATTERNS.winArm64));
  setRow('btn-linux',   find(PATTERNS.linux));

  const version = release.tag_name?.replace(/^v/, '');
  if (!version) return;
  const setText = (id, text) => { const n = document.getElementById(id); if (n) n.textContent = text; };
  setText('version-badge', `v${version}`);
  setText('nav-version', `v${version}`);
  setText('line-version', `v${version}`);

  // Point CTA "download" button at the matched OS asset if we have one.
  const os = detectOS();
  const osAsset = {
    macos: find(PATTERNS.macos),
    winX64: find(PATTERNS.winX64),
    winArm64: find(PATTERNS.winArm64),
    linux: find(PATTERNS.linux),
  }[os];
  if (osAsset) {
    const primaryCta = document.querySelector('.cta.primary');
    if (primaryCta) primaryCta.href = osAsset.browser_download_url;
  }
}

// ────────────────────────────────────────────────────────────────
// Hide missing optional screenshots cleanly.
// ────────────────────────────────────────────────────────────────

function hideBrokenImages() {
  document.querySelectorAll('img.optional-screenshot').forEach(img => {
    img.addEventListener('error', () => {
      const parent = img.closest('.showcase, .output-type-media, figure');
      if (parent) parent.style.display = 'none';
      else img.style.display = 'none';
    });
  });
}

function init() {
  hideBrokenImages();
  highlightOS();
  upgrade();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
