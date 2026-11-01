//! Fork-owned problem and crash report saving to the Desktop.
//! Replaces remote upload transport with a self-contained ZIP file
//! written to the user's Desktop (or config directory fallback).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::report::{Button, Kind, Sent, Status, clean_now, dir};

/// Returns the Enhanced header label identifying this client fork.
pub fn enhanced_label() -> String {
    let version = include_str!("../../../../VERSION").trim();
    let commit = crate::update::COMMIT.map_or("local build", |c| {
        if c.len() > 7 {
            &c[..7]
        } else {
            c
        }
    });
    format!(
        "EQOA Enhanced {version} ({commit}), a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"
    )
}

/// Returns the formatted list of enabled mods and addons, or "none".
pub fn enabled_mods_addons_string() -> String {
    let mut items = Vec::new();

    // Plain companion tools and mods from mods/
    for m in crate::fork_ext::mods::list_mods() {
        if m.enabled {
            let ver = if m.version.is_empty() {
                String::new()
            } else {
                format!(" v{}", m.version)
            };
            items.push(format!("{}{ver}", m.name));
        }
    }

    // Discovered Lua addons in mods/
    let mods_root = crate::fork_ext::mods::mods_dir();
    let discovery = crate::fork_ext::addons::discovery::discover(&mods_root);
    let storage = crate::fork_ext::addons::storage::Storage::new(crate::settings::config_dir());
    for addon in discovery.addons {
        let is_enabled = match crate::fork_ext::addons::storage::Storage::key(&addon) {
            Ok(key) => storage
                .enabled(&key)
                .unwrap_or(None)
                .unwrap_or(addon.initially_enabled),
            Err(_) => addon.initially_enabled,
        };
        if is_enabled {
            items.push(addon.name);
        }
    }

    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

/// The Enhanced log header line pushed after the run head line.
pub fn log_header() -> String {
    format!(
        "{}; enabled mods/addons: {}",
        enhanced_label(),
        enabled_mods_addons_string()
    )
}

/// Formats civil date and time as YYYYMMDD-HHMMSS (Howard Hinnant's algorithm).
fn stamp(t: SystemTime) -> String {
    let s = t.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, secs) = (s / 86400, s % 86400);
    let (h, _m, sec) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}-{h:02}{m_val:02}{sec:02}", m_val = m)
}

/// Formats items as "a", "a and b", or "a, b and c".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [two_a, two_b] => format!("{two_a} and {two_b}"),
        many => format!("{}, and {}", many[..many.len() - 1].join(", "), many.last().unwrap()),
    }
}

/// Resolves the user's Desktop directory.
/// Checks EQOA_REPORT_DESKTOP first for testing and explicit override.
pub fn desktop_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("EQOA_REPORT_DESKTOP") {
        let p = PathBuf::from(d);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    desktop_dir_os()
}

#[cfg(windows)]
fn desktop_dir_os() -> Option<PathBuf> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let shell32 = windows_sys::Win32::System::LibraryLoader::LoadLibraryA(b"shell32.dll\0".as_ptr());
        if !shell32.is_null() {
            let proc = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                shell32,
                b"SHGetKnownFolderPath\0".as_ptr(),
            );
            if let Some(func) = proc {
                type SHGetKnownFolderPathFn = unsafe extern "system" fn(
                    *const windows_sys::core::GUID,
                    u32,
                    windows_sys::Win32::Foundation::HANDLE,
                    *mut *mut u16,
                ) -> i32;
                let sh_get_known_folder_path: SHGetKnownFolderPathFn = std::mem::transmute(func);

                // FOLDERID_Desktop: {B4BFCC3A-DB2C-424C-B029-7FE99A87C641}
                let folder_id = windows_sys::core::GUID {
                    data1: 0xB4BFCC3A,
                    data2: 0xDB2C,
                    data3: 0x424C,
                    data4: [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41],
                };
                let mut path_ptr: *mut u16 = std::ptr::null_mut();
                if sh_get_known_folder_path(&folder_id, 0, std::ptr::null_mut(), &mut path_ptr) == 0 && !path_ptr.is_null() {
                    let mut len = 0;
                    while *path_ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(path_ptr, len);
                    let os_str = std::ffi::OsString::from_wide(slice);
                    windows_sys::Win32::System::Com::CoTaskMemFree(path_ptr as *mut c_void);
                    windows_sys::Win32::Foundation::FreeLibrary(shell32);
                    let path = PathBuf::from(os_str);
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
            windows_sys::Win32::Foundation::FreeLibrary(shell32);
        }
    }

    // Fallbacks on Windows
    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        let one_drive_desktop = PathBuf::from(&userprofile).join("OneDrive").join("Desktop");
        if one_drive_desktop.exists() {
            return Some(one_drive_desktop);
        }
        let std_desktop = PathBuf::from(&userprofile).join("Desktop");
        if std_desktop.exists() {
            return Some(std_desktop);
        }
    }
    None
}

