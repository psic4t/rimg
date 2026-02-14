## Why

rimg currently only runs as a regular xdg_toplevel window. Users want to set a wallpaper on their Wayland desktop using rimg. This requires using the `wlr-layer-shell-unstable-v1` protocol to place an image on the background layer behind all windows, filling each monitor's screen without stretching.

## What Changes

- Add `-w` CLI flag to activate wallpaper mode
- Integrate `wlr-layer-shell-unstable-v1` Wayland protocol for background layer surfaces
- Track `wl_output` objects to support multi-monitor wallpaper (one surface per output)
- Add "scale-to-fill" rendering: scale image to cover the entire screen, center-crop excess (no stretching, no letterboxing)
- In wallpaper mode: no gallery, no keybindings, no status bar, no animation — static image only, first frame used for animated formats
- Process stays running to maintain the wallpaper; terminate with signal (Ctrl+C)

## Capabilities

### New Capabilities

- `wallpaper`: Wallpaper mode using wlr-layer-shell background layer with per-output surfaces and fill scaling

### Modified Capabilities

- `rendering`: Add scale-to-fill (cover) function alongside existing scale-to-fit
- `windowing`: Track wl_output globals, bind wlr-layer-shell protocol, create layer surfaces

## Impact

- New protocol XML file: `protocols/wlr-layer-shell-unstable-v1.xml`
- New protocol bindings in `src/protocols.rs`
- New rendering function in `src/render.rs`
- Significant additions to `src/wayland.rs` for output tracking and layer-shell dispatch
- Changes to `src/main.rs` for CLI flag parsing
- Changes to `src/app.rs` for wallpaper mode run loop
- No changes to existing viewer/gallery behavior — wallpaper mode is a separate code path
