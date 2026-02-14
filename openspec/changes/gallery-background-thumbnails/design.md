## Context

rimg is a minimal Wayland image viewer written in Rust. It is entirely single-threaded. The gallery mode renders a grid of thumbnails by synchronously loading each visible image at full resolution, resizing to 200x200, and caching the result. This happens inside `Gallery::render()`, blocking the UI until all visible thumbnails are generated. For directories with high-resolution JPEGs, the gallery takes several seconds to appear.

The `turbojpeg` crate (already a dependency) supports DCT scaling via `Decompressor::set_scaling_factor()`, which can decode JPEGs at 1/2, 1/4, or 1/8 resolution directly in the IDCT stage. This is unused today.

## Goals / Non-Goals

**Goals:**
- Gallery opens in under one frame (~16ms) regardless of image count or resolution
- Thumbnails appear progressively as they are generated in the background
- JPEG thumbnail generation is significantly faster via DCT-scaled decoding
- No new crate dependencies (use only stdlib threading primitives and existing turbojpeg API)

**Non-Goals:**
- Viewer mode image loading optimization (remains synchronous, separate concern)
- Image cache or thumbnail cache eviction policies
- WebP/PNG/GIF/BMP/TIFF/SVG decode-time optimizations (no format-level scaling available or justified)
- Thumbnail disk caching between sessions
- Multi-threaded worker pool (single worker thread is sufficient)

## Decisions

### 1. Single background worker thread with mpsc channels

The worker thread receives batches of `(index, PathBuf)` via `mpsc::Sender` and sends back `(index, RgbaImage)` via a separate `mpsc::Sender`. The main thread sends work in `Gallery::render()` and polls results via `try_recv()` in the event loop.

**Alternatives considered:**
- Thread pool (rayon/crossbeam): Rejected — adds dependencies, more complex, single worker is fast enough with DCT scaling (~5-20ms per JPEG thumbnail)
- Async runtime (tokio): Rejected — massive dependency, the work is CPU-bound not I/O-bound, doesn't fit the synchronous Wayland event loop
- Batch N per frame (no threading): Rejected — still blocks the render path, causes visible stutter during thumbnail generation

### 2. Short poll timeout (16ms) when thumbnails are pending

Instead of integrating a custom `eventfd` or pipe into the Wayland `poll()` call for worker wakeup, we use a short 16ms poll timeout when `gallery.has_pending()` is true. This polls at ~60fps, which is fast enough to display thumbnails as they arrive.

**Alternatives considered:**
- eventfd + custom poll fd: Rejected — requires modifying the Wayland event loop to poll multiple fds, adds complexity for negligible latency improvement (16ms vs instant)
- Busy loop: Rejected — wastes CPU when no events and no thumbnails are pending

### 3. JPEG DCT scaling for thumbnails

For JPEG files, use `turbojpeg::Decompressor` with `set_scaling_factor()` to pick the smallest DCT scale (1/2, 1/4, 1/8) where the decoded dimensions are still >= the thumbnail target size. Then bilinear resize to exact thumbnail dimensions. For non-JPEG formats, fall back to full decode + resize (no format-level optimization available).

**Alternatives considered:**
- Always decode at full resolution: Rejected — wastes ~16x memory and CPU for typical 4000x3000 JPEGs being thumbnailed to 200x200
- Always use 1/8 scale: Rejected — for small JPEGs (e.g., 400x300), 1/8 gives 50x38 which is smaller than the 200px thumbnail target, producing blurry upscaled results

### 4. Worker processes items in received order, new batches supersede old

When the visible range changes (scroll/navigation), a new batch is sent to the worker. The worker always processes the most recently received batch first. Items from old batches that were already generated are kept in the cache. Items not yet processed from old batches are discarded when a new batch arrives.

**Alternatives considered:**
- Priority queue with visible-first ordering: Rejected — adds complexity, the simple approach of sending new batches on scroll is sufficient since each thumbnail generates in ~5-20ms
- Cancel individual items: Rejected — mpsc channels don't support cancellation, and the overhead of generating an off-screen thumbnail is trivial

## Risks / Trade-offs

- [Thread safety] Gallery now owns channel endpoints that reference a background thread → The worker thread only accesses `image_loader` functions and `render::generate_thumbnail`, which are stateless and use no global mutable state. `RgbaImage` is `Send` since it's just `Vec<u8>` + dimensions.

- [Panic in worker thread] If the worker panics (e.g., segfault in turbojpeg FFI), the channel disconnects → Main thread detects disconnected channel via `try_recv()` returning `Err(Disconnected)`. Thumbnails stop generating but the gallery remains functional with placeholders. No crash propagation.

- [Memory] Background thread allocates full-resolution images for non-JPEG formats before thumbnailing → This is the same as the current behavior, just happening on a different thread. Transient allocations are freed as soon as the thumbnail is extracted. For JPEGs, DCT scaling dramatically reduces the transient allocation.

- [Ordering] Thumbnails may arrive out of order → Acceptable. The gallery draws whatever is in the cache. Visual effect is thumbnails appearing in near-order with occasional out-of-order fills. No correctness issue.
