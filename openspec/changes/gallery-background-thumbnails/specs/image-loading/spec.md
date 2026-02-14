## ADDED Requirements

### Requirement: Thumbnail-optimized image loading

The system SHALL provide a `load_image_thumbnail` function that loads an image and returns a thumbnail-sized RgbaImage. For JPEG files, this function SHALL use turbojpeg DCT scaling to decode at a reduced resolution before resizing to the target thumbnail size. For all other formats (PNG, WebP, GIF, BMP, TIFF, SVG), the function SHALL decode at full resolution and resize.

#### Scenario: JPEG thumbnail with DCT scaling
- **WHEN** a 4000x3000 JPEG is loaded for a 200x200 thumbnail
- **THEN** the JPEG is decoded at a DCT-scaled resolution (e.g., 1/4 = 1000x750) instead of full resolution
- **AND** the decoded image is then resized to fit within 200x200
- **AND** EXIF orientation is applied before the final resize

#### Scenario: JPEG smaller than thumbnail target
- **WHEN** a 150x100 JPEG is loaded for a 200x200 thumbnail
- **THEN** the JPEG is decoded at full resolution (no DCT scaling applied)
- **AND** the result is returned at its native size (no upscaling)

#### Scenario: PNG thumbnail loading
- **WHEN** a PNG file is loaded for a 200x200 thumbnail
- **THEN** the PNG is decoded at full resolution (no format-level scaling available)
- **AND** the decoded image is resized to fit within 200x200

#### Scenario: WebP thumbnail loading
- **WHEN** a WebP file is loaded for a 200x200 thumbnail
- **THEN** the WebP is decoded at full resolution
- **AND** the decoded image is resized to fit within 200x200

#### Scenario: GIF thumbnail loading
- **WHEN** a GIF file is loaded for a 200x200 thumbnail
- **THEN** only the first frame is used
- **AND** the first frame is resized to fit within 200x200

### Requirement: JPEG DCT scaling factor selection

The system SHALL select the optimal DCT scaling factor for JPEG thumbnail generation. The selected factor SHALL be the smallest available factor (from 1/2, 1/4, 1/8) where both scaled dimensions remain >= the target thumbnail size. If no such factor exists, no scaling SHALL be applied (decode at full resolution).

#### Scenario: Large JPEG selects 1/8 scale
- **WHEN** a 4000x3000 JPEG is thumbnailed to 200x200
- **THEN** the 1/8 factor is selected (producing 500x375, both >= 200)

#### Scenario: Medium JPEG selects 1/4 scale
- **WHEN** a 1200x900 JPEG is thumbnailed to 200x200
- **THEN** the 1/4 factor is selected (producing 300x225, both >= 200)
- **AND** 1/8 is rejected because it would produce 150x113 (< 200)

#### Scenario: Small JPEG uses no scaling
- **WHEN** a 300x200 JPEG is thumbnailed to 200x200
- **THEN** no DCT scaling is applied (full resolution decode)
- **AND** 1/2 is rejected because it would produce 150x100 (< 200)
