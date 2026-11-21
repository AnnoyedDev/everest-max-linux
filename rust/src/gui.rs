use crate::config::{Action, Binding, Config, DisplayKey, HostKey, PowerOp, DISPLAY_KEY_NAMES};
use crate::device::Device;
use crate::protocol::{self as p, Effect, MacroEvent};
use crate::setup::{self, Check};
use crate::theme::{self, Icons, ACCENT, BAD, DIM, INK, LINE, OK, WARN};
use crate::pad::{self, Kind};
use crate::{actions, apply, audio, fx, icons, images, keys, stats};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::BTreeMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use crate::i18n::t;

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Eclairage,
    Touches,
    Ecrans,
    Dock,
    Pads,
    Systeme,
}

const PAGES: [(Page, &str, &str); 6] = [
    (Page::Eclairage, "palette", "Éclairage"),
    (Page::Touches, "keyboard", "Touches & macros"),
    (Page::Ecrans, "screens", "Touches écran"),
    (Page::Dock, "dial", "Dock média"),
    (Page::Pads, "pad", "Pads"),
    (Page::Systeme, "settings", "Système"),
];

const EFFECT_ICONS: [&str; 9] = ["fx-static", "fx-wave", "fx-tornado", "fx-breath", "fx-reactive", "fx-matrix", "brush", "fx-yeti", "power"];
const DARK_INK: Color32 = Color32::from_rgb(4, 24, 46);

const SAVER_TYPES: [(u8, &str); 13] = [
    (0, "Image"), (1, "Horloge"), (2, "Horloge (variante)"), (3, "Minuteur"), (4, "Chronomètre"), (7, "Volume"), (8, "Luminosité"),
    (9, "PC info — CPU"), (10, "PC info — GPU"), (11, "PC info — disque"), (12, "PC info — RAM"), (13, "PC info — internet"), (14, "APM"),
];
const DOCK_MENUS: [(u8, &str); 8] = [(0, "Horloge"), (1, "Profil"), (2, "Éclairage"), (3, "Volume"), (4, "Luminosité"), (5, "PC info"), (6, "APM"), (7, "Personnalisé")];

#[derive(Clone, Copy, PartialEq)]
struct DockEdit {
    menus: u8,
    color: [u8; 3],
    saver_type: u8,
    saver_on: bool,
    off_on: bool,
    saver_secs: u16,
    off_secs: u16,
}

impl DockEdit {
    fn from(i: &[u8; 29]) -> DockEdit {
        let s = i[p::EXT_SCREEN_SETUP];
        DockEdit {
            menus: i[p::EXT_SHOW_MENU],
            color: [i[p::EXT_MENU_COLOR], i[p::EXT_MENU_COLOR + 1], i[p::EXT_MENU_COLOR + 2]],
            saver_type: s >> 4,
            saver_on: s & 1 != 0,
            off_on: s & 2 != 0,
            saver_secs: u16::from_le_bytes([i[p::EXT_SAVER], i[p::EXT_SAVER + 1]]).clamp(5, 1800),
            off_secs: u16::from_le_bytes([i[p::EXT_TURNOFF], i[p::EXT_TURNOFF + 1]]).clamp(5, 1800),
        }
    }

    fn setup(&self) -> u8 {
        self.saver_type << 4 | (self.off_on as u8) << 1 | self.saver_on as u8
    }
}

#[derive(Default)]
struct Rec {
    events: Vec<MacroEvent>,
    done: Option<Result<(), String>>,
}

struct Recorder {
    key: &'static str,
    stop: Arc<std::sync::atomic::AtomicBool>,
    rec: Arc<Mutex<Rec>>,
}

fn record_macro(stop: Arc<std::sync::atomic::AtomicBool>, out: Arc<Mutex<Rec>>, ctx: egui::Context) {
    let (started, mut last) = (Instant::now(), Instant::now());
    let r = crate::input::read_keys(|ev| {
        let mut s = out.lock().unwrap();
        if let Some(k) = ev.and_then(|(code, down)| crate::input::name_of(code).and_then(keys::by_name).filter(|k| k.dll != 0).map(|k| (k.dll, down))) {
            let (key, down) = k;
            if down || s.events.iter().any(|e| e.key == key && e.action == 2) {
                if let Some(prev) = s.events.last_mut() {
                    prev.delay = last.elapsed().as_millis().min(65535) as u16;
                }
                s.events.push(MacroEvent { key, page: 0, action: if down { 2 } else { 1 }, delay: 30 });
                last = Instant::now();
                ctx.request_repaint();
            }
        }
        !stop.load(std::sync::atomic::Ordering::Relaxed) && started.elapsed() < Duration::from_secs(120) && s.events.len() < 400
    });
    out.lock().unwrap().done = Some(r);
    ctx.request_repaint();
}

type Task = Box<dyn FnOnce(&mut Device, &dyn Fn(f32)) -> Result<(), String> + Send>;

#[derive(Default)]
struct JobState {
    message: String,
    progress: Option<f32>,
    pending: usize,
    ok: Option<bool>,
}

pub struct App {
    cfg: Config,
    saved: Config,
    page: Page,
    save_flash: bool,
    brush: [u8; 3],
    erase: bool,
    selected: Option<&'static str>,
    state: Arc<Mutex<JobState>>,
    tasks: mpsc::Sender<(String, Option<Kind>, Task)>,
    sinks: Vec<(String, String)>,
    previews: Vec<Option<(DisplayKey, egui::TextureHandle)>>,
    pad_tab: Kind,
    pad_sel: usize,
    pad_page: usize,
    dock_preview: Option<(String, egui::TextureHandle)>,
    logo: Option<egui::TextureHandle>,
    icons: Icons,
    connected: bool,
    service_on: bool,
    last_poll: Instant,
    checks: Vec<Check>,
    show_setup: bool,
    setup_result: Arc<Mutex<Option<(Config, Vec<String>)>>>,
    setup_running: bool,
    gallery: Option<usize>,
    gallery_search: String,
    learning: Option<Arc<Mutex<Option<Result<(usize, u8), String>>>>>,
    learning_fn: bool,
    start: Instant,
    ext: Arc<Mutex<Option<[u8; 29]>>>,
    dock_edit: Option<DockEdit>,
    audio_list: Vec<audio::Source>,
    ifaces: Vec<(String, Option<u32>)>,
    update_info: Option<String>,
    update_confirm: bool,
    update_pending: bool,
    show_translate: bool,
    show_heat: bool,
    heat: (Instant, BTreeMap<String, u64>),
    recorder: Option<Recorder>,
}

fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t) as u8)
}

fn scale(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * k.clamp(0.0, 1.0)) as u8)
}

fn hue(h: f32) -> [u8; 3] {
    let c: Color32 = egui::ecolor::Hsva::new(h.rem_euclid(1.0), 1.0, 1.0, 1.0).into();
    [c.r(), c.g(), c.b()]
}

fn hash(x: f32) -> f32 {
    ((x * 127.1).sin() * 43758.547).fract().abs()
}

fn preview(e: &Effect, custom: &BTreeMap<u8, [u8; 3]>, matrix: u8, nx: f32, ny: f32, t: f32) -> Color32 {
    let sp = 0.25 + e.speed as f32 / 100.0 * 2.0;
    let (x, y) = match e.direction {
        1 => (1.0 - nx, ny),
        2 => (ny, nx),
        3 => (1.0 - ny, nx),
        _ => (nx, ny),
    };
    let alt = e.color2.unwrap_or(scale(e.color, 0.12));
    let rgb = match e.effect {
        0 => e.color,
        1 if e.rainbow => hue(x - t * sp * 0.25),
        1 => mix(e.color, alt, ((x * 6.0 - t * sp * 2.0).sin() + 1.0) / 2.0),
        2 => {
            let a = ((y - 0.5).atan2((x - 0.5) * 2.4) / std::f32::consts::TAU + 0.5 + t * sp * 0.2).fract();
            if e.rainbow { hue(a) } else { mix(e.color, alt, a) }
        }
        3 => {
            let k = 0.08 + 0.92 * (((t * sp * 1.5).sin() + 1.0) / 2.0);
            scale(if e.rainbow { hue(t * 0.05) } else { e.color }, k)
        }
        4 => {
            let k = matrix as f32;
            let beat = t * sp * 0.6 + hash(k * 0.37);
            let v = if hash(k * 7.3 + beat.floor()) > 0.86 { (1.0 - beat.fract()).powi(3) } else { 0.0 };
            mix(e.background, if e.rainbow { hue(x) } else { e.color }, v)
        }
        5 => mix(e.background, e.color, (y * 1.5 - t * sp * 0.6 + hash(x)).fract().powf(4.0)),
        6 => custom.get(&matrix).copied().unwrap_or([6, 18, 32]),
        7 => mix(e.color, [235, 248, 255], ((x * 9.0 + t * sp).sin() * 0.5 + 0.5) * 0.6),
        _ => [0, 0, 0],
    };
    c32(scale(rgb, 0.15 + 0.85 * e.brightness as f32 / 100.0))
}

