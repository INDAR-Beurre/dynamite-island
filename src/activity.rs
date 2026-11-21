//! Notification mirror and short-lived island feedback. The existing daemon
//! keeps ownership of org.freedesktop.Notifications and its notification history.
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::calloop;
use crate::controlcenter_panel::{self as cc, Rect};
use fontdue::Font;
use zvariant::OwnedValue;

const MAX_NOTIFICATIONS: usize = 32;
pub const WIDTH: u32 = cc::CC_W;
pub const PAGE_ROWS: usize = 4;
const ROW_H: f32 = 90.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LevelKind {
    Volume,
    Brightness,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub kind: LevelKind,
    pub percent: u16,
    pub muted: bool,
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub received: Instant,
    pub silent: bool,
}

#[derive(Clone, Debug)]
pub enum Event {
    Notification(Notification),
    Level(Level),
    Closed(u32),
}

#[derive(Default)]
struct History {
    notes: VecDeque<Notification>,
    preview: Option<Event>,
    offset: usize,
}

impl History {
    fn apply(&mut self, event: Event) {
        match &event {
            Event::Notification(note) => {
                self.notes.retain(|n| n.id != note.id);
                self.notes.push_front(note.clone());
                self.notes.truncate(MAX_NOTIFICATIONS);
                self.offset = 0;
                if !note.silent {
                    self.preview = Some(event);
                }
            }
            Event::Level(_) => self.preview = Some(event),
            Event::Closed(id) => {
                self.notes.retain(|n| n.id != *id);
                self.offset = self.offset.min(self.notes.len().saturating_sub(PAGE_ROWS));
                if matches!(&self.preview, Some(Event::Notification(n)) if n.id == *id) {
                    self.preview = None;
                }
            }
        }
    }
}

static HISTORY: Mutex<History> = Mutex::new(History {
    notes: VecDeque::new(),
    preview: None,
    offset: 0,
});

pub fn apply(event: Event) {
    HISTORY.lock().unwrap().apply(event);
}
pub fn latest() -> Option<Notification> {
    HISTORY.lock().unwrap().notes.front().cloned()
}
pub fn preview() -> Option<Event> {
    HISTORY.lock().unwrap().preview.clone()
}
pub fn count() -> usize {
    HISTORY.lock().unwrap().notes.len()
}
pub fn on_enter() {
    HISTORY.lock().unwrap().offset = 0;
}

pub fn scroll(dir: i32) {
    let mut h = HISTORY.lock().unwrap();
    h.offset =
        (h.offset as i32 + dir).clamp(0, h.notes.len().saturating_sub(PAGE_ROWS) as i32) as usize;
}

pub fn inbox_size() -> (u32, u32) {
    (
        WIDTH,
        98 + count().clamp(1, PAGE_ROWS) as u32 * ROW_H as u32,
    )
}

pub fn preview_size() -> (u32, u32) {
    (
        WIDTH,
        if matches!(preview(), Some(Event::Level(_))) {
            68
        } else {
            148
        },
    )
}

pub fn dismiss_at(x: f32, y: f32) -> bool {
    let id = {
        let h = HISTORY.lock().unwrap();
        let row = ((y - 52.0) / ROW_H).floor();
        if x < WIDTH as f32 - 52.0 || y < 52.0 || row >= PAGE_ROWS as f32 {
            return false;
        }
        h.notes.get(h.offset + row as usize).map(|n| n.id)
    };
    if let Some(id) = id {
        apply(Event::Closed(id));
        // The inbox also keeps expired notifications, which the daemon may no longer own.
        std::thread::spawn(move || {
            let result = zbus::blocking::Connection::session().and_then(|c| {
                c.call_method(
                    Some("org.freedesktop.Notifications"),
                    "/org/freedesktop/Notifications",
                    Some("org.freedesktop.Notifications"),
                    "CloseNotification",
                    &id,
                )
            });
            if let Err(e) = result {
                eprintln!("dynamite-rust: dismiss notification: {e}");
            }
        });
        true
    } else {
        false
    }
}

pub fn plain_text(text: &str) -> String {
    let mut result = String::new();
    let mut tag = false;
    for c in text.chars().take(4096) {
        match c {
            '<' => tag = true,
            '>' if tag => {
                tag = false;
                result.push(' ');
            }
            _ if !tag => result.push(if c.is_control() { ' ' } else { c }),
            _ => {}
        }
    }
    let decoded = result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    decoded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(512)
        .collect()
}

