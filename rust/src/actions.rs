use crate::config::{Action, PowerOp};
use crate::uinput::{key_code, VirtualInput};
use std::process::{Command, Stdio};

fn spawn(prog: &str, args: &[&str]) -> Result<(), String> {
    Command::new(prog).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map(|_| ()).map_err(|e| format!("{prog}: {e}"))
}

pub fn sinks() -> Vec<(String, String)> {
    let out = Command::new("pactl").args(["-f", "json", "list", "sinks"]).output();
    let Ok(out) = out else { return vec![] };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    v.as_array().into_iter().flatten()
        .filter_map(|s| Some((s["name"].as_str()?.to_string(), s["description"].as_str().unwrap_or("").to_string())))
        .collect()
}

pub fn players() -> Vec<String> {
    let Ok(out) = Command::new("busctl").args(["--user", "list", "--no-legend", "--no-pager"]).output() else { return vec![] };
    String::from_utf8_lossy(&out.stdout).lines()
        .filter_map(|l| l.split_whitespace().next()?.strip_prefix("org.mpris.MediaPlayer2.").map(String::from))
        .collect()
}

pub fn media(op: &str, player: &str) -> Result<(), String> {
    let name = players().into_iter().find(|p| p.starts_with(player)).ok_or(crate::i18n::t("aucun lecteur multimédia ouvert"))?;
    spawn("busctl", &["--user", "call", &format!("org.mpris.MediaPlayer2.{name}"), "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", op])
}

pub struct Runner {
    vk: Option<VirtualInput>,
}

impl Runner {
    pub fn new() -> Runner {
        Runner { vk: None }
    }

    fn vk(&mut self) -> Result<&mut VirtualInput, String> {
        if self.vk.is_none() {
            self.vk = Some(VirtualInput::new()?);
        }
        Ok(self.vk.as_mut().unwrap())
    }

    pub fn run(&mut self, a: &Action) -> Result<(), String> {
        match a {
            Action::Aucune => Ok(()),
            Action::Commande { cmd } => spawn("sh", &["-c", cmd]),
            Action::Url { url } => spawn("xdg-open", &[url]),
            Action::Dossier { path } => spawn("xdg-open", &[path]),
            Action::Son { file, sink, volume } => {
                let vol = format!("--volume={}", *volume.min(&150) as u32 * 65536 / 100);
                if sink.is_empty() { spawn("paplay", &[&vol, file]) } else { spawn("paplay", &[&vol, "--device", sink, file]) }
            }
            Action::Raccourci { keys } => self.vk()?.chord(keys),
            Action::ToucheBas { key } | Action::ToucheHaut { key } => {
                let down = matches!(a, Action::ToucheBas { .. });
                match key.as_str() {
                    "wheelup" if down => self.vk()?.wheel(1),
                    "wheeldown" if down => self.vk()?.wheel(-1),
                    "wheelup" | "wheeldown" => {}
                    k => {
                        let c = key_code(k).ok_or(format!("touche inconnue : {k}"))?;
                        self.vk()?.key(c, down)
                    }
                }
                Ok(())
            }
            Action::Delai { ms } => {
                std::thread::sleep(std::time::Duration::from_millis(*ms));
                Ok(())
            }
            Action::Systeme { op } => match op {
                PowerOp::Eteindre => spawn("systemctl", &["poweroff"]),
                PowerOp::Redemarrer => spawn("systemctl", &["reboot"]),
                PowerOp::Veille => spawn("systemctl", &["suspend"]),
                PowerOp::Hibernation => spawn("systemctl", &["hibernate"]),
                PowerOp::Verrouiller => spawn("loginctl", &["lock-session"]),
                PowerOp::Deconnexion => spawn("loginctl", &["terminate-session", &std::env::var("XDG_SESSION_ID").unwrap_or_default()]),
            },
            Action::Multi { actions } => actions.iter().try_for_each(|x| self.run(x)),
            Action::Page { .. } => Ok(()),
            Action::Media { op, player } => media(op, player),
            Action::Obs { request, arg } => {
                let cfg = crate::config::Config::load();
                crate::obs::run(cfg.obs_port, &cfg.obs_password, request, arg)
            }
        }
    }
}
