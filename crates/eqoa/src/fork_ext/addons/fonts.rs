//! Safe TrueType font loading, atlas lifecycle management, and ImFont userdata.
//!
//! Implements `UiForge.LoadFont`, `ImGui.PushFont`, `ImGui.PopFont`, and
//! `ImGui.SetWindowFontScale` matching FrontiersForge reference semantics:
//! - Context requirement: returns `nil` before ImGui context initialization.
//! - Graceful fallback: missing/corrupt font files return a fallback handle resolving to default font.
//! - Safe loading: uses `dear_imgui_rs::fonts::atlas::validated::StbTrueTypeFontData` and
//!   `FontSource::stb_truetype_with_size` to validate all tables and composite glyph bounds before FFI.
//! - Queued pre-frame registration: atlas mutation occurs only outside frame scope (before `try_frame`).
//! - One-frame adaptation: Frame 0 renders default font; pre-frame flush bakes requested size; Frame 1+ renders at target size.
//! - Push reference LegacySize: Dear ImGui 1.92 pushes with reference size so 20px vs 36px are distinguished.
//! - Reverse unwind: unbalanced `PushFont` tokens are popped on script error or frame end.
//! - Epoch invalidation: stale handles from destroyed contexts are rejected.

use super::discovery::DiscoveredAddon;
use dear_imgui_rs::fonts::{FontSource, StbTrueTypeFontData};
use dear_imgui_rs::{Context, sys};
use mlua::{Lua, Result as LuaResult, Table, UserData, UserDataMethods};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const MAX_FONT_FILE_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB
pub const MAX_FONT_HANDLES: usize = 16;
pub const MAX_AGGREGATE_FONT_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB
pub const MAX_DIAGNOSTICS: usize = 32;
pub const MAX_DIAGNOSTIC_BYTES: usize = 2048;

static CONTEXT_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn current_context_epoch() -> u64 {
    CONTEXT_EPOCH.load(std::sync::atomic::Ordering::Acquire)
}

pub fn bump_context_epoch() -> u64 {
    CONTEXT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1
}

#[derive(Clone, Debug, PartialEq)]
pub enum FontHandleState {
    Pending {
        canonical_path: PathBuf,
        size_pixels: f32,
        file_size: u64,
    },
    Ready {
        raw_font: *mut sys::ImFont,
        size_pixels: f32,
        file_size: u64,
    },
    Fallback,
}

#[derive(Clone)]
pub struct LuaImFont {
    pub(crate) handle_id: u32,
    pub(crate) state: Rc<RefCell<FontHandleState>>,
    pub(crate) context_epoch: u64,
}

impl UserData for LuaImFont {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, ()| {
            let desc = match &*this.state.borrow() {
                FontHandleState::Pending { size_pixels, .. } => {
                    format!("ImFont(pending, size={size_pixels})")
                }
                FontHandleState::Ready { size_pixels, .. } => {
                    format!("ImFont(ready, size={size_pixels})")
                }
                FontHandleState::Fallback => "ImFont(fallback)".to_string(),
            };
            Ok(desc)
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FontDiagnostic {
    ContextNotInitialized,
    PathSanitizationFailed(&'static str),
    FileNotFound(PathBuf),
    FileTooLarge { size: u64, max: u64 },
    HandleLimitExceeded { limit: usize },
    AggregateLimitExceeded { current: u64, adding: u64, max: u64 },
    ValidationFailed(String),
}

impl std::fmt::Display for FontDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextNotInitialized => {
                write!(f, "LoadFont called before ImGui was initialized")
            }
            Self::PathSanitizationFailed(reason) => {
                write!(f, "font path rejected for security: {reason}")
            }
            Self::FileNotFound(path) => write!(f, "font file not found: {}", path.display()),
            Self::FileTooLarge { size, max } => {
                write!(f, "font file size {size} exceeds limit {max}")
            }
            Self::HandleLimitExceeded { limit } => {
                write!(f, "font handle limit {limit} exceeded")
            }
            Self::AggregateLimitExceeded { current, adding, max } => {
                write!(f, "aggregate font size {current}+{adding} exceeds limit {max}")
            }
            Self::ValidationFailed(err) => write!(f, "font validation failed: {err}"),
        }
    }
}

pub struct FontBank {
    _owner_id: u64,
    context_epoch: u64,
    next_handle_id: u32,
    handles: HashMap<u32, LuaImFont>,
    cache_by_key: HashMap<String, LuaImFont>,
    aggregate_bytes: u64,
    diagnostics: Vec<String>,
}

