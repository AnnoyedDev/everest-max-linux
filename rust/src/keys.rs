use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize, Clone, Debug)]
pub struct Key {
    pub name: String,
    pub matrix: u8,
    pub dll: u8,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Function {
    pub group: String,
    pub label: String,
    pub value: u16,
}

pub fn keys() -> &'static [Key] {
    static K: OnceLock<Vec<Key>> = OnceLock::new();
    K.get_or_init(|| serde_json::from_str(include_str!("../data/keys.json")).expect("keys.json"))
}

pub fn functions() -> &'static [Function] {
    static F: OnceLock<Vec<Function>> = OnceLock::new();
    F.get_or_init(|| {
        let all: Vec<Function> = serde_json::from_str(include_str!("../data/function_values.json")).expect("function_values.json");
        let mut out: Vec<Function> = Vec::new();
        for f in all.into_iter().filter(|f| f.value != 0 && f.group != "Hotkey Switch") {
            if !out.iter().any(|o| o.value == f.value) {
                out.push(f);
            }
        }
        out
    })
}

pub fn by_name(name: &str) -> Option<&'static Key> {
    keys().iter().find(|k| k.name == name)
}

pub type Cell = (&'static str, &'static str, f32);
const G: Cell = ("", "", 0.5);

pub const ROWS: [&[Cell]; 6] = [
    &[("Esc", "Esc", 1.0), G, ("F1", "F1", 1.0), ("F2", "F2", 1.0), ("F3", "F3", 1.0), ("F4", "F4", 1.0), G, ("F5", "F5", 1.0), ("F6", "F6", 1.0), ("F7", "F7", 1.0), ("F8", "F8", 1.0), G, ("F9", "F9", 1.0), ("F10", "F10", 1.0), ("F11", "F11", 1.0), ("F12", "F12", 1.0), G, ("Impr", "Prt Sc", 1.0), ("Défil", "Scroll Lk", 1.0), ("Pause", "Pause", 1.0), G, ("D1", "D1", 1.0), ("D2", "D2", 1.0), ("D3", "D3", 1.0), ("D4", "D4", 1.0)],
    &[("`", "`", 1.0), ("1", "1", 1.0), ("2", "2", 1.0), ("3", "3", 1.0), ("4", "4", 1.0), ("5", "5", 1.0), ("6", "6", 1.0), ("7", "7", 1.0), ("8", "8", 1.0), ("9", "9", 1.0), ("0", "0", 1.0), ("-", "-", 1.0), ("=", "=", 1.0), ("⌫", "⭠", 2.0), G, ("Ins", "Insert", 1.0), ("Orig", "Home", 1.0), ("PgUp", "PgUp", 1.0), G, ("Num", "Num Lk", 1.0), ("/", "Num /", 1.0), ("*", "Num *", 1.0), ("-", "Num -", 1.0)],
    &[("Tab", "Tab", 1.5), ("Q", "Q", 1.0), ("W", "W", 1.0), ("E", "E", 1.0), ("R", "R", 1.0), ("T", "T", 1.0), ("Y", "Y", 1.0), ("U", "U", 1.0), ("I", "I", 1.0), ("O", "O", 1.0), ("P", "P", 1.0), ("[", "[", 1.0), ("]", "]", 1.0), ("\\", "\\", 1.5), G, ("Suppr", "Del", 1.0), ("Fin", "End", 1.0), ("PgDn", "PgDn", 1.0), G, ("7", "Num 7", 1.0), ("8", "Num 8", 1.0), ("9", "Num 9", 1.0), ("+", "Num +", 1.0)],
    &[("Caps", "Caps Lk", 1.75), ("A", "A", 1.0), ("S", "S", 1.0), ("D", "D", 1.0), ("F", "F", 1.0), ("G", "G", 1.0), ("H", "H", 1.0), ("J", "J", 1.0), ("K", "K", 1.0), ("L", "L", 1.0), (";", ";", 1.0), ("'", "'", 1.0), ("Entrée", "Enter", 2.25), G, ("", "", 3.0), G, ("4", "Num 4", 1.0), ("5", "Num 5", 1.0), ("6", "Num 6", 1.0)],
    &[("Maj", "LShift", 2.25), ("Z", "Z", 1.0), ("X", "X", 1.0), ("C", "C", 1.0), ("V", "V", 1.0), ("B", "B", 1.0), ("N", "N", 1.0), ("M", "M", 1.0), (",", ",", 1.0), (".", ".", 1.0), ("/", "/", 1.0), ("Maj", "RShift", 2.75), G, ("", "", 1.0), ("↑", "⏶", 1.0), ("", "", 1.0), G, ("1", "Num 1", 1.0), ("2", "Num 2", 1.0), ("3", "Num 3", 1.0), ("Ent", "Num Enter", 1.0)],
    &[("Ctrl", "LCtrl", 1.25), ("Win", "LWin", 1.25), ("Alt", "Alt", 1.25), ("Espace", "Space", 6.25), ("AltGr", "Alt Gr", 1.25), ("Win", "RWin", 1.25), ("Fn", "Fn", 1.25), ("Ctrl", "RCtrl", 1.25), G, ("←", "⏴", 1.0), ("↓", "⏷", 1.0), ("→", "⏵", 1.0), G, ("0", "Num 0", 2.0), (".", "Num .", 1.0)],
];

pub fn positions() -> &'static [(f32, f32); 126] {
    static P: OnceLock<[(f32, f32); 126]> = OnceLock::new();
    P.get_or_init(|| {
        let width = ROWS.iter().map(|r| r.iter().map(|c| c.2).sum::<f32>()).fold(0.0, f32::max);
        let mut p = [(0.5, 0.0); 126];
        for (r, row) in ROWS.iter().enumerate() {
            let mut x = 0.0;
            for &(_, name, w) in row.iter() {
                if let Some(k) = by_name(name).filter(|k| (k.matrix as usize) < 126) {
                    p[k.matrix as usize] = ((x + w / 2.0) / width, r as f32 / 5.0);
                }
                x += w;
            }
        }
        p
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_names_exist() {
        for row in super::ROWS {
            for (_, name, _) in row.iter() {
                assert!(name.is_empty() || super::by_name(name).is_some(), "{name}");
            }
        }
        assert!(super::functions().iter().any(|f| f.value == 183));
    }
}
