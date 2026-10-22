use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use mosaic_lib::animated_sheet::{self, AnimatedSheetOptions};
use mosaic_lib::contact_sheet::{self, SheetOptions};
use mosaic_lib::jobs::{PipelineContext, ProgressReporter};
use mosaic_lib::output_path::{OutputFormat, ReelFormat, SheetTheme};
use mosaic_lib::preview_reel::{self, PreviewOptions};
use mosaic_lib::screenshots::{self, ScreenshotsOptions};
use mosaic_lib::video_info::VideoInfo;

fn fixture_path(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name].iter().collect()
}

fn font_path() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect()
}

struct TestEnv {
    tools: mosaic_lib::ffmpeg::Tools,
    fixture: PathBuf,
    info: VideoInfo,
    tmp: tempfile::TempDir,
}

impl TestEnv {
    fn ctx<'a>(&'a self, reporter: &'a ProgressReporter<'a>) -> PipelineContext<'a> {
        PipelineContext {
            ffmpeg: &self.tools.ffmpeg,
            cancelled: Arc::new(AtomicBool::new(false)),
            reporter,
            has_zscale: self.tools.detect_caps().has_zscale(),
        }
    }
}

async fn setup(fixture_name: &str) -> Option<TestEnv> {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return None;
    }
    let fixture = fixture_path(fixture_name);
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools, &fixture.to_string_lossy())
        .await.expect("probe failed");
    Some(TestEnv { tools, fixture, info, tmp: tempfile::tempdir().unwrap() })
}

/// End-to-end: run every pipeline on a bundled anamorphic fixture
/// (1080×1080 + SAR 9:16) and verify each output preserves the displayed
/// aspect (9:16 portrait). Exercises the full probe → scale → render path,
/// not just the arg-builder logic the `tests::*` units cover.
#[tokio::test]
async fn all_pipelines_preserve_aspect_on_anamorphic_sample() {
    let Some(env) = setup("anamorphic_sample.mp4").await else { return };
    // SAR 9:16 on 1080×1080 → 1080 * 9/16 = 607.5 → round_even = 606.
    assert_eq!((env.info.video.width, env.info.video.height), (606, 1080));
    assert_eq!(env.info.video.sar, Some((9, 16)));

    let font = font_path();
    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let sheet_out = env.tmp.path().join("sheet.png");
    let sheet_opts = SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 6,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: false, show_header: false, header_title: false,
        format: OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(), theme: SheetTheme::Dark,
    };
    contact_sheet::generate(&env.fixture, &env.info, &sheet_out, &sheet_opts, &font, &ctx)
        .await.expect("static sheet failed");
    let (sw, sh) = probe_wh(&env.tools.ffprobe, &sheet_out).await;
    assert!(sh > sw, "static sheet should be portrait for anamorphic source, got {}×{}", sw, sh);

    let shots_dir = env.tmp.path().join("shots");
    let shots_opts = ScreenshotsOptions {
        count: 2, format: OutputFormat::Png, jpeg_quality: 92, suffix: String::new(),
    };
    let shots = screenshots::generate(&env.fixture, &env.info, &shots_dir, &shots_opts, &ctx)
        .await.expect("screenshots failed");
    for shot in &shots {
        let (w, h) = probe_wh(&env.tools.ffprobe, shot).await;
        assert_eq!((w, h), (606, 1080), "screenshot {:?} has wrong dims {}×{}", shot, w, h);
    }

    let anim_out = env.tmp.path().join("anim.webp");
    let anim_opts = AnimatedSheetOptions {
        cols: 2, rows: 2, width: 640, gap: 6,
        clip_length_secs: 1, fps: 8, quality: 60,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: false, show_header: false, header_title: false,
        suffix: String::new(), theme: SheetTheme::Dark,
    };
    animated_sheet::generate(&env.fixture, &env.info, &anim_out, &anim_opts, &font, &ctx)
        .await.expect("animated sheet failed");
    let bytes = std::fs::read(&anim_out).unwrap();
    assert_animated_webp(&bytes);
    // ffprobe reports 0×0 for animated WebPs (no demuxer for the animation
    // stream); read canvas dims directly from the VP8X chunk instead.
    let (aw, ah) = webp_canvas_wh(&bytes);
    assert!(ah > aw, "animated sheet should be portrait for anamorphic source, got {}×{}", aw, ah);

    let reel_out = env.tmp.path().join("reel.webm");
    let reel_opts = PreviewOptions {
        count: 2, clip_length_secs: 1, height: 480, fps: 12, quality: 75,
        suffix: String::new(), format: ReelFormat::Webm,
    };
    preview_reel::generate(&env.fixture, &env.info, &reel_out, &reel_opts, &ctx)
        .await.expect("preview reel failed");
    let (rw, rh) = probe_wh(&env.tools.ffprobe, &reel_out).await;
    assert!(rh > rw, "reel should be portrait for anamorphic source, got {}×{}", rw, rh);
    assert_eq!(rh, 480, "reel height should equal target; got {}", rh);
}

