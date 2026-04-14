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

pub fn header_height(header_font_size: u32, gap: u32) -> u32 {
    let line_h = ((header_font_size as f64) * 1.3).round() as u32;
    2 * line_h + 2 * gap
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
    fn header_height_default() {
        // font=20 → line_h=26; 2*26 + 2*10 = 72
        assert_eq!(header_height(20, 10), 72);
    }
}
