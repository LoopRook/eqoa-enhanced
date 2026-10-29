//! Windows MCI-backed sound bank for UiForge compatibility.
//!
//! Implements `UiForge.LoadSound`, `PlaySound`, `StopSound`, `IsSoundPlaying`,
//! `SetSoundVolume`, and `ReleaseSound` matching the pinned FrontiersForge C++
//! reference (`audio_manager.cpp` / `core.cpp`). Sound playback is routed through
//! the Windows Media Control Interface (`winmm.dll`) independently of the shared
//! `snd`/audio stack.
//!
//! # Platform boundary
//! Non-Windows platforms return `nil` with an explicit unsupported diagnostic.
//! Production never fakes playback success.
//!
//! # Lifecycle
//! Each addon instance owns one `SoundBank`. Banks start inactive and are only
//! activated while the instance is enabled/running. Ordinary disable stops
//! playback but preserves handles; terminal events close all aliases.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Lua, Result as LuaResult, Table};

use super::discovery::DiscoveredAddon;

/// Maximum file size permitted for a single sound asset (16 MiB).
pub const MAX_SOUND_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// Maximum number of open sound handles per addon bank.
pub const MAX_SOUND_HANDLES: usize = 16;

/// Maximum aggregate size of loaded sound assets per addon bank (64 MiB).
pub const MAX_AGGREGATE_BYTES: u64 = 64 * 1024 * 1024;

/// Maximum positive sound ID in signed 32-bit range matching C++ `int sound_id`.
pub const MAX_SOUND_ID_U32: u32 = i32::MAX as u32;

/// Maximum retry attempts for failed MCI close commands before terminal Drop.
pub const MAX_CLOSE_RETRIES: u32 = 3;

const MAX_DIAGNOSTICS: usize = 32;

static NEXT_BANK_OWNER_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_bank_owner_id() -> Result<u64, SoundDiagnostic> {
    NEXT_BANK_OWNER_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| SoundDiagnostic::IdExhausted("bank owner token exhausted"))
}

/// Checked positive sound identifier corresponding to FrontiersForge `int sound_id`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct SoundId(u32);

impl SoundId {
    /// Creates a new checked `SoundId` within the positive `i32` range.
    pub fn new(id: u32) -> Result<Self, SoundDiagnostic> {
        if id == 0 || id > MAX_SOUND_ID_U32 {
            Err(SoundDiagnostic::InvalidSoundId(id))
        } else {
            Ok(Self(id))
        }
    }

    /// Returns the positive integer identifier as `u32`.
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Returns the positive integer identifier as signed `i32` matching C++ `int`.
    pub fn as_i32(self) -> i32 {
        self.0 as i32
    }
}

impl fmt::Display for SoundId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Playback options accepted by `UiForge.PlaySound`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayOptions {
    pub volume: f32,
    pub looping: bool,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looping: false,
        }
    }
}

/// Validated volume: finite values are clamped to `[0.0, 1.0]`; non-finite values are rejected.
pub fn validate_volume(volume: f32) -> Result<f32, SoundDiagnostic> {
    if !volume.is_finite() {
        return Err(SoundDiagnostic::InvalidVolume(volume));
    }
    Ok(volume.clamp(0.0, 1.0))
}

/// Diagnostic outcome from a sound operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SoundDiagnostic {
    UnsupportedPlatform(&'static str),
    PathSanitizationFailed(&'static str),
    FileNotFound(PathBuf),
    FileTooLarge {
        size: u64,
        max: u64,
    },
    HandleLimitExceeded {
        limit: usize,
    },
    AggregateLimitExceeded {
        current: u64,
        adding: u64,
        max: u64,
    },
    IdExhausted(&'static str),
    InvalidVolume(f32),
    MciCommandFailed {
        command: String,
        error: String,
    },
    MciCloseFailed {
        alias: String,
        attempts: u32,
        error: String,
    },
    InactiveAddon,
    InvalidSoundId(u32),
}

impl fmt::Display for SoundDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform(p) => {
                write!(f, "sound playback is unsupported on platform '{p}'")
            }
            Self::PathSanitizationFailed(reason) => {
                write!(f, "sound path rejected for security: {reason}")
            }
            Self::FileNotFound(path) => write!(f, "sound file not found: {}", path.display()),
            Self::FileTooLarge { size, max } => {
                write!(f, "sound file size {size} exceeds limit {max}")
            }
            Self::HandleLimitExceeded { limit } => write!(f, "sound handle limit {limit} exceeded"),
            Self::AggregateLimitExceeded {
                current,
                adding,
                max,
            } => {
                write!(
                    f,
                    "aggregate sound size {current}+{adding} exceeds limit {max}"
                )
            }
            Self::IdExhausted(reason) => write!(f, "identifier exhausted: {reason}"),
            Self::InvalidVolume(v) => write!(f, "invalid non-finite volume (NaN/Inf): {v}"),
            Self::MciCommandFailed { command, error } => {
                write!(f, "MCI command \"{command}\" failed: {error}")
            }
            Self::MciCloseFailed {
                alias,
                attempts,
                error,
            } => {
                write!(
                    f,
                    "MCI close on \"{alias}\" failed (attempt {attempts}): {error}"
                )
            }
            Self::InactiveAddon => write!(f, "addon is currently inactive/disabled"),
            Self::InvalidSoundId(id) => write!(f, "invalid or unowned sound handle: {id}"),
        }
    }
}

