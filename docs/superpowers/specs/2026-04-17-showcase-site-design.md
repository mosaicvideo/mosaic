# Showcase Site — Design

**Status:** Shipped in v0.1.2 across commits `39b02a3` (initial darkroom-loupe design, 2026-04-17) and `fc7831f` (rework as terminal archive with real screenshots, 2026-04-17).
**Scope:** Static marketing + user-guide site deployed to GitHub Pages, intentionally decoupled from the app's release cycle.

## Goal

A small static site that surfaces Mosaic to first-time visitors, hosts the user guide, and exposes platform-appropriate download links. No build step, no framework, no app coupling.

## Layout

```
site/
  index.html               — landing page
  guide.html               — full user guide (requirements, outputs, FAQ)
  assets/
    style.css              — single-file CSS, terminal-archive aesthetic
    download.js            — runtime release-metadata fetch (see below)
    favicon.png
    icon.png
    screenshots/           — real app screenshots + sample outputs
```

No Vite / bundler / templating engine. Two `.html` files, one `.css`, one `.js`. Intentional — this is infrastructure that must keep working without maintenance.

Deployed URL: `https://mosaicvideo.github.io/mosaic/`.

## Deployment

`.github/workflows/pages.yml`:

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'site/**'
      - '.github/workflows/pages.yml'
  workflow_dispatch:
```

Only fires on pushes that touch the site — tag pushes (which trigger app releases) don't redeploy the site, and app-code changes don't redeploy unnecessarily. Trigger manually via `workflow_dispatch` if needed.

Uses the standard `actions/configure-pages@v5 → upload-pages-artifact@v3 → deploy-pages@v4` pipeline with `path: site`.

## Runtime release metadata

`site/assets/download.js` upgrades static fallback links using the GitHub Releases API at page-load time:

1. Fetches `https://api.github.com/repos/mosaicvideo/mosaic/releases/latest` (no auth — 60 reqs/hr/IP).
2. Extracts `tag_name`, asset URLs, asset sizes, and release title.
3. Rewrites the DOM of three specific ids:
   - `#version-badge` → `v0.1.3`
   - `#nav-version` → `v0.1.3`
   - `#line-version` → `v0.1.3`
4. Rewrites download button `href`s + asset-size labels for macOS / Windows x64 / Windows ARM64 / Linux rows.

Without JS (or on API failure), the buttons fall back to `releases/latest` (GitHub's canonical redirect to the newest release) — the site stays functional, just less precise.

## Version-mention strategy

Hardcoded references to versions in HTML copy are deliberately kept only where the fact is **historical**:

- "auto-updates from v0.1.2 on" (factual — the updater shipped in v0.1.2).
- "v0.1.1 and earlier: manual download required" (factual — predates the updater).

Version strings that describe the **current** release live in the three dynamic DOM ids above. If any copy needs to read "vCURRENT", it should get an id and be added to `download.js` — not hardcoded.

## Preview locally

```
cd site && python3 -m http.server 8000
```

No hot reload; edit + refresh. Acceptable trade for zero-build simplicity.

## Decoupling invariants

- Site copy updates must **never** require an app version bump. The site lives entirely inside `site/` + `.github/workflows/pages.yml`; no app tests or build pipelines touch it.
- App version bumps **do not** automatically redeploy the site. If a release changes something user-visible that the guide documents (install commands, system requirements, FAQ answers), the site has to be updated as a separate commit.
- The download buttons read from the Releases API, so a newly-published draft release on GitHub becomes downloadable from the site the moment the user publishes, with no site redeploy.

## Scope exclusions

- No analytics / tracking.
- No CDN layer — GitHub Pages serves directly.
- No dark-mode toggle — the terminal aesthetic is dark-by-design; light mode is not a goal.
- No search / index — the guide is one page with anchor links.
- No translations — English only.
- No comments system / contact form — GitHub Issues is the contact channel.
