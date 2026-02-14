use crate::gallery::Gallery;
use crate::image_loader::{self, LoadedImage};
use crate::input::{Action, Mode, PanDirection};
use crate::viewer::Viewer;
use crate::wayland::{WaylandEvent, WaylandState};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wayland_client::Connection;

/// Duration to show transient error messages in the status bar.
const ERROR_DISPLAY_DURATION: Duration = Duration::from_secs(3);
/// Duration to show the sort mode toast overlay.
const TOAST_DISPLAY_DURATION: Duration = Duration::from_millis(1500);

/// Sort mode for image list ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Name,
    Size,
    ExifDate,
    ModTime,
}

impl SortMode {
    fn next(self) -> Self {
        match self {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::ExifDate,
            SortMode::ExifDate => SortMode::ModTime,
            SortMode::ModTime => SortMode::Name,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::Size => "Size",
            SortMode::ExifDate => "EXIF Date",
            SortMode::ModTime => "Mod Time",
        }
    }
}

pub struct App {
    state: WaylandState,
    conn: Connection,
    paths: Vec<PathBuf>,
    current_index: usize,
    mode: Mode,
    viewer: Viewer,
    gallery: Gallery,
    image_cache: HashMap<usize, LoadedImage>,
    win_w: u32,
    win_h: u32,
    needs_redraw: bool,
    wallpaper_mode: bool,
    /// Transient error message for the status bar (auto-dismissed).
    error_message: Option<String>,
    /// Deadline after which the error message should be cleared.
    error_deadline: Option<Instant>,
    /// Current sort mode.
    sort_mode: SortMode,
    /// Toast overlay message (e.g., "Sort: Name").
    toast_message: Option<String>,
    /// Deadline after which the toast should be cleared.
    toast_deadline: Option<Instant>,
    /// Cached file metadata: path -> (size_bytes, mtime_secs).
    meta_cache: HashMap<PathBuf, (u64, u64)>,
    /// Cached EXIF dates: path -> Option<timestamp_secs>.
    exif_date_cache: HashMap<PathBuf, Option<u64>>,
}

