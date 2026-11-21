//! Native display-scaling controls. Commands run only on short-lived workers.
use std::sync::{atomic::{AtomicBool, Ordering}, Mutex};
use std::time::{Duration, Instant};

pub const PANEL_W: u32 = 438;
pub const PANEL_H: u32 = 284;
const PRESETS: [f32; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];
const MIN_SCALE: f32 = 0.5;
const MAX_SCALE: f32 = 2.5;

#[derive(Clone, Debug)]
struct Output {
    name: String,
    enabled: bool,
    scale: f32,
    mode: String,
}
#[derive(Clone)]
struct View {
    outputs: Vec<Output>,
    selected: usize,
    draft: f32,
    message: String,
    last_refresh: Option<Instant>,
}
static VIEW: Mutex<View> = Mutex::new(View {
    outputs: Vec::new(), selected: 0, draft: 1.5,
    message: String::new(), last_refresh: None,
});
static BUSY: AtomicBool = AtomicBool::new(false);
static CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action { Select(i32), Step(i32), Preset(usize), Apply, Refresh }

fn parse_outputs(text: &str) -> Vec<Output> {
    let mut outputs = Vec::<Output>::new();
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) && !line.is_empty() {
            if let Some(name) = line.split_whitespace().next() {
                outputs.push(Output { name: name.into(), enabled: false, scale: 1.0, mode: String::new() });
            }
        } else if let Some(output) = outputs.last_mut() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("Enabled: ") {
                output.enabled = value == "yes";
            } else if let Some(value) = line.strip_prefix("Scale: ") {
                if let Ok(scale) = value.parse::<f32>() {
                    if scale.is_finite() && (MIN_SCALE..=MAX_SCALE).contains(&scale) { output.scale = scale; }
                }
            } else if line.contains(" px,") && (line.contains("(current") || line.contains(", current") || output.mode.is_empty()) {
                output.mode = line.split(" (").next().unwrap_or(line).replace(" px,", " ·");
            }
        }
    }
    outputs
}

fn run(args: &[&str]) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new("wlr-randr").args(args).stdin(Stdio::null())
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .map_err(|e| format!("Cannot start wlr-randr: {e}"))?;
    let Some(mut stdout) = child.stdout.take() else { return Err("No display response".into()) };
    let Some(mut stderr) = child.stderr.take() else { return Err("No display error stream".into()) };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.by_ref().take(256 * 1024).read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.by_ref().take(16 * 1024).read_to_end(&mut bytes);
        let _ = err_tx.send(bytes);
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(10)),
            _ => { let _ = child.kill(); let _ = child.wait(); return Err("Display command timed out".into()); }
        }
    };
    let out = rx.recv_timeout(Duration::from_millis(200)).unwrap_or_default();
    if !status.success() {
        let error = err_rx.recv_timeout(Duration::from_millis(200)).unwrap_or_default();
        return Err(format!("Display rejected change: {}", String::from_utf8_lossy(&error).trim()));
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Change only the selected output's scale; preserve modes, positions and comments.
fn updated_rules(text: &str, name: &str, scale: f32) -> Result<String, String> {
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        || !scale.is_finite() || !(MIN_SCALE..=MAX_SCALE).contains(&scale) {
        return Err("Invalid display or scale".into());
    }
    let mut found = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let (body, comment) = line.split_once('#').map_or((line, None), |(a, b)| (a, Some(b)));
        if let Some((key, value)) = body.split_once('=') {
            let mut fields: Vec<String> = value.split(',').map(str::to_owned).collect();
            if key.trim() == "monitor_rule" && fields.iter().any(|f| f.trim() == format!("name:{name}")) {
                found = true;
                let mut has_scale = false;
                for field in &mut fields {
                    if field.trim().starts_with("scale:") { *field = format!("scale:{scale:.2}"); has_scale = true; }
                }
                if !has_scale { fields.push(format!("scale:{scale:.2}")); }
                let suffix = comment.map_or(String::new(), |c| format!(" #{c}"));
                lines.push(format!("{key}={}{}", fields.join(","), suffix));
                continue;
            }
        }
        lines.push(line.to_owned());
    }
    if !found { lines.push(format!("monitor_rule=name:{name},scale:{scale:.2}")); }
    Ok(format!("{}\n", lines.join("\n")))
}