#[cfg(not(windows))]
fn desktop_dir_os() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join("Desktop");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Reveals the saved file in the platform's file manager with the file selected.
pub fn reveal_in_file_manager(path: &Path) {
    if std::env::var_os("EQOA_NO_EXPLORER").is_some() {
        return;
    }

    #[cfg(windows)]
    {
        let arg = format!("/select,{}", path.display());
        let _ = std::process::Command::new("explorer").arg(arg).spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
        }
    }
}

/// Structure representing an uncompressed or pre-compressed entry to pack into a ZIP.
struct ZipEntry {
    name: String,
    data: Vec<u8>,
}

/// Creates a standard, unencrypted PKZIP 2.0 archive containing the entries.
fn write_zip(entries: &[ZipEntry]) -> Result<Vec<u8>, std::io::Error> {
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for entry in entries {
        let offset = out.len() as u32;
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&entry.data)?;
        let compressed_data = encoder.finish()?;

        let mut crc = flate2::Crc::new();
        crc.update(&entry.data);
        let checksum = crc.sum();

        let filename_bytes = entry.name.as_bytes();
        let filename_len = filename_bytes.len() as u16;
        let uncompressed_size = entry.data.len() as u32;
        let compressed_size = compressed_data.len() as u32;

        // Local file header: 30 bytes + filename_len
        out.write_all(b"PK\x03\x04")?;
        out.write_all(&20u16.to_le_bytes())?; // version needed: 2.0
        out.write_all(&0x0800u16.to_le_bytes())?; // flags: UTF-8 filename (bit 11)
        out.write_all(&8u16.to_le_bytes())?; // method: deflate
        out.write_all(&0u16.to_le_bytes())?; // time
        out.write_all(&0u16.to_le_bytes())?; // date
        out.write_all(&checksum.to_le_bytes())?;
        out.write_all(&compressed_size.to_le_bytes())?;
        out.write_all(&uncompressed_size.to_le_bytes())?;
        out.write_all(&filename_len.to_le_bytes())?;
        out.write_all(&0u16.to_le_bytes())?; // extra len
        out.write_all(filename_bytes)?;
        out.write_all(&compressed_data)?;

        // Central directory entry: 46 bytes + filename_len
        central_dir.write_all(b"PK\x01\x02")?;
        central_dir.write_all(&20u16.to_le_bytes())?; // version made by
        central_dir.write_all(&20u16.to_le_bytes())?; // version needed
        central_dir.write_all(&0x0800u16.to_le_bytes())?; // flags: UTF-8
        central_dir.write_all(&8u16.to_le_bytes())?; // method: deflate
        central_dir.write_all(&0u16.to_le_bytes())?; // time
        central_dir.write_all(&0u16.to_le_bytes())?; // date
        central_dir.write_all(&checksum.to_le_bytes())?;
        central_dir.write_all(&compressed_size.to_le_bytes())?;
        central_dir.write_all(&uncompressed_size.to_le_bytes())?;
        central_dir.write_all(&filename_len.to_le_bytes())?;
        central_dir.write_all(&0u16.to_le_bytes())?; // extra len
        central_dir.write_all(&0u16.to_le_bytes())?; // comment len
        central_dir.write_all(&0u16.to_le_bytes())?; // disk number
        central_dir.write_all(&0u16.to_le_bytes())?; // internal attr
        central_dir.write_all(&0u32.to_le_bytes())?; // external attr
        central_dir.write_all(&offset.to_le_bytes())?;
        central_dir.write_all(filename_bytes)?;
    }

    let central_dir_offset = out.len() as u32;
    let central_dir_size = central_dir.len() as u32;
    out.write_all(&central_dir)?;

    // End of central directory record: 22 bytes
    out.write_all(b"PK\x05\x06")?;
    out.write_all(&0u16.to_le_bytes())?; // disk number
    out.write_all(&0u16.to_le_bytes())?; // disk where central dir starts
    out.write_all(&(entries.len() as u16).to_le_bytes())?; // number of entries on this disk
    out.write_all(&(entries.len() as u16).to_le_bytes())?; // total number of entries
    out.write_all(&central_dir_size.to_le_bytes())?;
    out.write_all(&central_dir_offset.to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes())?; // comment length

    Ok(out)
}