impl App {
    pub fn new(paths: Vec<PathBuf>, wallpaper_mode: bool) -> Self {
        let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
        let state = WaylandState::new(wallpaper_mode);

        Self {
            state,
            conn,
            paths,
            current_index: 0,
            mode: Mode::Viewer,
            viewer: Viewer::new(),
            gallery: Gallery::new(),
            image_cache: HashMap::new(),
            win_w: 0,
            win_h: 0,
            needs_redraw: true,
            wallpaper_mode,
            error_message: None,
            error_deadline: None,
            sort_mode: SortMode::Name,
            toast_message: None,
            toast_deadline: None,
            meta_cache: HashMap::new(),
            exif_date_cache: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        if self.wallpaper_mode {
            self.run_wallpaper();
            return;
        }
        self.run_viewer();
    }

    fn run_viewer(&mut self) {
        let mut event_queue = self.conn.new_event_queue();
        let qh = event_queue.handle();

        // Register globals
        let display = self.conn.display();
        display.get_registry(&qh, ());

        // Initial roundtrip to bind all globals
        event_queue
            .roundtrip(&mut self.state)
            .expect("Roundtrip failed");

        // Second roundtrip to ensure everything is configured
        event_queue
            .roundtrip(&mut self.state)
            .expect("Roundtrip failed");

        // Load first image
        self.ensure_image_loaded();
        if let Some(loaded) = self.image_cache.get(&self.current_index) {
            self.viewer.start_animation(loaded);
        }
        self.load_exif_for_current();
        self.update_title();

        // Main event loop using poll
        // SAFETY: The connection fd is valid for the lifetime of self.conn
        let raw_fd = self.conn.backend().poll_fd().as_raw_fd();
        let wl_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };

        while self.state.running {
            // Flush outgoing messages
            let _ = self.conn.flush();

            // Calculate poll timeout based on GIF animation, pan animation, error dismiss, or pending thumbnails
            let timeout_ms = {
                let now = Instant::now();
                let mut min_timeout: i32 = -1;

                // Error message auto-dismiss deadline
                if let Some(deadline) = self.error_deadline {
                    let t = if deadline > now {
                        deadline.duration_since(now).as_millis() as i32
                    } else {
                        0
                    };
                    min_timeout = t;
                }

                // Toast auto-dismiss deadline
                if let Some(deadline) = self.toast_deadline {
                    let t = if deadline > now {
                        deadline.duration_since(now).as_millis() as i32
                    } else {
                        0
                    };
                    min_timeout = if min_timeout < 0 {
                        t
                    } else {
                        min_timeout.min(t)
                    };
                }

                if self.mode == Mode::Viewer {
                    if let Some(deadline) = self.viewer.next_frame_deadline() {
                        let t = if deadline > now {
                            deadline.duration_since(now).as_millis() as i32
                        } else {
                            0
                        };
                        min_timeout = if min_timeout < 0 {
                            t
                        } else {
                            min_timeout.min(t)
                        };
                    }
                    if let Some(deadline) = self.viewer.pan_deadline() {
                        let t = if deadline > now {
                            deadline.duration_since(now).as_millis() as i32
                        } else {
                            0
                        };
                        min_timeout = if min_timeout < 0 {
                            t
                        } else {
                            min_timeout.min(t)
                        };
                    }
                } else if self.mode == Mode::Gallery && self.gallery.has_pending() {
                    let t = 16; // Poll at ~60fps while thumbnails are being generated
                    min_timeout = if min_timeout < 0 {
                        t
                    } else {
                        min_timeout.min(t)
                    };
                }

                min_timeout
            };

            // Poll the wayland fd
            let mut pollfd = rustix::event::PollFd::new(&wl_fd, rustix::event::PollFlags::IN);
            let _ = rustix::event::poll(std::slice::from_mut(&mut pollfd), timeout_ms);

            // Read and dispatch events
            if let Some(guard) = event_queue.prepare_read() {
                if let Ok(_) = guard.read() {
                    // Events read successfully
                }
            }
            event_queue
                .dispatch_pending(&mut self.state)
                .expect("Dispatch failed");

            // Process all pending wayland events
            let events: Vec<WaylandEvent> = self.state.events.drain(..).collect();
            for event in events {
                match event {
                    WaylandEvent::Configure { width, height } => {
                        self.win_w = width;
                        self.win_h = height;
                        self.state.resize_buffers(width, height, &qh);
                        self.needs_redraw = true;
                    }
                    WaylandEvent::Close => {
                        return;
                    }
                    WaylandEvent::Key(key_event) => {
                        if let Some(action) = crate::input::map_key(&key_event, self.mode) {
                            let should_quit = self.handle_action(action);
                            if should_quit {
                                return;
                            }
                        }
                    }
                    WaylandEvent::FrameCallback => {
                        // Frame was displayed, we can draw again if needed
                        if self.needs_redraw {
                            self.redraw();
                        }
                    }
                    WaylandEvent::WallpaperConfigure { .. } => {
                        // Not in wallpaper mode, ignore
                    }
                }
            }

            // Poll for completed thumbnails from background worker
            if self.mode == Mode::Gallery {
                if self.gallery.poll_thumbnails() {
                    self.needs_redraw = true;
                }
            }

            // Handle GIF animation
            if self.mode == Mode::Viewer {
                if let Some(loaded) = self.image_cache.get(&self.current_index) {
                    if self.viewer.advance_frame(loaded) {
                        self.needs_redraw = true;
                    }
                }
            }

            // Handle pan animation
            if self.mode == Mode::Viewer {
                if self.viewer.update_pan() {
                    self.needs_redraw = true;
                }
            }

            // Handle error message auto-dismiss
            if let Some(deadline) = self.error_deadline {
                if Instant::now() >= deadline {
                    self.error_message = None;
                    self.error_deadline = None;
                    self.needs_redraw = true;
                }
            }

            // Handle toast auto-dismiss
            if let Some(deadline) = self.toast_deadline {
                if Instant::now() >= deadline {
                    self.toast_message = None;
                    self.toast_deadline = None;
                    self.needs_redraw = true;
                }
            }

            // Draw if needed
            if self.needs_redraw && self.win_w > 0 && self.win_h > 0 {
                self.redraw();

                // If animating (GIF or pan), request next frame callback
                if self.mode == Mode::Viewer
                    && (self.viewer.next_frame_deadline().is_some()
                        || self.viewer.is_pan_animating())
                {
                    self.state.request_frame(&qh);
                }
            }
        }
    }

