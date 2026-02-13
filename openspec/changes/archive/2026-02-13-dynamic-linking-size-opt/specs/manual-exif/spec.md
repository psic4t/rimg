## ADDED Requirements

### Requirement: EXIF orientation parsing without external crate

Parse JPEG EXIF data to extract the orientation tag (0x0112) using a minimal manual parser.

#### Scenario: JPEG with EXIF orientation
- **WHEN** a JPEG file contains an APP1 (EXIF) marker with orientation tag
- **THEN** the orientation value (1-8) is extracted
- **AND** the image is rotated/flipped according to the EXIF orientation before display

#### Scenario: JPEG without EXIF data
- **WHEN** a JPEG file has no APP1 marker
- **THEN** no orientation is applied (defaults to normal/1)

#### Scenario: JPEG with EXIF but no orientation tag
- **WHEN** a JPEG file has EXIF data but no orientation tag in IFD0
- **THEN** no orientation is applied (defaults to normal/1)

#### Scenario: Big-endian and little-endian TIFF headers
- **WHEN** the EXIF TIFF header uses "MM" (big-endian) or "II" (little-endian) byte order
- **THEN** all IFD entry values are read in the correct byte order

#### Scenario: Corrupt EXIF data
- **WHEN** the EXIF data is truncated or malformed
- **THEN** the parser returns None and no orientation is applied (no crash)
