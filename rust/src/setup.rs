use crate::config::{self, Config};
use crate::{device, icons};
use std::path::PathBuf;
use std::process::Command;
use crate::i18n::t;

pub const RULES: &str = include_str!("../70-mountain-everest.rules");
const RULES_PATH: &str = "/etc/udev/rules.d/70-mountain-everest.rules";
const SERVICE_NAME: &str = "everest-basecamp";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Device,
    Udev,
    Uinput,
    Binary,
    Desktop,
    Service,
    DisplayKeys,
    Update,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub item: Item,
    pub ok: bool,
    pub title: &'static str,
    pub detail: String,
    pub icon: &'static str,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
}

pub fn installed_bin() -> PathBuf {
    home().join(".local/bin/everest")
}

fn desktop_file() -> PathBuf {
    data_home().join("applications/everest-basecamp.desktop")
}

fn icon_file() -> PathBuf {
    data_home().join("icons/hicolor/256x256/apps/everest-basecamp.png")
}

fn unit_file() -> PathBuf {
    config::path().parent().unwrap().parent().unwrap().join("systemd/user/everest-basecamp.service")
}

pub fn icons_dir() -> PathBuf {
    data_home().join("everest-basecamp/icons")
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl").arg("--user").args(args).status().map(|s| s.success()).unwrap_or(false)
}

pub enum ServiceState {
    Absent,
    Current,
    Outdated(String),
}

fn version_of(path: &std::path::Path) -> Option<String> {
    let out = Command::new(path).arg("version").output().ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty() && v.len() < 32).then_some(v)
}

pub fn service_state() -> ServiceState {
    if !service_active() {
        return ServiceState::Absent;
    }
    let pid = Command::new("systemctl").args(["--user", "show", "-p", "MainPID", "--value", SERVICE_NAME]).output()
        .ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok()).unwrap_or(0);
    let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe")) else { return ServiceState::Current };
    let shown = exe.to_string_lossy().to_string();
    if shown.ends_with(" (deleted)") {
        return ServiceState::Outdated(t("ancienne copie remplacée sur le disque, encore en mémoire").into());
    }
    let cur = std::env::current_exe().unwrap_or_default();
    if same_file(&exe, &cur) {
        ServiceState::Current
    } else {
        ServiceState::Outdated(match version_of(&exe) {
            Some(v) if v != env!("CARGO_PKG_VERSION") => format!("version {v}"),
            Some(v) => tf!("version {}, compilation différente", v),
            None => t("version antérieure à 0.2.0").into(),
        })
    }
}

fn version_tuple(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn update_needed() -> Option<String> {
    let cur = std::env::current_exe().ok()?;
    let inst = installed_bin();
    if inst.exists() && !same_file(&cur, &inst) {
        let v = version_of(&inst);
        if v.as_deref().is_some_and(|v| version_tuple(v) > version_tuple(env!("CARGO_PKG_VERSION"))) {
            return None;
        }
        return Some(match v {
            Some(v) if v == env!("CARGO_PKG_VERSION") => tf!("version {}, compilation différente", v),
            Some(v) => format!("{v}"),
            None => t("version antérieure à 0.2.0").into(),
        });
    }
    match service_state() {
        ServiceState::Outdated(v) => Some(v),
        _ => None,
    }
}

pub fn update() -> Result<(), String> {
    install_binary()?;
    if service_active() && !systemctl(&["restart", SERVICE_NAME]) {
        return Err(t("systemctl --user a échoué").into());
    }
    Ok(())
}

pub fn service_active() -> bool {
    Command::new("systemctl").args(["--user", "is-active", SERVICE_NAME]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active").unwrap_or(false)
}

fn same_file(a: &PathBuf, b: &PathBuf) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.len() == y.len() && std::fs::read(a).ok() == std::fs::read(b).ok(),
        _ => false,
    }
}

pub fn run(cfg: &Config) -> Vec<Check> {
    let rules_ok = std::fs::read_to_string(RULES_PATH).map(|s| s.trim() == RULES.trim()).unwrap_or(false);
    let found = device::find_hidraw().is_ok();
    let opened = found && device::Device::open().is_ok();
    let uinput_ok = std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok();
    let cur = std::env::current_exe().unwrap_or_default();
    let bin_ok = same_file(&cur, &installed_bin());
    let unit = std::fs::read_to_string(unit_file()).unwrap_or_default();
    let service_ok = unit.contains(&installed_bin().to_string_lossy().to_string()) && service_active();
    let keys_ok = (0..4).all(|i| cfg.display_key_in_sync(i));
    vec![
        Check { item: Item::Device, ok: opened, icon: "usb", title: t("Clavier"),
            detail: if opened { t("Everest Max connecté").into() } else if found { t("Clavier détecté mais accès refusé").into() } else { t("Clavier non détecté — vérifie le câble").into() } },
        Check { item: Item::Udev, ok: rules_ok, icon: "shield", title: t("Règle udev"),
            detail: if rules_ok { t("Accès au clavier autorisé").into() } else { t("Absente ou ancienne version (mot de passe administrateur requis)").into() } },
        Check { item: Item::Uinput, ok: uinput_ok, icon: "keyboard", title: t("Clavier virtuel"),
            detail: if uinput_ok { t("Raccourcis et macros des touches écran disponibles").into() } else { t("/dev/uinput inaccessible (module ou permission)").into() } },
        Check { item: Item::Binary, ok: bin_ok, icon: "download", title: t("Programme installé"),
            detail: if bin_ok { "~/.local/bin/everest".into() } else { tf!("Copier vers {}", "~/.local/bin/everest") } },
        Check { item: Item::Desktop, ok: desktop_file().exists(), icon: "desktop", title: t("Raccourci d'application"),
            detail: if desktop_file().exists() { t("Présent dans le menu des applications").into() } else { t("Absent du menu des applications").into() } },
        Check { item: Item::Service, ok: service_ok, icon: "settings", title: t("Service"),
            detail: if service_ok { t("Actif, démarre à la connexion").into() } else if service_active() { t("Actif mais lancé depuis une ancienne copie — à mettre à jour").into() } else { t("Arrêté ou non installé — les actions des touches écran ne marchent pas").into() } },
        Check { item: Item::DisplayKeys, ok: keys_ok, icon: "screens", title: t("Touches écran"),
            detail: if keys_ok { t("Frappe d'origine coupée sur les touches qui ont une action").into() } else { t("Une touche avec action tape encore sa touche d'origine").into() } },
    ]
}

