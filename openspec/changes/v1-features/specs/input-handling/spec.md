## ADDED Requirements

### Requirement: Sort cycling keybind

The system SHALL map the `s` key to cycle through sort modes in both Viewer and Gallery modes.

#### Scenario: Press s in viewer mode
- **WHEN** the user presses `s` in viewer mode
- **THEN** the sort mode advances to the next mode in the cycle
- **AND** the image list is re-sorted
- **AND** the currently viewed image remains displayed

#### Scenario: Press s in gallery mode
- **WHEN** the user presses `s` in gallery mode
- **THEN** the sort mode advances to the next mode in the cycle
- **AND** the image list is re-sorted
- **AND** the gallery selection updates to the same image's new position
