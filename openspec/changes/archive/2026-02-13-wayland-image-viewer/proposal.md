## Why

There is no minimal, keyboard-driven image viewer for Wayland that follows the sxiv philosophy: fast, simple, vim-keyed, zero-config. Existing options either depend on heavy GUI toolkits (GTK/Qt) or are terminal-based with limited rendering fidelity. rimg fills this gap as a native Wayland image viewer with direct pixel rendering, no GPU requirement, and an embedded bitmap font for zero system font dependencies.

## What Changes

- New binary `rimg` — a standalone Wayland image viewer
- Supports PNG, JPEG, GIF (animated), and WebP formats
- Single-image viewer with fit-to-window, zoom, and pan (vim keys)
- Thumbnail grid gallery mode, toggled with Enter
- EXIF orientation detection and automatic correction
- Status bar overlay showing filename, dimensions, file size, and modification time
- Embedded bitmap font for text rendering (no system font dependency)
- CLI interface: `rimg <files...>` or `rimg <directory>` (recursive scan)
- Keyboard-only interaction — no mouse support

## Capabilities

### New Capabilities

- `image-loading` — Image decoding (PNG, JPEG, GIF, WebP), EXIF orientation, animated GIF frame extraction
- `viewer` — Single-image display with fit-to-window, zoom, pan, and image navigation
- `gallery` — Thumbnail grid view with selection navigation and image opening
- `rendering` — Pixel buffer operations, image scaling, compositing, status bar overlay
- `input-handling` — Vim keybinding dispatch for both viewer and gallery modes
- `windowing` — Wayland window lifecycle, event loop, resize handling

### Modified Capabilities

<!-- None — greenfield project -->

## Impact

- New Rust binary crate with dependencies: winit, softbuffer, image, fast_image_resize, kamadak-exif, anyhow
- No existing code affected (greenfield)
- Target platform: Linux with Wayland compositor
- No configuration files, no runtime dependencies beyond Wayland compositor
