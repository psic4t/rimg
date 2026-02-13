## ADDED Requirements

### Requirement: Decode static image formats

The system SHALL decode PNG, JPEG, and WebP image files into RGBA pixel buffers. The system SHALL support all standard variants of each format (progressive JPEG, interlaced PNG, lossy/lossless WebP). Decoding failures SHALL result in the image being skipped with a warning, not a crash.

#### Scenario: Load a JPEG file
- **WHEN** a valid JPEG file path is provided
- **THEN** the system decodes it into an RgbaImage pixel buffer

#### Scenario: Load a PNG file with transparency
- **WHEN** a PNG file with alpha channel is provided
- **THEN** the system decodes it into an RgbaImage preserving the alpha channel

#### Scenario: Load a WebP file
- **WHEN** a valid WebP file is provided
- **THEN** the system decodes it into an RgbaImage pixel buffer

#### Scenario: Corrupt image file
- **WHEN** a file cannot be decoded (corrupt or unsupported variant)
- **THEN** the system skips the image and continues to the next one without crashing

### Requirement: Decode animated GIF

The system SHALL decode animated GIF files into a sequence of frames, each with its associated delay duration. The system SHALL preserve the frame disposal method and compositing order. Single-frame GIFs SHALL be treated as static images.

#### Scenario: Load an animated GIF
- **WHEN** an animated GIF file is provided
- **THEN** the system decodes all frames with their delay durations into a Vec of (RgbaImage, Duration) pairs

#### Scenario: Load a single-frame GIF
- **WHEN** a GIF with only one frame is provided
- **THEN** the system treats it as a static image (no animation timer)

### Requirement: Apply EXIF orientation

The system SHALL read the EXIF orientation tag from JPEG files and apply the corresponding transformation (rotation and/or flip) to the decoded image. Images without EXIF data or without an orientation tag SHALL be displayed as-is.

#### Scenario: JPEG with orientation tag 6 (rotate 90 CW)
- **WHEN** a JPEG file with EXIF orientation value 6 is loaded
- **THEN** the decoded image is rotated 90 degrees clockwise before display

#### Scenario: JPEG with orientation tag 3 (rotate 180)
- **WHEN** a JPEG file with EXIF orientation value 3 is loaded
- **THEN** the decoded image is rotated 180 degrees before display

#### Scenario: Image without EXIF data
- **WHEN** a PNG or WebP file (no EXIF support) is loaded
- **THEN** the image is displayed without any orientation transformation

### Requirement: Collect image paths from CLI arguments

The system SHALL accept file paths and directory paths as CLI arguments. When a directory is provided, the system SHALL recursively scan for files with extensions: jpg, jpeg, png, gif, webp (case-insensitive). The resulting file list SHALL be sorted alphabetically.

#### Scenario: Single file argument
- **WHEN** `rimg photo.jpg` is invoked
- **THEN** the viewer opens with photo.jpg as the only image

#### Scenario: Directory argument
- **WHEN** `rimg ./photos/` is invoked
- **THEN** the system recursively finds all supported image files in ./photos/ and opens the viewer with them sorted alphabetically

#### Scenario: Multiple file arguments
- **WHEN** `rimg a.png b.jpg c.gif` is invoked
- **THEN** the viewer opens with all three images in the given order

#### Scenario: No arguments
- **WHEN** `rimg` is invoked with no arguments
- **THEN** the system prints usage information and exits

#### Scenario: No valid images found
- **WHEN** arguments resolve to zero valid image files
- **THEN** the system prints an error message and exits with a non-zero status