/// Regression guard: on a plain 16:9 landscape source, every pipeline must
/// still produce landscape output at roughly the expected dims. Catches a
/// bad displayed-dim swap / thumb_width inversion that would only show up
/// when the source already had square pixels.
#[tokio::test]
async fn landscape_source_still_renders_landscape() {
    let Some(env) = setup("landscape_4k_sample.mp4").await else { return };
    // No rotation/SAR on a plain landscape source — displayed == encoded.
    assert_eq!((env.info.video.width, env.info.video.height), (3840, 2160));
    assert!(env.info.video.sar.is_none());
    assert!(env.info.video.rotation.is_none());

    let font = font_path();
    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let sheet_out = env.tmp.path().join("sheet.png");
    contact_sheet::generate(&env.fixture, &env.info, &sheet_out, &SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 4,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: false, show_header: false, header_title: false,
        format: OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(), theme: SheetTheme::Dark,
    }, &font, &ctx).await.expect("sheet");
    let (sw, sh) = probe_wh(&env.tools.ffprobe, &sheet_out).await;
    assert!(sw > sh, "landscape source should stay landscape in sheet, got {}×{}", sw, sh);

    let shots_dir = env.tmp.path().join("shots");
    let shots = screenshots::generate(&env.fixture, &env.info, &shots_dir, &ScreenshotsOptions {
        count: 1, format: OutputFormat::Png, jpeg_quality: 92, suffix: String::new(),
    }, &ctx).await.expect("shots");
    let (w, h) = probe_wh(&env.tools.ffprobe, &shots[0]).await;
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
    let out = std::process::Command::new(ffprobe).args(args).output().expect("ffprobe");
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
    let Some(env) = setup("sample.mp4").await else { return };
    assert!(env.info.duration_secs > 1.0);

    let font = font_path();
    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let out = env.tmp.path().join("sample_contact_sheet.png");
    let opts = SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 8,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: true, show_header: true, header_title: false,
        format: OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(), theme: SheetTheme::Dark,
    };
    contact_sheet::generate(&env.fixture, &env.info, &out, &opts, &font, &ctx).await.unwrap();
    assert!(out.exists(), "sheet not written");
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);

    let shots_dir = env.tmp.path().join("shots");
    let shots_opts = ScreenshotsOptions {
        count: 3, format: OutputFormat::Png, jpeg_quality: 92, suffix: String::new(),
    };
    let outs = screenshots::generate(&env.fixture, &env.info, &shots_dir, &shots_opts, &ctx).await.unwrap();
    assert_eq!(outs.len(), 3);
    for p in outs { assert!(p.exists()); }
}

/// Renders `text` with drawtext twice, once escaped inline and once read
/// raw from a file with expansion off, and returns both frames' pixels.
fn render_both_ways(ffmpeg: &std::path::Path, dir: &std::path::Path, text: &str) -> (Vec<u8>, Vec<u8>) {
    let font = mosaic_lib::drawtext::font_for_ffmpeg(&font_path());
    let raw = dir.join("raw.txt");
    std::fs::write(&raw, text).unwrap();
    let render = |filter: String| {
        let out = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=900x40"])
            .args(["-frames:v", "1", "-vf", &filter, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{text:?}: {}", String::from_utf8_lossy(&out.stderr));
        out.stdout
    };
    let style = format!("fontfile='{font}':fontsize=20:fontcolor=white:x=5:y=5");
    let raw_path = mosaic_lib::drawtext::font_for_ffmpeg(&raw);
    let expected = render(format!("drawtext=textfile='{raw_path}':expansion=none:{style}"));
    let escaped = render(format!("drawtext=text='{}':{style}", mosaic_lib::drawtext::escape_drawtext(text)));
    (escaped, expected)
}

#[tokio::test]
async fn escaped_drawtext_renders_special_characters_literally() {
    let Some(env) = setup("sample.mp4").await else { return };
    for text in [
        "Now You See Me Now You Don't (2025) [WEBDL-2160p-AV1-Opus].mkv",
        "it's 100% done",
        r"back\slash and %{pts}",
        "Size: 1 | a,b;c[d]",
    ] {
        let (escaped, expected) = render_both_ways(&env.tools.ffmpeg, env.tmp.path(), text);
        assert!(escaped == expected, "{text:?} rendered differently from the raw text");
    }
}

#[tokio::test]
async fn contact_sheet_handles_quotes_and_percent_in_the_filename_and_font_path() {
    let Some(env) = setup("sample.mp4").await else { return };
    let video = env.tmp.path().join("Now You Don't, 100% (2025).mp4");
    std::fs::copy(&env.fixture, &video).unwrap();
    let font = env.tmp.path().join("O'Brien's fonts").join("DejaVuSans.ttf");
    std::fs::create_dir(font.parent().unwrap()).unwrap();
    std::fs::copy(font_path(), &font).unwrap();
    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);
    let out = env.tmp.path().join("sheet.png");
    let opts = SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 8,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: true, show_header: true, header_title: false,
        format: OutputFormat::Png, jpeg_quality: 92,
        suffix: String::new(), theme: SheetTheme::Dark,
    };
    contact_sheet::generate(&video, &env.info, &out, &opts, &font, &ctx).await.unwrap();
    assert!(out.exists());
}