/// Dialog button labels for Enhanced. Offers "Save to Desktop" instead of "Send".
pub fn dialog_labels(
    kind: &Kind,
    sent: bool,
    several: bool,
) -> Option<Vec<(String, Button)>> {
    match (kind, sent) {
        (_, true) => Some(vec![("Close (Enter)".into(), Button::Send)]),
        (Kind::Problem, false) => Some(vec![
            ("Save to Desktop (Enter)".into(), Button::Send),
            ("Close (Esc)".into(), Button::Close),
        ]),
        (Kind::Crash(_), false) if several => Some(vec![
            ("Save all to Desktop (Enter)".into(), Button::Send),
            ("Not now (Esc)".into(), Button::Close),
            ("Delete all (Del)".into(), Button::Delete),
        ]),
        (Kind::Crash(_), false) => Some(vec![
            ("Save to Desktop (Enter)".into(), Button::Send),
            ("Not now (Esc)".into(), Button::Close),
            ("Delete (Del)".into(), Button::Delete),
        ]),
    }
}

/// Dialog status message for Enhanced. Shows confirmation with saved path.
pub fn dialog_status(status: &Status, _left: usize) -> Option<String> {
    match status {
        Status::Editing => Some(String::new()),
        Status::Sending => Some("Saving to Desktop…".to_string()),
        Status::Sent(paths) if paths.iter().any(|p| p.contains('/') || p.contains('\\')) => {
            if paths.len() == 1 {
                Some(format!("Saved to {}. Thank you.", paths[0]))
            } else {
                Some(format!("Saved all {} as {}. Thank you.", paths.len(), and_list(paths)))
            }
        }
        _ => None,
    }
}

/// Resolves the configuration directory, supporting test environment fixtures.
pub(crate) fn config_dir() -> Option<PathBuf> {
    dir()
        .or_else(|| std::env::var_os("EQOA_CONFIG").map(PathBuf::from))
        .or_else(crate::settings::config_dir)
}

