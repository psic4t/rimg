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

### Requirement: Error handling uses Result<T, String>

Previously used `anyhow::Result`. Now uses `Result<T, String>`.

#### Scenario: Image load error
- **WHEN** an image file cannot be decoded
- **THEN** an error `String` is returned describing the failure
- **AND** the caller prints a warning and skips the file
