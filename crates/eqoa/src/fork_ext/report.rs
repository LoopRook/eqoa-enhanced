// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Fork-owned problem and crash report saving to the Desktop.
//! Replaces remote upload transport with a self-contained ZIP file
//! written to the user's Desktop (or config directory fallback).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::report::{Button, Kind, Sent, Status, dir};

/// Returns the Enhanced header label identifying this client fork.
pub fn enhanced_label() -> String {
    let version = include_str!("../../../../VERSION").trim();
    let commit =
        crate::update::COMMIT.map_or("local build", |c| if c.len() > 7 { &c[..7] } else { c });
    format!(
        "EQOA Enhanced {version} ({commit}), a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client"
    )
}

#[cfg(test)]
thread_local! { static MOD_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
pub(crate) fn mod_scans_on_this_thread() -> usize {
    MOD_SCANS.get()
}

/// Reads the snapshot's background-refreshed addon cache without touching disk.
pub(crate) fn enabled_mods_addons_cached() -> String {
    super::report_background::enabled_mods_addons_cached()
}

/// Scans the enabled mods/addons for startup, export, or the background cache.
pub fn enabled_mods_addons_string() -> String {
    #[cfg(test)]
    MOD_SCANS.set(MOD_SCANS.get() + 1);
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

static SAVED_LOGIN_CACHE: Mutex<Option<(Vec<String>, Vec<String>)>> = Mutex::new(None);
static SETTINGS_READ_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn settings_read_count() -> usize {
    SETTINGS_READ_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn reset_settings_read_count() {
    SETTINGS_READ_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
}

pub fn reset_saved_login_cache_for_test() {
    let mut guard = SAVED_LOGIN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
}

/// Helper to read saved login credentials from settings/disk, tracking read count for testing.
pub fn read_saved_login_from_disk() -> (Option<String>, Option<String>) {
    SETTINGS_READ_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (saved_user, saved_pass, _) = crate::settings::saved_login();
    let user = if !saved_user.trim().is_empty() {
        Some(saved_user.trim().to_string())
    } else {
        None
    };
    let pass = if saved_pass.len() >= 4 {
        Some(saved_pass)
    } else {
        None
    };
    (user, pass)
}

/// Warms the in-memory cache if not already populated.
/// Must be called outside the LOG lock.
pub fn warm_saved_login_cache() {
    let mut guard = SAVED_LOGIN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let (user, pass) = read_saved_login_from_disk();
        let mut accounts = Vec::new();
        let mut passwords = Vec::new();
        if let Some(u) = user {
            accounts.push(u);
        }
        if let Some(p) = pass {
            passwords.push(p);
        }
        *guard = Some((accounts, passwords));
    }
}

/// Refreshes the in-memory cache from settings/disk (e.g. when report::secret runs).
/// Must be called outside the LOG lock.
pub fn refresh_saved_login_cache() {
    let (user, pass) = read_saved_login_from_disk();
    let mut accounts = Vec::new();
    let mut passwords = Vec::new();
    if let Some(u) = user {
        accounts.push(u);
    }
    if let Some(p) = pass {
        passwords.push(p);
    }
    let mut guard = SAVED_LOGIN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some((accounts, passwords));
}

/// Directly updates the in-memory cache with specified credentials without reading disk (e.g. when save_login runs).
pub fn update_saved_login_cache(user: &str, password: &str, on: bool) {
    let mut guard = SAVED_LOGIN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let mut accounts = Vec::new();
    let mut passwords = Vec::new();
    if on && !user.trim().is_empty() {
        accounts.push(user.trim().to_string());
    }
    if on && password.len() >= 4 {
        passwords.push(password.to_string());
    }
    *guard = Some((accounts, passwords));
}

/// Helper to get remembered login account and password from the in-memory cache.
pub fn saved_login_secrets() -> (Option<String>, Option<String>) {
    let guard = SAVED_LOGIN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((accounts, passwords)) = guard.as_ref() {
        (accounts.first().cloned(), passwords.first().cloned())
    } else {
        drop(guard);
        warm_saved_login_cache();
        let guard = SAVED_LOGIN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map_or((None, None), |(a, p)| {
            (a.first().cloned(), p.first().cloned())
        })
    }
}

/// Returns true if at least one verified account is known to scrub against
/// (either in NOTED from a login/account action, or stored in settings).
pub fn has_verified_account() -> bool {
    let (saved_user, _) = saved_login_secrets();
    if saved_user.is_some() {
        return true;
    }
    crate::report::has_noted_accounts()
}

/// Scrubs text using NOTED secrets, client settings' remembered login, and home directories.
pub fn clean_text(text: &str) -> String {
    clean_text_with_homes(text, &[])
}

/// Scrubs text with optional additional home directories for test seams without env vars.
pub fn clean_text_with_homes(text: &str, extra_homes: &[&str]) -> String {
    // 1. Upstream clean_noted scrubs NOTED accounts, passwords, and env HOME/USERPROFILE
    let mut cleaned = crate::report::clean_noted(text);

    // 2. Extra test home paths if provided
    for h in extra_homes {
        if h.len() > 1 {
            cleaned = cleaned.replace(h, "~");
        }
    }

    // 3. Client settings remembered login if available
    let (saved_user, saved_pass) = saved_login_secrets();
    let mut accounts = Vec::new();
    let mut passwords = Vec::new();
    if let Some(u) = saved_user {
        accounts.push(u);
    }
    if let Some(p) = saved_pass {
        passwords.push(p);
    }
    if !accounts.is_empty() || !passwords.is_empty() {
        cleaned = crate::report::scrub(&cleaned, &accounts, &passwords);
    }

    cleaned
}

/// Scrub at log-write time: every line pushed into the log file on disk and ring buffer is pre-scrubbed.
/// CRITICAL: Runs inside push_line while holding LOG lock. Must NEVER touch disk or take LOG.
pub fn clean_log_line(line: &str) -> String {
    // 1. Scrub NOTED secrets and home directories in memory
    let cleaned = crate::report::clean_noted(line);

    // 2. Scrub cached saved login secrets without touching disk
    if let Ok(guard) = SAVED_LOGIN_CACHE.try_lock() {
        if let Some((accounts, passwords)) = guard.as_ref() {
            if !accounts.is_empty() || !passwords.is_empty() {
                return crate::report::scrub(&cleaned, accounts, passwords);
            }
        }
    }

    cleaned
}

/// Formats civil date and time as YYYYMMDD-HHMMSS (Howard Hinnant's algorithm).
pub(crate) fn stamp(t: SystemTime) -> String {
    let s = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, secs) = (s / 86400, s % 86400);
    let (h, min, sec) = (secs / 3600, (secs % 3600) / 60, secs % 60);
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
    format!("{y:04}{m:02}{d:02}-{h:02}{min:02}{sec:02}")
}

/// Resolves an unused ZIP filename by adding -2, -3 suffixes on collision.
pub(crate) fn unique_report_path(dir: &Path, stamp_str: &str) -> PathBuf {
    let base = format!("EQOA-Enhanced-report-{stamp_str}");
    let candidate = dir.join(format!("{base}.zip"));
    if !candidate.exists() {
        return candidate;
    }
    let mut i = 2;
    loop {
        let candidate = dir.join(format!("{base}-{i}.zip"));
        if !candidate.exists() {
            return candidate;
        }
        i += 1;
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
        let shell32 =
            windows_sys::Win32::System::LibraryLoader::LoadLibraryA(b"shell32.dll\0".as_ptr());
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
                if sh_get_known_folder_path(&folder_id, 0, std::ptr::null_mut(), &mut path_ptr) == 0
                    && !path_ptr.is_null()
                {
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
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
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
pub fn dialog_labels(kind: &Kind, sent: bool, several: bool) -> Option<Vec<(String, Button)>> {
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
                Some(format!(
                    "Saved {} reports to {}. Thank you.",
                    paths.len(),
                    paths[0]
                ))
            }
        }
        _ => None,
    }
}

/// Dialog fine notice for Enhanced. Plainly states that the zip includes a screenshot.
pub fn dialog_fine(kind: &Kind) -> Option<&'static str> {
    match kind {
        Kind::Problem => Some(
            "The saved report zip includes any note and screenshot you choose (which may show character names), available logs, client state, and a manifest with the Enhanced version and enabled mods. Nothing is uploaded anywhere.",
        ),
        Kind::Crash(_) => Some(
            "The saved report zip includes any note you enter, available crash logs, client state, and a manifest with the Enhanced version and enabled mods. Nothing is uploaded anywhere.",
        ),
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
#[allow(dead_code)]
pub fn save_report(
    kind: &Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
) -> Result<(PathBuf, bool), String> {
    save_report_to(
        kind,
        note,
        name,
        shot,
        desktop_dir().as_deref(),
        config_dir().as_deref(),
        true,
    )
}

/// Saves the report with explicit desktop and config directory test seams (no global env needed).
pub fn save_report_to(
    kind: &Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
    desktop: Option<&Path>,
    cfg_dir: Option<&Path>,
    reveal: bool,
) -> Result<(PathBuf, bool), String> {
    let stamp_str = stamp(SystemTime::now());

    let mut entries = Vec::new();
    let mut files_list = Vec::new();

    // 1. note.txt
    let clean_note = clean_text(note);
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
    if let Some(d) = cfg_dir {
        let log_path = d.join("eqoa.log");
        if log_path.exists() {
            if let Ok(content) = fs::read_to_string(&log_path) {
                entries.push(ZipEntry {
                    name: "eqoa.log".into(),
                    data: clean_text(&content).into_bytes(),
                });
                files_list.push("eqoa.log");
            }
        }
    }

    // 4. eqoa.prev.log (only included if a verified account is known to scrub against;
    // pre-login/unverified logs are omitted to protect privacy, as documented in manifest)
    if let Some(d) = cfg_dir {
        let prev_path = d.join("eqoa.prev.log");
        if prev_path.exists() {
            if has_verified_account() {
                if let Ok(content) = fs::read_to_string(&prev_path) {
                    entries.push(ZipEntry {
                        name: "eqoa.prev.log".into(),
                        data: clean_text(&content).into_bytes(),
                    });
                    files_list.push("eqoa.prev.log");
                }
            } else {
                files_list.push(
                    "eqoa.prev.log (omitted: pre-login state with no verified account to scrub)",
                );
            }
        }
    }

    // Last noted snapshot belongs to this window and is scrubbed again at export.
    if let Some(d) = cfg_dir {
        if let Ok(content) = fs::read_to_string(d.join(crate::report::state_file())) {
            entries.push(ZipEntry {
                name: "client_state.txt".into(),
                data: clean_text(&content).into_bytes(),
            });
            files_list.push("client_state.txt");
        }
    }

    // 5. mods/addons_error.log (if present)
    let addons_error_path = crate::fork_ext::mods::mods_dir().join("addons_error.log");
    if addons_error_path.exists() {
        if let Ok(content) = fs::read_to_string(&addons_error_path) {
            entries.push(ZipEntry {
                name: "addons_error.log".into(),
                data: clean_text(&content).into_bytes(),
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
                        data: clean_text(&content).into_bytes(),
                    });
                    files_list.push("crash report");
                }
            }
        }
        Kind::Problem => {
            let crashes_dir = cfg_dir.map(|d| d.join("crashes"));
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
                                    data: clean_text(&content).into_bytes(),
                                });
                                files_list.push("waiting crash report");
                            }
                        }
                    }
                }
            }
        }
    }

    // 7. manifest.txt (with the required labelling and header)
    let header = enhanced_label();
    let mods_addons = enabled_mods_addons_string();
    let manifest_text = format!(
        "{header}\nEnabled mods/addons: {mods_addons}\nTimestamp: {stamp_str}\nType: {}\nName: {}\nNote: {}\n\nFiles included:\n{}\n",
        match kind {
            Kind::Problem => "Problem report",
            Kind::Crash(_) => "Crash report",
        },
        if name.trim().is_empty() {
            "-"
        } else {
            name.trim()
        },
        if clean_note.trim().is_empty() {
            "-"
        } else {
            clean_note.trim()
        },
        if files_list.is_empty() {
            "- (manifest only)".into()
        } else {
            files_list.join("\n")
        }
    );
    let clean_manifest = clean_text(&manifest_text);
    entries.insert(
        0,
        ZipEntry {
            name: "manifest.txt".into(),
            data: clean_manifest.into_bytes(),
        },
    );

    let zip_bytes =
        write_zip(&entries).map_err(|e| format!("Failed to compress report zip: {e}"))?;

    // Determine target folder: try Desktop, fall back to config directory.
    // Use collision suffixes (-2, -3) to never overwrite an existing report.
    let mut fell_back = false;
    let final_path = if let Some(d) = desktop {
        let dest = unique_report_path(d, &stamp_str);
        match fs::create_dir_all(d).and_then(|_| fs::write(&dest, &zip_bytes)) {
            Ok(()) => dest,
            Err(_) => {
                fell_back = true;
                let c = cfg_dir.ok_or("Both Desktop and config directory are unavailable")?;
                fs::create_dir_all(c).map_err(|e| e.to_string())?;
                let dest = unique_report_path(c, &stamp_str);
                fs::write(&dest, &zip_bytes).map_err(|e| e.to_string())?;
                dest
            }
        }
    } else {
        fell_back = true;
        let c = cfg_dir.ok_or("No Desktop or config directory available")?;
        fs::create_dir_all(c).map_err(|e| e.to_string())?;
        let dest = unique_report_path(c, &stamp_str);
        fs::write(&dest, &zip_bytes).map_err(|e| e.to_string())?;
        dest
    };

    if reveal {
        reveal_in_file_manager(&final_path);
    }
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
    #[cfg(test)]
    if let Some((desktop, config)) = TEST_DESTINATION.with(|d| d.borrow().clone()) {
        return handle_send_to(
            kind,
            note,
            name,
            shot,
            result,
            Some(&desktop),
            Some(&config),
            false,
        );
    }
    handle_send_to(
        kind,
        note,
        name,
        shot,
        result,
        desktop_dir().as_deref(),
        config_dir().as_deref(),
        true,
    )
}

