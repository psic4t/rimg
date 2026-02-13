use crate::font;
use crate::image_loader::LoadedImage;
use crate::image_loader::RgbaImage;
use crate::render;
use crate::status;
use std::path::Path;
use std::time::Instant;

/// Zoom step factor.
const ZOOM_STEP: f64 = 1.25;
/// Pan step in pixels.
const PAN_STEP: i32 = 50;

pub struct Viewer {
    /// Current zoom level (1.0 = fit-to-window).
    zoom: f64,
    /// Pan offset from center (in display pixels).
    pan_x: i32,
    pan_y: i32,
    /// Fit-to-window scale factor for current image + window size.
    fit_scale: f64,

    // Animation state
    pub current_frame: usize,
    pub next_frame_time: Option<Instant>,

    // EXIF overlay state
    show_exif: bool,
    exif_lines: Vec<String>,
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0,
            pan_y: 0,
            fit_scale: 1.0,
            current_frame: 0,
            next_frame_time: None,
            show_exif: false,
            exif_lines: Vec::new(),
        }
    }

    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0;
        self.pan_y = 0;
        self.current_frame = 0;
        self.next_frame_time = None;
        self.show_exif = false;
    }

    pub fn toggle_exif(&mut self) {
        self.show_exif = !self.show_exif;
    }

    pub fn hide_exif(&mut self) {
        self.show_exif = false;
    }

    pub fn is_exif_visible(&self) -> bool {
        self.show_exif
    }

    pub fn set_exif_data(&mut self, tags: Vec<(String, String)>) {
        self.exif_lines = if tags.is_empty() {
            vec!["No EXIF data".to_string()]
        } else {
            tags.into_iter()
                .map(|(label, value)| format!("{}: {}", label, value))
                .collect()
        };
    }

    pub fn zoom_in(&mut self) {
        self.zoom *= ZOOM_STEP;
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / ZOOM_STEP).max(1.0);
        if self.zoom <= 1.0 {
            self.pan_x = 0;
            self.pan_y = 0;
        }
    }

    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0;
        self.pan_y = 0;
    }

    pub fn pan_left(&mut self) {
        if self.zoom > 1.0 {
            self.pan_x += PAN_STEP;
        }
    }

    pub fn pan_right(&mut self) {
        if self.zoom > 1.0 {
            self.pan_x -= PAN_STEP;
        }
    }

    pub fn pan_up(&mut self) {
        if self.zoom > 1.0 {
            self.pan_y += PAN_STEP;
        }
    }

    pub fn pan_down(&mut self) {
        if self.zoom > 1.0 {
            self.pan_y -= PAN_STEP;
        }
    }

    /// Start animation for a new animated image.
    pub fn start_animation(&mut self, loaded: &LoadedImage) {
        self.current_frame = 0;
        if let LoadedImage::Animated { frames } = loaded {
            if !frames.is_empty() {
                self.next_frame_time = Some(Instant::now() + frames[0].1);
            }
        }
    }

    /// Advance animation frame if the timer has elapsed.
    /// Returns true if a frame was advanced (needs redraw).
    pub fn advance_frame(&mut self, loaded: &LoadedImage) -> bool {
        if let LoadedImage::Animated { frames } = loaded {
            if let Some(deadline) = self.next_frame_time {
                if Instant::now() >= deadline {
                    self.current_frame = (self.current_frame + 1) % frames.len();
                    let delay = frames[self.current_frame].1;
                    self.next_frame_time = Some(Instant::now() + delay);
                    return true;
                }
            }
        }
        false
    }

    /// Get the delay until the next frame (for ControlFlow::WaitUntil).
    pub fn next_frame_deadline(&self) -> Option<Instant> {
        self.next_frame_time
    }

    /// Render the current view into an XRGB pixel buffer.
    pub fn render(
        &mut self,
        loaded: &LoadedImage,
        win_w: u32,
        win_h: u32,
        path: &Path,
        index: usize,
        total: usize,
    ) -> Vec<u32> {
        if win_w == 0 || win_h == 0 {
            return vec![];
        }

        // Get the current frame
        let frame: &RgbaImage = match loaded {
            LoadedImage::Static(img) => img,
            LoadedImage::Animated { frames } => &frames[self.current_frame.min(frames.len() - 1)].0,
        };

        let (src_w, src_h) = frame.dimensions();
        if src_w == 0 || src_h == 0 {
            return vec![render::BG_COLOR; (win_w * win_h) as usize];
        }

        // Calculate fit-to-window scale
        self.fit_scale = (win_w as f64 / src_w as f64).min(win_h as f64 / src_h as f64);
        let actual_scale = self.fit_scale * self.zoom;

        // Scale image
        let scaled = render::scale_by_factor(frame, actual_scale);
        let (scaled_w, scaled_h) = scaled.dimensions();

        // Clamp pan to keep image edges within window
        let max_pan_x = ((scaled_w as i32 - win_w as i32) / 2).max(0);
        let max_pan_y = ((scaled_h as i32 - win_h as i32) / 2).max(0);
        self.pan_x = self.pan_x.clamp(-max_pan_x, max_pan_x);
        self.pan_y = self.pan_y.clamp(-max_pan_y, max_pan_y);

        // Composite onto background
        let mut buf = render::composite_centered(&scaled, win_w, win_h, self.pan_x, self.pan_y);

        // Draw status bar
        let status_text = status::format_status(path, src_w, src_h, index, total);
        status::draw_status_bar(&mut buf, win_w, win_h, &status_text);

        // Draw EXIF overlay
        if self.show_exif && !self.exif_lines.is_empty() {
            self.draw_exif_overlay(&mut buf, win_w, win_h);
        }

        buf
    }

    fn draw_exif_overlay(&self, buf: &mut [u32], win_w: u32, win_h: u32) {
        let padding: u32 = 8;
        let margin: u32 = 10;
        let line_h = font::GLYPH_H + 2; // 2px spacing between lines
        let radius: u32 = 6;

        // Calculate overlay dimensions
        let max_line_len = self.exif_lines.iter().map(|l| l.len()).max().unwrap_or(0) as u32;
        let overlay_w = max_line_len * font::GLYPH_W + padding * 2;
        let overlay_h = self.exif_lines.len() as u32 * line_h + padding * 2 - 2; // -2: no trailing spacing

        // Position at top-right
        let overlay_x = win_w.saturating_sub(overlay_w + margin);
        let overlay_y = margin;

        // Clamp to window
        let overlay_w = overlay_w.min(win_w.saturating_sub(margin));
        let overlay_h = overlay_h.min(win_h.saturating_sub(margin * 2));

        // Draw rounded dark overlay (same style as status bar: alpha 160)
        render::draw_overlay_rounded(
            buf, win_w, overlay_x, overlay_y, overlay_w, overlay_h, 160, radius,
        );

        // Draw text lines (same color as status bar: 0x00DDDDDD)
        let text_x = overlay_x + padding;
        let mut text_y = overlay_y + padding;
        for line in &self.exif_lines {
            if text_y + font::GLYPH_H > overlay_y + overlay_h {
                break;
            }
            font::draw_string(buf, win_w, win_h, line, text_x, text_y, 0x00DDDDDD);
            text_y += line_h;
        }
    }
}
