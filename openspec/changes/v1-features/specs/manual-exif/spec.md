## ADDED Requirements

### Requirement: EXIF tag reading from TIFF files

The system SHALL extract EXIF tags from TIFF files using the existing manual TIFF/IFD parser. Since a TIFF file IS a TIFF structure, the parser SHALL be invoked directly at byte offset 0.

#### Scenario: TIFF with EXIF sub-IFD
- **WHEN** a TIFF file containing an EXIF sub-IFD (tag 0x8769) is opened
- **THEN** IFD0 tags and EXIF sub-IFD tags are extracted
- **AND** the EXIF overlay shows the same tag types as for JPEG (Make, Model, Date, Exposure, etc.)

#### Scenario: TIFF without EXIF data
- **WHEN** a TIFF file has no EXIF sub-IFD pointer in IFD0
- **THEN** only IFD0 tags are extracted (if any match known EXIF tags)
- **AND** the EXIF overlay shows available data or "No EXIF data"

### Requirement: EXIF tag reading from WebP files

The system SHALL extract EXIF tags from WebP files by walking the RIFF container to locate the `EXIF` chunk, then passing the payload to the existing TIFF/IFD parser.

#### Scenario: WebP with EXIF chunk
- **WHEN** a WebP file containing an `EXIF` RIFF chunk is opened
- **THEN** the EXIF chunk payload is located by scanning RIFF chunks
- **AND** the payload (with or without `Exif\0\0` prefix) is parsed as a TIFF structure
- **AND** extracted tags are displayed in the EXIF overlay

#### Scenario: WebP without EXIF chunk
- **WHEN** a WebP file has no `EXIF` RIFF chunk
- **THEN** the EXIF overlay shows "No EXIF data"

### Requirement: EXIF tag reading from PNG files

The system SHALL extract EXIF tags from PNG files by scanning for the `eXIf` chunk, then passing the payload to the existing TIFF/IFD parser.

#### Scenario: PNG with eXIf chunk
- **WHEN** a PNG file containing an `eXIf` chunk is opened
- **THEN** the chunk payload is parsed as a raw TIFF structure (no `Exif\0\0` prefix)
- **AND** extracted tags are displayed in the EXIF overlay

#### Scenario: PNG without eXIf chunk
- **WHEN** a PNG file has no `eXIf` chunk
- **THEN** the EXIF overlay shows "No EXIF data"

### Requirement: EXIF orientation for WebP and PNG

The system SHALL apply EXIF orientation correction when loading WebP and PNG files, matching the existing JPEG behavior.

#### Scenario: WebP with EXIF orientation
- **WHEN** a WebP file has an EXIF chunk containing orientation tag 0x0112
- **THEN** the decoded image is rotated/flipped according to the orientation value before display

#### Scenario: PNG with eXIf orientation
- **WHEN** a PNG file has an eXIf chunk containing orientation tag 0x0112
- **THEN** the decoded image is rotated/flipped according to the orientation value before display
