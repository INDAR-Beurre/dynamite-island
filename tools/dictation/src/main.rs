//! static-dictation v1: lightweight push-to-talk dictation → Groq whisper-large-v3-turbo.

mod bands;
mod capture;
mod config;
mod ctrl;
mod endpoint;
mod groq;
mod hk;
mod output;
mod pill;
mod vad;

// H1 frame dump (see maybe_spawn call in main): dead weight unless the env
// flag is set — when it is not, the module's one function returns immediately.
mod dump;

use crossbeam_channel::unbounded;
use endpoint::Decision;
use hk::HotkeyEvent;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{atomic::Ordering, Arc};

/// The waveform is drawn by the Dynamite pill itself, not a second overlay.
fn dynamite_send(msg: &str) {
    let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return;
    };
    let path = PathBuf::from(rt).join("dynamite-rust.sock");
    if let Ok(sock) = UnixDatagram::unbound() {
        let _ = sock.send_to(msg.as_bytes(), path);
    }
}

fn dictation_show() {
    dynamite_send("dictation_start");
}

fn dictation_hide() {
    dynamite_send("dictation_end");
}

/// ms of silence after speech before we finalize an utterance.
const SILENCE_MS: u32 = 700;
/// 30 ms frames @ 16 kHz.
const FRAME: usize = 480;
/// Keep ~300 ms of pre-voice audio so utterance starts aren't clipped.
const PRE_ROLL: usize = 4800;
/// Whisper invents "Thank you."/"Amen." on room noise. Require ~300 ms of voiced
/// frames in the utterance before spending a Groq request on it.
const MIN_VOICE_FRAMES: u32 = 10;

fn worth_transcribing(voice_frames: u32, samples: usize) -> bool {
    voice_frames >= MIN_VOICE_FRAMES && samples > 0
}

