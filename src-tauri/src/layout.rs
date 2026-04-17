#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetLayout {
    pub cols: u32,
    pub rows: u32,
    pub total: u32,
    pub thumb_w: u32,
    pub grid_w: u32,
}

pub fn compute_sheet_layout(cols: u32, rows: u32, width: u32, gap: u32) -> SheetLayout {
    let total = cols * rows;
    let padding = gap * (cols + 1);
    let raw = width.saturating_sub(padding) / cols;
    let thumb_w = raw - (raw % 2);
    let grid_w = padding + cols * thumb_w;
    SheetLayout { cols, rows, total, thumb_w, grid_w }
}

/// Timestamps (in seconds) for `n` evenly-spaced samples inside (0, duration).
/// Matches the original script: `interval = duration / (n + 1)`, `ts_i = i * interval`.
pub fn sample_timestamps(duration_secs: f64, n: u32) -> Vec<f64> {
    if n == 0 || duration_secs <= 0.0 { return Vec::new(); }
    let interval = duration_secs / (n as f64 + 1.0);
    (1..=n).map(|i| i as f64 * interval).collect()
}

/// Timestamps for animated output (reel, animated sheet) where each sample
/// starts a `clip_length_secs` segment. Constrained to (0, duration - clip_length)
/// so no clip runs past the video end — which can produce an empty mp4 with no
/// video stream and break the downstream stitch filter graph on some HEVC GOP
/// structures (e.g. 120fps transport streams).
///
/// Returns empty if the video is shorter than the clip length.
pub fn sample_clip_timestamps(duration_secs: f64, n: u32, clip_length_secs: f64) -> Vec<f64> {
    if n == 0 || duration_secs <= clip_length_secs { return Vec::new(); }
    let span = duration_secs - clip_length_secs;
    let interval = span / (n as f64 + 1.0);
    (1..=n).map(|i| i as f64 * interval).collect()
}

/// Line height (in pixels) used to stack drawtext lines: 1.3× font size, rounded.
pub fn line_height(font: u32) -> u32 {
    ((font as f64) * 1.3).round() as u32
}

pub fn header_height(header_font_size: u32, gap: u32) -> u32 {
    2 * line_height(header_font_size) + 2 * gap
}

pub fn pad_width_for_count(n: u32) -> usize {
    n.to_string().len().max(2)
}

/// xstack `layout=` expression for a uniform grid where every input has
/// identical padded size `step_w × step_h`. Cells fill row-by-row.
pub fn xstack_layout(cols: u32, rows: u32, step_w: u32, step_h: u32) -> String {
    let mut parts: Vec<String> = Vec::with_capacity((cols * rows) as usize);
    for r in 0..rows {
        for c in 0..cols {
            parts.push(format!("{}_{}", c * step_w, r * step_h));
        }
    }
    parts.join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_3x7_1920_gap10() {
        let l = compute_sheet_layout(3, 7, 1920, 10);
        assert_eq!(l.total, 21);
        assert_eq!(l.thumb_w, 626);
        assert_eq!(l.grid_w, 1918);
    }

    #[test]
    fn thumb_width_forced_even() {
        // 4 cols * 10 gap = 50 padding, 1920-50 = 1870, /4 = 467 (odd) → 466
        let l = compute_sheet_layout(4, 2, 1920, 10);
        assert_eq!(l.thumb_w % 2, 0);
    }

    #[test]
    fn timestamps_evenly_spaced_in_open_interval() {
        let ts = sample_timestamps(100.0, 4);
        assert_eq!(ts.len(), 4);
        for (i, v) in ts.iter().enumerate() {
            let expected = (i as f64 + 1.0) * 100.0 / 5.0;
            assert!((v - expected).abs() < 1e-9, "ts[{}]={} expected {}", i, v, expected);
        }
        assert!(ts[0] > 0.0);
        assert!(*ts.last().unwrap() < 100.0);
    }

    #[test]
    fn timestamps_zero_count_returns_empty() {
        assert!(sample_timestamps(100.0, 0).is_empty());
    }

    #[test]
    fn clip_timestamps_leave_room_for_clip_length() {
        // Reproduces the 120fps DoVi.P8 transport-stream bug: 28.63s video,
        // 18 cells, 2s clip — naive sampling gave last_ts=27.12 which overshot
        // the video end and produced an empty mp4 with no video stream.
        let ts = sample_clip_timestamps(28.63, 18, 2.0);
        assert_eq!(ts.len(), 18);
        // Every timestamp + clip_length must fit inside the video.
        for (i, v) in ts.iter().enumerate() {
            assert!(v + 2.0 <= 28.63, "ts[{}]={} + clip_length overshoots", i, v);
        }
        // And first > 0.
        assert!(ts[0] > 0.0);
    }

    #[test]
    fn clip_timestamps_return_empty_when_shorter_than_clip() {
        assert!(sample_clip_timestamps(1.0, 8, 2.0).is_empty());
        assert!(sample_clip_timestamps(2.0, 8, 2.0).is_empty());  // exactly equal, no room
    }

    #[test]
    fn header_height_default() {
        // font=20 → line_h=26; 2*26 + 2*10 = 72
        assert_eq!(header_height(20, 10), 72);
    }

    #[test]
    fn pad_width_for_count_floors_at_two() {
        assert_eq!(pad_width_for_count(1), 2);
        assert_eq!(pad_width_for_count(9), 2);
        assert_eq!(pad_width_for_count(99), 2);
        assert_eq!(pad_width_for_count(100), 3);
        assert_eq!(pad_width_for_count(9999), 4);
    }

    #[test]
    fn xstack_layout_1x1() {
        assert_eq!(xstack_layout(1, 1, 100, 50), "0_0");
    }

    #[test]
    fn xstack_layout_3x2_fills_row_by_row() {
        assert_eq!(
            xstack_layout(3, 2, 110, 60),
            "0_0|110_0|220_0|0_60|110_60|220_60"
        );
    }

    #[test]
    fn xstack_layout_4x4_has_16_positions() {
        let s = xstack_layout(4, 4, 100, 80);
        assert_eq!(s.split('|').count(), 16);
        assert!(s.starts_with("0_0|100_0|200_0|300_0|"));
        assert!(s.ends_with("|300_240"));
    }
}
