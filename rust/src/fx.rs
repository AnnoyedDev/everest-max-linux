use crate::keys;
use crate::protocol::{Effect, CUSTOM_KEYS, FIRMWARE_EFFECTS};
use std::f32::consts::TAU;

pub const SOFT: [(&str, &str); 12] = [
    ("Dégradé", "fx-gradient"),
    ("Arc-en-ciel", "fx-rainbow"),
    ("Aurore", "fx-aurora"),
    ("Feu", "fire"),
    ("Pluie", "fx-rain"),
    ("Étoiles", "star"),
    ("Spectre audio", "fx-spectrum"),
    ("Pulsation audio", "music"),
    ("Charge CPU", "cpu"),
    ("Charge GPU", "gpu"),
    ("Charge RAM", "ram"),
    ("Charge internet", "globe"),
];

pub const BANDS: usize = 16;

#[derive(Clone, Copy, Default)]
pub struct Inputs {
    pub bands: [f32; BANDS],
    pub level: f32,
    pub cpu: f32,
    pub gpu: f32,
    pub ram: f32,
    pub net: f32,
}

pub fn soft_index(effect: usize) -> Option<usize> {
    effect.checked_sub(FIRMWARE_EFFECTS).filter(|&i| i < SOFT.len())
}

pub fn uses_audio(effect: usize) -> bool {
    matches!(soft_index(effect), Some(6 | 7))
}

pub fn demo_inputs(t: f32) -> Inputs {
    let mut bands = [0.0; BANDS];
    for (i, b) in bands.iter_mut().enumerate() {
        let f = i as f32 / BANDS as f32;
        *b = ((t * (2.0 + f * 3.0) + f * 9.0).sin() * 0.5 + 0.5) * (1.0 - f * 0.55) * (0.6 + 0.4 * (t * 1.7).sin().abs());
    }
    let wave = |f: f32, o: f32| (t * f + o).sin() * 0.35 + 0.5;
    Inputs { bands, level: ((t * 4.0).sin() * 0.5 + 0.5).powf(2.0), cpu: wave(0.3, 0.0), gpu: wave(0.45, 1.0), ram: wave(0.12, 2.0), net: wave(0.8, 3.0) }
}

fn hash(x: f32) -> f32 {
    ((x * 127.1 + 311.7).sin() * 43758.547).fract().abs()
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

pub fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|u| ((u + m) * 255.0) as u8)
}

fn fire(heat: f32) -> [u8; 3] {
    let h = heat.clamp(0.0, 1.0);
    if h < 0.35 {
        mix([0, 0, 0], [180, 10, 0], h / 0.35)
    } else if h < 0.7 {
        mix([180, 10, 0], [255, 110, 0], (h - 0.35) / 0.35)
    } else {
        mix([255, 110, 0], [255, 230, 120], (h - 0.7) / 0.3)
    }
}

fn pixel(i: usize, e: &Effect, m: usize, x: f32, y: f32, t: f32, inp: &Inputs) -> [u8; 3] {
    let sp = 0.25 + e.speed as f32 / 100.0 * 2.0;
    let alt = e.color2.unwrap_or(e.background);
    let accent = |h: f32| if e.rainbow { hsv(h, 1.0, 1.0) } else { e.color };
    match i {
        0 => if e.rainbow { hsv(x * 0.85, 1.0, 1.0) } else { mix(e.color, alt, x) },
        1 => hsv((x + y * 0.35) * 0.8 - t * sp * 0.15, 1.0, 1.0),
        2 => {
            let n = (x * 3.0 + t * 0.5 * sp).sin() + (x * 7.0 - t * 0.3 * sp + y * 2.0).sin();
            let h = 0.38 + 0.16 * (n + y * 1.5 + t * 0.2).sin();
            let v = 0.25 + 0.75 * (((x * 4.0 + y * 3.0 + t * sp * 0.7).sin() + 1.0) / 2.0) * (1.0 - y * 0.35);
            hsv(h, 0.85, v)
        }
        3 => {
            let flicker = (x * 13.0 + t * sp * 7.0).sin() * (x * 5.0 - t * sp * 3.0 + y * 4.0).sin();
            let heat = y * 0.95 - 0.1 + 0.35 * flicker + 0.15 * hash(x * 40.0 + (t * sp * 10.0).floor());
            if e.rainbow { mix([0, 0, 0], hsv(t * 0.05 + x * 0.2, 1.0, 1.0), heat) } else { fire(heat) }
        }
        4 => {
            let col = (x * 24.0).floor();
            let rate = 0.6 + hash(col) * 0.8;
            let head = (t * sp * 0.35 * rate + hash(col * 1.7)).fract() * 1.5 - 0.25;
            let d = head - y;
            let v = if (0.0..0.45).contains(&d) { (1.0 - d / 0.45).powf(2.0) } else { 0.0 };
            mix(e.background, accent(col / 24.0), v)
        }
        5 => {
            let phase = hash(m as f32 * 1.3);
            let rate = 0.3 + hash(m as f32 * 2.1);
            let v = (t * sp * rate * 2.0 + phase * TAU).sin().max(0.0).powf(12.0);
            mix(e.background, accent(hash(m as f32 * 3.7)), v)
        }
        6 => {
            let band = ((x * BANDS as f32) as usize).min(BANDS - 1);
            let h = inp.bands[band];
            let fill = 1.0 - y;
            if fill <= h + 0.05 {
                if e.rainbow { hsv(x * 0.8, 1.0, 1.0) } else { mix(e.color, e.color2.unwrap_or([255, 40, 40]), fill) }
            } else {
                e.background
            }
        }
        7 => {
            let d = (((x - 0.5) * 2.2).powi(2) + (y - 0.5).powi(2)).sqrt();
            let v = (inp.level * 1.4 - d * 0.8).clamp(0.0, 1.0).max(0.12);
            mix(e.background, accent(t * 0.08 + d * 0.3), v)
        }
        8..=11 => {
            let load = [inp.cpu, inp.gpu, inp.ram, inp.net][i - 8];
            if x <= load {
                mix(e.color, e.color2.unwrap_or([255, 40, 40]), x)
            } else {
                mix(e.background, e.color, 0.06)
            }
        }
        _ => [0, 0, 0],
    }
}

pub fn frame(effect: usize, e: &Effect, t: f32, inp: &Inputs) -> [[u8; 3]; CUSTOM_KEYS] {
    let Some(i) = soft_index(effect) else { return [[0; 3]; CUSTOM_KEYS] };
    let pos = keys::positions();
    let k = e.brightness.min(100) as f32 / 100.0;
    std::array::from_fn(|m| {
        let (x, y) = pos[m];
        pixel(i, e, m, x, y, t, inp).map(|v| (v as f32 * k) as u8)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_soft_effect_lights_something() {
        let inp = demo_inputs(1.3);
        for i in 0..SOFT.len() {
            let e = Effect { effect: FIRMWARE_EFFECTS + i, color: [0, 120, 255], background: [0, 0, 0], ..Default::default() };
            let f = frame(e.effect, &e, 1.3, &inp);
            assert!(f.iter().any(|c| c.iter().any(|&v| v > 20)), "{}", SOFT[i].0);
        }
    }
}
