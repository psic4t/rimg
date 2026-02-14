## Context

rimg is a minimal Wayland image viewer using xdg-shell for windowing. It creates a single `xdg_toplevel` surface and renders images via CPU-based software rendering into SHM buffers. The application currently has no awareness of `wl_output` objects or the `wlr-layer-shell` protocol.

To display as a wallpaper, we need to place surfaces on the compositor's background layer using `zwlr_layer_shell_v1`, which is supported by wlroots-based compositors (sway, dwl, hyprland, river, etc.).

## Goals / Non-Goals

**Goals:**
- Display a single image as desktop wallpaper on all connected monitors
- Fill each screen completely (crop, never stretch or letterbox)
- Support multi-monitor with independent per-output surfaces and rendering
- Minimal resource usage — render once per output, then idle

**Non-Goals:**
- Slideshow / image rotation (can be done externally by restarting rimg)
- Animated wallpaper (GIF animation not supported in wallpaper mode)
- X11 support (Wayland-only, wlroots-based compositors only)
- Per-monitor image selection (same image on all outputs)
- Hot-plug monitor detection (new outputs after startup not handled)

## Decisions

### 1. Use wlr-layer-shell-unstable-v1 protocol

The `zwlr_layer_shell_v1` protocol is the standard way for Wayland clients to place surfaces on desktop layers (background, bottom, top, overlay). For wallpaper, we use `layer::background` with all four anchors set and exclusive zone -1 to extend under panels.

**Alternatives considered:**
- ext-layer-shell: Not yet standardized or widely available. wlr-layer-shell has broad compositor support.
- Compositor-specific APIs: Not portable across wlroots compositors.

### 2. Per-output surface architecture

Each `wl_output` gets its own `wl_surface` → `zwlr_layer_surface_v1` → `ShmBuffer` triple. The image is independently scaled and cropped for each output's resolution. This is necessary because monitors may have different resolutions.

**Alternatives considered:**
- Single surface with NULL output: Compositor picks one output. Doesn't support multi-monitor.

### 3. Scale-to-fill (cover) rendering

Use `scale = max(screen_w / img_w, screen_h / img_h)` to ensure the image covers the entire screen. The scaled image is then center-cropped to exact output dimensions. This produces no letterboxing and no stretching — only cropping of overflow.

**Alternatives considered:**
- Scale-to-fit (existing): Would leave bars on sides. User explicitly wants "fill screen".
- Stretch: Distorts aspect ratio. Explicitly excluded.

### 4. Separate wallpaper run loop in App

Wallpaper mode uses a completely separate code path in `app.rs`. The run loop only handles output configure events and close events — no keyboard input, no gallery, no viewer, no status bar. This avoids polluting the existing viewer/gallery logic with wallpaper conditionals.

### 5. Output tracking via wl_output

We bind `wl_output` globals in the registry to discover connected monitors and their dimensions. The `wl_output::Event::Mode` event with `current` flag provides the output's native resolution. Layer surfaces are created after both the layer shell global and outputs are known (after the second roundtrip).

### 6. Vendor the protocol XML

Copy `wlr-layer-shell-unstable-v1.xml` into `protocols/` and use `wayland_scanner` macros to generate Rust bindings, consistent with the existing approach for `xdg-shell.xml`.

## Risks / Trade-offs

- [Compositor support] The `wlr-layer-shell` protocol is only available on wlroots-based compositors. GNOME/KDE will not work. → This is acceptable for the target audience. The app exits with an error if the protocol is unavailable.
- [Hot-plug] New monitors connected after startup won't get a wallpaper surface. → Acceptable for v1. User can restart rimg.
- [Memory] Each output gets a full-resolution SHM buffer (2x for double-buffering). A 4K monitor needs ~66MB. → Acceptable; this is standard for SHM-based rendering.
- [No graceful quit] Wallpaper mode has no keyboard input. Must be terminated via signal. → Standard for wallpaper daemons (swaybg, wbg work the same way).
