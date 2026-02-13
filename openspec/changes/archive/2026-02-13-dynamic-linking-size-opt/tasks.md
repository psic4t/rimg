## 1. Build Configuration

- [x] 1.1 Update `rust-toolchain.toml` to `channel = "nightly"`
- [x] 1.2 Create `.cargo/config.toml` with `build-std = ["std", "core"]` and `target = "x86_64-unknown-linux-gnu"`
- [x] 1.3 Update `Cargo.toml`: add `cargo-features = ["panic-immediate-abort"]`, set `panic = "immediate-abort"`, `opt-level = "z"`, `strip = "symbols"`
- [x] 1.4 Update `Cargo.toml`: remove `fast_image_resize`, `png`, `gif`, `kamadak-exif`, `anyhow` dependencies
- [x] 1.5 Update `Cargo.toml`: set `wayland-backend` features to `["client_system", "dlopen"]`
- [x] 1.6 Verify nightly build-std compiles with existing code (before source changes)

## 2. Drop anyhow

- [x] 2.1 Replace `anyhow::Result` with `Result<T, String>` in `src/image_loader.rs`
- [x] 2.2 Replace `anyhow::bail!` with `return Err(format!(...))` or `return Err(String::from(...))`
- [x] 2.3 Replace `.with_context(|| ...)` with `.map_err(|e| format!(...))`
- [x] 2.4 Update `src/app.rs` error handling to match new return types

## 3. Bilinear Resize

- [x] 3.1 Remove `fast_image_resize` imports from `src/render.rs`
- [x] 3.2 Implement `resize_bilinear(src: &RgbaImage, dst_w: u32, dst_h: u32) -> RgbaImage` in `src/render.rs`
- [x] 3.3 Update `resize_rgba()` to call `resize_bilinear` instead of fast_image_resize

## 4. Manual EXIF Parser

- [x] 4.1 Remove `kamadak-exif` import from `src/image_loader.rs`
- [x] 4.2 Implement `read_exif_orientation(data: &[u8]) -> Option<u32>` that parses JPEG APP1 marker, TIFF header, and IFD0 orientation tag
- [x] 4.3 Update `load_jpeg()` to call the new manual parser (pass raw bytes, not path)

## 5. System libpng16 FFI

- [x] 5.1 Add FFI declarations for libpng16: `png_create_read_struct`, `png_create_info_struct`, `png_set_read_fn`, `png_read_info`, `png_get_IHDR`, transform functions, `png_read_image`, `png_destroy_read_struct`, `png_set_longjmp_fn`/`png_jmpbuf` for error handling
- [x] 5.2 Implement `load_png(path: &Path) -> Result<LoadedImage, String>` using libpng16 FFI
- [x] 5.3 Handle all PNG color types via libpng transforms (palette→RGB, gray→RGB, add alpha, expand 16→8)
- [x] 5.4 Handle libpng error callbacks via setjmp/longjmp

## 6. System libgif FFI

- [x] 6.1 Add FFI declarations for libgif: `DGifOpenFileName`, `DGifSlurp`, `DGifCloseFile`, `DGifSavedExtensionToGCB`, and required struct definitions (`GifFileType`, `SavedImage`, `GifImageDesc`, `ColorMapObject`, `GifColorType`, `GraphicsControlBlock`, `ExtensionBlock`)
- [x] 6.2 Implement `load_gif(path: &Path) -> Result<LoadedImage, String>` using libgif FFI
- [x] 6.3 Map palette indices to RGBA using local/global ColorMap
- [x] 6.4 Handle transparency via GraphicsControlBlock.TransparentColor
- [x] 6.5 Extract frame timing (DelayTime in centiseconds → Duration) from GraphicsControlBlock
- [x] 6.6 Composite frames onto canvas at (Left, Top) offset for animated GIFs

## 7. Wayland Backend Switch

- [x] 7.1 Verify `wayland-backend` with `client_system` + `dlopen` compiles with existing `src/wayland.rs`
- [x] 7.2 Confirm `libwayland-client.so` appears in `ldd` output of the binary (Note: uses dlopen, so not in ldd but loaded at runtime)

## 8. Integration and Verification

- [x] 8.1 Build release binary and verify it compiles cleanly (0 warnings)
- [x] 8.2 Verify binary size is under 200 KB (achieved: 194 KB / 198,160 bytes)
- [x] 8.3 Verify `ldd` shows dynamic deps: libpng16, libgif, libturbojpeg, libwebp (libwayland-client via dlopen)
- [x] 8.4 Test: load JPEG file — passes (runs without crash)
- [x] 8.5 Test: load PNG file — passes (runs without crash)
- [x] 8.6 Test: load animated GIF — passes (runs without crash)
- [x] 8.7 Test: load WebP file — passes (runs without crash)
- [x] 8.8 Test: gallery mode with multiple images — passes (directory loading works)
- [ ] 8.9 Test: vim keybindings (h/j/k/l, n/p, g/G, +/-/0, Enter, q/Esc) — requires interactive session

## 9. Additional Optimizations (beyond original plan)

- [x] 9.1 Linker script to discard `.eh_frame` / `.eh_frame_hdr` sections (~34 KB saved)
- [x] 9.2 Non-PIE binary (`relocation-model=static`, `-no-pie`) to eliminate R_X86_64_RELATIVE relocations (~23 KB saved)
- [x] 9.3 `build-std-features = ["optimize_for_size"]` for smaller std internals (~4 KB saved)
- [x] 9.4 Replace `to_lowercase()` with ASCII-only lowering to eliminate Unicode tables (~14 KB saved)
- [x] 9.5 Add `log = { features = ["max_level_off"] }` to eliminate log formatting code (~500 bytes saved)
- [x] 9.6 Replace float formatting (`{:.1}`) with integer arithmetic in status bar
- [x] 9.7 Remove unused FFI declarations (`png_init_io`, `png_set_sig_bytes`)