    fn run_wallpaper(&mut self) {
        let mut event_queue = self.conn.new_event_queue();
        let qh = event_queue.handle();

        // Register globals
        let display = self.conn.display();
        display.get_registry(&qh, ());

        // Initial roundtrip to bind globals (compositor, shm, outputs, layer_shell)
        event_queue
            .roundtrip(&mut self.state)
            .expect("Roundtrip failed");

        // Second roundtrip to get output mode events
        event_queue
            .roundtrip(&mut self.state)
            .expect("Roundtrip failed");

        // Verify layer shell is available
        if !self.state.has_layer_shell() {
            eprintln!("Error: compositor does not support wlr-layer-shell protocol");
            eprintln!(
                "Wallpaper mode requires a wlroots-based compositor (sway, dwl, hyprland, etc.)"
            );
            std::process::exit(1);
        }

        // Load the first image
        self.ensure_image_loaded();
        let loaded = match self.image_cache.get(&0) {
            Some(l) => l,
            None => {
                eprintln!("Error: failed to load wallpaper image");
                std::process::exit(1);
            }
        };

        // Get the first frame (static or first frame of animated)
        let frame = match loaded {
            LoadedImage::Static(img) => img.clone(),
            LoadedImage::Animated { frames } => frames[0].0.clone(),
        };

        // Create layer surfaces for all outputs
        self.state.create_wallpaper_surfaces(&qh);

        // Flush to send the surface creation + initial commits
        let _ = self.conn.flush();

        // Main event loop
        let raw_fd = self.conn.backend().poll_fd().as_raw_fd();
        let wl_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };

