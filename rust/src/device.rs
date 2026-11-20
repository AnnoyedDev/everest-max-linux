use crate::protocol::{self as p, Report};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use crate::i18n::t;

pub type Result<T> = std::result::Result<T, String>;

const REPLY_TIMEOUT: Duration = Duration::from_millis(600);

pub fn find_hidraw() -> Result<String> {
    let mut devs: Vec<_> = std::fs::read_dir("/sys/class/hidraw").map_err(|e| e.to_string())?.flatten().collect();
    devs.sort_by_key(|d| d.file_name());
    for d in devs {
        let base = d.path().join("device");
        let uevent = std::fs::read_to_string(base.join("uevent")).unwrap_or_default();
        let desc = std::fs::read(base.join("report_descriptor")).unwrap_or_default();
        if uevent.contains("HID_ID=0003:00003282:00000001") && desc.starts_with(&[0x06, 0x00, 0xFF]) {
            return Ok(format!("/dev/{}", d.file_name().to_string_lossy()));
        }
    }
    Err(t("Everest Max introuvable (branché ? règle udev installée ?)").into())
}

pub struct Device {
    f: File,
    pending: VecDeque<Report>,
}

impl Device {
    pub fn open() -> Result<Device> {
        let path = find_hidraw()?;
        let f = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open(&path)
            .map_err(|e| tf!("{}: {} — installez 70-mountain-everest.rules", path, e))?;
        Ok(Device { f, pending: VecDeque::new() })
    }

    pub fn read(&mut self, timeout: Duration) -> Option<Report> {
        if let Some(r) = self.pending.pop_front() {
            return Some(r);
        }
        self.read_raw(timeout)
    }

    fn read_raw(&mut self, timeout: Duration) -> Option<Report> {
        let mut pfd = libc::pollfd { fd: self.f.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        if unsafe { libc::poll(&mut pfd, 1, timeout.as_millis() as i32) } <= 0 {
            return None;
        }
        let mut r = [0u8; 64];
        match self.f.read(&mut r) {
            Ok(n) if n > 0 => Some(r),
            _ => None,
        }
    }

    pub fn transact(&mut self, report: &Report) -> Result<Report> {
        while let Some(r) = self.read_raw(Duration::ZERO) {
            self.keep(r);
        }
        let mut buf = [0u8; 65];
        buf[1..].copy_from_slice(report);
        self.f.write_all(&buf).map_err(|e| tf!("écriture: {}", e))?;
        let timeout = match (report[0], report[1]) {
            (0x14, 0x00) => Duration::from_millis(950),
            (0x13, 0x55) => Duration::from_millis(1300),
            (0x11, 0x14) if report[3] == 1 => Duration::from_millis(2800),
            _ => REPLY_TIMEOUT,
        };
        let end = Instant::now() + timeout;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            let Some(r) = self.read_raw(left) else { break };
            if r[0] == 0xFF && r[1] == 0xAA {
                return Err(tf!("commande {} {} refusée par le clavier", format!("{:02x}", report[0]), format!("{:02x}", report[1])));
            }
            if r[0] == report[0] {
                return Ok(r);
            }
            self.keep(r);
        }
        Err(tf!("pas de réponse à {} {}", format!("{:02x}", report[0]), format!("{:02x}", report[1])))
    }

    fn keep(&mut self, r: Report) {
        if (1..=10).contains(&r[0]) && self.pending.len() < 256 {
            self.pending.push_back(r);
        }
    }

    pub fn send_all(&mut self, reports: &[Report]) -> Result<()> {
        reports.iter().try_for_each(|r| self.transact(r).map(|_| ()))
    }

    fn feature(&mut self, data: &[u8; 64], get: bool) -> Result<[u8; 64]> {
        let mut buf = [0u8; 65];
        buf[1..].copy_from_slice(data);
        let nr = if get { 0x07 } else { 0x06 };
        let req = (3u64 << 30) | (65u64 << 16) | (0x48 << 8) | nr;
        let rc = unsafe { libc::ioctl(self.f.as_raw_fd(), req as _, buf.as_mut_ptr()) };
        if rc < 0 {
            return Err(tf!("feature report: {}", std::io::Error::last_os_error()));
        }
        let mut out = [0u8; 64];
        out.copy_from_slice(&buf[1..]);
        Ok(out)
    }

    fn feature_cmd(&mut self, data: &[u8; 64], wait: Duration) -> Result<(u8, [u8; 64])> {
        self.feature(data, false)?;
        std::thread::sleep(wait);
        let r = self.feature(&[0; 64], true)?;
        Ok((r[3], r))
    }

    pub fn set_effect(&mut self, profile: u8, e: &p::Effect, save: bool) -> Result<()> {
        let s = p::slot(e.effect);
        self.transact(&p::switch_profile(profile, s))?;
        if let Some(r) = p::effect(e) {
            self.transact(&r)?;
        }
        if save {
            self.transact(&p::save_flash(s))?;
        }
        Ok(())
    }

