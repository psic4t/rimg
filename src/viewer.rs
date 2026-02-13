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
        }
    }

    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0;
        self.pan_y = 0;
        self.current_frame = 0;
        self.next_frame_time = None;
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
            self.pan_x -= PAN_STEP;
        }
    }

    pub fn pan_right(&mut self) {
        if self.zoom > 1.0 {
            self.pan_x += PAN_STEP;
        }
    }

    pub fn pan_up(&mut self) {
        if self.zoom > 1.0 {
            self.pan_y -= PAN_STEP;
        }
    }

    pub fn pan_down(&mut self) {
        if self.zoom > 1.0 {
            self.pan_y += PAN_STEP;
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

        buf
    }
}
