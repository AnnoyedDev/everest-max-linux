use crate::config::{Action, Config, DisplayKey};
use crate::device::{self, Device, Result};
use crate::i18n::t;
use crate::protocol::{self as p, Effect, PAD_KEYS};
use crate::{apply, images};
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    MacroPad,
    DisplayPad,
}

impl Kind {
    pub fn pid(self) -> u16 {
        match self {
            Kind::MacroPad => 8,
            Kind::DisplayPad => 9,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::MacroPad => "MacroPad",
            Kind::DisplayPad => "DisplayPad",
        }
    }
}

pub const BITS: [(usize, u8); PAD_KEYS] = [(42, 1), (42, 2), (42, 3), (42, 4), (42, 5), (42, 6), (42, 7), (47, 0), (47, 1), (47, 2), (47, 3), (47, 4)];
pub const ICON: u32 = 102;
const HEADER: usize = 306;
const PAYLOAD: usize = 31744;
const PIXEL_EP: u32 = 0x02;
const PIXEL_IFACE: u32 = 1;

pub fn connected(k: Kind) -> bool {
    device::find(k.pid()).is_some()
}

pub fn open(k: Kind) -> Result<Device> {
    let node = device::find(k.pid()).ok_or_else(|| tf!("{} introuvable (branché ? règle udev à jour ?)", k.name()))?;
    let mut d = Device::open_path(&node.dev)?;
    if k == Kind::DisplayPad {
        enable_events();
        let echo = p::pad_init();
        let ok = (0..20).any(|_| d.send(&echo).is_ok() && d.wait_for(Duration::from_millis(500), |r| r[..5] == echo[..5]).is_some());
        if !ok {
            return Err(t("le DisplayPad ne répond pas").into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(d)
}

fn enable_events() {
    let Some(n) = device::nodes(Kind::DisplayPad.pid(), false).into_iter().find(|n| n.iface == 0) else { return };
    let Ok(f) = OpenOptions::new().write(true).open(&n.dev) else { return };
    let mut buf = [0x03u8, 0x01];
    let req = (3u64 << 30) | (2u64 << 16) | (0x48 << 8) | 0x0B;
    unsafe { libc::ioctl(f.as_raw_fd(), req as _, buf.as_mut_ptr()) };
}

pub fn pressed(r: &[u8; 64], i: usize) -> bool {
    let (byte, bit) = BITS[i];
    r[byte] >> bit & 1 == 1
}

#[repr(C)]
struct DisconnectClaim {
    iface: u32,
    flags: u32,
    driver: [u8; 256],
}

#[repr(C)]
struct Bulk {
    ep: u32,
    len: u32,
    timeout: u32,
    data: *mut libc::c_void,
}

const USBDEVFS_BULK: u64 = 0xC018_5502;
const USBDEVFS_DISCONNECT_CLAIM: u64 = 0x8108_551B;

fn claim_pixels(node: &device::Node) -> Result<File> {
    let num = |f: &str| std::fs::read_to_string(node.usb.join(f)).ok().and_then(|s| s.trim().parse::<u32>().ok());
    let (Some(bus), Some(dev)) = (num("busnum"), num("devnum")) else { return Err(t("périphérique USB introuvable").into()) };
    let path = format!("/dev/bus/usb/{bus:03}/{dev:03}");
    let f = OpenOptions::new().read(true).write(true).open(&path).map_err(|e| tf!("{}: {} — règle udev à mettre à jour (Système)", path, e))?;
    let mut dc = DisconnectClaim { iface: PIXEL_IFACE, flags: 0, driver: [0; 256] };
    if unsafe { libc::ioctl(f.as_raw_fd(), USBDEVFS_DISCONNECT_CLAIM as _, &mut dc) } < 0 {
        return Err(tf!("interface d'images occupée : {}", std::io::Error::last_os_error()));
    }
    Ok(f)
}

fn bulk(f: &File, data: &[u8]) -> Result<()> {
    let mut b = Bulk { ep: PIXEL_EP, len: data.len() as u32, timeout: 2000, data: data.as_ptr() as *mut _ };
    let n = unsafe { libc::ioctl(f.as_raw_fd(), USBDEVFS_BULK as _, &mut b) };
    if n < 0 {
        return Err(tf!("envoi des pixels : {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

pub fn key_image(dk: &DisplayKey) -> Result<image::RgbImage> {
    images::render(ICON, ICON, &apply::art(dk))
}

fn payload(img: &image::RgbImage) -> Vec<u8> {
    let mut v = vec![0u8; PAYLOAD];
    for (i, px) in img.pixels().enumerate() {
        v[HEADER + i * 3..HEADER + i * 3 + 3].copy_from_slice(&[px[2], px[1], px[0]]);
    }
    v
}

fn upload(d: &mut Device, usb: &File, key: u8, img: &image::RgbImage) -> Result<()> {
    d.send(&p::pad_image(key))?;
    d.wait_for(Duration::from_secs(2), |r| r[..3] == [0x21, 0, 0]).ok_or_else(|| tf!("touche {} : le DisplayPad n'est pas prêt", key + 1))?;
    payload(img).chunks(1024).try_for_each(|c| bulk(usb, c))?;
    d.wait_for(Duration::from_secs(3), |r| r[..3] == [0x21, 0, 0xFF]).map(|_| ()).ok_or_else(|| tf!("touche {} : transfert non confirmé", key + 1))
}

fn upload_many(d: &mut Device, items: &[(u8, image::RgbImage)], progress: &dyn Fn(f32)) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let node = device::find(Kind::DisplayPad.pid()).ok_or(t("DisplayPad débranché"))?;
    let usb = claim_pixels(&node)?;
    for (n, (key, img)) in items.iter().enumerate() {
        upload(d, &usb, *key, img)?;
        progress((n + 1) as f32 / items.len() as f32);
    }
    Ok(())
}

pub fn push_display(d: &mut Device, keys: &[DisplayKey; PAD_KEYS], brightness: u8, progress: &dyn Fn(f32)) -> Result<()> {
    d.send(&p::pad_brightness(brightness))?;
    let items = keys.iter().enumerate().map(|(i, dk)| Ok((i as u8, key_image(dk)?))).collect::<Result<Vec<_>>>()?;
    upload_many(d, &items, progress)
}

pub fn split(path: &str, page: usize) -> Result<Vec<String>> {
    let img = image::open(path).map_err(|e| format!("{path}: {e}"))?;
    let (w, h) = (ICON * 6, ICON * 2);
    let img = img.resize_to_fill(w, h, image::imageops::FilterType::Lanczos3);
    let dir = crate::config::path().with_file_name("displaypad");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    (0..PAD_KEYS as u32).map(|i| {
        let tile = img.crop_imm(i % 6 * ICON, i / 6 * ICON, ICON, ICON);
        let out = dir.join(format!("p{page}-k{}-{stamp}.png", i + 1));
        tile.save(&out).map_err(|e| e.to_string())?;
        Ok(out.to_string_lossy().into_owned())
    }).collect()
}

struct Anim {
    key: u8,
    frames: Vec<(image::RgbImage, Duration)>,
    at: usize,
    next: Instant,
}

fn anims(keys: &[DisplayKey; PAD_KEYS]) -> Vec<Anim> {
    keys.iter().enumerate().filter_map(|(i, dk)| {
        let frames = images::gif_frames(&dk.image)?;
        let frames: Vec<_> = frames.into_iter().filter_map(|(f, t)| images::compose(ICON, ICON, &apply::art(dk), Some(&f)).ok().map(|img| (img, t))).collect();
        Some(Anim { key: i as u8, next: Instant::now() + frames.first()?.1, frames, at: 0 })
    }).collect()
}

fn show(d: &mut Device, cfg: &Config, page: usize, shown: &mut Option<[DisplayKey; PAD_KEYS]>, anim: &mut Vec<Anim>) {
    let keys = cfg.displaypad.page(page);
    let items: Vec<(u8, image::RgbImage)> = keys.iter().enumerate()
        .filter(|(i, dk)| shown.as_ref().is_none_or(|s| s[*i] != **dk))
        .filter_map(|(i, dk)| key_image(dk).ok().map(|img| (i as u8, img)))
        .collect();
    if items.is_empty() {
        return;
    }
    if let Err(e) = upload_many(d, &items, &|_| {}) {
        eprintln!("DisplayPad : {e}");
        return;
    }
    *shown = Some(keys.clone());
    *anim = anims(keys);
}

fn step(d: &mut Device, anim: &mut [Anim]) {
    let now = Instant::now();
    let mut due = Vec::new();
    for a in anim.iter_mut().filter(|a| a.next <= now) {
        a.at = (a.at + 1) % a.frames.len();
        a.next = now + a.frames[a.at].1;
        due.push((a.key, a.frames[a.at].0.clone()));
    }
    if let Err(e) = upload_many(d, &due, &|_| {}) {
        eprintln!("DisplayPad : {e}");
    }
}

pub fn macropad_lighting(d: &mut Device, e: &Effect, colors: &[[u8; 3]; PAD_KEYS], save: bool) -> Result<()> {
    let s = p::slot(e.effect);
    d.command_loose(&p::switch_profile(1, s))?;
    if e.effect == p::CUSTOM {
        for r in p::pad_custom(e.brightness, colors) {
            d.command_loose(&r)?;
        }
    } else if let Some(r) = p::pad_effect(e) {
        d.command_loose(&r)?;
    }
    if save {
        d.command_loose(&p::save_flash(s))?;
    }
    Ok(())
}

pub fn listen(k: Kind, tx: mpsc::Sender<Action>) {
    loop {
        if !connected(k) {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        let mut d = match open(k) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{e}");
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        eprintln!("{} connecté", k.name());
        let mut cfg = Config::load();
        let mut cfg_time = crate::config::mtime();
        let display = k == Kind::DisplayPad;
        let (mut page, mut shown, mut anim) = (0usize, None, Vec::new());
        let mut brightness = cfg.displaypad.brightness;
        if display {
            let _ = d.send(&p::pad_brightness(brightness));
            show(&mut d, &cfg, page, &mut shown, &mut anim);
        }
        let mut prev = [0u8; 64];
        let mut tick = Instant::now();
        loop {
            let wait = anim.iter().map(|a| a.next.saturating_duration_since(Instant::now())).min().unwrap_or(Duration::from_millis(500)).min(Duration::from_millis(500));
            if let Some(r) = d.read(wait) {
                if r[0] == 0x01 {
                    for i in (0..PAD_KEYS).filter(|&i| pressed(&r, i) && !pressed(&prev, i)) {
                        let a = if display { cfg.displaypad.page(page)[i].action.clone() } else { cfg.macropad.actions[i].clone() };
                        match a {
                            Action::Page { page: to } if display && to <= cfg.displaypad.pages.len() => {
                                page = to;
                                show(&mut d, &cfg, page, &mut shown, &mut anim);
                            }
                            a => {
                                let _ = tx.send(a);
                            }
                        }
                    }
                    prev = r;
                }
            }
            if !anim.is_empty() {
                step(&mut d, &mut anim);
            }
            if tick.elapsed() >= Duration::from_secs(1) {
                tick = Instant::now();
                if !connected(k) {
                    break;
                }
                if crate::config::mtime() != cfg_time {
                    cfg_time = crate::config::mtime();
                    cfg = Config::load();
                    if display {
                        page = page.min(cfg.displaypad.pages.len());
                        if cfg.displaypad.brightness != brightness {
                            brightness = cfg.displaypad.brightness;
                            let _ = d.send(&p::pad_brightness(brightness));
                        }
                        show(&mut d, &cfg, page, &mut shown, &mut anim);
                    }
                }
            }
        }
        eprintln!("{} déconnecté", k.name());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn key_bits_and_payload_layout() {
        let mut r = [0u8; 64];
        r[42] = 0b1000_0010;
        r[47] = 0b1_0000;
        let on: Vec<usize> = (0..super::PAD_KEYS).filter(|&i| super::pressed(&r, i)).collect();
        assert_eq!(on, [0, 6, 11]);
        let img = image::RgbImage::from_pixel(super::ICON, super::ICON, image::Rgb([1, 2, 3]));
        let v = super::payload(&img);
        assert_eq!(v.len(), 31 * 1024);
        assert_eq!(&v[super::HEADER - 1..super::HEADER + 3], &[0, 3, 2, 1]);
    }
}
