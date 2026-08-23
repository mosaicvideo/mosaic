# Mosaic — SEO & AEO plan

**Date:** 2026-08-23
**Site:** https://mosaicvideo.github.io/mosaic/
**Status:** analysis + plan, nothing implemented

---

## 1. The honest diagnosis

The site is not missing from Google because of meta tags. It is missing because
**it is four months old, has ~zero inbound links, and sits on a brand-new
hostname.**

| Signal | Value |
|---|---|
| Repo created | 2026-04-14 (~4 months) |
| GitHub stars | 2 |
| Inbound links | effectively none found |
| `robots.txt` | 404 |
| `sitemap.xml` | 404 |
| Search Console | not verified (assumed) |
| Homepage visible words | **396** |
| Homepage `<h1>` | **none** |
| JSON-LD structured data | **none on any page** |
| `canonical` / `og:url` / `twitter:card` | **none on any page** |
| `og:image` | relative path — invalid |

`github.io` is on the Public Suffix List, so Google treats
`mosaicvideo.github.io` as its own registrable site. That is good for isolation
but it means the host inherits **no** authority from GitHub. It starts from zero,
exactly like a domain registered in April.

The page *is* in at least one non-Google index (it surfaced on a `site:` query
during research), so nothing is actively blocking crawlers. There is no
`X-Robots-Tag` on the response and the deployed HTML matches the repo. This is a
discovery-and-authority problem, not a blocking problem.

**Implication:** the technical fixes in Phase 1–2 below are necessary but not
sufficient. They cost a few hours and remove every excuse Google has. The
rankings come from Phase 4 and Phase 5.

### The name problem

"Mosaic" is one of the most contested nouns in computing — NCSA Mosaic, mosaic
art, mosaic tiling, plus **an existing Homebrew cask named `mosaic`** (a window
manager from lightpillar.com — verified, `brew info --cask mosaic` resolves to
it). You will never rank for "mosaic" and you cannot claim the obvious cask
token.

Every ranking win has to come from **long-tail intent phrases**, not the brand:

- `video contact sheet generator`
- `batch video thumbnail app macOS`
- `vcsi alternative GUI`
- `animated contact sheet ffmpeg`
- `make contact sheet from video windows`
- `ffmpeg thumbnail grid tool`

> **Note on keyword volumes:** Semrush was queried but the account is out of API
> units, so none of these have validated search volume attached. Top up units at
> https://www.semrush.com/mcp-access to attach real numbers before committing to
> a content order.

### The competitive gap (this is the good news)

Searching the space turns up: `vcsi` (Python CLI, the incumbent), `Thumbnailer`,
`MTN/movie-thumbnailer`, `vcs` (bash), `ffmpegthumbs` (KDE), and a scatter of
gists and one-off Python scripts.

**Every serious option is CLI-only or Linux-only.** There is no well-known,
signed, auto-updating, cross-platform GUI. That is Mosaic's actual wedge and it
should be the spine of the content strategy — not "contact sheet tool", but
"the contact sheet tool you don't have to be a terminal user to run".

---

## 2. Answer to: can the site live at `mosaicvideo.github.io` root?

**Yes.** Create a repo named exactly `mosaicvideo/mosaicvideo.github.io` and
enable Pages on it. GitHub serves org-pages repos at the org root. It coexists
with project pages — `mosaicvideo.github.io/` and
`mosaicvideo.github.io/mosaic/` can both be live at once.

Right now the root **404s** because that repo doesn't exist (the org has only
`mosaic` and a private `bdinfo-rust`).

**But rank it correctly: this is a low-SEO-impact change.** Root vs. subfolder on
the same host is not a meaningful ranking factor for Google. The real reasons to
do it are cosmetic and citation-shaped — a shorter URL is what AI answer engines
print, and a 404 at your own root looks unfinished.

The one genuine cost: `https://mosaicvideo.github.io/mosaic/install.sh` is baked
into the site copy, the README, and any blog post or comment anyone has ever
pasted. GitHub Pages has no redirect config, so those URLs must keep working.

**Recommended shape if you do it:**

1. New repo `mosaicvideo.github.io` holds the marketing site → serves at root.
2. `mosaic/site/` keeps deploying `install.sh` / `install.ps1` at the old paths
   forever. They are static scripts, they cost nothing in SEO, and breaking them
   breaks installs in the wild.
