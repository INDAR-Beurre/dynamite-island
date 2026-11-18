# dynamite-rust — STATUS
v0.1 prototype: collapsed clock pill only, architected for all 9 states.

## Design choices
- Compositor client: smithay-client-toolkit (layer-shell + shm SlotPool + calloop WaylandSource)
- Text: fontdue (light) over cosmic-text; font = Quicksand variable TTF (default instance)
- Raster: manual SDF capsule + glyph coverage into wl_shm Argb8888 — no tiny-skia, no GPU
- Redraws: configure / minute-boundary timer / control-socket state change only; blocks in epoll otherwise
- Control: unix datagram $XDG_RUNTIME_DIR/dynamite-rust.sock; second invocation = remote
- Namespace: dynamite-rust, layer overlay, anchor TOP, margin-top 15, collapsed 90x24, radius 12

## Log
- (build pending)
