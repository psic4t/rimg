use crate::font;
use crate::image_loader::LoadedImage;
use crate::image_loader::RgbaImage;
use crate::input::PanDirection;
use crate::render;
use crate::status;
use std::path::Path;
use std::time::{Duration, Instant};

/// Zoom step factor.
const ZOOM_STEP: f64 = 1.25;

/// Constant pan speed in pixels per second.
const PAN_SPEED: f64 = 600.0;
/// Target frame interval for pan animation (~60fps).
const PAN_FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Cache key for the scaled image: (actual_scale_bits, win_w, win_h, frame_index).
/// We store scale as u64 bits to get exact equality checks.
type ScaleCacheKey = (u64, u32, u32, usize);

pub struct Viewer {
    /// Current zoom level (1.0 = fit-to-window).
    zoom: f64,
    /// Pan offset from center (integer, for rendering).
    pan_x: i32,
    pan_y: i32,
    /// Pan offset from center (floating-point, for smooth sub-pixel movement).
    pan_x_f: f64,
    pan_y_f: f64,
    /// Which pan directions are currently held down.
    pan_active: [bool; 4],
    /// Timestamp of last pan animation tick.
    last_pan_tick: Option<Instant>,
    /// Fit-to-window scale factor for current image + window size.
    fit_scale: f64,

    /// Cached scaled image to avoid re-scaling every frame during panning.
    scaled_cache: Option<RgbaImage>,
    scaled_cache_key: ScaleCacheKey,

    // Animation state
    pub current_frame: usize,
    pub next_frame_time: Option<Instant>,

    /// Whether to scale small images up to fit the window.
    fit_to_window: bool,

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
            pan_x_f: 0.0,
            pan_y_f: 0.0,
            pan_active: [false; 4],
            last_pan_tick: None,
            fit_scale: 1.0,
            scaled_cache: None,
            scaled_cache_key: (0, 0, 0, 0),
            current_frame: 0,
            next_frame_time: None,
            fit_to_window: false,
            show_exif: false,
            exif_lines: Vec::new(),
        }
    }

    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0;
        self.pan_y = 0;
        self.pan_x_f = 0.0;
        self.pan_y_f = 0.0;
        self.pan_active = [false; 4];
        self.last_pan_tick = None;
        self.scaled_cache = None;
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
            self.stop_all_pan();
        }
    }

    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
        self.stop_all_pan();
    }

    pub fn toggle_fit_to_window(&mut self) {
        self.fit_to_window = !self.fit_to_window;
        self.zoom = 1.0;
        self.stop_all_pan();
        self.scaled_cache = None;
    }

    /// Start panning in the given direction.
    pub fn pan_start(&mut self, dir: PanDirection) {
        if self.zoom <= 1.0 {
            return;
        }
        self.pan_active[dir as usize] = true;
        if self.last_pan_tick.is_none() {
            self.last_pan_tick = Some(Instant::now());
        }
    }

    /// Stop panning in the given direction (key released).
    pub fn pan_stop(&mut self, dir: PanDirection) {
        self.pan_active[dir as usize] = false;
    }

    /// Reset all pan state to zero.
    fn stop_all_pan(&mut self) {
        self.pan_x = 0;
        self.pan_y = 0;
        self.pan_x_f = 0.0;
        self.pan_y_f = 0.0;
        self.pan_active = [false; 4];
        self.last_pan_tick = None;
    }

    /// Update pan position at constant speed based on which keys are held.
    /// Returns true if any pan key is active (needs continued redraws).
    pub fn update_pan(&mut self) -> bool {
        if !self.is_pan_animating() {
            self.last_pan_tick = None;
            return false;
        }

        if self.zoom <= 1.0 {
            self.stop_all_pan();
            return false;
        }

        let now = Instant::now();
        let dt = if let Some(last) = self.last_pan_tick {
            let elapsed = now.duration_since(last).as_secs_f64();
            elapsed.min(0.1) // Cap to avoid huge jumps if the app stalls
        } else {
            0.0
        };
        self.last_pan_tick = Some(now);

        if dt <= 0.0 {
            return true;
        }

        // Compute direction from active keys (immediate full speed, no ramp)
        let mut dx: f64 = 0.0;
        let mut dy: f64 = 0.0;
        if self.pan_active[PanDirection::Left as usize] {
            dx += 1.0;
        }
        if self.pan_active[PanDirection::Right as usize] {
            dx -= 1.0;
        }
        if self.pan_active[PanDirection::Up as usize] {
            dy += 1.0;
        }
        if self.pan_active[PanDirection::Down as usize] {
            dy -= 1.0;
        }

        // Normalize diagonal so it doesn't move faster
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 {
            dx /= len;
            dy /= len;
        }

        // Move at constant speed
        self.pan_x_f += dx * PAN_SPEED * dt;
        self.pan_y_f += dy * PAN_SPEED * dt;

        // Convert to integer for rendering
        self.pan_x = self.pan_x_f.round() as i32;
        self.pan_y = self.pan_y_f.round() as i32;

        true
    }

    /// Returns true if any pan key is currently held.
    pub fn is_pan_animating(&self) -> bool {
        self.pan_active.iter().any(|&a| a)
    }

    /// Returns the deadline for the next pan animation frame, if animating.
    pub fn pan_deadline(&self) -> Option<Instant> {
        if self.is_pan_animating() {
            Some(Instant::now() + PAN_FRAME_INTERVAL)
        } else {
            None
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
        let scale = (win_w as f64 / src_w as f64).min(win_h as f64 / src_h as f64);
        self.fit_scale = if self.fit_to_window {
            scale
        } else {
            scale.min(1.0)
        };
        let actual_scale = self.fit_scale * self.zoom;

        // Scale image (cached — only recompute when zoom/window/frame changes)
        let frame_idx = match loaded {
            LoadedImage::Static(_) => 0,
            LoadedImage::Animated { .. } => self.current_frame,
        };
        let cache_key: ScaleCacheKey = (actual_scale.to_bits(), win_w, win_h, frame_idx);
        if self.scaled_cache.is_none() || self.scaled_cache_key != cache_key {
            self.scaled_cache = Some(render::scale_by_factor(frame, actual_scale));
            self.scaled_cache_key = cache_key;
        }
        let scaled = self.scaled_cache.as_ref().unwrap();
        let (scaled_w, scaled_h) = scaled.dimensions();

        // Clamp pan to keep image edges within window
        let max_pan_x = ((scaled_w as i32 - win_w as i32) / 2).max(0);
        let max_pan_y = ((scaled_h as i32 - win_h as i32) / 2).max(0);
        self.pan_x = self.pan_x.clamp(-max_pan_x, max_pan_x);
        self.pan_y = self.pan_y.clamp(-max_pan_y, max_pan_y);
        // Keep floating-point in sync with clamped integer values
        self.pan_x_f = self.pan_x_f.clamp(-max_pan_x as f64, max_pan_x as f64);
        self.pan_y_f = self.pan_y_f.clamp(-max_pan_y as f64, max_pan_y as f64);

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
