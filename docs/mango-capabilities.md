# MangoWM capability audit vs Hyprland — for the `dynamite-rust` panel

Machine: Artix Linux, `XDG_CURRENT_DESKTOP=mango`, `WAYLAND_DISPLAY=wayland-0`.
Compositor: MangoWM (MangoWC, dwl/wlroots-based), version **0.18.3 (6f322a3b)** from `mmsg get version`.
Panel: `dynamite-rust`, Rust, smithay-client-toolkit 0.19, `wlr-layer-shell` `Layer::Overlay`, `Anchor::TOP`, `set_exclusive_zone`.

Everything below is evidence-cited unless explicitly marked `[Hyprland knowledge]`.

---

## 1. Config summary (`~/.config/mango/`)

### config.conf — visual/motion settings
- Blur: `blur=1`, `blur_layer=1`, `blur_optimized=1`, `blur_params_num_passes=2`, `blur_params_radius=3`
  (also supported per strings: `blur_params_saturation/brightness/contrast/noise`, `blur_opacity`)
- Shadows: `shadows=1`, `layer_shadows=1`, `shadow_only_floating=1`, `shadows_size=10`, `shadows_blur=14`, `shadows_color=0x00000077`
- `border_radius=8`, `no_radius_when_single=0`
- Opacity: `focused_opacity=1.0`, `unfocused_opacity=0.92`
- Animations: `animations=1`, **`layer_animations=1`**, open/close type `slide`, `layer_animation_type_open/close=slide`, `animation_fade_in/out=1`, durations: move 250 / open 280 / tag 220 / close 220 / focus 250 ms
- Curves: cubic-bezier, e.g. `animation_curve_open=0.46,1.0,0.29,1.1` (overshoot), `animation_curve_close=0.08,0.92,0,1`, etc.
- Gaps: `gap_inner_*`=6, `gap_outer_*`=10; `smart_gaps=0`
- Borders: `border_px=2` + color keys (`focus_color=0x7aa2faff` etc.)
- No `dim_inactive` key in config.conf. Strings show only `dim_enable`, `current_dim_color`, `get_dim_color`, `target_dim_color`, `initial_dim_color` — this is the **idle dimming** engine (dimland integration), not a focus-based dim-inactive feature. **Mango has no `dim_inactive`.**

### rule.conf — layer rules (applied to the panel already)
- `layer_rule=no_blur:1,no_shadow:1,no_animation:1,layer_name:dynamite` — Mango accepts per-layer rules by layer-name.
- Layer rules exist for: `selection` (no_blur, no_animation), `swaync-control-center`, `swaync-notification-window`, `dimland_layer`, `launcher`, `rofi`, `hyprpicker`, `showkeys`, `wofi`, `mangobar`, `mangobar-menu`, `swayosd` — with per-layer `no_blur`, `no_shadow`, `no_animation`, `animation_type_open/close:fade|zoom`.

### monitor.conf
- `monitor_rule=name:eDP-1,width:2256,height:1504,refresh:60,x:0,y:1543,scale:1.5`

### env.conf, tag.conf, bind.conf
- env.conf: Wayland/HiDPI env vars. tag.conf: 9 tags, 1-4 set to `tile` layout.
- bind.conf: `SUPER+CTRL,l → swaylock -f -c 000000` (current lock screen = swaylock).

---

## 2. Capability findings (evidence)