fn percent_in(text: &str) -> Option<u16> {
    let end = text.find('%')?;
    let digits: String = text[..end]
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .chars()
        .rev()
        .collect::<String>()
        .parse::<u16>()
        .ok()
        .map(|n| n.min(150))
}

fn level_from_notify(summary: &str, hints: &HashMap<String, OwnedValue>) -> Option<Level> {
    let hint = hints
        .get("x-canonical-private-synchronous")
        .and_then(|v| <&str>::try_from(v).ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = if hint.contains("volume") {
        LevelKind::Volume
    } else if hint.contains("brightness") {
        LevelKind::Brightness
    } else {
        return None;
    };
    let text = summary.to_lowercase();
    let muted = text.contains("muted") || text.contains("stumm");
    let percent = hints
        .get("value")
        .and_then(|v| i32::try_from(v).ok())
        .map(|v| v.clamp(0, 150) as u16)
        .or_else(|| percent_in(summary))
        .unwrap_or(0);
    Some(Level {
        kind,
        percent,
        muted,
    })
}

/// Runs on its own connection: monitoring never takes the daemon's bus name.
pub fn watch(tx: calloop::channel::Sender<Event>) {
    std::thread::spawn(move || loop {
        if let Err(e) = monitor(&tx) {
            eprintln!("dynamite-rust: notification monitor: {e}");
        }
        std::thread::sleep(Duration::from_secs(5));
    });
}

fn monitor(tx: &calloop::channel::Sender<Event>) -> zbus::Result<()> {
    let control = zbus::blocking::Connection::session()?;
    let connection = zbus::blocking::Connection::session()?;
    let mut messages = zbus::blocking::MessageIterator::from(&connection);
    connection.call_method(
        Some("org.freedesktop.DBus"), "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus.Monitoring"), "BecomeMonitor",
        &(&[
            "type='method_call',interface='org.freedesktop.Notifications',member='Notify'",
            "type='method_return',sender='org.freedesktop.Notifications'",
            "type='signal',sender='org.freedesktop.Notifications',interface='org.freedesktop.Notifications',member='NotificationClosed'",
        ][..], 0u32),
    )?;
    let mut pending = HashMap::new();
    for message in &mut messages {
        let message = message?;
        let header = message.header();
        match message.message_type() {
            zbus::message::Type::MethodCall
                if header.member().map(|m| m.as_str()) == Some("Notify") =>
            {
                type Notify = (
                    String,
                    u32,
                    String,
                    String,
                    String,
                    Vec<String>,
                    HashMap<String, OwnedValue>,
                    i32,
                );
                let Ok((app, _, _, summary, body, _, hints, _)) =
                    message.body().deserialize::<Notify>()
                else {
                    continue;
                };
                if let Some(level) = level_from_notify(&summary, &hints) {
                    if tx.send(Event::Level(level)).is_err() {
                        return Ok(());
                    }
                    continue;
                }
                let silent = false;
                let note = Notification {
                    id: 0,
                    app: plain_text(&app),
                    summary: plain_text(&summary),
                    body: plain_text(&body),
                    received: Instant::now(),
                    silent,
                };
                if pending.len() >= 64 {
                    pending.clear();
                }
                if let Some(sender) = header.sender() {
                    pending.insert(
                        (sender.to_string(), header.primary().serial_num().get()),
                        note,
                    );
                }
            }
            zbus::message::Type::MethodReturn => {
                if let (Some(destination), Some(serial)) =
                    (header.destination(), header.reply_serial())
                {
                    if let Some(mut note) = pending.remove(&(destination.to_string(), serial.get()))
                    {
                        if let Ok(id) = message.body().deserialize::<u32>() {
                            note.id = id;
                            // DND governs banners; suppress-sound only disables notification audio.
                            note.silent = control
                                .call_method(
                                    Some("org.erikreider.swaync.cc"),
                                    "/org/erikreider/swaync/cc",
                                    Some("org.erikreider.swaync.cc"),
                                    "GetDnd",
                                    &(),
                                )
                                .ok()
                                .and_then(|reply| reply.body().deserialize::<bool>().ok())
                                .unwrap_or(false);
                            if tx.send(Event::Notification(note)).is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            zbus::message::Type::Signal
                if header.member().map(|m| m.as_str()) == Some("NotificationClosed") =>
            {
                if let Ok((id, reason)) = message.body().deserialize::<(u32, u32)>() {
                    // Expiration hides a toast; keep its contents available for review.
                    if reason != 1 && tx.send(Event::Closed(id)).is_err() {
                        return Ok(());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn text(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &Font,
    value: &str,
    px: f32,
    x: f32,
    y: f32,
    color: (u8, u8, u8),
    max_w: f32,
) {
    let value = cc::fit_text(font, value, px, max_w);
    cc::draw_text(canvas, w, h, font, &value, px, x, y, color, false);
}

fn body_lines(font: &Font, body: &str, width: f32) -> [String; 2] {
    let mut lines = [String::new(), String::new()];
    let words: Vec<_> = body.split_whitespace().collect();
    let mut index = 0;
    for (line_no, line) in lines.iter_mut().enumerate() {
        while index < words.len() {
            let candidate = if line.is_empty() {
                words[index].into()
            } else {
                format!("{line} {}", words[index])
            };
            if line_no == 0 && !line.is_empty() && cc::text_width(font, &candidate, 11.0) > width {
                break;
            }
            *line = candidate;
            index += 1;
            if line_no == 0 && cc::text_width(font, line, 11.0) > width {
                break;
            }
        }
        *line = cc::fit_text(font, line, 11.0, width);
    }
    lines
}

fn draw_note(canvas: &mut [u8], w: u32, h: u32, font: &Font, n: &Notification, y: f32, full: bool) {
    let right = WIDTH as f32 - 30.0;
    text(
        canvas,
        w,
        h,
        font,
        if n.app.is_empty() {
            "Notification"
        } else {
            &n.app
        },
        11.0,
        20.0,
        y,
        cc::accent(),
        right - 65.0,
    );
    let age = n.received.elapsed().as_secs() / 60;
    text(
        canvas,
        w,
        h,
        font,
        &if age == 0 {
            "now".into()
        } else {
            format!("{age}m")
        },
        10.0,
        right - 42.0,
        y,
        cc::C_SECONDARY,
        40.0,
    );
    let summary_w = if full {
        WIDTH as f32 - 76.0
    } else {
        right - 24.0
    };
    text(
        canvas,
        w,
        h,
        font,
        &n.summary,
        14.0,
        20.0,
        y + 21.0,
        cc::C_PRIMARY,
        summary_w,
    );
    let lines = body_lines(font, &n.body, right - 20.0);
    for (i, line) in lines.iter().enumerate() {
        text(
            canvas,
            w,
            h,
            font,
            line,
            11.0,
            20.0,
            y + 43.0 + i as f32 * 16.0,
            cc::C_SECONDARY,
            right - 20.0,
        );
    }
    if full {
        text(
            canvas,
            w,
            h,
            font,
            "×",
            18.0,
            WIDTH as f32 - 38.0,
            y + 20.0,
            cc::C_SECONDARY,
            22.0,
        );
    }
}

pub fn draw_preview(canvas: &mut [u8], w: u32, h: u32, font: &Font, icon_font: Option<&Font>) {
    cc::set_scale(w);
    match preview() {
        Some(Event::Level(level)) => {
            let (icon, label) = match level.kind {
                LevelKind::Volume => ("volume_up", if level.muted { "Muted" } else { "Volume" }),
                LevelKind::Brightness => ("brightness_6", "Brightness"),
            };
            cc::draw_icon(
                canvas,
                w,
                h,
                icon_font,
                icon,
                24.0,
                30.0,
                34.0,
                cc::C_PRIMARY,
            );
            text(
                canvas,
                w,
                h,
                font,
                label,
                11.0,
                54.0,
                17.0,
                cc::C_SECONDARY,
                120.0,
            );
            text(
                canvas,
                w,
                h,
                font,
                &format!("{}%", level.percent),
                13.0,
                WIDTH as f32 - 58.0,
                26.0,
                cc::C_PRIMARY,
                50.0,
            );
            let track = Rect::new(54.0, 40.0, WIDTH as f32 - 134.0, 5.0);
            cc::fill_rounded_rect(canvas, w, h, track, 2.5, cc::C_SLIDER_TRACK);
            let amount = if level.muted {
                0.0
            } else {
                (level.percent as f32 / 100.0).min(1.0)
            };
            if amount > 0.0 {
                cc::fill_rounded_rect(
                    canvas,
                    w,
                    h,
                    Rect::new(track.x, track.y, track.w * amount, track.h),
                    2.5,
                    cc::accent(),
                );
            }
        }
        Some(Event::Notification(note)) => {
            draw_note(canvas, w, h, font, &note, 18.0, false);
            text(
                canvas,
                w,
                h,
                font,
                "Click to review notifications",
                10.0,
                20.0,
                123.0,
                cc::C_SECONDARY,
                WIDTH as f32 - 40.0,
            );
        }
        _ => {}
    }
}

pub fn draw_inbox(canvas: &mut [u8], w: u32, h: u32, font: &Font) {
    cc::set_scale(w);
    let history = HISTORY.lock().unwrap();
    text(
        canvas,
        w,
        h,
        font,
        "Notifications",
        16.0,
        20.0,
        18.0,
        cc::C_PRIMARY,
        300.0,
    );
    text(
        canvas,
        w,
        h,
        font,
        "×",
        20.0,
        WIDTH as f32 - 38.0,
        14.0,
        cc::C_SECONDARY,
        24.0,
    );
    if history.notes.is_empty() {
        text(
            canvas,
            w,
            h,
            font,
            "No recent notifications",
            14.0,
            20.0,
            70.0,
            cc::C_PRIMARY,
            360.0,
        );
        text(
            canvas,
            w,
            h,
            font,
            "New notifications appear here.",
            11.0,
            20.0,
            96.0,
            cc::C_SECONDARY,
            360.0,
        );
    } else {
        for (i, note) in history
            .notes
            .iter()
            .skip(history.offset)
            .take(PAGE_ROWS)
            .enumerate()
        {
            let y = 52.0 + i as f32 * ROW_H;
            cc::fill_rounded_rect(
                canvas,
                w,
                h,
                Rect::new(12.0, y - 3.0, WIDTH as f32 - 24.0, ROW_H - 5.0),
                12.0,
                cc::C_NOTIF_CARD,
            );
            draw_note(canvas, w, h, font, note, y + 6.0, true);
        }
    }
    let footer_y = (h as f32 / (w as f32 / WIDTH as f32)) - 30.0;
    text(
        canvas,
        w,
        h,
        font,
        "Open notification center  ›",
        11.0,
        20.0,
        footer_y,
        cc::C_SECONDARY,
        WIDTH as f32 - 40.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(id: u32, summary: &str) -> Notification {
        Notification {
            id,
            app: "Mail".into(),
            summary: summary.into(),
            body: "Message".into(),
            received: Instant::now(),
            silent: false,
        }
    }
    #[test]
    fn replacements_and_dismissals_keep_history_correct() {
        let mut h = History::default();
        h.apply(Event::Notification(note(1, "First")));
        h.apply(Event::Notification(note(2, "Second")));
        h.apply(Event::Notification(note(1, "Updated")));
        assert_eq!(h.notes.iter().map(|n| n.id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(h.notes[0].summary, "Updated");
        h.apply(Event::Closed(1));
        assert_eq!(h.notes[0].id, 2);
        assert!(h.preview.is_none());
        for i in 3..100 {
            h.apply(Event::Notification(note(i, "New")));
        }
        assert_eq!(h.notes.len(), MAX_NOTIFICATIONS);
    }
    #[test]
    fn silent_notifications_stay_in_history_and_levels_do_not() {
        let mut h = History::default();
        let mut silent = note(1, "Quiet update");
        silent.silent = true;
        h.apply(Event::Notification(silent));
        assert_eq!(h.notes.len(), 1);
        assert!(h.preview.is_none());
        h.apply(Event::Level(Level {
            kind: LevelKind::Volume,
            percent: 55,
            muted: false,
        }));
        assert_eq!(h.notes.len(), 1);
        assert!(matches!(h.preview, Some(Event::Level(_))));
    }

    #[test]
    fn notification_text_is_plain_and_bounded() {
        assert_eq!(
            plain_text("<b>Meeting</b>\nRoom &amp; tea"),
            "Meeting Room & tea"
        );
        assert_eq!(plain_text(&"é".repeat(600)).chars().count(), 512);
        assert_eq!(percent_in("Lautstärke: 105%"), Some(105));
        let hints = HashMap::from([(
            "x-canonical-private-synchronous".into(),
            OwnedValue::from(zvariant::Str::from("brightness")),
        )]);
        let level = level_from_notify("Helligkeit: 35%", &hints).unwrap();
        assert_eq!(level.kind, LevelKind::Brightness);
        assert_eq!(level.percent, 35);
        assert!(level_from_notify("Ordinary message", &HashMap::new()).is_none());
    }
}
