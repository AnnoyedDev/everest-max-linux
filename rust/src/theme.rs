use crate::icons;
use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, TextStyle, TextureHandle, Vec2};
use std::collections::HashMap;
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x08, 0x21, 0x3D);
pub const PANEL: Color32 = Color32::from_rgba_premultiplied(9, 38, 70, 230);
pub const CARD: Color32 = Color32::from_rgba_premultiplied(9, 39, 72, 217);
pub const INK: Color32 = Color32::from_rgb(0xDC, 0xEC, 0xFF);
pub const DIM: Color32 = Color32::from_rgb(0x86, 0xA9, 0xD1);
pub const ACCENT: Color32 = Color32::from_rgb(0x5F, 0xB4, 0xFF);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(29, 54, 77, 77);
pub const LINE: Color32 = Color32::from_rgba_premultiplied(48, 90, 128, 128);
pub const OK: Color32 = Color32::from_rgb(0x5B, 0xE7, 0xA9);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xB4, 0x54);
pub const BAD: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);

pub fn title_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("title".into()))
}

fn load_font(pattern: &str) -> Option<Vec<u8>> {
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", pattern]).output().ok()?;
    std::fs::read(String::from_utf8_lossy(&out.stdout).trim()).ok()
}

pub fn cjk_font(bold: bool) -> Option<(Vec<u8>, u32)> {
    let pattern = if bold { "sans:bold:lang=ja" } else { "sans:lang=ja" };
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}|%{index}", pattern]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    let (file, index) = s.split_once('|')?;
    Some((std::fs::read(file.trim()).ok()?, index.trim().parse().unwrap_or(0)))
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut title = vec![];
    if let Some(b) = load_font("Fira Sans:medium").or_else(|| load_font("sans")) {
        fonts.font_data.insert("body".into(), Arc::new(egui::FontData::from_owned(b)));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "body".into());
    }
    if let Some(b) = load_font("monospace:bold") {
        fonts.font_data.insert("mono".into(), Arc::new(egui::FontData::from_owned(b)));
        fonts.families.get_mut(&FontFamily::Monospace).unwrap().insert(0, "mono".into());
        title.push("mono".to_string());
    }
    if let Some((b, index)) = cjk_font(false) {
        let mut data = egui::FontData::from_owned(b);
        data.index = index;
        fonts.font_data.insert("cjk".into(), Arc::new(data));
        for fam in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.get_mut(&fam).unwrap().push("cjk".into());
        }
    }
    title.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
    fonts.families.insert(FontFamily::Name("title".into()), title);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    style.text_styles = [
        (TextStyle::Heading, title_font(22.0)),
        (TextStyle::Body, FontId::proportional(14.5)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ].into();
    style.spacing.item_spacing = Vec2::new(10.0, 9.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.slider_width = 260.0;
    style.spacing.combo_width = 220.0;
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.override_text_color = Some(INK);
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgb(0x0B, 0x2B, 0x50);
    v.window_stroke = Stroke::new(1.0_f32, ACCENT);
    v.window_corner_radius = CornerRadius::same(6);
    v.extreme_bg_color = Color32::from_rgb(0x06, 0x1A, 0x31);
    v.faint_bg_color = Color32::from_rgb(0x0C, 0x30, 0x58);
    v.selection.bg_fill = Color32::from_rgba_premultiplied(26, 53, 80, 160);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.hyperlink_color = ACCENT;
    v.slider_trailing_fill = true;
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, Color32::TRANSPARENT, LINE),
        (&mut v.widgets.inactive, Color32::from_rgba_premultiplied(12, 39, 71, 192), LINE),
        (&mut v.widgets.hovered, Color32::from_rgba_premultiplied(21, 61, 105, 224), ACCENT),
        (&mut v.widgets.active, Color32::from_rgba_premultiplied(32, 85, 141, 240), ACCENT),
        (&mut v.widgets.open, Color32::from_rgba_premultiplied(21, 61, 105, 224), ACCENT),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0_f32, stroke);
        w.corner_radius = CornerRadius::same(4);
        w.fg_stroke = Stroke::new(1.5_f32, INK);
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, INK);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    ctx.set_global_style(style);
}

pub fn paint_grid(ctx: &egui::Context) {
    let rect = ctx.content_rect();
    let p = ctx.layer_painter(egui::LayerId::background());
    p.rect_filled(rect, 0.0, BG);
    let minor = Color32::from_rgba_premultiplied(15, 29, 41, 41);
    let major = Color32::from_rgba_premultiplied(31, 59, 84, 84);
    let mut x = rect.left();
    let mut i = 0;
    while x < rect.right() {
        p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(1.0_f32, if i % 5 == 0 { major } else { minor }));
        x += 24.0;
        i += 1;
    }
    let mut y = rect.top();
    i = 0;
    while y < rect.bottom() {
        p.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(1.0_f32, if i % 5 == 0 { major } else { minor }));
        y += 24.0;
        i += 1;
    }
}

