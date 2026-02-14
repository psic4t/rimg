## 1. Protocol Setup

- [x] 1.1 Copy `wlr-layer-shell-unstable-v1.xml` into `protocols/` directory
- [x] 1.2 Add `wlr_layer_shell` module to `src/protocols.rs` with `wayland_scanner` macros for code generation

## 2. Rendering

- [x] 2.1 Add `scale_to_fill` function in `src/render.rs` that scales an image to cover target dimensions (scale = max of width/height ratios), then center-crops to exact target size, returning an `RgbaImage`

## 3. Wayland Output and Layer Shell

- [x] 3.1 Add `wl_output` tracking to `WaylandState`: bind outputs from registry, store global name and resolution from mode events, implement `Dispatch<wl_output::WlOutput>`
- [x] 3.2 Add `zwlr_layer_shell_v1` binding: bind the global from registry (conditional on wallpaper mode flag), add `Dispatch` implementation
- [x] 3.3 Add per-output wallpaper surface struct: each output gets its own `wl_surface`, `ShmBuffer`, and `zwlr_layer_surface_v1` with background layer, all-edges anchor, exclusive zone -1, no keyboard interactivity
- [x] 3.4 Implement `Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>`: handle configure events (ack + emit event with output dimensions) and closed events

## 4. CLI and App Integration

- [x] 4.1 Parse `-w` flag in `src/main.rs`, separate it from image path args, pass wallpaper mode to `App`
- [x] 4.2 Add wallpaper mode run loop in `src/app.rs`: load image, create layer surfaces per output after roundtrips, render with `scale_to_fill` on each output's configure, present to each output's surface, block indefinitely with no input handling
