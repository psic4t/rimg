## ADDED Requirements

### Requirement: Background thumbnail worker thread

The system SHALL generate thumbnails in a dedicated background worker thread, separate from the main UI/rendering thread. The worker SHALL receive thumbnail generation requests via a channel and send completed thumbnails back via a separate channel.

#### Scenario: Worker receives a batch of requests
- **WHEN** the gallery sends a batch of (index, path) pairs to the worker
- **THEN** the worker generates a thumbnail for each item in the batch
- **AND** sends each completed (index, RgbaImage) back to the main thread via channel

#### Scenario: Worker thread lifecycle
- **WHEN** the Gallery is created
- **THEN** a single background worker thread is spawned
- **AND** the thread exits cleanly when the Gallery's sender channel is dropped

#### Scenario: Worker thread panics or disconnects
- **WHEN** the worker thread panics or the channel disconnects
- **THEN** the gallery remains functional with placeholder rectangles for ungenerated thumbnails
- **AND** no crash propagates to the main thread

### Requirement: Non-blocking thumbnail polling

The main event loop SHALL poll for completed thumbnails without blocking. Completed thumbnails SHALL be inserted into the gallery's cache and trigger a redraw.

#### Scenario: Thumbnails arrive while in gallery mode
- **WHEN** the worker completes thumbnail generation for visible items
- **THEN** the main thread receives them via try_recv on the next poll cycle
- **AND** the gallery redraws with the new thumbnails replacing placeholders

#### Scenario: Poll timeout when thumbnails are pending
- **WHEN** the gallery has pending thumbnail requests
- **THEN** the event loop uses a short poll timeout (16ms) instead of blocking indefinitely
- **AND** completed thumbnails appear within one poll cycle of completion

#### Scenario: No pending thumbnails
- **WHEN** all visible thumbnails are cached and no requests are pending
- **THEN** the event loop reverts to its normal poll timeout behavior

### Requirement: Duplicate request prevention

The system SHALL track which thumbnail indices have been sent to the worker but not yet received. Duplicate requests for the same index SHALL NOT be sent.

#### Scenario: Already-pending thumbnail
- **WHEN** a render cycle identifies a missing thumbnail that is already pending
- **THEN** no duplicate request is sent to the worker

#### Scenario: Completed thumbnail clears pending state
- **WHEN** a completed thumbnail is received from the worker
- **THEN** the index is removed from the pending set

### Requirement: New batches on scroll

The system SHALL send new thumbnail generation batches when the visible range changes due to scrolling or navigation. The worker SHALL prioritize the most recently received batch.

#### Scenario: User scrolls to new region
- **WHEN** the user scrolls to reveal new thumbnails not in cache
- **THEN** a new batch of (index, path) pairs is sent to the worker for the newly visible items
- **AND** the worker processes the new batch
