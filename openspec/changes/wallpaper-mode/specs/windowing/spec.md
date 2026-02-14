## ADDED Requirements

### Requirement: Track wl_output globals

The system SHALL bind `wl_output` objects from the Wayland registry. For each output, the system SHALL record its global name and native resolution from the `wl_output::Event::Mode` event (when the mode has the `current` flag set).

#### Scenario: Output discovery
- **WHEN** the Wayland registry advertises a `wl_output` global
- **THEN** the system binds it and stores its global name

#### Scenario: Output mode event
- **WHEN** a `wl_output` sends a mode event with the current flag
- **THEN** the system records that output's width and height

### Requirement: Bind wlr-layer-shell protocol

The system SHALL bind the `zwlr_layer_shell_v1` global from the Wayland registry when wallpaper mode is active. If the global is not advertised, the system SHALL exit with an error.

#### Scenario: Layer shell available
- **WHEN** the registry advertises `zwlr_layer_shell_v1`
- **THEN** the system binds it for creating layer surfaces

#### Scenario: Layer shell unavailable
- **WHEN** wallpaper mode is active and the registry does not advertise `zwlr_layer_shell_v1`
- **THEN** the application exits with an error message indicating the compositor lacks layer-shell support

### Requirement: Create layer surfaces per output

The system SHALL create one `zwlr_layer_surface_v1` per discovered output. Each layer surface SHALL have its own `wl_surface` and SHM buffer pair. The system SHALL handle `configure` events from the layer surface to learn the output dimensions, and `closed` events to clean up.

#### Scenario: Layer surface configure
- **WHEN** a layer surface sends a configure event with width and height
- **THEN** the system acknowledges the configure, allocates SHM buffers of that size, renders the wallpaper, and presents it

#### Scenario: Layer surface closed
- **WHEN** a layer surface sends a closed event
- **THEN** the system destroys that surface's resources
