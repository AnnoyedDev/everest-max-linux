use crate::icons;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{imageops::FilterType, Rgb, RgbImage};
use crate::i18n::t;

pub fn to_rgb565(img: &RgbImage) -> Vec<u8> {
    img.pixels()
        .flat_map(|Rgb([r, g, b])| ((*r as u16 >> 3) << 11 | (*g as u16 >> 2) << 5 | (*b as u16 >> 3)).to_le_bytes())
        .collect()
}

pub fn system_font(text: &str) -> Option<FontVec> {
    if text.chars().any(|c| ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ff00}'..='\u{ffef}').contains(&c)) {
        let (data, index) = crate::theme::cjk_font(true)?;
        return FontVec::try_from_vec_and_index(data, index).ok();
    }
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans:bold"]).output().ok()?;
    FontVec::try_from_vec(std::fs::read(String::from_utf8_lossy(&out.stdout).trim()).ok()?).ok()
}

pub struct Art<'a> {
    pub image: &'a str,
    pub icon: &'a str,
    pub text: &'a str,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub grid: bool,
}

fn blend(p: &mut Rgb<u8>, c: [u8; 3], a: f32) {
    for i in 0..3 {
        p.0[i] = (p.0[i] as f32 * (1.0 - a) + c[i] as f32 * a).round() as u8;
    }
}

fn grid(img: &mut RgbImage) {
    let (w, h) = img.dimensions();
    let line = [0x7F, 0xC4, 0xFF];
    for y in 0..h {
        for x in 0..w {
            let major = x % 24 == 0 || y % 24 == 0;
            let minor = x % 8 == 0 || y % 8 == 0;
            let border = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            let a = if border { 0.45 } else if major { 0.2 } else if minor { 0.08 } else { 0.0 };
            if a > 0.0 {
                blend(img.get_pixel_mut(x, y), line, a);
            }
        }
    }
}

fn draw_icon(img: &mut RgbImage, name: &str, size: u32, x0: i32, y0: i32, color: [u8; 3]) {
    let Some(pm) = icons::rasterize(name, size, color) else { return };
    for (i, px) in pm.data().chunks(4).enumerate() {
        let a = px[3] as f32 / 255.0;
        if a == 0.0 {
            continue;
        }
        let (x, y) = (x0 + (i as u32 % size) as i32, y0 + (i as u32 / size) as i32);
        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
            let p = img.get_pixel_mut(x as u32, y as u32);
            for c in 0..3 {
                p.0[c] = (px[c] as f32 + p.0[c] as f32 * (1.0 - a)).min(255.0) as u8;
            }
        }
    }
}

pub fn render(w: u32, h: u32, art: &Art) -> Result<RgbImage, String> {
    let mut img = if art.image.is_empty() {
        let mut img = RgbImage::from_pixel(w, h, Rgb(art.bg));
        if art.grid {
            grid(&mut img);
        }
        img
    } else {
        image::open(art.image).map_err(|e| format!("{}: {e}", art.image))?.resize_to_fill(w, h, FilterType::Lanczos3).to_rgb8()
    };
    let text = art.text.trim();
    let has_icon = !art.icon.is_empty();
    if has_icon {
        let size = if text.is_empty() { h * 62 / 100 } else { h * 48 / 100 };
        let y = if text.is_empty() { (h - size) / 2 } else { h * 9 / 100 };
        draw_icon(&mut img, art.icon, size, ((w - size) / 2) as i32, y as i32, art.fg);
    }
    if !text.is_empty() {
        let font = system_font(text).ok_or(t("aucune police trouvée (fc-match)"))?;
        let band = if has_icon { (h as f32 * 0.62, h as f32 * 0.98) } else { (0.0, h as f32) };
        draw_text(&mut img, &font, text, art.fg, band);
    }
    Ok(img)
}

fn draw_text(img: &mut RgbImage, font: &FontVec, text: &str, fg: [u8; 3], (top, bottom): (f32, f32)) {
    let w = img.width() as f32;
    let lines: Vec<&str> = text.lines().collect();
    let mut size = (bottom - top) / lines.len() as f32 * 0.85;
    let width = |s: f32, l: &str| {
        let f = font.as_scaled(PxScale::from(s));
        l.chars().map(|c| f.h_advance(f.glyph_id(c))).sum::<f32>()
    };
    while size > 6.0 && lines.iter().any(|l| width(size, l) > w * 0.9) {
        size -= 1.0;
    }
    let f = font.as_scaled(PxScale::from(size));
    let line_h = f.ascent() - f.descent();
    let mut y = top + ((bottom - top) - line_h * lines.len() as f32) / 2.0 + f.ascent();
    for line in lines {
        let mut x = (w - width(size, line)) / 2.0;
        for c in line.chars() {
            let g = f.glyph_id(c).with_scale_and_position(size, ab_glyph::point(x, y));
            x += f.h_advance(f.glyph_id(c));
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| {
                    let (px, py) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                        blend(img.get_pixel_mut(px as u32, py as u32), fg, cov);
                    }
                });
            }
        }
        y += line_h;
    }
}

pub const BLUEPRINT: [u8; 3] = [0x0B, 0x2A, 0x4C];

pub fn export_library(dir: &std::path::Path) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (name, _) in icons::LIBRARY {
        let art = Art { image: "", icon: name, text: "", fg: [255; 3], bg: BLUEPRINT, grid: true };
        render(72, 72, &art)?.save(dir.join(format!("{name}.png"))).map_err(|e| e.to_string())?;
    }
    Ok(icons::LIBRARY.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_packing() {
        let img = RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 255, 255]).unwrap();
        assert_eq!(to_rgb565(&img), vec![0x00, 0xF8, 0xFF, 0x07]);
    }

    #[test]
    fn icon_art_has_foreground() {
        let img = render(72, 72, &Art { image: "", icon: "lock", text: "", fg: [255; 3], bg: [0; 3], grid: false }).unwrap();
        assert!(img.pixels().filter(|p| p.0 == [255, 255, 255]).count() > 50);
    }
}
