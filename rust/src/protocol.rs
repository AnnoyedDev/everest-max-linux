pub type Report = [u8; 64];

pub fn pkt(head: &[u8], data: &[u8]) -> Report {
    let mut r = [0u8; 64];
    r[..head.len()].copy_from_slice(head);
    r[head.len()..head.len() + data.len()].copy_from_slice(data);
    r
}

pub const EFFECTS: [(&str, u8, u8); 9] = [
    ("Statique", 0, 0),
    ("Vague", 4, 1),
    ("Tornade", 7, 2),
    ("Respiration", 1, 3),
    ("Réactif", 3, 4),
    ("Matrix", 9, 5),
    ("Personnalisé", 10, 6),
    ("Yeti", 6, 7),
    ("Éteint", 12, 8),
];
pub const CUSTOM: usize = 6;
pub const CUSTOM_KEYS: usize = 126;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Effect {
    pub effect: usize,
    pub brightness: u8,
    pub speed: u8,
    pub direction: u8,
    pub color: [u8; 3],
    pub color2: Option<[u8; 3]>,
    pub background: [u8; 3],
    pub rainbow: bool,
    #[serde(default)]
    pub variant: u8,
}

impl Default for Effect {
    fn default() -> Self {
        Effect { effect: 1, brightness: 100, speed: 50, direction: 0, color: [0, 0x44, 0xff], color2: None, background: [0; 3], rainbow: false, variant: 0 }
    }
}

pub const FIRMWARE_EFFECTS: usize = 9;

pub fn slot(effect: usize) -> u8 {
    if effect < FIRMWARE_EFFECTS { EFFECTS[effect].2 } else { EFFECTS[CUSTOM].2 }
}

pub fn switch_profile(profile: u8, slot: u8) -> Report {
    assert!((1..=5).contains(&profile) && slot <= 8);
    pkt(&[0x14, 0x00, 0, 0, profile, slot], &[])
}

pub fn save_flash(slot: u8) -> Report {
    pkt(&[0x13, 0x55, 0, 0, slot], &[])
}

pub fn speed_code(index: u8, speed: u8) -> u8 {
    let s = speed;
    match index {
        4 | 7 => match s { 0..=12 => 10, 13..=37 => 9, 38..=62 => 8, 63..=87 => 7, _ => 6 },
        1 | 3 | 5 | 11 => match s { 0..=12 => 5, 13..=37 => 4, 38..=62 => 3, 63..=87 => 1, _ => 0 },
        9 => match s { 0..=12 => 20, 13..=37 => 15, 38..=62 => 10, 63..=87 => 5, _ => 0 },
        6 => match s { 0..=12 => 10, 13..=37 => 7, 38..=62 => 5, 63..=87 => 3, _ => 0 },
        _ => s,
    }
}

fn eff_data(e: &Effect, index: u8) -> [u8; 62] {
    let mut d = [0u8; 62];
    d[0] = index;
    d[3] = e.brightness;
    d[5] = 0xFF;
    d[6] = 0xFF;
    d[7..10].copy_from_slice(&e.color);
    match index {
        0 => {
            d[2] = 0xFF;
            d[4] = if e.rainbow { 2 } else { 0 };
        }
        12 => {
            d[2] = 0xFF;
            d[4] = 0xFF;
            d[7..10].copy_from_slice(&[0; 3]);
        }
        6 => d[2] = speed_code(index, e.speed),
        _ => {
            d[2] = speed_code(index, e.speed);
            d[4] = if e.rainbow { 2 } else if e.color2.is_some() && index == 1 { 16 } else { 0 };
            if index == 1 {
                d[10..13].copy_from_slice(&e.color2.unwrap_or([0; 3]));
            }
        }
    }
    if matches!(index, 3 | 6 | 9 | 11) {
        d[16..19].copy_from_slice(&e.background);
    }
    d
}

fn block_data(e: &Effect) -> [u8; 62] {
    let index = EFFECTS[e.effect].1;
    let mut d = [0u8; 62];
    d[0] = index;
    d[2] = speed_code(index, e.speed);
    d[3] = e.brightness;
    d[5] = if index == 4 { [0, 4, 2, 6][(e.direction & 3) as usize] } else if e.direction % 2 == 0 { 10 } else { 9 };
    let stop = |d: &mut [u8; 62], n: usize, level: u8, c: [u8; 3]| {
        d[8 + n * 4] = level;
        d[9 + n * 4..12 + n * 4].copy_from_slice(&c);
    };
    if e.rainbow {
        d[4] = 2;
        d[6] = 2;
        d[8] = 0xFF;
        d[12] = 0xFF;
    } else if index == 4 && e.color2.is_some() {
        let c2 = e.color2.unwrap();
        d[6] = 2;
        d[7] = 4;
        for (n, (level, c)) in [(25, e.color), (50, c2), (75, e.color), (100, c2)].into_iter().enumerate() {
            stop(&mut d, n, level, c);
        }
    } else {
        d[7] = 1;
        stop(&mut d, 0, 100, e.color);
        d[12] = 0xFF;
    }
    d
}

