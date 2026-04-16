#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

const PACKAGE_JSON = resolve(root, "package.json");
const TAURI_CONF = resolve(root, "src-tauri/tauri.conf.json");
const CARGO_TOML = resolve(root, "src-tauri/Cargo.toml");
const CARGO_LOCK = resolve(root, "src-tauri/Cargo.lock");

const SEMVER_RE = /^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$/;

const version = process.argv[2];
const shouldTag = process.argv.includes("--tag");

if (!version) {
  console.error("Usage: node scripts/bump-version.mjs <version> [--tag]");
  console.error("  <version>  Semver string, e.g. 0.2.0 or 1.0.0-rc1");
  console.error("  --tag      Git add, commit, and tag after updating files");
  process.exit(1);
}

if (!SEMVER_RE.test(version)) {
  console.error(`Invalid semver: "${version}". Expected format: X.Y.Z or X.Y.Z-pre`);
  process.exit(1);
}

// Update package.json
const pkg = JSON.parse(readFileSync(PACKAGE_JSON, "utf8"));
pkg.version = version;
writeFileSync(PACKAGE_JSON, JSON.stringify(pkg, null, 2) + "\n");
console.log(`  package.json → ${version}`);

// Update tauri.conf.json
const tauri = JSON.parse(readFileSync(TAURI_CONF, "utf8"));
tauri.version = version;
writeFileSync(TAURI_CONF, JSON.stringify(tauri, null, 2) + "\n");
console.log(`  tauri.conf.json → ${version}`);

// Update Cargo.toml (replace version line in [package] section only)
let cargo = readFileSync(CARGO_TOML, "utf8");
const pkgIdx = cargo.indexOf("[package]");
const nextSection = cargo.indexOf("\n[", pkgIdx + 1);
const pkgSection = cargo.substring(pkgIdx, nextSection === -1 ? undefined : nextSection);
const updatedSection = pkgSection.replace(/^version = ".*"$/m, `version = "${version}"`);
cargo = cargo.substring(0, pkgIdx) + updatedSection + (nextSection === -1 ? "" : cargo.substring(nextSection));
writeFileSync(CARGO_TOML, cargo);
console.log(`  Cargo.toml → ${version}`);

// Update Cargo.lock so it stays in sync (no compilation needed)
execSync("cargo generate-lockfile", { cwd: resolve(root, "src-tauri"), stdio: "inherit" });
console.log(`  Cargo.lock updated`);

console.log(`\nVersion bumped to ${version}`);

if (shouldTag) {
  const files = [PACKAGE_JSON, TAURI_CONF, CARGO_TOML, CARGO_LOCK].map(
    (f) => relative(root, f)
  );
  execSync(`git add ${files.join(" ")}`, { cwd: root, stdio: "inherit" });
  let hasStaged = false;
  try { execSync("git diff --cached --quiet", { cwd: root, stdio: "ignore" }); } catch { hasStaged = true; }
  if (hasStaged) {
    execSync(`git commit -m "chore: bump version to ${version}"`, {
      cwd: root,
      stdio: "inherit",
    });
  } else {
    console.log("  No changes to commit (version already current)");
  }
  execSync(`git tag -f v${version}`, { cwd: root, stdio: "inherit" });
  console.log(`Created tag v${version}`);
}
