## ADDED Requirements

### Requirement: Wayland window creation

The system SHALL create a native Wayland window using winit. The window SHALL have a default size of 800x600 pixels. The window title SHALL reflect the current image filename. The window SHALL be resizable.

#### Scenario: Open window
- **WHEN** the application starts with valid image arguments
- **THEN** a Wayland window opens at 800x600 with the first image's filename as the title

#### Scenario: Window title updates on navigation
- **WHEN** the user navigates to a different image
- **THEN** the window title updates to the new image's filename

### Requirement: Window resize handling

The system SHALL respond to window resize events by re-rendering the current view at the new dimensions. The pixel buffer SHALL be reallocated to match the new window size. In viewer mode, the image SHALL be re-scaled to fit. In gallery mode, the grid layout SHALL be recalculated.

#### Scenario: Resize in viewer mode
- **WHEN** the window is resized while in viewer mode
- **THEN** the image is re-scaled to fit the new window dimensions

#### Scenario: Resize in gallery mode
- **WHEN** the window is resized while in gallery mode
- **THEN** the thumbnail grid recalculates column count and re-layouts

### Requirement: Event loop with animation support

The system SHALL run a winit event loop that handles keyboard input, window events, and timed redraws for GIF animation. When a GIF is playing, the event loop SHALL use timed wakeups (ControlFlow::WaitUntil) to advance frames. When no animation is active, the loop SHALL wait for events (ControlFlow::Wait).

#### Scenario: Static image displayed
- **WHEN** a static image (non-animated) is the current view
- **THEN** the event loop waits for user input events (no CPU usage when idle)

#### Scenario: Animated GIF playing
- **WHEN** an animated GIF is displayed
- **THEN** the event loop uses timed wakeups to advance frames at the correct intervals

### Requirement: Graceful window close

The system SHALL handle the window close event (compositor close request) by exiting cleanly.

#### Scenario: Close button clicked
- **WHEN** the compositor sends a close request (e.g., window manager close)
- **THEN** the application exits cleanly with status code 0
