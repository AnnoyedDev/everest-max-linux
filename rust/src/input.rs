use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::time::Duration;

const NAMES: &[(u16, &str)] = &[
    (1, "Esc"), (2, "1"), (3, "2"), (4, "3"), (5, "4"), (6, "5"), (7, "6"), (8, "7"), (9, "8"), (10, "9"), (11, "0"), (12, "-"), (13, "="), (14, "⭠"),
    (15, "Tab"), (16, "Q"), (17, "W"), (18, "E"), (19, "R"), (20, "T"), (21, "Y"), (22, "U"), (23, "I"), (24, "O"), (25, "P"), (26, "["), (27, "]"), (28, "Enter"),
    (29, "LCtrl"), (30, "A"), (31, "S"), (32, "D"), (33, "F"), (34, "G"), (35, "H"), (36, "J"), (37, "K"), (38, "L"), (39, ";"), (40, "'"), (41, "`"),
    (42, "LShift"), (43, "\\"), (44, "Z"), (45, "X"), (46, "C"), (47, "V"), (48, "B"), (49, "N"), (50, "M"), (51, ","), (52, "."), (53, "/"), (54, "RShift"),
    (55, "Num *"), (56, "Alt"), (57, "Space"), (58, "Caps Lk"), (59, "F1"), (60, "F2"), (61, "F3"), (62, "F4"), (63, "F5"), (64, "F6"), (65, "F7"), (66, "F8"),
    (67, "F9"), (68, "F10"), (69, "Num Lk"), (70, "Scroll Lk"), (71, "Num 7"), (72, "Num 8"), (73, "Num 9"), (74, "Num -"), (75, "Num 4"), (76, "Num 5"),
    (77, "Num 6"), (78, "Num +"), (79, "Num 1"), (80, "Num 2"), (81, "Num 3"), (82, "Num 0"), (83, "Num ."), (86, "ISO <"), (87, "F11"), (88, "F12"),
    (96, "Num Enter"), (97, "RCtrl"), (98, "Num /"), (99, "Prt Sc"), (100, "Alt Gr"), (102, "Home"), (103, "⏶"), (104, "PgUp"), (105, "⏴"), (106, "⏵"),
    (107, "End"), (108, "⏷"), (109, "PgDn"), (110, "Insert"), (111, "Del"), (113, "Mute"), (119, "Pause"), (125, "LWin"), (126, "RWin"),
    (163, "Next Track"), (164, "Play Pause"), (165, "Pre Track"), (183, "D1"), (184, "D2"), (185, "D3"), (186, "D4"),
];

pub fn name_of(code: u16) -> Option<&'static str> {
    NAMES.iter().find(|n| n.0 == code).map(|n| n.1)
}

fn nodes() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else { return vec![] };
    let read = |p: PathBuf| std::fs::read_to_string(p).unwrap_or_default().trim().to_string();
    dir.flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
        .filter(|e| {
            let d = e.path().join("device");
            read(d.join("id/vendor")) == "3282" && read(d.join("id/product")) == "0001" && read(d.join("capabilities/key")).len() > 8
        })
        .map(|e| PathBuf::from("/dev/input").join(e.file_name()))
        .collect()
}

pub fn read_keys(mut f: impl FnMut(Option<(u16, bool)>) -> bool) -> Result<(), String> {
    let mut files: Vec<File> = nodes().into_iter().filter_map(|p| File::open(p).ok()).collect();
    if files.is_empty() {
        return Err(crate::i18n::t("frappes du clavier illisibles (règle udev à mettre à jour)").into());
    }
    loop {
        let mut fds: Vec<libc::pollfd> = files.iter().map(|f| libc::pollfd { fd: f.as_raw_fd(), events: libc::POLLIN, revents: 0 }).collect();
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, Duration::from_millis(250).as_millis() as i32) };
        if n < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if n == 0 && !f(None) {
            return Ok(());
        }
        for (i, p) in fds.iter().enumerate() {
            if p.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                return Err("clavier débranché".into());
            }
            if p.revents & libc::POLLIN == 0 {
                continue;
            }
            let mut ev = [0u8; 24];
            if files[i].read_exact(&mut ev).is_err() {
                return Err("clavier débranché".into());
            }
            let kind = u16::from_ne_bytes([ev[16], ev[17]]);
            let code = u16::from_ne_bytes([ev[18], ev[19]]);
            let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
            if kind == 1 && value != 2 && !f(Some((code, value == 1))) {
                return Ok(());
            }
        }
    }
}

pub fn heatmap_path() -> PathBuf {
    crate::config::path().with_file_name("heatmap.json")
}

pub fn load_heatmap() -> BTreeMap<String, u64> {
    std::fs::read_to_string(heatmap_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn merge_heatmap(delta: &mut BTreeMap<String, u64>) {
    if delta.is_empty() {
        return;
    }
    let mut m = load_heatmap();
    for (k, v) in std::mem::take(delta) {
        *m.entry(k).or_insert(0) += v;
    }
    let _ = std::fs::write(heatmap_path(), serde_json::to_string(&m).unwrap());
}

pub fn heatmap() {
    loop {
        if !crate::config::Config::load().heatmap {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }
        let mut delta = BTreeMap::new();
        let mut since = std::time::Instant::now();
        let r = read_keys(|ev| {
            if let Some((code, true)) = ev {
                if let Some(n) = name_of(code) {
                    *delta.entry(n.to_string()).or_insert(0) += 1;
                }
            }
            if since.elapsed() < Duration::from_secs(5) {
                return true;
            }
            since = std::time::Instant::now();
            merge_heatmap(&mut delta);
            crate::config::Config::load().heatmap
        });
        merge_heatmap(&mut delta);
        if let Err(e) = r {
            eprintln!("heatmap: {e}");
            std::thread::sleep(Duration::from_secs(5));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_name_is_a_real_key() {
        for (_, n) in super::NAMES {
            assert!(crate::keys::by_name(n).is_some(), "{n}");
        }
        assert_eq!(super::name_of(30), Some("A"));
    }
}