/// Saves the report to the user's Desktop as a ZIP file.
/// If Desktop is unavailable or unwritable, falls back to the config directory.
pub fn save_report(
    kind: &Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
) -> Result<(PathBuf, bool), String> {
    let cfg_dir = config_dir();
    let stamp_str = stamp(SystemTime::now());
    let zip_name = format!("EQOA-Enhanced-report-{stamp_str}.zip");

    let mut entries = Vec::new();
    let mut files_list = Vec::new();

    // 1. note.txt
    let clean_note = clean_now(note);
    if !clean_note.trim().is_empty() {
        entries.push(ZipEntry {
            name: "note.txt".into(),
            data: clean_note.clone().into_bytes(),
        });
        files_list.push("note.txt");
    }

    // 2. screenshot.png
    if let Some(png) = shot {
        entries.push(ZipEntry {
            name: "screenshot.png".into(),
            data: png.to_vec(),
        });
        files_list.push("screenshot.png");
    }

    // 3. eqoa.log
    if let Some(d) = &cfg_dir {
        let log_path = d.join("eqoa.log");
        if log_path.exists() {
            if let Ok(content) = fs::read_to_string(&log_path) {
                entries.push(ZipEntry {
                    name: "eqoa.log".into(),
                    data: clean_now(&content).into_bytes(),
                });
                files_list.push("eqoa.log");
            }
        }
    }

    // 4. eqoa.prev.log
    if let Some(d) = &cfg_dir {
        let prev_path = d.join("eqoa.prev.log");
        if prev_path.exists() {
            if let Ok(content) = fs::read_to_string(&prev_path) {
                entries.push(ZipEntry {
                    name: "eqoa.prev.log".into(),
                    data: clean_now(&content).into_bytes(),
                });
                files_list.push("eqoa.prev.log");
            }
        }
    }

    // 5. mods/addons_error.log (if present)
    let addons_error_path = crate::fork_ext::mods::mods_dir().join("addons_error.log");
    if addons_error_path.exists() {
        if let Ok(content) = fs::read_to_string(&addons_error_path) {
            entries.push(ZipEntry {
                name: "addons_error.log".into(),
                data: clean_now(&content).into_bytes(),
            });
            files_list.push("addons_error.log");
        }
    }

    // 6. waiting crash reports
    match kind {
        Kind::Crash(files) => {
            for f in files {
                if let Ok(content) = fs::read_to_string(f) {
                    let file_name = f
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "crash.txt".into());
                    let zip_path = format!("crashes/{file_name}");
                    entries.push(ZipEntry {
                        name: zip_path.clone(),
                        data: clean_now(&content).into_bytes(),
                    });
                    files_list.push("crash report");
                }
            }
        }
        Kind::Problem => {
            let crashes_dir = cfg_dir.as_ref().map(|d| d.join("crashes"));
            if let Some(crashes_d) = crashes_dir {
                if let Ok(read_dir) = fs::read_dir(crashes_d) {
                    for entry in read_dir.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "txt") {
                            if let Ok(content) = fs::read_to_string(&path) {
                                let file_name = path
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "crash.txt".into());
                                entries.push(ZipEntry {
                                    name: format!("crashes/{file_name}"),
                                    data: clean_now(&content).into_bytes(),
                                });
                                files_list.push("waiting crash report");
                            }
                        }
                    }
                }
            }
        }
    }

    // 7. client state
    let state_text = cfg_dir
        .as_ref()
        .and_then(|d| fs::read_to_string(d.join("state.txt")).ok())
        .unwrap_or_default();
    if !state_text.trim().is_empty() {
        entries.push(ZipEntry {
            name: "client_state.txt".into(),
            data: clean_now(&state_text).into_bytes(),
        });
        files_list.push("client_state.txt");
    }

    // 8. manifest.txt (with the required labelling and header)
    let header = enhanced_label();
    let mods_addons = enabled_mods_addons_string();
    let manifest_text = format!(
        "{header}\nEnabled mods/addons: {mods_addons}\nTimestamp: {stamp_str}\nType: {}\nName: {}\nNote: {}\n\nFiles included:\n{}\n",
        match kind {
            Kind::Problem => "Problem report",
            Kind::Crash(_) => "Crash report",
        },
        if name.trim().is_empty() { "-" } else { name.trim() },
        if clean_note.trim().is_empty() { "-" } else { clean_note.trim() },
        if files_list.is_empty() {
            "- (manifest only)".into()
        } else {
            files_list.join("\n")
        }
    );
    let clean_manifest = clean_now(&manifest_text);
    entries.insert(
        0,
        ZipEntry {
            name: "manifest.txt".into(),
            data: clean_manifest.into_bytes(),
        },
    );

    let zip_bytes = write_zip(&entries)
        .map_err(|e| format!("Failed to compress report zip: {e}"))?;

    // Determine target folder: try Desktop, fall back to config directory
    let desktop = desktop_dir();
    let mut fell_back = false;
    let final_path = if let Some(d) = desktop {
        let dest = d.join(&zip_name);
        match fs::create_dir_all(&d).and_then(|_| fs::write(&dest, &zip_bytes)) {
            Ok(()) => dest,
            Err(_) => {
                fell_back = true;
                let c = cfg_dir
                    .as_ref()
                    .ok_or("Both Desktop and config directory are unavailable")?;
                fs::create_dir_all(c).map_err(|e| e.to_string())?;
                let dest = c.join(&zip_name);
                fs::write(&dest, &zip_bytes).map_err(|e| e.to_string())?;
                dest
            }
        }
    } else {
        fell_back = true;
        let c = cfg_dir
            .as_ref()
            .ok_or("No Desktop or config directory available")?;
        fs::create_dir_all(c).map_err(|e| e.to_string())?;
        let dest = c.join(&zip_name);
        fs::write(&dest, &zip_bytes).map_err(|e| e.to_string())?;
        dest
    };

    reveal_in_file_manager(&final_path);
    Ok((final_path, fell_back))
}

