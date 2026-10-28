//! Map-only presentation: parser-inspired surfaces, semantic marker colors,
//! and an openly licensed, supersampled font. No gameplay or position state.
use vi::ui::{Raster, wide};
pub const TEXT: [f32; 4] = [232.0 / 255.0, 232.0 / 255.0, 232.0 / 255.0, 1.0];
pub const MUTED: [f32; 4] = [0.6, 0.6, 0.6, 1.0];
pub const SURFACE: [f32; 4] = [17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0, 1.0];
pub const INPUT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
pub const RAISED: [f32; 4] = [26.0 / 255.0, 26.0 / 255.0, 26.0 / 255.0, 1.0];
pub const BORDER: [f32; 4] = [0.2, 0.2, 0.2, 1.0];
pub const ACCENT: [f32; 4] = [0.843, 0.098, 0.129, 1.0];
pub const SELECTED: [f32; 4] = [0.17, 0.067, 0.08, 1.0];
pub const KNOWN: [f32; 4] = [0.93, 0.72, 0.25, 0.9];
pub const LIVE: [f32; 4] = [0.25, 1.0, 0.65, 1.0];
pub const GROUP: [f32; 4] = [0.3, 0.75, 1.0, 1.0];
pub const FONT_SCALE: f32 = 1.0 / 3.0;

pub fn fill(r: &mut dyn Raster, b: [i32; 4], c: [f32; 4]) {
    r.fill_fan(c, [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]]);
}
pub fn frame(r: &mut dyn Raster, b: [i32; 4], c: [f32; 4]) {
    for e in [
        [b[0], b[1], b[2], b[1] + 1],
        [b[0], b[3] - 1, b[2], b[3]],
        [b[0], b[1], b[0] + 1, b[3]],
        [b[2] - 1, b[1], b[2], b[3]],
    ] {
        fill(r, e, c);
    }
}
pub fn panel(r: &mut dyn Raster, b: [i32; 4]) {
    fill(r, b, SURFACE);
    frame(r, b, BORDER);
}
pub fn control(r: &mut dyn Raster, b: [i32; 4], active: bool) {
    fill(r, b, if active { SELECTED } else { RAISED });
    frame(r, b, if active { ACCENT } else { BORDER });
}
pub fn text(r: &mut dyn Raster, s: &str, at: [i32; 2], width: i32, c: [f32; 4]) {
    let mut s = s.to_string();
    while !s.is_empty() && r.string_width(&wide(&s)) > width {
        s.pop();
    }
    r.draw_string(at, c, &wide(&s));
}

/// Vector search symbol: no missing-font glyph or quest-question-mark ambiguity.
pub fn magnifier(r: &mut dyn Raster, b: [i32; 4], color: [f32; 4]) {
    let (x, y) = (b[0] + 8, b[1] + 8);
    for i in 0..24 {
        let a = i as f32 * std::f32::consts::TAU / 24.0;
        let z = (i + 1) as f32 * std::f32::consts::TAU / 24.0;
        let point = |t: f32, rad: f32| {
            [
                x + (t.cos() * rad).round() as i32,
                y + (t.sin() * rad).round() as i32,
            ]
        };
        r.fill_fan(
            color,
            [point(a, 5.0), point(a, 4.0), point(z, 4.0), point(z, 5.0)],
        );
    }
    r.fill_fan(
        color,
        [
            [x + 3, y + 5],
            [x + 5, y + 3],
            [x + 10, y + 8],
            [x + 8, y + 10],
        ],
    );
}

pub fn font() -> Option<esf::font::Font> {
    use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
    let bundled = include_bytes!("../../resources/map-ui/SpaceMono-Regular.ttf");
    #[cfg(not(target_arch = "wasm32"))]
    let custom = std::env::var_os("EQOA_MAP_FONT")
        .and_then(|p| std::fs::read(p).ok())
        .filter(|b| b.len() < 16 * 1024 * 1024);
    #[cfg(target_arch = "wasm32")]
    let custom: Option<Vec<u8>> = None;
    let face = custom
        .and_then(|b| FontArc::try_from_vec(b).ok())
        .or_else(|| FontArc::try_from_slice(bundled).ok())?;
    let scale = PxScale::from(33.0);
    let scaled = face.as_scaled(scale);
    let height = 36;
    let baseline = scaled.ascent().ceil();
    let mut glyphs = Vec::new();
    for ch in (32..=255)
        .chain([0x2013, 0x2014, 0x2022, 0x2190, 0x2191, 0x2192, 0x2193])
        .filter_map(char::from_u32)
    {
        let id = face.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, point(0.0, baseline));
        let width = scaled.h_advance(id).ceil().clamp(1.0, 250.0) as u8;
        let mut rgba = vec![0; usize::from(width) * height as usize * 4];
        if let Some(outline) = face.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|x, y, coverage| {
                let x = x as i32 + bounds.min.x as i32;
                let y = y as i32 + bounds.min.y as i32;
                if x >= 0 && y >= 0 && x < i32::from(width) && y < height as i32 {
                    let at = (y as usize * usize::from(width) + x as usize) * 4;
                    rgba[at..at + 4].copy_from_slice(&[
                        255,
                        255,
                        255,
                        (coverage * 255.0).round() as u8,
                    ]);
                }
            });
        }
        glyphs.push(esf::font::Glyph {
            code: ch as u16,
            width,
            rgba,
        });
    }
    Some(esf::font::Font {
        id: vi::ui::font_id() as i32,
        height,
        glyphs,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_font_is_readable_and_has_no_missing_ascii_glyphs() {
        let f = super::font().unwrap();
        assert_eq!(f.height, 36);
        for c in 32..=126 {
            assert!(f.glyph(c).is_some());
        }
        assert!(
            f.glyph(b'W' as u16)
                .unwrap()
                .rgba
                .iter()
                .skip(3)
                .step_by(4)
                .any(|a| *a > 0)
        );
        assert!(!f.glyph(b' ' as u16).unwrap().rgba.iter().any(|a| *a > 0));
    }
}
