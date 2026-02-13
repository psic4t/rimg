## ADDED Requirements

### Requirement: Thumbnail grid layout

The system SHALL display a grid of image thumbnails. Each thumbnail SHALL be a fixed size (200x200 pixels). The grid SHALL fill the window width with as many columns as fit, with consistent spacing. Rows SHALL extend vertically and be scrollable.

#### Scenario: Window fits 5 columns
- **WHEN** the window is 1100 pixels wide and thumbnails are 200x200 with 10px gaps
- **THEN** the grid displays 5 columns of thumbnails with spacing between them

#### Scenario: Window resize changes columns
- **WHEN** the window is resized from 1100px to 600px wide
- **THEN** the grid recalculates to fewer columns and re-layouts thumbnails

### Requirement: Gallery navigation with vim keys

The system SHALL allow navigating the thumbnail grid using vim keybindings. The selected thumbnail SHALL be visually highlighted with a distinct border. Navigation SHALL wrap within rows and scroll vertically when the selection moves off-screen.

#### Scenario: Move selection right
- **WHEN** the user presses `l` in gallery mode
- **THEN** the selection moves to the next thumbnail (wrapping to the next row if at end of row)

#### Scenario: Move selection down
- **WHEN** the user presses `j` in gallery mode
- **THEN** the selection moves down one row (same column position)

#### Scenario: Move to first thumbnail
- **WHEN** the user presses `g` in gallery mode
- **THEN** the selection jumps to the first thumbnail and scrolls to top

#### Scenario: Move to last thumbnail
- **WHEN** the user presses `G` in gallery mode
- **THEN** the selection jumps to the last thumbnail and scrolls to make it visible

#### Scenario: Auto-scroll on selection
- **WHEN** the selection moves to a thumbnail that is not visible in the current scroll position
- **THEN** the gallery scrolls vertically to make the selected thumbnail visible

### Requirement: Open image from gallery

The system SHALL open the selected thumbnail in single-image viewer mode when the user presses Enter.

#### Scenario: Open selected image
- **WHEN** the user presses `Enter` on a selected thumbnail in gallery mode
- **THEN** the application switches to viewer mode showing the selected image at fit-to-window

### Requirement: Lazy thumbnail generation

The system SHALL generate thumbnails lazily — only when they are about to become visible. Generated thumbnails SHALL be cached in memory for the duration of the session. Thumbnails not yet generated SHALL display a placeholder (solid dark rectangle).

#### Scenario: First gallery open
- **WHEN** the user opens gallery mode with 100 images
- **THEN** only thumbnails visible in the viewport (plus a small buffer) are generated

#### Scenario: Scroll reveals new thumbnails
- **WHEN** the user scrolls down revealing previously unseen thumbnails
- **THEN** those thumbnails are generated and cached, replacing the placeholders

#### Scenario: Return to previously viewed thumbnails
- **WHEN** the user scrolls back to previously viewed thumbnails
- **THEN** the cached thumbnails are displayed immediately without re-generation