impl FontBank {
    pub fn new(owner_id: u64, context_epoch: u64) -> Self {
        Self {
            _owner_id: owner_id,
            context_epoch,
            next_handle_id: 1,
            handles: HashMap::new(),
            cache_by_key: HashMap::new(),
            aggregate_bytes: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn set_context_epoch(&mut self, epoch: u64) {
        if self.context_epoch != epoch {
            self.context_epoch = epoch;
            self.cache_by_key.clear();
            self.handles.clear();
            self.aggregate_bytes = 0;
        }
    }

    pub fn drain_diagnostics(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diagnostics)
    }

    pub fn record_diagnostic(&mut self, diag: FontDiagnostic) {
        let mut text = diag.to_string();
        if text.len() > MAX_DIAGNOSTIC_BYTES {
            let mut end = MAX_DIAGNOSTIC_BYTES;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        if !self.diagnostics.contains(&text) {
            if self.diagnostics.len() >= MAX_DIAGNOSTICS {
                self.diagnostics.remove(0);
            }
            self.diagnostics.push(text);
        }
    }

    pub fn load_font(
        &mut self,
        path_str: &str,
        size_px: Option<f32>,
        addon: &DiscoveredAddon,
        context_initialized: bool,
    ) -> Option<LuaImFont> {
        if !context_initialized {
            self.record_diagnostic(FontDiagnostic::ContextNotInitialized);
            return None;
        }

        let size = size_px.unwrap_or(0.0);
        let size = if size > 0.0 { size } else { 0.0 };
        let cache_key = format!("{}|{}", path_str, size);

        if let Some(cached) = self.cache_by_key.get(&cache_key) {
            return Some(cached.clone());
        }

        let fallback_handle = |bank: &mut Self, key: String| -> LuaImFont {
            let id = bank.next_handle_id;
            bank.next_handle_id = bank.next_handle_id.wrapping_add(1);
            let handle = LuaImFont {
                handle_id: id,
                state: Rc::new(RefCell::new(FontHandleState::Fallback)),
                context_epoch: bank.context_epoch,
            };
            bank.cache_by_key.insert(key, handle.clone());
            bank.handles.insert(id, handle.clone());
            handle
        };

        if self.handles.len() >= MAX_FONT_HANDLES {
            self.record_diagnostic(FontDiagnostic::HandleLimitExceeded {
                limit: MAX_FONT_HANDLES,
            });
            return Some(fallback_handle(self, cache_key));
        }

        let canonical_path = match resolve_font_path(path_str, &addon.script_root, &addon.resource_dirs) {
            Ok(p) => p,
            Err(diag) => {
                self.record_diagnostic(diag);
                return Some(fallback_handle(self, cache_key));
            }
        };

        let file_size = match fs::metadata(&canonical_path) {
            Ok(meta) => meta.len(),
            Err(_e) => {
                self.record_diagnostic(FontDiagnostic::FileNotFound(canonical_path));
                return Some(fallback_handle(self, cache_key));
            }
        };

        if file_size > MAX_FONT_FILE_BYTES {
            self.record_diagnostic(FontDiagnostic::FileTooLarge {
                size: file_size,
                max: MAX_FONT_FILE_BYTES,
            });
            return Some(fallback_handle(self, cache_key));
        }

        if self.aggregate_bytes.saturating_add(file_size) > MAX_AGGREGATE_FONT_BYTES {
            self.record_diagnostic(FontDiagnostic::AggregateLimitExceeded {
                current: self.aggregate_bytes,
                adding: file_size,
                max: MAX_AGGREGATE_FONT_BYTES,
            });
            return Some(fallback_handle(self, cache_key));
        }

        let id = self.next_handle_id;
        self.next_handle_id = self.next_handle_id.wrapping_add(1);
        self.aggregate_bytes += file_size;

        let handle = LuaImFont {
            handle_id: id,
            state: Rc::new(RefCell::new(FontHandleState::Pending {
                canonical_path: canonical_path.clone(),
                size_pixels: size,
                file_size,
            })),
            context_epoch: self.context_epoch,
        };

        self.cache_by_key.insert(cache_key.clone(), handle.clone());
        let resolved_cache_key = format!("{}|{}", canonical_path.display(), size);
        if resolved_cache_key != cache_key {
            self.cache_by_key.insert(resolved_cache_key, handle.clone());
        }
        self.handles.insert(id, handle.clone());
        Some(handle)
    }

    pub fn flush_pending(&mut self, context: &mut Context) {
        let handles: Vec<LuaImFont> = self.handles.values().cloned().collect();
        for handle in &handles {
            let mut state = handle.state.borrow_mut();
            if let FontHandleState::Pending {
                canonical_path,
                size_pixels,
                file_size,
            } = &*state
            {
                let data = match StbTrueTypeFontData::from_file(canonical_path) {
                    Ok(d) => d,
                    Err(e) => {
                        self.record_diagnostic(FontDiagnostic::ValidationFailed(format!(
                            "{}: {e}",
                            canonical_path.display()
                        )));
                        *state = FontHandleState::Fallback;
                        continue;
                    }
                };

                let font_source = FontSource::stb_truetype_with_size(data, *size_pixels);
                let _font_id = context.font_atlas().add_font(&[font_source]);

                let raw_atlas = context.font_atlas().raw();
                let font_ptr = unsafe {
                    let count = (*raw_atlas).Fonts.Size as usize;
                    if count > 0 {
                        *(*raw_atlas).Fonts.Data.add(count - 1)
                    } else {
                        std::ptr::null_mut()
                    }
                };

                if font_ptr.is_null() {
                    *state = FontHandleState::Fallback;
                } else {
                    // If legacy renderer claimed (as in headless unit tests), rebuild atlas
                    let _ = context.font_atlas().try_claim_legacy_renderer().map(|r| r.build());
                    *state = FontHandleState::Ready {
                        raw_font: font_ptr,
                        size_pixels: *size_pixels,
                        file_size: *file_size,
                    };
                }
            }
        }
    }

    pub fn install_for_addon(
        owner: &Rc<RefCell<Self>>,
        lua: &Lua,
        addon: &DiscoveredAddon,
    ) -> LuaResult<()> {
        let forge: Table = lua.globals().get("UiForge")?;
        let bank = owner.clone();
        let addon_clone = addon.clone();

        forge.set(
            "LoadFont",
            lua.create_function(move |_, (path_str, size_px): (String, Option<f32>)| {
                let context_initialized = unsafe { !sys::igGetCurrentContext().is_null() };
                let mut bank = bank.borrow_mut();
                Ok(bank.load_font(&path_str, size_px, &addon_clone, context_initialized))
            })?,
        )?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Path sanitization and containment
// ---------------------------------------------------------------------------

pub fn sanitize_path_string(path_str: &str) -> Result<(), FontDiagnostic> {
    if path_str.is_empty() {
        return Err(FontDiagnostic::PathSanitizationFailed("empty path"));
    }
    if is_drive_relative(path_str) {
        return Err(FontDiagnostic::PathSanitizationFailed(
            "drive-relative paths (e.g. C:foo) prohibited",
        ));
    }
    for ch in path_str.chars() {
        if ch == '"' {
            return Err(FontDiagnostic::PathSanitizationFailed(
                "double quotes prohibited",
            ));
        }
        if ch == '\0' {
            return Err(FontDiagnostic::PathSanitizationFailed("null bytes prohibited"));
        }
        if (ch as u32) < 0x20 {
            return Err(FontDiagnostic::PathSanitizationFailed(
                "control characters prohibited",
            ));
        }
    }
    if contains_parent_traversal(path_str) {
        return Err(FontDiagnostic::PathSanitizationFailed(
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
fn is_allowed_system_fonts_path(canonical: &Path) -> bool {
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts_dir = system_root.join("Fonts");
    if let Ok(canon_fonts) = fs::canonicalize(&fonts_dir) {
        canonical.starts_with(&canon_fonts) && canonical.is_file()
    } else {
        false
    }
}

#[cfg(not(windows))]
fn is_allowed_system_fonts_path(_canonical: &Path) -> bool {
    false
}

pub fn resolve_font_path(
    path_str: &str,
    script_root: &Path,
    resource_dirs: &[PathBuf],
) -> Result<PathBuf, FontDiagnostic> {
    sanitize_path_string(path_str)?;

    let canon_script_root = script_root
        .canonicalize()
        .unwrap_or_else(|_| script_root.to_path_buf());

    let path = Path::new(path_str);
    let is_relative = !path_str.starts_with('/')
        && !path_str.starts_with('\\')
        && !path_str.contains(':');

    if is_relative {
        let normalized = path_str.replace('\\', "/");
        for res_dir in resource_dirs {
            let candidate = res_dir.join(Path::new(&normalized));
            if candidate.exists() {
                if let Ok(canon) = candidate.canonicalize() {
                    let in_res = res_dir
                        .canonicalize()
                        .map(|r| canon.starts_with(&r))
                        .unwrap_or(false);
                    let in_root = canon.starts_with(&canon_script_root);
                    if (in_res || in_root) && canon.is_file() {
                        return Ok(canon);
                    }
                }
            }
        }
        let cand_root = script_root.join(Path::new(&normalized));
        if cand_root.exists() {
            if let Ok(canon) = cand_root.canonicalize() {
                if canon.starts_with(&canon_script_root) && canon.is_file() {
                    return Ok(canon);
                }
            }
        }
        Err(FontDiagnostic::FileNotFound(PathBuf::from(path_str)))
    } else {
        let canonical = fs::canonicalize(path)
            .map_err(|_| FontDiagnostic::FileNotFound(path.to_path_buf()))?;
        if is_allowed_system_fonts_path(&canonical) {
            return Ok(canonical);
        }
        if canonical.starts_with(&canon_script_root) && canonical.is_file() {
            return Ok(canonical);
        }
        Err(FontDiagnostic::PathSanitizationFailed(
            "absolute font path outside allowed script root and system fonts",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::bridge::Bridge;
    use crate::fork_ext::addons::host::Host;
    use dear_imgui_rs::Context;

    fn temp_test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("eqoa_font_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test addon").unwrap();
        let res_dir = root.join("resources");
        fs::create_dir_all(&res_dir).unwrap();
        DiscoveredAddon {
            id: "test_addon".to_string(),
            name: "Test Addon".to_string(),
            entry,
            script_root: root.to_path_buf(),
            package_root: root.to_path_buf(),
            module_dirs: vec![root.to_path_buf()],
            resource_dirs: vec![res_dir],
            api: Some("eqoa/1".to_string()),
            requested_capabilities: vec![],
            initially_enabled: true,
        }
    }

    #[test]
    fn forge_fonts_path_sanitization_rejects_dangerous_inputs() {
        assert!(matches!(
            sanitize_path_string(""),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("C:foo.ttf"),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("font\"name.ttf"),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("font\0name.ttf"),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("font\x1fname.ttf"),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string("dir/../font.ttf"),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
        assert!(matches!(
            sanitize_path_string(r"dir\..\font.ttf"),
            Err(FontDiagnostic::PathSanitizationFailed(_))
        ));
    }

    #[test]
    fn forge_fonts_relative_and_absolute_containment() {
        let dir = temp_test_dir("containment");
        let addon = test_addon(&dir);

        // Escape relative
        let res = resolve_font_path("../secret.ttf", &addon.script_root, &addon.resource_dirs);
        assert!(res.is_err());

        // Inside resource dirs
        let font_file = addon.resource_dirs[0].join("test.ttf");
        fs::write(&font_file, b"dummy font").unwrap();
        let res = resolve_font_path("test.ttf", &addon.script_root, &addon.resource_dirs);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), fs::canonicalize(&font_file).unwrap());

        // Arbitrary absolute path outside script root and system fonts rejected
        let foreign = std::env::temp_dir().join("foreign_font.ttf");
        fs::write(&foreign, b"dummy foreign font").unwrap();
        let res = resolve_font_path(&foreign.to_string_lossy(), &addon.script_root, &addon.resource_dirs);
        assert!(matches!(res, Err(FontDiagnostic::PathSanitizationFailed(_))));

        #[cfg(windows)]
        {
            // Windows system font allowed
            let sys_font = r"C:\Windows\Fonts\times.ttf";
            if Path::new(sys_font).exists() {
                let res = resolve_font_path(sys_font, &addon.script_root, &addon.resource_dirs);
                assert!(res.is_ok());
            }
        }
    }

    #[test]
    fn forge_fonts_uninitialized_context_returns_nil() {
        let dir = temp_test_dir("no_ctx");
        let addon = test_addon(&dir);
        let mut bank = FontBank::new(1, 1);
        let res = bank.load_font(r"C:\Windows\Fonts\times.ttf", Some(20.0), &addon, false);
        assert!(res.is_none());
        assert_eq!(
            bank.drain_diagnostics(),
            vec!["LoadFont called before ImGui was initialized".to_string()]
        );
    }

    #[test]
    fn forge_fonts_missing_file_graceful_fallback_and_caching() {
        let dir = temp_test_dir("fallback");
        let addon = test_addon(&dir);
        let mut bank = FontBank::new(1, 1);

        let missing = r"C:\Windows\Fonts\nonexistent_font_12345.ttf";
        let handle1 = bank.load_font(missing, Some(20.0), &addon, true);
        assert!(handle1.is_some());
        let h1 = handle1.unwrap();
        assert_eq!(*h1.state.borrow(), FontHandleState::Fallback);

        // Second load with same parameters must return cached handle and NOT add more diagnostics
        let diags_before = bank.diagnostics.len();
        let handle2 = bank.load_font(missing, Some(20.0), &addon, true);
        assert!(handle2.is_some());
        let h2 = handle2.unwrap();
        assert_eq!(h1.handle_id, h2.handle_id);
        assert_eq!(bank.diagnostics.len(), diags_before);
    }

    #[test]
    fn forge_fonts_handle_and_size_limits() {
        let dir = temp_test_dir("limits");
        let addon = test_addon(&dir);
        let mut bank = FontBank::new(1, 1);

        // Max handles limit
        for i in 0..MAX_FONT_HANDLES {
            let p = addon.resource_dirs[0].join(format!("dummy_{i}.ttf"));
            fs::write(&p, b"font").unwrap();
            let h = bank.load_font(&format!("dummy_{i}.ttf"), Some(16.0), &addon, true);
            assert!(h.is_some());
        }

        let p_extra = addon.resource_dirs[0].join("dummy_extra.ttf");
        fs::write(&p_extra, b"font").unwrap();
        let h_extra = bank.load_font("dummy_extra.ttf", Some(16.0), &addon, true);
        assert!(h_extra.is_some());
        assert_eq!(*h_extra.unwrap().state.borrow(), FontHandleState::Fallback);
        let diags = bank.drain_diagnostics();
        assert!(diags.iter().any(|d| d.contains("handle limit 16 exceeded")));
    }

    #[test]
    fn forge_fonts_pre_frame_flush_and_render_size_distinction() {
        #[cfg(not(windows))]
        return;

        let sys_font = r"C:\Windows\Fonts\times.ttf";
        if !Path::new(sys_font).exists() {
            eprintln!("SKIP font test: C:\\Windows\\Fonts\\times.ttf not present");
            return;
        }

        let dir = temp_test_dir("render_distinction");
        let addon = test_addon(&dir);

        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        context.io_mut().set_display_size([800.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.font_atlas().try_claim_legacy_renderer().unwrap().build();

        let host = Host::for_addon(&addon).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let font_bank = Rc::new(RefCell::new(FontBank::new(1, current_context_epoch())));
        FontBank::install_for_addon(&font_bank, &host.lua, &addon).unwrap();

        // Frame 0: LoadFont returns Pending handle. PushFont uses default font safely.
        context.frame();
        let script0 = host.compile(r#"
            times20 = UiForge.LoadFont("C:\\Windows\\Fonts\\times.ttf", 20.0)
            times36 = UiForge.LoadFont("C:\\Windows\\Fonts\\times.ttf", 36.0)
            assert(times20 ~= nil and times36 ~= nil)
            ImGui.PushFont(times20)
            local w0, h0 = ImGui.CalcTextSize("Hello EQOA")
            ImGui.PopFont()
            _G.w0 = w0
            _G.h0 = h0
        "#).unwrap();
        bridge.invoke(|| host.run(&script0)).unwrap();
        drop(context.render_legacy());
        let w0: f32 = host.lua.globals().get("w0").unwrap();
        let h0: f32 = host.lua.globals().get("h0").unwrap();
        assert!(w0 > 0.0 && h0 > 0.0);

        // Pre-frame flush: flushes pending fonts into the atlas outside frame scope
        font_bank.borrow_mut().flush_pending(&mut context);

        // Frame 1: PushFont now uses real baked 20px and 36px fonts!
        context.frame();
        let script1 = host.compile(r#"
            ImGui.PushFont(times20)
            local w20, h20 = ImGui.CalcTextSize("Hello EQOA")
            ImGui.PopFont()

            ImGui.PushFont(times36)
            local w36, h36 = ImGui.CalcTextSize("Hello EQOA")
            ImGui.PopFont()

            _G.w20 = w20
            _G.h20 = h20
            _G.w36 = w36
            _G.h36 = h36
        "#).unwrap();
        bridge.invoke(|| host.run(&script1)).unwrap();
        let w20: f32 = host.lua.globals().get("w20").unwrap();
        let h20: f32 = host.lua.globals().get("h20").unwrap();
        let w36: f32 = host.lua.globals().get("w36").unwrap();
        let h36: f32 = host.lua.globals().get("h36").unwrap();
        drop(context.render_legacy());

        // 36px font must be strictly larger than 20px font
        assert!(
            h36 > h20,
            "36px font height ({h36}) must be strictly greater than 20px font height ({h20})"
        );
        assert!(
            w36 > w20,
            "36px font width ({w36}) must be strictly greater than 20px font width ({w20})"
        );
    }

    #[test]
    fn forge_fonts_reverse_unwind_preserves_healthy_neighbor() {
        let dir = temp_test_dir("unwind");
        let addon = test_addon(&dir);

        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        context.io_mut().set_display_size([800.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.font_atlas().try_claim_legacy_renderer().unwrap().build();

        let host = Host::for_addon(&addon).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let font_bank = Rc::new(RefCell::new(FontBank::new(1, current_context_epoch())));
        FontBank::install_for_addon(&font_bank, &host.lua, &addon).unwrap();

        context.frame();
        // Script pushes font and deliberately throws error without calling PopFont
        let bad_script = host.compile(r#"
            local font = UiForge.LoadFont("missing.ttf", 20.0)
            ImGui.PushFont(font)
            error("controlled error mid-font")
        "#).unwrap();
        let res = bridge.invoke(|| host.run(&bad_script));
        assert!(res.is_err());

        // Next healthy script in same frame must run cleanly
        let good_script = host.compile(r#"
            local font = UiForge.LoadFont("missing.ttf", 20.0)
            ImGui.PushFont(font)
            ImGui.PopFont()
            _G.good = true
        "#).unwrap();
        bridge.invoke(|| host.run(&good_script)).unwrap();
        assert_eq!(host.lua.globals().get::<bool>("good").unwrap(), true);
        drop(context.render_legacy());
    }

    #[test]
    fn forge_fonts_stale_epoch_handle_rejected() {
        let dir = temp_test_dir("stale_epoch");
        let addon = test_addon(&dir);

        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        context.io_mut().set_display_size([800.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.font_atlas().try_claim_legacy_renderer().unwrap().build();

        let host = Host::for_addon(&addon).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let font_bank = Rc::new(RefCell::new(FontBank::new(1, current_context_epoch())));
        FontBank::install_for_addon(&font_bank, &host.lua, &addon).unwrap();

        context.frame();
        let script = host.compile(r#"
            font = UiForge.LoadFont("missing.ttf", 20.0)
            return font
        "#).unwrap();
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());
        drop(context);

        // Context is destroyed and new context created, bumping epoch
        bump_context_epoch();
        let mut new_context = Context::create();
        new_context.set_ini_filename::<PathBuf>(None).unwrap();
        new_context.io_mut().set_display_size([800.0, 600.0]);
        new_context.io_mut().set_delta_time(1.0 / 60.0);
        new_context.font_atlas().try_claim_legacy_renderer().unwrap().build();

        new_context.frame();
        let stale_script = host.compile(r#"
            ImGui.PushFont(font)
        "#).unwrap();
        let err = bridge.invoke(|| host.run(&stale_script)).unwrap_err();
        assert!(err.to_string().contains("stale or foreign font handle"));
        drop(new_context.render_legacy());
    }

    #[test]
    fn forge_fonts_set_window_font_scale() {
        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        context.io_mut().set_display_size([800.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context.font_atlas().try_claim_legacy_renderer().unwrap().build();

        let host = Host::new().unwrap();
        let bridge = Bridge::install(&host).unwrap();

        context.frame();
        let script = host.compile(r#"
            assert(ImGui.Begin("ScaleTest"))
            ImGui.SetWindowFontScale(1.5)
            ImGui.Text("Scaled text")
            ImGui.End()
        "#).unwrap();
        assert!(bridge.invoke(|| host.run(&script)).is_ok());

        // Invalid non-positive or non-finite scale rejected
        let bad = host.compile(r#"
            assert(ImGui.Begin("BadScale"))
            ImGui.SetWindowFontScale(0.0)
            ImGui.End()
        "#).unwrap();
        assert!(bridge.invoke(|| host.run(&bad)).is_err());
        drop(context.render_legacy());
    }
}
