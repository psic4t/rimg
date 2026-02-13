## ADDED Requirements

### Requirement: PNG decoding via system libpng16

Decode PNG images by calling the system libpng16 shared library through FFI.

#### Scenario: Load a standard RGBA PNG
- **WHEN** a PNG file is opened
- **THEN** it is decoded to an RGBA pixel buffer using libpng16's `png_read_image` API
- **AND** all color types (RGB, RGBA, grayscale, grayscale+alpha, palette) are expanded to RGBA via libpng transforms (`png_set_expand`, `png_set_gray_to_rgb`, `png_set_add_alpha`)

#### Scenario: PNG decode error
- **WHEN** libpng encounters a corrupt or unsupported PNG file
- **THEN** an error string is returned and the file is skipped without crashing

### Requirement: GIF decoding via system libgif

Decode GIF images (including animated) by calling the system libgif shared library through FFI.

#### Scenario: Load a static GIF
- **WHEN** a single-frame GIF file is opened
- **THEN** it is decoded using `DGifOpenFileName` + `DGifSlurp`
- **AND** palette indices are mapped to RGBA using the frame's local ColorMap (or global ColorMap as fallback)
- **AND** transparent color index (from GraphicsControlBlock) maps to alpha 0

#### Scenario: Load an animated GIF
- **WHEN** a multi-frame GIF is opened
- **THEN** all frames are decoded with their timing (DelayTime from GraphicsControlBlock, in centiseconds)
- **AND** each frame is composited onto a canvas at the frame's (Left, Top) offset
- **AND** frames with transparent pixels preserve the previous canvas content

#### Scenario: GIF decode error
- **WHEN** libgif cannot decode the file
- **THEN** an error string is returned and the file is skipped without crashing

### Requirement: Link-time binding to system libraries

The FFI bindings use `#[link(name = "png16")]` and `#[link(name = "gif")]` for link-time resolution.

#### Scenario: Build with system libraries
- **WHEN** the project is built
- **THEN** the linker resolves libpng16 and libgif symbols against the system .so files
- **AND** the resulting binary has dynamic dependencies on `libpng16.so` and `libgif.so`
