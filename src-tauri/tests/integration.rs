use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Spot-check that probing a known-anamorphic sample (1080×1080 + SAR 9:16)
/// produces displayed dims of 606×1080. Requires the file at a fixed path on
/// the developer machine; marked `#[ignore]` so CI doesn't need it.
#[tokio::test]
#[ignore]
async fn probe_produces_displayed_dims_for_anamorphic_sample() {
    let fixture = std::path::Path::new("/Users/abi/Downloads/maeshima_mayu-beautiful.mp4");
    if !fixture.exists() {
        eprintln!("skipping: sample not available at {}", fixture.display());
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy())
        .await
        .expect("probe failed");
    // SAR 9:16 on 1080×1080 → 1080 * 9/16 = 607.5 → round_even = 606.
    assert_eq!((info.video.width, info.video.height), (606, 1080),
        "expected displayed dims 606×1080, got {}×{}", info.video.width, info.video.height);
    assert_eq!(info.video.sar, Some((9, 16)));
}

/// Spot-check that probing a 3D Blu-ray ISO (MVC multi-view coding) picks
/// the usable base-layer stream instead of the zero-dim dependent stream
/// that ffprobe lists first. Regression guard for the contact-sheet bug on
/// 3D ISOs where thumb_h fell back to square because of 0×0 inputs.
#[tokio::test]
#[ignore]
async fn probe_3d_iso_picks_base_layer_stream() {
    let fixture = std::path::Path::new("/Users/abi/Downloads/MosaicSamples/ISO-full3D-sample.iso");
    if !fixture.exists() {
        eprintln!("skipping: sample not available at {}", fixture.display());
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy())
        .await
        .expect("probe failed");
    assert_eq!((info.video.width, info.video.height), (1920, 1080),
        "expected 1920×1080 base layer, got {}×{}", info.video.width, info.video.height);
}

/// Spot-check MediaInfo enrichment on a real HDR+DV sample. MediaInfo is now
/// a first-party prerequisite, so `locate_tools` guarantees the binary is
/// present — no skip needed beyond the sample file being available.
#[tokio::test]
#[ignore]
async fn probe_enriches_with_mediainfo() {
    let fixture = std::path::Path::new("/Users/abi/Downloads/MosaicSamples/awaken-girl.4K.HDR.DV.mkv");
    if !fixture.exists() {
        eprintln!("skipping: sample not available at {}", fixture.display());
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy())
        .await
        .expect("probe failed");
    let e = info.enrichment.as_ref().expect("mediainfo is a prerequisite; enrichment must populate");
    assert_eq!(e.container_format.as_deref(), Some("Matroska"));
    assert_eq!(e.video_bit_depth, Some(10));
    assert!(e.video_hdr_format.as_deref().unwrap_or("").contains("Dolby Vision"));
    assert!(e.audio_language.as_deref() == Some("en"));
}

