## Why

rimg v0.1.0 covers the core viewing experience but has gaps in format support, error resilience, and organization tooling that prevent it from being considered a complete v1.0 release. Animated WebP is increasingly common, indexed BMP files fail to load, EXIF data is only available for JPEG, load failures produce a blank screen with no feedback, and there is no way to re-order images at runtime. These gaps need to be filled before a stable v1.0.

## What Changes

- Add animated WebP playback using the libwebp demux API, matching existing animated GIF behavior
- Add BMP support for 1-bit, 4-bit, and 8-bit indexed color depths with color table parsing
- Extend EXIF tag reading to TIFF, WebP, and PNG files (in addition to existing JPEG support)
- Auto-skip unloadable images with a brief status bar error message instead of showing a blank screen
- Add runtime sort cycling via `s` keybind (name, size, EXIF date, modification date) with a toast notification
- Add a unit test suite covering pure-Rust components (EXIF parser, BMP parser, image transforms, rendering, status formatting, input mapping, gallery navigation)

## Capabilities

### New Capabilities

- `runtime-sorting` — cycle through sort modes at runtime via keybind, with toast feedback
- `error-resilience` — graceful handling of unloadable images with auto-skip and status bar feedback
- `test-suite` — unit tests for pure-Rust components

### Modified Capabilities

- `image-loading` — BMP now supports 1/4/8-bit indexed depths; WebP now supports animated playback
- `manual-exif` — EXIF tag reading and orientation correction extended to TIFF, WebP, and PNG formats
- `input-handling` — new `s` keybind for sort cycling in both viewer and gallery modes
- `viewer` — animated WebP playback uses the same mechanism as animated GIF; toast overlay for sort mode changes

## Impact

- **Cargo.toml**: `libwebp-sys2` gains the `demux` feature flag (links `libwebpdemux`, already shipped with `libwebp`)
- **image_loader.rs**: Major changes — animated WebP decoder, BMP indexed color support, EXIF extraction for TIFF/WebP/PNG
- **input.rs**: New `Action::CycleSort` variant and `s` key mapping
- **app.rs**: Sort cycling logic, error-skip logic with path list mutation, toast state management
- **viewer.rs**: Toast overlay rendering and auto-dismiss timer
- **status.rs**: Error message display support
- **No new system library dependencies** — `libwebpdemux` is part of the existing `libwebp` package
- **No breaking changes** to CLI interface or existing keybindings
