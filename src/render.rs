use crate::image_loader::RgbaImage;

/// Background color: #1a1a1a
pub const BG_COLOR: u32 = 0x001a1a1a;

/// Scale an RGBA image to fit within (max_w, max_h) preserving aspect ratio.
pub fn scale_to_fit(img: &RgbaImage, max_w: u32, max_h: u32) -> RgbaImage {
    let (src_w, src_h) = img.dimensions();
    if src_w == 0 || src_h == 0 || max_w == 0 || max_h == 0 {
        return RgbaImage::new(1, 1);
    }

    let scale = (max_w as f64 / src_w as f64).min(max_h as f64 / src_h as f64);
    let dst_w = ((src_w as f64 * scale).round() as u32).max(1);
    let dst_h = ((src_h as f64 * scale).round() as u32).max(1);

    resize_rgba(img, dst_w, dst_h)
}

/// Scale an RGBA image by a zoom factor.
pub fn scale_by_factor(img: &RgbaImage, factor: f64) -> RgbaImage {
    let (src_w, src_h) = img.dimensions();
    let dst_w = ((src_w as f64 * factor).round() as u32).max(1);
    let dst_h = ((src_h as f64 * factor).round() as u32).max(1);
    resize_rgba(img, dst_w, dst_h)
}

/// Resize RGBA image using bilinear interpolation.
fn resize_rgba(src: &RgbaImage, dst_w: u32, dst_h: u32) -> RgbaImage {
    let (src_w, src_h) = src.dimensions();
    if src_w == dst_w && src_h == dst_h {
        return src.clone();
    }

    let raw = src.as_raw();
    let mut out = vec![0u8; (dst_w * dst_h * 4) as usize];

    let x_ratio = if dst_w > 1 {
        (src_w - 1) as f64 / (dst_w - 1) as f64
    } else {
        0.0
    };
    let y_ratio = if dst_h > 1 {
        (src_h - 1) as f64 / (dst_h - 1) as f64
    } else {
        0.0
    };

    for dy in 0..dst_h {
        let sy = y_ratio * dy as f64;
        let y0 = sy as u32;
        let y1 = (y0 + 1).min(src_h - 1);
        let fy = sy - y0 as f64;

        for dx in 0..dst_w {
            let sx = x_ratio * dx as f64;
            let x0 = sx as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let fx = sx - x0 as f64;

            let i00 = ((y0 * src_w + x0) * 4) as usize;
            let i10 = ((y0 * src_w + x1) * 4) as usize;
            let i01 = ((y1 * src_w + x0) * 4) as usize;
            let i11 = ((y1 * src_w + x1) * 4) as usize;

            let dst_idx = ((dy * dst_w + dx) * 4) as usize;
            for c in 0..4 {
                let v00 = raw[i00 + c] as f64;
                let v10 = raw[i10 + c] as f64;
                let v01 = raw[i01 + c] as f64;
                let v11 = raw[i11 + c] as f64;
                let v = v00 * (1.0 - fx) * (1.0 - fy)
                    + v10 * fx * (1.0 - fy)
                    + v01 * (1.0 - fx) * fy
                    + v11 * fx * fy;
                out[dst_idx + c] = v.round() as u8;
            }
        }
    }

    RgbaImage {
        data: out,
        width: dst_w,
        height: dst_h,
    }
}

