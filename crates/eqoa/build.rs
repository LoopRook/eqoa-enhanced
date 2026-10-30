//! The main thread's stack on Windows: 8 MB, as Linux and macOS give it.
//!
//! A Windows program's main thread has the stack its header reserves, 1 MB
//! unless the link says otherwise, and the game's first frame needs more:
//! its frames hold their state by value, and when the first frame opens the
//! sound device the stack holds about 980 KB (measured under cdb on a Windows
//! machine: `eqoa::run` 241 KB, `viewer::run_app` 236, `App::host_frame` 174,
//! `run_boot` 123, `Audio::open` 111, `window_event` 54). Without this the
//! client died there, "thread 'main' has overflowed its stack", before its
//! window had drawn anything: a white window (2026-10-22). `dev.yml` refuses
//! a Windows build whose header reserves less.
//!
//! And the Windows program's own icon (Explorer, a pinned shortcut, the
//! taskbar before the window sets its own): the owner's chosen .ico
//! (`src/fork_ext/app_icon_ico.rs`: STANDARD when `EQOA_UPDATE_CHANNEL` is
//! set, DEV otherwise), its images written into a resource file here and
//! linked in: MSVC's linker takes the .res as it is, GNU's needs it as an
//! object (windres). The same .ico is copied to OUT_DIR for the window's icon
//! (`icon`, `fork_ext::app_icon`).

#[allow(dead_code)]
mod app_icon_ico {
    include!("src/fork_ext/app_icon_ico.rs");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=EQOA_UPDATE_CHANNEL");
    println!("cargo:rerun-if-changed=src/fork_ext/app_icon_ico.rs");
    for icon in ["resources/app-icon/eqoa-dev.ico", "resources/app-icon/eqoa-standard.ico"] {
        println!("cargo:rerun-if-changed={icon}");
    }

    let set = app_icon_ico::choose(std::env::var("EQOA_UPDATE_CHANNEL").ok().as_deref());
    let bytes = std::fs::read(set.file()).unwrap_or_else(|e| panic!("{}: {e}", set.file()));
    let images: Vec<(u32, Vec<u8>)> = app_icon_ico::parse_ico(&bytes)
        .unwrap_or_else(|e| panic!("{}: {e}", set.file()))
        .into_iter()
        .map(|image| (image.size, image.bytes))
        .collect();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("app-icon.ico"), &bytes).expect("app-icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
            Ok("msvc") => println!("cargo:rustc-link-arg-bins=/STACK:8388608"),
            Ok("gnu") => println!("cargo:rustc-link-arg-bins=-Wl,--stack,8388608"),
            _ => {}
        }
        windows_icon(&out, &images);
    }
}

#[allow(dead_code)]
fn windows_icon(out: &std::path::Path, images: &[(u32, Vec<u8>)]) {
    let res = out.join("eqoa.res");
    std::fs::write(&res, resources(images)).expect("eqoa.res");
    match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => println!("cargo:rustc-link-arg-bins={}", res.display()),
        Ok("gnu") => {
            // `$WINDRES`, else the cross toolchain's, else the host's
            let obj = out.join("eqoa-res.o");
            let tried: Vec<String> = std::env::var("WINDRES").ok().into_iter().chain(["x86_64-w64-mingw32-windres".into(), "windres".into()]).collect();
            let made = tried.iter().any(|w| {
                std::process::Command::new(w).args(["-J", "res", "-O", "coff", "-i"]).arg(&res).arg("-o").arg(&obj).status().is_ok_and(|s| s.success())
            });
            if made {
                println!("cargo:rustc-link-arg-bins={}", obj.display());
            } else {
                println!("cargo:warning=no windres ({}): the program is built without its icon", tried.join(", "));
            }
        }
        _ => {}
    }
}

/// One image's fields in the RT_GROUP_ICON directory.
#[allow(dead_code)]
fn entry_head(size: u32, bytes: usize) -> Vec<u8> {
    let dim = if size >= 256 { 0 } else { size as u8 };
    let mut e = vec![dim, dim, 0, 0];
    e.extend_from_slice(&1u16.to_le_bytes()); // planes
    e.extend_from_slice(&32u16.to_le_bytes()); // bits a pixel
    e.extend_from_slice(&(bytes as u32).to_le_bytes());
    e
}

/// A resource file (.res): its empty first entry, each image an RT_ICON
/// named by its number, then the RT_GROUP_ICON (number 1) that lists them,
/// which Explorer shows as the program's icon. The image bytes are the
/// .ico's own PNG-compressed images, which Windows accepts as RT_ICON data.
#[allow(dead_code)]
fn resources(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    fn resource(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&32u32.to_le_bytes()); // the header's size
        for v in [0xFFFFu16, kind, 0xFFFF, name] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // data version
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&0x0409u16.to_le_bytes()); // English (US), as rc's default
        out.extend_from_slice(&0u32.to_le_bytes()); // version
        out.extend_from_slice(&0u32.to_le_bytes()); // characteristics
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    let mut out = Vec::new();
    // the .res file's mark: an entry of nothing
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    for v in [0xFFFFu16, 0, 0xFFFF, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0u8; 16]);
    for (i, (_, data)) in images.iter().enumerate() {
        resource(&mut out, RT_ICON, i as u16 + 1, 0x1010, data);
    }
    let mut group = Vec::new();
    for v in [0u16, 1, images.len() as u16] {
        group.extend_from_slice(&v.to_le_bytes());
    }
    for (i, (size, data)) in images.iter().enumerate() {
        group.extend(entry_head(*size, data.len()));
        group.extend_from_slice(&(i as u16 + 1).to_le_bytes());
    }
    resource(&mut out, RT_GROUP_ICON, 1, 0x1030, &group);
    out
}