        while self.state.running {
            let _ = self.conn.flush();

            // Block indefinitely — wallpaper is static
            let mut pollfd = rustix::event::PollFd::new(&wl_fd, rustix::event::PollFlags::IN);
            let _ = rustix::event::poll(std::slice::from_mut(&mut pollfd), -1);

            if let Some(guard) = event_queue.prepare_read() {
                if let Ok(_) = guard.read() {
                    // Events read successfully
                }
            }
            event_queue
                .dispatch_pending(&mut self.state)
                .expect("Dispatch failed");

            let events: Vec<WaylandEvent> = self.state.events.drain(..).collect();
            for event in events {
                match event {
                    WaylandEvent::WallpaperConfigure {
                        output_idx,
                        width,
                        height,
                    } => {
                        // Resize SHM buffers for this output
                        self.state
                            .resize_wallpaper_buffers(output_idx, width, height, &qh);

                        // Render wallpaper: scale-to-fill and convert to XRGB
                        let filled = crate::render::scale_to_fill(&frame, width, height);
                        let pixels = rgba_to_xrgb(&filled);

                        self.state.present_wallpaper(output_idx, &pixels);
                    }
                    WaylandEvent::Close => {
                        return;
                    }
                    _ => {}
                }
            }
        }
    }

    fn ensure_image_loaded(&mut self) {
        // Try loading the current image; if it fails, remove it and advance.
        // Loop in case multiple consecutive images fail.
        while !self.paths.is_empty() {
            let idx = self.current_index;
            if self.image_cache.contains_key(&idx) {
                return;
            }
            match image_loader::load_image(&self.paths[idx]) {
                Ok(loaded) => {
                    self.image_cache.insert(idx, loaded);
                    return;
                }
                Err(e) => {
                    let name = self.paths[idx]
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string();
                    eprintln!(
                        "Warning: failed to load {}: {}",
                        self.paths[idx].display(),
                        e
                    );

                    // Remove the failed path and adjust indices
                    self.paths.remove(idx);
                    // Shift any cached entries above this index down by one
                    let mut new_cache = HashMap::new();
                    for (k, v) in self.image_cache.drain() {
                        if k < idx {
                            new_cache.insert(k, v);
                        } else if k > idx {
                            new_cache.insert(k - 1, v);
                        }
                        // k == idx was the failed one (shouldn't be cached, but skip)
                    }
                    self.image_cache = new_cache;

                    if self.paths.is_empty() {
                        self.error_message = Some("No valid images".to_string());
                        self.error_deadline = Some(Instant::now() + ERROR_DISPLAY_DURATION);
                        return;
                    }
                    // Clamp current_index
                    if self.current_index >= self.paths.len() {
                        self.current_index = 0;
                    }
                    // Set error message
                    self.error_message = Some(format!("Skipped: {}", name));
                    self.error_deadline = Some(Instant::now() + ERROR_DISPLAY_DURATION);
                    // Continue loop to try the next image
                }
            }
        }
    }

    fn navigate_to(&mut self, index: usize) {
        if self.paths.is_empty() {
            return;
        }
        self.current_index = index % self.paths.len();
        self.viewer.reset_view();
        // Clear any transient error when user explicitly navigates
        self.error_message = None;
        self.error_deadline = None;
        self.ensure_image_loaded();

        if let Some(loaded) = self.image_cache.get(&self.current_index) {
            self.viewer.start_animation(loaded);
        }

        self.load_exif_for_current();
        self.update_title();
        self.needs_redraw = true;
    }

    fn load_exif_for_current(&mut self) {
        if let Some(path) = self.paths.get(self.current_index) {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            // Only read EXIF from reasonably-sized files (64 MiB limit for metadata)
            let too_large = std::fs::metadata(path)
                .map(|m| m.len() > 64 * 1024 * 1024)
                .unwrap_or(true);
            if !too_large {
                if let Ok(data) = std::fs::read(path) {
                    let tags = match ext.as_str() {
                        "jpg" | "jpeg" => image_loader::read_exif_tags(&data),
                        "tiff" | "tif" => image_loader::read_exif_tags_tiff(&data),
                        "webp" => image_loader::read_exif_tags_webp(&data),
                        "png" => image_loader::read_exif_tags_png(&data),
                        _ => Vec::new(),
                    };
                    self.viewer.set_exif_data(tags);
                    return;
                }
            }
            self.viewer.set_exif_data(Vec::new());
        }
    }

    fn update_title(&self) {
        if let Some(path) = self.paths.get(self.current_index) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("rimg");
            self.state.set_title(&format!("rimg - {}", name));
        }
    }

    fn redraw(&mut self) {
        if self.win_w == 0 || self.win_h == 0 {
            return;
        }

        let pixels = match self.mode {
            Mode::Viewer => {
                if self.paths.is_empty() {
                    // No valid images remain — show background with error message
                    let mut buf = vec![crate::render::BG_COLOR; (self.win_w * self.win_h) as usize];
                    if let Some(ref msg) = self.error_message {
                        crate::status::draw_status_bar(&mut buf, self.win_w, self.win_h, msg);
                    }
                    buf
                } else if let Some(loaded) = self.image_cache.get(&self.current_index) {
                    self.viewer.render(
                        loaded,
                        self.win_w,
                        self.win_h,
                        &self.paths[self.current_index],
                        self.current_index,
                        self.paths.len(),
                        self.error_message.as_deref(),
                        self.toast_message.as_deref(),
                    )
                } else {
                    vec![crate::render::BG_COLOR; (self.win_w * self.win_h) as usize]
                }
            }
            Mode::Gallery => {
                let mut buf = self.gallery.render(&self.paths, self.win_w, self.win_h);
                if let Some(ref msg) = self.toast_message {
                    crate::viewer::Viewer::draw_toast(&mut buf, self.win_w, self.win_h, msg);
                }
                buf
            }
        };

        if pixels.is_empty() {
            return;
        }

        self.state.present(&pixels);
        self.needs_redraw = false;
    }

    /// Rotate the current image in the cache (clockwise if `cw`, counterclockwise otherwise).
    fn rotate_current_image(&mut self, cw: bool) {
        if let Some(loaded) = self.image_cache.remove(&self.current_index) {
            let rotate_fn = if cw {
                image_loader::rotate_90
            } else {
                image_loader::rotate_270
            };
            let rotated = match loaded {
                LoadedImage::Static(img) => LoadedImage::Static(rotate_fn(img)),
                LoadedImage::Animated { frames } => LoadedImage::Animated {
                    frames: frames
                        .into_iter()
                        .map(|(img, dur)| (rotate_fn(img), dur))
                        .collect(),
                },
            };
            self.image_cache.insert(self.current_index, rotated);
            self.viewer.zoom_reset();
            self.needs_redraw = true;
        }
    }

    /// Handle an action. Returns true if the app should quit.
    fn handle_action(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => {
                return true;
            }
            Action::EscapeOrQuit => {
                if self.mode == Mode::Gallery {
                    self.mode = Mode::Viewer;
                    self.current_index = self.gallery.selected;
                    self.viewer.reset_view();
                    self.ensure_image_loaded();
                    if let Some(loaded) = self.image_cache.get(&self.current_index) {
                        self.viewer.start_animation(loaded);
                    }
                    self.load_exif_for_current();
                    self.update_title();
                    self.needs_redraw = true;
                } else if self.viewer.is_exif_visible() {
                    self.viewer.hide_exif();
                    self.needs_redraw = true;
                } else if self.viewer.is_zoomed() {
                    self.viewer.zoom_reset();
                    self.needs_redraw = true;
                } else {
                    return true;
                }
            }
            Action::ToggleMode => match self.mode {
                Mode::Viewer => {
                    self.mode = Mode::Gallery;
                    self.gallery.set_selected(self.current_index);
                    self.viewer.next_frame_time = None;
                    self.needs_redraw = true;
                }
                Mode::Gallery => {
                    self.mode = Mode::Viewer;
                    self.navigate_to(self.gallery.selected);
                }
            },
            Action::NextImage => {
                let next = if self.current_index + 1 >= self.paths.len() {
                    0
                } else {
                    self.current_index + 1
                };
                self.navigate_to(next);
            }
            Action::PrevImage => {
                let prev = if self.current_index == 0 {
                    self.paths.len().saturating_sub(1)
                } else {
                    self.current_index - 1
                };
                self.navigate_to(prev);
            }
            Action::FirstImage => {
                self.navigate_to(0);
            }
            Action::LastImage => {
                if !self.paths.is_empty() {
                    self.navigate_to(self.paths.len() - 1);
                }
            }
            Action::ZoomIn => {
                self.viewer.zoom_in();
                self.needs_redraw = true;
            }
            Action::ZoomOut => {
                self.viewer.zoom_out();
                self.needs_redraw = true;
            }
            Action::ZoomReset => {
                self.viewer.zoom_reset();
                self.needs_redraw = true;
            }
            Action::FitToWindow => {
                self.viewer.toggle_fit_to_window();
                self.needs_redraw = true;
            }
            Action::ActualSize => {
                self.viewer.zoom_actual_size();
                self.needs_redraw = true;
            }
            Action::PanStart(dir) => {
                if self.viewer.is_zoomed() {
                    self.viewer.pan_start(dir);
                    // No needs_redraw here — update_pan() in the event loop handles it
                } else {
                    // When not zoomed, h/l/Left/Right navigate between images
                    match dir {
                        PanDirection::Left => {
                            let prev = if self.current_index == 0 {
                                self.paths.len().saturating_sub(1)
                            } else {
                                self.current_index - 1
                            };
                            self.navigate_to(prev);
                        }
                        PanDirection::Right => {
                            let next = if self.current_index + 1 >= self.paths.len() {
                                0
                            } else {
                                self.current_index + 1
                            };
                            self.navigate_to(next);
                        }
                        _ => {} // Up/Down ignored when not zoomed
                    }
                }
            }
            Action::PanStop(dir) => {
                self.viewer.pan_stop(dir);
            }
            Action::Fullscreen => {
                self.state.toggle_fullscreen();
            }
            Action::RotateCW => {
                self.rotate_current_image(true);
            }
            Action::RotateCCW => {
                self.rotate_current_image(false);
            }
            Action::ToggleExif => {
                self.viewer.toggle_exif();
                self.needs_redraw = true;
            }
            Action::MoveLeft => {
                self.gallery.move_left(self.paths.len());
                self.needs_redraw = true;
            }
            Action::MoveRight => {
                self.gallery.move_right(self.paths.len());
                self.needs_redraw = true;
            }
            Action::MoveUp => {
                self.gallery.move_up(self.paths.len());
                self.needs_redraw = true;
            }
            Action::MoveDown => {
                self.gallery.move_down(self.paths.len());
                self.needs_redraw = true;
            }
            Action::GalleryFirst => {
                self.gallery.go_first();
                self.needs_redraw = true;
            }
            Action::GalleryLast => {
                self.gallery.go_last(self.paths.len());
                self.needs_redraw = true;
            }
            Action::CycleSort => {
                self.cycle_sort();
                self.ensure_image_loaded();
                self.needs_redraw = true;
            }
        }
        false
    }

    /// Cycle to the next sort mode, re-sort paths, and show a toast.
    fn cycle_sort(&mut self) {
        if self.paths.is_empty() {
            return;
        }

        // Remember current image path and old index to re-find it after sort
        let current_path = self.paths.get(self.current_index).cloned();
        let old_index = self.current_index;

        self.sort_mode = self.sort_mode.next();

        // Sort paths according to the new mode
        // We pre-populate caches then sort using them to avoid borrow conflicts.
        match self.sort_mode {
            SortMode::Name => {
                self.paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
            }
            SortMode::Size => {
                // Ensure all metadata is cached first
                for p in &self.paths {
                    if !self.meta_cache.contains_key(p) {
                        let meta = read_file_meta(p);
                        self.meta_cache.insert(p.clone(), meta);
                    }
                }
                let cache = &self.meta_cache;
                self.paths
                    .sort_by_cached_key(|p| cache.get(p).map(|m| m.0).unwrap_or(0));
            }
            SortMode::ModTime => {
                for p in &self.paths {
                    if !self.meta_cache.contains_key(p) {
                        let meta = read_file_meta(p);
                        self.meta_cache.insert(p.clone(), meta);
                    }
                }
                let cache = &self.meta_cache;
                self.paths
                    .sort_by_cached_key(|p| cache.get(p).map(|m| m.1).unwrap_or(0));
            }
            SortMode::ExifDate => {
                // Pre-populate both metadata and EXIF date caches
                for p in &self.paths {
                    if !self.meta_cache.contains_key(p) {
                        let meta = read_file_meta(p);
                        self.meta_cache.insert(p.clone(), meta);
                    }
                    if !self.exif_date_cache.contains_key(p) {
                        let ext = p
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        let exif_ts = if ext == "jpg" || ext == "jpeg" {
                            parse_exif_date_original(p)
                        } else {
                            None
                        };
                        self.exif_date_cache.insert(p.clone(), exif_ts);
                    }
                }
                let meta_cache = &self.meta_cache;
                let exif_cache = &self.exif_date_cache;
                self.paths.sort_by_cached_key(|p| {
                    exif_cache
                        .get(p)
                        .and_then(|v| *v)
                        .unwrap_or_else(|| meta_cache.get(p).map(|m| m.1).unwrap_or(0))
                });
            }
        }

        // Re-find the current image in the sorted list
        if let Some(ref path) = current_path {
            if let Some(pos) = self.paths.iter().position(|p| p == path) {
                self.current_index = pos;
            }
        }

        // Remap the cached image from old index to new index
        if old_index != self.current_index {
            if let Some(loaded) = self.image_cache.remove(&old_index) {
                self.image_cache.clear();
                self.image_cache.insert(self.current_index, loaded);
            }
        }

        // Update gallery: reset selection and invalidate stale thumbnail cache
        self.gallery.set_selected(self.current_index);
        self.gallery.invalidate_thumbnails();

        // Show toast
        self.toast_message = Some(format!("Sort: {}", self.sort_mode.label()));
        self.toast_deadline = Some(Instant::now() + TOAST_DISPLAY_DURATION);
    }
}

