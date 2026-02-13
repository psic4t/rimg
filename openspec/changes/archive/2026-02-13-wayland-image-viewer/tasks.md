## 1. Project Scaffold

- [x] 1.1 Run `cargo init` and configure Cargo.toml with all dependencies (winit, softbuffer, image, fast_image_resize, kamadak-exif, anyhow)
- [x] 1.2 Create module file structure (app.rs, viewer.rs, gallery.rs, image_loader.rs, render.rs, font.rs, input.rs, status.rs)

## 2. Windowing

- [x] 2.1 Create Wayland window with winit (800x600 default, resizable, titled)
- [x] 2.2 Initialize softbuffer surface and fill with background color (#1a1a1a)
- [x] 2.3 Handle window resize events (reallocate buffer, trigger re-render)
- [x] 2.4 Handle window close event (graceful exit)

## 3. Image Loading

- [x] 3.1 Implement CLI argument parsing (files, directories, usage/error messages)
- [x] 3.2 Implement directory scanning for supported formats (jpg, jpeg, png, gif, webp, case-insensitive, recursive, sorted)
- [x] 3.3 Implement image decoding for static formats (PNG, JPEG, WebP) via `image` crate
- [x] 3.4 Implement EXIF orientation reading (kamadak-exif) and image transformation (rotate/flip)
- [x] 3.5 Implement animated GIF decoding — extract all frames with delay durations

## 4. Rendering Pipeline

- [x] 4.1 Implement RGBA-to-XRGB pixel format conversion for softbuffer
- [x] 4.2 Implement image scaling using fast_image_resize (Lanczos3)
- [x] 4.3 Implement fit-to-window with aspect ratio preservation and letterboxing
- [x] 4.4 Implement compositing (draw scaled image centered onto background buffer)
- [x] 4.5 Implement alpha blending for images with transparency

## 5. Application State & Input

- [x] 5.1 Implement App struct with mode enum (Viewer/Gallery), image list, current index
- [x] 5.2 Implement vim keybinding dispatch — key event to action mapping, mode-aware
- [x] 5.3 Implement quit handling (q, Escape with mode-aware behavior)

## 6. Viewer Mode

- [x] 6.1 Implement single-image display with fit-to-window rendering
- [x] 6.2 Implement image navigation (n/Space next, p/Backspace prev, g first, G last, wrap-around)
- [x] 6.3 Implement zoom in/out (+/= and -, reset with 0) with stepped zoom levels
- [x] 6.4 Implement pan when zoomed (h/j/k/l, constrained to image bounds)
- [x] 6.5 Update window title on image navigation

## 7. Animated GIF Playback

- [x] 7.1 Implement frame timer using winit ControlFlow::WaitUntil
- [x] 7.2 Implement frame cycling (advance frame at delay interval, loop continuously)
- [x] 7.3 Handle animation start/stop on image navigation (stop on leave, restart on return)

## 8. Bitmap Font & Status Bar

- [x] 8.1 Create embedded bitmap font (8x16 monospace, ASCII printable range, compile-time const)
- [x] 8.2 Implement glyph rendering — draw font pixels onto XRGB buffer at given position and color
- [x] 8.3 Implement status bar — semi-transparent dark strip at bottom-left, white text
- [x] 8.4 Format status text: filename, dimensions, file size, modification time

## 9. Gallery Mode

- [x] 9.1 Implement thumbnail grid layout calculation (200x200 thumbnails, dynamic columns based on window width, spacing)
- [x] 9.2 Implement thumbnail generation (scale images to 200x200 via fast_image_resize, cache in HashMap)
- [x] 9.3 Implement lazy loading — only generate thumbnails for visible viewport + buffer
- [x] 9.4 Implement gallery rendering — draw thumbnail grid with placeholders for unloaded thumbnails
- [x] 9.5 Implement selection highlight (distinct border on selected thumbnail)
- [x] 9.6 Implement gallery navigation (h/j/k/l movement, g/G jump, auto-scroll on selection)
- [x] 9.7 Implement mode toggle — Enter to switch between viewer and gallery, preserving selection

## 10. Polish

- [x] 10.1 Error handling — graceful skip of unloadable images, user-visible error for zero images
- [x] 10.2 Edge cases — single image (no gallery needed), very large images, empty directories
- [x] 10.3 Performance check — verify responsive resize, smooth GIF animation, fast gallery scrolling
