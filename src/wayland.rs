use std::os::fd::{AsFd, OwnedFd};

use rustix::fs::{memfd_create, MemfdFlags};
use rustix::mm::{mmap, munmap, MapFlags, ProtFlags};

use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    wl_callback,
};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, WEnum};

use crate::protocols::xdg_shell::{xdg_surface, xdg_toplevel, xdg_wm_base};

/// Keyboard event data passed to the application.
pub struct KeyEvent {
    #[allow(dead_code)]
    pub keycode: u32,
    pub keysym: u32,
    pub pressed: bool,
    pub ctrl: bool,
}

/// Events produced by the Wayland state for the application to handle.
pub enum WaylandEvent {
    Configure { width: u32, height: u32 },
    Close,
    Key(KeyEvent),
    FrameCallback,
}

/// SHM double-buffer management.
struct ShmBuffer {
    fd: OwnedFd,
    pool: Option<wl_shm_pool::WlShmPool>,
    buffers: [Option<wl_buffer::WlBuffer>; 2],
    mmap_ptr: *mut u8,
    mmap_len: usize,
    width: u32,
    height: u32,
    current: usize, // which buffer index to draw into
}

impl ShmBuffer {
    fn new() -> Self {
        let fd = memfd_create(c"rimg-shm", MemfdFlags::CLOEXEC).expect("memfd_create failed");
        Self {
            fd,
            pool: None,
            buffers: [None, None],
            mmap_ptr: std::ptr::null_mut(),
            mmap_len: 0,
            width: 0,
            height: 0,
            current: 0,
        }
    }

    fn resize(
        &mut self,
        width: u32,
        height: u32,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandState>,
    ) {
        if width == 0 || height == 0 {
            return;
        }

        // Destroy old buffers
        for buf in &mut self.buffers {
            if let Some(b) = buf.take() {
                b.destroy();
            }
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }

        // Unmap old memory
        if !self.mmap_ptr.is_null() && self.mmap_len > 0 {
            unsafe {
                let _ = munmap(self.mmap_ptr as *mut std::ffi::c_void, self.mmap_len);
            }
            self.mmap_ptr = std::ptr::null_mut();
            self.mmap_len = 0;
        }

        let stride = width * 4;
        let buf_size = (stride * height) as usize;
        let pool_size = buf_size * 2; // double buffer

        // Resize the memfd
        rustix::fs::ftruncate(&self.fd, pool_size as u64).expect("ftruncate failed");

        // Mmap it
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                pool_size,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                self.fd.as_fd(),
                0,
            )
            .expect("mmap failed")
        };

        self.mmap_ptr = ptr as *mut u8;
        self.mmap_len = pool_size;
        self.width = width;
        self.height = height;

        // Create new pool
        let pool = shm.create_pool(self.fd.as_fd(), pool_size as i32, qh, ());

        // Create two buffers
        let b0 = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Xrgb8888,
            qh,
            (),
        );
        let b1 = pool.create_buffer(
            buf_size as i32,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Xrgb8888,
            qh,
            (),
        );

        self.pool = Some(pool);
        self.buffers = [Some(b0), Some(b1)];
        self.current = 0;
    }

    /// Get a mutable slice to the current back buffer pixel data.
    fn back_buffer_mut(&mut self) -> &mut [u32] {
        let stride = self.width * 4;
        let buf_size = (stride * self.height) as usize;
        let offset = self.current * buf_size;
        let ptr = unsafe { self.mmap_ptr.add(offset) as *mut u32 };
        let len = (self.width * self.height) as usize;
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }

    /// Get the current back buffer wl_buffer and swap.
    fn swap(&mut self) -> Option<&wl_buffer::WlBuffer> {
        let buf = self.buffers[self.current].as_ref();
        self.current = 1 - self.current;
        buf
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        for buf in &mut self.buffers {
            if let Some(b) = buf.take() {
                b.destroy();
            }
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        if !self.mmap_ptr.is_null() && self.mmap_len > 0 {
            unsafe {
                let _ = munmap(self.mmap_ptr as *mut std::ffi::c_void, self.mmap_len);
            }
        }
    }
}

/// The core Wayland state — handles all protocol dispatch.
pub struct WaylandState {
    pub running: bool,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    #[allow(dead_code)]
    seat: Option<wl_seat::WlSeat>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    toplevel: Option<xdg_toplevel::XdgToplevel>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    shm_buf: ShmBuffer,
    configured: bool,
    pending_configure_size: Option<(u32, u32)>,
    pub events: Vec<WaylandEvent>,
    fullscreen: bool,
    frame_pending: bool,