/// End-to-end: run every pipeline on the real anamorphic sample and verify
/// each output preserves the displayed aspect (9:16 portrait). Exercises the
/// full probe → scale → render path, not just the arg-builder logic the
/// `tests::*` units cover.
#[tokio::test]
#[ignore]
async fn all_pipelines_preserve_aspect_on_anamorphic_sample() {
    let fixture = std::path::Path::new("/Users/abi/Downloads/maeshima_mayu-beautiful.mp4");
    if !fixture.exists() {
        eprintln!("skipping: sample not available at {}", fixture.display());
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy())
        .await
        .expect("probe failed");
    assert_eq!((info.video.width, info.video.height), (606, 1080));

    let font: PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect();
    let tmp = tempfile::tempdir().unwrap();
    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };

    // 1. Static contact sheet → PNG with portrait aspect (height > width).
    let sheet_out = tmp.path().join("sheet.png");
    let sheet_opts = mosaic_lib::contact_sheet::SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 6,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: false, show_header: false,
        format: mosaic_lib::output_path::OutputFormat::Png,
        jpeg_quality: 92,
        suffix: String::new(),
        theme: mosaic_lib::output_path::SheetTheme::Dark,
    };
    mosaic_lib::contact_sheet::generate(fixture, &info, &sheet_out, &sheet_opts, &font, &ctx)
        .await.expect("static sheet failed");
    let (sw, sh) = probe_wh(&tools.ffprobe, &sheet_out).await;
    assert!(sh > sw, "static sheet should be portrait for anamorphic source, got {}×{}", sw, sh);

    // 2. Screenshots → one PNG per timestamp, each at displayed aspect.
    let shots_dir = tmp.path().join("shots");
    let shots_opts = mosaic_lib::screenshots::ScreenshotsOptions {
        count: 2,
        format: mosaic_lib::output_path::OutputFormat::Png,
        jpeg_quality: 92,
        suffix: String::new(),
    };
    let shots = mosaic_lib::screenshots::generate(fixture, &info, &shots_dir, &shots_opts, &ctx)
        .await.expect("screenshots failed");
    for shot in &shots {
        let (w, h) = probe_wh(&tools.ffprobe, shot).await;
        // Exact match: screenshots resize to displayed dims precisely.
        assert_eq!((w, h), (606, 1080), "screenshot {:?} has wrong dims {}×{}", shot, w, h);
    }

    // 3. Animated contact sheet → WebP with portrait cells.
    let anim_out = tmp.path().join("anim.webp");
    let anim_opts = mosaic_lib::animated_sheet::AnimatedSheetOptions {
        cols: 2, rows: 2, width: 640, gap: 6,
        clip_length_secs: 1, fps: 8, quality: 60,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: false, show_header: false,
        suffix: String::new(),
        theme: mosaic_lib::output_path::SheetTheme::Dark,
    };
    mosaic_lib::animated_sheet::generate(fixture, &info, &anim_out, &anim_opts, &font, &ctx)
        .await.expect("animated sheet failed");
    let bytes = std::fs::read(&anim_out).unwrap();
    assert_animated_webp(&bytes);
    // ffprobe reports 0×0 for animated WebPs (no demuxer for the animation
    // stream); read canvas dims directly from the VP8X chunk instead.
    let (aw, ah) = webp_canvas_wh(&bytes);
    assert!(ah > aw, "animated sheet should be portrait for anamorphic source, got {}×{}", aw, ah);

    // 4. Preview reel → WebM with portrait aspect.
    let reel_out = tmp.path().join("reel.webm");
    let reel_opts = mosaic_lib::preview_reel::PreviewOptions {
        count: 2,
        clip_length_secs: 1,
        height: 480,
        fps: 12,
        quality: 75,
        suffix: String::new(),
        format: mosaic_lib::output_path::ReelFormat::Webm,
    };
    mosaic_lib::preview_reel::generate(fixture, &info, &reel_out, &reel_opts, &ctx)
        .await.expect("preview reel failed");
    let (rw, rh) = probe_wh(&tools.ffprobe, &reel_out).await;
    assert!(rh > rw, "reel should be portrait for anamorphic source, got {}×{}", rw, rh);
    assert_eq!(rh, 480, "reel height should equal target; got {}", rh);
}

/// Regression guard: on a plain 16:9 landscape source, every pipeline must
/// still produce landscape output at roughly the expected dims. Catches a
/// bad displayed-dim swap / thumb_width inversion that would only show up
/// when the source already had square pixels.
#[tokio::test]
#[ignore]
async fn landscape_source_still_renders_landscape() {
    let fixture = std::path::Path::new("/Users/abi/Downloads/MosaicSamples/12185792_3840_2160_30fps.mp4");
    if !fixture.exists() {
        eprintln!("skipping: sample not available at {}", fixture.display());
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy())
        .await
        .expect("probe failed");
    // No rotation/SAR on a plain landscape source — displayed == encoded.
    assert_eq!((info.video.width, info.video.height), (3840, 2160));
    assert!(info.video.sar.is_none());
    assert!(info.video.rotation.is_none());

    let font: PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect();
    let tmp = tempfile::tempdir().unwrap();
    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };

    let sheet_out = tmp.path().join("sheet.png");
    mosaic_lib::contact_sheet::generate(fixture, &info, &sheet_out, &mosaic_lib::contact_sheet::SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 4,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: false, show_header: false,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(), theme: mosaic_lib::output_path::SheetTheme::Dark,
    }, &font, &ctx).await.expect("sheet");
    let (sw, sh) = probe_wh(&tools.ffprobe, &sheet_out).await;
    assert!(sw > sh, "landscape source should stay landscape in sheet, got {}×{}", sw, sh);

    // Screenshots: no scale filter because SAR is None and no tonemap.
    let shots_dir = tmp.path().join("shots");
    let shots = mosaic_lib::screenshots::generate(fixture, &info, &shots_dir, &mosaic_lib::screenshots::ScreenshotsOptions {
        count: 1, format: mosaic_lib::output_path::OutputFormat::Png,
        jpeg_quality: 92, suffix: String::new(),
    }, &ctx).await.expect("shots");
    let (w, h) = probe_wh(&tools.ffprobe, &shots[0]).await;
    assert_eq!((w, h), (3840, 2160), "landscape screenshot should be source res");
}

