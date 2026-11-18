#!/usr/bin/env python3
"""Iteration 29 edits: fake in-buffer blur (a3 retry). Applied on the Surface."""
import sys

def rep(path, old, new, count=1):
    with open(path) as f:
        s = f.read()
    n = s.count(old)
    assert n == count, f"{path}: expected {count}x, found {n}x: {old[:90]!r}"
    with open(path, "w") as f:
        f.write(s.replace(old, new))
    print(f"OK {path}: {old[:60]!r}")

B = "/home/alex/rust-island-build"
MAIN = f"{B}/src/main.rs"
CC = f"{B}/src/controlcenter_panel.rs"
LA = f"{B}/src/launcher_panel.rs"
CARGO = f"{B}/Cargo.toml"

GLASS_FILL_BLUR = '''
/// Premultiplied BGRA bytes for one glass-fill pixel over the FAKE
/// FROST (iter29): `bg` is the blurred wallpaper sample at this pixel's
/// screen-logical coords. The fill is `bg*(1-fa) + TINT*fa` (a dark
/// tint, not black), so on a pure-black backdrop the fill reads
/// TINT*fa ~= 18 and the material never vanishes; the specular is
/// composited over the opaque base. The interior is OPAQUE (A = a*255
/// with `a` the AA coverage) — no sharp wallpaper can leak through the
/// compositor, which is the whole point: the frost lives in-buffer.
/// Falls back to `glass_fill_px` (iter28 black fill) when the blurred
/// wallpaper is unavailable.
fn glass_fill_px_blur(a: f32, d: f32, y: f32, h: f32, fill_alpha: f32, bg: [u8; 3]) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let w_light = glass_edge_light(d, y, h) * a;
    let mut out = [0u8; 4];
    for c in 0..3 {
        let base = bg[c] as f32 * (1.0 - fill_alpha) + GLASS_TINT * fill_alpha;
        let v = base * (1.0 - w_light) + 255.0 * w_light;
        out[c] = (v * a) as u8;
    }
    out[3] = (a * 255.0) as u8;
    out
}
'''

# ---------------- main.rs ----------------
rep(MAIN, "mod wallpaper_panel;\n", "mod wallpaper_panel;\nmod wallpaper_blur;\n")

rep(MAIN, "const GLASS_ALPHA_PILL: f32 = 0.78;", "const GLASS_ALPHA_PILL: f32 = 0.90;")
rep(MAIN, "const GLASS_HIGHLIGHT_PEAK: f32 = 0.38;", "const GLASS_HIGHLIGHT_PEAK: f32 = 0.85;")
rep(MAIN, "const GLASS_HIGHLIGHT_WIDTH: f32 = 4.0;", "const GLASS_HIGHLIGHT_WIDTH: f32 = 9.0;")
rep(MAIN, "const GLASS_GLOW_WIDTH: f32 = 1.5;",
    "const GLASS_GLOW_WIDTH: f32 = 1.5;\n/// Dark tint mixed under the blurred wallpaper (iter29): the fill is\n/// blur*(1-fa) + TINT*fa, so on a pure-black backdrop the fill reads\n/// TINT*fa ~= 18 — the material never vanishes on dark.\nconst GLASS_TINT: f32 = 20.0;")
rep(MAIN, "(hl + gl).min(0.55)", "(hl + gl).min(0.95)")
rep(MAIN,
    "/// through; mango blurs behind the layer (blur=1, blur_layer=1 in\n/// config.conf, and no rule disables blur for namespace\n/// \"dynamite-rust\"), so the bleed is frosted rather than sharp.",
    "/// through; mango does NOT blur behind this layer surface (iter28\n/// A/B: pixel-identical), so iter29 fakes the frost in-buffer: the\n/// blurred wallpaper crop is composited under the fill\n/// (wallpaper_blur.rs) and the fill interior is opaque — no sharp\n/// wallpaper leaks through the compositor.")
