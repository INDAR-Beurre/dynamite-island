fn upscale_canvas_2x_into(src: &[u8], w: u32, h: u32, dst: &mut [u8]) {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return;
    }
    let (bw, bh) = (2 * w, 2 * h);
    debug_assert_eq!(src.len(), w * h * 4);
    debug_assert_eq!(dst.len(), bw * bh * 4);
    for dy in 0..bh {
        let j = dy / 2;
        let (y0, y1, wy0, wy1) = if dy % 2 == 0 {
            (j.saturating_sub(1), j, 1u32, 3u32)
        } else {
            (j, (j + 1).min(h - 1), 3u32, 1u32)
        };
        for dx in 0..bw {
            let i = dx / 2;
            let (x0, x1, wx0, wx1) = if dx % 2 == 0 {
                (i.saturating_sub(1), i, 1u32, 3u32)
            } else {
                (i, (i + 1).min(w - 1), 3u32, 1u32)
            };
            let di = (dy * bw + dx) * 4;
            for c in 0..4 {
                let s00 = src[(y0 * w + x0) * 4 + c] as u32;
                let s01 = src[(y0 * w + x1) * 4 + c] as u32;
                let s10 = src[(y1 * w + x0) * 4 + c] as u32;
                let s11 = src[(y1 * w + x1) * 4 + c] as u32;
                // Horizontal first (rounded), then vertical (rounded).
                let top = (wx0 * s00 + wx1 * s01 + 2) / 4;
                let bot = (wx0 * s10 + wx1 * s11 + 2) / 4;
                dst[di + c] = ((wy0 * top + wy1 * bot + 2) / 4) as u8;
            }
        }
    }
}

fn upscale_canvas_2x_region(
    src: &[u8],
    w: u32,
    h: u32,
    dst: &mut [u8],
    region: (i32, i32, i32, i32),
) {
    let (wu, hu) = (w as usize, h as usize);
    if wu == 0 || hu == 0 {
        return;
    }
    let r = clip_rect(region, w, h);
    if r.2 <= r.0 || r.3 <= r.1 {
        return;
    }
    let (bw, bh) = (2 * wu, 2 * hu);
    debug_assert_eq!(src.len(), wu * hu * 4);
    debug_assert_eq!(dst.len(), bw * bh * 4);
    let dx0 = (r.0 as usize) * 2;
    let dx1 = (r.2 as usize) * 2;
    let dy0 = (r.1 as usize) * 2;
    let dy1 = (r.3 as usize) * 2;
    for dy in dy0..dy1 {
        let j = dy / 2;
        let (y0, y1, wy0, wy1) = if dy % 2 == 0 {
            (j.saturating_sub(1), j, 1u32, 3u32)
        } else {
            (j, (j + 1).min(hu - 1), 3u32, 1u32)
        };
        for dx in dx0..dx1 {
            let i = dx / 2;
            let (x0, x1, wx0, wx1) = if dx % 2 == 0 {
                (i.saturating_sub(1), i, 1u32, 3u32)
            } else {
                (i, (i + 1).min(wu - 1), 3u32, 1u32)
            };
            let di = (dy * bw + dx) * 4;
            for c in 0..4 {
                let s00 = src[(y0 * wu + x0) * 4 + c] as u32;
                let s01 = src[(y0 * wu + x1) * 4 + c] as u32;
                let s10 = src[(y1 * wu + x0) * 4 + c] as u32;
                let s11 = src[(y1 * wu + x1) * 4 + c] as u32;
                let top = (wx0 * s00 + wx1 * s01 + 2) / 4;
                let bot = (wx0 * s10 + wx1 * s11 + 2) / 4;
                dst[di + c] = ((wy0 * top + wy1 * bot + 2) / 4) as u8;
            }
        }
    }
}

fn clip_rect(
    r: (i32, i32, i32, i32),
    w: u32,
    h: u32,
) -> (i32, i32, i32, i32) {
    let w = w as i32;
    let h = h as i32;
    (
        r.0.clamp(0, w),
        r.1.clamp(0, h),
        r.2.clamp(0, w),
        r.3.clamp(0, h),
    )
}