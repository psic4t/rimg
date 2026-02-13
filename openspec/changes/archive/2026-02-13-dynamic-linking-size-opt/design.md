## Context

rimg is a minimal Wayland image viewer currently at 1.2 MB (stripped). The previous optimization passes replaced winit/softbuffer with raw wayland-client and switched from the `image` crate to direct system library bindings for JPEG/WebP. However, 1.2 MB is still 10x the target of 100-200 KB.

Binary analysis shows the remaining bloat:
- `fast_image_resize`: 305 KB (SIMD Lanczos3 convolution with SSE4/AVX2 codepaths)
- `std/core` + backtrace/gimli/addr2line: 313 KB (panic infrastructure, formatting, EH frames)
- `png` + `gif` + deflate/crc: 93 KB (pure-Rust decoders)
- `wayland-backend` (pure-Rust wire protocol): 37 KB
- `kamadak-exif`: 23 KB
- `anyhow` + misc: ~20 KB

The reference binary `imv-wayland` achieves 116 KB by dynamically linking all heavy libraries (libwayland-client, libpng, libgif, libjpeg, libwebp, libxkbcommon). It's written in C where libc is always dynamic. For Rust, the equivalent strategy requires `build-std` with `panic = "immediate-abort"` to eliminate the ~280 KB std baseline.

## Goals / Non-Goals

**Goals:**
- Binary size under 200 KB stripped, targeting ~120-160 KB
- Dynamically link all heavy libraries: libpng16, libgif, libwayland-client
- Eliminate pure-Rust decoder/resize crates that have system library equivalents
- Eliminate std bloat via nightly build-std with panic = immediate-abort
- Maintain all existing functionality (viewer, gallery, GIF animation, EXIF orientation, vim keybindings, status bar)

**Non-Goals:**
- `no_std` — too much rewrite effort, build-std is sufficient
- Matching imv's exact 116 KB — C will always be smaller due to no std overhead
- Feature parity with imv — rimg is intentionally simpler (no mouse, no config file)
- Supporting stable Rust — nightly is required for build-std

## Decisions

### 1. Replace fast_image_resize with manual bilinear interpolation

Bilinear interpolation in ~50 lines of safe Rust. For an image viewer, bilinear quality is acceptable — the image is being displayed on screen, not printed. The 305 KB savings (25% of binary) makes this the highest-impact change.

**Alternatives considered:**
- Nearest-neighbor: Even simpler but produces visibly blocky results when downscaling. Rejected for quality.
- Link to a system resize library (e.g., libswscale): Adds a heavy dependency (ffmpeg) for one function. Rejected for complexity.
- Keep fast_image_resize with fewer features: The crate doesn't support feature-gating SIMD codepaths. Rejected.

### 2. FFI to system libpng16 and libgif via link-time binding

Use `#[link(name = "png16")]` and `#[link(name = "gif")]` with `extern "C"` function declarations. This is the simplest FFI approach — the linker resolves symbols at build time against the .so files.

For PNG: `png_create_read_struct` → `png_set_read_fn` → configure transforms (expand palette, gray-to-rgb, add alpha) → `png_read_image` → `png_destroy_read_struct`. ~60 lines of FFI + wrapper.

For GIF: `DGifOpenFileName` → `DGifSlurp` → iterate `SavedImages` array, map palette indices to RGBA using `ColorMap`, extract frame timing via `DGifSavedExtensionToGCB` → `DGifCloseFile`. ~80 lines of FFI + wrapper.

**Alternatives considered:**
- dlopen at runtime (like xkbcommon-dl): More complex, allows graceful fallback. Rejected — these libraries are essential, failing at link time is preferable to runtime.
- Keep pure-Rust crates: 93 KB of binary for no benefit. Rejected.
- Use libspng instead of libpng: Simpler API but less universal. Rejected for portability.

### 3. Switch wayland-backend to client_system + dlopen

The `wayland-backend` crate supports a `client_system` feature that uses `libwayland-client.so` via the `wayland-sys` crate instead of reimplementing the wire protocol in Rust. Combined with the `dlopen` feature, it loads `libwayland-client.so` at runtime. This eliminates ~37 KB of Rust wire protocol code.

**Alternatives considered:**
- Drop wayland-client entirely, raw FFI to libwayland-client.so: Saves more (~90 KB total) but requires reimplementing all Dispatch trait machinery as C-style listener callbacks. Rejected for now — complexity vs ~50 KB additional savings.
- Keep pure-Rust backend: 37 KB of unnecessary code. Rejected.

### 4. Nightly build-std with panic = immediate-abort

Use `cargo-features = ["panic-immediate-abort"]` + `-Zbuild-std` to recompile std with our optimization flags and eliminate all panic/backtrace infrastructure. Testing shows this reduces a hello-world from 293 KB to 11.5 KB.

This eliminates: gimli (DWARF parser), addr2line, rustc_demangle, backtrace_rs, EH frame data, panic message formatting.

**Alternatives considered:**
- Stable Rust with force-unwind-tables=no: Tested, no effect — std is precompiled with unwind tables. Rejected.
- `no_std`: Eliminates std entirely but requires reimplementing allocator, I/O, file operations. Rejected for effort.

### 5. Manual EXIF orientation parser

We read exactly one EXIF field: orientation (tag 0x0112) from JPEG APP1 data. A manual parser (~80 lines) that finds the APP1 marker, parses the TIFF header byte order, walks IFD0 entries for tag 0x0112, and returns the u16 value.

**Alternatives considered:**
- Keep kamadak-exif: 23 KB for one tag read. Rejected.

### 6. Drop anyhow, use Result<T, String>

Replace `anyhow::Result` with `Result<T, String>`. This eliminates anyhow's trait object machinery and reduces format! usage. Error messages use simple string concatenation.

**Alternatives considered:**
- Custom error enum: More idiomatic but unnecessary — image loading errors are just reported and skipped. Rejected for overengineering.

## Risks / Trade-offs

- **[Image quality degradation]** Bilinear is noticeably worse than Lanczos3 for large downscales. → Acceptable for screen display. Users won't notice at typical DPI.
- **[Nightly Rust dependency]** build-std is unstable. → Pin to a known-working nightly date in rust-toolchain.toml. The `panic = "immediate-abort"` feature is on track for stabilization.
- **[Silent panics]** `immediate-abort` means no error message on panic. → Audit all unwrap/expect calls. Use explicit error handling everywhere. Remaining panics are genuine bugs.
- **[System library dependency]** Requires libpng16, libgif, libwayland-client at runtime. → All are standard on any Linux desktop with Wayland. Same deps as imv.
- **[libgif API complexity]** GIF frames use palette indices that must be mapped to RGBA. Transparency and disposal methods need manual handling. → Well-documented API, imv and other viewers do the same.
