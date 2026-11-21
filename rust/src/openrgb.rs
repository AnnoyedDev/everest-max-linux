use crate::protocol::CUSTOM_KEYS;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const PORT: u16 = 6742;
static CLIENTS: AtomicUsize = AtomicUsize::new(0);
static FRAME: Mutex<Option<[[u8; 3]; CUSTOM_KEYS]>> = Mutex::new(None);

fn leds() -> &'static [(String, u8)] {
    static L: OnceLock<Vec<(String, u8)>> = OnceLock::new();
    L.get_or_init(|| {
        let mut out: Vec<(String, u8)> = Vec::new();
        for k in crate::keys::keys() {
            if (k.matrix as usize) < CUSTOM_KEYS && !out.iter().any(|l| l.1 == k.matrix) {
                out.push((k.name.clone(), k.matrix));
            }
        }
        out
    })
}

pub fn frame() -> Option<[[u8; 3]; CUSTOM_KEYS]> {
    if CLIENTS.load(Ordering::Relaxed) == 0 {
        return None;
    }
    *FRAME.lock().unwrap()
}

fn string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16 + 1).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn u32s(out: &mut Vec<u8>, v: &[u32]) {
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

pub fn controller_data() -> Vec<u8> {
    let leds = leds();
    let colors = FRAME.lock().unwrap().unwrap_or([[0; 3]; CUSTOM_KEYS]);
    let mut d = Vec::new();
    d.extend_from_slice(&5i32.to_le_bytes());
    for s in ["Mountain Everest Max", "Everest Base Camp (Linux)", env!("CARGO_PKG_VERSION"), "", "everest-basecamp"] {
        string(&mut d, s);
    }
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&0i32.to_le_bytes());
    string(&mut d, "Direct");
    d.extend_from_slice(&0i32.to_le_bytes());
    u32s(&mut d, &[1 << 5, 0, 0, 0, 0, 0, 0, 1]);
    d.extend_from_slice(&0u16.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    string(&mut d, "Keyboard");
    d.extend_from_slice(&1i32.to_le_bytes());
    u32s(&mut d, &[leds.len() as u32; 3]);
    d.extend_from_slice(&0u16.to_le_bytes());
    d.extend_from_slice(&(leds.len() as u16).to_le_bytes());
    for (name, m) in leds {
        string(&mut d, &format!("Key: {name}"));
        d.extend_from_slice(&(*m as u32).to_le_bytes());
    }
    d.extend_from_slice(&(leds.len() as u16).to_le_bytes());
    for (_, m) in leds {
        let c = colors[*m as usize];
        d.extend_from_slice(&[c[0], c[1], c[2], 0]);
    }
    let mut out = ((d.len() + 4) as u32).to_le_bytes().to_vec();
    out.extend(d);
    out
}

fn apply_colors(start: usize, data: &[u8]) {
    let leds = leds();
    let mut f = FRAME.lock().unwrap();
    let frame = f.get_or_insert([[0; 3]; CUSTOM_KEYS]);
    for (i, c) in data.chunks_exact(4).enumerate() {
        if let Some((_, m)) = leds.get(start + i) {
            frame[*m as usize] = [c[0], c[1], c[2]];
        }
    }
}

fn reply(s: &mut TcpStream, id: u32, body: &[u8]) -> std::io::Result<()> {
    let mut h = b"ORGB".to_vec();
    u32s(&mut h, &[0, id, body.len() as u32]);
    h.extend_from_slice(body);
    s.write_all(&h)
}

fn handle(mut s: TcpStream) -> std::io::Result<()> {
    loop {
        let mut h = [0u8; 16];
        s.read_exact(&mut h)?;
        if &h[..4] != b"ORGB" {
            return Ok(());
        }
        let word = |i: usize| u32::from_le_bytes(h[i..i + 4].try_into().unwrap());
        let (dev, id, size) = (word(4), word(8), word(12) as usize);
        let mut body = vec![0u8; size.min(1 << 20)];
        s.read_exact(&mut body)?;
        let u16_at = |i: usize| body.get(i..i + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]])) as usize;
        match id {
            0 => reply(&mut s, 0, &1u32.to_le_bytes())?,
            1 if dev == 0 => reply(&mut s, 1, &controller_data())?,
            40 => reply(&mut s, 40, &0u32.to_le_bytes())?,
            1050 if dev == 0 => apply_colors(0, body.get(6..6 + u16_at(4) * 4).unwrap_or(&[])),
            1051 if dev == 0 => apply_colors(0, body.get(10..10 + u16_at(8) * 4).unwrap_or(&[])),
            1052 if dev == 0 && body.len() >= 8 => apply_colors(u32::from_le_bytes(body[..4].try_into().unwrap()) as usize, &body[4..8]),
            _ => {}
        }
    }
}

pub fn serve() {
    let mut listener: Option<TcpListener> = None;
    loop {
        let on = crate::config::Config::load().openrgb;
        if !on {
            listener = None;
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }
        if listener.is_none() {
            match TcpListener::bind(("127.0.0.1", PORT)).and_then(|l| l.set_nonblocking(true).map(|_| l)) {
                Ok(l) => listener = Some(l),
                Err(e) => {
                    eprintln!("OpenRGB : {e}");
                    std::thread::sleep(Duration::from_secs(10));
                    continue;
                }
            }
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match listener.as_ref().unwrap().accept() {
                Ok((s, _)) => {
                    std::thread::spawn(move || {
                        let _ = s.set_nonblocking(false);
                        CLIENTS.fetch_add(1, Ordering::Relaxed);
                        let _ = handle(s);
                        if CLIENTS.fetch_sub(1, Ordering::Relaxed) == 1 {
                            *FRAME.lock().unwrap() = None;
                        }
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leds_are_unique_and_colors_land_on_the_matrix() {
        let l = leds();
        assert!(l.len() > 100 && l.len() <= CUSTOM_KEYS);
        assert!(l.iter().all(|a| l.iter().filter(|b| b.1 == a.1).count() == 1));
        assert!(l.iter().any(|a| a.0 == "F1") && !l.iter().any(|a| a.0 == "F13"));
        let d = controller_data();
        assert_eq!(u32::from_le_bytes(d[..4].try_into().unwrap()) as usize, d.len());
        let a = l.iter().position(|x| x.0 == "A").unwrap();
        let mut c = vec![0u8; 4 * (a + 1)];
        c[4 * a..4 * a + 3].copy_from_slice(&[1, 2, 3]);
        apply_colors(0, &c);
        let m = crate::keys::by_name("A").unwrap().matrix as usize;
        assert_eq!(FRAME.lock().unwrap().unwrap()[m], [1, 2, 3]);
    }
}
