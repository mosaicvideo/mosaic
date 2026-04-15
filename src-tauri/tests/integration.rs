use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
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
    };
    mosaic_lib::contact_sheet::generate(
        &fixture, &info, &out, &opts, &tools.ffmpeg, &font,
        Arc::new(AtomicBool::new(false)), &reporter,
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
    let outs = mosaic_lib::screenshots::generate(
        &fixture, &info, &shots_dir, &shots_opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
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

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
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

    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();

    assert!(out.exists(), "reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "reel suspiciously small: {} bytes", bytes.len());

    // WebP container: "RIFF"....WEBP (bytes 0-3 = RIFF, bytes 8-11 = WEBP).
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");

    // Animated WebP must include a VP8X chunk with the animation flag (0x02) set.
    // VP8X layout: "VP8X" (4) + chunk_size=10 (4) + flags byte + ...
    let vp8x_pos = bytes.windows(4).position(|w| w == b"VP8X")
        .expect("missing VP8X chunk — not an animated WebP");
    let flags_byte = bytes[vp8x_pos + 8];
    assert!(flags_byte & 0x02 != 0, "animation flag not set in VP8X flags byte: {:#04x}", flags_byte);
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

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
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

    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();

    assert!(out.exists(), "webm reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "webm suspiciously small: {} bytes", bytes.len());

    // WebM is a Matroska subset; first four bytes are the EBML header magic.
    assert_eq!(&bytes[0..4], &[0x1A, 0x45, 0xDF, 0xA3], "missing EBML magic");
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

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
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

    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();

    assert!(out.exists(), "gif reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "gif suspiciously small: {} bytes", bytes.len());

    // GIF signature: "GIF87a" or "GIF89a" — both start with "GIF8".
    assert_eq!(&bytes[0..4], b"GIF8", "missing GIF magic");
}