rep(MAIN, "/// Fill alphas: pill 0.78, menus 0.85 (task ranges 0.72-0.85 / 0.82-0.88).",
    "/// Fill alphas: pill 0.90, menus 0.85 (iter29: the alpha is now the\n/// tint strength over the blurred crop, not surface translucency).")

# glass_fill_px_blur after glass_fill_px (anchor: its end + next doc line)
rep(MAIN,
    "    let v = (w_light * 255.0) as u8;\n    [v, v, v, (a_out * 255.0) as u8]\n}\n\n/// Layer top margin for the stage: the panel top stays at gap_top.",
    "    let v = (w_light * 255.0) as u8;\n    [v, v, v, (a_out * 255.0) as u8]\n}\n" + GLASS_FILL_BLUR +
    "\n/// Layer top margin for the stage: the panel top stays at gap_top.")

# stage_screen_origin after stage_margin_top
rep(MAIN,
    "fn stage_margin_top() -> i32 {\n    (cfg().gap_top - STAGE_PAD_TOP as i32).max(0)\n}",
    "fn stage_margin_top() -> i32 {\n    (cfg().gap_top - STAGE_PAD_TOP as i32).max(0)\n}\n\n/// Screen-logical coords of the stage top-left: the layer is anchored\n/// TOP and centred horizontally (pill centre 752 = screen centre), so\n/// x0 = (1504 - STAGE_W)/2 = 164 and y0 = the layer top margin (11).\nfn stage_screen_origin() -> (f32, f32) {\n    (\n        (LOCKSCREEN_SIZE.0 as f32 - STAGE_W as f32) / 2.0,\n        stage_margin_top() as f32,\n    )\n}")

# draw_panel_into: new bg_origin param
rep(MAIN,
    "    lock_data: &lockscreen::LockscreenData,\n    lock_wallpaper: Option<&[u8]>,\n) {\n    let (wf, hf) = (w as f32, h as f32);",
    "    lock_data: &lockscreen::LockscreenData,\n    lock_wallpaper: Option<&[u8]>,\n    // Screen-logical coords of this panel canvas's top-left (for the\n    // fake-frost wallpaper sample).\n    bg_origin: (f32, f32),\n) {\n    let (wf, hf) = (w as f32, h as f32);")

# draw_panel_into fill loop: fake frost
rep(MAIN,
    """    {
        for y in 0..h {
            for x in 0..w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                // SDF once; alpha is (0.5 - d) clamped — the same math
                // as rounded_rect_alpha.
                let d = rounded_rect_sdf(px, py, wf, hf, radius);
                let a = (0.5 - d).clamp(0.0, 1.0);
                let i = ((y * w + x) * 4) as usize;
                let p = glass_fill_px(a, d, py, hf, glass_alpha(state));""",
    """    {
        // Fake frost (iter29): blurred wallpaper under the glass fill.
        // None => iter28 black fill (wallpaper unreadable).
        let bg = wallpaper_blur::ensure_loaded();
        for y in 0..h {
            for x in 0..w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                // SDF once; alpha is (0.5 - d) clamped — the same math
                // as rounded_rect_alpha.
                let d = rounded_rect_sdf(px, py, wf, hf, radius);
                let a = (0.5 - d).clamp(0.0, 1.0);
                let i = ((y * w + x) * 4) as usize;
                let p = match &bg {
                    Some(bw) => glass_fill_px_blur(
                        a,
                        d,
                        py,
                        hf,
                        glass_alpha(state),
                        bw.sample(bg_origin.0 + px, bg_origin.1 + py),
                    ),
                    None => glass_fill_px(a, d, py, hf, glass_alpha(state)),
                };""")

# CC / Launcher call sites inside draw_panel_into
rep(MAIN,
    """                &controlcenter_panel::ControlCenterData::live(),
            );""",
    """                &controlcenter_panel::ControlCenterData::live(),
                bg_origin,
            );""")
rep(MAIN,
    """                    data,
                );
            });""",
    """                    data,
                    bg_origin,
                );
            });""")

