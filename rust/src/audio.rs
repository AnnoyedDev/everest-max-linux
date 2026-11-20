use crate::fx::BANDS;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const RATE: f32 = 22050.0;
const WINDOW: usize = 512;
pub const ALL_SOUNDS: &str = "@DEFAULT_MONITOR@";

type Level = Arc<Mutex<([f32; BANDS], f32)>>;

pub struct Audio {
    children: Vec<Child>,
    states: Vec<Level>,
}

pub struct Source {
    pub name: String,
    pub label: String,
    pub output: bool,
}

pub fn sources() -> Vec<Source> {
    let out = Command::new("pactl").args(["-f", "json", "list", "sources"]).output();
    let Ok(out) = out else { return vec![] };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    v.as_array().into_iter().flatten()
        .filter_map(|s| {
            let name = s["name"].as_str()?.to_string();
            let desc = s["description"].as_str().unwrap_or(&name).to_string();
            let output = name.ends_with(".monitor");
            let label = if output { desc.trim_start_matches("Monitor of ").to_string() } else { desc };
            Some(Source { name, label, output })
        })
        .collect()
}

fn goertzel(samples: &[f32], freq: f32) -> f32 {
    let w = std::f32::consts::TAU * freq / RATE;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &x in samples {
        let s0 = x + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt() / samples.len() as f32
}

fn capture(source: &str) -> Option<(Child, Level)> {
    let mut child = Command::new("parec")
        .args(["-d", source, "--format=s16le", "--rate=22050", "--channels=1", "--latency-msec=20", "--raw"])
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let state: Level = Arc::new(Mutex::new(([0.0; BANDS], 0.0)));
    let st = state.clone();
    std::thread::spawn(move || {
        let freqs: Vec<f32> = (0..BANDS).map(|i| 50.0 * (8000.0f32 / 50.0).powf(i as f32 / (BANDS - 1) as f32)).collect();
        let mut buf = vec![0u8; WINDOW * 2];
        let mut peak = 0.02f32;
        while out.read_exact(&mut buf).is_ok() {
            let samples: Vec<f32> = buf.chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
            let rms = (samples.iter().map(|s| s * s).sum::<f32>() / WINDOW as f32).sqrt();
            let mags: Vec<f32> = freqs.iter().map(|&f| goertzel(&samples, f)).collect();
            let top = mags.iter().cloned().fold(0.0, f32::max);
            peak = (peak * 0.995).max(top).max(0.004);
            let mut s = st.lock().unwrap();
            for (b, m) in s.0.iter_mut().zip(&mags) {
                let v = (m / peak).sqrt().clamp(0.0, 1.0);
                *b = if v > *b { v } else { *b * 0.82 };
            }
            let lv = (rms * 6.0).clamp(0.0, 1.0);
            s.1 = if lv > s.1 { lv } else { s.1 * 0.85 };
        }
    });
    Some((child, state))
}

impl Audio {
    pub fn start(sources: &[String]) -> Option<Audio> {
        let list: Vec<&str> = if sources.is_empty() { vec![ALL_SOUNDS] } else { sources.iter().map(String::as_str).collect() };
        let (children, states): (Vec<_>, Vec<_>) = list.iter().filter_map(|s| capture(s)).unzip();
        (!children.is_empty()).then_some(Audio { children, states })
    }

    pub fn get(&self) -> ([f32; BANDS], f32) {
        let mut out: ([f32; BANDS], f32) = ([0.0; BANDS], 0.0);
        for st in &self.states {
            let s = st.lock().unwrap();
            for (o, v) in out.0.iter_mut().zip(s.0.iter()) {
                *o = (*o).max(*v);
            }
            out.1 = out.1.max(s.1);
        }
        out
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        for c in &mut self.children {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