    // xkbcommon state
    xkb_context: *mut xkbcommon_dl::xkb_context,
    xkb_keymap: *mut xkbcommon_dl::xkb_keymap,
    xkb_state: *mut xkbcommon_dl::xkb_state,
    ctrl_pressed: bool,
}

// Safety: WaylandState is only used from the main thread.
unsafe impl Send for WaylandState {}

impl WaylandState {
    pub fn new() -> Self {
        let xkb = xkbcommon_dl::xkbcommon_handle();
        let xkb_context = unsafe {
            (xkb.xkb_context_new)(xkbcommon_dl::xkb_context_flags::XKB_CONTEXT_NO_FLAGS)
        };

        Self {
            running: true,
            compositor: None,
            shm: None,
            seat: None,
            wm_base: None,
            surface: None,
            xdg_surface: None,
            toplevel: None,
            keyboard: None,
            shm_buf: ShmBuffer::new(),
            configured: false,
            pending_configure_size: None,
            events: Vec::new(),
            fullscreen: false,
            frame_pending: false,
            xkb_context,
            xkb_keymap: std::ptr::null_mut(),
            xkb_state: std::ptr::null_mut(),
            ctrl_pressed: false,
        }
    }

    fn init_xdg_surface(&mut self, qh: &QueueHandle<WaylandState>) {
        let wm_base = self.wm_base.as_ref().unwrap();
        let surface = self.surface.as_ref().unwrap();

        let xdg_surface = wm_base.get_xdg_surface(surface, qh, ());
        let toplevel = xdg_surface.get_toplevel(qh, ());
        toplevel.set_title("rimg".into());

        surface.commit();

        self.xdg_surface = Some(xdg_surface);
        self.toplevel = Some(toplevel);
    }

    /// Set the window title.
    pub fn set_title(&self, title: &str) {
        if let Some(toplevel) = &self.toplevel {
            toplevel.set_title(title.into());
        }
    }

    /// Toggle fullscreen state.
    pub fn toggle_fullscreen(&self) {
        if let Some(toplevel) = &self.toplevel {
            if self.fullscreen {
                toplevel.unset_fullscreen();
            } else {
                toplevel.set_fullscreen(None);
            }
        }
    }

    /// Write pixel data to the back buffer and present.
    pub fn present(&mut self, pixels: &[u32]) {
        if self.shm_buf.width == 0 || self.shm_buf.height == 0 {
            return;
        }

        let back = self.shm_buf.back_buffer_mut();
        let len = back.len().min(pixels.len());
        back[..len].copy_from_slice(&pixels[..len]);

        let surface = self.surface.as_ref().unwrap();
        if let Some(buffer) = self.shm_buf.swap() {
            surface.attach(Some(buffer), 0, 0);
            surface.damage_buffer(0, 0, self.shm_buf.width as i32, self.shm_buf.height as i32);
            surface.commit();
        }
    }

    /// Request a frame callback for animation.
    pub fn request_frame(&mut self, qh: &QueueHandle<WaylandState>) {
        if !self.frame_pending {
            if let Some(surface) = &self.surface {
                surface.frame(qh, ());
                self.frame_pending = true;
            }
        }
    }

    /// Resize SHM buffers (called after configure).
    pub fn resize_buffers(&mut self, width: u32, height: u32, qh: &QueueHandle<WaylandState>) {
        if let Some(shm) = &self.shm.clone() {
            self.shm_buf.resize(width, height, shm, qh);
        }
    }

    #[allow(dead_code)]
    pub fn width(&self) -> u32 {
        self.shm_buf.width
    }

    #[allow(dead_code)]
    pub fn height(&self) -> u32 {
        self.shm_buf.height
    }
}

impl Drop for WaylandState {
    fn drop(&mut self) {
        let xkb = xkbcommon_dl::xkbcommon_handle();
        if !self.xkb_state.is_null() {
            unsafe { (xkb.xkb_state_unref)(self.xkb_state) };
        }
        if !self.xkb_keymap.is_null() {
            unsafe { (xkb.xkb_keymap_unref)(self.xkb_keymap) };
        }
        if !self.xkb_context.is_null() {
            unsafe { (xkb.xkb_context_unref)(self.xkb_context) };
        }
    }
}

