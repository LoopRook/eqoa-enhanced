//! The program's icon: the owner's chosen .ico (`fork_ext::app_icon`, picked
//! at build time: STANDARD for an official release, DEV otherwise) on the
//! window, the taskbar and the .exe, for the whole run. The fork no longer
//! switches to the game's memory-card save icon off the player's disc.
//!
//! Where it shows: Windows, the window's title bar and taskbar button (and
//! the .exe's own icon for Explorer and a pinned shortcut: build.rs); Linux,
//! the window under X11 (`_NET_WM_ICON`; Wayland takes a program's icon from
//! its desktop entry, which a loose binary has none of); macOS, the Dock
//! (`NSApplication`'s icon image: winit sets no window icon there).

use winit::window::{Icon, Window, WindowAttributes};

/// The picture's own size: the Dock's and Windows' taskbar's.
const BIG: u32 = 256;
/// The window's (X11 scales it as it needs).
const WINDOW: u32 = 128;
/// Windows' title bar (it scales it to 16, 20 or 24 by the screen's scale).
#[cfg(windows)]
const SMALL: u32 = 32;

/// `size` square RGBA from the chosen icon, or None if it cannot be read.
fn picture(size: u32) -> Option<Vec<u8>> {
    let (from, rgba) = crate::fork_ext::app_icon::rgba(size)?;
    Some(shrink(&rgba, from, size))
}

/// A square RGBA picture as PNG.
pub fn encode(rgba: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// `rgba` (`from` square) made `to` square by averaging (premultiplied), for
/// sizes that divide it; any other a nearest pick.
pub fn shrink(rgba: &[u8], from: u32, to: u32) -> Vec<u8> {
    let (from, to) = (from as usize, to as usize);
    if from == to {
        return rgba.to_vec();
    }
    let mut out = vec![0u8; to * to * 4];
    if from % to != 0 {
        for y in 0..to {
            for x in 0..to {
                let (sx, sy) = (x * from / to, y * from / to);
                out[(y * to + x) * 4..][..4].copy_from_slice(&rgba[(sy * from + sx) * 4..][..4]);
            }
        }
        return out;
    }
    let k = from / to;
    for y in 0..to {
        for x in 0..to {
            let mut acc = [0f32; 4];
            for j in 0..k {
                for i in 0..k {
                    let p = &rgba[((y * k + j) * from + x * k + i) * 4..][..4];
                    let a = p[3] as f32 / 255.0;
                    for c in 0..3 {
                        acc[c] += p[c] as f32 * a;
                    }
                    acc[3] += a;
                }
            }
            let o = (y * to + x) * 4;
            if acc[3] > 0.0 {
                for c in 0..3 {
                    out[o + c] = (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
            }
            out[o + 3] = (acc[3] / (k * k) as f32 * 255.0).round() as u8;
        }
    }
    out
}

fn icon(size: u32) -> Option<Icon> {
    Icon::from_rgba(picture(size)?, size, size).ok()
}

/// A window's attributes with the program's icon, as it opens.
pub fn attributes(a: WindowAttributes) -> WindowAttributes {
    let a = a.with_window_icon(icon(WINDOW));
    #[cfg(windows)]
    let a = {
        use winit::platform::windows::WindowAttributesExtWindows;
        a.with_window_icon(icon(SMALL)).with_taskbar_icon(icon(BIG))
    };
    a
}

/// macOS: the Dock's picture, as the program starts (winit's `resumed`).
#[cfg(target_os = "macos")]
pub fn dock_now() {
    if let Some(picture) = picture(BIG) {
        dock(&picture);
    }
}

#[cfg(target_os = "macos")]
fn dock(rgba: &[u8]) {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Ok(png) = encode(rgba, BIG) else { return };
    let data = NSData::with_bytes(&png);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else { return };
    // SAFETY: an image, never None (which the setter may not allow).
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&*image)) };
}

/// The fork keeps the chosen program icon for the whole run, so there is
/// nothing to poll. The viewer still calls this once a frame.
pub fn poll(_window: &Window) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_disc_is_round_with_a_hole_and_clear_corners() {
        let n = 64usize;
        let d = crate::icon_art::disc_rgba(n as u32);
        assert_eq!(d.len(), n * n * 4);
        let a = |x: usize, y: usize| d[(y * n + x) * 4 + 3];
        // the corners and the hole clear, the ring solid
        for (x, y) in [(0, 0), (n - 1, 0), (0, n - 1), (n - 1, n - 1), (n / 2, n / 2), (n / 2 - 1, n / 2 - 1)] {
            assert_eq!(a(x, y), 0, "clear at {x},{y}");
        }
        for (x, y) in [(n / 2, 3), (n / 2, n - 4), (3, n / 2), (n - 4, n / 2), (n / 2 + n / 4, n / 2)] {
            assert_eq!(a(x, y), 255, "solid at {x},{y}");
        }
        // round: the alpha is the same at the four quarter turns
        for r in [5usize, 12, 20, 28] {
            let c = n / 2;
            let v = [a(c + r, c), a(c - 1 - r, c), a(c, c + r), a(c, c - 1 - r)];
            assert!(v.iter().all(|&x| x == v[0]), "symmetric at radius {r}: {v:?}");
        }
        // control: a square of the same size would be solid in its corners
        let square = vec![255u8; n * n * 4];
        assert_ne!(square[3], a(0, 0));
    }

    #[test]
    fn small_icons_keep_the_disc() {
        for s in [16u32, 24, 32, 48] {
            let d = crate::icon_art::disc_rgba(s);
            let n = s as usize;
            let cover = d.chunks(4).filter(|p| p[3] > 128).count() as f32 / (n * n) as f32;
            // a disc of radius s/2 - 0.5 less its hole: a bit under pi/4
            assert!(cover > 0.6 && cover < 0.8, "size {s}: coverage {cover}");
        }
    }

    #[test]
    fn shrinking_averages_and_keeps_the_clear() {
        let big = crate::icon_art::disc_rgba(BIG);
        let small = shrink(&big, BIG, 32);
        assert_eq!(small.len(), 32 * 32 * 4);
        assert_eq!(small[3], 0, "a clear corner stays clear");
        assert!(small[(16 * 32 + 2) * 4 + 3] > 200, "the ring stays solid");
        // control: a nearest pick of a size that does not divide still has its size
        assert_eq!(shrink(&big, BIG, 48).len(), 48 * 48 * 4);
    }
}
