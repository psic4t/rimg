## MODIFIED Requirements

### Requirement: PNG loading uses system libpng16

Previously used the pure-Rust `png` crate. Now uses FFI to system `libpng16.so`.

#### Scenario: Load PNG file
- **WHEN** a PNG file is loaded
- **THEN** decoding is performed by `libpng16.so` via FFI (not the Rust png crate)
- **AND** the result is the same `RgbaImage` struct as before

### Requirement: GIF loading uses system libgif

Previously used the pure-Rust `gif` crate. Now uses FFI to system `libgif.so`.

#### Scenario: Load static GIF
- **WHEN** a single-frame GIF is loaded
- **THEN** decoding is performed by `libgif.so` via FFI
- **AND** the result is `LoadedImage::Static`

#### Scenario: Load animated GIF
- **WHEN** a multi-frame GIF is loaded
- **THEN** all frames with timing are decoded by `libgif.so` via FFI
- **AND** the result is `LoadedImage::Animated` with frame data and durations

### Requirement: EXIF orientation uses manual parser

Previously used the `kamadak-exif` crate. Now uses a manual parser.

#### Scenario: JPEG with orientation
- **WHEN** a JPEG is loaded
- **THEN** EXIF orientation is read by the manual parser (not kamadak-exif)
- **AND** the same rotate/flip transforms are applied as before

### Requirement: SVG loading uses system librsvg + cairo

Decode and rasterize SVG images by calling the system librsvg-2 and cairo shared libraries through FFI.

#### Scenario: Load SVG with intrinsic dimensions
- **WHEN** an SVG file with `width` and `height` attributes is loaded
- **THEN** it is rasterized at its intrinsic pixel dimensions using librsvg + cairo
- **AND** the DPI is set to 96 for unit conversion
- **AND** cairo's premultiplied ARGB32 output is converted to straight RGBA
- **AND** the result is `LoadedImage::Static`

#### Scenario: Load SVG without intrinsic dimensions
- **WHEN** an SVG file has only a `viewBox` or percentage-based dimensions
- **THEN** it is rasterized at a default size of 1024x1024 pixels
- **AND** the result is `LoadedImage::Static`

#### Scenario: SVG load error
- **WHEN** an SVG file cannot be parsed or rendered
- **THEN** an error string is returned describing the failure
- **AND** the caller prints a warning and skips the file

### Requirement: TIFF loading uses system libtiff

Decode TIFF images by calling the system libtiff shared library through FFI.

#### Scenario: Load TIFF file
- **WHEN** a TIFF file is loaded
- **THEN** decoding is performed by `libtiff.so` via FFI
- **AND** libtiff's ABGR packed u32 output is converted to RGBA bytes
- **AND** the result is `LoadedImage::Static`

### Requirement: BMP loading uses manual parser

Decode BMP images using a hand-written binary parser (no external library).

#### Scenario: Load 24-bit or 32-bit BMP
- **WHEN** a 24-bit or 32-bit BMP file is loaded
- **THEN** the BMP header is parsed manually
- **AND** pixel rows are read (handling bottom-up row order and row padding)
- **AND** the result is `LoadedImage::Static`

#### Scenario: Unsupported BMP bit depth
- **WHEN** a 1-bit, 4-bit, or 8-bit BMP file is loaded
- **THEN** an error string is returned indicating the unsupported bit depth

### Requirement: Error handling uses Result<T, String>

Previously used `anyhow::Result`. Now uses `Result<T, String>`.

#### Scenario: Image load error
- **WHEN** an image file cannot be decoded
- **THEN** an error `String` is returned describing the failure
- **AND** the caller prints a warning and skips the file