    pub fn set_custom(&mut self, profile: u8, colors: &[[u8; 3]; p::CUSTOM_KEYS], brightness: u8, save: bool) -> Result<()> {
        let s = p::slot(p::CUSTOM);
        self.transact(&p::switch_profile(profile, s))?;
        let k = brightness.min(100) as u16;
        let scaled = colors.map(|c| c.map(|v| (v as u16 * k / 100) as u8));
        self.send_all(&p::custom_static(&scaled))?;
        if save {
            self.transact(&p::save_flash(s))?;
        }
        Ok(())
    }

    pub fn extend_info(&mut self) -> Result<[u8; 29]> {
        let r = self.transact(&p::get_extend())?;
        let mut info = [0u8; 29];
        info.copy_from_slice(&r[4..33]);
        Ok(info)
    }

    pub fn set_extend_info(&mut self, info: &[u8; 29]) -> Result<()> {
        self.transact(&p::set_extend(info)).map(|_| ())
    }

    pub fn set_macro(&mut self, src: u8, mode: u8, events: &[p::MacroEvent]) -> Result<()> {
        self.transact(&p::macro_header(mode, 1))?;
        self.send_all(&p::macro_chunks(src, events))
    }

    pub fn upload_picture(&mut self, data: &[u8], dev: u8, idx: u8, sub: u8, progress: &mut dyn FnMut(f32)) -> Result<()> {
        let (w, h) = p::pic_dims(dev, idx).ok_or(t("cible d'image invalide"))?;
        if data.len() != (w * h * 2) as usize {
            return Err(tf!("image de {} octets, {}×{} attendu", data.len(), w, h));
        }
        let mut last = String::new();
        for _attempt in 0..2 {
            match self.try_upload(data, dev, idx, sub, progress) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last = e;
                    let _ = self.transact(&p::exit_transfer());
                }
            }
        }
        Err(last)
    }

    fn try_upload(&mut self, data: &[u8], dev: u8, idx: u8, sub: u8, progress: &mut dyn FnMut(f32)) -> Result<()> {
        let kind = 3 + dev;
        let mut data = data.to_vec();
        let mut ck = p::checksum(&data);
        if ck == 0 {
            data[0] = data[0].wrapping_add(1);
            ck = 1;
        }
        let status_ok = |s: u8, r: &[u8; 64], cmd: u8| s == 0xFA && r[2] == cmd;
        let mut ok = false;
        for _ in 0..10 {
            let (s, r) = self.feature_cmd(&p::pic_handshake(kind), Duration::from_millis(1))?;
            if status_ok(s, &r, 0x21) {
                ok = true;
                break;
            }
            if s == 0xFE {
                return Err(t("le clavier refuse le mode transfert").into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !ok {
            return Err(t("pas de réponse au mode transfert").into());
        }
        let start = if dev == 1 && idx == 6 { p::pic_start(data.len() as u32, ck, 3, sub, 0) } else { p::pic_start(data.len() as u32, ck, 2, idx, sub) };
        ok = false;
        for _ in 0..10 {
            let (s, r) = self.feature_cmd(&start, Duration::from_millis(110))?;
            std::thread::sleep(Duration::from_millis(120));
            if status_ok(s, &r, 0x10) {
                ok = true;
                break;
            }
            if s == 0xFE {
                return Err(t("début de transfert refusé").into());
            }
        }
        if !ok {
            return Err(t("début de transfert sans réponse").into());
        }
        let chunks: Vec<&[u8]> = data.chunks(64).collect();
        for (i, c) in chunks.iter().enumerate() {
            let mut buf = [0u8; 64];
            buf[..c.len()].copy_from_slice(c);
            let last = i + 1 == chunks.len();
            let sent = (i * 64 + c.len()) as u32;
            let mut tries = 0;
            loop {
                let wait = if last { Duration::from_millis(610 + 600 * (data.len() as u64 >> 16)) } else { Duration::from_millis(1) };
                let (s, r) = self.feature_cmd(&buf, wait)?;
                let got = r[4] as u32 | (r[5] as u32) << 8 | (r[6] as u32) << 16;
                match s {
                    0xFA | 0xFC if s == 0xFC || got == sent => {
                        if last && s == 0xFA && (r[7] as u16 | (r[8] as u16) << 8) != ck {
                            return Err(t("somme de contrôle refusée").into());
                        }
                        break;
                    }
                    0xFA if got > sent => return Err(t("décalage du transfert").into()),
                    0xFE if !last => return Err(t("bloc refusé").into()),
                    _ => {}
                }
                tries += 1;
                if tries > 200 {
                    return Err(tf!("bloc {} : pas d'acquittement", i));
                }
                std::thread::sleep(Duration::from_millis(if s == 0xFE { 50 } else { 2 }));
            }
            progress((i + 1) as f32 / chunks.len() as f32);
        }
        Ok(())
    }
}
