use crate::config::{display_key_binding, Action, Binding, Config, DisplayKey, DISPLAY_KEY_NAMES};
use crate::device::{Device, Result};
use crate::{images, keys, protocol as p};


fn dll(name: &str) -> Result<u8> {
    keys::by_name(name).map(|k| k.dll).ok_or(tf!("touche inconnue : {}", name))
}

pub fn binding(dev: &mut Device, key: &str, b: &Binding) -> Result<()> {
    let src = dll(key)?;
    match b {
        Binding::ParDefaut => dev.transact(&p::change_key(src, 255)).map(|_| ()),
        Binding::Desactivee => dev.transact(&p::change_key(src, 0)).map(|_| ()),
        Binding::Touche { target } => dev.transact(&p::change_key(src, dll(target)? as u16)).map(|_| ()),
        Binding::Raccourci { key, mods } => dev.transact(&p::change_shortcut(src, dll(key)?, *mods)).map(|_| ()),
        Binding::Fonction { value, .. } => dev.transact(&p::change_key(src, *value)).map(|_| ()),
        Binding::Macro { mode, events } => dev.set_macro(src, *mode, events),
    }
}

pub fn lighting(dev: &mut Device, cfg: &Config, save: bool) -> Result<()> {
    if crate::fx::soft_index(cfg.effect.effect).is_some() {
        return dev.transact(&p::switch_profile(cfg.profile, p::slot(cfg.effect.effect))).map(|_| ());
    }
    if cfg.effect.effect == p::CUSTOM {
        dev.set_custom(cfg.profile, &cfg.custom_array(), cfg.effect.brightness, save)
    } else {
        dev.set_effect(cfg.profile, &cfg.effect, save)
    }
}

pub fn all_bindings(dev: &mut Device, cfg: &Config) -> Result<()> {
    dev.transact(&p::switch_profile(cfg.profile, p::slot(cfg.effect.effect)))?;
    cfg.bindings.iter().try_for_each(|(k, b)| binding(dev, k, b))
}

pub fn sync_display_key(cfg: &mut Config, i: usize) -> Binding {
    let b = display_key_binding(i, cfg.display_keys[i].action != Action::Aucune);
    cfg.bindings.insert(DISPLAY_KEY_NAMES[i].to_string(), b.clone());
    b
}

pub fn art(dk: &DisplayKey) -> images::Art<'_> {
    images::Art { image: &dk.image, icon: &dk.icon, text: &dk.text, fg: dk.fg, bg: dk.bg, grid: dk.grid }
}

pub fn display_key_image(dk: &DisplayKey) -> Result<image::RgbImage> {
    images::render(72, 72, &art(dk))
}

pub fn display_key(dev: &mut Device, profile: u8, n: u8, dk: &DisplayKey, progress: &mut dyn FnMut(f32)) -> Result<()> {
    let img = display_key_image(dk)?;
    dev.upload_picture(&images::to_rgb565(&img), 1, profile, n, progress)
}

pub fn dock_image(dev: &mut Device, path: &str, progress: &mut dyn FnMut(f32)) -> Result<()> {
    let art = images::Art { image: path, icon: "", text: "", fg: [255; 3], bg: [0; 3], grid: false };
    dev.upload_picture(&images::to_rgb565(&images::render(240, 204, &art)?), 0, 1, 0, progress)
}

pub fn clock_now(dev: &mut Device) -> Result<()> {
    let t = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    dev.transact(&p::clock(t.tm_mon as u8 + 1, t.tm_mday as u8, t.tm_hour as u8, t.tm_min as u8, t.tm_sec as u8)).map(|_| ())
}

pub fn set_profiles(dev: &mut Device, enabled: [bool; 5]) -> Result<[u8; 29]> {
    let mut info = dev.extend_info()?;
    info[p::EXT_PROFILES..p::EXT_PROFILES + 5].copy_from_slice(&p::profile_table(enabled));
    dev.set_extend_info(&info)?;
    Ok(info)
}

pub fn set_dock(dev: &mut Device, menus: u8, color: [u8; 3], setup: u8, saver: u16, turnoff: u16) -> Result<[u8; 29]> {
    let mut info = dev.extend_info()?;
    info[p::EXT_SHOW_MENU] = menus.max(1);
    info[p::EXT_MENU_COLOR..p::EXT_MENU_COLOR + 3].copy_from_slice(&color);
    info[p::EXT_SCREEN_SETUP] = setup;
    info[p::EXT_SAVER..p::EXT_SAVER + 2].copy_from_slice(&saver.clamp(5, 1800).to_le_bytes());
    info[p::EXT_TURNOFF..p::EXT_TURNOFF + 2].copy_from_slice(&turnoff.clamp(5, 1800).to_le_bytes());
    dev.set_extend_info(&info)?;
    Ok(info)
}