/// Extract canvas dimensions from an animated WebP's VP8X chunk header.
/// Layout (RFC 9649 §2.5.2): after the 8-byte chunk header, 1 flags byte,
/// 3 reserved bytes, 3 bytes canvas-width-minus-1 (little-endian), 3 bytes
/// canvas-height-minus-1.
fn webp_canvas_wh(bytes: &[u8]) -> (u32, u32) {
    let pos = bytes.windows(4).position(|w| w == b"VP8X").expect("VP8X chunk");
    let payload = &bytes[pos + 8..];
    let w_minus_1 = u32::from(payload[4]) | (u32::from(payload[5]) << 8) | (u32::from(payload[6]) << 16);
    let h_minus_1 = u32::from(payload[7]) | (u32::from(payload[8]) << 8) | (u32::from(payload[9]) << 16);
    (w_minus_1 + 1, h_minus_1 + 1)
}

async fn probe_wh(ffprobe: &std::path::Path, path: &std::path::Path) -> (u32, u32) {
    let args = ["-v", "error", "-select_streams", "v:0",
        "-show_entries", "stream=width,height", "-of", "csv=p=0", &path.to_string_lossy()];
    let out = std::process::Command::new(ffprobe).args(&args).output().expect("ffprobe");
    let s = String::from_utf8_lossy(&out.stdout);
    let (w, h) = s.trim().split_once(',').expect("w,h csv");
    (w.parse().unwrap(), h.parse().unwrap())
}

/// Assert bytes are a valid animated WebP: RIFF/WEBP container + VP8X chunk
/// with the animation flag (bit 0x02 of the flags byte) set.
fn assert_animated_webp(bytes: &[u8]) {
    assert!(bytes.len() > 100, "webp suspiciously small: {} bytes", bytes.len());
    assert_eq!(&bytes[0..4], b"RIFF", "missing RIFF magic");
    assert_eq!(&bytes[8..12], b"WEBP", "missing WEBP marker");
    let vp8x_pos = bytes.windows(4).position(|w| w == b"VP8X")
        .expect("missing VP8X chunk — not an animated WebP");
    let flags_byte = bytes[vp8x_pos + 8];
    assert!(flags_byte & 0x02 != 0, "animation flag not set in VP8X flags byte: {:#04x}", flags_byte);
}

#[tokio::test]
async fn end_to_end_contact_sheet_and_screenshots() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    let font: PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());
    assert!(font.exists(), "missing bundled font");

    // Probe
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy()).await.unwrap();
    assert!(info.duration_secs > 1.0);

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample_contact_sheet.png");

    let reporter = mosaic_lib::jobs::ProgressReporter {
        emit: &|_, _, _| {},
    };
    let opts = mosaic_lib::contact_sheet::SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 8,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: true, show_header: true,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(),
        theme: mosaic_lib::output_path::SheetTheme::Dark,
    };
    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    mosaic_lib::contact_sheet::generate(
        &fixture, &info, &out, &opts, &font, &ctx,
    ).await.unwrap();
    assert!(out.exists(), "sheet not written");
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);

    // Screenshots
    let shots_dir = tmp.path().join("shots");
    let shots_opts = mosaic_lib::screenshots::ScreenshotsOptions {
        count: 3,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(),
    };
    let ctx2 = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    let outs = mosaic_lib::screenshots::generate(
        &fixture, &info, &shots_dir, &shots_opts, &ctx2,
    ).await.unwrap();
    assert_eq!(outs.len(), 3);
    for p in outs { assert!(p.exists()); }
}

