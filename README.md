# rimg

Minimal Wayland image viewer with vim keybindings.

rimg is a fast, lightweight image viewer for Wayland with no GUI toolkit
dependencies. It supports JPEG, PNG, GIF (animated), and WebP formats. It can
also set wallpapers on wlroots-based compositors via the wlr-layer-shell
protocol.

## Features

- Vim-style keybindings for navigation
- Viewer mode with zoom, pan, and rotation
- Gallery mode with thumbnail grid
- Animated GIF playback
- EXIF metadata overlay (JPEG)
- Automatic EXIF orientation correction
- Wallpaper mode for wlroots compositors (sway, Hyprland, dwl, etc.)
- Bilinear image scaling
- Embedded bitmap font (no external font dependencies)
- CPU-based software rendering via Wayland SHM

## Dependencies

### Build

- Rust nightly toolchain
- pkg-config

### Runtime libraries

- libwayland-client
- libxkbcommon
- libturbojpeg
- libpng16
- libgif
- libwebp

On Debian/Ubuntu:

```sh
apt install libwayland-dev libxkbcommon-dev libturbojpeg0-dev libpng-dev libgif-dev libwebp-dev
```

On Arch Linux:

```sh
pacman -S wayland libxkbcommon libjpeg-turbo libpng giflib libwebp
```

## Building

```sh
cargo build --release
```

The binary is placed at `target/x86_64-unknown-linux-gnu/release/rimg`.

## Usage

```sh
rimg [options] <file>... | rimg [options] <directory>
```

When given a directory, rimg recursively scans for supported image files.

### Options

| Flag | Description |
|------|-------------|
| `-h`, `--help` | Show help message |
| `-w` | Set image as wallpaper (wlr-layer-shell) |

### Examples

```sh
# View a single image
rimg photo.jpg

# View multiple images
rimg photo1.jpg photo2.png image3.gif

# View all images in a directory
rimg ~/Pictures/

# Set wallpaper (wlroots compositors only)
rimg -w wallpaper.jpg
```

## Keybindings

### Viewer mode

| Key | Action |
|-----|--------|
| `n` / `Space` | Next image |
| `p` / `Backspace` | Previous image |
| `g` | First image |
| `G` | Last image |
| `+` / `=` | Zoom in |
| `-` | Zoom out |
| `0` | Zoom reset (fit-to-window) |
| `h/j/k/l` | Pan when zoomed, `h`/`l` navigate images otherwise (also arrow keys) |
| `Shift+w` | Toggle fit-to-window for small images |
| `Ctrl+0` | Display at actual size (1:1 pixels) |
| `r` | Rotate clockwise 90 degrees |
| `R` | Rotate counterclockwise 90 degrees |
| `e` | Toggle EXIF info overlay |
| `f` | Toggle fullscreen |
| `Enter` | Enter gallery mode |
| `q` / `Escape` | Quit |

### Gallery mode

| Key | Action |
|-----|--------|
| `h/j/k/l` | Navigate thumbnail grid (also arrow keys) |
| `g` | First image |
| `G` | Last image |
| `Enter` | Open selected image |
| `q` | Quit |
| `Escape` | Return to viewer mode |

## Author

psic4t <psic4t@data.haus>