/// Save worker with path seams for testability without process-global environment mutation.
pub fn handle_send_to(
    kind: &Kind,
    note: &str,
    name: &str,
    shot: Option<&[u8]>,
    result: &Arc<Mutex<Option<Sent>>>,
    desktop: Option<&Path>,
    config: Option<&Path>,
    reveal: bool,
) -> bool {
    let kind = kind.clone();
    let note = note.to_string();
    let name = name.to_string();
    let shot = shot.map(|s| s.to_vec());
    let thread_result = result.clone();
    let desktop = desktop.map(PathBuf::from);
    let config = config.map(PathBuf::from);

    let spawn_res = std::thread::Builder::new()
        .name("report".into())
        .spawn(move || {
            let outcome = save_report_to(
                &kind,
                &note,
                &name,
                shot.as_deref(),
                desktop.as_deref(),
                config.as_deref(),
                reveal,
            );
            match outcome {
                Ok((path, fell_back)) => {
                    let path_str = if fell_back {
                        format!(
                            "{} (saved to config directory; Desktop unavailable)",
                            path.display()
                        )
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
                            *thread_result.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(Sent::success(sent_list));
                        }
                        Kind::Problem => {
                            *thread_result.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(Sent::success(vec![(None, path_str)]));
                        }
                    }
                }
                Err(err) => {
                    crate::say(&format!("report: failed to save: {err}"));
                    *thread_result.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(Sent::failure(err));
                }
            }
        });

    if let Err(err) = spawn_res {
        crate::say(&format!("report: failed to spawn worker thread: {err}"));
        *result.lock().unwrap_or_else(|e| e.into_inner()) = Some(Sent::failure(format!(
            "failed to spawn background thread: {err}"
        )));
    }

    true
}

