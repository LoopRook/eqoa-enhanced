//! The owner's program icon: build.rs chooses one of two .ico files
//! (`app_icon_ico::choose`: STANDARD when `EQOA_UPDATE_CHANNEL` is set, DEV
//! otherwise) and copies it into OUT_DIR. This module decodes the image
//! nearest each size `icon` asks for, so the window, the taskbar and the
//! .exe carry the same picture for the whole run.

#[path = "app_icon_ico.rs"]
mod app_icon_ico;

/// The chosen icon, copied into OUT_DIR by build.rs.
pub const ICO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app-icon.ico"));

/// `size` square RGBA from the embedded image nearest `size`, and that
/// image's own size (the caller shrinks it when the two differ).
pub fn rgba(size: u32) -> Option<(u32, Vec<u8>)> {
    let images = app_icon_ico::parse_ico(ICO).ok()?;
    let image = images.iter().min_by_key(|image| (image.size.abs_diff(size), image.size))?;
    let (width, _, rgba) = decode(&image.bytes)?;
    Some((width, rgba))
}

/// A PNG as RGBA8, if that is what it is.
fn decode(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    (info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight).then_some((info.width, info.height, buf))
}

#[cfg(test)]
mod tests {
    use super::app_icon_ico::{choose, parse_ico, Set};
    use super::*;

    const DEV: &[u8] = include_bytes!("../../resources/app-icon/eqoa-dev.ico");
    const STANDARD: &[u8] = include_bytes!("../../resources/app-icon/eqoa-standard.ico");

    #[test]
    fn both_icon_files_hold_seven_png_images_from_16_to_256() {
        for (name, bytes) in [("dev", DEV), ("standard", STANDARD)] {
            let images = parse_ico(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            let sizes: Vec<u32> = images.iter().map(|image| image.size).collect();
            assert_eq!(sizes, [16, 24, 32, 48, 64, 128, 256], "{name}");
            for image in &images {
                assert!(image.bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{name}: {} is not PNG", image.size);
            }
        }
        // control: a truncated directory is refused rather than silently short
        assert!(parse_ico(&DEV[..20]).is_err(), "a cut-off .ico is not a full icon");
    }

    #[test]
    fn the_release_channel_chooses_the_standard_icon() {
        assert_eq!(choose(Some("enhanced")), Set::Standard);
        assert_eq!(choose(Some("dev")), Set::Standard);
        assert_eq!(choose(None), Set::Dev);
        assert_eq!(choose(Some("")), Set::Dev);
        // control: the two files differ, so the choice has a visible effect
        assert_ne!(DEV, STANDARD);
    }

    #[test]
    fn the_window_icon_is_the_chosen_picture_not_the_drawn_disc() {
        for size in [16u32, 32, 128, 256] {
            let (from, rgba) = rgba(size).unwrap_or_else(|| panic!("an image for {size}"));
            assert_eq!(from, size);
            assert_eq!(rgba.len(), (size * size * 4) as usize);
            assert_ne!(rgba, crate::icon_art::disc_rgba(size), "size {size}: the chosen icon, not the drawn disc");
        }
        // control: a size the .ico does not carry takes the nearest image
        assert_eq!(rgba(100).map(|(from, _)| from), Some(128));
    }
}