/// Read file size and modification time. Returns (size_bytes, mtime_secs).
fn read_file_meta(path: &PathBuf) -> (u64, u64) {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let size = meta.len();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (size, mtime)
        }
        Err(_) => (0, 0),
    }
}

/// Parse EXIF DateTimeOriginal from a JPEG file, returning a Unix timestamp.
fn parse_exif_date_original(path: &PathBuf) -> Option<u64> {
    let data = std::fs::read(path).ok()?;
    let tags = image_loader::read_exif_tags(&data);
    for (label, value) in &tags {
        if label == "Date Original" || label == "Date/Time" {
            // EXIF date format: "YYYY:MM:DD HH:MM:SS"
            return parse_exif_datetime(value);
        }
    }
    None
}

/// Parse "YYYY:MM:DD HH:MM:SS" into a rough Unix timestamp (seconds since epoch).
fn parse_exif_datetime(s: &str) -> Option<u64> {
    // "2024:01:15 14:30:00" -> parse as approximate seconds
    let parts: Vec<&str> = s.split(|c: char| c == ':' || c == ' ').collect();
    if parts.len() < 6 {
        return None;
    }
    let year: u64 = parts[0].parse().ok()?;
    let month: u64 = parts[1].parse().ok()?;
    let day: u64 = parts[2].parse().ok()?;
    let hour: u64 = parts[3].parse().ok()?;
    let min: u64 = parts[4].parse().ok()?;
    let sec: u64 = parts[5].parse().ok()?;
    if year < 1970 || month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    // Approximate days since epoch (good enough for sorting)
    let days_approx = (year - 1970) * 365 + (year - 1969) / 4 + (month - 1) * 30 + day;
    Some(days_approx * 86400 + hour * 3600 + min * 60 + sec)
}

/// Convert an RgbaImage to a Vec<u32> XRGB8888 pixel buffer.
fn rgba_to_xrgb(img: &crate::image_loader::RgbaImage) -> Vec<u32> {
    let raw = img.as_raw();
    let (w, h) = img.dimensions();
    let mut buf = Vec::with_capacity((w * h) as usize);
    for i in 0..(w * h) as usize {
        let idx = i * 4;
        let r = raw[idx] as u32;
        let g = raw[idx + 1] as u32;
        let b = raw[idx + 2] as u32;
        buf.push((r << 16) | (g << 8) | b);
    }
    buf
}
