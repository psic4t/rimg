## Why

Opening the gallery currently blocks the UI for several seconds because every visible thumbnail is synchronously loaded at full resolution and resized before the first frame is drawn. For a screen of 25 thumbnails of high-resolution JPEGs, this means decoding ~375 megapixels on the render path before anything appears.

## What Changes

- Gallery opens instantly, displaying placeholder rectangles for all cells
- A background worker thread generates thumbnails asynchronously, sending completed thumbnails back to the main thread via channel
- Placeholders are progressively replaced with real thumbnails as they become ready
- JPEG thumbnails use turbojpeg DCT scaling (1/2, 1/4, or 1/8) to decode at reduced resolution before resizing, dramatically reducing decode time and memory usage
- Main event loop polls for completed thumbnails at short intervals (16ms) when work is pending
- A new `load_image_thumbnail()` function in `image_loader` provides optimized thumbnail-specific loading

## Capabilities

### New Capabilities

- `background-thumbnail-generation`: Background worker thread for non-blocking thumbnail generation with channel-based communication

### Modified Capabilities

- `gallery`: Thumbnail generation moves from synchronous inline to asynchronous background; render always returns immediately
- `image-loading`: New `load_image_thumbnail()` function with JPEG DCT scaling for thumbnail-optimized loading

## Impact

- `src/gallery.rs`: Major restructuring — adds thread spawn, channels, pending tracking; removes synchronous load loop from render
- `src/image_loader.rs`: New public function `load_image_thumbnail()` using `turbojpeg::Decompressor` with scaling
- `src/app.rs`: Event loop changes — polls gallery for completed thumbnails, adjusts poll timeout when thumbnails are pending
- `src/render.rs`: No changes
- `Cargo.toml`: No new dependencies (std::thread, std::sync::mpsc are stdlib; turbojpeg Decompressor already available)
- Threading: Project moves from fully single-threaded to having one background worker thread