# draw(): compute origin before the panel render, pass it through
rep(MAIN,
    """    let (pw, ph) = state.size();
    let mut scratch = vec![0u8; (pw * ph * 4) as usize];
    draw_panel_into(
        &mut scratch, pw, ph, state, font, icon_font, lock_data, lock_wallpaper,
    );
    let (ox, oy) = panel_origin(state);""",
    """    let (pw, ph) = state.size();
    let mut scratch = vec![0u8; (pw * ph * 4) as usize];
    let (ox, oy) = panel_origin(state);
    let (sox, soy) = stage_screen_origin();
    draw_panel_into(
        &mut scratch,
        pw,
        ph,
        state,
        font,
        icon_font,
        lock_data,
        lock_wallpaper,
        (sox + ox, soy + oy),
    );""")

# Morph: load blurred bg before the capsule loop
rep(MAIN,
    """    draw_state_into(
        scratch_to,
        w,
        h,
        to_state,
        frame.incoming_scale,
        font,
        icon_to,
        lock_data,
        lock_wallpaper,
    );
    for y in 0..h {""",
    """    draw_state_into(
        scratch_to,
        w,
        h,
        to_state,
        frame.incoming_scale,
        font,
        icon_to,
        lock_data,
        lock_wallpaper,
    );
    // Fake frost (iter29): the capsule base is the blurred wallpaper
    // under the interpolated glass fill (opaque interior — no sharp
    // bleed); endpoint scratches composite over it below.
    let morph_bg = wallpaper_blur::ensure_loaded();
    let (msox, msoy) = stage_screen_origin();
    for y in 0..h {""")

# Morph: blurred base inside the capsule loop (after the a<=0 early-out)
rep(MAIN,
    """            // Glass base alpha (iter28 a3): was opaque (a * 255).
            canvas[i + 3] = (a * morph_glass * 255.0) as u8;
            if a <= 0.0 {
                continue;
            }
            let ao = frame.outgoing_opacity * a;""",
    """            // Glass base alpha (iter28 a3): was opaque (a * 255).
            canvas[i + 3] = (a * morph_glass * 255.0) as u8;
            if a <= 0.0 {
                continue;
            }
            // Fake frost (iter29): blurred wallpaper under the
            // interpolated glass fill replaces the black base.
            if let Some(bw) = &morph_bg {
                let d = rounded_rect_sdf(
                    x as f32 + 0.5 - ox,
                    y as f32 + 0.5 - oy,
                    fw,
                    fh,
                    fr,
                );
                let s = bw.sample(msox + x as f32 + 0.5, msoy + y as f32 + 0.5);
                let p = glass_fill_px_blur(
                    a,
                    d,
                    y as f32 + 0.5 - oy,
                    fh,
                    morph_glass,
                    s,
                );
                canvas[i] = p[0];
                canvas[i + 1] = p[1];
                canvas[i + 2] = p[2];
                canvas[i + 3] = p[3];
            }
            let ao = frame.outgoing_opacity * a;""")

# ---- iter28 test updates for the new constants ----
rep(MAIN, "assert!((glass_alpha(IslandState::Collapsed) - 0.78).abs() < 1e-6);",
    "assert!((glass_alpha(IslandState::Collapsed) - 0.90).abs() < 1e-6);")
rep(MAIN, "assert!((glass_alpha(IslandState::Lock) - 0.78).abs() < 1e-6);",
    "assert!((glass_alpha(IslandState::Lock) - 0.90).abs() < 1e-6);")
rep(MAIN,
    """        let p = glass_fill_px(1.0, -0.5, 0.5, 24.0, GLASS_ALPHA_PILL);
        assert!(p[0] >= 60 && p[0] <= 110, "highlight RGB {} out of band", p[0]);
        assert!(p[3] >= 200 && p[3] <= 230, "pill-top alpha {} out of band", p[3]);""",
    """        let p = glass_fill_px(1.0, -0.5, 0.5, 24.0, GLASS_ALPHA_PILL);
        // iter29: near-white specular (peak 0.85, 9 px falloff) over the
        // 0.90 fill => RGB ~209, A ~244.
        assert!(p[0] >= 195 && p[0] <= 220, "highlight RGB {} out of band", p[0]);
        assert!(p[3] >= 240 && p[3] <= 250, "pill-top alpha {} out of band", p[3]);""")
