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

fn format_file_size(bytes: u64) -> String {
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
fn days_to_date(days: u64) -> (u64, u64, u64) {
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
