//! The owner's program icon: build.rs chooses one of two .ico files
//! (`app_icon_ico::choose`: STANDARD when `EQOA_UPDATE_CHANNEL` is set, DEV
//! otherwise) and copies it into OUT_DIR. This module decodes the image
//! nearest each size `icon` asks for, so the window, the taskbar and the
//! .exe carry the same picture for the whole run.

#[path = "app_icon_ico.rs"]
mod app_icon_ico;

/// The chosen icon, copied into OUT_DIR by build.rs.
pub const ICO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app-icon.ico"));

/// `size` square RGBA from `ico`, and the image's own size. The nearest image
/// wins; a tie takes the larger one, so callers downscale rather than upscale.
pub fn rgba_from(ico: &[u8], size: u32) -> Option<(u32, Vec<u8>)> {
    let images = app_icon_ico::parse_ico(ico).ok()?;
    let image = images.iter().min_by_key(|image| (image.size.abs_diff(size), std::cmp::Reverse(image.size)))?;
    let (width, height, rgba) = decode(&image.bytes)?;
    (width > 0 && width == height).then_some((width, rgba))
}

/// The chosen icon's image nearest `size`, and that image's own size.
pub fn rgba(size: u32) -> Option<(u32, Vec<u8>)> {
    rgba_from(ICO, size)
}

/// `size` square RGBA from the chosen icon.
pub fn picture(size: u32) -> Option<Vec<u8>> {
    let (from, rgba) = rgba(size)?;
    Some(crate::icon::shrink(&rgba, from, size))
}

/// The fork keeps the chosen icon, so `icon::poll`'s save-icon switch never
/// runs. It stays a call so the upstream body is still compiled and checked.
pub fn keeps_icon() -> bool {
    true
}

/// A PNG as RGBA8, if it is a non-empty square.
fn decode(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    (info.width > 0 && info.width == info.height && info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight).then_some((info.width, info.height, buf))
}

#[cfg(test)]
mod tests {
    use super::app_icon_ico::{choose, parse_ico, Set};
    use super::*;

    const DEV: &[u8] = include_bytes!("../../resources/app-icon/eqoa-dev.ico");
    const STANDARD: &[u8] = include_bytes!("../../resources/app-icon/eqoa-standard.ico");
    const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

    /// A minimal .ico directory around the given entries, for malformed cases.
    fn ico_with(entries: &[(u8, u8, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for v in [0u16, 1, entries.len() as u16] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let mut offset = 6 + 16 * entries.len();
        for (width, height, data) in entries {
            out.extend_from_slice(&[*width, *height, 0, 0]);
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&32u16.to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += data.len();
        }
        for (_, _, data) in entries {
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn both_icon_files_hold_seven_png_images_from_16_to_256() {
        for (name, bytes) in [("dev", DEV), ("standard", STANDARD)] {
            let images = parse_ico(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            let sizes: Vec<u32> = images.iter().map(|image| image.size).collect();
            assert_eq!(sizes, SIZES, "{name}");
            for image in &images {
                assert!(image.bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{name}: {} is not PNG", image.size);
            }
        }
    }

    #[test]
    fn every_image_in_both_icons_decodes_to_its_rgba() {
        for (name, ico) in [("dev", DEV), ("standard", STANDARD)] {
            for size in SIZES {
                let (from, rgba) = rgba_from(ico, size).unwrap_or_else(|| panic!("{name} {size}"));
                assert_eq!(from, size);
                assert_eq!(rgba.len(), (size * size * 4) as usize);
                assert_ne!(rgba, crate::icon_art::disc_rgba(size), "{name} {size}: the chosen icon, not the drawn disc");
            }
        }
        // a size the .ico does not carry takes the larger neighbour
        assert_eq!(rgba_from(DEV, 100).map(|(from, _)| from), Some(128));
        // control: the two icons really differ, so the choice has a visible effect
        let (_, dev) = rgba_from(DEV, 256).unwrap();
        let (_, standard) = rgba_from(STANDARD, 256).unwrap();
        assert_ne!(dev, standard);
    }

    #[test]
    fn malformed_icons_are_refused_without_panicking() {
        let png = b"\x89PNG\r\n\x1a\n";
        let mut truncated = ico_with(&[(16, 16, png)]);
        truncated.pop();
        let cases = [
            ("count 0", ico_with(&[])),
            ("non-square", ico_with(&[(16, 32, png)])),
            ("bad magic", ico_with(&[(16, 16, b"notpng!!")])),
            ("truncated", truncated),
        ];
        for (name, bytes) in cases {
            assert!(parse_ico(&bytes).is_err(), "{name}: parse_ico must refuse it");
            assert!(rgba_from(&bytes, 16).is_none(), "{name}: rgba_from must return None");
        }
        // control: the same builder with a square PNG signature is accepted
        assert_eq!(parse_ico(&ico_with(&[(16, 16, png)])).map(|images| images[0].size), Ok(16));
    }

    #[test]
    fn the_release_channel_chooses_the_standard_icon() {
        assert_eq!(choose(Some("enhanced")), Set::Standard);
        assert_eq!(choose(Some("dev")), Set::Standard);
        assert_eq!(choose(None), Set::Dev);
        assert_eq!(choose(Some("")), Set::Dev);
        assert_ne!(DEV, STANDARD);
    }
}