3. Replace `mosaic/site/*.html` with stubs carrying
   `<link rel="canonical" href="https://mosaicvideo.github.io/...">` plus a meta
   refresh, so the old HTML URLs consolidate rather than compete.

**Alternative worth weighing first:** a real domain (`mosaicvideo.app`,
`getmosaic.video`, etc.). Same low direct SEO impact, but much higher *entity*
value — AI answer engines and Wikipedia-style knowledge graphs anchor on brand
domains, and it decouples you from GitHub forever. At four months old with
near-zero index presence, **there is nothing to lose by moving now**, and the
cost only rises with every month of accumulated links.

---

## 3. Phased plan

### Phase 0 — Instrumentation (do first; you are currently blind)

Without this you cannot tell "not indexed" from "indexed but not ranking", and
those need opposite responses.

- [ ] **Google Search Console.** Verify via HTML file — drop the
      `google<hash>.html` Google gives you into `site/`, push, done. No DNS
      needed. Verify the exact prefix you intend to keep.
- [ ] **Bing Webmaster Tools** — import from GSC in one click. Bing feeds
      ChatGPT and Copilot, so this is an AEO action, not just an SEO one.
- [ ] Submit the sitemap (Phase 1) in both.
- [ ] Use **URL Inspection → Request Indexing** on all three pages manually.
- [ ] Check **GSC → Pages** for the actual exclusion reason. "Discovered –
      currently not indexed" means low-value/low-authority. "Crawled – currently
      not indexed" means quality. They lead to different fixes.

### Phase 1 — Technical foundation (~2 hours, purely mechanical)

- [ ] `site/robots.txt` — allow all, point at the sitemap. Explicitly allow
      `GPTBot`, `ClaudeBot`, `PerplexityBot`, `Google-Extended`, `OAI-SearchBot`
      (default is allow, but stating it is a deliberate AEO signal).
- [ ] `site/sitemap.xml` — three URLs with `lastmod`. Static is fine at this
      size; wire it into `pages.yml` only if the page count grows.
- [ ] **Add an `<h1>` to `index.html`.** It currently has none — only `<h2>`s.
      Make it the long-tail phrase, not the brand: something like
      *"Video contact sheets, screenshots and animated previews — batch, local,
      cross-platform"*.
- [ ] `<link rel="canonical">` on all three pages, absolute URLs.
- [ ] Fix `og:image` — it is currently the relative path `assets/icon.png`.
      Open Graph requires absolute URLs, so **every share preview and every AI
      crawler reading OG currently gets nothing.** Also add `og:image:width`,
      `og:image:height`, `og:image:alt`.
- [ ] Add `og:url`, `og:site_name`, and full OG blocks to `guide.html` and
      `cli.html` — they have none at all.
- [ ] Add `twitter:card` = `summary_large_image` + `twitter:title` /
      `:description` / `:image` to all three.
- [ ] Create a proper 1200×630 OG image. Currently it would fall back to the
      square 298 KB app icon, which crops badly everywhere.
- [ ] **Performance:** `site/assets/` is 5.1 MB, dominated by
      `output-animated-reel.webp` at **3.5 MB**. It is lazy-loaded so it is not
      an LCP problem, but it is a bandwidth and Core-Web-Vitals-adjacent problem
      on the guide page. Re-encode to under 800 KB. `hero.png` at 380 KB is
      above the fold — convert to WebP and add explicit `width`/`height` to kill
      layout shift.

### Phase 2 — Structured data

Nothing on the site has any. This is the cheapest AEO win available.

- [ ] **`SoftwareApplication`** on `index.html`:
      `applicationCategory: MultimediaApplication`, `operatingSystem: "macOS,
      Windows, Linux"`, `offers: {price: "0", priceCurrency: "USD"}`,
      `softwareVersion`, `downloadUrl`, `license`, `screenshot`,
      `featureList`. This is what makes an app rich result eligible and what AI
      engines read to answer "is it free / what platforms".
- [ ] **`FAQPage`** on `guide.html`. The FAQ is already ~10 well-written Q&A
      pairs in `<details>` — perfect raw material, zero schema. This is the
      single highest-value markup item on the site for AI citation.