/// Abstraction over MCI command transport. Production uses Windows MCI; tests
/// can inject a recorder to prove command syntax and fault behavior without
/// touching the OS.
pub(crate) trait MciBackend {
    fn send(&mut self, command: &str) -> Result<String, String>;
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Native Windows MCI backend using `mciSendStringW`.
#[cfg(windows)]
struct WindowsMciBackend;

#[cfg(windows)]
#[link(name = "winmm")]
unsafe extern "system" {
    fn mciSendStringW(
        lpstrCommand: *const u16,
        lpstrReturnString: *mut u16,
        uReturnLength: u32,
        hwndCallback: *mut core::ffi::c_void,
    ) -> u32;
    fn mciGetErrorStringW(fdwError: u32, lpszErrorText: *mut u16, cchErrorText: u32) -> u32;
}

#[cfg(windows)]
impl WindowsMciBackend {
    fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl MciBackend for WindowsMciBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn send(&mut self, command: &str) -> Result<String, String> {
        let wide: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
        let mut return_buffer = [0u16; 128];
        let error_code = unsafe {
            mciSendStringW(
                wide.as_ptr(),
                return_buffer.as_mut_ptr(),
                return_buffer.len() as u32,
                std::ptr::null_mut(),
            )
        };
        if error_code != 0 {
            let mut error_buffer = [0u16; 256];
            let ok = unsafe {
                mciGetErrorStringW(
                    error_code,
                    error_buffer.as_mut_ptr(),
                    error_buffer.len() as u32,
                )
            };
            let text = if ok != 0 {
                let len = error_buffer
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(error_buffer.len());
                String::from_utf16_lossy(&error_buffer[..len])
            } else {
                format!("MCI error code {error_code}")
            };
            return Err(text);
        }
        let len = return_buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(return_buffer.len());
        Ok(String::from_utf16_lossy(&return_buffer[..len]))
    }
}

/// Non-Windows backend that always fails. It is never used for production
/// playback because `load_sound` short-circuits with `UnsupportedPlatform`.
#[cfg(not(windows))]
struct UnsupportedBackend;

#[cfg(not(windows))]
impl MciBackend for UnsupportedBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn send(&mut self, _command: &str) -> Result<String, String> {
        Err("sound playback is unsupported on non-Windows platforms".into())
    }
}

