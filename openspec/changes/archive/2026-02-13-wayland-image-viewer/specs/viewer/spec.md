## ADDED Requirements

### Requirement: Display image fit-to-window

The system SHALL display the current image scaled to fit within the window while maintaining aspect ratio. The image SHALL be centered. Unused space SHALL be filled with a dark background color (#1a1a1a). When the window is resized, the image SHALL be re-scaled to fit the new dimensions.

#### Scenario: Landscape image in landscape window
- **WHEN** a 1920x1080 image is displayed in a 960x720 window
- **THEN** the image is scaled to 960x540 and centered vertically with dark bars above and below

#### Scenario: Window resize
- **WHEN** the window is resized while displaying an image
- **THEN** the image is re-scaled to fit the new window dimensions and re-centered

### Requirement: Zoom in and out

The system SHALL support zooming in and out of the current image. Zoom levels SHALL increase/decrease by a factor (e.g., 1.25x per step). Zoom SHALL be centered on the current view center. The zoom level SHALL reset to fit-to-window when explicitly requested.

#### Scenario: Zoom in
- **WHEN** the user presses `+` or `=`
- **THEN** the image zoom increases by one step, centered on the current view

#### Scenario: Zoom out
- **WHEN** the user presses `-`
- **THEN** the image zoom decreases by one step (minimum: fit-to-window)

#### Scenario: Reset zoom
- **WHEN** the user presses `0`
- **THEN** the zoom level resets to fit-to-window

### Requirement: Pan when zoomed

The system SHALL allow panning the image when zoomed beyond fit-to-window. Panning SHALL be constrained so the image edges cannot be panned past the window edges (no empty space beyond the image). Panning SHALL have no effect when at fit-to-window zoom level.

#### Scenario: Pan right while zoomed
- **WHEN** the image is zoomed in and the user presses `l`
- **THEN** the viewport shifts right, revealing more of the image's right side

#### Scenario: Pan at fit-to-window
- **WHEN** the image is at fit-to-window zoom and the user presses `h`
- **THEN** nothing happens (no panning possible)

### Requirement: Navigate between images

The system SHALL allow navigating to the next and previous image in the list. Navigation SHALL wrap around (last image → first, first → last).

#### Scenario: Next image
- **WHEN** the user presses `n` or `Space`
- **THEN** the viewer displays the next image in the list and resets zoom to fit-to-window

#### Scenario: Previous image
- **WHEN** the user presses `p` or `Backspace`
- **THEN** the viewer displays the previous image in the list and resets zoom to fit-to-window

#### Scenario: First image
- **WHEN** the user presses `g`
- **THEN** the viewer displays the first image in the list

#### Scenario: Last image
- **WHEN** the user presses `G`
- **THEN** the viewer displays the last image in the list

#### Scenario: Wrap around at end
- **WHEN** the user is on the last image and presses `n`
- **THEN** the viewer wraps to the first image

### Requirement: Animate GIF playback

The system SHALL automatically play animated GIFs at the correct frame timing. Animation SHALL start immediately when the image is displayed. The animation SHALL loop continuously.

#### Scenario: Display animated GIF
- **WHEN** an animated GIF is the current image
- **THEN** frames advance at their specified delay intervals, looping back to frame 0 after the last frame

#### Scenario: Navigate away from animated GIF
- **WHEN** the user navigates to a different image while a GIF is animating
- **THEN** the animation timer stops and the new image is displayed

#### Scenario: Return to animated GIF
- **WHEN** the user navigates back to an animated GIF
- **THEN** animation restarts from frame 0