#[tokio::test]
async fn end_to_end_animated_preview_reel() {
    let Some(env) = setup("sample.mp4").await else { return };
    assert!(env.info.duration_secs > 1.0);

    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let out = env.tmp.path().join("sample - reel.webp");
    let opts = PreviewOptions {
        count: 3, clip_length_secs: 1, height: 240, fps: 12, quality: 60,
        suffix: String::new(), format: ReelFormat::Webp,
    };
    preview_reel::generate(&env.fixture, &env.info, &out, &opts, &ctx).await.unwrap();

    assert!(out.exists(), "reel not written");
    assert_animated_webp(&std::fs::read(&out).unwrap());
}

#[tokio::test]
async fn end_to_end_animated_preview_reel_webm() {
    let Some(env) = setup("sample.mp4").await else { return };

    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let out = env.tmp.path().join("sample - reel.webm");
    let opts = PreviewOptions {
        count: 2, clip_length_secs: 1, height: 240, fps: 12, quality: 50,
        suffix: String::new(), format: ReelFormat::Webm,
    };
    preview_reel::generate(&env.fixture, &env.info, &out, &opts, &ctx).await.unwrap();

    assert!(out.exists(), "webm reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "webm suspiciously small: {} bytes", bytes.len());
    // WebM is a Matroska subset; first four bytes are the EBML header magic.
    assert_eq!(&bytes[0..4], &[0x1A, 0x45, 0xDF, 0xA3], "missing EBML magic");
}

#[tokio::test]
async fn end_to_end_animated_contact_sheet() {
    let Some(env) = setup("sample.mp4").await else { return };

    let font = font_path();
    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let out = env.tmp.path().join("sample_animated_sheet.webp");
    let opts = AnimatedSheetOptions {
        cols: 2, rows: 2, width: 640, gap: 8,
        clip_length_secs: 1, fps: 8, quality: 60,
        thumb_font_size: 12, header_font_size: 14,
        show_timestamps: true, show_header: true, header_title: false,
        suffix: String::new(), theme: SheetTheme::Dark,
    };
    animated_sheet::generate(&env.fixture, &env.info, &out, &opts, &font, &ctx).await.unwrap();

    assert!(out.exists(), "animated sheet not written");
    assert_animated_webp(&std::fs::read(&out).unwrap());
}

#[tokio::test]
async fn end_to_end_animated_contact_sheet_no_header() {
    let Some(env) = setup("sample.mp4").await else { return };

    let font = font_path();
    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let out = env.tmp.path().join("sample_bare.webp");
    let opts = AnimatedSheetOptions {
        cols: 2, rows: 2, width: 480, gap: 0,
        clip_length_secs: 1, fps: 6, quality: 60,
        thumb_font_size: 12, header_font_size: 14,
        show_timestamps: false, show_header: false, header_title: false,
        suffix: String::new(), theme: SheetTheme::Light,
    };
    animated_sheet::generate(&env.fixture, &env.info, &out, &opts, &font, &ctx).await.unwrap();

    assert!(out.exists(), "bare animated sheet not written");
    assert_animated_webp(&std::fs::read(&out).unwrap());
}

#[tokio::test]
async fn end_to_end_animated_preview_reel_gif() {
    let Some(env) = setup("sample.mp4").await else { return };

    let reporter = ProgressReporter { emit: &|_, _, _| {} };
    let ctx = env.ctx(&reporter);

    let out = env.tmp.path().join("sample - reel.gif");
    let opts = PreviewOptions {
        count: 2, clip_length_secs: 1, height: 240, fps: 10,
        quality: 0, // ignored for gif
        suffix: String::new(), format: ReelFormat::Gif,
    };
    preview_reel::generate(&env.fixture, &env.info, &out, &opts, &ctx).await.unwrap();

    assert!(out.exists(), "gif reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "gif suspiciously small: {} bytes", bytes.len());
    // GIF signature: "GIF87a" or "GIF89a" — both start with "GIF8".
    assert_eq!(&bytes[0..4], b"GIF8", "missing GIF magic");
}
