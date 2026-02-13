## ADDED Requirements

### Requirement: Pixel buffer presentation

The system SHALL render images to a software pixel buffer and present it to the Wayland compositor via softbuffer. The pixel format SHALL be XRGB (0x00RRGGBB). The buffer SHALL be resized when the window dimensions change.

#### Scenario: Present image to window
- **WHEN** an image frame is ready for display
- **THEN** the system writes XRGB pixels to the softbuffer surface and presents the frame

#### Scenario: Window resize triggers buffer resize
- **WHEN** the window is resized
- **THEN** the pixel buffer is resized to match the new dimensions and the image is re-rendered

### Requirement: Image scaling with quality

The system SHALL use SIMD-accelerated Lanczos3 resampling for all image scaling operations (fit-to-window, zoom, thumbnails). Scaling SHALL maintain the source image's aspect ratio.

#### Scenario: Scale large image to fit window
- **WHEN** a 4000x3000 image is displayed in a 1920x1080 window
- **THEN** the image is scaled down using Lanczos3 filter maintaining aspect ratio

#### Scenario: Generate thumbnail
- **WHEN** a gallery thumbnail is needed for a 4000x3000 image
- **THEN** the image is scaled to fit within 200x200 pixels using Lanczos3 filter

### Requirement: Compositing with dark background

The system SHALL composite images onto a dark background (#1a1a1a). In viewer mode, the image is centered with letterboxing. Images with alpha channels SHALL be composited against the background color (alpha blending).

#### Scenario: Image with transparency
- **WHEN** a PNG with alpha channel is displayed
- **THEN** transparent regions show the dark background color through alpha blending

#### Scenario: Letterbox display
- **WHEN** an image does not fill the entire window
- **THEN** unused areas are filled with #1a1a1a background color

### Requirement: Status bar overlay rendering

The system SHALL render a status bar as a semi-transparent dark strip at the bottom of the window. Text SHALL be rendered using the embedded bitmap font in white color. The bar SHALL overlay the image content.

#### Scenario: Render status bar
- **WHEN** an image is displayed in viewer mode
- **THEN** a semi-transparent dark bar appears at the bottom-left with white text showing image metadata

#### Scenario: Status bar content
- **WHEN** displaying an image named "photo.jpg" that is 1920x1080, 2.4 MB, modified 2025-01-15
- **THEN** the status bar shows text like: `photo.jpg | 1920x1080 | 2.4 MB | 2025-01-15 14:30`