rep(MAIN,
    """        // AA edge pixel (a=0.5, d=0): no light, alpha = 0.5*0.78*255.
        let p = glass_fill_px(0.5, 0.0, 0.5, 24.0, GLASS_ALPHA_PILL);
        assert_eq!(p, [0, 0, 0, 99]); // 0.39 * 255 = 99.45 -> 99""",
    """        // AA edge pixel (a=0.5, d=0): no light, alpha = 0.5*0.90*255.
        let p = glass_fill_px(0.5, 0.0, 0.5, 24.0, GLASS_ALPHA_PILL);
        assert_eq!(p, [0, 0, 0, 114]); // 0.45 * 255 = 114.75 -> 114""")

# ---- new iter29 tests appended at EOF ----
with open(MAIN, "a") as f:
    f.write('''
#[cfg(test)]
mod iter29_tests {
    use super::*;

    #[test]
    fn glass_fill_px_blur_bright_interior() {
        // Blurred bright wallpaper under the 0.90 pill fill:
        // base = bg*0.10 + 20*0.90 = ~35/channel, opaque interior.
        let p = glass_fill_px_blur(1.0, -10.0, 100.0, 24.0, 0.90, [170, 180, 200]);
        assert_eq!(p[3], 255);
        assert!((p[0] as i32 - 35).abs() <= 2, "R {} want ~35", p[0]);
        assert!((p[1] as i32 - 36).abs() <= 2, "G {} want ~36", p[1]);
        assert!((p[2] as i32 - 38).abs() <= 2, "B {} want ~38", p[2]);
    }

    #[test]
    fn glass_fill_px_blur_dark_floor() {
        // Pure-black backdrop: the fill must NOT vanish — TINT*fa = 18.
        let p = glass_fill_px_blur(1.0, -10.0, 100.0, 24.0, 0.90, [0, 0, 0]);
        assert_eq!(p, [18, 18, 18, 255]);
    }

    #[test]
    fn glass_fill_px_blur_specular_top_near_white() {
        // Top edge of the pill: near-white specular over the tinted base.
        let p = glass_fill_px_blur(1.0, -0.5, 0.5, 24.0, 0.90, [100, 100, 100]);
        assert!(p[0] > 180, "top specular RGB {} too dim", p[0]);
        assert_eq!(p[3], 255);
    }

    #[test]
    fn glass_fill_px_blur_aa_edge_premultiplied() {
        // AA edge pixel: RGB premultiplied by coverage, A = coverage.
        let p = glass_fill_px_blur(0.5, 0.0, 12.0, 24.0, 0.90, [100, 100, 100]);
        assert_eq!(p, [14, 14, 14, 127]); // base 28 * 0.5, A 0.5*255
        assert!(p[0] as u16 <= p[3] as u16, "premultiplied RGB > A");
    }

    #[test]
    fn glass_fill_px_blur_transparent_outside() {
        assert_eq!(
            glass_fill_px_blur(0.0, 0.5, 0.5, 24.0, 0.90, [200, 200, 200]),
            [0, 0, 0, 0]
        );
    }

    #[test]
    fn glass_fill_px_blur_menu_keeps_085() {
        // Menus stay at 0.85: base = bg*0.15 + 20*0.85.
        let p = glass_fill_px_blur(1.0, -10.0, 100.0, 525.0, GLASS_ALPHA_MENU, [200, 200, 200]);
        assert_eq!(p[3], 255);
        assert!((p[0] as i32 - 47).abs() <= 2, "menu R {} want ~47", p[0]);
    }
}
''')
print("OK main.rs: iter29_tests appended")