pub fn corner_ticks(p: &egui::Painter, r: Rect, color: Color32) {
    let l = 10.0;
    let s = Stroke::new(2.0_f32, color);
    for (c, dx, dy) in [(r.left_top(), 1.0, 1.0), (r.right_top(), -1.0, 1.0), (r.left_bottom(), 1.0, -1.0), (r.right_bottom(), -1.0, -1.0)] {
        p.line_segment([c, c + Vec2::new(l * dx, 0.0)], s);
        p.line_segment([c, c + Vec2::new(0.0, l * dy)], s);
    }
}

#[derive(Default)]
pub struct Icons {
    cache: HashMap<(String, u32), TextureHandle>,
}

impl Icons {
    pub fn tex(&mut self, ctx: &egui::Context, name: &str, px: u32) -> egui::TextureId {
        let px = (px as f32 * ctx.pixels_per_point()).round() as u32;
        self.cache.entry((name.to_string(), px)).or_insert_with(|| {
            let img = icons::rasterize(name, px, [255, 255, 255])
                .map(|pm| egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data()))
                .unwrap_or_else(|| egui::ColorImage::filled([1, 1], Color32::TRANSPARENT));
            ctx.load_texture(format!("icon-{name}-{px}"), img, egui::TextureOptions::LINEAR)
        }).id()
    }

    pub fn image(&mut self, ctx: &egui::Context, name: &str, size: f32, tint: Color32) -> egui::Image<'static> {
        egui::Image::new((self.tex(ctx, name, size as u32), Vec2::splat(size))).tint(tint)
    }
}

pub fn card<R>(ui: &mut egui::Ui, img: egui::Image<'static>, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let out = egui::Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(img);
                ui.label(egui::RichText::new(title.to_uppercase()).font(title_font(13.0)).color(ACCENT));
            });
            let r = ui.available_rect_before_wrap();
            ui.painter().line_segment([Pos2::new(r.left(), r.top()), Pos2::new(r.right(), r.top())], Stroke::new(1.0_f32, LINE));
            ui.add_space(6.0);
            add(ui)
        });
    corner_ticks(ui.painter(), out.response.rect, ACCENT);
    out.inner
}

pub fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(egui::RichText::new(title.to_uppercase()).font(title_font(24.0)).color(INK));
    ui.label(egui::RichText::new(subtitle).color(DIM));
    ui.add_space(8.0);
}

pub fn nav_button(ui: &mut egui::Ui, icons: &mut Icons, icon: &str, label: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 4.0, ACCENT_SOFT);
        p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())), 1.0, ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, Color32::from_rgba_premultiplied(11, 22, 31, 31));
    }
    let col = if selected { ACCENT } else if resp.hovered() { INK } else { DIM };
    let tex = icons.tex(ui.ctx(), icon, 20);
    let ir = Rect::from_center_size(Pos2::new(rect.left() + 26.0, rect.center().y), Vec2::splat(20.0));
    p.image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), col);
    p.text(Pos2::new(rect.left() + 48.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(15.0), if selected { INK } else { col });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn tile(ui: &mut egui::Ui, icons: &mut Icons, icon: &str, label: &str, selected: bool, size: Vec2) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if selected { ACCENT_SOFT } else if resp.hovered() { Color32::from_rgba_premultiplied(17, 32, 46, 46) } else { Color32::from_rgba_premultiplied(4, 14, 26, 128) };
    p.rect_filled(rect, 4.0, fill);
    p.rect_stroke(rect, 4.0, Stroke::new(if selected { 1.5_f32 } else { 1.0 }, if selected { ACCENT } else { LINE }), egui::StrokeKind::Inside);
    if selected {
        corner_ticks(p, rect.shrink(1.0), ACCENT);
    }
    let col = if selected { ACCENT } else { INK };
    let s = (size.y * 0.42).min(30.0);
    let ir = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + size.y * 0.4), Vec2::splat(s));
    p.image(icons.tex(ui.ctx(), icon, s as u32), ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), col);
    p.text(Pos2::new(rect.center().x, rect.bottom() - 13.0), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.5), col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn icon_button(ui: &mut egui::Ui, icons: &mut Icons, icon: &str, label: &str, primary: bool) -> egui::Response {
    styled_button(ui, icons, icon, label, primary, false)
}

