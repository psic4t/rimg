## 1. BMP 1/4/8-bit Indexed Color Support

- [x] 1.1 Parse DIB header fields: read `biCompression` (bytes 30-33) and `biClrUsed` (bytes 46-49) in `load_bmp()` in `image_loader.rs`
- [x] 1.2 Parse the color table: read `biClrUsed` BGRA entries (4 bytes each) starting after the DIB header; validate bounds and default `biClrUsed` to `2^bits_per_pixel` when zero
- [x] 1.3 Implement 8-bit pixel decoding: each byte is one palette index, map through color table to RGBA output
- [x] 1.4 Implement 4-bit pixel decoding: each byte is two indices (high nibble = left, low nibble = right), handle odd-width rows
- [x] 1.5 Implement 1-bit pixel decoding: each byte is 8 indices (MSB = leftmost), handle partial last byte per row
- [x] 1.6 Reject BI_RLE4 and BI_RLE8 compression with clear error messages
- [x] 1.7 Add BMP parser unit tests: synthetic byte arrays for 1-bit, 4-bit, 8-bit, 24-bit, 32-bit decoding plus error cases

## 2. EXIF for TIFF, WebP, and PNG

- [x] 2.1 Create `read_exif_tags_tiff(data: &[u8])` that calls `parse_all_exif_tags(data, 0)` directly in `image_loader.rs`
- [x] 2.2 Create `read_exif_orientation_tiff(data: &[u8])` that calls `parse_tiff_orientation(data, 0)` in `image_loader.rs`
- [x] 2.3 Create `read_exif_from_webp(data: &[u8])` that walks the RIFF container for the `EXIF` chunk, handles optional `Exif\0\0` prefix, passes payload to `parse_all_exif_tags`
- [x] 2.4 Create `read_exif_orientation_webp(data: &[u8])` for WebP orientation extraction
- [x] 2.5 Create `read_exif_from_png(data: &[u8])` that walks PNG chunks for `eXIf`, passes payload to `parse_all_exif_tags`
- [x] 2.6 Create `read_exif_orientation_png(data: &[u8])` for PNG orientation extraction
- [x] 2.7 Wire EXIF orientation into `load_webp()` and `load_png()` — call orientation functions and apply transforms
- [x] 2.8 Extend `load_exif_for_current()` in `app.rs` to handle `tiff`, `tif`, `webp`, and `png` extensions
- [x] 2.9 Add EXIF parser unit tests: crafted TIFF segments (LE/BE), RIFF container with EXIF chunk, PNG chunk walking

## 3. Animated WebP Support

- [x] 3.1 Enable `demux` feature on `libwebp-sys2` in `Cargo.toml`: `libwebp-sys2 = { version = "0.2", features = ["demux"] }`
- [x] 3.2 Add animation detection in `load_webp()`: call `WebPGetFeatures` and check `has_animation`
- [x] 3.3 Implement animated WebP decoding: create `WebPAnimDecoder`, iterate frames with `WebPAnimDecoderGetNext`, compute frame durations from cumulative timestamp deltas (min 10ms)
- [x] 3.4 Return `LoadedImage::Animated { frames }` for multi-frame WebP, `LoadedImage::Static` for single-frame
- [x] 3.5 Verify animated WebP playback works end-to-end with the existing viewer animation loop

## 4. Error Resilience

- [x] 4.1 Add an optional transient error message field to the status bar state (with auto-dismiss deadline) in `status.rs` or `app.rs`
- [x] 4.2 Modify `ensure_image_loaded()` in `app.rs`: on load failure, remove the failed path from `self.paths`, adjust `self.current_index`, and set the error message
- [x] 4.3 Handle the all-paths-exhausted case: display "No valid images" message, allow quit
- [x] 4.4 Render the transient error message in the status bar; check deadline in the event loop to clear it
- [x] 4.5 Handle gallery thumbnail failures: keep placeholder, attempt full load on Enter (triggering auto-skip if it fails)

## 5. Runtime Sort Cycling

- [x] 5.1 Add `SortMode` enum (`Name`, `Size`, `ExifDate`, `ModTime`) and `Action::CycleSort` variant
- [x] 5.2 Map `s` key to `Action::CycleSort` in both Viewer and Gallery modes in `input.rs`
- [x] 5.3 Implement sort cycling logic in `handle_action()`: advance mode, re-sort paths, update `current_index` to maintain current image selection
- [x] 5.4 Implement sort comparators: lexicographic name, file size via `fs::metadata`, modification time via `fs::metadata`, EXIF DateTimeOriginal (JPEG-only, fallback to mtime)
- [x] 5.5 Add metadata caching: `HashMap<PathBuf, (u64, u64)>` for (size, mtime) and `HashMap<PathBuf, Option<u64>>` for EXIF dates
- [x] 5.6 Implement toast overlay: add toast state (message + deadline) to `App`, render as small rounded rectangle at top-right in `viewer.rs` or `render.rs`
- [x] 5.7 Integrate toast deadline into event loop poll timeout; clear toast on expiry and trigger redraw
- [x] 5.8 Update gallery selection after re-sort to track the same image

## 6. Test Suite Foundation

- [x] 6.1 Add `tempfile` as a dev-dependency in `Cargo.toml`
- [x] 6.2 Add image transform tests in `image_loader.rs`: `rotate_90`, `rotate_180`, `rotate_270`, `flip_h`, `flip_v` on small pixel buffers
- [x] 6.3 Add status formatting tests in `status.rs`: `format_file_size` and `days_to_date` edge cases
- [x] 6.4 Add input mapping tests in `input.rs`: verify `map_key` returns correct `Action` for key+mode combinations
- [x] 6.5 Add gallery navigation tests in `gallery.rs`: `move_left`, `move_right`, `move_up`, `move_down`, `go_first`, `go_last` index arithmetic
- [x] 6.6 Add render tests in `render.rs`: `scale_to_fit` dimension calculations, `composite_centered` alpha blending on small buffers
- [x] 6.7 Verify full test suite passes with `cargo test`

## 7. Documentation Updates

- [x] 7.1 Update `README.md`: mention animated WebP support, indexed BMP, EXIF for TIFF/WebP/PNG, `s` keybind for sorting, error handling behavior
- [x] 7.2 Update `rimg.1` man page: add `s` keybind, document sort modes
- [x] 7.3 Update version to `1.0.0` in `Cargo.toml` and `rimg.1`