pub fn effect(e: &Effect) -> Option<Report> {
    if e.effect >= FIRMWARE_EFFECTS {
        return None;
    }
    match EFFECTS[e.effect].1 {
        4 | 7 => Some(pkt(&[0x14, 0x2C], &block_data(e))),
        10 => None,
        _ => Some(pkt(&[0x14, 0x2C], &eff_data(e, EFFECTS[e.effect].1))),
    }
}

pub fn pad_effect(e: &Effect) -> Option<Report> {
    if e.effect >= FIRMWARE_EFFECTS || EFFECTS[e.effect].1 != 3 || e.variant == 0 {
        return effect(e);
    }
    if e.variant == 2 {
        return Some(pkt(&[0x14, 0x2C], &eff_data(e, 11)));
    }
    let mut d = [0u8; 62];
    d[0] = 5;
    d[2] = speed_code(5, e.speed);
    d[3] = e.brightness;
    d[4] = if e.rainbow { 2 } else if e.color2.is_some() { 16 } else { 0 };
    d[5] = 0xFF;
    d[6] = 2;
    if !e.rainbow {
        d[7] = 1;
        d[8] = 0xFF;
        d[9..12].copy_from_slice(&e.color);
        d[12] = 0xFF;
        if let Some(c2) = e.color2 {
            d[13..16].copy_from_slice(&c2);
        }
    }
    Some(pkt(&[0x14, 0x2C], &d))
}

pub fn custom_static(colors: &[[u8; 3]; CUSTOM_KEYS]) -> Vec<Report> {
    let flat: Vec<u8> = colors.iter().flatten().copied().collect();
    flat.chunks(57).enumerate().map(|(n, c)| pkt(&[0x14, 0x2C, 0x00, 0x01, n as u8, 0x4B, 0x00], c)).collect()
}

pub fn change_key(src: u8, value: u16) -> Report {
    pkt(&[0x14, 0x20, src, 0, value as u8, (value >> 8) as u8], &[])
}

pub const MOD_CTRL: u8 = 1;
pub const MOD_SHIFT: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_WIN: u8 = 8;

