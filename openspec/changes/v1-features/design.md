## Context

rimg is a minimalist Wayland image viewer (~5,100 lines of Rust) that uses system C libraries for decoding and direct Wayland for display. The codebase has no GUI toolkit, no async runtime, and no configuration file. The v0.1.0 release covers core viewing but has gaps: animated WebP is not decoded, indexed BMPs fail, EXIF is JPEG-only, load failures show a blank screen, and there's no runtime sort.

All image decoders return `Result<LoadedImage, String>` where `LoadedImage` is either `Static(RgbaImage)` or `Animated { frames: Vec<(RgbaImage, Duration)> }`. The event loop uses `rustix::event::poll` with deadline-based animation timing. The key dispatch maps `(keysym, modifiers, mode)` → `Action` enum variants.

## Goals / Non-Goals

**Goals:**
- Complete format coverage: animated WebP, indexed BMP (1/4/8-bit)
- EXIF tag reading for TIFF, WebP, and PNG in addition to JPEG
- Graceful error recovery: auto-skip bad files, show feedback in status bar
- Runtime sort cycling with visual toast feedback
- Unit test coverage for all pure-Rust components

**Non-Goals:**
- Mouse/scroll input (deferred)
- Configuration file (deferred)
- RLE-compressed BMP support (rare format, error message instead)
- Animated WebP editing or re-encoding
- Integration tests requiring a Wayland compositor
- EXIF writing or modification

## Decisions

### 1. Animated WebP via libwebp-sys2 demux feature

Enable the `demux` feature on the existing `libwebp-sys2` crate to access `WebPAnimDecoder*` functions. Detect animation with `WebPGetFeatures` (available without demux), then iterate frames via `WebPAnimDecoderGetNext` which returns fully-composited RGBA frames with cumulative timestamps.

The `WebPAnimDecoder` handles all frame compositing internally (blending, disposal, alpha), unlike GIF where we manually composite palette-indexed sub-frames onto a canvas. Frame durations are computed as deltas between consecutive cumulative timestamps.

Return `LoadedImage::Animated { frames }` — the existing viewer animation loop (`start_animation`, `advance_frame`, `next_frame_deadline`) is format-agnostic and handles animated WebP without modification.

**Alternatives considered:**
- Pure-Rust WebP decoder: No mature animated WebP crate exists. The `image` crate's WebP support is limited. Rejected for reliability.
- FFI to libwebpdemux directly: Unnecessary since `libwebp-sys2` already provides complete bindings behind a feature flag.

### 2. BMP indexed color: color table lookup, no RLE

Parse the DIB header to extract `biCompression` and `biClrUsed`. Read the color table (BGRA entries) between the DIB header and pixel data. Unpack pixel indices from 8-bit (1 byte = 1 index), 4-bit (1 byte = 2 indices, high nibble first), and 1-bit (1 byte = 8 indices, MSB first). Map each index to the color table and write RGBA output.

Reject `BI_RLE4` (2) and `BI_RLE8` (1) compression with a clear error message. Only support `BI_RGB` (0) uncompressed format for indexed BMPs.

**Alternatives considered:**
- Implement RLE decompression: Adds complexity for a format that's very rarely encountered. Rejected for now, can add later if needed.
- Use an external BMP library: Against the project philosophy of minimal dependencies and manual parsers.

### 3. EXIF for TIFF/WebP/PNG via existing parser infrastructure

The existing `parse_all_exif_tags(data, tiff_offset)` function handles the complete TIFF/IFD/EXIF structure. Each new format just needs a thin extraction layer:

- **TIFF**: The file IS a TIFF structure. Call `parse_all_exif_tags(data, 0)` directly — the TIFF header is at byte 0.
- **WebP**: Walk the RIFF container (`RIFF....WEBP` header, then 4-byte FourCC + 4-byte LE size chunks) looking for the `EXIF` chunk. The payload is a TIFF header (possibly prefixed with `Exif\0\0`). Pass to `parse_all_exif_tags`.
- **PNG**: Walk PNG chunks (`4-byte length + 4-byte type + payload + 4-byte CRC`) looking for `eXIf`. The payload is a raw TIFF header. Pass to `parse_all_exif_tags`.

Wire into `load_exif_for_current()` (app.rs) by extending the extension match to include `tiff`, `tif`, `webp`, and `png`.

