## MODIFIED Requirements

### Requirement: Wayland backend uses system libwayland-client.so

Previously used the pure-Rust wayland-backend wire protocol implementation. Now uses the system `libwayland-client.so` via wayland-backend's `client_system` + `dlopen` features.

#### Scenario: Wayland connection
- **WHEN** the application connects to the Wayland compositor
- **THEN** the connection is established via `libwayland-client.so` (loaded at runtime via dlopen)
- **AND** all Wayland protocol dispatch uses the C library's event loop implementation

#### Scenario: Runtime library missing
- **WHEN** `libwayland-client.so` is not available on the system
- **THEN** the application exits with an error message at startup

### Requirement: Build configuration uses nightly build-std

The release profile uses nightly Rust features to minimize binary size.

#### Scenario: Release build
- **WHEN** `cargo build --release` is run
- **THEN** std is recompiled from source with `panic = "immediate-abort"` and `opt-level = "z"`
- **AND** the resulting binary has no backtrace/unwinding infrastructure
- **AND** the binary is stripped of symbols
