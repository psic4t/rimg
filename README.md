# rimg

Minimal Wayland image viewer with vim keybindings.

rimg is a fast, lightweight image viewer for Wayland with no GUI toolkit
dependencies. It supports JPEG, PNG, GIF (animated), WebP (animated), BMP, TIFF,
SVG, AVIF (animated), HEIC/HEIF, and JPEG XL (animated) formats. It can also
set wallpapers on wlroots-based compositors via the
wlr-layer-shell protocol.

## Features

- Vim-style keybindings for navigation
- Viewer mode with zoom, pan, and rotation
- Gallery mode with thumbnail grid
- Animated GIF, WebP, AVIF, and JPEG XL playback
- EXIF metadata overlay (JPEG, TIFF, WebP, PNG, AVIF, HEIC/HEIF, JPEG XL)
- Automatic EXIF orientation correction (JPEG, TIFF, WebP, PNG, AVIF, JPEG XL)
- Runtime sort cycling (name, size, EXIF date, modification time)
- Graceful error handling: corrupt/unsupported images are auto-skipped
- BMP support for 1-bit, 4-bit, and 8-bit indexed color
- Wallpaper mode for wlroots compositors (sway, Hyprland, dwl, etc.)
- Bilinear image scaling
- Embedded bitmap font (no external font dependencies)
- CPU-based software rendering via Wayland SHM

## Dependencies

### Build

- Rust stable toolchain
- pkg-config

### Runtime libraries

- libwayland-client
- libxkbcommon
- libturbojpeg
- libpng16
- libgif
- libwebp
- libtiff
- librsvg-2 (SVG rendering)
- libcairo (used by librsvg)
- libavif (AVIF decoding)
- libheif (HEIC/HEIF decoding)
- libjxl (JPEG XL decoding)

On Debian/Ubuntu:

```sh
apt install libwayland-dev libxkbcommon-dev libturbojpeg0-dev libpng-dev libgif-dev libwebp-dev libtiff-dev librsvg2-dev libcairo2-dev libavif-dev libheif-dev libjxl-dev
```

On Arch Linux:

```sh
pacman -S wayland libxkbcommon libjpeg-turbo libpng giflib libwebp libtiff librsvg cairo libavif libheif libjxl
```

## Building

```sh
cargo build --release
```

The binary is placed at `target/release/rimg`.

## Installation

```sh
sudo make install
```

This installs the binary to `/usr/local/bin`, the man page to
`/usr/local/share/man/man1`, and the README to `/usr/local/share/doc/rimg`.
Set `PREFIX` to change the install location (e.g. `sudo make PREFIX=/usr install`).

To uninstall:

```sh
sudo make uninstall
```

### Arch Linux (AUR)

rimg is available in the [AUR](https://aur.archlinux.org/packages/rimg):

```sh
# Using an AUR helper (e.g., yay)
yay -S rimg

# Or manually
git clone https://aur.archlinux.org/rimg.git
cd rimg
makepkg -si
```

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
| `s` | Cycle sort mode (Name / Size / EXIF Date / Mod Time) |
| `f` | Toggle fullscreen |
| `Enter` | Enter gallery mode |
| `q` / `Escape` | Quit |

### Gallery mode

| Key | Action |
|-----|--------|
| `h/j/k/l` | Navigate thumbnail grid (also arrow keys) |
| `g` | First image |
| `G` | Last image |
| `s` | Cycle sort mode |
| `Enter` | Open selected image |
| `q` | Quit |
| `Escape` | Return to viewer mode |

## License
Copyright (C) 2026 psic4t

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>. 