fn default_backend() -> Box<dyn MciBackend> {
    #[cfg(windows)]
    {
        Box::new(WindowsMciBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(UnsupportedBackend)
    }
}

struct SoundRecord {
    id: SoundId,
    alias: String,
    canonical_path: PathBuf,
    file_size: u64,
    close_attempts: u32,
}

/// Per-addon sound bank managing MCI aliases, lifecycle, and resource bounds.
pub struct SoundBank {
    owner_id: u64,
    active: bool,
    next_sound_id: u32,
    sounds: HashMap<SoundId, SoundRecord>,
    sounds_by_path: HashMap<PathBuf, SoundId>,
    aggregate_bytes: u64,
    last_diagnostic: Option<SoundDiagnostic>,
    diagnostics: Vec<String>,
    backend: Box<dyn MciBackend>,
}

impl SoundBank {
    /// Creates a new inactive sound bank using a monotonic owner token and the
    /// platform MCI backend.
    pub fn new() -> Result<Self, SoundDiagnostic> {
        let owner_id = allocate_bank_owner_id()?;
        Ok(Self::with_owner_id_and_backend(owner_id, default_backend()))
    }

    fn with_owner_id_and_backend(owner_id: u64, backend: Box<dyn MciBackend>) -> Self {
        Self {
            owner_id,
            active: false,
            next_sound_id: 1,
            sounds: HashMap::new(),
            sounds_by_path: HashMap::new(),
            aggregate_bytes: 0,
            last_diagnostic: None,
            diagnostics: Vec::new(),
            backend,
        }
    }

    /// Test-only constructor using the platform backend.
    #[cfg(test)]
    pub(crate) fn with_owner_id(owner_id: u64) -> Self {
        Self::with_owner_id_and_backend(owner_id, default_backend())
    }

    /// Test-only constructor with an injected backend.
    #[cfg(test)]
    pub(crate) fn with_backend(owner_id: u64, backend: Box<dyn MciBackend>) -> Self {
        Self::with_owner_id_and_backend(owner_id, backend)
    }

    pub fn owner_id(&self) -> u64 {
        self.owner_id
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activates or deactivates the bank. Deactivation stops playback immediately
    /// but preserves handles so ordinary disable/enable cycles survive.
    pub fn set_active(&mut self, active: bool) {
        if self.active && !active {
            self.stop_all();
        }
        self.active = active;
    }

    pub fn last_diagnostic(&self) -> Option<&SoundDiagnostic> {
        self.last_diagnostic.as_ref()
    }

    pub fn clear_diagnostic(&mut self) {
        self.last_diagnostic = None;
    }

    pub fn len(&self) -> usize {
        self.sounds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }

    pub fn aggregate_bytes(&self) -> u64 {
        self.aggregate_bytes
    }

    /// Drains the bounded diagnostic ring.
    pub fn drain_diagnostics(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diagnostics)
    }

    #[cfg(test)]
    pub(crate) fn backend_log(&self) -> Vec<String> {
        self.backend
            .as_any()
            .downcast_ref::<RecorderBackend>()
            .map(|r| r.log())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn handles(&self) -> Vec<SoundId> {
        self.sounds.keys().copied().collect()
    }

    fn record(&mut self, diag: SoundDiagnostic) {
        self.last_diagnostic = Some(diag.clone());
        let text = diag.to_string();
        if self.diagnostics.contains(&text) {
            return;
        }
        if self.diagnostics.len() == MAX_DIAGNOSTICS {
            self.diagnostics.remove(0);
        }
        self.diagnostics.push(text);
    }

    /// Resolves, bounds-checks, and opens a sound file. Returns a positive handle
    /// on success or `None` on failure.
    pub fn load_sound(
        &mut self,
        path_str: &str,
        script_root: &Path,
        resource_dirs: &[PathBuf],
    ) -> Option<SoundId> {
        self.last_diagnostic = None;

        #[cfg(not(windows))]
        {
            let _ = (path_str, script_root, resource_dirs);
            self.record(SoundDiagnostic::UnsupportedPlatform(std::env::consts::OS));
            return None;
        }

        #[cfg(windows)]
        {
            let canonical = match resolve_sound_path(path_str, script_root, resource_dirs) {
                Ok(p) => p,
                Err(diag) => {
                    self.record(diag);
                    return None;
                }
            };

            if let Some(&existing) = self.sounds_by_path.get(&canonical) {
                return Some(existing);
            }

            if self.sounds.len() >= MAX_SOUND_HANDLES {
                self.record(SoundDiagnostic::HandleLimitExceeded {
                    limit: MAX_SOUND_HANDLES,
                });
                return None;
            }

            let meta = match fs::metadata(&canonical) {
                Ok(m) if m.is_file() => m,
                _ => {
                    self.record(SoundDiagnostic::FileNotFound(canonical));
                    return None;
                }
            };
            let file_size = meta.len();
            if file_size > MAX_SOUND_FILE_BYTES {
                self.record(SoundDiagnostic::FileTooLarge {
                    size: file_size,
                    max: MAX_SOUND_FILE_BYTES,
                });
                return None;
            }
            if self.aggregate_bytes.saturating_add(file_size) > MAX_AGGREGATE_BYTES {
                self.record(SoundDiagnostic::AggregateLimitExceeded {
                    current: self.aggregate_bytes,
                    adding: file_size,
                    max: MAX_AGGREGATE_BYTES,
                });
                return None;
            }

            if self.next_sound_id > MAX_SOUND_ID_U32 {
                self.record(SoundDiagnostic::IdExhausted("sound handle range exhausted"));
                return None;
            }
            let sound_id = match SoundId::new(self.next_sound_id) {
                Ok(id) => id,
                Err(diag) => {
                    self.record(diag);
                    return None;
                }
            };
            self.next_sound_id += 1;

            let alias = format!("uiforge_snd_{:08x}_{}", self.owner_id, sound_id.as_u32());
            let open_cmd = format!(
                "open \"{}\" type mpegvideo alias {}",
                canonical.display(),
                alias
            );
            if let Err(err) = self.backend.send(&open_cmd) {
                self.record(SoundDiagnostic::MciCommandFailed {
                    command: open_cmd,
                    error: err,
                });
                return None;
            }

            self.aggregate_bytes += file_size;
            self.sounds_by_path.insert(canonical.clone(), sound_id);
            self.sounds.insert(
                sound_id,
                SoundRecord {
                    id: sound_id,
                    alias,
                    canonical_path: canonical,
                    file_size,
                    close_attempts: 0,
                },
            );
            Some(sound_id)
        }
    }

    /// Plays a loaded sound from the beginning. Replaying a currently playing
    /// sound restarts it. Returns `false` when inactive, invalid, or the play
    /// command fails.
    pub fn play_sound(&mut self, sound_id: Option<SoundId>, options: Option<PlayOptions>) -> bool {
        self.last_diagnostic = None;
        let Some(id) = sound_id else {
            return false;
        };
        if !self.active {
            self.record(SoundDiagnostic::InactiveAddon);
            return false;
        }
        let opts = options.unwrap_or_default();
        let volume = match validate_volume(opts.volume) {
            Ok(v) => v,
            Err(diag) => {
                self.record(diag);
                return false;
            }
        };
        let alias = match self.sounds.get(&id) {
            Some(r) => r.alias.clone(),
            None => {
                self.record(SoundDiagnostic::InvalidSoundId(id.as_u32()));
                return false;
            }
        };

        let mci_vol = (volume * 1000.0) as u32;
        let set_cmd = format!("setaudio {alias} volume to {mci_vol}");
        if let Err(err) = self.backend.send(&set_cmd) {
            self.record(SoundDiagnostic::MciCommandFailed {
                command: set_cmd,
                error: err,
            });
        }

        let seek_cmd = format!("seek {alias} to start");
        if let Err(err) = self.backend.send(&seek_cmd) {
            self.record(SoundDiagnostic::MciCommandFailed {
                command: seek_cmd,
                error: err,
            });
        }

        let play_cmd = format!("play {alias}{}", if opts.looping { " repeat" } else { "" });
        match self.backend.send(&play_cmd) {
            Ok(_) => true,
            Err(err) => {
                self.record(SoundDiagnostic::MciCommandFailed {
                    command: play_cmd,
                    error: err,
                });
                false
            }
        }
    }

    pub fn stop_sound(&mut self, sound_id: Option<SoundId>) -> bool {
        self.last_diagnostic = None;
        let Some(id) = sound_id else {
            return false;
        };
        let alias = match self.sounds.get(&id) {
            Some(r) => r.alias.clone(),
            None => {
                self.record(SoundDiagnostic::InvalidSoundId(id.as_u32()));
                return false;
            }
        };
        self.backend.send(&format!("stop {alias}")).is_ok()
    }

    pub fn is_sound_playing(&mut self, sound_id: Option<SoundId>) -> bool {
        self.last_diagnostic = None;
        let Some(id) = sound_id else {
            return false;
        };
        let alias = match self.sounds.get(&id) {
            Some(r) => r.alias.clone(),
            None => return false,
        };
        match self.backend.send(&format!("status {alias} mode")) {
            Ok(mode) => mode.trim().eq_ignore_ascii_case("playing"),
            Err(_) => false,
        }
    }

    pub fn set_sound_volume(&mut self, sound_id: Option<SoundId>, volume: f32) -> bool {
        self.last_diagnostic = None;
        let Some(id) = sound_id else {
            return false;
        };
        let volume = match validate_volume(volume) {
            Ok(v) => v,
            Err(diag) => {
                self.record(diag);
                return false;
            }
        };
        let alias = match self.sounds.get(&id) {
            Some(r) => r.alias.clone(),
            None => {
                self.record(SoundDiagnostic::InvalidSoundId(id.as_u32()));
                return false;
            }
        };
        let mci_vol = (volume * 1000.0) as u32;
        self.backend
            .send(&format!("setaudio {alias} volume to {mci_vol}"))
            .is_ok()
    }

    pub fn release_sound(&mut self, sound_id: Option<SoundId>) {
        self.last_diagnostic = None;
        let Some(id) = sound_id else {
            return;
        };
        let Some(mut record) = self.sounds.remove(&id) else {
            return;
        };
        let cmd = format!("close {}", record.alias);
        if let Err(err) = self.backend.send(&cmd) {
            record.close_attempts += 1;
            self.record(SoundDiagnostic::MciCloseFailed {
                alias: record.alias.clone(),
                attempts: record.close_attempts,
                error: err,
            });
            self.sounds.insert(id, record);
        } else {
            self.sounds_by_path.remove(&record.canonical_path);
            self.aggregate_bytes = self.aggregate_bytes.saturating_sub(record.file_size);
        }
    }

    /// Stops playback for all sounds without releasing handles.
    fn stop_all(&mut self) {
        let aliases: Vec<String> = self.sounds.values().map(|r| r.alias.clone()).collect();
        for alias in aliases {
            if let Err(err) = self.backend.send(&format!("stop {alias}")) {
                self.record(SoundDiagnostic::MciCommandFailed {
                    command: format!("stop {alias}"),
                    error: err,
                });
            }
        }
    }

    /// Closes all open aliases. Failed closes retain records so limits cannot be
    /// bypassed by repeatedly opening and leaking aliases.
    pub fn close_all(&mut self) {
        let records: Vec<(SoundId, SoundRecord)> = self.sounds.drain().collect();
        let mut failed = HashMap::new();
        for (id, mut record) in records {
            if record.close_attempts >= MAX_CLOSE_RETRIES {
                failed.insert(id, record);
                continue;
            }
            let cmd = format!("close {}", record.alias);
            if let Err(err) = self.backend.send(&cmd) {
                record.close_attempts += 1;
                self.record(SoundDiagnostic::MciCloseFailed {
                    alias: record.alias.clone(),
                    attempts: record.close_attempts,
                    error: err,
                });
                failed.insert(id, record);
            }
        }
        self.sounds = failed;
        self.sounds_by_path
            .retain(|_, id| self.sounds.contains_key(id));
        self.aggregate_bytes = self.sounds.values().map(|r| r.file_size).sum();
    }
}

impl Drop for SoundBank {
    fn drop(&mut self) {
        let aliases: Vec<String> = self.sounds.values().map(|r| r.alias.clone()).collect();
        for alias in aliases {
            let _ = self.backend.send(&format!("close {alias}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Path sanitization and resolution
// ---------------------------------------------------------------------------

fn sanitize_path_string(path_str: &str) -> Result<(), SoundDiagnostic> {
    if path_str.is_empty() {
        return Err(SoundDiagnostic::PathSanitizationFailed("empty path"));
    }
    if is_drive_relative(path_str) {
        return Err(SoundDiagnostic::PathSanitizationFailed(
            "drive-relative paths (e.g. C:foo) prohibited",
        ));
    }
    for ch in path_str.chars() {
        if ch == '"' {
            return Err(SoundDiagnostic::PathSanitizationFailed(
                "double quotes prohibited",
            ));
        }
        if ch == '\0' {
            return Err(SoundDiagnostic::PathSanitizationFailed(
                "null bytes prohibited",
            ));
        }
        if (ch as u32) < 0x20 {
            return Err(SoundDiagnostic::PathSanitizationFailed(
                "control characters prohibited",
            ));
        }
    }
    if contains_parent_traversal(path_str) {
        return Err(SoundDiagnostic::PathSanitizationFailed(
            "parent directory traversal ('..') prohibited",
        ));
    }
    Ok(())
}

fn is_drive_relative(path_str: &str) -> bool {
    let bytes = path_str.as_bytes();
    bytes.len() >= 2
        && bytes[1] == b':'
        && (bytes.len() == 2 || (bytes[2] != b'/' && bytes[2] != b'\\'))
}

fn contains_parent_traversal(path_str: &str) -> bool {
    for segment in path_str.split(['/', '\\']) {
        if segment == ".." {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn resolve_sound_path(
    path_str: &str,
    script_root: &Path,
    resource_dirs: &[PathBuf],
) -> Result<PathBuf, SoundDiagnostic> {
    sanitize_path_string(path_str)?;

    let path = Path::new(path_str);

    if path.is_absolute() {
        let canonical = fs::canonicalize(path)
            .map_err(|_| SoundDiagnostic::FileNotFound(path.to_path_buf()))?;
        let canonical_str = canonical
            .to_str()
            .ok_or(SoundDiagnostic::PathSanitizationFailed(
                "non-unicode canonical path",
            ))?;
        sanitize_path_string(canonical_str)?;

        if is_allowed_system_media_path(&canonical) {
            return Ok(canonical);
        }

        let canon_root = fs::canonicalize(script_root)
            .map_err(|_| SoundDiagnostic::FileNotFound(script_root.to_path_buf()))?;
        if canonical.starts_with(&canon_root) && canonical.is_file() {
            return Ok(canonical);
        }

        for res_dir in resource_dirs {
            if let Ok(canon_res) = fs::canonicalize(res_dir) {
                if canonical.starts_with(&canon_res) && canonical.is_file() {
                    return Ok(canonical);
                }
            }
        }

        return Err(SoundDiagnostic::PathSanitizationFailed(
            "absolute path outside permitted roots prohibited",
        ));
    }

    let canonical_script_root = fs::canonicalize(script_root)
        .map_err(|_| SoundDiagnostic::FileNotFound(script_root.to_path_buf()))?;
    let normalized = path_str.replace('\\', "/");

    for res_dir in resource_dirs {
        let candidate = res_dir.join(Path::new(&normalized));
        if candidate.exists() || candidate.is_symlink() {
            if let Ok(canonical_candidate) = fs::canonicalize(&candidate) {
                let candidate_str =
                    canonical_candidate
                        .to_str()
                        .ok_or(SoundDiagnostic::PathSanitizationFailed(
                            "non-unicode canonical path",
                        ))?;
                sanitize_path_string(candidate_str)?;
                if canonical_candidate.starts_with(&canonical_script_root)
                    && canonical_candidate.is_file()
                {
                    return Ok(canonical_candidate);
                }
            }
        }
    }

    Err(SoundDiagnostic::FileNotFound(path.to_path_buf()))
}

#[cfg(windows)]
fn is_allowed_system_media_path(canonical: &Path) -> bool {
    let sys_root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    let media_dir = sys_root.join("Media");
    if let Ok(canonical_media) = fs::canonicalize(&media_dir) {
        if canonical.starts_with(&canonical_media) && canonical.is_file() {
            return true;
        }
    }
    if let Ok(canonical_win_media) = fs::canonicalize("C:\\Windows\\Media") {
        if canonical.starts_with(&canonical_win_media) && canonical.is_file() {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Lua bridge
// ---------------------------------------------------------------------------

impl SoundBank {
    pub fn install_for_addon(
        owner: &std::rc::Rc<std::cell::RefCell<Self>>,
        lua: &Lua,
        addon: &DiscoveredAddon,
    ) -> LuaResult<()> {
        let forge: Table = lua.globals().get("UiForge")?;
        let script_root = addon.script_root.clone();
        let resource_dirs = addon.resource_dirs.clone();

        let bank = owner.clone();
        forge.set(
            "LoadSound",
            lua.create_function(move |_, path: String| {
                let mut bank = bank.borrow_mut();
                let id = bank.load_sound(&path, &script_root, &resource_dirs);
                Ok(id.map(|id| id.as_u32()))
            })?,
        )?;

        let bank = owner.clone();
        forge.set(
            "PlaySound",
            lua.create_function(
                move |_, (sound_id, options): (Option<u32>, Option<Table>)| {
                    let mut bank = bank.borrow_mut();
                    let id = sound_id.and_then(|raw| SoundId::new(raw).ok());
                    let opts = match options {
                        Some(table) => {
                            let volume = match table.get::<Option<f32>>("volume") {
                                Ok(Some(v)) => match validate_volume(v) {
                                    Ok(clamped) => clamped,
                                    Err(_) => return Ok(false),
                                },
                                Ok(None) => 1.0,
                                Err(_) => return Ok(false),
                            };
                            let looping = table
                                .get::<Option<bool>>("loop")
                                .unwrap_or(Some(false))
                                .unwrap_or(false);
                            PlayOptions { volume, looping }
                        }
                        None => PlayOptions::default(),
                    };
                    Ok(bank.play_sound(id, Some(opts)))
                },
            )?,
        )?;

        let bank = owner.clone();
        forge.set(
            "StopSound",
            lua.create_function(move |_, sound_id: Option<u32>| {
                let mut bank = bank.borrow_mut();
                let id = sound_id.and_then(|raw| SoundId::new(raw).ok());
                Ok(bank.stop_sound(id))
            })?,
        )?;

        let bank = owner.clone();
        forge.set(
            "IsSoundPlaying",
            lua.create_function(move |_, sound_id: Option<u32>| {
                let mut bank = bank.borrow_mut();
                let id = sound_id.and_then(|raw| SoundId::new(raw).ok());
                Ok(bank.is_sound_playing(id))
            })?,
        )?;

        let bank = owner.clone();
        forge.set(
            "SetSoundVolume",
            lua.create_function(move |_, (sound_id, volume): (Option<u32>, f32)| {
                let mut bank = bank.borrow_mut();
                let id = sound_id.and_then(|raw| SoundId::new(raw).ok());
                Ok(bank.set_sound_volume(id, volume))
            })?,
        )?;

        let bank = owner.clone();
        forge.set(
            "ReleaseSound",
            lua.create_function(move |_, sound_id: Option<u32>| {
                let mut bank = bank.borrow_mut();
                let id = sound_id.and_then(|raw| SoundId::new(raw).ok());
                bank.release_sound(id);
                Ok(())
            })?,
        )?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Test-only recorder/noop backends
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) struct NoopBackend;

#[cfg(test)]
impl MciBackend for NoopBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn send(&mut self, _command: &str) -> Result<String, String> {
        Ok(String::new())
    }
}

#[cfg(test)]
pub(crate) struct RecorderBackend {
    log: std::cell::RefCell<Vec<String>>,
    responses: std::cell::RefCell<Vec<Result<String, String>>>,
}

#[cfg(test)]
impl RecorderBackend {
    pub fn new(responses: Vec<Result<String, String>>) -> Self {
        Self {
            log: std::cell::RefCell::new(Vec::new()),
            responses: std::cell::RefCell::new(responses),
        }
    }

    pub fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    pub fn push_response(&self, response: Result<String, String>) {
        self.responses.borrow_mut().push(response);
    }
}

#[cfg(test)]
impl MciBackend for RecorderBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn send(&mut self, command: &str) -> Result<String, String> {
        self.log.borrow_mut().push(command.to_string());
        if self.responses.borrow().is_empty() {
            return Ok(String::new());
        }
        self.responses.borrow_mut().remove(0).or_else(|e| {
            if e.is_empty() {
                Ok(String::new())
            } else {
                Err(e)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eqoa_sound_{}_{}", label, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn generate_minimal_pcm_wav(samples: usize) -> Vec<u8> {
        let data_size = (samples * 4) as u32;
        let file_size = 36 + data_size;
        let mut buf = Vec::with_capacity(44 + data_size as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&44100u32.to_le_bytes());
        buf.extend_from_slice(&176400u32.to_le_bytes());
        buf.extend_from_slice(&4u16.to_le_bytes());
        buf.extend_from_slice(&16u16.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.resize(44 + data_size as usize, 0);
        buf
    }

    #[test]
    fn forge_sound_volume_validation_and_clamping() {
        assert_eq!(validate_volume(0.5).unwrap(), 0.5);
        assert_eq!(validate_volume(-0.5).unwrap(), 0.0);
        assert_eq!(validate_volume(1.5).unwrap(), 1.0);
        assert_eq!(validate_volume(0.0).unwrap(), 0.0);
        assert_eq!(validate_volume(1.0).unwrap(), 1.0);

        assert!(
            matches!(validate_volume(f32::NAN), Err(SoundDiagnostic::InvalidVolume(v)) if v.is_nan())
        );
        assert!(
            matches!(validate_volume(f32::INFINITY), Err(SoundDiagnostic::InvalidVolume(v)) if v.is_infinite())
        );
        assert!(
            matches!(validate_volume(f32::NEG_INFINITY), Err(SoundDiagnostic::InvalidVolume(v)) if v.is_infinite())
        );

        let mut bank = SoundBank::with_owner_id_and_backend(100, Box::new(NoopBackend));
        bank.set_active(true);
        let id = SoundId::new(1).unwrap();
        bank.sounds.insert(
            id,
            SoundRecord {
                id,
                alias: "uiforge_snd_test_1".to_string(),
                canonical_path: PathBuf::from("dummy.wav"),
                file_size: 100,
                close_attempts: 0,
            },
        );
        assert!(!bank.set_sound_volume(Some(id), f32::NAN));
        assert!(
            matches!(bank.last_diagnostic(), Some(SoundDiagnostic::InvalidVolume(v)) if v.is_nan())
        );
        assert!(!bank.play_sound(
            Some(id),
            Some(PlayOptions {
                volume: f32::INFINITY,
                looping: false
            })
        ));
        assert!(
            matches!(bank.last_diagnostic(), Some(SoundDiagnostic::InvalidVolume(v)) if v.is_infinite())
        );
    }

    #[test]
    fn forge_sound_path_sanitization_rejects_dangerous_inputs() {
        assert!(sanitize_path_string("normal_path.wav").is_ok());
        assert!(sanitize_path_string("sub/dir/audio.wav").is_ok());

        assert!(matches!(
            sanitize_path_string("sound.wav\" type mpegvideo alias injected"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("sound.wav\0evil"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("sound\r\n.wav"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("sound\x1b.wav"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string(""),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("../escape.wav"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("foo\\..\\escape.wav"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("C:relative_file.wav"),
            Err(SoundDiagnostic::PathSanitizationFailed(_))
        ));
    }

    #[test]
    fn forge_sound_bank_defaults_inactive_and_blocks_playback() {
        let mut bank = SoundBank::with_owner_id_and_backend(101, Box::new(NoopBackend));
        assert!(!bank.is_active());
        assert!(!bank.play_sound(Some(SoundId(1)), None));
        assert!(matches!(
            bank.last_diagnostic(),
            Some(SoundDiagnostic::InactiveAddon)
        ));

        bank.set_active(true);
        assert!(bank.is_active());
        assert!(!bank.play_sound(Some(SoundId(1)), None));
        assert!(matches!(
            bank.last_diagnostic(),
            Some(SoundDiagnostic::InvalidSoundId(1))
        ));
    }

    #[test]
    fn forge_sound_monotonic_id_exhaustion_guard() {
        let mut bank = SoundBank::with_owner_id_and_backend(102, Box::new(NoopBackend));
        bank.set_active(true);
        bank.next_sound_id = MAX_SOUND_ID_U32 + 1;

        let script_root = PathBuf::from("C:\\Windows\\Media");
        let res = bank.load_sound("C:\\Windows\\Media\\tada.wav", &script_root, &[]);
        assert!(res.is_none());
        assert!(matches!(
            bank.last_diagnostic(),
            Some(SoundDiagnostic::IdExhausted(_))
        ));
    }

    #[test]
    fn forge_sound_nil_and_invalid_handles_are_safe_noops() {
        let mut bank = SoundBank::with_owner_id_and_backend(103, Box::new(NoopBackend));
        bank.set_active(true);

        assert!(!bank.play_sound(None, None));
        assert!(!bank.stop_sound(None));
        assert!(!bank.is_sound_playing(None));
        assert!(!bank.set_sound_volume(None, 0.5));
        bank.release_sound(None);

        let bogus = Some(SoundId(9999));
        assert!(!bank.play_sound(bogus, None));
        assert!(!bank.stop_sound(bogus));
        assert!(!bank.is_sound_playing(bogus));
        assert!(!bank.set_sound_volume(bogus, 0.5));
        bank.release_sound(bogus);
    }

    #[test]
    fn forge_sound_handle_and_file_size_limits() {
        let mut bank = SoundBank::with_owner_id_and_backend(104, Box::new(NoopBackend));
        bank.set_active(true);
        let dir = temp_dir("bounds");

        let big_path = dir.join("oversized.wav");
        let f = std::fs::File::create(&big_path).unwrap();
        f.set_len(MAX_SOUND_FILE_BYTES + 1024).unwrap();

        let res = bank.load_sound(&big_path.to_string_lossy(), &dir, &[dir.clone()]);
        assert!(res.is_none());
        assert!(matches!(
            bank.last_diagnostic(),
            Some(SoundDiagnostic::FileTooLarge { .. })
        ));
    }

    #[test]
    fn forge_sound_close_failure_retains_record_and_bounds() {
        let recorder = RecorderBackend::new(vec![Err("close failed".into())]);
        let mut bank = SoundBank::with_owner_id_and_backend(105, Box::new(recorder));
        bank.set_active(true);

        let id = SoundId::new(1).unwrap();
        bank.sounds.insert(
            id,
            SoundRecord {
                id,
                alias: "uiforge_snd_mock_1".to_string(),
                canonical_path: PathBuf::from("mock.wav"),
                file_size: 500,
                close_attempts: MAX_CLOSE_RETRIES,
            },
        );
        bank.aggregate_bytes = 500;

        assert_eq!(bank.len(), 1);
        assert_eq!(bank.aggregate_bytes(), 500);

        bank.close_all();
        assert_eq!(
            bank.len(),
            1,
            "failed close record must remain counted while bank alive"
        );
        assert_eq!(bank.aggregate_bytes(), 500);
    }

    #[test]
    #[cfg(windows)]
    fn forge_sound_real_mci_generated_wav_playback_or_explicit_device_skip() {
        let mut bank = SoundBank::with_owner_id(106);
        bank.set_active(true);
        let dir = temp_dir("mci");
        let wav_path = dir.join("silence.wav");
        let wav_bytes = generate_minimal_pcm_wav(4410);
        std::fs::write(&wav_path, &wav_bytes).unwrap();

        let id = match bank.load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()]) {
            Some(id) => id,
            None => {
                eprintln!(
                    "SKIP: Windows MCI audio device not available or open failed: {:?}",
                    bank.last_diagnostic()
                );
                return;
            }
        };

        assert_eq!(bank.len(), 1);
        let cached_id = bank.load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()]);
        assert_eq!(cached_id, Some(id));
        assert_eq!(bank.len(), 1);

        let play_ok = bank.play_sound(
            Some(id),
            Some(PlayOptions {
                volume: 0.5,
                looping: false,
            }),
        );
        if play_ok {
            assert!(bank.set_sound_volume(Some(id), 0.8));
            assert!(bank.stop_sound(Some(id)));
        } else {
            eprintln!(
                "SKIP: Windows MCI play returned false (device missing or muted in CI): {:?}",
                bank.last_diagnostic()
            );
        }
        bank.release_sound(Some(id));
        assert_eq!(bank.len(), 0);
    }

    #[test]
    fn forge_sound_lua_bindings_contract_and_nil_safety() {
        let lua = Lua::new();
        let uiforge_table = lua.create_table().expect("create table");
        let bank = Rc::new(RefCell::new(SoundBank::with_owner_id_and_backend(
            107,
            Box::new(NoopBackend),
        )));
        bank.borrow_mut().set_active(true);
        let script_root = std::env::temp_dir();
        lua.globals().set("UiForge", uiforge_table.clone()).unwrap();
        SoundBank::install_for_addon(
            &bank,
            &lua,
            &DiscoveredAddon {
                id: "test".into(),
                name: "test".into(),
                entry: script_root.join("main.lua"),
                script_root: script_root.clone(),
                package_root: script_root.clone(),
                module_dirs: vec![],
                resource_dirs: vec![script_root.clone()],
                api: None,
                requested_capabilities: vec![],
                initially_enabled: true,
            },
        )
        .unwrap();
        lua.globals().set("UiForge", uiforge_table).unwrap();

        let script = r#"
            assert(UiForge.PlaySound(nil) == false)
            assert(UiForge.StopSound(nil) == false)
            assert(UiForge.IsSoundPlaying(nil) == false)
            assert(UiForge.SetSoundVolume(nil, 0.5) == false)
            UiForge.ReleaseSound(nil)

            assert(UiForge.PlaySound(1, { volume = 0/0 }) == false)
            assert(UiForge.PlaySound(nil, { volume = -0.5 }) == false)
            assert(UiForge.PlaySound(nil, { volume = 1.5 }) == false)

            local missing = UiForge.LoadSound("does_not_exist_xyz.wav")
            assert(missing == nil)

            local evil = UiForge.LoadSound('sound.wav" injected')
            assert(evil == nil)
        "#;
        lua.load(script).exec().expect("lua script must succeed");
    }

    #[test]
    #[cfg(windows)]
    fn forge_sound_recorder_open_play_stop_volume_release_syntax_and_order() {
        let dir = temp_dir("recorder");
        let wav_path = dir.join("silence.wav");
        std::fs::write(&wav_path, generate_minimal_pcm_wav(8)).unwrap();

        let recorder = RecorderBackend::new(vec![
            Ok("".into()), // open
            Ok("".into()), // setaudio
            Ok("".into()), // seek
            Ok("".into()), // play
            Ok("".into()), // setaudio volume
            Ok("".into()), // stop
            Ok("".into()), // close
        ]);
        let mut bank = SoundBank::with_owner_id_and_backend(200, Box::new(recorder));
        bank.set_active(true);

        let id = bank
            .load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()])
            .unwrap();
        assert!(bank.play_sound(
            Some(id),
            Some(PlayOptions {
                volume: 0.75,
                looping: false
            })
        ));
        assert!(bank.set_sound_volume(Some(id), 0.25));
        assert!(bank.stop_sound(Some(id)));
        bank.release_sound(Some(id));

        let log = bank.backend_log();
        assert!(log[0].starts_with("open \""));
        assert!(log[0].contains("type mpegvideo alias uiforge_snd_"));
        assert_eq!(
            log[1],
            format!(
                "setaudio uiforge_snd_{:08x}_{} volume to 750",
                bank.owner_id,
                id.as_u32()
            )
        );
        assert_eq!(
            log[2],
            format!(
                "seek uiforge_snd_{:08x}_{} to start",
                bank.owner_id,
                id.as_u32()
            )
        );
        assert_eq!(
            log[3],
            format!("play uiforge_snd_{:08x}_{}", bank.owner_id, id.as_u32())
        );
        assert_eq!(
            log[4],
            format!(
                "setaudio uiforge_snd_{:08x}_{} volume to 250",
                bank.owner_id,
                id.as_u32()
            )
        );
        assert_eq!(
            log[5],
            format!("stop uiforge_snd_{:08x}_{}", bank.owner_id, id.as_u32())
        );
        assert_eq!(
            log[6],
            format!("close uiforge_snd_{:08x}_{}", bank.owner_id, id.as_u32())
        );
    }

    #[test]
    #[cfg(windows)]
    fn forge_sound_recorder_restart_replaces_play_and_loop_repeat() {
        let dir = temp_dir("recorder_loop");
        let wav_path = dir.join("silence.wav");
        std::fs::write(&wav_path, generate_minimal_pcm_wav(8)).unwrap();

        let recorder = RecorderBackend::new(vec![
            Ok("".into()), // open
            Ok("".into()), // setaudio
            Ok("".into()), // seek
            Ok("".into()), // play
            Ok("".into()), // setaudio
            Ok("".into()), // seek
            Ok("".into()), // play repeat
        ]);
        let mut bank = SoundBank::with_owner_id_and_backend(201, Box::new(recorder));
        bank.set_active(true);

        let id = bank
            .load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()])
            .unwrap();
        assert!(bank.play_sound(Some(id), None));
        assert!(bank.play_sound(
            Some(id),
            Some(PlayOptions {
                volume: 1.0,
                looping: true
            })
        ));

        let log = bank.backend_log();
        assert_eq!(
            log[3],
            format!("play uiforge_snd_{:08x}_{}", bank.owner_id, id.as_u32())
        );
        assert_eq!(
            log[6],
            format!(
                "play uiforge_snd_{:08x}_{} repeat",
                bank.owner_id,
                id.as_u32()
            )
        );
    }

    #[test]
    #[cfg(windows)]
    fn forge_sound_recorder_failed_close_retains_accounting_and_limits() {
        let dir = temp_dir("recorder_close_fail");
        let wav_path = dir.join("silence.wav");
        std::fs::write(&wav_path, generate_minimal_pcm_wav(8)).unwrap();

        let recorder = RecorderBackend::new(vec![
            Ok("".into()),              // open
            Err("close failed".into()), // close on release
        ]);
        let mut bank = SoundBank::with_owner_id_and_backend(202, Box::new(recorder));
        bank.set_active(true);

        let id = bank
            .load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()])
            .unwrap();
        assert_eq!(bank.len(), 1);
        bank.release_sound(Some(id));
        assert_eq!(bank.len(), 1, "failed close must retain record");
        assert_eq!(bank.aggregate_bytes(), wav_bytes_len(8));

        // A second load of the same file must still reuse the cached ID.
        let cached_id = bank.load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()]);
        assert_eq!(cached_id, Some(id));
        assert_eq!(bank.len(), 1);
    }

    #[test]
    #[cfg(windows)]
    fn forge_sound_recorder_quota_rejects_before_os_open() {
        let dir = temp_dir("recorder_quota");
        let wav_path = dir.join("silence.wav");
        std::fs::write(&wav_path, generate_minimal_pcm_wav(8)).unwrap();

        let recorder = RecorderBackend::new(vec![]);
        let mut bank = SoundBank::with_owner_id_and_backend(203, Box::new(recorder));
        bank.set_active(true);
        bank.sounds.insert(
            SoundId::new(1).unwrap(),
            SoundRecord {
                id: SoundId::new(1).unwrap(),
                alias: "uiforge_snd_filler".to_string(),
                canonical_path: wav_path.clone(),
                file_size: 1,
                close_attempts: 0,
            },
        );
        // Fill handle budget.
        for i in 2..=MAX_SOUND_HANDLES as u32 {
            bank.sounds.insert(
                SoundId::new(i).unwrap(),
                SoundRecord {
                    id: SoundId::new(i).unwrap(),
                    alias: format!("uiforge_snd_filler_{i}"),
                    canonical_path: wav_path.clone(),
                    file_size: 1,
                    close_attempts: 0,
                },
            );
        }

        let res = bank.load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()]);
        assert!(res.is_none());
        assert!(matches!(
            bank.last_diagnostic(),
            Some(SoundDiagnostic::HandleLimitExceeded { .. })
        ));
        assert!(
            bank.backend_log().is_empty(),
            "quota rejection must not call OS"
        );
    }

    #[test]
    #[cfg(windows)]
    fn forge_sound_recorder_setaudio_seek_failure_ignored_for_play_boolean() {
        let dir = temp_dir("recorder_partial");
        let wav_path = dir.join("silence.wav");
        std::fs::write(&wav_path, generate_minimal_pcm_wav(8)).unwrap();

        let recorder = RecorderBackend::new(vec![
            Ok("".into()),                 // open
            Err("setaudio failed".into()), // ignored
            Err("seek failed".into()),     // ignored
            Ok("".into()),                 // play succeeds
        ]);
        let mut bank = SoundBank::with_owner_id_and_backend(204, Box::new(recorder));
        bank.set_active(true);

        let id = bank
            .load_sound(&wav_path.to_string_lossy(), &dir, &[dir.clone()])
            .unwrap();
        assert!(bank.play_sound(Some(id), None));
        assert!(matches!(
            bank.last_diagnostic(),
            Some(SoundDiagnostic::MciCommandFailed { .. })
        ));
    }

    fn wav_bytes_len(samples: usize) -> u64 {
        (44 + samples * 4) as u64
    }
}
