// scripts/sync-defaults.mjs
// Reads src-tauri/src/defaults.rs and rewrites the `value="…"` attributes
// in src/index.html so GUI and CLI share the same shipping defaults.
// Run via `pnpm sync:defaults`. Also invoked by scripts/bump-version.mjs.

import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const rs = readFileSync(resolve(root, "src-tauri/src/defaults.rs"), "utf8");
const htmlPath = resolve(root, "src/index.html");
let html = readFileSync(htmlPath, "utf8");

// Map of (section, key) -> HTML input id.
// Keep in alphabetical order by section, mirrors defaults.rs groups.
const map = {
  "screenshots.COUNT":         "shots-count",
  "screenshots.JPEG_QUALITY":  "shots-quality",
  "sheet.COLS":                "sheet-cols",
  "sheet.ROWS":                "sheet-rows",
  "sheet.WIDTH":               "sheet-width",
  "sheet.GAP":                 "sheet-gap",
  "sheet.THUMB_FONT":          "sheet-thumb-font",
  "sheet.HEADER_FONT":         "sheet-header-font",
  "sheet.JPEG_QUALITY":        "sheet-quality",
  "reel.COUNT":                "preview-count",
  "reel.CLIP_LENGTH_SECS":     "preview-clip-length",
  "reel.HEIGHT":               "preview-height",
  "reel.FPS":                  "preview-fps",
  "reel.QUALITY":              "preview-quality",
  "animated_sheet.COLS":       "asheet-cols",
  "animated_sheet.ROWS":       "asheet-rows",
  "animated_sheet.WIDTH":      "asheet-width",
  "animated_sheet.GAP":        "asheet-gap",
  "animated_sheet.CLIP_LENGTH_SECS": "asheet-clip-length",
  "animated_sheet.FPS":        "asheet-fps",
  "animated_sheet.QUALITY":    "asheet-quality",
  "animated_sheet.THUMB_FONT": "asheet-thumb-font",
  "animated_sheet.HEADER_FONT":"asheet-header-font",
};

// Only numeric constants (u32/i32/etc.) — &str constants aren't handled
// because the HTML uses <select> elements for those.
function extract(section, key) {
  // Anchor `pub mod <section>` to column 0 (all module declarations in
  // defaults.rs start there) so `pub mod sheet` doesn't match inside
  // `pub mod animated_sheet`.
  const re = new RegExp(
    `(?:^|\\n)pub mod ${section}\\s*\\{[\\s\\S]*?pub const ${key}: [a-zA-Z0-9_]+ = (-?\\d+);`,
  );
  const m = rs.match(re);
  if (!m) throw new Error(`defaults.rs: could not find ${section}::${key}`);
  return m[1];
}

let changed = 0;
for (const [qualified, id] of Object.entries(map)) {
  const [section, key] = qualified.split(".");
  const value = extract(section, key);
  const re = new RegExp(`(id="${id}"[^>]*\\svalue=")[^"]*(")`);
  if (!re.test(html)) throw new Error(`index.html: no input with id="${id}"`);
  const next = html.replace(re, `$1${value}$2`);
  if (next !== html) { changed++; html = next; }
}

if (changed === 0) {
    console.log("sync-defaults: no changes");
    process.exit(0);
}

writeFileSync(htmlPath, html);
console.log(`sync-defaults: updated ${changed} attribute(s) in src/index.html`);
