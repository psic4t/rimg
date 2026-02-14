## ADDED Requirements

### Requirement: Auto-skip unloadable images

The system SHALL automatically skip images that fail to decode. When an image fails to load, it SHALL be removed from the path list and the viewer SHALL navigate to the next valid image. The status bar position count SHALL reflect the updated list size.

#### Scenario: Navigate to corrupt image
- **WHEN** the user navigates to an image that fails to decode
- **THEN** the failed image is removed from the path list
- **AND** the next valid image in the list is displayed
- **AND** the status bar count decreases by one (e.g., `[3/41]` instead of `[3/42]`)

#### Scenario: Multiple consecutive bad files
- **WHEN** several consecutive images in the list fail to decode
- **THEN** all failed images are removed and the first valid image after them is displayed

#### Scenario: All images fail to load
- **WHEN** every image in the path list fails to decode
- **THEN** the application displays a "No valid images" message
- **AND** the user can quit with `q` or `Escape`

### Requirement: Status bar error feedback

The system SHALL display a brief error message in the status bar when an image is skipped due to a load failure. The message SHALL include the filename. The message SHALL auto-dismiss after approximately 3 seconds.

#### Scenario: Error message displayed
- **WHEN** an image "corrupt.jpg" fails to load and is skipped
- **THEN** the status bar shows a message like "Skipped: corrupt.jpg"
- **AND** the message disappears after approximately 3 seconds

#### Scenario: Error message replaced by navigation
- **WHEN** an error message is displayed and the user navigates to another image
- **THEN** the error message is cleared and the normal status bar for the new image is shown

### Requirement: Gallery handles failed thumbnails

The system SHALL handle thumbnail generation failures gracefully in gallery mode. Failed thumbnails SHALL display the gray placeholder. The failed image path SHALL remain in the list for gallery display but SHALL be removed if the user attempts to open it.

#### Scenario: Thumbnail fails to generate
- **WHEN** a thumbnail cannot be generated for an image in gallery mode
- **THEN** the gray placeholder remains for that cell
- **AND** the cell is still selectable via navigation

#### Scenario: Open failed image from gallery
- **WHEN** the user selects a failed-thumbnail image in gallery and presses Enter
- **THEN** the image load is attempted in viewer mode
- **AND** if it fails, the auto-skip behavior removes it and shows the next valid image
