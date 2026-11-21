//! A compact recording capsule containing only seven live level bars.
//! The existing datagram producer owns recording; this module only paints.

use std::time::Instant;

pub const SURF_W: u32 = 74;
pub const SURF_H: u32 = 24;

pub struct Recording {
    pub bars: [f32; 27],
    updated: Option<Instant>,
}

impl Recording {
    pub const fn new() -> Self {
        Self {
            bars: [0.0; 27],
            updated: None,
        }
    }

    pub fn start(&mut self) {
        self.bars.fill(0.0);
        self.updated = None;
    }

    pub fn update(&mut self, values: [f32; 27]) {
        let now = Instant::now();
        let dt = self
            .updated
            .map_or(0.03, |last| now.duration_since(last).as_secs_f32());
        for (shown, value) in self.bars.iter_mut().zip(values) {
            let value = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Fast attack, longer release; silence has no decorative motion.
            let tau = if value > *shown { 0.025 } else { 0.075 };
            *shown += (value - *shown) * (1.0 - (-dt / tau).exp());
        }
        self.updated = Some(now);
    }
}

fn rect(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    bounds: (f32, f32, f32, f32),
    radius: f32,
    rgb: (u8, u8, u8),
) {
    let (x0, y0, rw, rh) = bounds;
    let r = radius.min(rw / 2.0).min(rh / 2.0);
    for y in (y0.floor() as i32 - 1).max(0)..((y0 + rh).ceil() as i32 + 1).min(h as i32) {
        for x in (x0.floor() as i32 - 1).max(0)..((x0 + rw).ceil() as i32 + 1).min(w as i32) {
            let qx = (x as f32 + 0.5 - x0 - rw / 2.0).abs() - (rw / 2.0 - r);
            let qy = (y as f32 + 0.5 - y0 - rh / 2.0).abs() - (rh / 2.0 - r);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            for (ch, ink) in [rgb.2, rgb.1, rgb.0].into_iter().enumerate() {
                canvas[i + ch] = (ink as f32 * a + canvas[i + ch] as f32 * (1.0 - a)).round() as u8;
            }
            canvas[i + 3] = (255.0 * a + canvas[i + 3] as f32 * (1.0 - a)).round() as u8;
        }
    }
}

/// Device-sized painting keeps the capsule and live bars crisp at every scale.
pub fn render_pill(canvas: &mut [u8], w: u32, h: u32, bars: &[f32]) {
    if w == 0 || h == 0 || canvas.len() < w as usize * h as usize * 4 {
        return;
    }
    let s = w as f32 / SURF_W as f32;
    canvas[..w as usize * h as usize * 4].fill(0);
    rect(
        canvas,
        w,
        h,
        (0.0, 0.0, w as f32, h as f32),
        h as f32 / 2.0,
        (0, 0, 0),
    );
    let n = 7;
    let group = 27 / n;
    for i in 0..n {
        // Combine the producer's bands into a few quiet level marks.
        let start = i * group;
        let end = if i == n - 1 { 27 } else { (i + 1) * group };
        let level = bars
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .map(|v| {
                if v.is_finite() {
                    v.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .sum::<f32>()
            / (end - start).max(1) as f32;
        let height = (2.0 + 10.0 * level) * s;
        rect(
            canvas,
            w,
            h,
            (
                (21.25 + i as f32 * 5.0) * s,
                (h as f32 - height) / 2.0,
                1.5 * s,
                height,
            ),
            0.75 * s,
            if level > 0.025 {
                (208, 212, 220)
            } else {
                (92, 96, 104)
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_has_real_levels_and_resets_between_utterances() {
        let mut meter = Recording::new();
        meter.start();
        meter.update([1.0; 27]);
        assert!(meter.bars.iter().all(|v| *v > 0.5 && *v < 1.0));
        meter.update([f32::NAN; 27]);
        assert!(meter.bars.iter().all(|v| v.is_finite() && *v < 1.0));
        meter.start();
        assert_eq!(meter.bars, [0.0; 27]);
    }

    #[test]
    fn recording_paints_only_a_centred_meter_at_native_scales() {
        for scale in [1, 2, 3] {
            let (w, h) = (SURF_W * scale, SURF_H * scale);
            let mut silent = vec![0; (w * h * 4) as usize];
            render_pill(&mut silent, w, h, &[0.0; 27]);
            let mut voiced = vec![0; silent.len()];
            render_pill(&mut voiced, w, h, &[0.9; 27]);
            assert_eq!(&voiced[..4], &[0; 4]);
            assert!(voiced
                .chunks_exact(4)
                .all(|p| p[..3].iter().all(|c| *c <= p[3])));
            let count = |pixels: &[u8]| pixels.chunks_exact(4).filter(|p| p[0] > 80).count();
            assert!(count(&voiced) > count(&silent) * 2);
            for y in 0..h {
                for x in 0..w {
                    let i = ((y * w + x) * 4) as usize;
                    if voiced[i..i + 3].iter().any(|c| *c > 0) {
                        assert!((19 * scale..55 * scale).contains(&x));
                        assert!((6 * scale..18 * scale).contains(&y));
                    }
                }
            }
            let mut malformed = vec![0; silent.len()];
            render_pill(&mut malformed, w, h, &[f32::NAN]);
            assert_eq!(malformed, silent);
        }
    }
}
