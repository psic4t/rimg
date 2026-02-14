## 1. JPEG DCT-Scaled Thumbnail Loading

- [x] 1.1 Add `load_image_thumbnail(path, thumb_size) -> Result<RgbaImage>` to `src/image_loader.rs` — for JPEG: use `turbojpeg::Decompressor` with `set_scaling_factor()` selecting the best DCT scale (1/8, 1/4, 1/2, or none) where scaled dims >= thumb_size, then apply EXIF orientation, then `render::generate_thumbnail()` for final resize
- [x] 1.2 Add non-JPEG fallback path in `load_image_thumbnail()` — call existing `load_image()`, extract first frame, call `render::generate_thumbnail()`

## 2. Background Worker Thread

- [x] 2.1 Add channel types and worker fields to `Gallery` struct — `mpsc::Sender<Vec<(usize, PathBuf)>>` for requests, `mpsc::Receiver<(usize, RgbaImage)>` for results, `HashSet<usize>` for pending indices
- [x] 2.2 Spawn worker thread in `Gallery::new()` — loops on receiving batches, calls `load_image_thumbnail()` for each item, sends results back; exits on channel disconnect
- [x] 2.3 Add `Gallery::poll_thumbnails() -> bool` — drains receiver via `try_recv()`, inserts completed thumbnails into cache, removes from pending set, returns true if any received

## 3. Gallery Render Integration

- [x] 3.1 Replace synchronous thumbnail generation loop in `Gallery::render()` — remove the `for i in load_start..load_end` block that calls `image_loader::load_image()`; instead collect missing non-pending indices and send as batch to worker
- [x] 3.2 Add `Gallery::has_pending() -> bool` method — returns `!self.pending.is_empty()`

## 4. Event Loop Integration

- [x] 4.1 Add thumbnail polling to `App::run()` event loop — when in gallery mode, call `self.gallery.poll_thumbnails()` and set `needs_redraw = true` if it returns true
- [x] 4.2 Adjust poll timeout — when in gallery mode and `gallery.has_pending()`, use 16ms timeout instead of -1 (block indefinitely)

## 5. Verification

- [x] 5.1 Build the project with `cargo build --release` and verify no compile errors
