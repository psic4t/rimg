use crate::image_loader;
use crate::image_loader::RgbaImage;
use crate::render;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

/// Thumbnail size in pixels.
const THUMB_SIZE: u32 = 200;
/// Gap between thumbnails.
const GAP: u32 = 10;
/// Padding from window edges.
const PADDING: u32 = 10;
/// Selection border width.
const BORDER_W: u32 = 3;
/// Selection border color (white-ish).
const SELECTION_COLOR: u32 = 0x00CCCCCC;
/// Placeholder color (dark gray).
const PLACEHOLDER_COLOR: u32 = 0x00333333;

pub struct Gallery {
    /// Selected index in the image list.
    pub selected: usize,
    /// Vertical scroll offset in pixels.
    scroll_y: u32,
    /// Cached thumbnails.
    thumbnails: HashMap<usize, RgbaImage>,
    /// Number of columns in the current layout.
    cols: usize,
    /// Sender to dispatch thumbnail generation requests to the worker.
    work_tx: mpsc::Sender<Vec<(usize, PathBuf)>>,
    /// Receiver for completed thumbnails from the worker.
    result_rx: mpsc::Receiver<(usize, RgbaImage)>,
    /// Indices sent to worker but not yet received.
    pending: HashSet<usize>,
}

impl Gallery {
    pub fn new() -> Self {
        // Channel: main -> worker (batches of work)
        let (work_tx, work_rx) = mpsc::channel::<Vec<(usize, PathBuf)>>();
        // Channel: worker -> main (completed thumbnails)
        let (result_tx, result_rx) = mpsc::channel::<(usize, RgbaImage)>();

        // Spawn background worker thread
        thread::spawn(move || {
            while let Ok(batch) = work_rx.recv() {
                for (index, path) in batch {
                    if let Ok(thumb) = image_loader::load_image_thumbnail(&path, THUMB_SIZE) {
                        if result_tx.send((index, thumb)).is_err() {
                            return; // Main thread dropped receiver, exit
                        }
                    }
                }
            }
            // work_rx disconnected, exit cleanly
        });

        Self {
            selected: 0,
            scroll_y: 0,
            thumbnails: HashMap::new(),
            cols: 1,
            work_tx,
            result_rx,
            pending: HashSet::new(),
        }
    }

    /// Set selected index (when switching from viewer).
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }

    fn cell_size() -> u32 {
        THUMB_SIZE + GAP
    }

    fn calc_cols(&self, win_w: u32) -> usize {
        let usable = win_w.saturating_sub(PADDING * 2 + GAP);
        ((usable / Self::cell_size()) as usize).max(1)
    }

    /// Move selection left.
    pub fn move_left(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection right.
    pub fn move_right(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        if self.selected + 1 < total {
            self.selected += 1;
        }
    }

    /// Move selection up one row.
    pub fn move_up(&mut self, _total: usize) {
        if self.selected >= self.cols {
            self.selected -= self.cols;
        }
    }

    /// Move selection down one row.
    pub fn move_down(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        if self.selected + self.cols < total {
            self.selected += self.cols;
        }
    }

    /// Jump to first.
    pub fn go_first(&mut self) {
        self.selected = 0;
        self.scroll_y = 0;
    }

    /// Jump to last.
    pub fn go_last(&mut self, total: usize) {
        if total > 0 {
            self.selected = total - 1;
        }
    }

    /// Returns true if there are thumbnail requests pending in the worker.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Poll for completed thumbnails from the background worker.
    /// Returns true if any new thumbnails were received.
    pub fn poll_thumbnails(&mut self) -> bool {
        let mut received = false;
        while let Ok((index, thumb)) = self.result_rx.try_recv() {
            self.thumbnails.insert(index, thumb);
            self.pending.remove(&index);
            received = true;
        }
        received
    }

    /// Ensure the selected thumbnail is visible by adjusting scroll.
    fn ensure_visible(&mut self, win_h: u32) {
        let row = self.selected / self.cols;
        let cell = Self::cell_size();
        let y_top = PADDING + row as u32 * cell;
        let y_bottom = y_top + cell;

        if y_top < self.scroll_y {
            self.scroll_y = y_top.saturating_sub(PADDING);
        }
        if y_bottom > self.scroll_y + win_h {
            self.scroll_y = y_bottom.saturating_sub(win_h) + PADDING;
        }
    }

    /// Render the gallery into an XRGB pixel buffer.
    pub fn render(&mut self, paths: &[PathBuf], win_w: u32, win_h: u32) -> Vec<u32> {
        if win_w == 0 || win_h == 0 {
            return vec![];
        }

        self.cols = self.calc_cols(win_w);
        self.ensure_visible(win_h);

        let total = paths.len();
        let cell = Self::cell_size();
        let grid_x_offset =
            PADDING + (win_w.saturating_sub(PADDING * 2 + self.cols as u32 * cell - GAP)) / 2;

        let mut buf = vec![render::BG_COLOR; (win_w * win_h) as usize];

        // Determine visible range
        let first_visible_row = (self.scroll_y / cell) as usize;
        let last_visible_row = ((self.scroll_y + win_h) / cell) as usize + 1;
        let first_visible = first_visible_row * self.cols;
        let last_visible = ((last_visible_row + 1) * self.cols).min(total);

        // Buffer zone: load one extra row above and below
        let load_start = first_visible.saturating_sub(self.cols);
        let load_end = (last_visible + self.cols).min(total);

        // Dispatch missing thumbnails to background worker
        let mut batch = Vec::new();
        for i in load_start..load_end {
            if !self.thumbnails.contains_key(&i) && !self.pending.contains(&i) {
                batch.push((i, paths[i].clone()));
                self.pending.insert(i);
            }
        }
        if !batch.is_empty() {
            let _ = self.work_tx.send(batch);
        }

        // Draw thumbnails
        for i in first_visible..last_visible.min(total) {
            let col = i % self.cols;
            let row = i / self.cols;

            let x = grid_x_offset + col as u32 * cell;
            let y = (PADDING + row as u32 * cell) as i32 - self.scroll_y as i32;

            if y + cell as i32 <= 0 || y >= win_h as i32 {
                continue; // Off screen
            }

            let dy = y.max(0) as u32;

            // Draw selection border or placeholder background
            if i == self.selected {
                // Selection: draw border
                let bx = x.saturating_sub(BORDER_W);
                let by = dy.saturating_sub(if y >= 0 { BORDER_W } else { 0 });
                let bw = THUMB_SIZE + BORDER_W * 2;
                let bh = THUMB_SIZE + BORDER_W * 2;
                render::fill_rect(&mut buf, win_w, bx, by, bw, bh, SELECTION_COLOR);
            }

            if let Some(thumb) = self.thumbnails.get(&i) {
                render::blit_thumbnail(
                    &mut buf, win_w, win_h, thumb, x, dy, THUMB_SIZE, THUMB_SIZE,
                );
            } else {
                // Placeholder
                render::fill_rect(
                    &mut buf,
                    win_w,
                    x,
                    dy,
                    THUMB_SIZE,
                    THUMB_SIZE,
                    PLACEHOLDER_COLOR,
                );
            }
        }

        buf
    }
}
