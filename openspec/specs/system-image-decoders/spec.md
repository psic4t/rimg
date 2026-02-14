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

### Requirement: TIFF decoding via system libtiff

Decode TIFF images by calling the system libtiff shared library through FFI.

#### Scenario: Load a TIFF image
- **WHEN** a TIFF file is opened
- **THEN** it is decoded using `TIFFOpen` + `TIFFReadRGBAImageOriented` with top-left orientation
- **AND** libtiff's ABGR packed u32 pixels are converted to RGBA byte order
- **AND** the result is an `RgbaImage`

#### Scenario: TIFF decode error
- **WHEN** libtiff cannot open or decode the file
- **THEN** an error string is returned and the file is skipped without crashing

### Requirement: SVG decoding via system librsvg and cairo

Rasterize SVG images by calling the system librsvg-2 and cairo shared libraries through FFI. SVGs are rendered to a cairo image surface and converted to RGBA pixels.

#### Scenario: Load an SVG with intrinsic pixel dimensions
- **WHEN** an SVG file with `width` and `height` attributes (in resolvable units) is opened
- **THEN** `rsvg_handle_new_from_file` loads and parses the SVG
- **AND** `rsvg_handle_get_intrinsic_size_in_pixels` determines the rasterization size
- **AND** the DPI is set to 96 via `rsvg_handle_set_dpi`
- **AND** a cairo ARGB32 image surface is created at those dimensions
- **AND** `rsvg_handle_render_document` renders the SVG to the surface
- **AND** cairo's premultiplied BGRA pixel data is converted to straight RGBA

#### Scenario: Load an SVG without intrinsic dimensions
- **WHEN** an SVG file has only a `viewBox` or percentage-based width/height
- **THEN** `rsvg_handle_get_intrinsic_size_in_pixels` returns false
- **AND** the SVG is rasterized at a default size of 1024x1024 pixels

#### Scenario: SVG decode error
- **WHEN** librsvg cannot parse or render the SVG
- **THEN** an error string is returned and the file is skipped without crashing

#### Scenario: Premultiplied alpha conversion
- **WHEN** the cairo surface data is read after rendering
- **THEN** each pixel is converted from premultiplied ARGB32 (native byte order: BGRA on little-endian) to straight RGBA
- **AND** fully transparent pixels (alpha=0) are set to all zeros
- **AND** fully opaque pixels (alpha=255) are reordered without division
- **AND** partially transparent pixels are un-premultiplied with rounding

### Requirement: Link-time binding to system libraries

The FFI bindings use `#[link(name = "...")]` for link-time resolution of all system image decoder libraries.

#### Scenario: Build with system libraries
- **WHEN** the project is built
- **THEN** the linker resolves symbols against system .so files for libpng16, libgif, libtiff, librsvg-2, libcairo, libgobject-2.0, and libglib-2.0
- **AND** the resulting binary has dynamic dependencies on these shared libraries
