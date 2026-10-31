//! Fork-owned problem and crash report saving to the Desktop.
//! Replaces remote upload transport with a self-contained ZIP file
//! written to the user's Desktop (or config directory fallback).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

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

/// The first line of every log written from now on.
pub fn log_header() -> String {
    format!(
        "{}; enabled mods/addons: {}",
        enhanced_label(),
        enabled_mods_addons_string()
    )
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
                    *const [u8; 16],
                    u32,
                    *mut c_void,
                    *mut *mut u16,
                ) -> i32;
                let sh_get_known_folder_path: SHGetKnownFolderPathFn = std::mem::transmute(func);
                // FOLDERID_Desktop: {B4BFCC3A-DB2C-424C-B029-7FE99A87C641}
                let folder_id: [u8; 16] = [
                    0x3A, 0xCC, 0xBF, 0xB4, 0x2C, 0xDB, 0x4C, 0x42,
                    0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41,
                ];
                let mut path_ptr: *mut u16 = std::ptr::null_mut();
                let hr = sh_get_known_folder_path(&folder_id, 0, std::ptr::null_mut(), &mut path_ptr);
                if hr == 0 && !path_ptr.is_null() {
                    let mut len = 0;
                    while *path_ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(path_ptr, len);
                    let os_str = std::ffi::OsString::from_wide(slice);
                    let ole32 = windows_sys::Win32::System::LibraryLoader::LoadLibraryA(b"ole32.dll\0".as_ptr());
                    if !ole32.is_null() {
                        let free_proc = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                            ole32,
                            b"CoTaskMemFree\0".as_ptr(),
                        );
                        if let Some(free_fn) = free_proc {
                            let co_task_mem_free: unsafe extern "system" fn(*mut c_void) =
                                std::mem::transmute(free_fn);
                            co_task_mem_free(path_ptr as *mut c_void);
                        }
                    }
                    return Some(PathBuf::from(os_str));
                }
            }
        }
    }

    if let Some(od) = std::env::var_os("OneDrive") {
        let p = PathBuf::from(od).join("Desktop");
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(up) = std::env::var_os("USERPROFILE") {
        let p = PathBuf::from(up).join("Desktop");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

#[cfg(not(windows))]
fn desktop_dir_os() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join("Desktop");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

/// Opens the file or folder in Explorer / file manager with the file selected.
pub fn reveal_in_file_manager(path: &Path) {
    if std::env::var_os("EQOA_NO_EXPLORER").is_some()
        || std::env::var_os("EQOA_REPORT_DESKTOP").is_some()
    {
        return;
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn();
        }
    }
}

pub struct ZipEntry {
    pub name: String,
    pub data: Vec<u8>,
}

/// Writes entries into a standard PKZIP archive.
pub fn write_zip(entries: &[ZipEntry]) -> Result<Vec<u8>, std::io::Error> {
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for entry in entries {
        let offset = out.len() as u32;
        let mut crc = flate2::Crc::new();
        crc.update(&entry.data);
        let checksum = crc.sum();
        let uncompressed_size = entry.data.len() as u32;

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&entry.data)?;
        let compressed_data = encoder.finish()?;
        let compressed_size = compressed_data.len() as u32;

        let filename_bytes = entry.name.as_bytes();
        let filename_len = filename_bytes.len() as u16;

        // Local file header: 30 bytes
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
    kind: &crate::report::Kind,
    sent: bool,
    several: bool,
) -> Vec<(String, crate::report::Button)> {
    match (kind, sent) {
        (_, true) => vec![("Close (Enter)".into(), crate::report::Button::Send)],
        (crate::report::Kind::Problem, false) => vec![
            ("Save to Desktop (Enter)".into(), crate::report::Button::Send),
            ("Close (Esc)".into(), crate::report::Button::Close),
        ],
        (crate::report::Kind::Crash(_), false) if several => vec![
            (
                "Save all to Desktop (Enter)".into(),
                crate::report::Button::Send,
            ),
            ("Not now (Esc)".into(), crate::report::Button::Close),
            ("Delete all (Del)".into(), crate::report::Button::Delete),
        ],
        (crate::report::Kind::Crash(_), false) => vec![
            ("Save to Desktop (Enter)".into(), crate::report::Button::Send),
            ("Not now (Esc)".into(), crate::report::Button::Close),
            ("Delete (Del)".into(), crate::report::Button::Delete),
        ],
    }
}

/// Dialog status message for Enhanced. Shows confirmation with saved path.
pub fn dialog_status(status: &crate::report::Status, left: usize) -> String {
    match status {
        crate::report::Status::Editing => String::new(),
        crate::report::Status::Sending => "Saving to Desktop…".to_string(),
        crate::report::Status::Sent(paths)
            if paths.len() == 1 && (paths[0].contains('/') || paths[0].contains('\\')) =>
        {
            format!("Saved to {}. Thank you.", paths[0])
        }
        crate::report::Status::Sent(paths) if paths.len() > 1 => {
            format!("Sent all {} as {}. Thank you.", paths.len(), crate::report::and_list(paths))
        }
        crate::report::Status::Sent(paths) => {
            format!("Sent as {}. Thank you.", crate::report::and_list(paths))
        }
        crate::report::Status::Failed(paths, e) if paths.is_empty() => {
            format!("Not sent: {e}. Try again in a moment.")
        }
        crate::report::Status::Failed(paths, e) => {
            format!(
                "Sent {} as {}; {left} not sent: {e}. Try again in a moment.",
                paths.len(),
                crate::report::and_list(paths)
            )
        }
    }
}

/// Saves the report to the user's Desktop as a ZIP file.
/// If Desktop is unavailable or unwritable, falls back to the config directory.
pub fn save_report(
    kind: &crate::report::Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
) -> Result<(PathBuf, bool), String> {
    crate::report::flush_log();
    let config_dir = std::env::var_os("EQOA_CONFIG")
        .map(PathBuf::from)
        .or_else(crate::report::config_dir);
    let stamp = crate::report::stamp(SystemTime::now());
    let zip_name = format!("EQOA-Enhanced-report-{stamp}.zip");

    let mut entries = Vec::new();
    let mut files_list = Vec::new();

    // 1. note.txt
    let clean_note = crate::report::clean_now(note);
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
    if let Some(d) = &config_dir {
        let log_path = d.join("eqoa.log");
        if log_path.exists() {
            if let Ok(content) = fs::read_to_string(&log_path) {
                entries.push(ZipEntry {
                    name: "eqoa.log".into(),
                    data: crate::report::clean_now(&content).into_bytes(),
                });
                files_list.push("eqoa.log");
            }
        }
    }

    // 4. eqoa.prev.log
    if let Some(d) = &config_dir {
        let prev_path = d.join("eqoa.prev.log");
        if prev_path.exists() {
            if let Ok(content) = fs::read_to_string(&prev_path) {
                entries.push(ZipEntry {
                    name: "eqoa.prev.log".into(),
                    data: crate::report::clean_now(&content).into_bytes(),
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
                data: crate::report::clean_now(&content).into_bytes(),
            });
            files_list.push("addons_error.log");
        }
    }

    // 6. waiting crash reports
    match kind {
        crate::report::Kind::Crash(files) => {
            for f in files {
                if let Ok(content) = fs::read_to_string(f) {
                    let file_name = f
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "crash.txt".into());
                    let zip_path = format!("crashes/{file_name}");
                    entries.push(ZipEntry {
                        name: zip_path.clone(),
                        data: crate::report::clean_now(&content).into_bytes(),
                    });
                    files_list.push("crash report");
                }
            }
        }
        crate::report::Kind::Problem => {
            let crashes_dir = config_dir.as_ref().map(|d| d.join("crashes"));
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
                                    data: crate::report::clean_now(&content).into_bytes(),
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
    let state_text = {
        let noted = crate::report::noted_state();
        if !noted.trim().is_empty() {
            noted
        } else if let Some(d) = &config_dir {
            fs::read_to_string(d.join("state.txt")).unwrap_or_default()
        } else {
            String::new()
        }
    };
    if !state_text.trim().is_empty() {
        entries.push(ZipEntry {
            name: "client_state.txt".into(),
            data: crate::report::clean_now(&state_text).into_bytes(),
        });
        files_list.push("client_state.txt");
    }

    // 8. manifest.txt (with the required labelling and header)
    let header = enhanced_label();
    let mods_addons = enabled_mods_addons_string();
    let manifest_text = format!(
        "{header}\nEnabled mods/addons: {mods_addons}\nTimestamp: {stamp}\nType: {}\nName: {}\nNote: {}\n\nFiles included:\n{}\n",
        match kind {
            crate::report::Kind::Problem => "Problem report",
            crate::report::Kind::Crash(_) => "Crash report",
        },
        if name.trim().is_empty() { "-" } else { name.trim() },
        if clean_note.trim().is_empty() { "-" } else { clean_note.trim() },
        if files_list.is_empty() {
            "- (manifest only)".into()
        } else {
            files_list.join("\n")
        }
    );
    let clean_manifest = crate::report::clean_now(&manifest_text);
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
                let c = config_dir
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
        let c = config_dir
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
    kind: &crate::report::Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
    result: &Arc<Mutex<Option<crate::report::Sent>>>,
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
                        crate::report::Kind::Crash(files) => {
                            let mut sent_list = Vec::new();
                            for f in files {
                                let _ = fs::remove_file(f);
                                sent_list.push((Some(f.clone()), path_str.clone()));
                            }
                            *crate::report::lock_result(&result) = Some(crate::report::Sent {
                                sent: sent_list,
                                gone: Vec::new(),
                                failed: None,
                            });
                        }
                        crate::report::Kind::Problem => {
                            *crate::report::lock_result(&result) = Some(crate::report::Sent {
                                sent: vec![(None, path_str)],
                                gone: Vec::new(),
                                failed: None,
                            });
                        }
                    }
                }
                Err(err) => {
                    crate::say(&format!("report: failed to save: {err}"));
                    *crate::report::lock_result(&result) = Some(crate::report::Sent {
                        sent: Vec::new(),
                        gone: Vec::new(),
                        failed: Some(err),
                    });
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

            let content = if method == 8 {
                let mut decoder = DeflateDecoder::new(compressed_data);
                let mut decompressed = Vec::with_capacity(uncompressed_size);
                decoder.read_to_end(&mut decompressed).unwrap();
                decompressed
            } else {
                compressed_data.to_vec()
            };

            out.push((name, content));
            idx = data_end;
        }
        out
    }

    #[test]
    fn enhanced_label_and_log_header_format() {
        let label = enhanced_label();
        assert!(label.starts_with("EQOA Enhanced "));
        assert!(label.contains("a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"));

        let log_head = log_header();
        assert!(log_head.starts_with(&label));
        assert!(log_head.contains("; enabled mods/addons: "));
    }

    #[test]
    fn saving_produces_zip_with_all_parts_and_enhanced_header() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_parts");
        let desktop_dir = temp_dir.path().join("Desktop");
        let config_dir = temp_dir.path().join("Config");
        fs::create_dir_all(&desktop_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        // Populate config dir with logs
        fs::write(config_dir.join("eqoa.log"), "eqoa log line 1\neqoa log line 2").unwrap();
        fs::write(config_dir.join("eqoa.prev.log"), "prev log line 1").unwrap();
        fs::write(config_dir.join("state.txt"), "player standing in Freeport").unwrap();

        // Populate crash report
        let crashes = config_dir.join("crashes");
        fs::create_dir_all(&crashes).unwrap();
        fs::write(crashes.join("crash-20261031-120000.txt"), "A panic occurred").unwrap();

        // Set env vars
        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &desktop_dir);
            std::env::set_var("EQOA_CONFIG", &config_dir);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
        }

        let fake_shot = b"\x89PNG\r\n\x1a\nfake screenshot";
        let (zip_path, fell_back) = save_report(
            &crate::report::Kind::Problem,
            "Game froze when casting spell",
            "TesterBob",
            Some(fake_shot),
        )
        .unwrap();

        assert!(!fell_back);
        assert!(zip_path.exists());
        assert!(zip_path.file_name().unwrap().to_str().unwrap().starts_with("EQOA-Enhanced-report-"));

        let zip_bytes = fs::read(&zip_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();

        assert!(names.contains(&"manifest.txt"), "manifest missing: {names:?}");
        assert!(names.contains(&"note.txt"), "note missing: {names:?}");
        assert!(names.contains(&"screenshot.png"), "screenshot missing: {names:?}");
        assert!(names.contains(&"eqoa.log"), "eqoa.log missing: {names:?}");
        assert!(names.contains(&"eqoa.prev.log"), "eqoa.prev.log missing: {names:?}");
        assert!(names.contains(&"client_state.txt"), "client_state missing: {names:?}");
        assert!(names.iter().any(|n| n.starts_with("crashes/")), "crashes missing: {names:?}");

        // Check manifest content has the Enhanced label and enabled mods
        let manifest_bytes = entries.iter().find(|(n, _)| n == "manifest.txt").unwrap().1.clone();
        let manifest_text = String::from_utf8(manifest_bytes).unwrap();
        let first_line = manifest_text.lines().next().unwrap();
        assert_eq!(first_line, enhanced_label());
        assert!(manifest_text.contains("Enabled mods/addons: "));
        assert!(manifest_text.contains("Game froze when casting spell"));
    }

    #[test]
    fn secrets_and_home_paths_are_scrubbed_from_every_file_in_zip() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_secrets");
        let desktop_dir = temp_dir.path().join("Desktop");
        let config_dir = temp_dir.path().join("Config");
        fs::create_dir_all(&desktop_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        let secret_account = "SuperSecretAccount42";
        let secret_pass = "SuperSecretPassword999";
        crate::report::secret(secret_account, secret_pass);

        let fake_home = temp_dir.path().join("HomeUser");
        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &desktop_dir);
            std::env::set_var("EQOA_CONFIG", &config_dir);
            std::env::set_var("USERPROFILE", &fake_home);
            std::env::set_var("HOME", &fake_home);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
        }
        let home_str = fake_home.to_string_lossy().into_owned();

        // Plant secrets into logs and state
        let log_content = format!("Login attempt with {secret_account} and {secret_pass} from {home_str}/eqoa");
        fs::write(config_dir.join("eqoa.log"), &log_content).unwrap();
        fs::write(config_dir.join("eqoa.prev.log"), format!("prev log {secret_account} at {home_str}")).unwrap();
        fs::write(config_dir.join("state.txt"), format!("state with {secret_pass}")).unwrap();

        let note = format!("I had an issue with {secret_account} and {secret_pass} on {home_str}");
        let (zip_path, _) = save_report(&crate::report::Kind::Problem, &note, "Anon", None).unwrap();

        let zip_bytes = fs::read(&zip_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);

        for (name, content) in &entries {
            let text = String::from_utf8_lossy(content);
            assert!(
                !text.contains(secret_account),
                "Secret account found in file {name}: {text}"
            );
            assert!(
                !text.contains(secret_pass),
                "Secret password found in file {name}: {text}"
            );
            assert!(
                !text.contains(&home_str),
                "Home path found in file {name}: {text}"
            );
        }
    }

    #[test]
    fn no_network_transport_is_invoked() {
        // Assert report_policy rejects default or remote URLs
        assert!(crate::report_policy::upload_url(None, "report").is_err());
        assert!(crate::report_policy::upload_url(Some("https://eqoaclassic.com/eqoa/reports"), "report").is_err());
    }

    #[test]
    fn crash_offer_path_saves_the_same_way_and_cleans_up() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_crash_offer");
        let desktop_dir = temp_dir.path().join("Desktop");
        let config_dir = temp_dir.path().join("Config");
        fs::create_dir_all(&desktop_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        let crash_file = config_dir.join("crash-20261031-150000.txt");
        fs::write(&crash_file, "Panic backtrace data").unwrap();

        unsafe {
            std::env::set_var("EQOA_REPORT_DESKTOP", &desktop_dir);
            std::env::set_var("EQOA_CONFIG", &config_dir);
            std::env::set_var("EQOA_NO_EXPLORER", "1");
        }

        let result = Arc::new(Mutex::new(None));
        let kind = crate::report::Kind::Crash(vec![crash_file.clone()]);
        handle_send(&kind, "I was walking near coach", "", None, &result);

        // Wait for worker thread to complete
        let start = std::time::Instant::now();
        let s = loop {
            if let Some(res) = result.lock().unwrap().take() {
                break res;
            }
            if start.elapsed().as_secs() > 5 {
                panic!("handle_send timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };

        assert!(s.failed.is_none());
        assert_eq!(s.sent.len(), 1);
        assert!(!crash_file.exists(), "crash file should be deleted after saving");

        // Check the generated zip file
        let zips: Vec<_> = fs::read_dir(&desktop_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "zip"))
            .collect();
        assert_eq!(zips.len(), 1);
        let zip_bytes = fs::read(&zips[0]).unwrap();
        let entries = extract_zip_entries(&zip_bytes);
        assert!(entries.iter().any(|(n, _)| n == "crashes/crash-20261031-150000.txt"));
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
            save_report(&crate::report::Kind::Problem, "Test fallback", "", None).unwrap();
        assert!(fell_back, "Expected fallback to config directory");
        assert_eq!(saved_path.parent().unwrap(), config_dir);
        assert!(saved_path.exists());
    }

    #[test]
    fn logs_begin_with_the_enhanced_line() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_log");
        let config_dir = temp_dir.path().join("Config");
        fs::create_dir_all(&config_dir).unwrap();
        unsafe {
            std::env::set_var("EQOA_CONFIG", &config_dir);
        }
        crate::report::start();
        let log_file = config_dir.join("eqoa.log");
        assert!(log_file.exists());
        let content = fs::read_to_string(&log_file).unwrap();
        let first_line = content.lines().next().unwrap();
        assert!(
            first_line.starts_with("EQOA Enhanced "),
            "first line does not start with Enhanced line: {first_line}"
        );
        assert!(
            first_line.contains("a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"),
            "first line missing fork notice: {first_line}"
        );
        assert!(first_line.contains("; enabled mods/addons: "));
    }

    #[test]
    fn button_labels_and_status_confirmation() {
        let labels_problem = dialog_labels(&crate::report::Kind::Problem, false, false);
        assert_eq!(labels_problem[0].0, "Save to Desktop (Enter)");
        assert_eq!(labels_problem[1].0, "Close (Esc)");

        let labels_crash_single =
            dialog_labels(&crate::report::Kind::Crash(vec!["a".into()]), false, false);
        assert_eq!(labels_crash_single[0].0, "Save to Desktop (Enter)");

        let labels_crash_multi = dialog_labels(
            &crate::report::Kind::Crash(vec!["a".into(), "b".into()]),
            false,
            true,
        );
        assert_eq!(labels_crash_multi[0].0, "Save all to Desktop (Enter)");

        let status_sent = dialog_status(
            &crate::report::Status::Sent(vec!["C:/path/report.zip".into()]),
            0,
        );
        assert_eq!(status_sent, "Saved to C:/path/report.zip. Thank you.");
    }
}