### Globals / protocols advertised
`wayland-info` is **not installed** on this machine (could not enumerate live globals; noted as unavailable). Evidence from `strings /usr/bin/mango` (symbol/protocol-name strings in the wlroots build):
- `zwlr_layer_shell_v1` / `zwlr_layer_surface_v1` (layer shell) — **yes**
- `wlr_fractional_scale_manager_v1` + `wlr_fractional_scale_v1_notify_scale`, `wlr_viewporter` — **fractional-scale-v1 yes**
- `wlr_session_lock_manager_v1`, `wlr_session_lock_surface_v1`, `wlr_session_lock_v1` (+ `handle_session_lock_*`, `session_locked`) — **ext-session-lock-v1 yes** (wlroots impl)
- Also: `ext_foreign_toplevel_list_v1`, `ext_workspace_manager_v1`, `ext_image_copy_capture_v1`, `wlr_screencopy_v1`, `wlr_output_management_v1`, `wlr_gamma_control`, `xdg_activation_v1`, `wlr_virtual_keyboard/pointer`, `wlr_relative_pointer`, `wlr_pointer_constraints`, `single_pixel_buffer`, `tearing_control`, `color_manager`, `cursor_shape`.
- **No** `wp_alpha_modifier`-style per-layer alpha hooks beyond `wlr_alpha_modifier_surface_v1` (present, actually). No xdg-decoration blur protocol (blur is compositor-side only).
- No protocol for compositing a texture *behind* a layer surface (nothing like KWin/ Hyprland's `blur_layers`-over-wallpaper compositing hook exposed to clients). Blur is entirely compositor-rendered from the wall behind the surface.

### IPC (mmsg)
Commands: `get version/cursorpos/keymode/keyboardlayout/last_open_surface/monitor/focusing-client/client/tag/all-clients/all-monitors/all-devices/all-layers/all-tags/layouts/tags`, `dispatch <func>,args`, `watch ...` streams for the same. `mmsg get all` is not a command (`{"error":"unknown command"}`) — use `mmsg get all-monitors` etc.

### Live output query (fractional scale, question (b))
`mmsg get all-monitors` / `mmsg get monitor eDP-1`:
```json
{"name":"eDP-1","width":1504,"height":1002,"scale":1.5, ...}
```
Output logical size 1504x1002 for 2256x1504 at scale **1.5**. Mango reports the fractional scale (1.5) and it implements `wp_fractional_scale_v1`; the panel receives `wl_surface.preferred_buffer_scale` fractional/`wp_fractional_scale_v1.preferred_scale` = 1.5 (=120 in 120ths). Width 1504 = 2256/1.5 — i.e. the surface is scaled, logical coordinates apply, and the layer shell maps into logical space.

### Layer animation support
Config keys `layer_animations`, `layer_animation_type_open/close`, plus layer rules like `layer_rule=animation_type_open:zoom,layer_name:launcher` and string symbols `fadeout_layer_animation_next_tick` prove Mango **does animate layer surfaces** (types: slide, zoom, fade, none; per-layer override by name).

### dim / dim_inactive
No `dim_inactive` key; no focus-dim engine. The dim strings are idle-dim (dimland). **Mango cannot dim inactive clients.**

---

## 3. Direct answers (a)-(e)

**(a) Can the compositor blur behind a layer-shell surface?**
No. Mango's blur is fully compositor-side (`blur=1`, `blur_layer=1`, `blur_params_*`) and there is no protocol or config that lets a client ask for blur of the content behind itself. There is nothing like Hyprland's per-layer `blur_layers` mechanism exposed to the client, and `blur_layer_on_wallpaper` / `layer_blur_uses_wallpaper` strings show Mango can only blur from what it composites (wallpaper behind layer), never a client-driven backdrop. **A translucent panel wanting Hyprland-style "blur behind me" must implement it itself: the panel must sample/capture the wallpaper (or whatever is behind), blur it, and render it as its own background.** Note: with `Layer::Overlay` over a `swaybg` background layer, Mango's compositor-side `blur_layer=1` does *not* apply to the Overlay layer content (blur_layer applies to layer surfaces generally, but the panel's own layer rule disables blur for `dynamite` anyway: `no_blur:1`), so the panel sees raw wallpaper, not a blurred backdrop. If the rule were removed, blur would be compositor-applied behind the panel — but then it is compositor-controlled, unconfigurable from the panel, and would conflict with `no_blur:1` that kills the shadow-band artifact.

