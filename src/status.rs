use crate::font;
use crate::render;
use std::fs;
use std::path::Path;

/// Format the status text for a given image file.
/// Format: "filename.jpg | 1920x1080 | 2.4 MB | 2025-01-15 14:30 | [3/42]"
pub fn format_status(path: &Path, img_w: u32, img_h: u32, index: usize, total: usize) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

    let size_str = match fs::metadata(path) {
        Ok(meta) => format_file_size(meta.len()),
        Err(_) => "? B".to_string(),
    };

    let mtime_str = match fs::metadata(path) {
        Ok(meta) => match meta.modified() {
            Ok(t) => format_system_time(t),
            Err(_) => "?".to_string(),
        },
        Err(_) => "?".to_string(),
    };

    format!(
        "{} | {}x{} | {} | {} | [{}/{}]",
        name,
        img_w,
        img_h,
        size_str,
        mtime_str,
        index + 1,
        total
    )
}

pub(crate) fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        let whole = bytes / 1_000_000;
        let frac = (bytes % 1_000_000) / 100_000;
        format!("{}.{} MB", whole, frac)
    } else if bytes >= 1_000 {
        let whole = bytes / 1_000;
        let frac = (bytes % 1_000) / 100;
        format!("{}.{} KB", whole, frac)
    } else {
        format!("{} B", bytes)
    }
}

fn format_system_time(t: std::time::SystemTime) -> String {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(dur) => {
            let secs = dur.as_secs();
            // Simple date formatting without chrono dependency
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;

            // Calculate year/month/day from days since epoch
            let (year, month, day) = days_to_date(days);
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                year, month, day, hours, minutes
            )
        }
        Err(_) => "?".to_string(),
    }
}

/// Convert days since Unix epoch to (year, month, day).
pub(crate) fn days_to_date(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Draw the status bar overlay onto an XRGB buffer.
pub fn draw_status_bar(buf: &mut [u32], buf_w: u32, buf_h: u32, text: &str) {
    if buf_w == 0 || buf_h == 0 {
        return;
    }

    let bar_h = font::GLYPH_H + 6; // 3px padding top and bottom
    let bar_y = buf_h.saturating_sub(bar_h);

    // Draw semi-transparent dark overlay
    let text_pixel_width = text.len() as u32 * font::GLYPH_W + 12; // 6px padding each side
    let bar_w = text_pixel_width.min(buf_w);
    render::draw_overlay(buf, buf_w, 0, bar_y, bar_w, bar_h, 160);

    // Draw text
    let text_x = 6;
    let text_y = bar_y + 3;
    font::draw_string(buf, buf_w, buf_h, text, text_x, text_y, 0x00DDDDDD);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size_bytes() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(999), "999 B");
    }

    #[test]
    fn test_format_file_size_kb() {
        assert_eq!(format_file_size(1000), "1.0 KB");
        assert_eq!(format_file_size(1500), "1.5 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(999_999), "999.9 KB");
    }

    #[test]
    fn test_format_file_size_mb() {
        assert_eq!(format_file_size(1_000_000), "1.0 MB");
        assert_eq!(format_file_size(2_400_000), "2.4 MB");
        assert_eq!(format_file_size(10_500_000), "10.5 MB");
    }

    #[test]
    fn test_days_to_date_epoch() {
        // Unix epoch: Jan 1, 1970 = day 0
        let (y, m, d) = days_to_date(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_date_known() {
        // 2025-01-15 = 20103 days since epoch
        // Let's verify with a well-known date: 2000-01-01 = 10957 days
        let (y, m, d) = days_to_date(10957);
        assert_eq!((y, m, d), (2000, 1, 1));
    }

    #[test]
    fn test_days_to_date_leap_year() {
        // 2000-02-29 = day 10957 + 59 = 11016 (2000 is a leap year)
        let (y, m, d) = days_to_date(11016);
        assert_eq!((y, m, d), (2000, 2, 29));
    }

    #[test]
    fn test_days_to_date_end_of_year() {
        // 1970-12-31 = day 364
        let (y, m, d) = days_to_date(364);
        assert_eq!((y, m, d), (1970, 12, 31));
    }
}
