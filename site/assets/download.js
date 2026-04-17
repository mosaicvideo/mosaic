// Mosaic showcase — hero contact sheet animation + GitHub-release download links.
// Plain JS, no deps, no build.

const REPO = 'bhutanio/mosaic';
const API = `https://api.github.com/repos/${REPO}/releases/latest`;

const PATTERNS = {
  macos:    /universal\.dmg$/,
  winX64:   /_x64-setup\.exe$/,
  winArm64: /_arm64-setup\.exe$/,
  linux:    /\.AppImage$/,
};

// ────────────────────────────────────────────────────────────────
// Hero "contact sheet" — 12 gradient frames that cycle like a real
// loupe table where a single cell occasionally flickers to a new frame.
// ────────────────────────────────────────────────────────────────

const FRAMES = [
  // 18 cinematic gradients — each cell picks one at random and shuffles.
  'linear-gradient(135deg, #ff6a3d 0%, #c44d44 45%, #2a1c1c 100%)',        // sunset
  'linear-gradient(180deg, #3e5c3e 0%, #1a2818 100%)',                    // forest
  'linear-gradient(160deg, #2d3f7a 0%, #1a1a2e 50%, #0a0a1c 100%)',       // night sky
  'linear-gradient(180deg, #e9a86a 0%, #f3d49b 45%, #8b9cb0 60%, #2d3a4f 100%)', // beach
  'radial-gradient(circle at 50% 40%, #f3e9d2 0%, #a9967a 42%, #2a241c 100%)',  // studio
  'linear-gradient(135deg, #ff3b7a 0%, #2a1438 50%, #0c0b0c 100%)',       // neon
  'linear-gradient(180deg, #0a1a0a 0%, #1a3a1a 60%, #0a1a0a 100%)',        // matrix
  'linear-gradient(180deg, #5f7a9f 0%, #2f3f5f 55%, #161820 100%)',        // blue hour
  'radial-gradient(circle at 30% 70%, #ff6a3d 0%, #c44d44 40%, #2a1412 70%, #0c0b09 100%)', // fire
  'linear-gradient(135deg, #f3e9d2 0%, #2a241c 100%)',                    // mono silver
  'linear-gradient(160deg, #ff2d6f 0%, #2d0a3a 55%, #0c0428 100%)',        // cyberpunk
  'linear-gradient(180deg, #d49669 0%, #a9764a 55%, #2a1814 100%)',        // desert
  'linear-gradient(135deg, #78b3c8 0%, #2e5566 45%, #0d1a22 100%)',        // underwater
  'radial-gradient(circle at 70% 30%, #f0d28a 0%, #c4802a 35%, #3a1e12 100%)', // candlelight
  'linear-gradient(180deg, #1a1a1a 0%, #3a3a3a 50%, #0a0a0a 100%)',        // B&W
  'linear-gradient(160deg, #c97b7b 0%, #5d2a3d 50%, #1a0a14 100%)',        // dusk rose
  'linear-gradient(135deg, #2a4a3a 0%, #1a2f26 60%, #081410 100%)',        // deep forest
  'radial-gradient(circle at 50% 80%, #e9a86a 0%, #5c3820 50%, #0c0b09 100%)', // hearth
];

const CELL_COUNT = 12; // 4×3

// Format seconds as SMPTE timecode HH:MM:SS:FF at 24fps.
function smpte(totalSeconds) {
  const frames = Math.floor((totalSeconds % 1) * 24);
  const s = Math.floor(totalSeconds) % 60;
  const m = Math.floor(totalSeconds / 60) % 60;
  const h = Math.floor(totalSeconds / 3600);
  const pad = (n) => String(n).padStart(2, '0');
  return `${pad(h)}:${pad(m)}:${pad(s)}:${pad(frames)}`;
}

function pickFrame(exclude = -1) {
  let idx;
  do { idx = Math.floor(Math.random() * FRAMES.length); } while (idx === exclude);
  return idx;
}

