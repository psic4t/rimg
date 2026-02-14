## MODIFIED Requirements

### Requirement: Lazy thumbnail generation

The system SHALL generate thumbnails lazily — only when they are about to become visible. Generated thumbnails SHALL be cached in memory for the duration of the session. Thumbnails not yet generated SHALL display a placeholder (solid dark rectangle). Thumbnail generation SHALL occur in a background thread and SHALL NOT block the gallery render path.

#### Scenario: First gallery open
- **WHEN** the user opens gallery mode with 100 images
- **THEN** the gallery grid is displayed immediately with placeholder rectangles
- **AND** thumbnail generation requests are dispatched to the background worker for visible items plus a buffer zone
- **AND** placeholders are progressively replaced with real thumbnails as they complete

#### Scenario: Scroll reveals new thumbnails
- **WHEN** the user scrolls down revealing previously unseen thumbnails
- **THEN** placeholders are displayed immediately for the new items
- **AND** thumbnail generation requests are dispatched to the background worker
- **AND** those thumbnails replace placeholders as they complete

#### Scenario: Return to previously viewed thumbnails
- **WHEN** the user scrolls back to previously viewed thumbnails
- **THEN** the cached thumbnails are displayed immediately without re-generation