**(b) Does Mango support `wp_fractional_scale_v1`, and what scale does it report for eDP-1 @ 1.5?**
Yes — `wlr_fractional_scale_manager_v1` + `wlr_viewporter` in the binary. `mmsg get monitor eDP-1` → `"scale": 1.5`, logical 1504x1002 for the 2256x1504 panel. The panel gets scale 1.5 (preferred_scale 120/120ths) via fractional-scale-v1. Buffer is upscaled; logical coords are 1504x1002.

**(c) Does Mango honour `set_exclusive_zone`?**
Yes. Strings: `WLR_LAYER_SURFACE_V1_STATE_EXCLUSIVE_ZONE`, `EXCLUSIVE_EDGE`, `layer_consume_exclusive`, `reset_exclusive_layers_focus` — the wlroots layer-shell state machine is fully wired, so `set_exclusive_zone` reserves space and pushes tiled clients below the panel.

**(d) Does Mango support `wlr-layer-shell` `keyboard_interactivity` modes?**
Yes. Strings: `WLR_LAYER_SURFACE_V1_STATE_KEYBOARD_INTERACTIVITY`, `set_keyboard_interactivity`, `exclusive_focus`. OnDemand and Exclusive both handled (wlroots layer-shell implementation). For an Overlay panel: `OnDemand` gives focus when the user clicks; `Exclusive` grabs keyboard globally (what a lock screen / launcher wants).

**(e) Lock screen implementation options?**
Three options, best-first:
1. **`ext-session-lock-v1` (native, correct):** `wlr_session_lock_manager_v1`/`wlr_session_lock_surface_v1` symbols prove the protocol is implemented (wlroots 0.18 impl: compositor blanks outputs, destroys all prior lock surfaces, gives focus to the lock surface). A lock client binds `ext_session_lock_manager_v1`, creates a lock, maps a lock surface per output. `swaylock` (already bound to Super+Ctrl+L in bind.conf) uses exactly this. **Recommended for any lock screen the panel needs to implement: use ext-session-lock-v1, not a layer surface.**
2. **Overlay layer surface:** works but wrong — no input grab guarantee, no secure-attention (an app can pop above or steal focus), no locking of session state.
3. There is **no Mango-native lock function** (no `lock` in `mmsg dispatch`; lock is delegated to external clients like swaylock).

---

## 4. Hyprland → Mango gap table

| Effect needed by reference design | Hyprland mechanism `[Hyprland knowledge]` | Mango equivalent | Verdict |
|---|---|---|---|
| Blur behind panel | `decoration:blur` + per-window/layer blur, compositor-rendered backdrop behind translucent surfaces | `blur_layer=1` compositor-side; no client-facing backdrop protocol | **Partially** — compositor can blur behind the layer if `no_blur` rule is removed, but panel cannot drive or customize it; panel-owns-its-blur is the reliable path |
| Blur wallpaper behind panel | Hyprland composites blur of whatever is behind, incl. wallpaper | `blur_layer_on_wallpaper`/`layer_blur_uses_wallpaper` symbols exist; but no per-panel control | **Must be implemented in panel** (capture wallpaper + blur itself) if panel-driven look is required |
| Fractional scale | fractionalscale-v1 native | `wlr_fractional_scale_manager_v1`; reports 1.5 | **Supported** |
| Exclusive zone / reserve space | layer-shell standard | Full wlroots layer-shell state machine | **Supported** |
| Keyboard interactivity | layer-shell standard | on-demand + exclusive handled | **Supported** |
| Lock screen | `ext-session-lock-v1` + `hyprlock` | `wlr_session_lock_manager_v1` implemented; use ext-session-lock-v1 (swaylock bound) | **Supported** (no native hyprlock-equivalent; bring your own lock client) |
| `dim_inactive` | `decoration:inactive_opacity`/`dim_inactive` dims unfocused windows | Only idle-dim (`dim_enable`, dimland); no focus-based dim | **Not supported** |
| Animations | Hyprland animates windows/layers with bezier curves | `animations=1`, `layer_animations=1`, curves as cubic-bezier; types slide/zoom/fade/none | **Supported** (for layer surfaces too) |
| Per-layer rules by name | `layerrule` in hyprland.conf | `layer_rule=...,layer_name:X` (no_blur/no_shadow/no_animation/animation_type) | **Supported** (smaller key set) |
| Shadows on layer surfaces | `drop_shadow` + `shadow_ignore_window` | `layer_shadows=1`, `shadow_only_floating`, per-layer `no_shadow` | **Supported** (fixed params, no per-layer shadow geometry) |
| Rounded corners on layer surface | `decoration:rounding` (applies to windows) | `border_radius=8` global; **no per-layer radius**; layer rules have no `no_radius` equivalent (only window rules do) | **Partially** — global only, no per-layer override |
| Blur per-surface tunables | per-layer blur size/pass/saturation | Global `blur_params_*` only | **Partially** — global only |
| IPC for panel (state, events, watch) | `hyprctl`/`hyprland-qt` + `hyprland` IPC | `mmsg get/watch/dispatch` — richer event streams | **Supported** (different API surface; panel already uses it) |
| Special workspace / overview effects | Hyprland special workspaces, overview | `enteroverview`/`leaveoverview`, hotarea, `toggle_scratchpad` | **Supported** (different semantics) |
| Global shortcuts from a client | `globalshortcut` protocol / `hyprctl dispatch` | `mmsg dispatch` + Mango's own `global_key_binding` window_rule | **Partially** — via window rules and IPC dispatch, not a client-held global-shortcut protocol |

