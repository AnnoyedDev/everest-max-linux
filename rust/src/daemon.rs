use crate::actions::Runner;
use crate::audio::Audio;
use crate::config::{Action, Config, DISPLAY_KEY_BITS};
use crate::device::Device;
use crate::stats::Stats;
use crate::{apply, fx, protocol as p};
use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

const FRAME: Duration = Duration::from_millis(33);

fn bit(r: &[u8; 64], byte: usize, bit: u8) -> bool {
    r[byte] >> bit & 1 == 1
}

fn mtime() -> Option<SystemTime> {
    std::fs::metadata(crate::config::path()).and_then(|m| m.modified()).ok()
}

fn default_sink_volume() -> Option<u8> {
    let run = |args: &[&str]| std::process::Command::new("pactl").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    if run(&["get-sink-mute", "@DEFAULT_SINK@"])?.contains("yes") {
        return Some(0);
    }
    let v = run(&["get-sink-volume", "@DEFAULT_SINK@"])?;
    v.split('%').next()?.rsplit(|c: char| !c.is_ascii_digit()).next()?.parse::<u16>().ok().map(|x| x.min(100) as u8)
}

pub fn run() {
    let (tx, rx) = mpsc::channel::<Action>();
    std::thread::spawn(move || {
        let mut runner = Runner::new();
        for a in rx {
            if let Err(e) = runner.run(&a) {
                eprintln!("action: {e}");
            }
        }
    });

    let mut cfg = Config::load();
    let mut cfg_time = mtime();
    let mut stats = Stats::new();
    let start = Instant::now();
    loop {
        let mut dev = match Device::open() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e} — nouvel essai dans 3 s");
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        eprintln!("Everest Max connecté");
        let mut prev = [0u8; 64];
        let mut presses: VecDeque<Instant> = VecDeque::new();
        let mut audio: Option<Audio> = None;
        let mut loads = [0.0f32; 4];
        let mut audio_key: Vec<String> = Vec::new();
        let mut audio_try = Instant::now() - Duration::from_secs(10);
        let (mut last_min, mut last_vol) = (-1i32, None);
        let mut last_tick = Instant::now() - Duration::from_secs(10);
        let mut next_frame = Instant::now();
        let mut soft_active = false;
        let mut failures = 0;
        loop {
            let soft = fx::soft_index(cfg.effect.effect).is_some();
            let wait = if soft { next_frame.saturating_duration_since(Instant::now()).min(Duration::from_millis(250)) } else { Duration::from_millis(250) };
            while let Some(r) = dev.read(wait) {
                if r[0] == 0x01 {
                    for (i, &(byte, b)) in DISPLAY_KEY_BITS.iter().enumerate() {
                        if bit(&r, byte, b) && !bit(&prev, byte, b) {
                            let _ = tx.send(cfg.display_keys[i].action.clone());
                        }
                    }
                    for hk in &cfg.host_keys {
                        if hk.byte < 64 && bit(&r, hk.byte, hk.bit) && !bit(&prev, hk.byte, hk.bit) {
                            let _ = tx.send(hk.action.clone());
                        }
                    }
                    let newly: u32 = (1..64).map(|i| (r[i] & !prev[i]).count_ones()).sum();
                    for _ in 0..newly {
                        presses.push_back(Instant::now());
                    }
                    prev = r;
                }
                if soft && Instant::now() >= next_frame {
                    break;
                }
            }
            if crate::device::find_hidraw().is_err() {
                break;
            }
            if soft {
                if !soft_active {
                    let _ = dev.transact(&p::switch_profile(cfg.profile, p::slot(cfg.effect.effect)));
                    soft_active = true;
                }
                if fx::uses_audio(cfg.effect.effect) && ((audio.is_none() && audio_try.elapsed() > Duration::from_secs(3)) || audio_key != cfg.audio_sources) {
                    audio_try = Instant::now();
                    drop(audio.take());
                    audio = Audio::start(&cfg.audio_sources);
                    audio_key = cfg.audio_sources.clone();
                }
                if Instant::now() >= next_frame {
                    next_frame = Instant::now() + FRAME;
                    let (bands, level) = audio.as_ref().map(|a| a.get()).unwrap_or_default();
                    let inputs = fx::Inputs { bands, level, cpu: loads[0], gpu: loads[1], ram: loads[2], net: loads[3] };
                    let frame = fx::frame(cfg.effect.effect, &cfg.effect, start.elapsed().as_secs_f32(), &inputs);
                    if dev.send_all(&p::custom_static(&frame)).is_err() {
                        failures += 1;
                        if failures > 20 {
                            break;
                        }
                    } else {
                        failures = 0;
                    }
                }
            } else {
                soft_active = false;
                audio = None;
            }
            if last_tick.elapsed() >= Duration::from_secs(1) {
                last_tick = Instant::now();
                if mtime() != cfg_time {
                    cfg_time = mtime();
                    cfg = Config::load();
                }
                let min = unsafe { libc::time(std::ptr::null_mut()) / 60 } as i32;
                if cfg.sync_clock && min != last_min {
                    last_min = min;
                    let _ = apply::clock_now(&mut dev);
                }
                if cfg.sync_volume {
                    let v = default_sink_volume();
                    if v.is_some() && v != last_vol {
                        last_vol = v;
                        let _ = dev.transact(&p::volume(v.unwrap()));
                    }
                }
                while presses.front().is_some_and(|t| t.elapsed() > Duration::from_secs(60)) {
                    presses.pop_front();
                }
                let s = stats.sample(&cfg.net());
                loads = [s[0], s[1], s[4], s[3]].map(|v| v as f32 / 100.0);
                if cfg.sync_pcinfo {
                    for (kind, v) in s.iter().enumerate() {
                        let _ = dev.transact(&p::pc_info(kind as u8, *v));
                    }
                    let _ = dev.transact(&p::pc_info(5, presses.len().min(999) as u16));
                }
            }
        }
        eprintln!("Everest Max déconnecté");
    }
}
