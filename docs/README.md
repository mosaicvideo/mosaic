# Mosaic — docs

Design and planning documents. Code is the source of truth; these are here to capture **intent** and **rationale** — the "why" that doesn't survive in diffs.

## Layout

```
docs/
  2026-04-14-mosaic-*.md         — Original v1 spec + plans (design, build, CLI, distribution)
  superpowers/
    specs/                       — Design docs per feature (the "what" + "why")
    plans/                       — Step-by-step implementation plans (for TDD-driven builds)
  README.md                      — this file
```

Dated naming (`YYYY-MM-DD-topic-design.md`) makes chronology legible at `ls` time.

## Foundation (v1)

| Doc | Scope |
|---|---|
| [mosaic-design.md](2026-04-14-mosaic-design.md) | Product spec: goals, tech stack, output types, UX |
| [mosaic-plan.md](2026-04-14-mosaic-plan.md) | Implementation plan for v1 (Tauri scaffold → pipelines → queue UI) |
| [mosaic-cli-plan.md](2026-04-14-mosaic-cli-plan.md) | `mosaic` CLI — original v1 plan (superseded; see CLI spec + plan below) |
| [mosaic-distribution-plan.md](2026-04-14-mosaic-distribution-plan.md) | Packaging, CI, multi-platform release plan |

## Feature specs (post-v1)

Specs describe features shipped since v0.1.0. Each is a post-hoc record of design decisions with enough detail to understand the code without re-deriving it.

### v0.1.1

- [animated-preview-reel-design](superpowers/specs/2026-04-15-animated-preview-reel-design.md) — third output type (animated WebP / WebM / GIF).
- [animated-contact-sheet-design](superpowers/specs/2026-04-16-animated-contact-sheet-design.md) — fourth output type (grid of animated clips as one WebP).
- [extraction-fixes-design](superpowers/specs/2026-04-16-extraction-fixes-design.md) — accurate seeking, parallel extraction, audio stripping.
- [mediainfo-modal-design](superpowers/specs/2026-04-16-mediainfo-modal-design.md) — per-file metadata viewer. **Superseded in part** by the v0.1.3 enrichment spec; read this one only for the modal UI + keyboard dismissal details.

### v0.1.2

- [auto-update-design](superpowers/specs/2026-04-17-auto-update-design.md) — cryptographically-signed one-click updates.
- [showcase-site-design](superpowers/specs/2026-04-17-showcase-site-design.md) — GitHub Pages static site + runtime releases fetch.

### HDR / Dolby Vision

- [hdr-tonemapping-design](superpowers/specs/2026-04-17-hdr-tonemapping-design.md) — HDR10 / HLG zscale chain + Dolby Vision Profile 5 IPT-PQ-C2 matrix correction. Shipped incrementally across v0.1.1 and v0.1.3.

### v0.1.3

- [displayed-dims-and-enrichment-design](superpowers/specs/2026-04-17-displayed-dims-and-enrichment-design.md) — displayed square-pixel dimensions (SAR + rotation + 3D Blu-ray MVC), MediaInfo enrichment, multi-line header.

### CLI (implemented, pending release)

- [mosaic-cli-design](superpowers/specs/2026-04-18-mosaic-cli-design.md) — CLI design spec: subcommands, flag surface, config file, shared-defaults contract, module layout.
- [mosaic-cli plan](superpowers/plans/2026-04-18-mosaic-cli.md) — step-by-step implementation plan for the `mosaic-cli` binary.

## Plans

Plans are step-by-step implementation breakdowns used during TDD-driven builds. They're in `superpowers/plans/` and pair 1:1 with the spec from the same date:

- [2026-04-15-animated-preview-reel.md](superpowers/plans/2026-04-15-animated-preview-reel.md)
- [2026-04-16-extraction-fixes.md](superpowers/plans/2026-04-16-extraction-fixes.md)
- [2026-04-16-mediainfo-modal.md](superpowers/plans/2026-04-16-mediainfo-modal.md) — partially superseded (see modal spec note).
- [2026-04-18-mosaic-cli.md](superpowers/plans/2026-04-18-mosaic-cli.md) — CLI binary (`mosaic-cli`); implemented, pending release.

Features shipped without an accompanying plan doc were implemented directly from the spec or without one at all — that's allowed for tightly-scoped changes where TDD tracking would be overhead.

## Conventions

- **Filenames:** `YYYY-MM-DD-kebab-topic-design.md` for specs, `YYYY-MM-DD-kebab-topic.md` for plans.
- **Date:** the date the doc was written or the change shipped, not today's date when editing.
- **Status line:** every spec begins with a one-line status (`Shipped in vX.Y.Z`, `Draft`, `Superseded`).
- **Scope exclusions section:** every spec ends with explicit out-of-scope items so future readers don't re-propose them.
- **Commit refs:** link specs to the commits that implemented them via short hashes. Diffs are the source of truth; specs are the narrative.