#[tokio::test]
async fn end_to_end_animated_preview_reel() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy()).await.unwrap();
    assert!(info.duration_secs > 1.0);

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample - reel.webp");

    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let opts = mosaic_lib::preview_reel::PreviewOptions {
        count: 3,
        clip_length_secs: 1,
        height: 240,
        fps: 12,
        quality: 60,
        suffix: String::new(),
        format: mosaic_lib::output_path::ReelFormat::Webp,
    };

    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &ctx,
    ).await.unwrap();

    assert!(out.exists(), "reel not written");
    assert_animated_webp(&std::fs::read(&out).unwrap());
}

#[tokio::test]
async fn end_to_end_animated_preview_reel_webm() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample - reel.webm");

    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let opts = mosaic_lib::preview_reel::PreviewOptions {
        count: 2,
        clip_length_secs: 1,
        height: 240,
        fps: 12,
        quality: 50,
        suffix: String::new(),
        format: mosaic_lib::output_path::ReelFormat::Webm,
    };

    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &ctx,
    ).await.unwrap();

    assert!(out.exists(), "webm reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "webm suspiciously small: {} bytes", bytes.len());

    // WebM is a Matroska subset; first four bytes are the EBML header magic.
    assert_eq!(&bytes[0..4], &[0x1A, 0x45, 0xDF, 0xA3], "missing EBML magic");
}

#[tokio::test]
async fn end_to_end_animated_contact_sheet() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    let font: PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());
    assert!(font.exists(), "missing bundled font");

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample_animated_sheet.webp");

    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let opts = mosaic_lib::animated_sheet::AnimatedSheetOptions {
        cols: 2,
        rows: 2,
        width: 640,
        gap: 8,
        clip_length_secs: 1,
        fps: 8,
        quality: 60,
        thumb_font_size: 12,
        header_font_size: 14,
        show_timestamps: true,
        show_header: true,
        suffix: String::new(),
        theme: mosaic_lib::output_path::SheetTheme::Dark,
    };

    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    mosaic_lib::animated_sheet::generate(
        &fixture, &info, &out, &opts, &font, &ctx,
    ).await.unwrap();

    assert!(out.exists(), "animated sheet not written");
    assert_animated_webp(&std::fs::read(&out).unwrap());
}

#[tokio::test]
async fn end_to_end_animated_contact_sheet_no_header() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    let font: PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect();

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample_bare.webp");

    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let opts = mosaic_lib::animated_sheet::AnimatedSheetOptions {
        cols: 2, rows: 2, width: 480, gap: 0,
        clip_length_secs: 1, fps: 6, quality: 60,
        thumb_font_size: 12, header_font_size: 14,
        show_timestamps: false,
        show_header: false,
        suffix: String::new(),
        theme: mosaic_lib::output_path::SheetTheme::Light,
    };

    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    mosaic_lib::animated_sheet::generate(
        &fixture, &info, &out, &opts, &font, &ctx,
    ).await.unwrap();

    assert!(out.exists(), "bare animated sheet not written");
    assert_animated_webp(&std::fs::read(&out).unwrap());
}

#[tokio::test]
async fn end_to_end_animated_preview_reel_gif() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy()).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample - reel.gif");

    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let opts = mosaic_lib::preview_reel::PreviewOptions {
        count: 2,
        clip_length_secs: 1,
        height: 240,
        fps: 10,
        quality: 0, // ignored for gif
        suffix: String::new(),
        format: mosaic_lib::output_path::ReelFormat::Gif,
    };

    let ctx = mosaic_lib::jobs::PipelineContext {
        ffmpeg: &tools.ffmpeg,
        cancelled: Arc::new(AtomicBool::new(false)),
        reporter: &reporter,
        has_zscale: tools.detect_has_zscale(),
    };
    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &ctx,
    ).await.unwrap();

    assert!(out.exists(), "gif reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "gif suspiciously small: {} bytes", bytes.len());

    // GIF signature: "GIF87a" or "GIF89a" — both start with "GIF8".
    assert_eq!(&bytes[0..4], b"GIF8", "missing GIF magic");
}
