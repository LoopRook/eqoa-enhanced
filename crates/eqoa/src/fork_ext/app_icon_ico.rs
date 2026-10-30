// Fork-owned program-icon choice and .ico directory parsing. Plain std, no
// crate dependencies: build.rs `include!`s this file to choose and embed the
// icon, and the runtime module beside it (`app_icon`) parses the same file.

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Which of the owner's two program icons a build carries.
#[allow(dead_code)] // build.rs chooses; the runtime only parses the chosen file
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Set {
    /// Development builds: the DEV icon.
    Dev,
    /// Official release builds (any non-empty `EQOA_UPDATE_CHANNEL`): STANDARD.
    Standard,
}

/// A build with `EQOA_UPDATE_CHANNEL` set to a non-empty value is an official
/// release and carries the STANDARD icon; every other build carries DEV.
#[allow(dead_code)] // build.rs's included copy is the only caller
pub fn choose(channel: Option<&str>) -> Set {
    match channel {
        Some(channel) if !channel.is_empty() => Set::Standard,
        _ => Set::Dev,
    }
}

impl Set {
    /// The icon file, relative to `crates/eqoa/`.
    #[allow(dead_code)] // build.rs's included copy is the only caller
    pub fn file(self) -> &'static str {
        match self {
            Set::Dev => "resources/app-icon/eqoa-dev.ico",
            Set::Standard => "resources/app-icon/eqoa-standard.ico",
        }
    }
}

/// One image in an .ico: its square size and the raw image bytes (PNG here).
pub struct Image {
    pub size: u32,
    pub bytes: Vec<u8>,
}

/// The images in an .ico, in directory order. A width or height of 0 in the
/// directory means 256. Only the image bytes are returned: Windows accepts
/// PNG-compressed RT_ICON data as they are.
pub fn parse_ico(bytes: &[u8]) -> Result<Vec<Image>, String> {
    fn field(bytes: &[u8], at: usize) -> Result<[u8; 2], String> {
        bytes.get(at..at + 2).ok_or_else(|| "truncated .ico header".to_string())?.try_into().map_err(|_| "truncated .ico header".to_string())
    }
    let reserved = u16::from_le_bytes(field(bytes, 0)?);
    let kind = u16::from_le_bytes(field(bytes, 2)?);
    let count = u16::from_le_bytes(field(bytes, 4)?) as usize;
    if reserved != 0 || kind != 1 {
        return Err(format!("not an icon: reserved {reserved}, type {kind}"));
    }
    if count == 0 {
        return Err("icon has no images".into());
    }
    let mut images = Vec::with_capacity(count);
    for i in 0..count {
        let at = 6 + i * 16;
        let entry = bytes.get(at..at + 16).ok_or_else(|| "truncated .ico directory".to_string())?;
        let width = if entry[0] == 0 { 256 } else { entry[0] as u32 };
        let height = if entry[1] == 0 { 256 } else { entry[1] as u32 };
        if width != height {
            return Err(format!("image {i}: not square ({width}x{height})"));
        }
        let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let end = offset.checked_add(length).ok_or_else(|| format!("image {i}: length overflows"))?;
        let data = bytes.get(offset..end).ok_or_else(|| format!("image {i}: truncated"))?;
        if !data.starts_with(PNG) {
            return Err(format!("image {i}: not PNG"));
        }
        images.push(Image { size: width, bytes: data.to_vec() });
    }
    Ok(images)
}

static IMAGES: std::sync::OnceLock<Vec<(u32, Vec<u8>)>> = std::sync::OnceLock::new();

/// Stages the chosen .ico in OUT_DIR on every target, including non-Windows
/// builds that still take it for the window's icon.
#[allow(dead_code)] // build.rs's included copy is the only caller
pub fn stage() {
    images();
}

/// The chosen .ico's images for build.rs, staging the same file as
/// `OUT_DIR/app-icon.ico` for the program's `include_bytes!`.
#[allow(dead_code)] // build.rs's included copy is the only caller
pub fn images() -> &'static [(u32, Vec<u8>)] {
    IMAGES.get_or_init(|| {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let set = choose(std::env::var("EQOA_UPDATE_CHANNEL").ok().as_deref());
        let bytes = std::fs::read(set.file()).unwrap_or_else(|e| panic!("{}: {e}", set.file()));
        let images = parse_ico(&bytes).unwrap_or_else(|e| panic!("{}: {e}", set.file()));
        std::fs::write(out.join("app-icon.ico"), &bytes).expect("app-icon.ico");
        images.into_iter().map(|image| (image.size, image.bytes)).collect()
    }).as_slice()
}
