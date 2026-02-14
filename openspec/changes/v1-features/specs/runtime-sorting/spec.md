## ADDED Requirements

### Requirement: Cycle sort mode at runtime

The system SHALL cycle through sort modes when the user presses `s`. The sort modes SHALL be: Name, Size, EXIF Date, and Modification Date, cycling in that order and wrapping back to Name after Modification Date. After sorting, the currently viewed image SHALL remain selected (the index SHALL be updated to its new position).

#### Scenario: Cycle from Name to Size
- **WHEN** the current sort mode is Name and the user presses `s`
- **THEN** the image list is re-sorted by file size (ascending)
- **AND** the current image remains displayed (its index is updated)

#### Scenario: Cycle from Modification Date back to Name
- **WHEN** the current sort mode is Modification Date and the user presses `s`
- **THEN** the image list is re-sorted by filename (lexicographic ascending)
- **AND** the sort mode wraps back to Name

#### Scenario: Sort by EXIF Date
- **WHEN** the sort mode is set to EXIF Date
- **THEN** JPEG files are sorted by their EXIF DateTimeOriginal tag value
- **AND** files without EXIF date (including non-JPEG formats) fall back to filesystem modification time

#### Scenario: Sort preserves current image
- **WHEN** the user presses `s` while viewing image "photo.jpg"
- **THEN** after re-sorting, "photo.jpg" is still displayed
- **AND** the status bar position indicator (e.g., `[5/42]`) reflects the new position

### Requirement: Toast overlay for sort mode changes

The system SHALL display a brief toast notification when the sort mode changes. The toast SHALL show the new sort mode name (e.g., "Sort: Name"). The toast SHALL auto-dismiss after approximately 1.5 seconds.

#### Scenario: Toast appears on sort change
- **WHEN** the user presses `s` and the sort mode changes to Size
- **THEN** a toast overlay reading "Sort: Size" appears at the top-right of the window

#### Scenario: Toast auto-dismisses
- **WHEN** a sort toast is displayed
- **THEN** it disappears after approximately 1.5 seconds without user interaction

#### Scenario: Rapid sort cycling replaces toast
- **WHEN** the user presses `s` multiple times quickly
- **THEN** each press replaces the previous toast with the new sort mode name
- **AND** the 1.5-second timer resets from the latest press

### Requirement: Sort metadata caching

The system SHALL cache file metadata (size, modification time) and EXIF dates to avoid repeated filesystem or parsing operations when cycling sort modes. The cache SHALL persist for the duration of the session.

#### Scenario: First sort by size
- **WHEN** the user first sorts by Size
- **THEN** file sizes are read from the filesystem and cached

#### Scenario: Subsequent sort by size
- **WHEN** the user sorts by Size again after cycling through other modes
- **THEN** cached sizes are used without re-reading the filesystem
