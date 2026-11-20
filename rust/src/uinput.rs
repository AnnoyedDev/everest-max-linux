use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const REL_WHEEL: u16 = 8;

pub fn key_code(name: &str) -> Option<u16> {
    let n = name.trim().to_lowercase();
    if n.len() == 1 {
        let c = n.chars().next()?;
        for (row, base) in [("1234567890", 2), ("qwertyuiop", 16), ("asdfghjkl", 30), ("zxcvbnm", 44)] {
            if let Some(i) = row.find(c) {
                return Some(base + i as u16);
            }
        }
    }
    if let Some(f) = n.strip_prefix('f').and_then(|x| x.parse::<u16>().ok()) {
        return match f {
            1..=10 => Some(58 + f),
            11 | 12 => Some(76 + f),
            13..=24 => Some(170 + f),
            _ => None,
        };
    }
    Some(match n.as_str() {
        "esc" | "echap" => 1, "-" | "minus" => 12, "=" | "equal" => 13, "backspace" => 14, "tab" => 15,
        "[" => 26, "]" => 27, "enter" | "entrée" => 28, "ctrl" | "lctrl" => 29, ";" => 39, "'" => 40, "`" => 41,
        "shift" | "lshift" => 42, "\\" => 43, "," => 51, "." => 52, "/" => 53, "rshift" => 54, "alt" | "lalt" => 56,
        "space" | "espace" => 57, "capslock" => 58, "rctrl" => 97, "altgr" | "ralt" => 100, "home" => 102, "up" => 103,
        "pageup" => 104, "left" => 105, "right" => 106, "end" => 107, "down" => 108, "pagedown" => 109, "insert" => 110,
        "delete" | "suppr" => 111, "mute" => 113, "volumedown" => 114, "volumeup" => 115, "power" => 116,
        "pause" => 119, "super" | "win" | "meta" => 125, "rsuper" => 126, "menu" => 139, "calc" => 140,
        "sleep" => 142, "nexttrack" | "next" => 163, "playpause" | "play" => 164, "previoustrack" | "prev" => 165,
        "stop" => 166, "print" | "printscreen" => 99, "mouseleft" => 0x110, "mouseright" => 0x111, "mousemiddle" => 0x112,
        "mouseback" => 0x113, "mouseforward" => 0x114,
        _ => return None,
    })
}

pub struct VirtualInput {
    f: File,
}

impl VirtualInput {
    pub fn new() -> Result<Self, String> {
        let f = OpenOptions::new().write(true).open("/dev/uinput").map_err(|e| tf!("/dev/uinput: {} (règle udev)", e))?;
        let fd = f.as_raw_fd();
        unsafe {
            libc::ioctl(fd, 0x40045564, EV_KEY as libc::c_int);
            libc::ioctl(fd, 0x40045564, EV_REL as libc::c_int);
            libc::ioctl(fd, 0x40045566, REL_WHEEL as libc::c_int);
            for k in 1..0x118 {
                libc::ioctl(fd, 0x40045565, k as libc::c_int);
            }
            let mut setup = [0u8; 92];
            setup[0..2].copy_from_slice(&3u16.to_ne_bytes());
            setup[2..4].copy_from_slice(&0x3282u16.to_ne_bytes());
            setup[4..6].copy_from_slice(&0xFFFFu16.to_ne_bytes());
            setup[8..8 + 22].copy_from_slice(b"Everest Base Camp (vk)");
            if libc::ioctl(fd, 0x405c5503, setup.as_ptr()) < 0 || libc::ioctl(fd, 0x5501) < 0 {
                return Err(format!("uinput: {}", std::io::Error::last_os_error()));
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        Ok(VirtualInput { f })
    }

    fn emit(&mut self, kind: u16, code: u16, value: i32) {
        let mut ev = [0u8; 24];
        ev[16..18].copy_from_slice(&kind.to_ne_bytes());
        ev[18..20].copy_from_slice(&code.to_ne_bytes());
        ev[20..24].copy_from_slice(&value.to_ne_bytes());
        let _ = self.f.write_all(&ev);
    }

    pub fn key(&mut self, code: u16, down: bool) {
        self.emit(EV_KEY, code, down as i32);
        self.emit(EV_SYN, 0, 0);
    }

    pub fn wheel(&mut self, delta: i32) {
        self.emit(EV_REL, REL_WHEEL, delta);
        self.emit(EV_SYN, 0, 0);
    }

    pub fn chord(&mut self, combo: &str) -> Result<(), String> {
        let codes: Vec<u16> = combo.split('+').map(|k| key_code(k).ok_or(tf!("touche inconnue : {}", k))).collect::<Result<_, _>>()?;
        codes.iter().for_each(|&c| self.key(c, true));
        codes.iter().rev().for_each(|&c| self.key(c, false));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::key_code;
    #[test]
    fn codes() {
        assert_eq!(key_code("q"), Some(16));
        assert_eq!(key_code("p"), Some(25));
        assert_eq!(key_code("a"), Some(30));
        assert_eq!(key_code("l"), Some(38));
        assert_eq!(key_code("z"), Some(44));
        assert_eq!(key_code("m"), Some(50));
        assert_eq!(key_code("1"), Some(2));
        assert_eq!(key_code("0"), Some(11));
        assert_eq!(key_code("F12"), Some(88));
        assert_eq!(key_code("f13"), Some(183));
    }
}