fn apply(output: &Output, scale: f32) -> Result<(), String> {
    // Read and validate persistence before touching the live output.
    let path = crate::theme::config_home().join("mango/monitor.conf");
    let before = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("Cannot read monitor.conf: {e}")),
    };
    let after = updated_rules(&before, &output.name, scale)?;
    let value = format!("{scale:.2}");
    let old = format!("{:.2}", output.scale);
    run(&["--dryrun", "--output", &output.name, "--scale", &value])?;
    run(&["--output", &output.name, "--scale", &value])?;
    if let Err(e) = crate::theme::atomic_write(&path, after.as_bytes()) {
        return match run(&["--output", &output.name, "--scale", &old]) {
            Ok(_) => Err(format!("Could not save scale; restored previous scale: {e}")),
            Err(rollback) => Err(format!("Scale changed but could not be saved: {e}; {rollback}")),
        };
    }
    Ok(())
}

pub fn action(action: Action) {
    if BUSY.load(Ordering::Acquire) { return; }
    if matches!(action, Action::Apply | Action::Refresh) {
        if BUSY.swap(true, Ordering::AcqRel) { return; }
        let selected = {
            let Ok(mut view) = VIEW.lock() else { BUSY.store(false, Ordering::Release); return };
            view.last_refresh = Some(Instant::now());
            if action == Action::Apply { view.message = "Applying…".into(); }
            view.outputs.get(view.selected).cloned().map(|o| (o, view.draft))
        };
        CHANGED.store(true, Ordering::Release);
        let spawn = std::thread::Builder::new().name("display-settings".into()).spawn(move || {
            let result = if action == Action::Apply {
                selected.ok_or_else(|| "No display selected".to_string()).and_then(|(output, scale)| {
                    if !output.enabled { return Err("Wake this display before changing its scale".into()); }
                    apply(&output, scale)
                })
            } else { Ok(()) };
            let outputs = run(&[]).map(|text| parse_outputs(&text));
            let refreshed = outputs.is_ok();
            if let Ok(mut view) = VIEW.lock() {
                let old_name = view.outputs.get(view.selected).map(|o| o.name.clone());
                match outputs {
                    Ok(outputs) => {
                        view.outputs = outputs;
                        view.selected = old_name.and_then(|name| view.outputs.iter().position(|o| o.name == name)).unwrap_or(0);
                        view.draft = view.outputs.get(view.selected).map_or(1.5, |o| o.scale);
                    }
                    Err(ref e) => view.message = e.clone(),
                }
                match result {
                    Err(e) => view.message = e,
                    Ok(()) if refreshed && action == Action::Apply => view.message = "Applied and saved for your next login".into(),
                    Ok(()) if refreshed => view.message.clear(),
                    _ => {}
                }
            }
            BUSY.store(false, Ordering::Release);
            CHANGED.store(true, Ordering::Release);
        });
        if let Err(e) = spawn {
            if let Ok(mut view) = VIEW.lock() { view.message = format!("Cannot start display worker: {e}"); }
            BUSY.store(false, Ordering::Release);
        }
    } else if let Ok(mut view) = VIEW.lock() {
        match action {
            Action::Select(delta) if !view.outputs.is_empty() => {
                view.selected = (view.selected as i32 + delta).rem_euclid(view.outputs.len() as i32) as usize;
                view.draft = view.outputs[view.selected].scale;
            }
            Action::Step(delta) => view.draft = ((view.draft * 20.0).round() + delta as f32).clamp(10.0, 50.0) / 20.0,
            Action::Preset(index) => if let Some(value) = PRESETS.get(index) { view.draft = *value; },
            _ => {}
        }
        view.message.clear();
        CHANGED.store(true, Ordering::Release);
    }
}

/// No subprocess, timer or polling thread while this menu is closed.
pub fn poll() -> bool {
    let due = VIEW.lock().map(|v| v.last_refresh.is_none_or(|t| t.elapsed() > Duration::from_secs(5))
        && v.outputs.get(v.selected).is_none_or(|o| (o.scale - v.draft).abs() < 0.01)).unwrap_or(false);
    if due { action(Action::Refresh); }
    CHANGED.swap(false, Ordering::AcqRel)
}

