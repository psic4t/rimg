## Why

The rimg binary is currently 1.2 MB stripped. The target is 100-200 KB, matching `imv-wayland` (116 KB). The bloat comes from statically compiled pure-Rust libraries (305 KB for image resize alone, 98 KB for panic backtrace, 77 KB for PNG/GIF decoders) and Rust's std baseline (~280 KB). All equivalent system libraries already exist on the target platform and should be dynamically linked instead.

## What Changes

- **BREAKING** Drop `fast_image_resize` crate (305 KB) — replace with manual bilinear interpolation (~50 lines)
- **BREAKING** Drop `png` and `gif` crates (93 KB combined) — replace with FFI bindings to system `libpng16.so` and `libgif.so`
- **BREAKING** Drop `kamadak-exif` crate (23 KB) — replace with manual EXIF orientation parser (~80 lines)
- **BREAKING** Drop `anyhow` crate — replace with `Result<T, String>` throughout
- Switch `wayland-backend` to `client_system` + `dlopen` features — dynamically loads `libwayland-client.so` instead of pure-Rust wire protocol
- Switch to nightly Rust with `build-std` + `panic = "immediate-abort"` — eliminates backtrace/gimli/addr2line (~163 KB) and panic formatting overhead
- Add `opt-level = "z"` to release profile — optimize for binary size

## Capabilities

### New Capabilities

- `system-image-decoders` — FFI bindings to system libpng16 and libgif for PNG/GIF decoding
- `bilinear-resize` — Manual bilinear image interpolation replacing fast_image_resize
- `manual-exif` — Minimal EXIF orientation parser without external crate

### Modified Capabilities

- `image-loading` — PNG/GIF/EXIF loading switches from pure-Rust crates to system library FFI and manual parsing
- `rendering` — Image resize switches from fast_image_resize Lanczos3 to bilinear interpolation
- `windowing` — Wayland backend switches from pure-Rust to system libwayland-client.so via dlopen

## Impact

- **Runtime dependencies added**: `libpng16.so`, `libgif.so`, `libwayland-client.so` (all standard on Linux/Wayland systems)
- **Build requirement**: Nightly Rust toolchain (for `build-std` and `panic = "immediate-abort"`)
- **Image quality**: Bilinear resize is lower quality than Lanczos3 for downscaling. Acceptable for an image viewer.
- **Error handling**: `panic = "immediate-abort"` means panics silently abort instead of printing messages. All error paths must use explicit error handling, not unwrap/expect.
- **Files modified**: `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`, `src/image_loader.rs`, `src/render.rs`, `src/app.rs`
- **Files unchanged**: `src/viewer.rs`, `src/gallery.rs`, `src/font.rs`, `src/status.rs`, `src/input.rs`, `src/wayland.rs`, `src/protocols.rs`, `src/main.rs`