**Alternatives considered:**
- Use `kamadak-exif` or `exif` crate: Against minimalism. The manual parser already handles the heavy lifting.

### 4. Error resilience: remove-and-skip with status bar feedback

When `load_image()` fails in `ensure_image_loaded()`:
1. Remove the failed path from `self.paths`
2. Adjust `self.current_index` to stay valid
3. Set a transient error message on the status bar (e.g., `"Skipped: corrupt.jpg"`)
4. If all paths exhausted, display a "No valid images" message and allow quit

The status bar message uses a timed auto-dismiss (3 seconds). The existing `StatusInfo` struct in `status.rs` gains an optional error field and a deadline.

For gallery mode, failed thumbnails keep their gray placeholder but the cell is marked so navigation can skip or de-emphasize them.

**Alternatives considered:**
- Show an error placeholder screen: Requires rendering a special error image. Adds complexity for a state the user wants to pass through, not linger on.
- Keep bad files in the list: Confusing UX — user lands on blank screens repeatedly.

### 5. Runtime sort cycling via `s` keybind with toast overlay

Add a `SortMode` enum: `Name`, `Size`, `ExifDate`, `ModTime`. Store the current mode in `App`. On `Action::CycleSort`:
1. Advance to the next sort mode
2. Re-sort `self.paths` according to the new mode
3. Find the current image's new index to maintain selection
4. Show a toast overlay (e.g., `"Sort: Name"`) at the top-right corner

For `ExifDate` sort: read EXIF `DateTimeOriginal` from JPEG files. Cache parsed dates in a `HashMap<PathBuf, Option<u64>>`. Non-JPEG files and files without EXIF date fall back to filesystem modification time.

For `Size` and `ModTime` sorts: use `fs::metadata()`. Cache `(size, mtime)` per path on first sort.

The toast overlay is a small rounded rectangle rendered like the existing EXIF overlay, positioned at top-right, auto-dismissed after 1.5 seconds via a deadline checked in the event loop.

**Alternatives considered:**
- CLI flags for sort: Less useful for browsing; user can't change sort while viewing. Does not match the runtime cycling UX the user requested.
- Separate keybinds per sort mode: Uses more keyspace. Cycling is simpler and discoverable.

### 6. Test suite: unit tests for pure-Rust components

Place `#[cfg(test)] mod tests` in each source file for private function testing. Focus exclusively on pure-Rust units that require no C library or Wayland:

- `image_loader.rs`: BMP parsing (craft synthetic byte arrays), EXIF parsing (craft TIFF segments), image transforms (`rotate_90/180/270`, `flip_h/v` on 2x2 pixel buffers)
- `render.rs`: `scale_to_fit` dimension math, `composite_centered` alpha blending
- `status.rs`: `format_file_size`, `days_to_date` formatting
- `input.rs`: `map_key` → `Action` for each mode
- `gallery.rs`: Navigation index arithmetic (`move_left/right/up/down`, `go_first/last`)
- `font.rs`: `draw_char` pixel output verification

Add `tempfile` as a dev-dependency for path collection tests that need temporary directories.

**Alternatives considered:**
- Integration tests with real image files: Requires C libraries in CI. Worth adding later but not for the initial suite.
- Property-based testing (proptest): Overkill for the current scope. Standard assertions on known inputs are sufficient.

## Risks / Trade-offs

- **[Animated WebP memory]** Large animated WebPs can have hundreds of frames at high resolution. Each frame is `width * height * 4` bytes. → Mitigation: The existing `MAX_PIXEL_COUNT` (256 megapixels) limit applies per-frame; same as GIF.
- **[Sort performance on large directories]** `ExifDate` sort requires reading EXIF from potentially thousands of JPEGs on first sort. → Mitigation: Parse only `DateTimeOriginal` (not full EXIF), use JPEG DCT-scaling path for fast reads, cache results. Fall back to mtime for non-JPEG.
- **[Path removal during iteration]** Removing failed paths from `self.paths` while iterating can cause index confusion. → Mitigation: Remove-and-navigate is atomic (remove, then clamp index, then load). Gallery thumbnail cache uses path-based keys, not indices.
- **[BMP color table bounds]** Malformed BMPs may have invalid `biClrUsed` or truncated color tables. → Mitigation: Validate `biClrUsed <= 2^bits_per_pixel`, check data bounds before reading color table entries.
