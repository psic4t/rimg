## ADDED Requirements

### Requirement: Unit tests for pure-Rust components

The project SHALL include unit tests for all pure-Rust logic components. Tests SHALL be placed as `#[cfg(test)] mod tests` within each source file. Tests SHALL not require any C libraries, Wayland compositor, or external resources to run.

#### Scenario: Run test suite
- **WHEN** `cargo test` is executed
- **THEN** all unit tests pass
- **AND** no C library linking is required for the pure-Rust tests

### Requirement: BMP parser tests

The test suite SHALL verify BMP decoding for all supported bit depths using synthetic byte arrays constructed in test code.

#### Scenario: Parse 24-bit BMP
- **WHEN** a synthetic 24-bit BMP byte array is decoded
- **THEN** the correct RGBA pixel values are returned

#### Scenario: Parse 8-bit indexed BMP
- **WHEN** a synthetic 8-bit BMP with a 256-entry color table is decoded
- **THEN** pixel indices are mapped to the correct color table entries

#### Scenario: Parse 4-bit indexed BMP
- **WHEN** a synthetic 4-bit BMP is decoded
- **THEN** high and low nibbles are correctly unpacked as separate pixel indices

#### Scenario: Parse 1-bit indexed BMP
- **WHEN** a synthetic 1-bit BMP is decoded
- **THEN** individual bits are correctly unpacked as pixel indices (MSB = leftmost)

#### Scenario: Reject unsupported compression
- **WHEN** a BMP with BI_RLE8 or BI_RLE4 compression is encountered
- **THEN** an error is returned indicating unsupported compression

### Requirement: EXIF parser tests

The test suite SHALL verify EXIF tag extraction using crafted TIFF/EXIF byte sequences.

#### Scenario: Parse little-endian EXIF
- **WHEN** a crafted EXIF segment with "II" byte order is parsed
- **THEN** IFD tags are read in little-endian order and values are correct

#### Scenario: Parse big-endian EXIF
- **WHEN** a crafted EXIF segment with "MM" byte order is parsed
- **THEN** IFD tags are read in big-endian order and values are correct

#### Scenario: Parse orientation tag
- **WHEN** an EXIF segment containing orientation tag 0x0112 with value 6 is parsed
- **THEN** orientation value 6 is returned

### Requirement: Image transform tests

The test suite SHALL verify rotation and flip transforms on small pixel buffers.

#### Scenario: Rotate 90 degrees
- **WHEN** a 2x3 RGBA image is rotated 90 degrees clockwise
- **THEN** the result is a 3x2 image with correctly transposed pixels

#### Scenario: Flip horizontal
- **WHEN** a 2x2 RGBA image is flipped horizontally
- **THEN** left and right pixel columns are swapped

### Requirement: Status formatting tests

The test suite SHALL verify human-readable formatting functions.

#### Scenario: Format file size
- **WHEN** `format_file_size` is called with 1536 bytes
- **THEN** it returns "1.5 KB"

#### Scenario: Format date from days
- **WHEN** `days_to_date` is called with a known epoch day count
- **THEN** the correct year-month-day string is returned

### Requirement: Input mapping tests

The test suite SHALL verify that key-to-action mappings are correct for each mode.

#### Scenario: Viewer mode keys
- **WHEN** `map_key` is called with keysym for `q` in Viewer mode
- **THEN** `Action::Quit` is returned

#### Scenario: Gallery mode keys
- **WHEN** `map_key` is called with keysym for `j` in Gallery mode
- **THEN** `Action::MoveDown` is returned

### Requirement: Gallery navigation tests

The test suite SHALL verify gallery grid navigation index arithmetic.

#### Scenario: Move right at end of row
- **WHEN** selection is at the last column and `move_right` is called
- **THEN** selection wraps to the first column of the next row (or stays if at last item)

#### Scenario: Move down past last row
- **WHEN** selection is in the last row and `move_down` is called
- **THEN** selection stays at the current position (does not wrap or crash)