pub fn hit(x: f32, y: f32) -> Option<Action> {
    if (54.0..100.0).contains(&y) {
        if (24.0..64.0).contains(&x) { return Some(Action::Select(-1)); }
        if (374.0..414.0).contains(&x) { return Some(Action::Select(1)); }
    }
    if (148.0..184.0).contains(&y) {
        if (24.0..60.0).contains(&x) { return Some(Action::Step(-1)); }
        if (378.0..414.0).contains(&x) { return Some(Action::Step(1)); }
        for i in 0..5 { if (66.0 + i as f32 * 61.0..123.0 + i as f32 * 61.0).contains(&x) { return Some(Action::Preset(i)); } }
    }
    if (210.0..246.0).contains(&y) && (310.0..414.0).contains(&x) { return Some(Action::Apply); }
    if (14.0..44.0).contains(&y) && (350.0..414.0).contains(&x) { return Some(Action::Refresh); }
    None
}

pub fn draw(canvas: &mut [u8], w: u32, h: u32, font: &fontdue::Font) {
    let Ok(view) = VIEW.lock().map(|v| v.clone()) else { return };
    let s = w as f32 / PANEL_W as f32;
    let ink = [232, 234, 239];
    let dim = [148, 153, 164];
    let accent = crate::library::theme_accent();
    let accent = [accent.0, accent.1, accent.2];
    text(canvas, w, h, font, 24.0, 33.0, 18.0, "Displays", ink, s);
    text(canvas, w, h, font, 357.0, 31.0, 10.0, "Refresh", dim, s);
    rect(canvas, w, h, (24.0, 54.0, 390.0, 46.0), [22, 23, 27], s);
    text(canvas, w, h, font, 37.0, 83.0, 17.0, "‹", dim, s);
    text(canvas, w, h, font, 390.0, 83.0, 17.0, "›", dim, s);
    let output = view.outputs.get(view.selected);
    let title = output.map_or_else(|| {
        if BUSY.load(Ordering::Acquire) { "Finding displays…".into() } else { "No displays found".into() }
    }, |o| {
        if o.name.starts_with("eDP-") { format!("Laptop · {}", o.name) } else { o.name.clone() }
    });
    text(canvas, w, h, font, 78.0, 82.0, 13.0, &title, ink, s);
    text(canvas, w, h, font, 24.0, 128.0, 12.0, "Size of text and apps", dim, s);
    text(canvas, w, h, font, 363.0, 128.0, 13.0, &format!("{:.2}×", view.draft), ink, s);
    for (i, value) in PRESETS.iter().enumerate() {
        let active = (view.draft - value).abs() < 0.01;
        let x = 66.0 + i as f32 * 61.0;
        rect(canvas, w, h, (x, 148.0, 57.0, 36.0), if active { accent } else { [27, 28, 33] }, s);
        let label = format!("{value}×");
        let color = if active { crate::theme::ink_on(accent.into(), ink.into()).into() } else { ink };
        text(canvas, w, h, font, x + 10.0, 171.0, 11.0, &label, color, s);
    }
    for (x, label) in [(24.0, "−"), (378.0, "+")] {
        rect(canvas, w, h, (x, 148.0, 36.0, 36.0), [27, 28, 33], s);
        text(canvas, w, h, font, x + 11.0, 172.0, 16.0, label, ink, s);
    }
    let subtitle = output.map_or("No display detected", |o| if o.enabled { &o.mode } else { "Display sleeping" });
    text(canvas, w, h, font, 24.0, 230.0, 10.0, subtitle, dim, s);
    let enabled = output.is_some_and(|o| o.enabled) && !BUSY.load(Ordering::Acquire);
    rect(canvas, w, h, (310.0, 210.0, 104.0, 36.0), if enabled { accent } else { [27, 28, 33] }, s);
    let color = if enabled { crate::theme::ink_on(accent.into(), ink.into()).into() } else { dim };
    text(canvas, w, h, font, 342.0, 233.0, 12.0, "Apply", color, s);
    let message = if !view.message.is_empty() { &view.message } else { "Choose a scale, then Apply. Up/Down adjusts by 0.05." };
    text(canvas, w, h, font, 24.0, 265.0, 9.0, message, dim, s);
}