// --- Dispatch implementations ---

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match &interface[..] {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<wl_compositor::WlCompositor, _, _>(name, 4.min(version), qh, ());
                    let surface = compositor.create_surface(qh, ());
                    state.compositor = Some(compositor);
                    state.surface = Some(surface);

                    if state.wm_base.is_some() && state.xdg_surface.is_none() {
                        state.init_xdg_surface(qh);
                    }
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ());
                    state.shm = Some(shm);
                }
                "wl_seat" => {
                    registry.bind::<wl_seat::WlSeat, _, _>(name, 4.min(version), qh, ());
                }
                "xdg_wm_base" => {
                    let wm_base =
                        registry.bind::<xdg_wm_base::XdgWmBase, _, _>(name, 1, qh, ());
                    state.wm_base = Some(wm_base);

                    if state.surface.is_some() && state.xdg_surface.is_none() {
                        state.init_xdg_surface(qh);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let xdg_wm_base::Event::Ping { serial } = event;
        wm_base.pong(serial);
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for WaylandState {
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let xdg_surface::Event::Configure { serial } = event;
        xdg_surface.ack_configure(serial);
        state.configured = true;

        // If we got a pending size from the toplevel configure, emit it now
        if let Some((w, h)) = state.pending_configure_size.take() {
            let width = if w == 0 { 800 } else { w };
            let height = if h == 0 { 600 } else { h };
            state.events.push(WaylandEvent::Configure { width, height });
        } else if state.shm_buf.width == 0 {
            // First configure with no size hint — use default
            state
                .events
                .push(WaylandEvent::Configure { width: 800, height: 600 });
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Close => {
                state.running = false;
                state.events.push(WaylandEvent::Close);
            }
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                // Check if fullscreen state (value=2) is in the states array
                // States are u32 values encoded as native-endian in the byte array
                state.fullscreen = states
                    .chunks_exact(4)
                    .any(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) == 2);
                // Store pending size — will be used when xdg_surface::Configure arrives
                state.pending_configure_size = Some((width as u32, height as u32));
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                let kb = seat.get_keyboard(qh, ());
                state.keyboard = Some(kb);
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let xkb = xkbcommon_dl::xkbcommon_handle();

        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if let WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) = format {
                    // Map the keymap fd
                    let map = unsafe {
                        mmap(
                            std::ptr::null_mut(),
                            size as usize,
                            ProtFlags::READ,
                            MapFlags::PRIVATE,
                            fd.as_fd(),
                            0,
                        )
                    };

                    if let Ok(ptr) = map {
                        let keymap = unsafe {
                            (xkb.xkb_keymap_new_from_string)(
                                state.xkb_context,
                                ptr as *const std::os::raw::c_char,
                                xkbcommon_dl::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                                xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
                            )
                        };

                        unsafe {
                            let _ = munmap(ptr, size as usize);
                        }

                        if !keymap.is_null() {
                            // Clean up old state/keymap
                            if !state.xkb_state.is_null() {
                                unsafe { (xkb.xkb_state_unref)(state.xkb_state) };
                            }
                            if !state.xkb_keymap.is_null() {
                                unsafe { (xkb.xkb_keymap_unref)(state.xkb_keymap) };
                            }

                            state.xkb_keymap = keymap;
                            state.xkb_state = unsafe { (xkb.xkb_state_new)(keymap) };
                        }
                    }
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                if state.xkb_state.is_null() {
                    return;
                }

                let pressed = matches!(key_state, WEnum::Value(wl_keyboard::KeyState::Pressed));
                // Wayland keycodes are evdev keycodes; xkb expects evdev + 8
                let keycode = key + 8;
                let keysym = unsafe { (xkb.xkb_state_key_get_one_sym)(state.xkb_state, keycode) };

                state.events.push(WaylandEvent::Key(KeyEvent {
                    keycode: key,
                    keysym,
                    pressed,
                    ctrl: state.ctrl_pressed,
                }));
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if !state.xkb_state.is_null() {
                    unsafe {
                        (xkb.xkb_state_update_mask)(
                            state.xkb_state,
                            mods_depressed,
                            mods_latched,
                            mods_locked,
                            0,
                            0,
                            group,
                        );
                    }
                    state.ctrl_pressed = unsafe {
                        (xkb.xkb_state_mod_name_is_active)(
                            state.xkb_state,
                            xkbcommon_dl::XKB_MOD_NAME_CTRL.as_ptr().cast(),
                            xkbcommon_dl::xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
                        )
                    } == 1;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.frame_pending = false;
            state.events.push(WaylandEvent::FrameCallback);
        }
    }
}

// Ignore events from these types
delegate_noop!(WaylandState: ignore wl_compositor::WlCompositor);
delegate_noop!(WaylandState: ignore wl_surface::WlSurface);
delegate_noop!(WaylandState: ignore wl_shm::WlShm);
delegate_noop!(WaylandState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(WaylandState: ignore wl_buffer::WlBuffer);
