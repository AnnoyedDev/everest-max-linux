use crate::protocol::{Effect, MacroEvent, CUSTOM_KEYS, PAD_KEYS};
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
    Page { page: usize },
    Media { op: String, #[serde(default)] player: String },
    Obs { request: String, #[serde(default)] arg: String },
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MacroPadCfg {
    pub effect: Effect,
    pub colors: [[u8; 3]; PAD_KEYS],
    pub actions: [Action; PAD_KEYS],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PadPage {
    pub name: String,
    pub keys: [DisplayKey; PAD_KEYS],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayPadCfg {
    pub keys: [DisplayKey; PAD_KEYS],
    pub brightness: u8,
    pub pages: Vec<PadPage>,
}

impl Default for DisplayPadCfg {
    fn default() -> Self {
        DisplayPadCfg { keys: Default::default(), brightness: 100, pages: Vec::new() }
    }
}

impl DisplayPadCfg {
    pub fn page(&self, i: usize) -> &[DisplayKey; PAD_KEYS] {
        self.pages.get(i.wrapping_sub(1)).map_or(&self.keys, |p| &p.keys)
    }

    pub fn page_mut(&mut self, i: usize) -> &mut [DisplayKey; PAD_KEYS] {
        match i.checked_sub(1).and_then(|j| self.pages.get_mut(j)) {
            Some(p) => &mut p.keys,
            None => &mut self.keys,
        }
    }

    pub fn remove_page(&mut self, i: usize) {
        if i == 0 || i > self.pages.len() {
            return;
        }
        self.pages.remove(i - 1);
        for n in 0..=self.pages.len() {
            for k in self.page_mut(n).iter_mut() {
                if let Action::Page { page } = &mut k.action {
                    *page = if *page == i { 0 } else if *page > i { *page - 1 } else { *page };
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub effect: Effect,
    pub custom: BTreeMap<u8, [u8; 3]>,
    pub bindings: BTreeMap<String, Binding>,
    pub display_keys: [DisplayKey; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostKey {
    pub name: String,
    pub byte: usize,
    pub bit: u8,
    pub action: Action,
    #[serde(default)]
    pub fn_action: Action,
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
    pub macropad: MacroPadCfg,
    pub displaypad: DisplayPadCfg,
    pub layout: String,
    pub obs_port: u16,
    pub obs_password: String,
    pub profiles: BTreeMap<u8, Profile>,
    pub fn_key: Option<(usize, u8)>,
    pub dark_when_locked: bool,
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
            macropad: MacroPadCfg::default(),
            displaypad: DisplayPadCfg::default(),
            layout: String::new(),
            obs_port: 4455,
            obs_password: String::new(),
            profiles: BTreeMap::new(),
            fn_key: None,
            dark_when_locked: false,
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

pub fn mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
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

    pub fn profile_name(&self, n: u8) -> &str {
        self.profiles.get(&n).map_or("", |p| p.name.as_str())
    }

    pub fn snapshot(&self) -> Profile {
        Profile { name: self.profile_name(self.profile).to_string(), effect: self.effect.clone(), custom: self.custom.clone(), bindings: self.bindings.clone(), display_keys: self.display_keys.clone() }
    }

    pub fn restore(&mut self, p: Profile) {
        self.effect = p.effect;
        self.custom = p.custom;
        self.bindings = p.bindings;
        self.display_keys = p.display_keys;
    }

    pub fn switch_profile(&mut self, n: u8) {
        if n == self.profile {
            return;
        }
        self.profiles.insert(self.profile, self.snapshot());
        self.profile = n;
        if let Some(p) = self.profiles.get(&n).cloned() {
            self.restore(p);
        }
    }

    pub fn copy_profile_to(&mut self, n: u8) {
        let mut p = self.snapshot();
        p.name = self.profile_name(n).to_string();
        self.profiles.insert(n, p);
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
    fn removing_a_page_fixes_page_links() {
        let mut d = super::DisplayPadCfg::default();
        d.pages = vec![Default::default(), Default::default(), Default::default()];
        d.keys[0].action = super::Action::Page { page: 3 };
        d.keys[1].action = super::Action::Page { page: 2 };
        d.pages[2].keys[0].action = super::Action::Page { page: 0 };
        d.remove_page(2);
        assert_eq!(d.pages.len(), 2);
        assert_eq!(d.keys[0].action, super::Action::Page { page: 2 });
        assert_eq!(d.keys[1].action, super::Action::Page { page: 0 });
        assert_eq!(d.page(2)[0].action, super::Action::Page { page: 0 });
        assert!(std::ptr::eq(d.page(9), &d.keys));
    }

    #[test]
    fn each_profile_keeps_its_own_settings() {
        let mut c = super::Config::default();
        c.profiles.entry(1).or_default().name = "Jeu".into();
        c.effect.effect = 3;
        c.switch_profile(2);
        assert_eq!(c.effect.effect, 3);
        c.effect.effect = 5;
        c.switch_profile(1);
        assert_eq!((c.effect.effect, c.profile_name(1)), (3, "Jeu"));
        c.switch_profile(2);
        assert_eq!(c.effect.effect, 5);
        c.copy_profile_to(4);
        assert_eq!(c.profiles[&4].effect.effect, 5);
    }

    #[test]
    fn old_sound_action_keeps_full_volume() {
        let a: super::Action = serde_json::from_str(r#"{"type":"Son","file":"a.mp3","sink":""}"#).unwrap();
        assert_eq!(a, super::Action::Son { file: "a.mp3".into(), sink: String::new(), volume: 100 });
    }
}