fn main() {
    // A closed stdout reader (e.g. piped to `head`) must kill us silently, not panic in println.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cfg = config::load();
    // No separate overlay: the waveform lives in the Dynamite pill.
    let pill: Option<pill::PillHandle> = None;

    let (hk_tx, hk_rx) = unbounded::<HotkeyEvent>();
    let recording_flag = Arc::new(AtomicBool::new(false));
    ctrl::spawn(hk_tx.clone(), recording_flag.clone());
    std::thread::spawn(move || {
        match hk::listen(hk_tx) {
            Err(e) => {
                eprintln!("hotkey listener failed: {e}");
                std::process::exit(1);
            }
            // stdin fallback hit EOF/"quit": hotkey source gone → shut down the whole process.
            Ok(()) => {
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
                std::process::exit(0);
            }
        }
    });

    println!("static-dictation v1 — hold Super+D to dictate; release Super to transcribe");

    let audio = capture::start().expect("no microphone available");
    let mut vad = vad::Vad::new();
    let mut ep = endpoint::EndPointer::new(SILENCE_MS);
    let mut band_analyzer = bands::Analyzer::new();
    let mut sim = pill::vecmentor::Sim::new(pill::vecmentor::wave_params());
    let mut recording = false; // Super+D hold (or the control socket's toggle)
    let mut hold_active = false; // true while the Super+D hold is active
    let mut voice_frames: u32 = 0;
    let mut utterance: Vec<f32> = Vec::new();
    let mut buf: Vec<f32> = Vec::new();

    // H1 harness hook: STATIC_DICTATION_DUMP_FRAMES=<dir> spawns a headless
    // frame dumper rendering the pill's real Sim+canvas pipeline offscreen,
    // writing a PNG every fixed 100 ms wall interval — beats grim's ~0.3
    // s/shot aliasing. Env unset → nothing starts; zero behavior change.
    dump::maybe_spawn();

    loop {
        // Move any new audio into the local buffer (non-blocking).
        while let Ok(chunk) = audio.try_recv() {
            buf.extend_from_slice(&chunk);
        }

        // Process whole 30 ms frames.
        while buf.len() >= FRAME {
            let frame: Vec<f32> = buf.drain(..FRAME).collect();
            if recording {
                let voice = vad.is_voice(&frame);
                if voice {
                    voice_frames += 1;
                }
                if voice_frames == 0 && utterance.len() > PRE_ROLL {
                    let excess = utterance.len() - PRE_ROLL;
                    utterance.drain(..excess);
                }
                utterance.extend_from_slice(&frame);
                let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
                let bands = band_analyzer.energies(&frame);
                let active = voice || rms >= 0.006;
                let level = if active { (rms * 25.0).min(2.0) } else { 0.0 };
                sim.push(level, &bands, active);
                let msg = format!(
                    "dictation_bars:{}",
                    sim.bars()
                        .iter()
                        .map(|v| format!("{v:.3}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                dynamite_send(&msg);
                if hold_active {
                    // Hold mode keeps one utterance open until Super release,
                    // flushing only near Groq's 60 s safety limit (~55 s).
                    if utterance.len() >= 16_000 * 55 {
                        finalize(&cfg, &pill, &mut utterance, &mut voice_frames, &mut ep);
                        audio.drop_backlog();
                        dictation_show();
                    }
                } else if ep.push(voice) == Decision::End {
                    finalize(&cfg, &pill, &mut utterance, &mut voice_frames, &mut ep);
                    audio.drop_backlog();
                    // Socket-driven toggle mode keeps going until the next toggle
                    dictation_show();
                }
            }
        }

        // Hotkey events.
        while let Ok(ev) = hk_rx.try_recv() {
            match ev {
                HotkeyEvent::HoldStart => {
                    eprintln!("static-dictation: hold start (Super+D)");
                    hold_active = true;
                    if !recording {
                        ep = endpoint::EndPointer::new(SILENCE_MS);
                        voice_frames = 0;
                        audio.drop_backlog();
                        buf.clear();
                        utterance.clear();
                        sim = pill::vecmentor::Sim::new(pill::vecmentor::wave_params());
                        dictation_show();
                        recording = true;
                        recording_flag.store(true, Ordering::SeqCst);
                    }
                }
                HotkeyEvent::HoldEnd => {
                    eprintln!("static-dictation: hold end (Super+D)");
                    hold_active = false;
                    if recording {
                        recording = false;
                        recording_flag.store(false, Ordering::SeqCst);
                        dictation_hide();
                        finalize(&cfg, &pill, &mut utterance, &mut voice_frames, &mut ep);
                        audio.drop_backlog();
                    }
                }
                HotkeyEvent::Toggle => {
                    eprintln!(
                        "static-dictation: toggle event received (recording={} )",
                        recording
                    );
                    if recording {
                        recording = false;
                        hold_active = false;
                        recording_flag.store(false, Ordering::SeqCst);
                        dictation_hide();
                        finalize(&cfg, &pill, &mut utterance, &mut voice_frames, &mut ep);
                        audio.drop_backlog();
                    } else {
                        ep = endpoint::EndPointer::new(SILENCE_MS);
                        voice_frames = 0;
                        audio.drop_backlog();
                        buf.clear();
                        utterance.clear();
                        hold_active = false;
                        sim = pill::vecmentor::Sim::new(pill::vecmentor::wave_params());
                        dictation_show();
                        recording = true;
                        recording_flag.store(true, Ordering::SeqCst);
                    }
                }
            }
        }

        // Nothing to do: brief sleep instead of a busy loop.
        if buf.len() < FRAME && hk_rx.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

/// Send utterance → groq → output. Blocking; at most one utterance in flight by construction.
fn finalize(
    cfg: &config::Config,
    pill: &Option<pill::PillHandle>,
    utterance: &mut Vec<f32>,
    voice_frames: &mut u32,
    ep: &mut endpoint::EndPointer,
) {
    *ep = endpoint::EndPointer::new(SILENCE_MS);
    // Hide before the blocking network call so the pill vanishes the instant dictation stops.
    dictation_hide();
    if !worth_transcribing(*voice_frames, utterance.len()) {
        utterance.clear();
        *voice_frames = 0;
        return;
    }
    match groq::transcribe(&cfg.groq_api_key, &cfg.endpoint, utterance) {
        Ok(text) if !text.is_empty() => output::deliver(&text),
        Ok(_) => output::error("empty transcription"),
        Err(e) => output::error(&e.to_string()),
    }
    utterance.clear();
    *voice_frames = 0;
}

#[cfg(test)]
mod tests {
    use super::worth_transcribing;

    #[test]
    fn room_noise_blips_are_not_transcribed() {
        // Measured on this host: 12 s of ambient mic yields at most 8 voiced
        // frames in a row, 11 total — well under a real utterance.
        assert!(!worth_transcribing(3, 4800));
        assert!(!worth_transcribing(9, 4800));
        assert!(!worth_transcribing(10, 0));
    }

    #[test]
    fn sustained_speech_is_transcribed() {
        // 3 s of spoken "hello this is a dictation test one two three" = 36 voiced frames.
        assert!(worth_transcribing(10, 4800));
        assert!(worth_transcribing(36, 48000));
    }
}
