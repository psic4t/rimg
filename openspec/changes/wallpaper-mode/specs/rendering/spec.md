## ADDED Requirements

### Requirement: Scale-to-fill rendering

The system SHALL provide a scale-to-fill function that scales an image to cover a target area completely. The scale factor SHALL be `max(target_w / img_w, target_h / img_h)`. After scaling, the image SHALL be center-cropped to exactly `target_w x target_h`. The result SHALL have no letterboxing and no aspect ratio distortion.

#### Scenario: Landscape image filling a smaller landscape area
- **WHEN** a 3840x2160 image is scaled to fill 1920x1080
- **THEN** the scale factor is max(1920/3840, 1080/2160) = 0.5, producing 1920x1080 with no cropping needed

#### Scenario: Different aspect ratios requiring crop
- **WHEN** a 1000x500 image is scaled to fill 800x800
- **THEN** the scale factor is max(800/1000, 800/500) = 1.6, producing 1600x800, center-cropped to 800x800

#### Scenario: Image smaller than target
- **WHEN** a 640x480 image is scaled to fill 1920x1080
- **THEN** the scale factor is max(3.0, 2.25) = 3.0, producing 1920x1440, center-cropped to 1920x1080
