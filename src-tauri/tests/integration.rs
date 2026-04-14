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
    let json = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
    let info = mosaic_lib::video_info_test_hook_parse(&json).unwrap();
    assert!(info.duration_secs > 1.0);

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample_contact_sheet.png");

    let reporter = mosaic_lib::contact_sheet::ProgressReporter {
        emit: &|_, _, _| {},
    };
    let opts = mosaic_lib::contact_sheet::SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 8,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: true, show_header: true,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
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
        count: 3, width: 320,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
    };
    let outs = mosaic_lib::screenshots::generate(
        &fixture, &info, &shots_dir, &shots_opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();
    assert_eq!(outs.len(), 3);
    for p in outs { assert!(p.exists()); }
}