#[cfg(test)]
pub(crate) mod tests {
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
        let d =
            std::env::temp_dir().join(format!("eqoa-test-{prefix}-{}-{id}", std::process::id()));
        let _ = fs::create_dir_all(&d);
        TestDir(d)
    }

    // Helper to unzip test files
    pub(crate) fn extract_zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
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

        // Ensure verified account is present so eqoa.prev.log is included
        crate::report::secret("VerifiedTester", "TesterPass123");

        let fake_shot = b"\x89PNG\r\n\x1a\nfake screenshot data";
        let note = "Test problem note describing an issue";
        let name = "Tester";

        let (saved_path, fell_back) = save_report_to(
            &Kind::Problem,
            note,
            name,
            Some(fake_shot),
            Some(&fake_desktop),
            Some(&fake_config),
            false,
        )
        .expect("save_report_to failed");

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

        assert!(
            entry_names.contains(&"manifest.txt"),
            "Missing manifest.txt"
        );
        assert!(entry_names.contains(&"note.txt"), "Missing note.txt");
        assert!(
            entry_names.contains(&"screenshot.png"),
            "Missing screenshot.png"
        );
        assert!(entry_names.contains(&"eqoa.log"), "Missing eqoa.log");
        assert!(
            entry_names.contains(&"eqoa.prev.log"),
            "Missing eqoa.prev.log"
        );
        assert!(
            entry_names.contains(&"client_state.txt"),
            "missing client snapshot"
        );
        let mut expected = vec![
            "manifest.txt",
            "note.txt",
            "screenshot.png",
            "eqoa.log",
            "eqoa.prev.log",
            "client_state.txt",
            "crashes/crash-20261026-100000.txt",
        ];
        if crate::fork_ext::mods::mods_dir()
            .join("addons_error.log")
            .exists()
        {
            expected.push("addons_error.log");
        }
        let mut actual = entry_names.clone();
        actual.sort();
        expected.sort();
        assert_eq!(
            actual, expected,
            "ZIP contains exactly the promised note, screenshot, available logs, client state and manifest"
        );
        let promise = dialog_fine(&Kind::Problem).unwrap();
        for item in [
            "note",
            "screenshot",
            "logs",
            "manifest",
            "Enhanced version",
            "enabled mods",
            "client state",
        ] {
            assert!(promise.contains(item), "missing promise: {item}");
        }
        assert!(promise.contains("client state"));
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
        let temp_dir = test_dir("report_secrets");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        let fake_account = "SuperSecretAccount42";
        let fake_password = "SecretPassword999";
        let fake_home = "C:\\Users\\SecretHomeDirectory";

        crate::report::secret(fake_account, fake_password);

        fs::write(
            fake_config.join("eqoa.log"),
            format!("logged account {fake_account} with pass {fake_password} on {fake_home}\n"),
        )
        .unwrap();

        let note = format!("I had {fake_account} and {fake_password} in {fake_home}");
        let (saved_path, _) = save_report_to(
            &Kind::Problem,
            &note,
            fake_account,
            None,
            Some(&fake_desktop),
            Some(&fake_config),
            false,
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
        }
    }

    /// Exercises process-global USERPROFILE environment variable reading for home directory scrubbing.
    /// Requires --test-threads=1 to prevent race conditions with parallel tests reading USERPROFILE.
    #[test]
    fn secrets_scrubbed_with_env_userprofile_needs_test_threads_1() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = test_dir("report_env_userprofile");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        let fake_home = "C:\\Users\\EnvScrubbedHome42";
        unsafe {
            std::env::set_var("USERPROFILE", fake_home);
        }

        let note = format!("saved in {fake_home}\\data");
        let (saved_path, _) = save_report_to(
            &Kind::Problem,
            &note,
            "tester",
            None,
            Some(&fake_desktop),
            Some(&fake_config),
            false,
        )
        .unwrap();

        let zip_bytes = fs::read(&saved_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);
        let note_entry = entries.iter().find(|(n, _)| n == "note.txt").unwrap();
        let text = String::from_utf8_lossy(&note_entry.1);
        assert!(
            !text.contains(fake_home),
            "Leaked USERPROFILE path in note.txt"
        );
        assert!(text.contains("~\\data"), "Expected ~ replacement: {text}");
    }

    #[test]
    fn upload_url_refuses_remote_endpoints_even_when_report_policy_enabled() {
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
        let temp_dir = test_dir("report_crash_offer");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        let crash_file = fake_config
            .join("crashes")
            .join("crash-20261026-123456.txt");
        fs::create_dir_all(crash_file.parent().unwrap()).unwrap();
        fs::write(&crash_file, "fatal crash log content").unwrap();

        let kind = Kind::Crash(vec![crash_file.clone()]);
        let result = Arc::new(Mutex::new(None));
        let handled = handle_send_to(
            &kind,
            "crash note",
            "crash reporter",
            None,
            &result,
            Some(&fake_desktop),
            Some(&fake_config),
            false,
        );
        assert!(handled, "handle_send_to must return true for crash kind");

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
        assert!(
            !crash_file.exists(),
            "Crash file should be cleaned up after saving"
        );
    }

    #[test]
    fn unwritable_desktop_falls_back_to_config_directory() {
        let temp_dir = test_dir("report_fallback");
        let config_dir = temp_dir.path().join("Config");
        fs::create_dir_all(&config_dir).unwrap();

        // Make desktop a file so create_dir_all or write inside it fails
        let blocker = temp_dir.path().join("blocker_file");
        fs::write(&blocker, "blocking file").unwrap();
        let invalid_desktop = blocker.join("desktop_sub");

        let (saved_path, fell_back) = save_report_to(
            &Kind::Problem,
            "Test fallback",
            "",
            None,
            Some(&invalid_desktop),
            Some(&config_dir),
            false,
        )
        .unwrap();

        assert!(fell_back, "Expected fallback to config directory");
        assert_eq!(saved_path.parent().unwrap(), config_dir);
        assert!(saved_path.exists());
    }

    #[test]
    fn stamp_uses_correct_minutes_not_month() {
        // 2026-10-31 18:57:45 UTC
        // days = 20757, secs = 68265 -> 18:57:45
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1793473065);
        let s = stamp(t);
        assert_eq!(s, "20261031-185745");
    }

    #[test]
    fn save_report_collision_suffixes_existing_zip() {
        let temp_dir = test_dir("report_collision");
        let desktop = temp_dir.path().join("Desktop");
        let config = temp_dir.path().join("Config");
        fs::create_dir_all(&desktop).unwrap();
        fs::create_dir_all(&config).unwrap();

        // The suffix rule, deterministic: a fixed stamp, so a second boundary cannot flake it.
        let stamp_str = "20261101-120000";
        let first_file = desktop.join(format!("EQOA-Enhanced-report-{stamp_str}.zip"));
        fs::write(&first_file, "existing report 1").unwrap();
        let second = unique_report_path(&desktop, stamp_str);
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            format!("EQOA-Enhanced-report-{stamp_str}-2.zip")
        );
        fs::write(&second, "existing report 2").unwrap();
        let third = unique_report_path(&desktop, stamp_str);
        assert_eq!(
            third.file_name().unwrap().to_string_lossy(),
            format!("EQOA-Enhanced-report-{stamp_str}-3.zip")
        );

        // End to end: saving next to existing reports never overwrites one.
        let now_stamp = stamp(SystemTime::now());
        let now_file = desktop.join(format!("EQOA-Enhanced-report-{now_stamp}.zip"));
        fs::write(&now_file, "existing report now").unwrap();
        let (saved, _) = save_report_to(
            &Kind::Problem,
            "another report",
            "tester",
            None,
            Some(&desktop),
            Some(&config),
            false,
        )
        .unwrap();
        assert_ne!(
            saved, now_file,
            "an existing report must never be the save target"
        );
        assert_eq!(
            fs::read_to_string(&now_file).unwrap(),
            "existing report now"
        );
        assert_eq!(
            fs::read_to_string(&first_file).unwrap(),
            "existing report 1"
        );
        assert_eq!(fs::read_to_string(&second).unwrap(), "existing report 2");
    }

    #[test]
    fn regression_noted_empty_with_saved_login_scrubs_account_from_previous_log() {
        let temp_dir = test_dir("noted_empty_saved_login");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        let remembered_account = "RememberedUser99";
        let remembered_password = "SecretPassWord77";
        crate::settings::save_login(remembered_account, remembered_password, true);

        fs::write(
            fake_config.join("eqoa.prev.log"),
            format!(
                "auth: account {remembered_account} status 1 with pass {remembered_password}\n"
            ),
        )
        .unwrap();

        // NOTED is empty (no secret(...) call made in current run)
        let (saved_path, _) = save_report_to(
            &Kind::Problem,
            "note",
            "reporter",
            None,
            Some(&fake_desktop),
            Some(&fake_config),
            false,
        )
        .unwrap();

        let zip_bytes = fs::read(&saved_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);
        let prev_entry = entries
            .iter()
            .find(|(n, _)| n == "eqoa.prev.log")
            .expect("eqoa.prev.log should be included when saved login is verified");
        let text = String::from_utf8_lossy(&prev_entry.1);

        assert!(
            !text.contains(remembered_account),
            "File eqoa.prev.log contains leaked account {remembered_account}"
        );
        assert!(
            !text.contains(remembered_password),
            "File eqoa.prev.log contains leaked password"
        );
        assert!(text.contains("<account>"), "Expected <account> replacement");
    }

    #[test]
    fn regression_noted_empty_without_verified_account_omits_previous_log() {
        let temp_dir = test_dir("noted_empty_no_account");
        let fake_desktop = temp_dir.path().join("Desktop");
        let fake_config = temp_dir.path().join("Config");
        fs::create_dir_all(&fake_desktop).unwrap();
        fs::create_dir_all(&fake_config).unwrap();

        // Clear saved login so no verified account exists
        crate::settings::save_login("", "", false);

        let unverified_account = "UnverifiedSecretAcc";
        fs::write(
            fake_config.join("eqoa.prev.log"),
            format!("login {unverified_account} ok\n"),
        )
        .unwrap();

        let (saved_path, _) = save_report_to(
            &Kind::Problem,
            "note",
            "reporter",
            None,
            Some(&fake_desktop),
            Some(&fake_config),
            false,
        )
        .unwrap();

        let zip_bytes = fs::read(&saved_path).unwrap();
        let entries = extract_zip_entries(&zip_bytes);

        // eqoa.prev.log must NOT be included in the zip
        assert!(
            !entries.iter().any(|(n, _)| n == "eqoa.prev.log"),
            "eqoa.prev.log must be omitted when no verified account exists"
        );

        // Manifest must document the omission
        let manifest_bytes = &entries.iter().find(|(n, _)| n == "manifest.txt").unwrap().1;
        let manifest_str = String::from_utf8_lossy(manifest_bytes);
        assert!(
            manifest_str.contains(
                "eqoa.prev.log (omitted: pre-login state with no verified account to scrub)"
            ),
            "Manifest must state why eqoa.prev.log was omitted: {manifest_str}"
        );

        // Entire zip must be free of the unverified account
        for (name, data) in entries {
            let text = String::from_utf8_lossy(&data);
            assert!(
                !text.contains(unverified_account),
                "File {name} in zip contains unverified account {unverified_account}"
            );
        }
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
        assert!(
            crash_file.exists(),
            "crash report must be written when [eqoa] run is first line"
        );
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
        assert!(
            !crash_file.exists(),
            "negative control: crash report must NOT be written when header is placed first"
        );
    }

    #[test]
    fn logs_begin_with_the_enhanced_line() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        struct CloseLog;
        impl Drop for CloseLog {
            fn drop(&mut self) {
                crate::logfile::close();
            }
        }
        let _log = CloseLog;
        crate::logfile::start();
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
            enhanced_line.starts_with("EQOA Enhanced ")
                || enhanced_line.starts_with("[eqoa] EQOA Enhanced "),
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

        let labels_crash_multi =
            dialog_labels(&Kind::Crash(vec!["a".into(), "b".into()]), false, true).unwrap();
        assert_eq!(labels_crash_multi[0].0, "Save all to Desktop (Enter)");

        let status_single =
            dialog_status(&Status::Sent(vec!["C:/path/report.zip".into()]), 0).unwrap();
        assert_eq!(status_single, "Saved to C:/path/report.zip. Thank you.");

        let status_multi = dialog_status(
            &Status::Sent(vec![
                "C:/path/report.zip".into(),
                "C:/path/report.zip".into(),
                "C:/path/report.zip".into(),
            ]),
            0,
        )
        .unwrap();
        assert_eq!(
            status_multi,
            "Saved 3 reports to C:/path/report.zip. Thank you."
        );
    }

    #[test]
    fn settings_reads_across_1000_log_lines_at_most_one() {
        crate::fork_ext::report::reset_saved_login_cache_for_test();
        crate::fork_ext::report::reset_settings_read_count();

        // Warm up once when logging starts (as report::start does outside LOG lock)
        crate::fork_ext::report::warm_saved_login_cache();

        // 1000 log lines pushed through clean_log_line while holding LOG lock
        for i in 0..1000 {
            let line = format!("event {i}: test log line content");
            let _ = crate::fork_ext::report::clean_log_line(&line);
        }

        let total_reads = crate::fork_ext::report::settings_read_count();
        assert_eq!(
            total_reads, 1,
            "Expected exactly 1 settings read across warm-up + 1000 log lines, but got {total_reads}"
        );
    }

    #[test]
    fn dialog_fine_plainly_states_screenshot_included() {
        let fine_problem = dialog_fine(&Kind::Problem).unwrap();
        assert!(
            fine_problem.contains("screenshot"),
            "Fine text for problem must state screenshot is included"
        );
        assert!(
            fine_problem.contains("character names"),
            "Fine text must mention character names"
        );
        assert!(
            fine_problem.contains("Nothing is uploaded"),
            "Fine text must state nothing is uploaded"
        );

        let fine_crash = dialog_fine(&Kind::Crash(vec!["a".into()])).unwrap();
        assert!(fine_crash.contains("crash logs"));
        assert!(fine_crash.contains("manifest") && fine_crash.contains("client state"));
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_DESTINATION: std::cell::RefCell<Option<(PathBuf, PathBuf)>> = const { std::cell::RefCell::new(None) };
}