- [ ] **`SoftwareSourceCode`** linking `codeRepository` to GitHub — helps bind
      the site and the repo into one entity.
- [ ] **`BreadcrumbList`** on `guide.html` and `cli.html`.
- [ ] **`HowTo`** on the install sections. Be aware Google retired *HowTo rich
      results* in 2023, so expect no SERP change — it is worth it purely because
      LLM extractors still parse it.
- [ ] Validate everything at the Rich Results Test and schema.org validator.

### Phase 3 — Site-at-root or custom domain

Decide per §2 above. If moving, do it **before** Phase 4, so the content you
write lands on its final URLs and you never migrate links twice.

### Phase 4 — Content depth (this is where rankings actually come from)

396 words on the page you most want to rank is the core content problem, and a
large share of those words are decorative terminal-mock text
(`01. mac  mosaic_universal.dmg  --`). Search engines and LLMs read that as
noise.

- [ ] **Rewrite the homepage to 800–1,200 words of real prose.** Keep the
      terminal aesthetic — just add substantive copy beneath it. What a contact
      sheet is, who needs one, why batch matters, why local-only matters, how it
      compares to CLI tools.
- [ ] **Open every major section with a 40–60 word direct-answer block.** This
      is the single most-cited structural pattern in AI answer extraction — the
      first paragraph should fully answer the heading's implicit question,
      standing alone with no context.
- [ ] **Named-entity density in the first 500 words.** `ffmpeg`, `ffprobe`,
      `MediaInfo`, `Tauri`, `Rust`, `contact sheet`, `HDR10`, `Dolby Vision`,
      `WebP`, `macOS`, `Windows`, `Linux`. Entity density is how retrieval
      systems decide a page is topically authoritative.
- [ ] **New pages, in priority order** — each targets one long-tail intent:
      1. **`vcsi-alternative.html`** — honest side-by-side with vcsi and the
         other CLI tools. Comparison pages punch far above their weight in both
         search and AI citation, because they are the shape of the question
         people actually ask.
      2. **`how-to-make-a-video-contact-sheet.html`** — the tutorial that ranks.
         Show the raw ffmpeg command *first* (this is what earns the ranking and
         the links), then show Mosaic doing it in one drag.
      3. **`animated-contact-sheets.html`** — near-uncontested term, and it is a
         genuine differentiator; almost nothing else does animated grids.
- [ ] **Freshness.** AI citations skew hard toward recently-updated pages —
      the research consensus is that a large majority of cited pages were
      updated within 12 months, most within 6. Put a visible "Last updated"
      date on each page and a `dateModified` in the JSON-LD, and actually keep
      them current.

### Phase 5 — Off-site (highest leverage; almost none of it is on the website)

For a developer tool, the website is rarely the thing that gets cited. The repo,
the package managers and the community lists are. This phase matters more than
Phases 1–4 combined.

- [ ] **GitHub topics.** Currently five weak ones: `contact-sheets`,
      `screenshots`, `thumbnails`, `video-sheets`, `animated`. Add the ones
      people actually browse and LLMs actually index: `ffmpeg`, `tauri`, `rust`,
      `video-thumbnails`, `contact-sheet`, `mediainfo`, `desktop-app`,
      `cross-platform`, `video-tools`, `screenshot-tool`, `hdr`. GitHub topic
      pages are crawled and rank.
- [ ] **Rewrite the README as the primary SEO/AEO surface.** For a dev tool the
      README is the single most-cited artifact — it outranks the site and it is
      what every LLM has ingested. It needs the comparison table, the long-tail
      phrases, the screenshots, and an explicit "vs. vcsi / MTN / Thumbnailer"
      section.
- [ ] **Package managers — the highest-ROI item on this list.** Each one creates
      an indexed canonical page *and* is precisely what an LLM cites when asked
      "how do I install this on macOS":
      - **Homebrew cask** — note the `mosaic` token is **taken** by an unrelated
        window manager. Submit as `mosaic-video` or similar. Check the naming
        rules before writing the PR.
      - **winget** (`Mosaic.Mosaic` style manifest)
      - **AUR** and/or **Flathub** for Linux
      - Homebrew's own `formulae.brew.sh` pages rank extremely well.
