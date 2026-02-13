## ADDED Requirements

### Requirement: Bilinear image interpolation

Resize RGBA images using bilinear interpolation without external crate dependencies.

#### Scenario: Downscale image to fit window
- **WHEN** an image is larger than the display area
- **THEN** it is resized using bilinear interpolation (weighted average of 4 nearest source pixels)
- **AND** the aspect ratio is preserved
- **AND** the output is an RGBA pixel buffer

#### Scenario: Upscale image with zoom
- **WHEN** the user zooms in beyond 1:1
- **THEN** the image is upscaled using bilinear interpolation
- **AND** the result is smooth (no blocky nearest-neighbor artifacts)

#### Scenario: Identity resize
- **WHEN** source and destination dimensions are equal
- **THEN** the image is returned unchanged (no resize operation performed)

#### Scenario: Thumbnail generation
- **WHEN** gallery mode generates thumbnails
- **THEN** images are resized to fit within the thumbnail cell using the same bilinear resize function