function buildHeroGrid() {
  const host = document.getElementById('hero-grid');
  if (!host) return;

  const cells = [];
  const seed = 17 + Math.floor(Math.random() * 40); // starting timecode seconds
  const stride = 8.5 + Math.random() * 4;           // seconds between frames

  for (let i = 0; i < CELL_COUNT; i++) {
    const cell = document.createElement('div');
    cell.className = 'frame';
    const gradIdx = pickFrame();
    cell.style.backgroundImage = FRAMES[gradIdx];
    cell.dataset.gradient = gradIdx;

    const label = document.createElement('span');
    label.className = 'frame-label';
    label.textContent = smpte(seed + i * stride);
    cell.appendChild(label);

    const idx = document.createElement('span');
    idx.className = 'frame-index';
    idx.textContent = String(i + 1).padStart(2, '0');
    cell.appendChild(idx);

    host.appendChild(cell);
    cells.push(cell);
  }

  // Every 1.5–3.5s, flash one random cell to a new frame — a darkroom
  // tech flipping through the sheet. Respects reduced-motion.
  const prefersReduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (prefersReduced) return;

  const tick = () => {
    const cell = cells[Math.floor(Math.random() * cells.length)];
    const newGrad = pickFrame(Number(cell.dataset.gradient));
    cell.classList.add('flashing');
    setTimeout(() => {
      cell.style.backgroundImage = FRAMES[newGrad];
      cell.dataset.gradient = newGrad;
      // Bump the timecode forward on the cycled frame.
      const label = cell.querySelector('.frame-label');
      if (label) {
        const base = Math.random() * 180 + 5;
        label.textContent = smpte(base);
      }
    }, 120);
    setTimeout(() => cell.classList.remove('flashing'), 280);

    const delay = 1500 + Math.random() * 2000;
    setTimeout(tick, delay);
  };
  setTimeout(tick, 900);
}

// ────────────────────────────────────────────────────────────────
// OS detection — highlights the relevant download button.
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
  const primaryId = {
    macos: 'btn-macos',
    winX64: 'btn-win',
    winArm64: 'btn-win',
    linux: 'btn-linux',
  }[os];
  const btn = document.getElementById(primaryId);
  if (btn) btn.classList.add('primary');
}

// ────────────────────────────────────────────────────────────────
// Release fetch — rewrite button hrefs to direct-download URLs,
// populate version badge. Falls back to /releases/latest on failure.
// ────────────────────────────────────────────────────────────────

async function upgradeDownloads() {
  let release;
  try {
    const res = await fetch(API, { headers: { 'Accept': 'application/vnd.github+json' } });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    release = await res.json();
  } catch (e) {
    console.warn('[mosaic] release API unavailable, falling back to releases page:', e);
    return;
  }
  if (!release.assets) return;

  const find = (pattern) => release.assets.find(a => pattern.test(a.name))?.browser_download_url;

  const mac = find(PATTERNS.macos);
  const winX64 = find(PATTERNS.winX64);
  const winArm64 = find(PATTERNS.winArm64);
  const linux = find(PATTERNS.linux);

  const set = (id, url) => { const el = document.getElementById(id); if (el && url) el.href = url; };
  set('btn-macos', mac);
  set('btn-win', winX64);
  set('btn-linux', linux);

  const armLink = document.getElementById('win-arm-link');
  if (armLink && winArm64) { armLink.href = winArm64; armLink.hidden = false; }

  const version = release.tag_name?.replace(/^v/, '');
  const badge = document.getElementById('version-badge');
  if (badge && version) badge.textContent = `v${version}`;
  const navVer = document.getElementById('nav-version');
  if (navVer && version) navVer.textContent = `v${version}`;
}

// ────────────────────────────────────────────────────────────────
// Missing-image fallback — hide gracefully rather than show broken icons.
// ────────────────────────────────────────────────────────────────

function hideBrokenImages() {
  document.querySelectorAll('img.optional-screenshot').forEach(img => {
    img.addEventListener('error', () => {
      const parent = img.closest('.showcase-frame, .showcase, .output-type-media, figure');
      if (parent) parent.style.display = 'none';
      else img.style.display = 'none';
    });
  });
}

function init() {
  buildHeroGrid();
  highlightOS();
  upgradeDownloads();
  hideBrokenImages();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
