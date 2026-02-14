## MODIFIED Requirements

### Requirement: Animate GIF playback

The system SHALL automatically play animated images (GIF and WebP) at the correct frame timing. Animation SHALL start immediately when the image is displayed. The animation SHALL loop continuously.

#### Scenario: Display animated GIF
- **WHEN** an animated GIF is the current image
- **THEN** frames advance at their specified delay intervals, looping back to frame 0 after the last frame

#### Scenario: Display animated WebP
- **WHEN** an animated WebP is the current image
- **THEN** frames advance at their specified delay intervals, looping back to frame 0 after the last frame

#### Scenario: Navigate away from animated image
- **WHEN** the user navigates to a different image while an animation is playing
- **THEN** the animation timer stops and the new image is displayed

#### Scenario: Return to animated image
- **WHEN** the user navigates back to an animated image
- **THEN** animation restarts from frame 0

## ADDED Requirements

### Requirement: Toast overlay display

The system SHALL support displaying a toast notification overlay for transient messages. The toast SHALL appear at the top-right corner of the window as a small rounded rectangle with text. The toast SHALL auto-dismiss after its specified duration.

#### Scenario: Toast appears
- **WHEN** a toast message is triggered (e.g., sort mode change)
- **THEN** a rounded semi-transparent overlay with the message text appears at the top-right corner

#### Scenario: Toast auto-dismisses
- **WHEN** a toast's display duration elapses
- **THEN** the toast disappears and the window is redrawn without it

#### Scenario: Toast replaced by newer toast
- **WHEN** a new toast is triggered while a previous toast is still visible
- **THEN** the previous toast is replaced by the new one
- **AND** the dismiss timer resets
