## MODIFIED Requirements

### Requirement: BMP loading uses manual parser

Decode BMP images using a hand-written binary parser (no external library).

#### Scenario: Load 24-bit or 32-bit BMP
- **WHEN** a 24-bit or 32-bit BMP file is loaded
- **THEN** the BMP header is parsed manually
- **AND** pixel rows are read (handling bottom-up row order and row padding)
- **AND** the result is `LoadedImage::Static`

#### Scenario: Load 8-bit indexed BMP
- **WHEN** an 8-bit uncompressed BMP file is loaded
- **THEN** the BMP header and DIB header are parsed to extract `biClrUsed` and `biCompression`
- **AND** the color table is read (up to 256 BGRA entries)
- **AND** each pixel byte is used as an index into the color table
- **AND** the result is `LoadedImage::Static`

#### Scenario: Load 4-bit indexed BMP
- **WHEN** a 4-bit uncompressed BMP file is loaded
- **THEN** the color table is read (up to 16 BGRA entries)
- **AND** each byte is unpacked into two pixel indices (high nibble = left pixel, low nibble = right pixel)
- **AND** each index maps to a color table entry
- **AND** the result is `LoadedImage::Static`

#### Scenario: Load 1-bit indexed BMP
- **WHEN** a 1-bit uncompressed BMP file is loaded
- **THEN** the color table is read (2 BGRA entries)
- **AND** each byte is unpacked into 8 pixel indices (MSB = leftmost pixel)
- **AND** each index maps to a color table entry
- **AND** the result is `LoadedImage::Static`

#### Scenario: Reject RLE-compressed BMP
- **WHEN** a BMP file with BI_RLE4 or BI_RLE8 compression is loaded
- **THEN** an error string is returned indicating unsupported compression

#### Scenario: Invalid color table
- **WHEN** a BMP file has a `biClrUsed` value exceeding `2^bits_per_pixel` or a truncated color table
- **THEN** an error string is returned describing the invalid color table

## MODIFIED Requirements

### Requirement: WebP loading supports animation

Decode WebP images using system libwebp. Static WebP is decoded with `WebPDecodeRGBA`. Animated WebP is decoded using the `WebPAnimDecoder` API from libwebpdemux.

#### Scenario: Load static WebP
- **WHEN** a static WebP file is loaded
- **THEN** decoding is performed by `WebPDecodeRGBA` from libwebp
- **AND** the result is `LoadedImage::Static`

#### Scenario: Load animated WebP
- **WHEN** an animated WebP file is loaded
- **THEN** animation is detected via `WebPGetFeatures` (checking `has_animation`)
- **AND** frames are decoded using `WebPAnimDecoderNew` + `WebPAnimDecoderGetNext`
- **AND** frame durations are computed from cumulative timestamp deltas (minimum 10ms)
- **AND** the result is `LoadedImage::Animated` with frame data and durations

#### Scenario: Single-frame animated WebP
- **WHEN** an animated WebP with exactly one frame is loaded
- **THEN** the result is `LoadedImage::Static` (not Animated)

#### Scenario: WebP decode error
- **WHEN** libwebp cannot decode the file
- **THEN** an error string is returned and the file is skipped
