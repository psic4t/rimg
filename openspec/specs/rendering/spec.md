## MODIFIED Requirements

### Requirement: Image resize uses bilinear interpolation

Previously used `fast_image_resize` crate with Lanczos3 filter. Now uses manual bilinear interpolation.

#### Scenario: Fit image to window
- **WHEN** an image is rendered in the viewer
- **THEN** it is scaled to fit the window using bilinear interpolation (not Lanczos3)
- **AND** the visual quality is acceptable for screen display

#### Scenario: Zoom and pan
- **WHEN** the user zooms in or out
- **THEN** the image is rescaled using bilinear interpolation at the new zoom factor
- **AND** the pan offset is applied as before

#### Scenario: Gallery thumbnails
- **WHEN** thumbnails are generated for gallery mode
- **THEN** they are created using bilinear resize (not fast_image_resize)