# ---------------- controlcenter_panel.rs ----------------
rep(CC, "const GLASS_HIGHLIGHT_PEAK: f32 = 0.38;", "const GLASS_HIGHLIGHT_PEAK: f32 = 0.85;")
rep(CC, "const GLASS_HIGHLIGHT_WIDTH: f32 = 4.0;", "const GLASS_HIGHLIGHT_WIDTH: f32 = 9.0;")
rep(CC, "const GLASS_GLOW_WIDTH: f32 = 1.5;",
    "const GLASS_GLOW_WIDTH: f32 = 1.5;\n/// Dark tint mixed under the blurred wallpaper (iter29, see main.rs).\nconst GLASS_TINT: f32 = 20.0;")
rep(CC, "(hl + gl).min(0.55)", "(hl + gl).min(0.95)")
rep(CC,
    """    let w_light = glass_edge_light(d, y, h) * a;
    let fa = a * fill_alpha;
    let a_out = fa + w_light * (1.0 - fa);
    let v = (w_light * 255.0) as u8;
    [v, v, v, (a_out * 255.0) as u8]
}""",
    """    let w_light = glass_edge_light(d, y, h) * a;
    let fa = a * fill_alpha;
    let a_out = fa + w_light * (1.0 - fa);
    let v = (w_light * 255.0) as u8;
    [v, v, v, (a_out * 255.0) as u8]
}

/// Fake-frost fill (iter29): blurred wallpaper under the dark tint —
/// same math as main.rs glass_fill_px_blur (duplicated per-module, as
/// the other glass helpers are in this codebase).
fn glass_fill_px_blur(a: f32, d: f32, y: f32, h: f32, fill_alpha: f32, bg: [u8; 3]) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let w_light = glass_edge_light(d, y, h) * a;
    let mut out = [0u8; 4];
    for c in 0..3 {
        let base = bg[c] as f32 * (1.0 - fill_alpha) + GLASS_TINT * fill_alpha;
        let v = base * (1.0 - w_light) + 255.0 * w_light;
        out[c] = (v * a) as u8;
    }
    out[3] = (a * 255.0) as u8;
    out
}""")
rep(CC,
    """    icon_font: Option<&Font>,
    data: &ControlCenterData,
) {""",
    """    icon_font: Option<&Font>,
    data: &ControlCenterData,
    // Screen-logical coords of this panel canvas's top-left (fake frost).
    bg_origin: (f32, f32),
) {""")
rep(CC,
    """    // --- Panel background: liquid-glass capsule (iter28 a3). ---
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = rounded_rect_sdf(px, py, w as f32, h as f32, CC_RADIUS);
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            let p = glass_fill_px(a, d, py, h as f32, GLASS_ALPHA_MENU);""",
    """    // --- Panel background: liquid-glass capsule (iter28 a3, iter29
    // --- fake frost: blurred wallpaper under the fill). ---
    let bg = crate::wallpaper_blur::ensure_loaded();
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = rounded_rect_sdf(px, py, w as f32, h as f32, CC_RADIUS);
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            let p = match &bg {
                Some(bw) => glass_fill_px_blur(
                    a,
                    d,
                    py,
                    h as f32,
                    GLASS_ALPHA_MENU,
                    bw.sample(bg_origin.0 + px, bg_origin.1 + py),
                ),
                None => glass_fill_px(a, d, py, h as f32, GLASS_ALPHA_MENU),
            };""")

# ---------------- launcher_panel.rs ----------------
rep(LA, "const GLASS_HIGHLIGHT_PEAK: f32 = 0.38;", "const GLASS_HIGHLIGHT_PEAK: f32 = 0.85;")
rep(LA, "const GLASS_HIGHLIGHT_WIDTH: f32 = 4.0;", "const GLASS_HIGHLIGHT_WIDTH: f32 = 9.0;")
rep(LA, "const GLASS_GLOW_WIDTH: f32 = 1.5;",
    "const GLASS_GLOW_WIDTH: f32 = 1.5;\n/// Dark tint mixed under the blurred wallpaper (iter29, see main.rs).\nconst GLASS_TINT: f32 = 20.0;")
