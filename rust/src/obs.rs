use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub const REQUESTS: [(&str, &str, bool); 7] = [
    ("SetCurrentProgramScene", "Changer de scène", true),
    ("ToggleRecord", "Lancer / arrêter l'enregistrement", false),
    ("ToggleRecordPause", "Mettre l'enregistrement en pause", false),
    ("ToggleStream", "Lancer / arrêter le live", false),
    ("ToggleInputMute", "Couper / rétablir une source audio", true),
    ("ToggleReplayBuffer", "Lancer / arrêter le replay", false),
    ("SaveReplayBuffer", "Sauvegarder le replay", false),
];

pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

pub fn auth(password: &str, salt: &str, challenge: &str) -> String {
    let secret = base64(&sha256(format!("{password}{salt}").as_bytes()));
    base64(&sha256(format!("{secret}{challenge}").as_bytes()))
}

struct Ws(TcpStream);

impl Ws {
    fn connect(port: u16) -> Result<Ws, String> {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| tf!("OBS injoignable sur le port {} ({})", port, e))?;
        s.set_read_timeout(Some(Duration::from_secs(4))).map_err(|e| e.to_string())?;
        write!(s, "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ZXZlcmVzdC1iYXNlY2FtcA==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: obswebsocket.json\r\n\r\n")
            .map_err(|e| e.to_string())?;
        let mut head = Vec::new();
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            s.read_exact(&mut b).map_err(|e| e.to_string())?;
            head.push(b[0]);
        }
        if !head.starts_with(b"HTTP/1.1 101") {
            return Err(String::from_utf8_lossy(&head).lines().next().unwrap_or_default().to_string());
        }
        Ok(Ws(s))
    }

    fn send(&mut self, v: &Value) -> Result<(), String> {
        let data = v.to_string().into_bytes();
        let mask = [0x5A, 0x17, 0xC3, 0x8E];
        let mut f = vec![0x81];
        match data.len() {
            n if n < 126 => f.push(0x80 | n as u8),
            n => {
                f.push(0x80 | 126);
                f.extend_from_slice(&(n as u16).to_be_bytes());
            }
        }
        f.extend_from_slice(&mask);
        f.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.0.write_all(&f).map_err(|e| e.to_string())
    }

    fn recv(&mut self) -> Result<Value, String> {
        loop {
            let mut h = [0u8; 2];
            self.0.read_exact(&mut h).map_err(|e| e.to_string())?;
            let mut len = (h[1] & 0x7F) as u64;
            if len == 126 {
                let mut x = [0u8; 2];
                self.0.read_exact(&mut x).map_err(|e| e.to_string())?;
                len = u16::from_be_bytes(x) as u64;
            } else if len == 127 {
                let mut x = [0u8; 8];
                self.0.read_exact(&mut x).map_err(|e| e.to_string())?;
                len = u64::from_be_bytes(x);
            }
            let mut body = vec![0u8; len.min(1 << 24) as usize];
            self.0.read_exact(&mut body).map_err(|e| e.to_string())?;
            match h[0] & 0x0F {
                1 => return serde_json::from_slice(&body).map_err(|e| e.to_string()),
                8 => {
                    let code = if body.len() >= 2 { u16::from_be_bytes([body[0], body[1]]) } else { 0 };
                    return Err(if code == 4009 { crate::i18n::t("mot de passe OBS refusé").to_string() } else { tf!("OBS a fermé la connexion ({})", code) });
                }
                _ => {}
            }
        }
    }
}

pub fn request(port: u16, password: &str, kind: &str, data: Value) -> Result<Value, String> {
    let mut ws = Ws::connect(port)?;
    let hello = ws.recv()?;
    let mut identify = json!({"op": 1, "d": {"rpcVersion": 1, "eventSubscriptions": 0}});
    if let Some(a) = hello["d"].get("authentication") {
        identify["d"]["authentication"] = auth(password, a["salt"].as_str().unwrap_or(""), a["challenge"].as_str().unwrap_or("")).into();
    }
    ws.send(&identify)?;
    ws.recv()?;
    ws.send(&json!({"op": 6, "d": {"requestType": kind, "requestId": "everest", "requestData": data}}))?;
    loop {
        let r = ws.recv()?;
        if r["op"] == 7 {
            let st = &r["d"]["requestStatus"];
            return if st["result"] == true { Ok(r["d"]["responseData"].clone()) } else { Err(st["comment"].as_str().unwrap_or("OBS").to_string()) };
        }
    }
}

pub fn run(port: u16, password: &str, kind: &str, arg: &str) -> Result<(), String> {
    let data = match kind {
        "SetCurrentProgramScene" => json!({"sceneName": arg}),
        "ToggleInputMute" => json!({"inputName": arg}),
        _ => json!({}),
    };
    request(port, password, kind, data).map(|_| ())
}

pub fn scenes(port: u16, password: &str) -> Result<Vec<String>, String> {
    let r = request(port, password, "GetSceneList", json!({}))?;
    Ok(r["scenes"].as_array().into_iter().flatten().rev().filter_map(|s| s["sceneName"].as_str().map(String::from)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_auth() {
        let hex = |b: [u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(hex(sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(sha256(&[b'a'; 1000])), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(auth("supersecretpassword", "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=", "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="), "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4=");
    }
}