fn text(dst: &mut [u8], w: u32, h: u32, font: &fontdue::Font, x: f32, baseline: f32, px: f32, value: &str, rgb: [u8; 3], s: f32) {
    let mut pen = x * s;
    for ch in value.chars() {
        let (m, cov) = crate::lock_timer::rasterize_cached(font, ch, px * s);
        if pen + m.advance_width > (PANEL_W as f32 - 24.0) * s { break; }
        let y0 = (baseline * s).round() as i32 - m.ymin - m.height as i32;
        let x0 = (pen + m.xmin as f32).round() as i32;
        for y in 0..m.height { for x in 0..m.width {
            let (dx, dy) = (x0 + x as i32, y0 + y as i32);
            if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 { continue; }
            let i = (dy as usize * w as usize + dx as usize) * 4;
            let a = cov[y * m.width + x] as f32 / 255.0;
            for c in 0..3 { dst[i + c] = (rgb[2-c] as f32 * a + dst[i+c] as f32 * (1.0-a)) as u8; }
        }}
        pen += m.advance_width;
    }
}
fn rect(dst: &mut [u8], w: u32, h: u32, r: (f32,f32,f32,f32), rgb: [u8;3], s: f32) {
    let (x,y,rw,rh) = r;
    for dy in (y*s) as u32..((y+rh)*s).ceil() as u32 {
        for dx in (x*s) as u32..((x+rw)*s).ceil() as u32 {
            if dx >= w || dy >= h { continue; }
            let qx = ((dx as f32+0.5)/s - x - rw/2.0).abs() - (rw/2.0-7.0);
            let qy = ((dy as f32+0.5)/s - y - rh/2.0).abs() - (rh/2.0-7.0);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - 7.0;
            let a = (0.5-d*s).clamp(0.0,1.0);
            let i = (dy as usize*w as usize+dx as usize)*4;
            for c in 0..3 { dst[i+c]=(rgb[2-c] as f32*a+dst[i+c] as f32*(1.0-a)) as u8; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_active_and_sleeping_outputs_without_losing_current_mode() {
        let outputs = parse_outputs("eDP-1 \"Laptop\"\n  Enabled: yes\n  Modes:\n    1920x1080 px, 60.000000 Hz (preferred)\n    2256x1504 px, 60.000000 Hz (preferred, current)\n  Scale: 1.500000\nDP-1 \"Dock\"\n  Enabled: no\n");
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].scale, 1.5);
        assert!(outputs[0].enabled && !outputs[1].enabled);
        assert_eq!(outputs[0].mode, "2256x1504 · 60.000000 Hz");
    }
    #[test]
    fn scale_save_preserves_other_outputs_positions_and_comments() {
        let before = "# dock\nmonitor_rule=name:DP-1,width:1920,height:1080,x:0,y:0,scale:0.7\nmonitor_rule=name:eDP-1,width:2256,height:1504,x:0,y:1543,scale:1 # laptop\n";
        let after = updated_rules(before, "eDP-1", 1.5).unwrap();
        assert!(after.contains("name:DP-1,width:1920,height:1080,x:0,y:0,scale:0.7\n"));
        assert!(after.contains("x:0,y:1543,scale:1.50 # laptop\n"));
        assert_eq!(updated_rules(&after, "eDP-1", 1.5).unwrap(), after);
        assert!(updated_rules(before, "eDP-1,scale:9", 1.5).is_err());
        assert!(updated_rules(before, "eDP-1", f32::NAN).is_err());
    }
    #[test]
    fn new_output_rules_match_exact_names_and_validate_scale() {
        let before = "monitor_rule=name:DP-10,scale:1\nmonitor_rule=name:DP-1,x:1920\n";
        assert_eq!(updated_rules(before, "DP-1", 0.7).unwrap(),
            "monitor_rule=name:DP-10,scale:1\nmonitor_rule=name:DP-1,x:1920,scale:0.70\n");
        assert_eq!(updated_rules("# empty\n", "eDP-1", 1.5).unwrap(),
            "# empty\nmonitor_rule=name:eDP-1,scale:1.50\n");
        for scale in [0.0, 2.51, f32::INFINITY] {
            assert!(updated_rules(before, "DP-1", scale).is_err());
        }
    }
    #[test]
    fn preset_and_apply_targets_match_visible_controls() {
        for i in 0..5 { assert_eq!(hit(90.0+i as f32*61.0,166.0), Some(Action::Preset(i))); }
        assert_eq!(hit(350.0,228.0), Some(Action::Apply));
        assert_eq!(hit(200.0,200.0), None);
    }
}