fn pick_file(title: &str, images_only: bool) -> Option<String> {
    let mut args = vec!["--file-selection".to_string(), "--title".into(), title.into()];
    if images_only {
        args.push("--file-filter=Images | *.png *.jpg *.jpeg *.gif *.bmp *.webp".into());
    }
    let out = std::process::Command::new("zenity").args(&args).output()
        .or_else(|_| std::process::Command::new("kdialog").args(["--getopenfilename", "."]).output()).ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn save_file(title: &str, name: &str) -> Option<String> {
    let out = std::process::Command::new("zenity").args(["--file-selection", "--save", "--confirm-overwrite", "--title", title, "--filename", name]).output()
        .or_else(|_| std::process::Command::new("kdialog").args(["--getsavefilename", name]).output()).ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn binding_color(b: Option<&Binding>) -> Color32 {
    match b {
        None | Some(Binding::ParDefaut) => Color32::from_rgb(0x12, 0x30, 0x52),
        Some(Binding::Desactivee) => Color32::from_rgb(0x5A, 0x22, 0x2E),
        Some(Binding::Touche { .. }) => Color32::from_rgb(0x1E, 0x6F, 0xB8),
        Some(Binding::Raccourci { .. }) => Color32::from_rgb(0x1F, 0x8F, 0x86),
        Some(Binding::Fonction { .. }) => Color32::from_rgb(0xB8, 0x86, 0x1E),
        Some(Binding::Macro { .. }) => Color32::from_rgb(0x7E, 0x4C, 0xC4),
    }
}

fn group_label(g: &str) -> (&'static str, &'static str) {
    match g {
        "Media" => ("music", t("Multimédia")),
        "Mouse" => ("mouse", t("Souris")),
        "Profile" => ("layers", t("Profils")),
        "OS Commands" => ("cpu", t("Système")),
        "Keyboard Shortcuts" => ("keyboard", t("Touches F13–F24")),
        _ => ("bolt", t("Autres")),
    }
}

fn learn_key() -> Result<(usize, u8), String> {
    let mut d = Device::open()?;
    let mut prev = [0u8; 64];
    let end = Instant::now() + Duration::from_secs(6);
    while Instant::now() < end {
        if let Some(r) = d.read(Duration::from_millis(200)) {
            if r[0] == 0x01 {
                for byte in 1..64 {
                    let newly = r[byte] & !prev[byte];
                    if newly != 0 {
                        return Ok((byte, newly.trailing_zeros() as u8));
                    }
                }
                prev = r;
            }
        }
    }
    Err(t("aucune touche détectée").into())
}

fn keyboard(ui: &mut egui::Ui, layout: &str, fill: &dyn Fn(&str, f32, f32) -> Color32, badge: &dyn Fn(&str) -> bool, selected: Option<&str>, paint: bool) -> Option<&'static str> {
    let rows = keys::rows(layout);
    let width = rows.iter().map(|r| r.iter().map(|c| c.2).sum::<f32>()).fold(0.0, f32::max);
    let u = ((ui.available_width() - 28.0) / width).clamp(24.0, 46.0);
    let size = Vec2::new(width * u + 28.0, 6.0 * u + 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, if paint { Sense::click_and_drag() } else { Sense::click() });
    let painter = ui.painter_at(rect.expand(4.0));
    painter.rect_filled(rect, 10.0, Color32::from_rgb(0x05, 0x16, 0x2A));
    painter.rect_stroke(rect, 10.0, Stroke::new(1.0_f32, LINE), egui::StrokeKind::Inside);
    theme::corner_ticks(&painter, rect.shrink(3.0), ACCENT.gamma_multiply(0.7));
    let pointer = resp.interact_pointer_pos().or_else(|| resp.hover_pos());
    let hit = resp.clicked() || (paint && resp.dragged());
    let mut out = None;
    for (r, row) in rows.iter().enumerate() {
        let mut x = rect.left() + 14.0;
        for &(label, name, w) in row.iter() {
            if !name.is_empty() {
                let k = Rect::from_min_size(Pos2::new(x + 2.0, rect.top() + 14.0 + r as f32 * u + 2.0), Vec2::new(w * u - 4.0, u - 4.0));
                let nx = (k.center().x - rect.left()) / rect.width();
                let ny = (k.center().y - rect.top()) / rect.height();
                let col = fill(name, nx, ny);
                let lum = col.r() as f32 * 0.3 + col.g() as f32 * 0.59 + col.b() as f32 * 0.11;
                if lum > 30.0 {
                    painter.rect_filled(k.expand(3.0), 7.0, col.gamma_multiply(0.22));
                }
                painter.rect_filled(k, 5.0, col);
                painter.rect_filled(Rect::from_min_size(k.min + Vec2::new(3.0, 2.0), Vec2::new(k.width() - 6.0, 2.0)), 1.0, Color32::from_white_alpha(28));
                let hovered = resp.hovered() && pointer.is_some_and(|p| k.contains(p));
                let stroke = if selected == Some(name) {
                    Stroke::new(2.0_f32, Color32::from_rgb(0xFF, 0xD1, 0x66))
                } else if hovered {
                    Stroke::new(1.5_f32, ACCENT)
                } else {
                    Stroke::new(1.0_f32, Color32::from_white_alpha(22))
                };
                painter.rect_stroke(k, 5.0, stroke, egui::StrokeKind::Inside);
                let txt = if lum > 150.0 { Color32::from_rgb(0x05, 0x16, 0x2A) } else { INK };
                painter.text(k.center(), egui::Align2::CENTER_CENTER, if matches!(label, "Impr" | "Défil" | "Orig" | "Suppr" | "Fin" | "Entrée" | "Espace" | "Maj") { t(label) } else { label }, egui::FontId::proportional((u * 0.3).clamp(9.0, 13.0)), txt);
                if badge(name) {
                    painter.circle_filled(Pos2::new(k.right() - 5.0, k.top() + 5.0), 3.0, Color32::from_rgb(0xFF, 0xD1, 0x66));
                }
                if hit && pointer.is_some_and(|p| k.contains(p)) {
                    out = Some(name);
                }
            }
            x += w * u;
        }
    }
    out
}

fn close_button(ui: &mut egui::Ui, icons: &mut Icons, ctx: &egui::Context) -> bool {
    ui.add(egui::Button::image(icons.image(ctx, "x", 12.0, DIM)).small()).on_hover_text(t("Supprimer")).clicked()
}

fn segmented(ui: &mut egui::Ui, icons: &mut Icons, items: &[(&str, &str)], current: usize) -> Option<usize> {
    let mut out = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, (icon, label)) in items.iter().enumerate() {
            let on = i == current;
            let img = icons.image(ui.ctx(), icon, 15.0, if on { DARK_INK } else { ACCENT });
            let b = egui::Button::image_and_text(img, RichText::new(*label).color(if on { DARK_INK } else { INK })).fill(if on { ACCENT } else { Color32::TRANSPARENT });
            if ui.add(b).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                out = Some(i);
            }
        }
    });
    out
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> App {
        theme::install(&cc.egui_ctx);
        let mut cfg = Config::load();
        crate::i18n::set(&cfg.lang);
        for i in 0..4 {
            let real = matches!(cfg.bindings.get(DISPLAY_KEY_NAMES[i]), Some(Binding::Raccourci { .. } | Binding::Fonction { .. } | Binding::Macro { .. }))
                || matches!(cfg.bindings.get(DISPLAY_KEY_NAMES[i]), Some(Binding::Touche { target }) if *target != tf!("F{}", 13 + i));
            if real {
                cfg.display_keys[i].action = Action::Aucune;
            }
        }
        let state = Arc::new(Mutex::new(JobState::default()));
        let (tx, rx) = mpsc::channel::<(String, Option<Kind>, Task)>();
        let st = state.clone();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            for (what, kind, task) in rx {
                {
                    let mut s = st.lock().unwrap();
                    s.message = format!("{what}…");
                    s.progress = None;
                    s.ok = None;
                }
                ctx.request_repaint();
                let progress = |x: f32| {
                    st.lock().unwrap().progress = Some(x);
                    ctx.request_repaint();
                };
                let r = kind.map_or_else(Device::open, pad::open).and_then(|mut d| task(&mut d, &progress));
                let mut s = st.lock().unwrap();
                s.pending = s.pending.saturating_sub(1);
                s.progress = None;
                s.ok = Some(r.is_ok());
                s.message = match r {
                    Ok(()) => tf!("{} — terminé", what),
                    Err(e) => format!("{what} — {e}"),
                };
                ctx.request_repaint();
            }
        });
        let checks = setup::run(&cfg);
        let update_info = setup::update_needed();
        let show_setup = update_info.is_none() && !cfg.skip_setup && checks.iter().any(|c| !c.ok && c.item != setup::Item::Device);
        let logo = icons::rasterize_svg(icons::APP_ICON, 96, 96)
            .map(|pm| cc.egui_ctx.load_texture("logo", egui::ColorImage::from_rgba_premultiplied([96, 96], pm.data()), egui::TextureOptions::LINEAR));
        let app = App {
            saved: cfg.clone(),
            cfg,
            page: Page::Eclairage,
            save_flash: true,
            brush: [0x5F, 0xB4, 0xFF],
            erase: false,
            selected: None,
            state,
            tasks: tx,
            sinks: actions::sinks(),
            previews: vec![None; 4 + p::PAD_KEYS],
            pad_tab: Kind::MacroPad,
            pad_sel: 0,
            pad_page: 0,
            dock_preview: None,
            logo,
            icons: Icons::default(),
            connected: crate::device::find_hidraw().is_ok(),
            service_on: setup::service_active(),
            last_poll: Instant::now(),
            checks,
            show_setup,
            setup_result: Arc::new(Mutex::new(None)),
            setup_running: false,
            gallery: None,
            gallery_search: String::new(),
            learning: None,
            learning_fn: false,
            start: Instant::now(),
            ext: Arc::new(Mutex::new(None)),
            dock_edit: None,
            audio_list: audio::sources(),
            ifaces: stats::interfaces(),
            update_info,
            update_confirm: false,
            update_pending: false,
            show_translate: false,
            show_heat: false,
            heat: (Instant::now() - Duration::from_secs(60), BTreeMap::new()),
            recorder: None,
        };
        app.read_ext();
        app
    }

    fn read_ext(&self) {
        let ext = self.ext.clone();
        self.job(t("Lecture du clavier"), move |d, _| {
            *ext.lock().unwrap() = Some(d.extend_info()?);
            Ok(())
        });
    }

    fn profiles_enabled(&self) -> Option<[bool; 5]> {
        self.ext.lock().unwrap().map(|i| p::profiles_enabled(&i[p::EXT_PROFILES..p::EXT_PROFILES + 5]))
    }

    fn set_profile_enabled(&self, n: u8, on: bool) {
        let Some(mut en) = self.profiles_enabled() else { return };
        en[n as usize - 1] = on;
        if !en.iter().any(|&e| e) {
            self.say(t("Il faut au moins un profil actif"), false);
            return;
        }
        let ext = self.ext.clone();
        self.job(&if on { tf!("Profil {} activé", n) } else { tf!("Profil {} désactivé", n) }, move |d, _| {
            let info = apply::set_profiles(d, en)?;
            *ext.lock().unwrap() = Some(info);
            Ok(())
        });
    }

    fn job(&self, what: &str, f: impl FnOnce(&mut Device, &dyn Fn(f32)) -> Result<(), String> + Send + 'static) {
        self.job_on(None, what, f);
    }

    fn job_on(&self, kind: Option<Kind>, what: &str, f: impl FnOnce(&mut Device, &dyn Fn(f32)) -> Result<(), String> + Send + 'static) {
        self.state.lock().unwrap().pending += 1;
        let _ = self.tasks.send((what.to_string(), kind, Box::new(f)));
    }

    fn say(&self, msg: impl Into<String>, ok: bool) {
        let mut s = self.state.lock().unwrap();
        s.message = msg.into();
        s.ok = Some(ok);
    }

    fn layout(&self) -> String {
        if self.cfg.layout.is_empty() { keys::default_layout(crate::i18n::current()).to_string() } else { self.cfg.layout.clone() }
    }

    fn layout_menu(ui: &mut egui::Ui, layout: &mut String) {
        let shown = keys::LAYOUTS.iter().find(|l| l.0 == layout.as_str()).map_or(layout.as_str(), |l| l.1).to_string();
        egui::ComboBox::from_id_salt("quick-layout").selected_text(shown).width(170.0).show_ui(ui, |ui| {
            for (code, name) in keys::LAYOUTS {
                ui.selectable_value(layout, code.to_string(), name);
            }
        });
        ui.label(RichText::new(t("Disposition")).color(DIM));
    }

    fn ic(&mut self, ctx: &egui::Context, name: &str) -> egui::Image<'static> {
        self.icons.image(ctx, name, 16.0, ACCENT)
    }

    fn sidebar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::left("nav").exact_size(236.0).resizable(false)
            .frame(egui::Frame::default().fill(theme::PANEL).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::symmetric(14, 18)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    if let Some(l) = &self.logo {
                        ui.add(egui::Image::new((l.id(), Vec2::splat(44.0))));
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new("EVEREST MAX").font(theme::title_font(18.0)).color(INK));
                        ui.label(RichText::new(t("BASE CAMP · LINUX")).font(theme::title_font(10.5)).color(ACCENT));
                    });
                });
                ui.add_space(14.0);
                egui::Frame::default().fill(Color32::from_rgba_premultiplied(2, 7, 13, 120)).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(4).inner_margin(10).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        theme::status_dot(ui, self.connected);
                        ui.label(RichText::new(if self.connected { t("Clavier connecté") } else { t("Clavier absent") }).color(if self.connected { INK } else { BAD }));
                    });
                    ui.horizontal(|ui| {
                        theme::status_dot(ui, self.service_on);
                        ui.label(RichText::new(if self.service_on { t("Service actif") } else { t("Service arrêté") }).color(DIM));
                    });
                });
                ui.add_space(14.0);
                ui.label(RichText::new(t("PROFIL")).font(theme::title_font(11.0)).color(DIM));
                let enabled = self.profiles_enabled();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for n in 1..=5u8 {
                        let sel = self.cfg.profile == n;
                        let on = enabled.is_none_or(|e| e[n as usize - 1]);
                        let col = if sel { DARK_INK } else if on { INK } else { DIM.gamma_multiply(0.6) };
                        let b = egui::Button::new(RichText::new(n.to_string()).font(theme::title_font(14.0)).color(col))
                            .min_size(Vec2::new(34.0, 30.0)).fill(if sel { ACCENT } else { Color32::TRANSPARENT })
                            .stroke(if on { Stroke::new(1.0_f32, LINE) } else { Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(20, 20, 20, 60)) });
                        let state = if on { t("Actif sur le clavier — clic droit pour désactiver") } else { t("Désactivé sur le clavier — clic droit pour activer") };
                        let name = self.cfg.profile_name(n);
                        let r = ui.add(b).on_hover_text(if name.is_empty() { state.to_string() } else { format!("{name}\n{state}") });
                        if r.clicked() {
                            self.cfg.switch_profile(n);
                        }
                        r.context_menu(|ui| {
                            if ui.button(if on { t("Désactiver sur le clavier") } else { t("Activer sur le clavier") }).clicked() {
                                self.set_profile_enabled(n, !on);
                                ui.close();
                            }
                        });
                    }
                });
                ui.add_space(18.0);
                for (page, icon, label) in PAGES {
                    let warn = page == Page::Systeme && self.checks.iter().any(|c| !c.ok);
                    let r = theme::nav_button(ui, &mut self.icons, icon, t(label), self.page == page);
                    if warn {
                        ui.painter().circle_filled(Pos2::new(r.rect.right() - 14.0, r.rect.center().y), 4.0, WARN);
                    }
                    if r.clicked() {
                        self.page = page;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().color(DIM));
                    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 60.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        let cur = crate::i18n::current();
                        let mut picked = None;
                        for (i, (code, name)) in crate::i18n::LANGS.iter().enumerate() {
                            let b = |name: &str| egui::Button::new(RichText::new(name).small().color(if cur == *code { DARK_INK } else { INK })).fill(if cur == *code { ACCENT } else { Color32::TRANSPARENT });
                            if i + 1 < crate::i18n::LANGS.len() {
                                if ui.add(b(name)).clicked() {
                                    picked = Some(*code);
                                }
                            } else {
                                ui.vertical(|ui| {
                                    if ui.add(b(name)).clicked() {
                                        picked = Some(*code);
                                    }
                                    if ui.add(egui::Button::new(RichText::new(t("Autres")).small())).clicked() {
                                        self.show_translate = true;
                                    }
                                });
                            }
                        }
                        if let Some(code) = picked {
                            self.cfg.lang = code.to_string();
                            crate::i18n::set(code);
                            self.refresh();
                            if self.update_info.is_some() {
                                self.update_info = setup::update_needed();
                            }
                            *self.state.lock().unwrap() = JobState::default();
                        }
                    });
                    ui.add(self.icons.image(ctx, "globe", 14.0, DIM));
                });
            });
    }

    fn statusbar(&mut self, root: &mut egui::Ui, ctx: &egui::Context) {
        let (msg, progress, pending, ok) = {
            let s = self.state.lock().unwrap();
            (s.message.clone(), s.progress, s.pending, s.ok)
        };
        egui::Panel::bottom("status").exact_size(36.0)
            .frame(egui::Frame::default().fill(theme::PANEL).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::symmetric(16, 6)))
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    let (icon, col) = match ok {
                        Some(true) => ("check", OK),
                        Some(false) => ("warn", BAD),
                        None if pending > 0 => ("send", ACCENT),
                        None => ("eye", DIM),
                    };
                    ui.add(self.icons.image(ctx, icon, 16.0, col));
                    ui.label(RichText::new(if msg.is_empty() { t("Prêt").to_string() } else { msg }).color(if ok == Some(false) { BAD } else { INK }));
                    if let Some(p) = progress {
                        ui.add(egui::ProgressBar::new(p).desired_width(220.0).show_percentage().fill(ACCENT));
                    } else if pending > 0 {
                        ui.add(egui::Spinner::new().color(ACCENT));
                    }
                    if pending > 1 {
                        ui.label(RichText::new(tf!("{} en attente", pending - 1)).small().color(DIM));
                    }
                });
            });
    }

    fn page_lighting(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_header(ui, t("Éclairage"), t("Effets intégrés au clavier, effets animés par l'ordinateur et dessin touche par touche."));
        let now = self.start.elapsed().as_secs_f32();
        let eff = self.cfg.effect.effect;
        let custom = eff == p::CUSTOM;
        let soft = fx::soft_index(eff).is_some();
        if soft && !self.service_on {
            ui.horizontal(|ui| {
                ui.add(self.icons.image(ctx, "warn", 16.0, WARN));
                ui.label(RichText::new(t("Cet effet est animé par le service, qui est arrêté. Démarre-le dans Système.")).color(WARN));
            });
        }
        let layout = self.layout();
        let img = self.ic(ctx, if custom { "brush" } else { "eye" });
        let title = if custom { t("Dessin — clique ou glisse sur les touches") } else { t("Aperçu en direct") };
        let mut quick = layout.clone();
        theme::card_with(ui, img, title, |ui| Self::layout_menu(ui, &mut quick), |ui| {
            let e = self.cfg.effect.clone();
            let c = self.cfg.custom.clone();
            let frame = soft.then(|| fx::frame(eff, &e, now, &fx::demo_inputs(now)));
            let fill = |n: &str, x: f32, y: f32| {
                let m = keys::by_name(n).map(|k| k.matrix).unwrap_or(255);
                match &frame {
                    Some(f) if (m as usize) < p::CUSTOM_KEYS => c32(f[m as usize]),
                    Some(_) => Color32::from_rgb(6, 18, 32),
                    None => preview(&e, &c, m, x, y, now),
                }
            };
            if let Some(name) = keyboard(ui, &layout, &fill, &|_| false, None, custom) {
                if let Some(k) = keys::by_name(name).filter(|k| (k.matrix as usize) < p::CUSTOM_KEYS) {
                    if self.erase {
                        self.cfg.custom.remove(&k.matrix);
                    } else {
                        self.cfg.custom.insert(k.matrix, self.brush);
                    }
                }
            }
            if custom {
                ui.horizontal(|ui| {
                    for c in [[0x5F, 0xB4, 0xFF], [0xFF, 0xFF, 0xFF], [0xFF, 0x3B, 0x3B], [0xFF, 0x9F, 0x1C], [0xFF, 0xE0, 0x3B], [0x3B, 0xFF, 0x7A], [0x3B, 0xE8, 0xFF], [0x7A, 0x3B, 0xFF], [0xFF, 0x3B, 0xC8]] {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        ui.painter().rect_filled(r, 4.0, c32(c));
                        if self.brush == c && !self.erase {
                            ui.painter().rect_stroke(r.expand(2.0), 5.0, Stroke::new(2.0_f32, INK), egui::StrokeKind::Outside);
                        }
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            self.brush = c;
                            self.erase = false;
                        }
                    }
                    egui::color_picker::color_edit_button_srgb(ui, &mut self.brush);
                    ui.separator();
                    if theme::icon_button(ui, &mut self.icons, "eraser", t("Gomme"), self.erase).clicked() {
                        self.erase = !self.erase;
                    }
                    if theme::icon_button(ui, &mut self.icons, "fill", t("Tout remplir"), false).clicked() {
                        for k in keys::keys().iter().filter(|k| (k.matrix as usize) < p::CUSTOM_KEYS) {
                            self.cfg.custom.insert(k.matrix, self.brush);
                        }
                    }
                    if theme::icon_button(ui, &mut self.icons, "trash", t("Tout effacer"), false).clicked() {
                        self.cfg.custom.clear();
                    }
                });
            }
        });
        if quick != layout {
            self.cfg.layout = quick;
        }
        if soft || !matches!(eff, 0 | 6 | 8) {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            let img = self.ic(ctx, "palette");
            theme::card(&mut cols[0], img, t("Effet"), |ui| {
                ui.label(RichText::new(t("Intégrés au clavier — marchent sans l'ordinateur")).small().color(DIM));
                egui::Grid::new("fx").spacing(Vec2::splat(8.0)).show(ui, |ui| {
                    for (i, (name, _, _)) in p::EFFECTS.iter().enumerate() {
                        if theme::tile(ui, &mut self.icons, EFFECT_ICONS[i], t(name), eff == i, Vec2::new(112.0, 66.0)).clicked() {
                            self.cfg.effect.effect = i;
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new(t("Animés par l'ordinateur — le service doit tourner")).small().color(DIM));
                egui::Grid::new("fx-soft").spacing(Vec2::splat(8.0)).show(ui, |ui| {
                    for (i, (name, icon)) in fx::SOFT.iter().enumerate() {
                        let id = p::FIRMWARE_EFFECTS + i;
                        if theme::tile(ui, &mut self.icons, icon, t(name), eff == id, Vec2::new(112.0, 66.0)).clicked() {
                            self.cfg.effect.effect = id;
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
            });
            let img = self.ic(ctx, "settings");
            theme::card(&mut cols[1], img, t("Réglages"), |ui| {
                Self::effect_settings(ui, &mut self.icons, ctx, &mut self.cfg.effect);
                if fx::uses_audio(self.cfg.effect.effect) {
                    ui.add_space(6.0);
                    ui.label(RichText::new(t("Écouter")).color(DIM));
                    let sel = &mut self.cfg.audio_sources;
                    let mut all = sel.is_empty() || sel.iter().any(|s| s == audio::ALL_SOUNDS);
                    if theme::toggle(ui, &mut all, t("Tous les sons (sortie par défaut)")).changed() {
                        sel.retain(|s| s != audio::ALL_SOUNDS);
                        if all {
                            sel.push(audio::ALL_SOUNDS.to_string());
                        }
                    }
                    ui.vertical(|ui| {
                        for (output, title, icon) in [(true, t("Sorties"), "headphones"), (false, t("Entrées"), "mic")] {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add(self.icons.image(ctx, icon, 14.0, ACCENT));
                                ui.label(RichText::new(title).font(theme::title_font(12.0)).color(ACCENT));
                            });
                            for src in self.audio_list.iter().filter(|s| s.output == output) {
                                let mut on = sel.contains(&src.name);
                                if theme::toggle(ui, &mut on, &src.label).changed() {
                                    if on {
                                        sel.push(src.name.clone());
                                        if sel.len() == 1 || !all {
                                            sel.retain(|s| s != audio::ALL_SOUNDS);
                                        }
                                    } else {
                                        sel.retain(|s| s != &src.name);
                                    }
                                }
                            }
                        }
                    });
                    let sel = &mut self.cfg.audio_sources;
                    if sel.iter().all(|s| s == audio::ALL_SOUNDS) {
                        sel.clear();
                    }
                }
                if fx::soft_index(self.cfg.effect.effect) == Some(11) {
                    let (iface, max) = self.cfg.net().resolve();
                    ui.label(RichText::new(tf!("Interface {}, 100 % = {} Mbit/s — réglable dans Dock média", iface, max)).small().color(DIM));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if fx::soft_index(self.cfg.effect.effect).is_none() {
                        theme::toggle(ui, &mut self.save_flash, t("Mémoriser dans le clavier"));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, &mut self.icons, "send", t("Appliquer"), true).clicked() {
                            let (cfg, save) = (self.cfg.clone(), self.save_flash);
                            self.job(t("Éclairage"), move |d, _| apply::lighting(d, &cfg, save));
                            if self.cfg.sync_devices {
                                self.sync_devices(save);
                            }
                        }
                    });
                });
                theme::toggle(ui, &mut self.cfg.sync_devices, t("Appliquer aussi au MacroPad et à la couleur des menus du dock"));
            });
        });
    }

    fn sync_devices(&mut self, save: bool) {
        let e = self.cfg.effect.clone();
        if e.effect < p::FIRMWARE_EFFECTS && e.effect != p::CUSTOM {
            self.cfg.macropad.effect = Effect { variant: 0, ..e.clone() };
            if pad::connected(Kind::MacroPad) {
                let (m, c) = (self.cfg.macropad.effect.clone(), self.cfg.macropad.colors);
                self.job_on(Some(Kind::MacroPad), t("Éclairage du MacroPad"), move |d, _| pad::macropad_lighting(d, &m, &c, save));
            }
        }
        let ext = self.ext.clone();
        if let Some(ed) = self.dock_edit.as_mut() {
            ed.color = e.color;
        }
        self.job(t("Couleur des menus"), move |d, _| {
            *ext.lock().unwrap() = Some(apply::set_menu_color(d, e.color)?);
            Ok(())
        });
    }

    fn effect_settings(ui: &mut egui::Ui, icons: &mut Icons, ctx: &egui::Context, e: &mut Effect) {
        let firmware = (e.effect < p::FIRMWARE_EFFECTS).then(|| p::EFFECTS[e.effect].1);
        let si = fx::soft_index(e.effect);
        let rainbow_ok = !matches!(firmware, Some(6 | 10 | 12)) && !matches!(si, Some(1 | 2));
        if !rainbow_ok {
            e.rainbow = false;
        }
        let colors = !matches!(firmware, Some(10 | 12)) && !matches!(si, Some(1 | 2)) && !e.rainbow;
        let color2 = matches!(firmware, Some(1 | 4)) || matches!(si, Some(0 | 6 | 8..=11));
        let background = matches!(firmware, Some(3 | 6 | 9)) || matches!(si, Some(4..=11));
        let speed = !matches!(firmware, Some(0 | 10 | 12)) && !matches!(si, Some(0 | 6..=11));
        egui::Grid::new("fxs").num_columns(2).spacing(Vec2::new(14.0, 10.0)).show(ui, |ui| {
            ui.label(RichText::new(t("Luminosité")).color(DIM));
            ui.add(egui::Slider::new(&mut e.brightness, 0..=100).suffix(" %"));
            ui.end_row();
            if rainbow_ok {
                ui.label(RichText::new(t("Arc-en-ciel")).color(DIM));
                theme::toggle(ui, &mut e.rainbow, "");
                ui.end_row();
            }
            if colors {
                ui.label(RichText::new(t("Couleurs")).color(DIM));
                ui.horizontal(|ui| {
                    egui::color_picker::color_edit_button_srgb(ui, &mut e.color);
                    if color2 {
                        let mut two = e.color2.is_some();
                        if theme::toggle(ui, &mut two, t("Couleur 2")).changed() {
                            e.color2 = two.then_some([255, 255, 255]);
                        }
                        if let Some(c) = e.color2.as_mut() {
                            egui::color_picker::color_edit_button_srgb(ui, c);
                        }
                    }
                });
                ui.end_row();
            }
            if background && !matches!(firmware, Some(10 | 12)) {
                ui.label(RichText::new(t("Fond")).color(DIM));
                egui::color_picker::color_edit_button_srgb(ui, &mut e.background);
                ui.end_row();
            }
            if speed {
                ui.label(RichText::new(t("Vitesse")).color(DIM));
                ui.add(egui::Slider::new(&mut e.speed, 0..=100));
                ui.end_row();
            }
            let dirs: &[(u8, &str, &str)] = match firmware {
                Some(4) => &[(0, "arrow-right", t("Vers la droite")), (1, "arrow-left", t("Vers la gauche")), (2, "arrow-down", t("Vers le bas")), (3, "arrow-up", t("Vers le haut"))],
                Some(7) => &[(0, "reboot", t("Sens horaire")), (1, "undo", t("Sens antihoraire"))],
                _ => &[],
            };
            if !dirs.is_empty() {
                ui.label(RichText::new(t("Sens")).color(DIM));
                ui.horizontal(|ui| {
                    for &(d, icon, tip) in dirs {
                        let on = e.direction == d;
                        let img = icons.image(ctx, icon, 16.0, if on { DARK_INK } else { ACCENT });
                        let b = egui::Button::image(img).min_size(Vec2::new(36.0, 30.0)).fill(if on { ACCENT } else { Color32::TRANSPARENT });
                        if ui.add(b).on_hover_text(tip).clicked() {
                            e.direction = d;
                        }
                    }
                });
                ui.end_row();
            }
        });
    }

    fn binding_editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, name: &'static str) {
        if keys::by_name(name).is_some_and(|k| k.dll == 0) {
            ui.label(RichText::new(t("Cette touche ne peut pas encore être remappée : son identifiant n'est pas connu.")).color(DIM));
            return;
        }
        let kinds: [(&str, &str); 6] = [("undo", t("Par défaut")), ("x", t("Désactivée")), ("arrow-right", t("Autre touche")), ("keyboard", t("Raccourci")), ("playpause", t("Fonction")), ("bolt", t("Macro"))];
        let cur = match self.cfg.bindings.entry(name.to_string()).or_default() {
            Binding::ParDefaut => 0,
            Binding::Desactivee => 1,
            Binding::Touche { .. } => 2,
            Binding::Raccourci { .. } => 3,
            Binding::Fonction { .. } => 4,
            Binding::Macro { .. } => 5,
        };
        ui.horizontal_wrapped(|ui| {
            for (i, (icon, label)) in kinds.iter().enumerate() {
                if theme::tile(ui, &mut self.icons, icon, label, cur == i, Vec2::new(104.0, 64.0)).clicked() && cur != i {
                    let nb = match i {
                        0 => Binding::ParDefaut,
                        1 => Binding::Desactivee,
                        2 => Binding::Touche { target: "A".into() },
                        3 => Binding::Raccourci { key: "C".into(), mods: p::MOD_CTRL },
                        4 => Binding::Fonction { value: 183, label: "Play/Pause".into() },
                        _ => Binding::Macro { mode: 1, events: vec![] },
                    };
                    self.cfg.bindings.insert(name.to_string(), nb);
                    if let Some(d) = DISPLAY_KEY_NAMES.iter().position(|d| *d == name).filter(|_| i >= 2) {
                        if self.cfg.display_keys[d].action != Action::Aucune {
                            self.cfg.display_keys[d].action = Action::Aucune;
                            self.say(tf!("{} : cette fonction remplace l'action ordinateur", name), true);
                        }
                    }
                }
            }
        });
        ui.add_space(6.0);
        let layout = self.layout();
        let key_combo = |ui: &mut egui::Ui, id: &str, v: &mut String| {
            egui::ComboBox::from_id_salt(id).selected_text(keys::label(&layout, v)).height(420.0).show_ui(ui, |ui| {
                for k in keys::keys() {
                    ui.selectable_value(v, k.name.clone(), keys::label(&layout, &k.name));
                }
            });
        };
        let icons = &mut self.icons;
        let recorder = &mut self.recorder;
        let mut message = None;
        let b = self.cfg.bindings.get_mut(name).unwrap();
        match b {
            Binding::Touche { target } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Envoie la touche")).color(DIM));
                    key_combo(ui, "target", target);
                });
            }
            Binding::Raccourci { key, mods } => {
                ui.horizontal(|ui| {
                    for (bit, label) in [(p::MOD_CTRL, "Ctrl"), (p::MOD_SHIFT, t("Maj")), (p::MOD_ALT, "Alt"), (p::MOD_WIN, "Super")] {
                        let on = *mods & bit != 0;
                        let btn = egui::Button::new(RichText::new(label).color(if on { DARK_INK } else { INK })).fill(if on { ACCENT } else { Color32::TRANSPARENT });
                        if ui.add(btn).clicked() {
                            *mods ^= bit;
                        }
                    }
                    ui.label(RichText::new("+").font(theme::title_font(18.0)).color(ACCENT));
                    key_combo(ui, "shortcut", key);
                });
            }
            Binding::Fonction { value, label } => {
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    let mut group = "";
                    for f in keys::functions() {
                        if f.group != group {
                            group = &f.group;
                            let (icon, title) = group_label(group);
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add(icons.image(ctx, icon, 14.0, ACCENT));
                                ui.label(RichText::new(title.to_uppercase()).font(theme::title_font(11.0)).color(ACCENT));
                            });
                        }
                        if ui.selectable_label(*value == f.value, format!("    {}", f.label)).clicked() {
                            *value = f.value;
                            *label = f.label.clone();
                        }
                    }
                });
            }
            Binding::Macro { mode, events } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Lecture")).color(DIM));
                    for (m, l) in [(1u8, t("Une fois")), (4, t("Tant que maintenue")), (3, t("En boucle"))] {
                        ui.selectable_value(mode, m, l);
                    }
                });
                let mut remove = None;
                let mut swap = None;
                egui::Frame::default().fill(Color32::from_rgba_premultiplied(2, 7, 13, 120)).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(4).inner_margin(8).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if events.is_empty() {
                        ui.label(RichText::new(t("Aucun événement — ajoute des touches ci-dessous.")).color(DIM));
                    }
                    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                        for (i, ev) in events.iter_mut().enumerate() {
                            ui.push_id(i, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{:02}", i + 1)).font(theme::title_font(12.0)).color(DIM));
                                    let mouse = ["", t("Clic gauche"), t("Clic droit"), t("Clic milieu")];
                                    let shown = if ev.page == 1 {
                                        match ev.key {
                                            1..=3 => mouse[ev.key as usize].to_string(),
                                            49 => t("Molette ↑").into(),
                                            _ => t("Molette ↓").into(),
                                        }
                                    } else {
                                        keys::keys().iter().find(|k| k.dll == ev.key).map(|k| keys::label(&layout, &k.name)).unwrap_or_default()
                                    };
                                    egui::ComboBox::from_id_salt("k").selected_text(shown).width(150.0).height(420.0).show_ui(ui, |ui| {
                                        for (k, n) in [(1u8, t("Clic gauche")), (2, t("Clic droit")), (3, t("Clic milieu")), (49, t("Molette ↑")), (48, t("Molette ↓"))] {
                                            if ui.selectable_label(ev.page == 1 && ev.key == k, n).clicked() {
                                                *ev = MacroEvent { key: k, page: 1, action: if k >= 48 { 3 } else { ev.action.clamp(1, 2) }, delay: ev.delay };
                                            }
                                        }
                                        ui.separator();
                                        for k in keys::keys() {
                                            if ui.selectable_label(ev.page == 0 && ev.key == k.dll, keys::label(&layout, &k.name)).clicked() {
                                                *ev = MacroEvent { key: k.dll, page: 0, action: if ev.action == 3 { 2 } else { ev.action }, delay: ev.delay };
                                            }
                                        }
                                    });
                                    if ev.action != 3 {
                                        ui.selectable_value(&mut ev.action, 2, t("▼ appui"));
                                        ui.selectable_value(&mut ev.action, 1, t("▲ relâche"));
                                    }
                                    ui.add(egui::DragValue::new(&mut ev.delay).range(0..=65535).suffix(" ms").speed(5));
                                    if ui.small_button("↑").clicked() && i > 0 {
                                        swap = Some(i);
                                    }
                                    if close_button(ui, icons, ctx) {
                                        remove = Some(i);
                                    }
                                });
                            });
                        }
                    });
                });
                if let Some(i) = remove {
                    events.remove(i);
                }
                if let Some(i) = swap {
                    events.swap(i, i - 1);
                }
                ui.horizontal(|ui| {
                    let finished = recorder.as_ref().filter(|r| r.key == name).and_then(|r| {
                        let mut g = r.rec.lock().unwrap();
                        g.done.take().map(|d| (d, std::mem::take(&mut g.events)))
                    });
                    match (finished, recorder.as_ref().filter(|r| r.key == name)) {
                        (Some((Ok(()), ev)), _) => {
                            if !ev.is_empty() {
                                *events = ev;
                            }
                            *recorder = None;
                        }
                        (Some((Err(e), _)), _) => {
                            message = Some(e);
                            *recorder = None;
                        }
                        (None, Some(r)) => {
                            ui.add(egui::Spinner::new().color(BAD));
                            ui.label(RichText::new(tf!("Enregistrement… {} événements", r.rec.lock().unwrap().events.len())).color(BAD));
                            if theme::icon_button(ui, icons, "stop", t("Arrêter"), true).clicked() {
                                r.stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        (None, None) => {
                            if recorder.is_none() && theme::icon_button(ui, icons, "record", t("Enregistrer en direct"), false).clicked() {
                                let r = Recorder { key: name, stop: Default::default(), rec: Default::default() };
                                let (stop, rec, c) = (r.stop.clone(), r.rec.clone(), ctx.clone());
                                std::thread::spawn(move || record_macro(stop, rec, c));
                                *recorder = Some(r);
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if theme::icon_button(ui, icons, "plus", t("Touche (appui + relâche)"), false).clicked() {
                        events.push(MacroEvent { key: 31, page: 0, action: 2, delay: 30 });
                        events.push(MacroEvent { key: 31, page: 0, action: 1, delay: 30 });
                    }
                    if theme::icon_button(ui, icons, "plus", t("Événement"), false).clicked() {
                        events.push(MacroEvent { key: 31, page: 0, action: 2, delay: 50 });
                    }
                });
                ui.label(RichText::new(t("L'enregistrement capte les touches du clavier avec leurs délais, pas la souris.")).small().color(DIM));
            }
            _ => {}
        }
        if let Some(e) = message {
            self.say(e, false);
        }
    }

    fn page_keys(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_header(ui, t("Touches & macros"), t("Remappage, raccourcis, fonctions et macros stockés dans le clavier : ils marchent partout, même sans ce logiciel."));
        let img = self.ic(ctx, "keyboard");
        let mut quick = self.layout();
        theme::card_with(ui, img, t("Clavier — clique une touche"), |ui| Self::layout_menu(ui, &mut quick), |ui| {
            let layout = self.layout();
            let bindings = self.cfg.bindings.clone();
            let dk: Vec<bool> = self.cfg.display_keys.iter().map(|d| d.action != Action::Aucune).collect();
            if self.show_heat && self.heat.0.elapsed() > Duration::from_secs(2) {
                self.heat = (Instant::now(), crate::input::load_heatmap());
                ctx.request_repaint_after(Duration::from_secs(2));
            }
            let (heat, max) = (&self.heat.1, self.heat.1.values().copied().max().unwrap_or(1).max(1) as f32);
            let show_heat = self.show_heat;
            let fill = |n: &str, _: f32, _: f32| {
                if !show_heat {
                    return binding_color(bindings.get(n));
                }
                let v = (*heat.get(n).unwrap_or(&0) as f32 / max).sqrt();
                c32(if v < 0.5 { mix([16, 32, 64], [95, 180, 255], v * 2.0) } else { mix([95, 180, 255], [255, 80, 60], v * 2.0 - 1.0) })
            };
            let badge = |n: &str| DISPLAY_KEY_NAMES.iter().position(|d| *d == n).is_some_and(|i| dk[i]);
            if let Some(k) = keyboard(ui, &layout, &fill, &badge, self.selected, false) {
                self.selected = Some(k);
            }
            ui.horizontal_wrapped(|ui| {
                for (c, l) in [
                    (binding_color(None), t("Par défaut")),
                    (binding_color(Some(&Binding::Desactivee)), t("Désactivée")),
                    (binding_color(Some(&Binding::Touche { target: String::new() })), t("Autre touche")),
                    (binding_color(Some(&Binding::Raccourci { key: String::new(), mods: 0 })), t("Raccourci")),
                    (binding_color(Some(&Binding::Fonction { value: 0, label: String::new() })), t("Fonction")),
                    (binding_color(Some(&Binding::Macro { mode: 0, events: vec![] })), t("Macro")),
                    (Color32::from_rgb(0xFF, 0xD1, 0x66), t("Action ordinateur")),
                ] {
                    theme::chip(ui, c, l);
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                theme::toggle(ui, &mut self.cfg.heatmap, t("Compter les frappes"));
                theme::toggle(ui, &mut self.show_heat, t("Afficher la carte de chaleur"));
                if self.show_heat {
                    ui.label(RichText::new(tf!("{} frappes", self.heat.1.values().sum::<u64>())).color(DIM));
                    if ui.small_button(t("Remettre à zéro")).clicked() {
                        let _ = std::fs::write(crate::input::heatmap_path(), "{}");
                        self.heat.1.clear();
                    }
                }
            });
            if self.cfg.heatmap {
                ui.label(RichText::new(t("Le service compte seulement combien de fois chaque touche est pressée, rien d'autre, et ça reste sur ton ordinateur.")).small().color(DIM));
            }
        });
        if quick != self.layout() {
            self.cfg.layout = quick;
        }
        ui.add_space(6.0);
        let img = self.ic(ctx, "bolt");
        let title = match self.selected {
            Some(n) => tf!("Fonction de la touche {}", n),
            None => t("Fonction de la touche").into(),
        };
        theme::card(ui, img, &title, |ui| {
            let Some(name) = self.selected else {
                ui.label(RichText::new(t("Sélectionne une touche sur le clavier ci-dessus.")).color(DIM));
                return;
            };
            if let Some(i) = DISPLAY_KEY_NAMES.iter().position(|d| *d == name) {
                if self.cfg.display_keys[i].action != Action::Aucune {
                    ui.horizontal(|ui| {
                        ui.add(self.icons.image(ctx, "warn", 16.0, WARN));
                        ui.label(RichText::new(t("Cette touche écran a une action ordinateur (onglet Touches écran). Choisir une fonction ici la remplacera.")).color(WARN));
                    });
                }
            }
            self.binding_editor(ui, ctx, name);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let b = self.cfg.bindings.get(name).cloned().unwrap_or_default();
                let (profile, slot) = (self.cfg.profile, p::slot(self.cfg.effect.effect));
                if theme::icon_button(ui, &mut self.icons, "send", t("Envoyer cette touche"), true).clicked() {
                    self.job(&tf!("Touche {}", name), move |d, _| {
                        d.transact(&p::switch_profile(profile, slot))?;
                        apply::binding(d, name, &b)
                    });
                }
                if theme::icon_button(ui, &mut self.icons, "upload", t("Envoyer toutes les touches"), false).clicked() {
                    let cfg = self.cfg.clone();
                    self.job(t("Toutes les touches"), move |d, _| apply::all_bindings(d, &cfg));
                }
            });
        });
    }

    fn action_editor(ui: &mut egui::Ui, icons: &mut Icons, ctx: &egui::Context, a: &mut Action, sinks: &[(String, String)], pages: &[String], depth: usize) {
        let kinds: [(&str, &str); 14] = [
            ("x", t("Aucune")), ("terminal", t("Lancer un programme")), ("globe", t("Ouvrir un site")), ("folder", t("Ouvrir un dossier")), ("music", t("Jouer un son")),
            ("keyboard", t("Raccourci clavier")), ("power", t("Système")), ("arrow-down", t("Appuyer une touche")), ("arrow-up", t("Relâcher une touche")), ("timer", t("Attendre")), ("layers", t("Plusieurs actions")), ("pad", t("Page du DisplayPad")),
            ("playpause", t("Lecteur multimédia")),
            ("record", t("OBS Studio")),
        ];
        let cur = match a {
            Action::Aucune => 0,
            Action::Commande { .. } => 1,
            Action::Url { .. } => 2,
            Action::Dossier { .. } => 3,
            Action::Son { .. } => 4,
            Action::Raccourci { .. } => 5,
            Action::Systeme { .. } => 6,
            Action::ToucheBas { .. } => 7,
            Action::ToucheHaut { .. } => 8,
            Action::Delai { .. } => 9,
            Action::Multi { .. } => 10,
            Action::Page { .. } => 11,
            Action::Media { .. } => 12,
            Action::Obs { .. } => 13,
        };
        let mut sel = cur;
        ui.horizontal(|ui| {
            ui.add(icons.image(ctx, kinds[cur].0, 18.0, ACCENT));
            egui::ComboBox::from_id_salt("action").selected_text(kinds[cur].1).width(210.0).show_ui(ui, |ui| {
                for (i, (icon, label)) in kinds.iter().enumerate() {
                    if (depth > 0 && i == 10) || (pages.is_empty() && i == 11) {
                        continue;
                    }
                    let img = icons.image(ctx, icon, 16.0, ACCENT);
                    if ui.add(egui::Button::image_and_text(img, *label).selected(sel == i).frame(false)).clicked() {
                        sel = i;
                    }
                }
            });
        });
        if sel != cur {
            *a = match sel {
                1 => Action::Commande { cmd: String::new() },
                2 => Action::Url { url: "https://".into() },
                3 => Action::Dossier { path: String::new() },
                4 => Action::Son { file: String::new(), sink: String::new(), volume: 85 },
                5 => Action::Raccourci { keys: "ctrl+c".into() },
                6 => Action::Systeme { op: PowerOp::Verrouiller },
                7 => Action::ToucheBas { key: "a".into() },
                8 => Action::ToucheHaut { key: "a".into() },
                9 => Action::Delai { ms: 100 },
                10 => Action::Multi { actions: vec![] },
                11 => Action::Page { page: 0 },
                12 => Action::Media { op: "PlayPause".into(), player: String::new() },
                13 => Action::Obs { request: "SetCurrentProgramScene".into(), arg: String::new() },
                _ => Action::Aucune,
            };
        }
        let path = |ui: &mut egui::Ui, v: &mut String, title: &str| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(v).desired_width(220.0));
                if ui.button(t("Parcourir…")).clicked() {
                    if let Some(f) = pick_file(title, false) {
                        *v = f;
                    }
                }
            });
        };
        match a {
            Action::Commande { cmd } => {
                ui.add(egui::TextEdit::singleline(cmd).hint_text("firefox, obs, ~/script.sh …").desired_width(300.0));
            }
            Action::Url { url } => {
                ui.add(egui::TextEdit::singleline(url).desired_width(300.0));
            }
            Action::Dossier { path: p } => path(ui, p, t("Dossier")),
            Action::Son { file, sink, volume } => {
                path(ui, file, t("Fichier son"));
                ui.horizontal(|ui| {
                    ui.add(icons.image(ctx, "vol-up", 16.0, DIM));
                    ui.add(egui::Slider::new(volume, 0..=150).suffix(" %").text(t("Volume")));
                });
                ui.horizontal(|ui| {
                    ui.add(icons.image(ctx, "headphones", 16.0, DIM));
                    let shown = sinks.iter().find(|s| &s.0 == sink).map(|s| s.1.clone()).unwrap_or_else(|| t("Sortie par défaut").into());
                    egui::ComboBox::from_id_salt("sink").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                        ui.selectable_value(sink, String::new(), t("Sortie par défaut"));
                        for (n, d) in sinks {
                            ui.selectable_value(sink, n.clone(), d);
                        }
                    });
                });
            }
            Action::Raccourci { keys } => {
                ui.add(egui::TextEdit::singleline(keys).hint_text("ctrl+shift+esc, super+l, playpause …").desired_width(260.0));
            }
            Action::ToucheBas { key } | Action::ToucheHaut { key } => {
                ui.add(egui::TextEdit::singleline(key).hint_text("a, enter, f13, mouseleft, wheelup").desired_width(200.0));
            }
            Action::Delai { ms } => {
                ui.add(egui::DragValue::new(ms).range(0..=600_000).suffix(" ms").speed(10));
            }
            Action::Systeme { op } => {
                ui.horizontal_wrapped(|ui| {
                    for (v, icon, n) in [(PowerOp::Verrouiller, "lock", t("Verrouiller")), (PowerOp::Veille, "moon", t("Veille")), (PowerOp::Hibernation, "moon", t("Hibernation")), (PowerOp::Deconnexion, "logout", t("Déconnexion")), (PowerOp::Redemarrer, "reboot", t("Redémarrer")), (PowerOp::Eteindre, "power", t("Éteindre"))] {
                        let on = *op == v;
                        let img = icons.image(ctx, icon, 15.0, if on { DARK_INK } else { ACCENT });
                        let b = egui::Button::image_and_text(img, RichText::new(n).color(if on { DARK_INK } else { INK })).fill(if on { ACCENT } else { Color32::TRANSPARENT });
                        if ui.add(b).clicked() {
                            *op = v;
                        }
                    }
                });
            }
            Action::Multi { actions } => {
                let mut remove = None;
                for (i, x) in actions.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        egui::Frame::default().stroke(Stroke::new(1.0_f32, LINE)).corner_radius(4).inner_margin(8).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{:02}", i + 1)).font(theme::title_font(12.0)).color(ACCENT));
                                ui.vertical(|ui| Self::action_editor(ui, icons, ctx, x, sinks, pages, depth + 1));
                                if close_button(ui, icons, ctx) {
                                    remove = Some(i);
                                }
                            });
                        });
                    });
                }
                if let Some(i) = remove {
                    actions.remove(i);
                }
                if theme::icon_button(ui, icons, "plus", t("Ajouter une étape"), false).clicked() {
                    actions.push(Action::Aucune);
                }
            }
            Action::Page { page } => {
                let shown = pages.get(*page).cloned().unwrap_or_default();
                egui::ComboBox::from_id_salt("page").selected_text(shown).width(220.0).show_ui(ui, |ui| {
                    for (i, n) in pages.iter().enumerate() {
                        ui.selectable_value(page, i, n);
                    }
                });
            }
            Action::Media { op, player } => {
                ui.horizontal_wrapped(|ui| {
                    for (v, n) in [("PlayPause", t("Lecture/Pause")), ("Previous", t("Précédent")), ("Next", t("Suivant")), ("Stop", t("Stop"))] {
                        ui.selectable_value(op, v.to_string(), n);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Lecteur")).color(DIM));
                    let shown = if player.is_empty() { t("Le premier ouvert").to_string() } else { player.clone() };
                    egui::ComboBox::from_id_salt("player").selected_text(shown).width(200.0).show_ui(ui, |ui| {
                        ui.selectable_value(player, String::new(), t("Le premier ouvert"));
                        for n in actions::players() {
                            let base = n.split('.').next().unwrap_or(&n).to_string();
                            ui.selectable_value(player, base.clone(), base);
                        }
                    });
                });
            }
            Action::Obs { request, arg } => {
                let shown = crate::obs::REQUESTS.iter().find(|r| r.0 == request.as_str()).map_or("?", |r| t(r.1));
                egui::ComboBox::from_id_salt("obs").selected_text(shown).width(280.0).show_ui(ui, |ui| {
                    for (v, n, _) in crate::obs::REQUESTS {
                        ui.selectable_value(request, v.to_string(), t(n));
                    }
                });
                match request.as_str() {
                    "SetCurrentProgramScene" => {
                        ui.add(egui::TextEdit::singleline(arg).hint_text(t("Nom de la scène")).desired_width(260.0));
                    }
                    "ToggleInputMute" => {
                        ui.add(egui::TextEdit::singleline(arg).hint_text(t("Nom de la source (ex. Mic/Aux)")).desired_width(260.0));
                    }
                    _ => {}
                }
                ui.label(RichText::new(t("Connexion à régler dans Système → OBS Studio.")).small().color(DIM));
            }
            Action::Aucune => {}
        }
    }

    fn art_mut(&mut self, slot: usize) -> &mut DisplayKey {
        if slot < 4 { &mut self.cfg.display_keys[slot] } else { &mut self.cfg.displaypad.page_mut(self.pad_page)[slot - 4] }
    }

    fn art_preview(&mut self, ctx: &egui::Context, slot: usize) -> Option<egui::TextureId> {
        let dk = self.art_mut(slot).clone();
        let size = if slot < 4 { 72 } else { pad::ICON };
        let stale = self.previews[slot].as_ref().is_none_or(|(d, _)| d.image != dk.image || d.icon != dk.icon || d.text != dk.text || d.fg != dk.fg || d.bg != dk.bg || d.grid != dk.grid);
        if stale {
            if let Ok(img) = images::render(size, size, &apply::art(&dk)) {
                let tex = ctx.load_texture(format!("art{slot}"), egui::ColorImage::from_rgb([size as usize; 2], img.as_raw()), egui::TextureOptions::LINEAR);
                self.previews[slot] = Some((dk, tex));
            }
        }
        self.previews[slot].as_ref().map(|(_, t)| t.id())
    }

    fn art_editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, slot: usize) {
        let dk = self.art_mut(slot).clone();
        let tex = self.art_preview(ctx, slot);
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(152.0), Sense::hover());
            ui.painter().rect_filled(r, 10.0, Color32::from_rgb(0x03, 0x0E, 0x1C));
            if let Some(tex) = tex {
                ui.painter().image(tex, r.shrink(4.0), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            ui.painter().rect_stroke(r, 10.0, Stroke::new(1.5_f32, ACCENT.gamma_multiply(0.6)), egui::StrokeKind::Inside);
            theme::corner_ticks(ui.painter(), r.expand(4.0), ACCENT);
            ui.vertical(|ui| {
                let source = if !dk.image.is_empty() { 1 } else if !dk.icon.is_empty() { 0 } else { 2 };
                if let Some(s) = segmented(ui, &mut self.icons, &[("star", t("Icône")), ("image", t("Image")), ("text", t("Texte"))], source) {
                    match s {
                        0 => {
                            self.gallery = Some(slot);
                            self.gallery_search.clear();
                        }
                        1 => {
                            if let Some(f) = pick_file(t("Image de la touche"), true) {
                                let dk = self.art_mut(slot);
                                dk.image = f;
                                dk.icon.clear();
                            }
                        }
                        _ => {
                            let dk = self.art_mut(slot);
                            dk.image.clear();
                            dk.icon.clear();
                        }
                    }
                }
                let dk = self.art_mut(slot);
                ui.add(egui::TextEdit::multiline(&mut dk.text).hint_text(t("Texte (facultatif, s'affiche sous l'icône)")).desired_rows(2).desired_width(240.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Encre")).color(DIM));
                    egui::color_picker::color_edit_button_srgb(ui, &mut dk.fg);
                    ui.label(RichText::new(t("Fond")).color(DIM));
                    egui::color_picker::color_edit_button_srgb(ui, &mut dk.bg);
                    theme::toggle(ui, &mut dk.grid, t("Grille"));
                });
            });
        });
    }

    fn display_key_card(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, i: usize) {
        let img = self.ic(ctx, "screens");
        theme::card(ui, img, &tf!("Touche D{}", i + 1), |ui| {
            self.art_editor(ui, ctx, i);
            let (profile, dk) = (self.cfg.profile, self.cfg.display_keys[i].clone());
            if theme::icon_button(ui, &mut self.icons, "upload", t("Envoyer au clavier"), true).clicked() {
                self.job(&tf!("Image D{}", i + 1), move |d, prog| apply::display_key(d, profile, i as u8, &dk, &mut |x| prog(x)));
            }
            ui.add_space(8.0);
            ui.label(RichText::new(t("ACTION AU APPUI")).font(theme::title_font(11.0)).color(DIM));
            let had = self.cfg.display_keys[i].action != Action::Aucune;
            Self::action_editor(ui, &mut self.icons, ctx, &mut self.cfg.display_keys[i].action, &self.sinks, &[], 0);
            let has = self.cfg.display_keys[i].action != Action::Aucune;
            if had != has || !self.cfg.display_key_in_sync(i) {
                let b = apply::sync_display_key(&mut self.cfg, i);
                let name = DISPLAY_KEY_NAMES[i];
                let what = if has { tf!("{} : frappe d'origine coupée", name) } else { tf!("{} : F{} rétablie", name, 13 + i) };
                self.job(&what, move |d, _| apply::binding(d, name, &b));
            }
            if has && theme::icon_button(ui, &mut self.icons, "play", t("Tester l'action"), false).clicked() {
                let a = self.cfg.display_keys[i].action.clone();
                std::thread::spawn(move || actions::Runner::new().run(&a));
            }
        });
    }

    fn service_warning(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.service_on {
            ui.horizontal(|ui| {
                ui.add(self.icons.image(ctx, "warn", 16.0, WARN));
                ui.label(RichText::new(t("Le service est arrêté : les actions ne se déclencheront pas.")).color(WARN));
                if theme::icon_button(ui, &mut self.icons, "play", t("Démarrer le service"), false).clicked() {
                    match setup::install_service() {
                        Ok(()) => self.say(t("Service démarré"), true),
                        Err(e) => self.say(tf!("Service : {}", e), false),
                    }
                    self.refresh();
                }
            });
            ui.add_space(6.0);
        }
    }

    fn page_screens(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_header(ui, t("Touches écran"), t("Les 4 écrans du pavé (72×72) : icône, image ou texte, et une action exécutée par l'ordinateur."));
        self.service_warning(ui, ctx);
        ui.columns(2, |cols| {
            for i in 0..4 {
                let ui = &mut cols[i % 2];
                ui.push_id(i, |ui| self.display_key_card(ui, ctx, i));
                ui.add_space(8.0);
            }
        });
    }

    fn gallery_window(&mut self, ctx: &egui::Context) {
        let Some(i) = self.gallery else { return };
        let mut open = true;
        let name = if i < 4 { format!("D{}", i + 1) } else { format!("K{}", i - 3) };
        egui::Window::new(RichText::new(tf!("BIBLIOTHÈQUE D'ICÔNES — {}", name)).font(theme::title_font(14.0)).color(ACCENT))
            .open(&mut open).collapsible(false).resizable(false).default_width(760.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(self.icons.image(ctx, "search", 16.0, DIM));
                    ui.add(egui::TextEdit::singleline(&mut self.gallery_search).hint_text(t("Rechercher…")).desired_width(260.0));
                });
                ui.add_space(6.0);
                let q = self.gallery_search.to_lowercase();
                egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                    egui::Grid::new("gallery").spacing(Vec2::splat(8.0)).show(ui, |ui| {
                        let mut n = 0;
                        for (name, label) in icons::LIBRARY.iter().filter(|(n, l)| q.is_empty() || t(l).to_lowercase().contains(&q) || l.to_lowercase().contains(&q) || n.contains(&q)) {
                            let sel = self.art_mut(i).icon == *name;
                            if theme::tile(ui, &mut self.icons, name, t(label), sel, Vec2::new(84.0, 76.0)).clicked() {
                                let dk = self.art_mut(i);
                                dk.icon = name.to_string();
                                dk.image.clear();
                                self.gallery = None;
                            }
                            n += 1;
                            if n % 8 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });
            });
        if !open {
            self.gallery = None;
        }
    }

    fn page_names(&self) -> Vec<String> {
        std::iter::once(t("Principale").to_string()).chain(self.cfg.displaypad.pages.iter().map(|p| p.name.clone())).collect()
    }

    fn page_tabs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let names = self.page_names();
        self.pad_page = self.pad_page.min(names.len() - 1);
        ui.horizontal_wrapped(|ui| {
            ui.add(self.icons.image(ctx, "layers", 16.0, ACCENT));
            ui.label(RichText::new(t("Pages")).color(DIM));
            for (i, n) in names.iter().enumerate() {
                if ui.selectable_label(self.pad_page == i, n).clicked() {
                    self.pad_page = i;
                }
            }
            if theme::icon_button(ui, &mut self.icons, "plus", t("Nouvelle page"), false).clicked() {
                let n = self.cfg.displaypad.pages.len() + 1;
                let mut page = crate::config::PadPage { name: tf!("Page {}", n), ..Default::default() };
                page.keys[0] = DisplayKey { icon: "undo".into(), text: t("Retour").into(), action: Action::Page { page: 0 }, ..Default::default() };
                self.cfg.displaypad.pages.push(page);
                self.pad_page = n;
            }
        });
        if self.pad_page > 0 {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("Nom de la page")).color(DIM));
                ui.add(egui::TextEdit::singleline(&mut self.cfg.displaypad.pages[self.pad_page - 1].name).desired_width(200.0));
                if theme::icon_button(ui, &mut self.icons, "trash", t("Supprimer la page"), false).clicked() {
                    self.cfg.displaypad.remove_page(self.pad_page);
                    self.pad_page -= 1;
                }
            });
            ui.label(RichText::new(t("Pour ouvrir cette page, donne à une touche l'action « Page du DisplayPad ».")).small().color(DIM));
        }
        ui.add_space(6.0);
    }

    fn page_pads(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_header(ui, t("Pads"), t("MacroPad et DisplayPad : 12 touches, chacune avec une action exécutée par l'ordinateur."));
        let kind = self.pad_tab;
        let display = kind == Kind::DisplayPad;
        let on = pad::connected(kind);
        ui.horizontal(|ui| {
            if let Some(i) = segmented(ui, &mut self.icons, &[("pad", "MacroPad"), ("screens", "DisplayPad")], display as usize) {
                self.pad_tab = [Kind::MacroPad, Kind::DisplayPad][i];
                self.pad_sel = 0;
            }
            ui.add_space(12.0);
            theme::status_dot(ui, on);
            ui.label(RichText::new(if on { t("Branché") } else { t("Non détecté") }).color(if on { OK } else { DIM }));
        });
        ui.add_space(6.0);
        self.service_warning(ui, ctx);
        if display {
            self.page_tabs(ui, ctx);
        }
        let name = |i: usize| if display { format!("K{}", i + 1) } else { format!("M{}", i + 1) };
        let e = self.cfg.macropad.effect.clone();
        let custom = e.effect == p::CUSTOM;
        let now = self.start.elapsed().as_secs_f32();
        let img = self.ic(ctx, "pad");
        theme::card(ui, img, kind.name(), |ui| {
            egui::Grid::new("padgrid").spacing(Vec2::splat(10.0)).show(ui, |ui| {
                for i in 0..p::PAD_KEYS {
                    let tex = if display { self.art_preview(ctx, 4 + i) } else { None };
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(92.0), Sense::click());
                    let painter = ui.painter();
                    painter.rect_filled(r, 10.0, Color32::from_rgb(0x03, 0x0E, 0x1C));
                    if let Some(tex) = tex {
                        painter.image(tex, r.shrink(5.0), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    } else {
                        let c = if custom { c32(self.cfg.macropad.colors[i]) } else { preview(&e, &BTreeMap::new(), 255, (i % 6) as f32 / 5.0, (i / 6) as f32, now) };
                        painter.rect_filled(r.shrink(6.0), 8.0, c.gamma_multiply(0.45));
                        painter.text(r.center(), egui::Align2::CENTER_CENTER, name(i), theme::title_font(18.0), INK);
                    }
                    let sel = self.pad_sel == i;
                    painter.rect_stroke(r, 10.0, Stroke::new(if sel { 2.5_f32 } else { 1.0 }, if sel { ACCENT } else { LINE }), egui::StrokeKind::Inside);
                    let has = if display { self.cfg.displaypad.page(self.pad_page)[i].action != Action::Aucune } else { self.cfg.macropad.actions[i] != Action::Aucune };
                    if has {
                        painter.circle_filled(r.right_top() + Vec2::new(-10.0, 10.0), 4.0, OK);
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.pad_sel = i;
                    }
                    if i % 6 == 5 {
                        ui.end_row();
                    }
                }
            });
        });
        if !display && !matches!(e.effect, 0 | 6 | 8) {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        ui.add_space(6.0);
        let sel = self.pad_sel;
        ui.columns(2, |cols| {
            let img = self.ic(ctx, if display { "screens" } else { "keyboard" });
            theme::card(&mut cols[0], img, &tf!("Touche {}", name(sel)), |ui| {
                if display {
                    self.art_editor(ui, ctx, 4 + sel);
                    ui.add_space(8.0);
                }
                ui.label(RichText::new(t("ACTION AU APPUI")).font(theme::title_font(11.0)).color(DIM));
                let names = if display { self.page_names() } else { Vec::new() };
                let a = if display { &mut self.cfg.displaypad.page_mut(self.pad_page)[sel].action } else { &mut self.cfg.macropad.actions[sel] };
                Self::action_editor(ui, &mut self.icons, ctx, a, &self.sinks, &names, 0);
                if *a != Action::Aucune && theme::icon_button(ui, &mut self.icons, "play", t("Tester l'action"), false).clicked() {
                    let a = a.clone();
                    std::thread::spawn(move || actions::Runner::new().run(&a));
                }
                if !display && custom {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("Couleur de la touche")).color(DIM));
                        egui::color_picker::color_edit_button_srgb(ui, &mut self.cfg.macropad.colors[sel]);
                        if theme::icon_button(ui, &mut self.icons, "fill", t("Tout remplir"), false).clicked() {
                            self.cfg.macropad.colors = [self.cfg.macropad.colors[sel]; p::PAD_KEYS];
                        }
                    });
                }
            });
            if display {
                let img = self.ic(ctx, "sun");
                theme::card(&mut cols[1], img, t("Écrans du DisplayPad"), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("Luminosité")).color(DIM));
                        for v in [0u8, 25, 50, 75, 100] {
                            ui.selectable_value(&mut self.cfg.displaypad.brightness, v, format!("{v} %"));
                        }
                    });
                    ui.add_space(6.0);
                    if theme::icon_button(ui, &mut self.icons, "image", t("Une image sur les 12 touches"), false).clicked() {
                        if let Some(f) = pick_file(t("Image à découper"), true) {
                            match pad::split(&f, self.pad_page) {
                                Ok(paths) => {
                                    for (k, path) in self.cfg.displaypad.page_mut(self.pad_page).iter_mut().zip(paths) {
                                        k.image = path;
                                        k.icon.clear();
                                    }
                                }
                                Err(e) => self.say(e, false),
                            }
                        }
                    }
                    ui.label(RichText::new(t("Un GIF animé sur une touche est joué par le service.")).small().color(DIM));
                    ui.add_space(8.0);
                    if self.service_on {
                        ui.label(RichText::new(t("Le service tient le DisplayPad à jour tout seul : les changements s'affichent en une seconde, et il renvoie tout à chaque branchement.")).small().color(OK));
                    } else {
                        let (keys, b) = (self.cfg.displaypad.page(self.pad_page).clone(), self.cfg.displaypad.brightness);
                        if theme::icon_button(ui, &mut self.icons, "upload", t("Envoyer au DisplayPad"), true).clicked() {
                            self.job_on(Some(Kind::DisplayPad), t("Images du DisplayPad"), move |d, prog| pad::push_display(d, &keys, b, prog));
                        }
                    }
                });
            } else {
                let img = self.ic(ctx, "palette");
                theme::card(&mut cols[1], img, t("Éclairage"), |ui| {
                    egui::Grid::new("padfx").spacing(Vec2::splat(8.0)).show(ui, |ui| {
                        for (i, (n, _, _)) in p::EFFECTS.iter().enumerate() {
                            if theme::tile(ui, &mut self.icons, EFFECT_ICONS[i], t(n), self.cfg.macropad.effect.effect == i, Vec2::new(104.0, 62.0)).clicked() {
                                self.cfg.macropad.effect.effect = i;
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                    if p::EFFECTS[self.cfg.macropad.effect.effect.min(p::FIRMWARE_EFFECTS - 1)].1 == 3 {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t("Variante")).color(DIM));
                            for (v, n) in ["A", "B", "C"].iter().enumerate() {
                                ui.selectable_value(&mut self.cfg.macropad.effect.variant, v as u8, *n);
                            }
                        });
                    }
                    ui.add_space(8.0);
                    Self::effect_settings(ui, &mut self.icons, ctx, &mut self.cfg.macropad.effect);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        theme::toggle(ui, &mut self.save_flash, t("Mémoriser dans le pad"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::icon_button(ui, &mut self.icons, "send", t("Appliquer"), true).clicked() {
                                let (e, c, save) = (self.cfg.macropad.effect.clone(), self.cfg.macropad.colors, self.save_flash);
                                self.job_on(Some(Kind::MacroPad), t("Éclairage du MacroPad"), move |d, _| pad::macropad_lighting(d, &e, &c, save));
                            }
                        });
                    });
                });
            }
        });
    }

    fn page_dock(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_header(ui, t("Dock média"), t("Le module à gauche du clavier : écran rond, molette et touches média."));
        let loaded = self.ext.lock().unwrap().is_some();
        if self.dock_edit.is_none() {
            if let Some(info) = *self.ext.lock().unwrap() {
                self.dock_edit = Some(DockEdit::from(&info));
            }
        }
        ui.columns(2, |cols| {
            let img = self.ic(ctx, "image");
            theme::card(&mut cols[0], img, t("Écran de veille"), |ui| {
                ui.label(RichText::new(t("L'écran rond affiche ce visuel quand le dock reste inactif. Pour voir ton image, choisis le type « Image » puis applique.")).small().color(DIM));
                let path = self.cfg.dock_image.clone();
                if self.dock_preview.as_ref().is_none_or(|(p, _)| *p != path) {
                    let art = images::Art { image: &path, icon: if path.is_empty() { "dial" } else { "" }, text: "", fg: [0x5F, 0xB4, 0xFF], bg: images::BLUEPRINT, grid: true };
                    if let Ok(img) = images::render(240, 204, &art) {
                        self.dock_preview = Some((path.clone(), ctx.load_texture("dock", egui::ColorImage::from_rgb([240, 204], img.as_raw()), egui::TextureOptions::LINEAR)));
                    }
                }
                ui.horizontal_top(|ui| {
                    if let Some((_, tex)) = &self.dock_preview {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(200.0, 170.0), Sense::hover());
                        ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        theme::corner_ticks(ui.painter(), r.expand(4.0), ACCENT);
                    }
                    ui.vertical(|ui| {
                        if theme::icon_button(ui, &mut self.icons, "image", t("Choisir une image…"), false).clicked() {
                            if let Some(f) = pick_file(t("Image du dock"), true) {
                                self.cfg.dock_image = f;
                            }
                        }
                        if !self.cfg.dock_image.is_empty() && theme::icon_button(ui, &mut self.icons, "upload", t("Envoyer l'image"), true).clicked() {
                            let path = self.cfg.dock_image.clone();
                            self.job(t("Image du dock"), move |d, prog| apply::dock_image(d, &path, &mut |x| prog(x)));
                        }
                    });
                });
                ui.add_space(8.0);
                let Some(edit) = self.dock_edit.as_mut() else {
                    ui.label(RichText::new(if loaded { "…" } else { t("Lecture des réglages du dock…") }).color(DIM));
                    return;
                };
                egui::Grid::new("saver").num_columns(2).spacing(Vec2::new(14.0, 10.0)).show(ui, |ui| {
                    ui.label(RichText::new(t("Veille")).color(DIM));
                    theme::toggle(ui, &mut edit.saver_on, t("Activer l'écran de veille"));
                    ui.end_row();
                    ui.label(RichText::new(t("Affiche")).color(DIM));
                    let shown = SAVER_TYPES.iter().find(|s| s.0 == edit.saver_type).map(|s| t(s.1)).unwrap_or("?");
                    egui::ComboBox::from_id_salt("saver-type").selected_text(shown).show_ui(ui, |ui| {
                        for (v, l) in SAVER_TYPES {
                            ui.selectable_value(&mut edit.saver_type, v, t(l));
                        }
                    });
                    ui.end_row();
                    ui.label(RichText::new(t("Après")).color(DIM));
                    ui.add(egui::Slider::new(&mut edit.saver_secs, 5..=1800).suffix(" s").logarithmic(true));
                    ui.end_row();
                    ui.label(RichText::new(t("Extinction")).color(DIM));
                    ui.horizontal(|ui| {
                        theme::toggle(ui, &mut edit.off_on, "");
                        ui.add_enabled(edit.off_on, egui::Slider::new(&mut edit.off_secs, 5..=1800).suffix(" s").logarithmic(true));
                    });
                    ui.end_row();
                });
            });
            let img = self.ic(ctx, "dial");
            theme::card(&mut cols[1], img, t("Menus de la molette"), |ui| {
                match self.dock_edit.as_mut() {
                    Some(edit) => {
                        egui::Grid::new("menus").num_columns(2).spacing(Vec2::new(18.0, 8.0)).show(ui, |ui| {
                            for (n, (bit, label)) in DOCK_MENUS.iter().enumerate() {
                                let mut on = edit.menus & (1 << bit) != 0;
                                if theme::toggle(ui, &mut on, t(label)).changed() {
                                    edit.menus ^= 1 << bit;
                                }
                                if n % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t("Couleur des menus")).color(DIM));
                            egui::color_picker::color_edit_button_srgb(ui, &mut edit.color);
                        });
                        ui.add_space(6.0);
                        let e = *edit;
                        let ext = self.ext.clone();
                        if theme::icon_button(ui, &mut self.icons, "send", t("Appliquer au dock"), true).clicked() {
                            self.job(t("Réglages du dock"), move |d, _| {
                                let info = apply::set_dock(d, e.menus, e.color, e.setup(), e.saver_secs, e.off_secs)?;
                                *ext.lock().unwrap() = Some(info);
                                Ok(())
                            });
                        }
                    }
                    None => {
                        ui.label(RichText::new(t("Lecture des réglages du dock…")).color(DIM));
                    }
                }
            });
            cols[1].add_space(8.0);
            let img = self.ic(ctx, "clock");
            theme::card(&mut cols[1], img, t("Données envoyées par le service"), |ui| {
                theme::toggle(ui, &mut self.cfg.sync_clock, t("Heure"));
                theme::toggle(ui, &mut self.cfg.sync_volume, t("Volume du système"));
                theme::toggle(ui, &mut self.cfg.sync_pcinfo, t("PC info : CPU, GPU, disque, internet, RAM, APM"));
                ui.add_space(6.0);
                ui.label(RichText::new(t("Internet (le dock affiche 0–100 %)")).color(DIM));
                let auto = stats::default_iface().unwrap_or_default();
                let speed_of = |n: &str| self.ifaces.iter().find(|i| i.0 == n).and_then(|i| i.1);
                let label = |n: &str| match speed_of(n) { Some(s) => tf!("{} — {} Mbit/s", n, s), None => n.to_string() };
                let shown = if self.cfg.net_iface.is_empty() { tf!("Automatique ({})", label(&auto)) } else { label(&self.cfg.net_iface) };
                egui::ComboBox::from_id_salt("iface").selected_text(shown).width(280.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.cfg.net_iface, String::new(), tf!("Automatique ({})", label(&auto)));
                    for (n, _) in &self.ifaces {
                        ui.selectable_value(&mut self.cfg.net_iface, n.clone(), label(n));
                    }
                });
                ui.horizontal(|ui| {
                    for (v, l) in [(0u8, t("Réception")), (1, t("Émission")), (2, t("Le plus haut"))] {
                        ui.selectable_value(&mut self.cfg.net_dir, v, l);
                    }
                });
                let link = speed_of(if self.cfg.net_iface.is_empty() { &auto } else { &self.cfg.net_iface });
                let mut manual = self.cfg.net_max_mbps > 0;
                ui.horizontal(|ui| {
                    if theme::toggle(ui, &mut manual, t("Plafond manuel")).changed() {
                        self.cfg.net_max_mbps = if manual { link.unwrap_or(1000) } else { 0 };
                    }
                    if manual {
                        ui.add(egui::DragValue::new(&mut self.cfg.net_max_mbps).range(1..=400_000).suffix(" Mbit/s").speed(10));
                    } else {
                        ui.label(RichText::new(tf!("100 % = vitesse du lien ({})", link.map(|s| format!("{s} Mbit/s")).unwrap_or("inconnue, 1000 Mbit/s".into()))).small().color(DIM));
                    }
                });
                ui.add_space(6.0);
                if theme::icon_button(ui, &mut self.icons, "clock", t("Mettre à l'heure maintenant"), false).clicked() {
                    self.job(t("Horloge"), |d, _| apply::clock_now(d));
                }
            });
        });
        if theme::icon_button(ui, &mut self.icons, "reboot", t("Relire les réglages du clavier"), false).clicked() {
            self.dock_edit = None;
            self.read_ext();
        }
    }

    fn refresh(&mut self) {
        self.checks = setup::run(&self.cfg);
        self.service_on = setup::service_active();
        self.connected = crate::device::find_hidraw().is_ok();
    }

    fn checklist(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, with_fix: bool) {
        let checks = self.checks.clone();
        for c in &checks {
            egui::Frame::default()
                .fill(Color32::from_rgba_premultiplied(2, 7, 13, 110))
                .stroke(Stroke::new(1.0_f32, if c.ok { LINE } else { WARN.gamma_multiply(0.5) }))
                .corner_radius(4).inner_margin(10)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.add(self.icons.image(ctx, c.icon, 20.0, if c.ok { ACCENT } else { WARN }));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if c.ok {
                                ui.add(self.icons.image(ctx, "check", 20.0, OK));
                            } else if with_fix && c.item != setup::Item::Device && !self.setup_running {
                                if theme::icon_button(ui, &mut self.icons, "bolt", t("Corriger"), false).clicked() {
                                    self.run_fix(Some(c.item));
                                }
                            } else {
                                ui.add(self.icons.image(ctx, "warn", 20.0, WARN));
                            }
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(c.title).strong());
                                    ui.label(RichText::new(&c.detail).small().color(DIM));
                                });
                            });
                        });
                    });
                });
        }
    }

    fn run_fix(&mut self, item: Option<setup::Item>) {
        self.setup_running = true;
        let mut cfg = self.cfg.clone();
        let out = self.setup_result.clone();
        std::thread::spawn(move || {
            let errors = match item {
                Some(i) => setup::fix(i, &mut cfg).err().into_iter().collect(),
                None => setup::fix_all(&mut cfg),
            };
            *out.lock().unwrap() = Some((cfg, errors));
        });
    }

    fn poll_fix(&mut self) {
        let result = self.setup_result.lock().unwrap().take();
        if let Some((cfg, errors)) = result {
            self.cfg.bindings = cfg.bindings;
            self.setup_running = false;
            self.refresh();
            if errors.is_empty() {
                self.say(t("Configuration terminée"), true);
            } else {
                self.say(errors.join(" · "), false);
            }
            if self.checks.iter().all(|c| c.ok || c.item == setup::Item::Device) {
                self.show_setup = false;
            }
            if std::mem::take(&mut self.update_pending) {
                self.update_info = setup::update_needed();
                self.show_setup = self.update_info.is_none() && !self.cfg.skip_setup && self.checks.iter().any(|c| !c.ok && c.item != setup::Item::Device);
            }
        }
    }

    fn page_system(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_header(ui, t("Système"), t("Installation, service d'arrière-plan et touches à action ordinateur."));
        ui.columns(2, |cols| {
            let img = self.ic(ctx, "shield");
            theme::card(&mut cols[0], img, t("État de l'installation"), |ui| {
                self.checklist(ui, ctx, true);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if self.setup_running {
                        ui.add(egui::Spinner::new().color(ACCENT));
                        ui.label(t("Configuration en cours…"));
                    } else if theme::icon_button(ui, &mut self.icons, "bolt", t("Tout corriger"), true).clicked() {
                        self.run_fix(None);
                    }
                    if theme::icon_button(ui, &mut self.icons, "reboot", t("Revérifier"), false).clicked() {
                        self.refresh();
                    }
                });
            });
            let img = self.ic(ctx, "keyboard");
            theme::card(&mut cols[1], img, t("Disposition du clavier"), |ui| {
                let cur = self.layout();
                let shown = keys::LAYOUTS.iter().find(|l| l.0 == cur).map_or(cur.as_str(), |l| l.1);
                egui::ComboBox::from_id_salt("layout").selected_text(shown).width(240.0).show_ui(ui, |ui| {
                    for (code, name) in keys::LAYOUTS {
                        ui.selectable_value(&mut self.cfg.layout, code.to_string(), name);
                    }
                });
                ui.label(RichText::new(t("Change seulement l'affichage des touches dans l'appli, pas le clavier lui-même.")).small().color(DIM));
            });
            cols[1].add_space(8.0);
            let img = self.ic(ctx, "settings");
            theme::card(&mut cols[1], img, t("Service"), |ui| {
                ui.horizontal(|ui| {
                    theme::status_dot(ui, self.service_on);
                    ui.label(if self.service_on { t("Actif — exécute les actions et synchronise le dock") } else { t("Arrêté") });
                });
                ui.horizontal(|ui| {
                    if theme::icon_button(ui, &mut self.icons, "play", t("Installer / redémarrer"), false).clicked() {
                        match setup::install_service() {
                            Ok(()) => self.say(t("Service démarré"), true),
                            Err(e) => self.say(tf!("Service : {}", e), false),
                        }
                        self.refresh();
                    }
                    if theme::icon_button(ui, &mut self.icons, "stop", t("Arrêter"), false).clicked() {
                        setup::stop_service();
                        self.refresh();
                    }
                });
                theme::toggle(ui, &mut self.cfg.dark_when_locked, t("Éteindre l'éclairage quand la session est verrouillée"));
                theme::toggle(ui, &mut self.cfg.openrgb, t("Serveur OpenRGB"));
                if self.cfg.openrgb {
                    ui.label(RichText::new(tf!("Dans OpenRGB : onglet Client, ajoute 127.0.0.1 port {}. Tant qu'OpenRGB est connecté, il pilote les couleurs du clavier.", crate::openrgb::PORT)).small().color(DIM));
                }
            });
            cols[1].add_space(8.0);
            let img = self.ic(ctx, "profile");
            theme::card(&mut cols[1], img, t("Profils du clavier"), |ui| {
                match self.profiles_enabled() {
                    Some(en) => {
                        ui.label(RichText::new(t("Les profils actifs défilent avec la touche profil du clavier.")).small().color(DIM));
                        ui.horizontal(|ui| {
                            for n in 1..=5u8 {
                                let mut on = en[n as usize - 1];
                                if theme::toggle(ui, &mut on, &n.to_string()).changed() {
                                    self.set_profile_enabled(n, on);
                                }
                            }
                        });
                    }
                    None => {
                        ui.label(RichText::new(t("Lecture du clavier…")).color(DIM));
                    }
                }
                ui.add_space(8.0);
                let cur = self.cfg.profile;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tf!("Nom du profil {}", cur)).color(DIM));
                    let snap = self.cfg.snapshot();
                    let entry = self.cfg.profiles.entry(cur).or_insert(snap);
                    ui.add(egui::TextEdit::singleline(&mut entry.name).hint_text(tf!("Profil {}", cur)).desired_width(160.0));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Copier vers")).color(DIM));
                    for n in (1..=5u8).filter(|&n| n != cur) {
                        if ui.button(n.to_string()).on_hover_text(t("Remplace l'éclairage, les touches et les touches écran de ce profil")).clicked() {
                            self.cfg.copy_profile_to(n);
                            self.say(tf!("Profil {} copié vers le profil {}", cur, n), true);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if theme::icon_button(ui, &mut self.icons, "upload", t("Exporter"), false).clicked() {
                        if let Some(f) = save_file(t("Exporter le profil"), &format!("everest-profil-{cur}.json")) {
                            let r = std::fs::write(&f, serde_json::to_string_pretty(&self.cfg.snapshot()).unwrap());
                            self.say(match r { Ok(()) => tf!("Profil exporté dans {}", f), Err(e) => e.to_string() }, true);
                        }
                    }
                    if theme::icon_button(ui, &mut self.icons, "folder", t("Importer"), false).clicked() {
                        if let Some(f) = pick_file(t("Importer un profil"), false) {
                            match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<crate::config::Profile>(&s).map_err(|e| e.to_string())) {
                                Ok(p) => {
                                    self.cfg.profiles.entry(cur).or_default().name = p.name.clone();
                                    self.cfg.restore(p);
                                    self.say(t("Profil importé — pense à l'envoyer au clavier"), true);
                                }
                                Err(e) => self.say(tf!("Fichier de profil invalide : {}", e), false),
                            }
                        }
                    }
                });
            });
            cols[1].add_space(8.0);
            let img = self.ic(ctx, "record");
            theme::card(&mut cols[1], img, t("OBS Studio"), |ui| {
                ui.label(RichText::new(t("Dans OBS : Outils → Paramètres du serveur WebSocket, active le serveur et copie le mot de passe ici.")).small().color(DIM));
                egui::Grid::new("obs").num_columns(2).spacing(Vec2::new(12.0, 6.0)).show(ui, |ui| {
                    ui.label(RichText::new(t("Port")).color(DIM));
                    ui.add(egui::DragValue::new(&mut self.cfg.obs_port).range(1..=65535));
                    ui.end_row();
                    ui.label(RichText::new(t("Mot de passe")).color(DIM));
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.obs_password).password(true).desired_width(200.0));
                    ui.end_row();
                });
                if theme::icon_button(ui, &mut self.icons, "check", t("Tester la connexion"), false).clicked() {
                    match crate::obs::scenes(self.cfg.obs_port, &self.cfg.obs_password) {
                        Ok(s) => self.say(tf!("OBS connecté — scènes : {}", s.join(", ")), true),
                        Err(e) => self.say(e, false),
                    }
                }
            });
            cols[1].add_space(8.0);
            let img = self.ic(ctx, "image");
            theme::card(&mut cols[1], img, t("Icônes PNG"), |ui| {
                ui.label(RichText::new(tf!("{} icônes 72×72 prêtes pour les touches écran.", icons::LIBRARY.len())).color(DIM));
                if theme::icon_button(ui, &mut self.icons, "folder", t("Exporter et ouvrir le dossier"), false).clicked() {
                    let dir = setup::icons_dir();
                    match images::export_library(&dir) {
                        Ok(n) => self.say(tf!("{} icônes dans {}", n, dir.display()), true),
                        Err(e) => self.say(e, false),
                    }
                    let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
                }
            });
            cols[1].add_space(8.0);
            let img = self.ic(ctx, "target");
            theme::card(&mut cols[1], img, t("Autres touches à action ordinateur"), |ui| {
                ui.label(RichText::new(t("Apprends une touche puis donne-lui une action. Désactive-la dans « Touches & macros » pour couper sa frappe.")).small().color(DIM));
                let learned = self.learning.as_ref().map(|l| l.lock().unwrap().take());
                match learned {
                    Some(Some(Ok((byte, bit)))) => {
                        if self.learning_fn {
                            self.cfg.fn_key = Some((byte, bit));
                        } else {
                            self.cfg.host_keys.push(HostKey { name: tf!("Touche {}.{}", byte, bit), byte, bit, action: Action::Aucune, fn_action: Action::Aucune });
                        }
                        self.learning = None;
                    }
                    Some(Some(Err(e))) => {
                        self.say(e, false);
                        self.learning = None;
                    }
                    Some(None) => {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().color(ACCENT));
                            ui.label(t("Appuie sur la touche…"));
                        });
                    }
                    None => {
                        let mut start = None;
                        ui.horizontal(|ui| {
                            if theme::icon_button(ui, &mut self.icons, "target", t("Apprendre une touche"), false).clicked() {
                                start = Some(false);
                            }
                            if theme::icon_button(ui, &mut self.icons, "layers", t("Choisir la touche Fn"), false).clicked() {
                                start = Some(true);
                            }
                        });
                        if let Some(is_fn) = start {
                            let slot = Arc::new(Mutex::new(None));
                            let (s2, c2) = (slot.clone(), ctx.clone());
                            std::thread::spawn(move || {
                                *s2.lock().unwrap() = Some(learn_key());
                                c2.request_repaint();
                            });
                            self.learning = Some(slot);
                            self.learning_fn = is_fn;
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("Touche Fn logicielle")).color(DIM));
                    match self.cfg.fn_key {
                        Some((b, n)) => {
                            ui.label(tf!("Touche {}.{}", b, n));
                            if close_button(ui, &mut self.icons, ctx) {
                                self.cfg.fn_key = None;
                            }
                        }
                        None => {
                            ui.label(RichText::new(t("aucune")).color(DIM));
                        }
                    }
                });
                ui.label(RichText::new(t("Maintenue, elle fait faire leur action « avec Fn » aux touches apprises.")).small().color(DIM));
                let mut remove = None;
                for (i, hk) in self.cfg.host_keys.iter_mut().enumerate() {
                    ui.push_id(("hk", i), |ui| {
                        egui::Frame::default().stroke(Stroke::new(1.0_f32, LINE)).corner_radius(4).inner_margin(8).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut hk.name).desired_width(160.0));
                                if close_button(ui, &mut self.icons, ctx) {
                                    remove = Some(i);
                                }
                            });
                            Self::action_editor(ui, &mut self.icons, ctx, &mut hk.action, &self.sinks, &[], 0);
                            if self.cfg.fn_key.is_some() {
                                ui.push_id("fn", |ui| {
                                    ui.label(RichText::new(t("AVEC FN")).font(theme::title_font(11.0)).color(DIM));
                                    Self::action_editor(ui, &mut self.icons, ctx, &mut hk.fn_action, &self.sinks, &[], 0);
                                });
                            }
                        });
                    });
                }
                if let Some(i) = remove {
                    self.cfg.host_keys.remove(i);
                }
            });
        });
    }

    fn update_modal(&mut self, ctx: &egui::Context) {
        let Some(running) = self.update_info.clone() else { return };
        egui::Area::new(egui::Id::unique("dim-update")).order(egui::Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            let r = ctx.content_rect();
            ui.allocate_rect(r, Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(170));
        });
        egui::Window::new(RichText::new(t("MISE À JOUR DU PROGRAMME")).font(theme::title_font(16.0)).color(ACCENT))
            .order(egui::Order::Foreground).collapsible(false).resizable(false).fixed_size([560.0, 0.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(self.icons.image(ctx, "download", 28.0, ACCENT));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(tf!("L'application est en version {}.", env!("CARGO_PKG_VERSION"))).strong());
                        ui.label(RichText::new(tf!("Version installée : {}.", running)).color(DIM));
                    });
                });
                ui.add_space(6.0);
                ui.label(t("Veux-tu remplacer le programme installé dans ~/.local/bin (celui du menu des applications) par cette version ? Le service sera redémarré s'il tourne. Quelques secondes, sans mot de passe."));
                ui.add_space(10.0);
                if self.setup_running {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().color(ACCENT));
                        ui.label(t("Mise à jour en cours…"));
                    });
                    return;
                }
                if !self.update_confirm {
                    ui.horizontal(|ui| {
                        if theme::icon_button(ui, &mut self.icons, "download", t("Remplacer et mettre à jour"), true).clicked() {
                            self.update_pending = true;
                            self.run_fix(Some(setup::Item::Update));
                        }
                        if ui.button(t("Ne pas mettre à jour")).clicked() {
                            self.update_confirm = true;
                        }
                    });
                } else {
                    egui::Frame::default().fill(Color32::from_rgba_premultiplied(60, 12, 12, 140)).stroke(Stroke::new(1.0_f32, BAD)).corner_radius(4).inner_margin(10).show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.add(self.icons.image(ctx, "warn", 22.0, BAD));
                            ui.add(egui::Label::new(RichText::new(t("Sans mise à jour, certaines fonctions de cette application peuvent ne pas marcher (effets animés, dock, touches écran), et l'ancien service peut écrire des réglages incompatibles et corrompre ta configuration.")).color(BAD)).wrap());
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if theme::danger_button(ui, &mut self.icons, "warn", t("Continuer sans mettre à jour")).clicked() {
                            self.update_info = None;
                            self.update_confirm = false;
                            self.show_setup = !self.cfg.skip_setup
                                && self.checks.iter().any(|c| !c.ok && !matches!(c.item, setup::Item::Device | setup::Item::Binary | setup::Item::Service));
                            self.say(t("Service non mis à jour"), false);
                        }
                        if theme::icon_button(ui, &mut self.icons, "download", t("Finalement, mettre à jour"), true).clicked() {
                            self.update_confirm = false;
                            self.update_pending = true;
                            self.run_fix(Some(setup::Item::Update));
                        }
                    });
                }
            });
    }

    fn translate_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.show_translate;
        egui::Window::new(RichText::new(t("AUTRES LANGUES")).font(theme::title_font(14.0)).color(ACCENT))
            .open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(self.icons.image(ctx, "globe", 28.0, ACCENT));
                    ui.label(RichText::new(t("Aidez-moi à traduire ce logiciel dans d'autres langues ! Rendez-vous sur GitHub !")).strong());
                });
                ui.add_space(6.0);
                if theme::icon_button(ui, &mut self.icons, "link", t("Ouvrir GitHub"), true).clicked() {
                    let _ = std::process::Command::new("xdg-open").arg("https://github.com/AnnoyedDev/everest-max-linux").spawn();
                }
            });
        self.show_translate = open;
    }

    fn setup_modal(&mut self, ctx: &egui::Context) {
        if !self.show_setup {
            return;
        }
        egui::Area::new(egui::Id::unique("dim")).order(egui::Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            let r = ctx.content_rect();
            ui.allocate_rect(r, Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(170));
        });
        egui::Window::new(RichText::new(t("CONFIGURATION INITIALE")).font(theme::title_font(16.0)).color(ACCENT))
            .order(egui::Order::Foreground).collapsible(false).resizable(false).fixed_size([580.0, 0.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(t("Quelques éléments manquent pour que tout fonctionne. Je peux m'en occuper ; le mot de passe administrateur est demandé une seule fois, pour la règle udev."));
                ui.add_space(8.0);
                self.checklist(ui, ctx, false);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if self.setup_running {
                        ui.add(egui::Spinner::new().color(ACCENT));
                        ui.label(t("Configuration en cours…"));
                    } else {
                        if theme::icon_button(ui, &mut self.icons, "bolt", t("Tout configurer"), true).clicked() {
                            self.run_fix(None);
                        }
                        if ui.button(t("Plus tard")).clicked() {
                            self.show_setup = false;
                        }
                        theme::toggle(ui, &mut self.cfg.skip_setup, t("Ne plus demander"));
                    }
                });
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        theme::paint_grid(ctx);
        if self.last_poll.elapsed() > Duration::from_secs(2) {
            self.last_poll = Instant::now();
            self.connected = crate::device::find_hidraw().is_ok();
            self.service_on = setup::service_active();
        }
        ctx.request_repaint_after(Duration::from_secs(2));
        self.poll_fix();
        self.sidebar(root, ctx);
        self.statusbar(root, ctx);
        egui::CentralPanel::default().frame(egui::Frame::default().inner_margin(egui::Margin::symmetric(26, 20))).show(root, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.page {
                Page::Eclairage => self.page_lighting(ui, ctx),
                Page::Touches => self.page_keys(ui, ctx),
                Page::Ecrans => self.page_screens(ui, ctx),
                Page::Dock => self.page_dock(ui, ctx),
                Page::Pads => self.page_pads(ui, ctx),
                Page::Systeme => self.page_system(ui, ctx),
            });
        });
        self.gallery_window(ctx);
        self.setup_modal(ctx);
        self.update_modal(ctx);
        self.translate_modal(ctx);
        if self.cfg != self.saved && !ctx.input(|i| i.pointer.any_down()) {
            if let Err(e) = self.cfg.save() {
                self.say(tf!("Sauvegarde : {}", e), false);
            }
            self.saved = self.cfg.clone();
        }
    }
}

pub fn run() -> eframe::Result {
    let icon = icons::rasterize_svg(icons::APP_ICON, 128, 128).map(|pm| {
        let rgba = pm.pixels().iter().flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        }).collect();
        egui::IconData { rgba, width: 128, height: 128 }
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1320.0, 860.0])
        .with_min_inner_size([1080.0, 700.0])
        .with_title("Everest Base Camp")
        .with_app_id("everest-basecamp");
    if let Some(i) = icon {
        viewport = viewport.with_icon(Arc::new(i));
    }
    eframe::run_native("Everest Base Camp", eframe::NativeOptions { viewport, ..Default::default() }, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