/// Composite a scaled image centered on a background buffer of given dimensions.
/// Returns the XRGB pixel buffer.
pub fn composite_centered(
    img: &RgbaImage,
    win_w: u32,
    win_h: u32,
    offset_x: i32,
    offset_y: i32,
) -> Vec<u32> {
    let (img_w, img_h) = img.dimensions();
    let mut buf = vec![BG_COLOR; (win_w * win_h) as usize];

    // Center position plus pan offset
    let cx = (win_w as i32 - img_w as i32) / 2 + offset_x;
    let cy = (win_h as i32 - img_h as i32) / 2 + offset_y;

    let raw = img.as_raw();
    for iy in 0..img_h as i32 {
        let dy = cy + iy;
        if dy < 0 || dy >= win_h as i32 {
            continue;
        }
        for ix in 0..img_w as i32 {
            let dx = cx + ix;
            if dx < 0 || dx >= win_w as i32 {
                continue;
            }
            let src_idx = (iy as u32 * img_w + ix as u32) as usize * 4;
            let r = raw[src_idx] as u32;
            let g = raw[src_idx + 1] as u32;
            let b = raw[src_idx + 2] as u32;
            let a = raw[src_idx + 3] as u32;

            let dst_idx = (dy as u32 * win_w + dx as u32) as usize;
            if a == 255 {
                buf[dst_idx] = (r << 16) | (g << 8) | b;
            } else if a > 0 {
                let bg_r = (BG_COLOR >> 16) & 0xFF;
                let bg_g = (BG_COLOR >> 8) & 0xFF;
                let bg_b = BG_COLOR & 0xFF;
                let out_r = (r * a + bg_r * (255 - a)) / 255;
                let out_g = (g * a + bg_g * (255 - a)) / 255;
                let out_b = (b * a + bg_b * (255 - a)) / 255;
                buf[dst_idx] = (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }
    buf
}

/// Generate a thumbnail: scale image to fit within thumb_size x thumb_size.
pub fn generate_thumbnail(img: &RgbaImage, thumb_size: u32) -> RgbaImage {
    scale_to_fit(img, thumb_size, thumb_size)
}

/// Draw a filled rectangle with a given XRGB color onto the buffer.
pub fn fill_rect(buf: &mut [u32], buf_w: u32, x: u32, y: u32, w: u32, h: u32, color: u32) {
    for row in y..y.saturating_add(h) {
        if row >= buf.len() as u32 / buf_w {
            break;
        }
        for col in x..x.saturating_add(w) {
            if col >= buf_w {
                break;
            }
            buf[(row * buf_w + col) as usize] = color;
        }
    }
}

/// Draw a semi-transparent dark overlay (for status bar background).
/// Blends a dark color at given alpha over existing pixels.
pub fn draw_overlay(buf: &mut [u32], buf_w: u32, x: u32, y: u32, w: u32, h: u32, alpha: u32) {
    let ov_r: u32 = 0;
    let ov_g: u32 = 0;
    let ov_b: u32 = 0;
    for row in y..y.saturating_add(h) {
        if row >= buf.len() as u32 / buf_w.max(1) {
            break;
        }
        for col in x..x.saturating_add(w) {
            if col >= buf_w {
                break;
            }
            let idx = (row * buf_w + col) as usize;
            if idx >= buf.len() {
                break;
            }
            let existing = buf[idx];
            let bg_r = (existing >> 16) & 0xFF;
            let bg_g = (existing >> 8) & 0xFF;
            let bg_b = existing & 0xFF;
            let out_r = (ov_r * alpha + bg_r * (255 - alpha)) / 255;
            let out_g = (ov_g * alpha + bg_g * (255 - alpha)) / 255;
            let out_b = (ov_b * alpha + bg_b * (255 - alpha)) / 255;
            buf[idx] = (out_r << 16) | (out_g << 8) | out_b;
        }
    }
}

/// Blit an RGBA thumbnail onto an XRGB buffer at position (dx, dy), centered within
/// a cell of (cell_w, cell_h).
pub fn blit_thumbnail(
    buf: &mut [u32],
    buf_w: u32,
    buf_h: u32,
    thumb: &RgbaImage,
    dx: u32,
    dy: u32,
    cell_w: u32,
    cell_h: u32,
) {
    let (tw, th) = thumb.dimensions();
    let ox = dx + (cell_w.saturating_sub(tw)) / 2;
    let oy = dy + (cell_h.saturating_sub(th)) / 2;
    let raw = thumb.as_raw();

    for iy in 0..th {
        let py = oy + iy;
        if py >= buf_h {
            break;
        }
        for ix in 0..tw {
            let px = ox + ix;
            if px >= buf_w {
                break;
            }
            let src = (iy * tw + ix) as usize * 4;
            let r = raw[src] as u32;
            let g = raw[src + 1] as u32;
            let b = raw[src + 2] as u32;
            let a = raw[src + 3] as u32;
            let dst = (py * buf_w + px) as usize;
            if a == 255 {
                buf[dst] = (r << 16) | (g << 8) | b;
            } else if a > 0 {
                let bg_r = (buf[dst] >> 16) & 0xFF;
                let bg_g = (buf[dst] >> 8) & 0xFF;
                let bg_b = buf[dst] & 0xFF;
                let out_r = (r * a + bg_r * (255 - a)) / 255;
                let out_g = (g * a + bg_g * (255 - a)) / 255;
                let out_b = (b * a + bg_b * (255 - a)) / 255;
                buf[dst] = (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }
}