/// Hook called from `Dialog::send`.
/// Performs Save to Desktop in the background thread.
pub fn handle_send(
    kind: &Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
    result: &Arc<Mutex<Option<Sent>>>,
) -> bool {
    let kind = kind.clone();
    let note = note.to_string();
    let name = name.to_string();
    let shot = shot.map(|s| s.to_vec());
    let result = result.clone();

    let _ = std::thread::Builder::new()
        .name("report".into())
        .spawn(move || {
            let outcome = save_report(&kind, &note, &name, shot.as_deref());
            match outcome {
                Ok((path, fell_back)) => {
                    let path_str = if fell_back {
                        format!("{} (saved to config directory; Desktop unavailable)", path.display())
                    } else {
                        path.display().to_string()
                    };
                    crate::say(&format!("report: saved to {}", path.display()));

                    match &kind {
                        Kind::Crash(files) => {
                            let mut sent_list = Vec::new();
                            for f in files {
                                let _ = fs::remove_file(f);
                                sent_list.push((Some(f.clone()), path_str.clone()));
                            }
                            *result.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(Sent::success(sent_list));
                        }
                        Kind::Problem => {
                            *result.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(Sent::success(vec![(None, path_str)]));
                        }
                    }
                }
                Err(err) => {
                    crate::say(&format!("report: failed to save: {err}"));
                    *result.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(Sent::failure(err));
                }
            }
        });

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    struct TestDir(PathBuf);
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    impl TestDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    fn test_dir(prefix: &str) -> TestDir {
        static CNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = CNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("eqoa-test-{prefix}-{}-{id}", std::process::id()));
        let _ = fs::create_dir_all(&d);
        TestDir(d)
    }

    // Helper to unzip test files
    fn extract_zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        use flate2::read::DeflateDecoder;
        let mut out = Vec::new();
        let mut idx = 0;
        while idx + 30 <= bytes.len() {
            if &bytes[idx..idx + 4] != b"PK\x03\x04" {
                break;
            }
            let method = u16::from_le_bytes(bytes[idx + 8..idx + 10].try_into().unwrap());
            let compressed_size =
                u32::from_le_bytes(bytes[idx + 18..idx + 22].try_into().unwrap()) as usize;
            let uncompressed_size =
                u32::from_le_bytes(bytes[idx + 22..idx + 26].try_into().unwrap()) as usize;
            let name_len =
                u16::from_le_bytes(bytes[idx + 26..idx + 28].try_into().unwrap()) as usize;
            let extra_len =
                u16::from_le_bytes(bytes[idx + 28..idx + 30].try_into().unwrap()) as usize;

            let name_start = idx + 30;
            let name_end = name_start + name_len;
            let name = String::from_utf8_lossy(&bytes[name_start..name_end]).into_owned();

            let data_start = name_end + extra_len;
            let data_end = data_start + compressed_size;
            let compressed_data = &bytes[data_start..data_end];

            let data = if method == 0 {
                compressed_data.to_vec()
            } else if method == 8 {
                let mut decoder = DeflateDecoder::new(compressed_data);
                let mut decomp = Vec::with_capacity(uncompressed_size);
                decoder.read_to_end(&mut decomp).unwrap();
                decomp
            } else {
                panic!("unsupported compression method: {method}");
            };

            out.push((name, data));
            idx = data_end;
        }
        out
    }

    #[test]
    fn saving_produces_zip_with_all_parts_and_enhanced_header() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_all_parts");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        fs::write(
            fake_config.join("eqoa.log"),
            "line 1\nline 2\nchat: hello world\n",
        )
        .unwrap();
        fs::write(
            fake_config.join("eqoa.prev.log"),
            "prev 1\nprev 2\n[eqoa] closed\n",
        )
        .unwrap();
        fs::write(
            fake_config.join("state.txt"),
            "frames 12345, clock 100.5 s\n",
        )
        .unwrap();

        let crashes_dir = fake_config.join("crashes");
        fs::create_dir_all(&crashes_dir).unwrap();
        fs::write(
            crashes_dir.join("crash-20261026-100000.txt"),
            "crash details 1",
        )
        .unwrap();

        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &fake_desktop);
            std::env::set_var("EQOA_CONFIG", &fake_config);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
        }

        let fake_shot = b"\x89PNG\r\n\x1a\nfake screenshot data";
        let note = "Test problem note describing an issue";
        let name = "Tester";

        let (saved_path, fell_back) = save_report(
            &Kind::Problem,
            note,
            name,
            Some(fake_shot),
        )
        .expect("save_report failed");

        assert!(!fell_back, "Report should save to Desktop");
        assert!(saved_path.exists());
        assert!(
            saved_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("EQOA-Enhanced-report-")
        );
        assert!(
            saved_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".zip")
        );

        let zip_bytes = fs::read(&saved_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);
        let entry_names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();

        assert!(entry_names.contains(&"manifest.txt"), "Missing manifest.txt");
        assert!(entry_names.contains(&"note.txt"), "Missing note.txt");
        assert!(entry_names.contains(&"screenshot.png"), "Missing screenshot.png");
        assert!(entry_names.contains(&"eqoa.log"), "Missing eqoa.log");
        assert!(entry_names.contains(&"eqoa.prev.log"), "Missing eqoa.prev.log");
        assert!(entry_names.contains(&"client_state.txt"), "Missing client_state.txt");
        assert!(
            entry_names.contains(&"crashes/crash-20261026-100000.txt"),
            "Missing waiting crash file"
        );

        let manifest_bytes = &entries.iter().find(|(n, _)| n == "manifest.txt").unwrap().1;
        let manifest_str = String::from_utf8_lossy(manifest_bytes);
        assert!(
            manifest_str.starts_with("EQOA Enhanced "),
            "Manifest must start with EQOA Enhanced header"
        );
        assert!(
            manifest_str.contains("a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"),
            "Manifest must contain fork report destination notice"
        );
        assert!(manifest_str.contains("Enabled mods/addons: "));
        assert!(manifest_str.contains("Note: Test problem note describing an issue"));
    }

    #[test]
    fn secrets_are_scrubbed_from_all_files_in_zip() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_secrets");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        let fake_account = "SuperSecretAccount42";
        let fake_password = "SecretPassword999";
        let fake_home = "C:\\Users\\SecretHomeDirectory";

        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &fake_desktop);
            std::env::set_var("EQOA_CONFIG", &fake_config);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
            std::env::set_var("USERPROFILE", fake_home);
        }

        crate::report::secret(fake_account, fake_password);

        fs::write(
            fake_config.join("eqoa.log"),
            format!("logged account {fake_account} with pass {fake_password} on {fake_home}\n"),
        )
        .unwrap();

        let note = format!("I had {fake_account} and {fake_password} in {fake_home}");
        let (saved_path, _) = save_report(
            &Kind::Problem,
            &note,
            fake_account,
            None,
        )
        .unwrap();

        let zip_bytes = fs::read(&saved_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);

        for (name, data) in entries {
            let text = String::from_utf8_lossy(&data);
            assert!(
                !text.contains(fake_account),
                "File {name} contains leaked account name"
            );
            assert!(
                !text.contains(fake_password),
                "File {name} contains leaked password"
            );
            assert!(
                !text.contains(fake_home),
                "File {name} contains leaked home path"
            );
        }
    }

    #[test]
    fn no_network_transport_is_invoked() {
        assert!(crate::report_policy::enabled());
        let remote_url = crate::report_policy::upload_url(
            Some("https://eqoaclassic.com/eqoa/reports"),
            "report",
        );
        assert!(
            remote_url.is_err(),
            "upload_url must reject remote non-loopback endpoints"
        );
        assert_eq!(
            remote_url.unwrap_err(),
            "Report uploads are disabled in EQOA Enhanced (local diagnostics are kept)"
        );
    }

    #[test]
    fn crash_offer_path_saves_to_desktop() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_crash_offer");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        let crash_file = fake_config.join("crashes").join("crash-20261026-123456.txt");
        fs::create_dir_all(crash_file.parent().unwrap()).unwrap();
        fs::write(&crash_file, "fatal crash log content").unwrap();

        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &fake_desktop);
            std::env::set_var("EQOA_CONFIG", &fake_config);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
        }

        let kind = Kind::Crash(vec![crash_file.clone()]);
        let result = Arc::new(Mutex::new(None));
        let handled = handle_send(&kind, "crash note", "crash reporter", None, &result);
        assert!(handled, "handle_send must return true for crash kind");

        let mut attempts = 0;
        let sent_res = loop {
            if let Some(res) = result.lock().unwrap().clone() {
                break res;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            attempts += 1;
            if attempts > 40 {
                panic!("timed out waiting for background save to finish");
            }
        };

        assert_eq!(sent_res.sent_records().len(), 1);
        let saved_zip_path = PathBuf::from(&sent_res.sent_records()[0].1);
        assert!(saved_zip_path.exists());
        assert_eq!(saved_zip_path.parent().unwrap(), fake_desktop);
        assert!(!crash_file.exists(), "Crash file should be cleaned up after saving");
    }

    #[test]
    fn unwritable_desktop_falls_back_to_config_directory() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_fallback");
        let config_dir = temp_dir.path().join("Config");
        fs::create_dir_all(&config_dir).unwrap();

        // Make desktop a file so create_dir_all or write inside it fails
        let blocker = temp_dir.path().join("blocker_file");
        fs::write(&blocker, "blocking file").unwrap();
        let invalid_desktop = blocker.join("desktop_sub");

        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &invalid_desktop);
            std::env::set_var("EQOA_CONFIG", &config_dir);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
        }

        let (saved_path, fell_back) =
            save_report(&Kind::Problem, "Test fallback", "", None).unwrap();
        assert!(fell_back, "Expected fallback to config directory");
        assert_eq!(saved_path.parent().unwrap(), config_dir);
        assert!(saved_path.exists());
    }

    #[test]
    fn log_with_enhanced_header_detected_as_ended_without_closing() {
        let temp_dir = test_dir("unclosed_test");
        let crashes_dir = temp_dir.path().join("crashes");
        let prev = temp_dir.path().join("eqoa.prev.log");
        let run_id = "20261031-120000";

        // Positive case: first line is [eqoa] run ..., second line is Enhanced header
        let log_content = format!(
            "[eqoa] run {run_id}, dev-1234567 (dev), windows x86_64, window 1\n{}\nwindow: as kept\n",
            log_header()
        );
        fs::write(&prev, log_content).unwrap();
        crate::report::ended_without_closing_for_test(temp_dir.path(), &prev);
        let crash_file = crashes_dir.join(format!("crash-{run_id}.txt"));
        assert!(crash_file.exists(), "crash report must be written when [eqoa] run is first line");
        let crash_content = fs::read_to_string(&crash_file).unwrap();
        assert!(crash_content.contains(run_id));

        // Negative control: Enhanced header placed first -> NOT detected as unclosed run
        let _ = fs::remove_file(&crash_file);
        let bad_log_content = format!(
            "{}\n[eqoa] run {run_id}, dev-1234567 (dev), windows x86_64, window 1\nwindow: as kept\n",
            log_header()
        );
        fs::write(&prev, bad_log_content).unwrap();
        crate::report::ended_without_closing_for_test(temp_dir.path(), &prev);
        assert!(!crash_file.exists(), "negative control: crash report must NOT be written when header is placed first");
    }

    #[test]
    fn logs_begin_with_the_enhanced_line() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::report::start();
        let tail_lines = crate::report::tail(10);
        let run_idx = tail_lines
            .iter()
            .rposition(|l| l.starts_with("[eqoa] run "))
            .expect("missing [eqoa] run head line");
        assert!(
            run_idx + 1 < tail_lines.len(),
            "Enhanced header line must follow [eqoa] run line"
        );
        let enhanced_line = &tail_lines[run_idx + 1];
        assert!(
            enhanced_line.starts_with("EQOA Enhanced ") || enhanced_line.starts_with("[eqoa] EQOA Enhanced "),
            "line after head does not start with Enhanced line: {enhanced_line}"
        );
        assert!(
            enhanced_line.contains("a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"),
            "Enhanced line missing fork notice: {enhanced_line}"
        );
        assert!(enhanced_line.contains("; enabled mods/addons: "));
    }

    #[test]
    fn button_labels_and_status_confirmation() {
        let labels_problem = dialog_labels(&Kind::Problem, false, false).unwrap();
        assert_eq!(labels_problem[0].0, "Save to Desktop (Enter)");
        assert_eq!(labels_problem[1].0, "Close (Esc)");

        let labels_crash_single =
            dialog_labels(&Kind::Crash(vec!["a".into()]), false, false).unwrap();
        assert_eq!(labels_crash_single[0].0, "Save to Desktop (Enter)");

        let labels_crash_multi = dialog_labels(
            &Kind::Crash(vec!["a".into(), "b".into()]),
            false,
            true,
        ).unwrap();
        assert_eq!(labels_crash_multi[0].0, "Save all to Desktop (Enter)");

        let status_sent = dialog_status(
            &Status::Sent(vec!["C:/path/report.zip".into()]),
            0,
        ).unwrap();
        assert_eq!(status_sent, "Saved to C:/path/report.zip. Thank you.");
    }
}