- [ ] **AlternativeTo** — list Mosaic as an alternative to vcsi, MTN,
      Thumbnailer, VLC's snapshot feature. AlternativeTo is heavily scraped by
      answer engines and is one of the most reliable ways to get into
      "alternatives to X" responses.
- [ ] **Awesome lists.** `awesome-tauri` is the strongest fit — Mosaic is a
      legitimate showcase app, and that list is high-traffic and high-authority.
      Also `awesome-ffmpeg`, `awesome-video`, `awesome-selfhosted` if applicable.
- [ ] **Communities where contact sheets are load-bearing:** r/DataHoarder
      (contact sheets are a staple there), r/ffmpeg, r/rust, r/tauri,
      Hacker News Show HN. One good Show HN is worth more than every meta tag in
      this document. Time it with a release.
- [ ] Add the site link to the repo sidebar (already set) and to every release
      note.

### Phase 6 — AEO specifics

- [ ] **`llms.txt`** at the site root — a markdown index of the site's key pages
      and what Mosaic is. **Be realistic:** adoption is mostly developer-facing
      tooling, and the research consensus is that citations, schema and genuine
      third-party mentions do most of the actual work. It costs ten minutes and
      has no downside, so add it — just don't expect it to move the needle on
      its own.
- [ ] **Rewrite FAQ answers to be self-contained.** Every answer must make sense
      quoted in isolation, with no reference to "the section above". Several
      currently don't (the auto-update answer defers to an earlier section) —
      that makes them unquotable, which is the one thing an AEO answer must
      never be.
- [ ] **Question-shaped headings.** LLM retrieval matches on question form.
      "Requirements" → "What do I need to run Mosaic?". "Output types" →
      "What can Mosaic generate?".
- [ ] **Consistent entity naming.** Pick one form — "Mosaic" — and use it
      identically across the site, README, releases, and package manifests.
      Inconsistent naming fragments the entity and stops knowledge graphs
      consolidating it.
- [ ] Once live, spot-check by asking ChatGPT, Claude, Perplexity and Google AI
      Overviews *"what's a good GUI tool for making video contact sheets?"* and
      track whether Mosaic appears. That is the actual AEO KPI — not rankings.

---

## 4. Priority order

| # | Action | Effort | Impact | Phase |
|---|---|---|---|---|
| 1 | Google + Bing Search Console, request indexing | 30 min | **Critical** — unblocks diagnosis | 0 |
| 2 | robots.txt + sitemap.xml | 20 min | High | 1 |
| 3 | GitHub topics + README rewrite | 2 h | **Very high** — most-cited artifact | 5 |
| 4 | `<h1>`, canonical, absolute `og:image`, twitter cards | 1 h | High | 1 |
| 5 | Homebrew cask + winget submission | 3 h | **Very high** — indexed pages + LLM install answers | 5 |
| 6 | `SoftwareApplication` + `FAQPage` JSON-LD | 2 h | High (AEO) | 2 |
| 7 | Homepage rewrite to 800–1,200 real words | 4 h | **Very high** | 4 |
| 8 | AlternativeTo + awesome-tauri listings | 1 h | High | 5 |
| 9 | `vcsi-alternative` comparison page | 3 h | High | 4 |
| 10 | Show HN / r/DataHoarder, timed with a release | 1 h | High but spiky | 5 |
| 11 | Root-domain or custom-domain move | 2 h | Low SEO, real brand value | 3 |
| 12 | `llms.txt` | 10 min | Low but free | 6 |
| 13 | Image optimisation (3.5 MB WebP) | 1 h | Low SEO, real UX | 1 |

## 5. Expectations

Be realistic about the timeline. A four-month-old hostname with two stars will
not rank next week no matter what is shipped.

- **Weeks 1–2:** indexed and findable for `mosaic video contact sheet` and other
  brand-qualified queries. This is achievable and is the first real milestone.
- **Months 2–4:** long-tail traction on `animated contact sheet` and
  `vcsi alternative` — the low-competition terms — assuming Phase 4 and 5 ship.
- **Months 4–8:** AI answer engines start naming Mosaic, driven almost entirely
  by the package-manager pages, AlternativeTo, and awesome-list entries — not by
  the website.
- **Never:** ranking for "mosaic".

The two things that would most change this trajectory are a Show HN that lands
and a Homebrew cask. Both are Phase 5. Neither is a meta tag.
