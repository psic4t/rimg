## ADDED Requirements

### Requirement: Wallpaper mode activation via -w flag

The system SHALL support a `-w` CLI flag that activates wallpaper mode. In wallpaper mode, the application SHALL display the first provided image as a desktop wallpaper using the `zwlr_layer_shell_v1` protocol on the background layer. Only the first image path SHALL be used; additional paths SHALL be ignored.

#### Scenario: Activate wallpaper mode
- **WHEN** the user runs `rimg -w image.jpg`
- **THEN** the application enters wallpaper mode and displays image.jpg as the desktop background

#### Scenario: Multiple images with -w flag
- **WHEN** the user runs `rimg -w img1.jpg img2.jpg`
- **THEN** only img1.jpg is used as the wallpaper; img2.jpg is ignored

#### Scenario: Missing wlr-layer-shell protocol
- **WHEN** wallpaper mode is activated but the compositor does not support `zwlr_layer_shell_v1`
- **THEN** the application exits with an error message

### Requirement: Wallpaper fills screen without stretching

The system SHALL scale the wallpaper image to fill the entire screen using a cover strategy: scale = max(screen_w / img_w, screen_h / img_h). The scaled image SHALL be center-cropped to the exact output dimensions. The image SHALL NOT be stretched or letterboxed.

#### Scenario: Wide image on square-ish monitor
- **WHEN** a 3840x2160 image is displayed on a 1920x1200 output
- **THEN** the image is scaled to 1920x1080 (by width), then since 1080 < 1200, it is instead scaled by height to 2133x1200, and center-cropped horizontally to 1920x1200

#### Scenario: Tall image on wide monitor
- **WHEN** a 1080x1920 image is displayed on a 1920x1080 output
- **THEN** the image is scaled to 1920x3413 (by width) and center-cropped vertically to 1920x1080

#### Scenario: Image exactly matches output
- **WHEN** a 1920x1080 image is displayed on a 1920x1080 output
- **THEN** the image is displayed at native resolution with no scaling or cropping

### Requirement: Multi-monitor wallpaper

The system SHALL create a separate wallpaper surface for each connected `wl_output`. Each surface SHALL independently scale and crop the image to fit that output's resolution. All outputs SHALL display the same source image.

#### Scenario: Two monitors with different resolutions
- **WHEN** the system has a 3840x2160 output and a 1920x1080 output
- **THEN** each output gets its own layer surface with the image independently scaled and cropped to its resolution

#### Scenario: Single monitor
- **WHEN** only one output is connected
- **THEN** one layer surface is created for that output

### Requirement: Wallpaper layer surface configuration

Each wallpaper layer surface SHALL be configured as follows: layer background (value 0), anchored to all four edges (top, bottom, left, right), exclusive zone -1, keyboard interactivity none, size 0x0 (compositor assigns output dimensions via configure event).

#### Scenario: Layer surface properties
- **WHEN** a wallpaper layer surface is created
- **THEN** it is on the background layer, anchored to all edges, with exclusive zone -1 and no keyboard interactivity

### Requirement: Wallpaper mode disables viewer features

In wallpaper mode, the system SHALL NOT display a status bar, accept keyboard input, support gallery mode, or animate GIF frames. Only the first frame of animated images SHALL be used.

#### Scenario: Animated GIF as wallpaper
- **WHEN** an animated GIF is set as wallpaper
- **THEN** only the first frame is displayed statically

#### Scenario: No keyboard interaction
- **WHEN** wallpaper mode is active
- **THEN** no keyboard events are processed; the process is terminated via signal (e.g., SIGTERM, SIGINT)