pub fn fix_root() -> Result<(), String> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let tmp = dir.join("everest-basecamp.rules");
    std::fs::write(&tmp, RULES).map_err(|e| e.to_string())?;
    let script = format!(
        "install -m 0644 '{}' {RULES_PATH} && printf 'uinput\\n' > /etc/modules-load.d/everest-uinput.conf && modprobe uinput && udevadm control --reload && udevadm trigger --action=change --subsystem-match=hidraw && udevadm trigger --action=change --subsystem-match=misc",
        tmp.display().to_string().replace('\'', "")
    );
    let ok = Command::new("pkexec").args(["/bin/sh", "-c", &script]).status().map(|s| s.success()).unwrap_or(false);
    let _ = std::fs::remove_file(tmp);
    std::thread::sleep(std::time::Duration::from_millis(1500));
    if ok { Ok(()) } else { Err(t("installation administrateur annulée ou refusée").into()) }
}

pub fn install_binary() -> Result<(), String> {
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    let dst = installed_bin();
    if same_file(&cur, &dst) {
        return Ok(());
    }
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = dst.with_extension("new");
    std::fs::copy(&cur, &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &dst).map_err(|e| e.to_string())
}

pub fn install_desktop() -> Result<(), String> {
    let icon = icons::rasterize_svg(icons::APP_ICON, 256, 256).ok_or(t("icône"))?;
    std::fs::create_dir_all(icon_file().parent().unwrap()).map_err(|e| e.to_string())?;
    icon.save_png(icon_file()).map_err(|e| e.to_string())?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Everest Base Camp\nComment=Mountain Everest Max : éclairage, touches, macros, écrans\nExec={}\nIcon=everest-basecamp\nCategories=Settings;HardwareSettings;Utility;\nTerminal=false\n",
        installed_bin().display()
    );
    std::fs::create_dir_all(desktop_file().parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(desktop_file(), entry).map_err(|e| e.to_string())
}

pub fn install_service() -> Result<(), String> {
    install_binary()?;
    let unit = format!(
        "[Unit]\nDescription=Everest Max Base Camp (touches écran, dock)\nAfter=graphical-session.target\n\n[Service]\nExecStart={} daemon\nRestart=on-failure\nRestartSec=3\n\n[Install]\nWantedBy=default.target\n",
        installed_bin().display()
    );
    std::fs::create_dir_all(unit_file().parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(unit_file(), unit).map_err(|e| e.to_string())?;
    if systemctl(&["daemon-reload"]) && systemctl(&["enable", SERVICE_NAME]) && systemctl(&["restart", SERVICE_NAME]) {
        Ok(())
    } else {
        Err(t("systemctl --user a échoué").into())
    }
}

pub fn stop_service() {
    systemctl(&["disable", "--now", SERVICE_NAME]);
}

pub fn fix(item: Item, cfg: &mut Config) -> Result<(), String> {
    match item {
        Item::Device => Err(t("branche le clavier").into()),
        Item::Udev | Item::Uinput => fix_root(),
        Item::Binary => install_binary(),
        Item::Desktop => install_binary().and_then(|_| install_desktop()),
        Item::Service => install_service(),
        Item::Update => update(),
        Item::DisplayKeys => {
            let mut dev = device::Device::open()?;
            for i in 0..4 {
                let b = crate::apply::sync_display_key(cfg, i);
                crate::apply::binding(&mut dev, config::DISPLAY_KEY_NAMES[i], &b)?;
            }
            Ok(())
        }
    }
}

pub fn fix_all(cfg: &mut Config) -> Vec<String> {
    let mut errors = Vec::new();
    let mut root_done = false;
    for c in run(cfg).into_iter().filter(|c| !c.ok && c.item != Item::Device) {
        if matches!(c.item, Item::Udev | Item::Uinput) {
            if root_done {
                continue;
            }
            root_done = true;
        }
        if let Err(e) = fix(c.item, cfg) {
            errors.push(format!("{} : {e}", c.title));
        }
    }
    errors
}
