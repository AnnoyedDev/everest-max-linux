use crate::protocol::{Effect, MacroEvent, CUSTOM_KEYS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type")]
pub enum Action {
    #[default]
    Aucune,
    Commande { cmd: String },
    Url { url: String },
    Dossier { path: String },
    Son { file: String, sink: String, #[serde(default = "full_volume")] volume: u16 },
    Raccourci { keys: String },
    Systeme { op: PowerOp },
    ToucheBas { key: String },
    ToucheHaut { key: String },
    Delai { ms: u64 },
    Multi { actions: Vec<Action> },
}

fn full_volume() -> u16 {
    100
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum PowerOp {
    #[default]
    Eteindre,
    Redemarrer,
    Veille,
    Hibernation,
    Verrouiller,
    Deconnexion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type")]
pub enum Binding {
    #[default]
    ParDefaut,
    Desactivee,
    Touche { target: String },
    Raccourci { key: String, mods: u8 },
    Fonction { value: u16, label: String },
    Macro { mode: u8, events: Vec<MacroEvent> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayKey {
    pub image: String,
    pub icon: String,
    pub text: String,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub grid: bool,
    pub action: Action,
}

impl Default for DisplayKey {
    fn default() -> Self {
        DisplayKey { image: String::new(), icon: String::new(), text: String::new(), fg: [255; 3], bg: crate::images::BLUEPRINT, grid: true, action: Action::Aucune }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostKey {
    pub name: String,
    pub byte: usize,
    pub bit: u8,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub profile: u8,
    pub effect: Effect,
    pub custom: BTreeMap<u8, [u8; 3]>,
    pub bindings: BTreeMap<String, Binding>,
    pub display_keys: [DisplayKey; 4],
    pub host_keys: Vec<HostKey>,
    pub dock_image: String,
    pub sync_clock: bool,
    pub sync_volume: bool,
    pub sync_pcinfo: bool,
    pub net_iface: String,
    pub net_dir: u8,
    pub net_max_mbps: u32,
    pub audio_sources: Vec<String>,
    pub lang: String,
    pub skip_setup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            profile: 1,
            effect: Effect::default(),
            custom: BTreeMap::new(),
            bindings: BTreeMap::new(),
            display_keys: Default::default(),
            host_keys: Vec::new(),
            dock_image: String::new(),
            sync_clock: true,
            sync_volume: true,
            sync_pcinfo: true,
            net_iface: String::new(),
            net_dir: 0,
            net_max_mbps: 0,
            audio_sources: Vec::new(),
            lang: String::new(),
            skip_setup: false,
        }
    }
}

pub const DISPLAY_KEY_BITS: [(usize, u8); 4] = [(42, 1), (42, 2), (42, 3), (42, 4)];
pub const DISPLAY_KEY_NAMES: [&str; 4] = ["D1", "D2", "D3", "D4"];

pub fn display_key_binding(i: usize, has_action: bool) -> Binding {
    if has_action {
        Binding::Desactivee
    } else {
        Binding::Touche { target: format!("F{}", 13 + i) }
    }
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("everest-basecamp/config.json")
}

impl Config {
    pub fn load() -> Config {
        std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = path();
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(tmp, p).map_err(|e| e.to_string())
    }

    pub fn net(&self) -> crate::stats::NetCfg<'_> {
        crate::stats::NetCfg { iface: &self.net_iface, dir: self.net_dir, max_mbps: self.net_max_mbps }
    }

    pub fn custom_array(&self) -> [[u8; 3]; CUSTOM_KEYS] {
        let mut a = [[0u8; 3]; CUSTOM_KEYS];
        for (&m, &c) in &self.custom {
            if (m as usize) < CUSTOM_KEYS {
                a[m as usize] = c;
            }
        }
        a
    }

    pub fn display_key_in_sync(&self, i: usize) -> bool {
        let want = display_key_binding(i, self.display_keys[i].action != Action::Aucune);
        let have = self.bindings.get(DISPLAY_KEY_NAMES[i]);
        match want {
            Binding::Desactivee => have == Some(&Binding::Desactivee),
            _ => have != Some(&Binding::Desactivee),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn old_sound_action_keeps_full_volume() {
        let a: super::Action = serde_json::from_str(r#"{"type":"Son","file":"a.mp3","sink":""}"#).unwrap();
        assert_eq!(a, super::Action::Son { file: "a.mp3".into(), sink: String::new(), volume: 100 });
    }
}