pub fn change_shortcut(src: u8, key: u8, mods: u8) -> Report {
    pkt(&[0x14, 0x21, src, 0, key, mods], &[])
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MacroEvent {
    pub key: u8,
    pub page: u8,
    pub action: u8,
    pub delay: u16,
}

pub fn macro_header(mode: u8, times: u8) -> Report {
    pkt(&[0x14, 0x18, mode, 0, times], &[])
}

pub fn macro_chunks(src: u8, events: &[MacroEvent]) -> Vec<Report> {
    let mut all: Vec<u8> = Vec::new();
    for e in events.iter().take(999) {
        all.extend_from_slice(&[e.key, e.page << 4 | (e.action & 15), e.delay as u8, (e.delay >> 8) as u8]);
    }
    all.extend_from_slice(&[0xFF, 0xFF, 0, 0]);
    all.chunks(60).enumerate().map(|(i, c)| pkt(&[0x15, src, (c.len() / 4) as u8, (i != 0) as u8], c)).collect()
}

pub fn clock(month: u8, day: u8, hour: u8, min: u8, sec: u8) -> Report {
    pkt(&[0x11, 0x84, 0x00, 0x01, 0, 0, month, day, hour, min, sec, 0, 0], &[])
}

pub fn volume(v: u8) -> Report {
    pkt(&[0x11, 0x83, 0, 0, v.min(100)], &[])
}

pub fn pc_info(kind: u8, value: u16) -> Report {
    pkt(&[0x11, 0x81, kind, 0, value as u8, (value >> 8) as u8], &[])
}

pub fn get_extend() -> Report {
    pkt(&[0x11, 0x14], &[])
}

pub fn set_extend(info: &[u8; 29]) -> Report {
    pkt(&[0x11, 0x14, 0x00, 0x01], info)
}

pub const EXT_SHOW_MENU: usize = 1;
pub const EXT_SCREEN_SETUP: usize = 6;
pub const EXT_SAVER: usize = 7;
pub const EXT_TURNOFF: usize = 9;
pub const EXT_PROFILES: usize = 11;

pub fn profile_table(enabled: [bool; 5]) -> [u8; 5] {
    let mut t: Vec<u8> = (1..=5u8).filter(|&n| enabled[n as usize - 1]).collect();
    if t.is_empty() {
        t.push(1);
    }
    let rest: Vec<u8> = (1..=5u8).filter(|n| !t.contains(n)).map(|n| 0x10 | n).collect();
    t.extend(rest);
    [t[0], t[1], t[2], t[3], t[4]]
}

pub fn profiles_enabled(table: &[u8]) -> [bool; 5] {
    let mut e = [false; 5];
    for &v in table.iter().take(5) {
        if v & 0xF0 == 0 && (1..=5).contains(&v) {
            e[v as usize - 1] = true;
        }
    }
    e
}

pub fn exit_transfer() -> Report {
    pkt(&[0x1D, 0x00], &[])
}

pub fn pic_handshake(kind: u8) -> [u8; 64] {
    pkt(&[0xAA, 0x55, 0x21, kind], &[])
}

pub fn pic_start(size: u32, checksum: u16, mode: u8, a: u8, b: u8) -> [u8; 64] {
    pkt(&[0xAA, 0x55, 0x10, size as u8, (size >> 8) as u8, (size >> 16) as u8, checksum as u8, (checksum >> 8) as u8, 0, 0, mode, a, b], &[])
}

pub fn checksum(data: &[u8]) -> u16 {
    data.iter().fold(0u16, |s, &b| s.wrapping_add(b as u16))
}

pub fn pic_dims(dev: u8, idx: u8) -> Option<(u32, u32)> {
    match (dev, idx) {
        (1, 1..=6) => Some((72, 72)),
        (0, 1) | (0, 7) => Some((240, 204)),
        (0, 2..=6) => Some((128, 32)),
        _ => None,
    }
}

pub const PAD_KEYS: usize = 12;

pub fn pad_init() -> Report {
    pkt(&[0x11, 0x80, 0, 0, 1], &[])
}

pub fn pad_brightness(percent: u8) -> Report {
    pkt(&[0x12, 0x03, 0, 0, percent.min(100)], &[])
}

pub fn pad_image(key: u8) -> Report {
    pkt(&[0x21, 0, 0, 0, key, 0x3D, 0, 0, 0x65, 0x65], &[])
}

pub fn pad_custom(brightness: u8, colors: &[[u8; 3]; PAD_KEYS]) -> [Report; 2] {
    let mut on = [0xFF; 64];
    on[..4].copy_from_slice(&[0x14, 0x2C, 10, 0]);
    on[5] = brightness.min(100);
    let flat: Vec<u8> = colors.iter().flatten().copied().collect();
    [on, pkt(&[0x14, 0x2C, 0, 1, 0, 0x4B, 0], &flat)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_packets_match_sdkdll() {
        assert_eq!(&switch_profile(2, 4)[..6], &[0x14, 0, 0, 0, 2, 4]);
        assert_eq!(&save_flash(6)[..5], &[0x13, 0x55, 0, 0, 6]);
        let wave = effect(&Effect { effect: 1, brightness: 75, speed: 100, direction: 3, ..Default::default() }).unwrap();
        assert_eq!(&wave[..15], &[0x14, 0x2C, 4, 0, 6, 75, 0, 6, 0, 1, 100, 0, 0x44, 0xFF, 0xFF]);
        let wave2 = effect(&Effect { effect: 1, color2: Some([1, 2, 3]), ..Default::default() }).unwrap();
        assert_eq!((wave2[8], wave2[9], wave2[10], wave2[14], wave2[18], wave2[22]), (2, 4, 25, 50, 75, 100));
        let rainbow = effect(&Effect { effect: 2, rainbow: true, direction: 1, ..Default::default() }).unwrap();
        assert_eq!(&rainbow[2..11], &[7, 0, 8, 100, 2, 9, 2, 0, 0xFF]);
        let stat = effect(&Effect { effect: 0, ..Default::default() }).unwrap();
        assert_eq!(&stat[2..9], &[0, 0, 0xFF, 100, 0, 0xFF, 0xFF]);
        let breath = effect(&Effect { effect: 3, speed: 50, color2: Some([9, 9, 9]), ..Default::default() }).unwrap();
        assert_eq!(&breath[2..7], &[1, 0, 3, 100, 16]);
        assert_eq!(&effect(&Effect { effect: 8, ..Default::default() }).unwrap()[2..9], &[12, 0, 255, 100, 255, 255, 255]);
        assert!(effect(&Effect { effect: CUSTOM, ..Default::default() }).is_none());
        assert_eq!(profile_table([true, false, true, false, false]), [1, 3, 0x12, 0x14, 0x15]);
        assert_eq!(profiles_enabled(&[1, 0x12, 0x13, 0x14, 0x15]), [true, false, false, false, false]);
        let mut c = [[0u8; 3]; CUSTOM_KEYS];
        c[0] = [0xFF, 0, 0];
        c[125] = [0, 0, 0xFF];
        let r = custom_static(&c);
        assert_eq!(r.len(), 7);
        assert_eq!(&r[0][..10], &[0x14, 0x2C, 0, 1, 0, 0x4B, 0, 0xFF, 0, 0]);
        assert_eq!(&r[6][7 + 33..7 + 36], &[0, 0, 0xFF]);
    }

    #[test]
    fn key_and_macro_packets() {
        assert_eq!(&change_key(110, 183)[..6], &[0x14, 0x20, 110, 0, 183, 0]);
        assert_eq!(&change_key(110, 257)[..6], &[0x14, 0x20, 110, 0, 1, 1]);
        assert_eq!(&change_shortcut(5, 39, MOD_WIN)[..6], &[0x14, 0x21, 5, 0, 39, 8]);
        assert_eq!(&macro_header(1, 1)[..5], &[0x14, 0x18, 1, 0, 1]);
        let ev: Vec<MacroEvent> = (0..16).map(|i| MacroEvent { key: i, page: 0, action: 2, delay: 300 }).collect();
        let r = macro_chunks(9, &ev);
        assert_eq!(r.len(), 2);
        assert_eq!(&r[0][..8], &[0x15, 9, 15, 0, 0, 0x02, 0x2C, 0x01]);
        assert_eq!(&r[1][..12], &[0x15, 9, 2, 1, 15, 0x02, 0x2C, 0x01, 0xFF, 0xFF, 0, 0]);
    }

    #[test]
    fn dock_and_picture_packets() {
        assert_eq!(&clock(11, 20, 9, 5, 7)[..13], &[0x11, 0x84, 0, 1, 0, 0, 11, 20, 9, 5, 7, 0, 0]);
        assert_eq!(&volume(150)[..5], &[0x11, 0x83, 0, 0, 100]);
        assert_eq!(&pic_start(10368, 0x1234, 2, 1, 3)[..13], &[0xAA, 0x55, 0x10, 0x80, 0x28, 0, 0x34, 0x12, 0, 0, 2, 1, 3]);
        assert_eq!(checksum(&[0xFF, 0xFF, 2]), 0x200);
        assert_eq!(pic_dims(1, 1), Some((72, 72)));
    }

    #[test]
    fn pad_packets() {
        assert_eq!(&pad_init()[..6], &[0x11, 0x80, 0, 0, 1, 0]);
        assert_eq!(&pad_brightness(75)[..5], &[0x12, 0x03, 0, 0, 75]);
        assert_eq!(&pad_image(4)[..11], &[0x21, 0, 0, 0, 4, 0x3D, 0, 0, 0x65, 0x65, 0]);
        let base = Effect { effect: 4, speed: 50, color: [9, 8, 7], ..Default::default() };
        assert_eq!(pad_effect(&base), effect(&base));
        let b = pad_effect(&Effect { variant: 1, color2: Some([1, 1, 1]), ..base.clone() }).unwrap();
        assert_eq!(&b[2..18], &[5, 0, 3, 100, 16, 0xFF, 2, 1, 0xFF, 9, 8, 7, 0xFF, 1, 1, 1]);
        assert_eq!(&pad_effect(&Effect { variant: 2, ..base }).unwrap()[2..5], &[11, 0, 3]);
        let mut c = [[0u8; 3]; PAD_KEYS];
        c[11] = [1, 2, 3];
        let [on, colors] = pad_custom(80, &c);
        assert_eq!(&on[..7], &[0x14, 0x2C, 10, 0, 0xFF, 80, 0xFF]);
        assert_eq!(&colors[..7], &[0x14, 0x2C, 0, 1, 0, 0x4B, 0]);
        assert_eq!(&colors[7 + 33..7 + 36], &[1, 2, 3]);
    }
}
