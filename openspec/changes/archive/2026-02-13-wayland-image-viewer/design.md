## Context

rimg is a greenfield Rust project — a minimal Wayland image viewer inspired by sxiv. The target is Linux systems running a Wayland compositor. There is no existing codebase. The viewer renders images in its own window (not inside a terminal), uses CPU-only rendering via shared memory buffers, and has zero runtime dependencies beyond the Wayland compositor itself.

## Goals / Non-Goals

**Goals:**
- Minimal, fast image viewer that opens in under 100ms for a single image
- Native Wayland window with direct pixel rendering (no GPU, no toolkit)
- Vim-style keyboard-only navigation
- Animated GIF playback at correct frame timing
- sxiv-like thumbnail gallery with grid layout
- Zero-config: no config files, sensible defaults only
- Embedded bitmap font for status bar (no system font dependency)

**Non-Goals:**
- Mouse interaction (explicitly excluded)
- X11 support (Wayland only)
- Image editing or manipulation
- Configuration files or theming
- Slideshow mode
- Printing or file management operations
- GPU-accelerated rendering (wgpu, OpenGL)
- Network image loading (URLs)

## Decisions

### 1. Window creation: winit + softbuffer

Use `winit` for window creation and event loop, paired with `softbuffer` for pixel buffer presentation. This gives us a native Wayland window with minimal abstraction.

**Alternatives considered:**
- smithay-client-toolkit (SCTK): More Wayland-native control but significantly more boilerplate for window management. Overkill for a simple viewer.
- raw wayland-client: Maximum control but enormous implementation effort for basic window management.
- GTK4/iced: Full GUI toolkits. Violates the minimalism goal and add massive dependency trees.

### 2. Image decoding: `image` crate (unified)

Use the `image` crate with selective feature flags for all four formats (PNG, JPEG, GIF, WebP). One dependency covers all decoding needs.

**Alternatives considered:**
- Individual codec crates (png, jpeg-decoder, gif, image-webp): More control but adds integration complexity. The `image` crate wraps these anyway.

### 3. Image scaling: `fast_image_resize`

Use `fast_image_resize` for SIMD-accelerated Lanczos3 resizing. Critical for responsive zoom and gallery thumbnail generation.

**Alternatives considered:**
- `image::imageops::resize`: Much slower (no SIMD), noticeable lag on large images or many thumbnails.

### 4. EXIF handling: `kamadak-exif`

Use `kamadak-exif` for reading EXIF orientation tags. Pure Rust, well-established, focused on the exact use case.

**Alternatives considered:**
- `nom-exif`: Newer, supports more formats (HEIC), but we don't need those formats and kamadak-exif is more battle-tested.

### 5. Text rendering: Embedded bitmap font

Embed a minimal 8x16 monospace bitmap font as a compile-time constant. Render glyphs by direct pixel copy — no font parsing, no rasterization, no system font discovery.

**Alternatives considered:**
- `fontdue` + system font: Better looking text but adds font discovery complexity and a runtime dependency on system fonts.
- `ab_glyph`: Same trade-off. Unnecessary for a single-line status bar.

### 6. Application architecture: Single-threaded event loop

Run everything on winit's main event loop thread. Image decoding is fast enough for single images on modern hardware. Gallery thumbnails are generated lazily as they become visible.

**Alternatives considered:**
- Background thread pool for image loading: Adds complexity (channels, synchronization). Only justified if we had hundreds of large images. Can be added later if needed.

### 7. GIF animation: winit timer-based frame advancement

Use `ControlFlow::WaitUntil` with the next frame's deadline to drive GIF animation. Each frame is pre-decoded and stored in memory.

**Alternatives considered:**
- Separate animation thread: Adds synchronization complexity for no benefit since we need to redraw on the main thread anyway.

### 8. Gallery thumbnail caching: In-memory HashMap

Cache resized thumbnails in a `HashMap<PathBuf, RgbaImage>`. Generate on first view, keep in memory. No disk cache.

**Alternatives considered:**
- Disk-based thumbnail cache (XDG thumbnails spec): More complex, benefits only repeat usage. Can be added in a future version.

## Risks / Trade-offs

- [Memory usage with many GIF frames] → Limit max decoded frames or decode on-demand for very large GIFs. Most GIFs are under 100 frames.
- [Gallery with thousands of images] → Lazy thumbnail generation mitigates this. Only visible thumbnails are decoded/resized.
- [No GPU acceleration] → CPU rendering via softbuffer is sufficient for image viewing. Resizing is the bottleneck, mitigated by fast_image_resize SIMD.
- [winit Wayland maturity] → winit has solid Wayland support as of 0.31. Known edge cases with some compositors but covers the mainstream (sway, GNOME, KDE).
- [Embedded font looks basic] → Acceptable for a minimal status bar. The focus is the image, not the UI chrome.

## Module Structure

```
src/
  main.rs          — CLI arg parsing, image path collection, app bootstrap
  app.rs           — Application state, mode enum (Viewer/Gallery), event dispatch
  viewer.rs        — Single-image rendering, zoom/pan state, GIF frame cycling
  gallery.rs       — Thumbnail grid layout, selection state, scroll offset
  image_loader.rs  — Decode images, apply EXIF orientation, extract GIF frames
  render.rs        — Pixel buffer ops: RGBA→XRGB, scaling, compositing, letterbox
  font.rs          — Embedded bitmap font data, glyph rendering to pixel buffer
  input.rs         — Key event → action mapping, mode-aware dispatch
  status.rs        — Format and render status bar text (name, size, mtime)
```
