use crate::gallery::Gallery;
use crate::image_loader::{self, LoadedImage};
use crate::input::{Action, Mode};
use crate::viewer::Viewer;
use crate::wayland::{WaylandEvent, WaylandState};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::PathBuf;
use std::time::Instant;
use wayland_client::Connection;

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

            // Calculate poll timeout based on GIF animation, pan animation, or pending thumbnails
            let timeout_ms = if self.mode == Mode::Viewer {
                let now = Instant::now();
                let gif_timeout = if let Some(deadline) = self.viewer.next_frame_deadline() {
                    if deadline > now {
                        deadline.duration_since(now).as_millis() as i32
                    } else {
                        0
                    }
                } else {
                    -1
                };
                let pan_timeout = if let Some(deadline) = self.viewer.pan_deadline() {
                    if deadline > now {
                        deadline.duration_since(now).as_millis() as i32
                    } else {
                        0
                    }
                } else {
                    -1
                };
                // Use the smallest non-negative timeout, or -1 if both are indefinite
                match (gif_timeout, pan_timeout) {
                    (-1, -1) => -1,
                    (-1, t) | (t, -1) => t,
                    (a, b) => a.min(b),
                }
            } else if self.mode == Mode::Gallery && self.gallery.has_pending() {
                16 // Poll at ~60fps while thumbnails are being generated
            } else {
                -1
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
        if self.paths.is_empty() {
            return;
        }
        let idx = self.current_index;
        if self.image_cache.contains_key(&idx) {
            return;
        }
        match image_loader::load_image(&self.paths[idx]) {
            Ok(loaded) => {
                self.image_cache.insert(idx, loaded);
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to load {}: {}",
                    self.paths[idx].display(),
                    e
                );
            }
        }
    }

    fn navigate_to(&mut self, index: usize) {
        if self.paths.is_empty() {
            return;
        }
        self.current_index = index % self.paths.len();
        self.viewer.reset_view();
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
            if ext == "jpg" || ext == "jpeg" {
                if let Ok(data) = std::fs::read(path) {
                    let tags = image_loader::read_exif_tags(&data);
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
                if let Some(loaded) = self.image_cache.get(&self.current_index) {
                    self.viewer.render(
                        loaded,
                        self.win_w,
                        self.win_h,
                        &self.paths[self.current_index],
                        self.current_index,
                        self.paths.len(),
                    )
                } else {
                    vec![crate::render::BG_COLOR; (self.win_w * self.win_h) as usize]
                }
            }
            Mode::Gallery => self.gallery.render(&self.paths, self.win_w, self.win_h),
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
            Action::PanStart(dir) => {
                self.viewer.pan_start(dir);
                // No needs_redraw here — update_pan() in the event loop handles it
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
                if self.mode == Mode::Viewer {
                    let prev = if self.current_index == 0 {
                        self.paths.len().saturating_sub(1)
                    } else {
                        self.current_index - 1
                    };
                    self.navigate_to(prev);
                } else {
                    self.gallery.move_left(self.paths.len());
                    self.needs_redraw = true;
                }
            }
            Action::MoveRight => {
                if self.mode == Mode::Viewer {
                    let next = if self.current_index + 1 >= self.paths.len() {
                        0
                    } else {
                        self.current_index + 1
                    };
                    self.navigate_to(next);
                } else {
                    self.gallery.move_right(self.paths.len());
                    self.needs_redraw = true;
                }
            }
            Action::MoveUp => {
                if self.mode == Mode::Viewer {
                    let prev = if self.current_index == 0 {
                        self.paths.len().saturating_sub(1)
                    } else {
                        self.current_index - 1
                    };
                    self.navigate_to(prev);
                } else {
                    self.gallery.move_up(self.paths.len());
                    self.needs_redraw = true;
                }
            }
            Action::MoveDown => {
                if self.mode == Mode::Viewer {
                    let next = if self.current_index + 1 >= self.paths.len() {
                        0
                    } else {
                        self.current_index + 1
                    };
                    self.navigate_to(next);
                } else {
                    self.gallery.move_down(self.paths.len());
                    self.needs_redraw = true;
                }
            }
            Action::GalleryFirst => {
                self.gallery.go_first();
                self.needs_redraw = true;
            }
            Action::GalleryLast => {
                self.gallery.go_last(self.paths.len());
                self.needs_redraw = true;
            }
        }
        false
    }
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
