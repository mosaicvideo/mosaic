# Tech Stack

Mosaic is a Tauri 2 cross-platform desktop app (plus a sibling CLI) for generating
video contact sheets, screenshots, animated preview reels, and animated contact sheets.

```mermaid
graph TD
    subgraph CLIENTS["User-facing entry points"]
        FE["Frontend — src/<br/>Vanilla HTML/CSS/JS<br/>main.js · dropzone · queue · options · events · mediainfo<br/>Vite 8 dev/build"]
        CLI["CLI — mosaic-cli/<br/>Rust binary<br/>clap 4 · clap_complete · clap_mangen<br/>indicatif · toml · embedded DejaVuSans"]
    end

    subgraph TAURI["Tauri 2 Runtime"]
        IPC["IPC: invoke / listen<br/>via @tauri-apps/api"]
        PLUGINS["Plugins:<br/>dialog · store · updater · process"]
    end

    subgraph BACKEND["Rust Backend — mosaic_lib (src-tauri/) · edition 2021 · Tokio"]
        CMD["Commands<br/>commands.rs — Tauri command handlers"]
        ORCH["Orchestration<br/>contact_sheet · screenshots<br/>preview_reel · animated_sheet"]
        PURE["Pure logic (unit-tested)<br/>drawtext · layout · header<br/>output_path · video_info · mediainfo"]
        IO["Subprocess I/O<br/>ffmpeg.rs (run_cancellable)<br/>jobs.rs (Arc-AtomicBool)"]
        CRATES["serde · serde_json<br/>thiserror · which · tempfile"]
    end

    subgraph TOOLS["External Tools (locate_tools)"]
        FFMPEG["ffmpeg"]
        FFPROBE["ffprobe"]
        MEDIAINFO["mediainfo"]
    end

    FE -->|invoke / listen| IPC
    CLI -->|path dep, feature=cli| CMD
    IPC --> PLUGINS
    PLUGINS --> CMD
    CMD --> ORCH
    CMD -.-> CRATES
    ORCH --> PURE
    ORCH --> IO
    IO -->|spawns| FFMPEG
    IO --> FFPROBE
    IO --> MEDIAINFO

    subgraph OPS["Build · Distribution · Site (tooling — not in runtime flow)"]
        direction LR
        BUILD["Tooling<br/>pnpm 10.33 · Cargo (2 crates)<br/>Vite 8 · Node scripts"]
        DIST["Distribution<br/>GitHub Actions: ci · release<br/>pages · defaults-sync-check<br/>macOS (signed+notarized)<br/>Win x64/ARM · Linux AppImage/deb/rpm<br/>Auto-updater: latest.json + ed25519/minisign"]
        SITE["Showcase site<br/>site/ static HTML/CSS/JS<br/>GitHub Pages · install.sh / install.ps1<br/>download.js"]
        BUILD --- DIST --- SITE
    end
```

> The **Build · Distribution · Site** cluster is the surrounding tooling/CI — it
> packages and ships the app rather than participating in the request flow, so it's
> shown detached from the runtime graph above.

## Summary by layer

| Layer | Technology |
|-------|-----------|
| **Desktop shell** | Tauri 2 (Rust core + WebView), plugins: dialog, store, updater, process |
| **Frontend** | Vanilla HTML/CSS/JS (no framework), Vite 8 dev/build, `@tauri-apps/api` for IPC |
| **Backend** | Rust 2021, Tokio async, serde, thiserror, which, tempfile — layered as *commands → orchestration → pure logic → subprocess I/O* |
| **CLI** | Sibling Rust crate (`mosaic-cli`) depending on `mosaic_lib` via `feature="cli"`; clap 4, clap_complete, clap_mangen, indicatif, toml |
| **Media engine** | External `ffmpeg` / `ffprobe` / `mediainfo` subprocesses |
| **Tooling** | pnpm 10.33 (JS), Cargo (Rust), Node build scripts |
| **CI/CD** | GitHub Actions (ci, release, pages, defaults-sync-check); signed multi-platform bundles; ed25519/minisign auto-updater |
| **Site** | Static `site/` on GitHub Pages with cross-platform install scripts |

**Defining architectural choice:** strict pipeline separation in the Rust backend — pure,
fully unit-testable logic modules (`drawtext`, `layout`, `header`, `output_path`,
`video_info`) feed orchestration modules that build ffmpeg arg vectors, with all
subprocess I/O isolated in `ffmpeg.rs`. The same library internals are shared between the
GUI and CLI through a feature-flag visibility switch rather than code duplication.
