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
}

impl App {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
        let state = WaylandState::new();

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
        }
    }

    pub fn run(&mut self) {
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
        self.update_title();

        // Main event loop using poll
        // SAFETY: The connection fd is valid for the lifetime of self.conn
        let raw_fd = self.conn.backend().poll_fd().as_raw_fd();
        let wl_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };

        while self.state.running {
            // Flush outgoing messages
            let _ = self.conn.flush();

            // Calculate poll timeout based on GIF animation or pending thumbnails
            let timeout_ms = if self.mode == Mode::Viewer {
                if let Some(deadline) = self.viewer.next_frame_deadline() {
                    let now = Instant::now();
                    if deadline > now {
                        deadline.duration_since(now).as_millis() as i32
                    } else {
                        0 // Frame is due now
                    }
                } else {
                    -1 // Block indefinitely
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

            // Draw if needed
            if self.needs_redraw && self.win_w > 0 && self.win_h > 0 {
                self.redraw();

                // If animating, request next frame callback
                if self.mode == Mode::Viewer && self.viewer.next_frame_deadline().is_some() {
                    self.state.request_frame(&qh);
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

        self.update_title();
        self.needs_redraw = true;
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
                    self.update_title();
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
            Action::PanLeft => {
                self.viewer.pan_left();
                self.needs_redraw = true;
            }
            Action::PanRight => {
                self.viewer.pan_right();
                self.needs_redraw = true;
            }
            Action::PanUp => {
                self.viewer.pan_up();
                self.needs_redraw = true;
            }
            Action::PanDown => {
                self.viewer.pan_down();
                self.needs_redraw = true;
            }
            Action::Fullscreen => {
                self.state.toggle_fullscreen();
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