rep(LA, "(hl + gl).min(0.55)", "(hl + gl).min(0.95)")
rep(LA,
    """    let w_light = glass_edge_light(d, y, h) * a;
    let fa = a * fill_alpha;
    let a_out = fa + w_light * (1.0 - fa);
    let v = (w_light * 255.0) as u8;
    [v, v, v, (a_out * 255.0) as u8]
}""",
    """    let w_light = glass_edge_light(d, y, h) * a;
    let fa = a * fill_alpha;
    let a_out = fa + w_light * (1.0 - fa);
    let v = (w_light * 255.0) as u8;
    [v, v, v, (a_out * 255.0) as u8]
}

/// Fake-frost fill (iter29): blurred wallpaper under the dark tint —
/// same math as main.rs glass_fill_px_blur (duplicated per-module, as
/// the other glass helpers are in this codebase).
fn glass_fill_px_blur(a: f32, d: f32, y: f32, h: f32, fill_alpha: f32, bg: [u8; 3]) -> [u8; 4] {
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let w_light = glass_edge_light(d, y, h) * a;
    let mut out = [0u8; 4];
    for c in 0..3 {
        let base = bg[c] as f32 * (1.0 - fill_alpha) + GLASS_TINT * fill_alpha;
        let v = base * (1.0 - w_light) + 255.0 * w_light;
        out[c] = (v * a) as u8;
    }
    out[3] = (a * 255.0) as u8;
    out
}""")
rep(LA,
    """    icon_font: Option<&Font>,
    data: &LauncherData,
) {""",
    """    icon_font: Option<&Font>,
    data: &LauncherData,
    // Screen-logical coords of this panel canvas's top-left (fake frost).
    bg_origin: (f32, f32),
) {""")
rep(LA,
    """    // --- Panel background: liquid-glass capsule (iter28 a3). ---
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = rounded_rect_sdf(px, py, w as f32, h as f32, PANEL_RADIUS);
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            let p = glass_fill_px(a, d, py, h as f32, GLASS_ALPHA_MENU);""",
    """    // --- Panel background: liquid-glass capsule (iter28 a3, iter29
    // --- fake frost: blurred wallpaper under the fill). ---
    let bg = crate::wallpaper_blur::ensure_loaded();
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = rounded_rect_sdf(px, py, w as f32, h as f32, PANEL_RADIUS);
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            let p = match &bg {
                Some(bw) => glass_fill_px_blur(
                    a,
                    d,
                    py,
                    h as f32,
                    GLASS_ALPHA_MENU,
                    bw.sample(bg_origin.0 + px, bg_origin.1 + py),
                ),
                None => glass_fill_px(a, d, py, h as f32, GLASS_ALPHA_MENU),
            };""")
rep(LA,
    """pub fn draw_launcher_panel(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
) {
    draw_launcher_panel_with_data(canvas, w, h, font, icon_font, &LauncherData::video_defaults());
}""",
    """pub fn draw_launcher_panel(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    icon_font: Option<&fontdue::Font>,
    bg_origin: (f32, f32),
) {
    draw_launcher_panel_with_data(
        canvas,
        w,
        h,
        font,
        icon_font,
        &LauncherData::video_defaults(),
        bg_origin,
    );
}""")

# ---------------- Cargo.toml ----------------
rep(CARGO, 'libc = "0.2"\n', 'libc = "0.2"\npng = "0.17"\n')
with open(CARGO, "a") as f:
    f.write('''
[patch.crates-io]
# iter29: the sparse-index cache has no `png` metadata (an offline build
# cannot resolve it from the registry), but the png-0.17.16 .crate IS
# cached — vendor it as a path dep. Its deps (bitflags / crc32fast /
# fdeflate / flate2 / miniz_oxide) all have index metadata and resolve
# offline from the cache.
png = { path = "vendor/png-0.17.16" }
''')
print("OK Cargo.toml: png dep + patch")
print("ALL EDITS DONE")