pub fn danger_button(ui: &mut egui::Ui, icons: &mut Icons, icon: &str, label: &str) -> egui::Response {
    styled_button(ui, icons, icon, label, true, true)
}

fn styled_button(ui: &mut egui::Ui, icons: &mut Icons, icon: &str, label: &str, primary: bool, danger: bool) -> egui::Response {
    let ink = if primary { Color32::from_rgb(0x04, 0x18, 0x2E) } else { INK };
    let font = FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, ink);
    let size = Vec2::new(galley.size().x + 52.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let down = resp.is_pointer_button_down_on();
    let c = 8.0;
    let r = rect.translate(Vec2::new(0.0, if down { 1.0 } else { 0.0 }));
    let pts = vec![
        Pos2::new(r.left() + c, r.top()),
        Pos2::new(r.right(), r.top()),
        Pos2::new(r.right(), r.bottom() - c),
        Pos2::new(r.right() - c, r.bottom()),
        Pos2::new(r.left(), r.bottom()),
        Pos2::new(r.left(), r.top() + c),
    ];
    let p = ui.painter();
    let (fill, stroke) = if danger {
        (if hovered { Color32::from_rgb(0xFF, 0x8E, 0x8E) } else { BAD }, Stroke::new(1.0_f32, Color32::from_rgb(0xFF, 0xC4, 0xC4)))
    } else if primary {
        (if hovered { Color32::from_rgb(0x8C, 0xCB, 0xFF) } else { ACCENT }, Stroke::new(1.0_f32, Color32::from_rgb(0xB8, 0xE0, 0xFF)))
    } else {
        (if hovered { ACCENT_SOFT } else { Color32::from_rgba_premultiplied(3, 12, 22, 110) }, Stroke::new(1.0_f32, if hovered { ACCENT } else { LINE }))
    };
    p.add(egui::Shape::convex_polygon(pts, fill, stroke));
    let notch = Stroke::new(2.0_f32, if primary { Color32::from_rgb(0x04, 0x18, 0x2E).gamma_multiply(0.55) } else { ACCENT });
    p.line_segment([Pos2::new(r.left() + 2.0, r.top() + c + 1.0), Pos2::new(r.left() + c + 1.0, r.top() + 2.0)], notch);
    let hatch = if primary { Color32::from_rgba_premultiplied(2, 10, 18, 70) } else { LINE };
    for k in 0..3 {
        let x = r.right() - 7.0 - k as f32 * 4.0;
        p.line_segment([Pos2::new(x, r.bottom() - 3.0), Pos2::new(x + 3.0, r.bottom() - 6.0)], Stroke::new(1.0_f32, hatch));
    }
    if resp.has_focus() {
        p.rect_stroke(rect.expand(3.0), 2.0, Stroke::new(1.5_f32, INK), egui::StrokeKind::Outside);
    }
    let tex = icons.tex(ui.ctx(), icon, 16);
    let ir = Rect::from_center_size(Pos2::new(r.left() + 22.0, r.center().y), Vec2::splat(16.0));
    p.image(tex, ir, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), if primary { ink } else { ACCENT });
    p.galley(Pos2::new(r.left() + 36.0, r.center().y - galley.size().y / 2.0), galley, ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn chip(ui: &mut egui::Ui, color: Color32, label: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, color);
        ui.label(egui::RichText::new(label).small().color(DIM));
    });
}

pub fn status_dot(ui: &mut egui::Ui, ok: bool) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    let c = if ok { OK } else { BAD };
    ui.painter().circle_filled(r.center(), 4.5, c);
    ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.0_f32, c.gamma_multiply(0.4)));
}

pub fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let resp = ui.horizontal(|ui| {
        let (r, mut resp) = ui.allocate_exact_size(Vec2::new(38.0, 20.0), Sense::click());
        if resp.clicked() {
            *on = !*on;
            resp.mark_changed();
        }
        let t = ui.ctx().animate_bool(resp.id, *on);
        let p = ui.painter();
        p.rect_filled(r, 10.0, Color32::from_rgb(0x06, 0x1A, 0x31).lerp_to_gamma(ACCENT.gamma_multiply(0.55), t));
        p.rect_stroke(r, 10.0, Stroke::new(1.0_f32, if *on { ACCENT } else { LINE }), egui::StrokeKind::Inside);
        p.circle_filled(Pos2::new(egui::lerp(r.left() + 10.0..=r.right() - 10.0, t), r.center().y), 7.0, if *on { INK } else { DIM });
        ui.label(label);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    });
    resp.inner
}
