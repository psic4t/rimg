## ADDED Requirements

### Requirement: Vim keybinding dispatch

The system SHALL map keyboard input to actions based on the current application mode (Viewer or Gallery). Key mappings SHALL follow vim conventions. Unmapped keys SHALL be ignored silently.

#### Scenario: Same key, different modes
- **WHEN** the user presses `j` in viewer mode
- **THEN** the action is "pan down" (if zoomed)
- **WHEN** the user presses `j` in gallery mode
- **THEN** the action is "move selection down"

#### Scenario: Unmapped key
- **WHEN** the user presses an unmapped key (e.g., `x`)
- **THEN** nothing happens

### Requirement: Mode toggle with Enter

The system SHALL toggle between Viewer and Gallery modes when the user presses Enter. When switching from Gallery to Viewer, the selected image SHALL be displayed. When switching from Viewer to Gallery, the current image SHALL be selected in the grid.

#### Scenario: Viewer to Gallery
- **WHEN** the user presses `Enter` in viewer mode
- **THEN** the application switches to gallery mode with the current image selected

#### Scenario: Gallery to Viewer
- **WHEN** the user presses `Enter` in gallery mode
- **THEN** the application switches to viewer mode showing the selected image

### Requirement: Application quit

The system SHALL quit when the user presses `q` or `Escape`. In gallery mode, `Escape` SHALL return to viewer mode instead of quitting; `q` SHALL always quit.

#### Scenario: Quit with q
- **WHEN** the user presses `q` in any mode
- **THEN** the application exits

#### Scenario: Escape in viewer mode
- **WHEN** the user presses `Escape` in viewer mode
- **THEN** the application exits

#### Scenario: Escape in gallery mode
- **WHEN** the user presses `Escape` in gallery mode
- **THEN** the application returns to viewer mode (does not quit)