---

## 5. "Do not attempt — the compositor cannot do this"

1. **No `dim_inactive` / focus-based dimming.** Mango only has idle-dim (dimland). Do not chase Hyprland's unfocused-window dimming via compositor config; if needed, implement in-panel (panel can't dim other clients, so this is effectively unavailable).
2. **No client-facing backdrop/blur protocol.** No protocol lets the panel request "blur whatever is behind me". Hyprland-style panel-owned blur must be done by the panel itself (capture the wallpaper with `ext_image_copy_capture_v1` or `wlr_screencopy_v1`, blur in-panel, draw as its own background). Compositor-side `blur_layer=1` is the only compositor blur, and it is unconfigurable per-panel.
3. **No per-layer rounded corners.** `border_radius` is global; layer rules offer only `no_blur/no_shadow/no_animation/animation_type_*`. Per-layer radius (e.g. rounded pill with square edges elsewhere) must be drawn by the panel itself.
4. **No per-layer blur/shadow geometry or color.** `blur_params_*` and `shadows_*` are global; layer rules only toggle them on/off by layer name.
5. **No native lock/secure-input compositing function in the compositor itself.** Locking is delegated to clients (swaylock) via `ext-session-lock-v1`; there is no `mmsg dispatch lock`.
6. **`wayland-info` is not installed** on this machine; live global enumeration wasn't possible. Evidence is from `strings /usr/bin/mango` (protocol symbol names in the build) and `mmsg` queries. If exact live global list is needed later, install `wayland-info` (package `wayland-utils`) and re-run.

## 6. Notes for the reimplementation

- The panel's existing layer rule `layer_rule=no_blur:1,no_shadow:1,no_animation:1,layer_name:dynamite` in rule.conf disables compositor blur/shadow/animation for the panel; the panel's own drawn shadow/blur must account for this (compositor will not add anything behind it).
- eDP-1 reports logical 1504x1002 at scale 1.5; the panel is anchored TOP on the Overlay layer. Fractional-scale-v1 is active, so the panel should consume `wp_fractional_scale_v1.preferred_scale` (1.5 → 120/120ths) and size buffers as `logical_size * scale` (e.g. width 1504 logical = 2256 buffer px).
- The reference's Hyprland-derived behaviours that Mango provides natively: animations, layer rules by name, shadows, exclusive zone, keyboard interactivity, ext-session-lock-v1, fractional scale.
- The reference's Hyprland-derived behaviours that Mango does NOT provide: `dim_inactive`, client-driven blur behind layer surfaces, per-layer radius/blur/shadow tuning.
