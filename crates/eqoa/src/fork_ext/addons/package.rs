//! Isolated per-addon package loader and resource resolver for Forge addons.

use super::discovery::DiscoveredAddon;
use mlua::chunk::ChunkMode;
use mlua::{Lua, Result as LuaResult, Table, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResourceError {
    InvalidPath(String),
    NotFound(String),
    EscapesScriptRoot(PathBuf),
    NotAFile(PathBuf),
    BrokenFile(String),
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(msg) => write!(f, "invalid resource path: {msg}"),
            Self::NotFound(msg) => write!(f, "resource not found: {msg}"),
            Self::EscapesScriptRoot(p) => {
                write!(f, "resource path escapes script root: {}", p.display())
            }
            Self::NotAFile(p) => write!(f, "resource path is not a file: {}", p.display()),
            Self::BrokenFile(msg) => write!(f, "broken resource candidate: {msg}"),
        }
    }
}

impl std::error::Error for ResourceError {}

pub(crate) fn resolve_resource(
    addon: &DiscoveredAddon,
    relative_path: &str,
) -> std::result::Result<PathBuf, ResourceError> {
    if relative_path.is_empty() {
        return Err(ResourceError::InvalidPath(
            "resource path cannot be empty".to_string(),
        ));
    }

    // Reject drive letters, UNC paths, and leading separators on any OS
    if relative_path.contains(':')
        || relative_path.starts_with('/')
        || relative_path.starts_with('\\')
    {
        return Err(ResourceError::InvalidPath(format!(
            "absolute or drive path rejected: {relative_path}"
        )));
    }

    // Split components using either '/' or '\' and check for traversal
    let normalized = relative_path.replace('\\', "/");
    for seg in normalized.split('/') {
        if seg == ".." {
            return Err(ResourceError::InvalidPath(format!(
                "parent traversal '..' rejected: {relative_path}"
            )));
        }
        if seg.is_empty() {
            return Err(ResourceError::InvalidPath(format!(
                "empty path segment rejected: {relative_path}"
            )));
        }
    }

    for res_dir in &addon.resource_dirs {
        let candidate = res_dir.join(Path::new(&normalized));
        if candidate.exists() || candidate.is_symlink() {
            let canon = match candidate.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    return Err(ResourceError::BrokenFile(format!(
                        "failed to resolve candidate '{}': {e}",
                        candidate.display()
                    )));
                }
            };
            if !canon.starts_with(&addon.script_root) {
                return Err(ResourceError::EscapesScriptRoot(canon));
            }
            if !canon.is_file() {
                return Err(ResourceError::NotAFile(canon));
            }
            return Ok(canon);
        }
    }

    Err(ResourceError::NotFound(format!(
        "resource '{relative_path}' not found in candidate resource directories: {:?}",
        addon.resource_dirs
    )))
}

pub(crate) fn validate_module_name(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("module name cannot be empty".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.contains(':') {
        return Err(format!(
            "invalid module name '{name}': slashes and colons are forbidden"
        ));
    }
    if name.chars().any(|c| c.is_whitespace()) {
        return Err(format!(
            "invalid module name '{name}': whitespace is forbidden"
        ));
    }
    for seg in name.split('.') {
        if seg.is_empty() {
            return Err(format!(
                "invalid module name '{name}': empty segment in dotted path"
            ));
        }
        if seg == ".." {
            return Err(format!(
                "invalid module name '{name}': parent traversal forbidden"
            ));
        }
        if !seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "invalid module name '{name}': segment '{seg}' contains invalid characters"
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_bounded_source(
    path: &Path,
    max_bytes: usize,
) -> std::result::Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("failed to open '{}': {e}", path.display()))?;
    let mut buf = Vec::new();
    let bytes_read = (&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| format!("failed to read '{}': {e}", path.display()))?;
    if bytes_read > max_bytes {
        return Err(format!(
            "file '{}' exceeds size limit of {} bytes",
            path.display(),
            max_bytes
        ));
    }
    String::from_utf8(buf)
        .map_err(|e| format!("file '{}' contains invalid UTF-8: {e}", path.display()))
}

pub(crate) fn find_module_file(
    addon: &DiscoveredAddon,
    name: &str,
) -> std::result::Result<PathBuf, String> {
    let rel_slash = name.replace('.', "/");
    for dir in &addon.module_dirs {
        let cand1 = dir.join(format!("{rel_slash}.lua"));
        let cand2 = dir.join(format!("{rel_slash}/init.lua"));
        for cand in [cand1, cand2] {
            if cand.exists() || cand.is_symlink() {
                let canon = match cand.canonicalize() {
                    Ok(p) => p,
                    Err(e) => {
                        return Err(format!(
                            "failed to canonicalize module candidate '{}': {e}",
                            cand.display()
                        ));
                    }
                };
                if !canon.starts_with(&addon.script_root) {
                    return Err(format!(
                        "module '{name}' at '{}' escapes active script root",
                        canon.display()
                    ));
                }
                if canon.is_file() {
                    return Ok(canon);
                }
            }
        }
    }
    Err(format!(
        "module '{name}' not found in addon module search paths: {:?}",
        addon.module_dirs
    ))
}

struct LoadingGuard {
    stack: Rc<RefCell<Vec<String>>>,
}

impl LoadingGuard {
    fn enter(stack: Rc<RefCell<Vec<String>>>, name: String) -> Self {
        stack.borrow_mut().push(name);
        Self { stack }
    }
}

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        self.stack.borrow_mut().pop();
    }
}

pub(crate) fn install_package_loader(
    lua: &Lua,
    addon: &DiscoveredAddon,
    util_table: Table,
    bit_table: Table,
    os_table: Table,
    host_registry: Rc<RefCell<HashMap<String, Value>>>,
) -> LuaResult<()> {
    let package: Table = lua.globals().get("package")?;
    let package_loaded: Table = package.get("loaded")?;

    // Pre-populate frontiers_forge.util, bit, and os in package.loaded
    package_loaded.set("frontiers_forge.util", Value::Table(util_table.clone()))?;
    package_loaded.set("bit", Value::Table(bit_table.clone()))?;
    package_loaded.set("os", Value::Table(os_table.clone()))?;
    package_loaded.set("coroutine", Value::Nil)?;

    let addon_data = addon.clone();
    let host_reg = host_registry;
    let loading_stack = Rc::new(RefCell::new(Vec::<String>::new()));
    let sentinel = lua.create_table()?;
    sentinel.set("__loading_sentinel", true)?;

    let require_fn = lua.create_function(move |lua, name: String| {
        // 1. Validate module name
        validate_module_name(&name).map_err(mlua::Error::runtime)?;

        // 2. Reserved & Registered host modules
        // ffi is strictly forbidden, even if registered
        if name == "ffi" {
            return Err(mlua::Error::runtime("ffi module is unavailable"));
        }
        if name == "coroutine" {
            return Err(mlua::Error::runtime("coroutine module is unavailable"));
        }
        // Consult registered native modules first (e.g. imgui, uiforge, frontiers_forge.player, etc.)
        if let Some(val) = host_reg.borrow().get(&name) {
            return Ok(val.clone());
        }
        // Native modules take precedence over disk lookup
        if name == "frontiers_forge.util" {
            return Ok(Value::Table(util_table.clone()));
        }
        if name == "bit" {
            return Ok(Value::Table(bit_table.clone()));
        }
        if name == "os" {
            return Ok(Value::Table(os_table.clone()));
        }
        // Unknown frontiers_forge.* fails naming unsupported API
        if name.starts_with("frontiers_forge.") {
            return Err(mlua::Error::runtime(format!(
                "unsupported FrontiersForge API: '{name}'"
            )));
        }
        // imgui / uiforge without host registration
        if name == "imgui" || name == "uiforge" {
            return Err(mlua::Error::runtime(format!(
                "unsupported reserved module: '{name}'"
            )));
        }

        // 3. Query package.loaded
        let package: Table = lua.globals().get("package")?;
        let package_loaded: Table = package.get("loaded")?;
        let cached: Value = package_loaded.get(name.as_str())?;

        // Lua 5.1 truthy check: nil and false reload
        if cached != Value::Nil && cached != Value::Boolean(false) {
            if cached == Value::Table(sentinel.clone()) {
                let mut chain = loading_stack.borrow().clone();
                chain.push(name.clone());
                return Err(mlua::Error::runtime(format!(
                    "require cycle detected: {}",
                    chain.join(" -> ")
                )));
            }
            return Ok(cached);
        }

        // 4. Nesting depth limit (32)
        if loading_stack.borrow().len() >= 32 {
            return Err(mlua::Error::runtime(
                "module require nesting depth exceeded limit (32)",
            ));
        }

        // 5. Search candidate module files
        let file_path = find_module_file(&addon_data, &name).map_err(mlua::Error::runtime)?;

        // 6. Bounded file read (1 MiB)
        let source = read_bounded_source(&file_path, 1024 * 1024).map_err(mlua::Error::runtime)?;

        // 7. Enter loading guard (guarantees stack unwind on ANY error or early exit)
        let _guard = LoadingGuard::enter(loading_stack.clone(), name.clone());
        package_loaded.set(name.as_str(), Value::Table(sentinel.clone()))?;

        // 8. Compile chunk (Mode::Text strictly rejects bytecode)
        let chunk_res = lua
            .load(&source)
            .set_name(file_path.to_string_lossy().to_string())
            .set_mode(ChunkMode::Text)
            .into_function();

        let func = match chunk_res {
            Ok(f) => f,
            Err(e) => {
                if package_loaded
                    .get::<Value>(name.as_str())
                    .map_or(false, |v| v == Value::Table(sentinel.clone()))
                {
                    let _ = package_loaded.set(name.as_str(), Value::Nil);
                }
                return Err(e);
            }
        };

        // 9. Execute chunk passing module name as '...'
        let call_res = func.call::<Value>(name.as_str());

        match call_res {
            Ok(res) => {
                let current: Value = package_loaded.get(name.as_str())?;
                // Lua 5.1 loadlib.c: Non-nil loader return overwrites package.loaded;
                // Otherwise if untouched sentinel, set to true; otherwise retain module-set value (including explicit nil/false).
                let final_val = if res != Value::Nil {
                    res
                } else if current == Value::Table(sentinel.clone()) {
                    Value::Boolean(true)
                } else {
                    current
                };
                package_loaded.set(name.as_str(), final_val.clone())?;
                Ok(final_val)
            }
            Err(e) => {
                if package_loaded
                    .get::<Value>(name.as_str())
                    .map_or(false, |v| v == Value::Table(sentinel.clone()))
                {
                    let _ = package_loaded.set(name.as_str(), Value::Nil);
                }
                Err(e)
            }
        }
    })?;

    lua.globals().set("require", require_fn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::discover;
    use crate::fork_ext::addons::host::Host;
    use std::fs;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("eqoa_pkg_{}_{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn create_dir_symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(src, dst)
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(src, dst)
        }
        #[cfg(not(any(windows, unix)))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "unsupported",
            ))
        }
    }

    fn is_privilege_error(err: &std::io::Error) -> bool {
        err.kind() == std::io::ErrorKind::PermissionDenied || err.raw_os_error() == Some(1314)
    }

    #[test]
    fn forge_package_two_addons_requiring_same_config_independently() {
        let temp = TempDir::new("two_addons_isolated");
        let root = temp.path();

        // Shared module config.lua
        fs::create_dir_all(root.join("modules")).unwrap();
        fs::write(
            root.join("modules").join("config.lua"),
            "return { value = 42 }",
        )
        .unwrap();

        // Addon 1
        let addon1_dir = root.join("addon_one");
        fs::create_dir_all(&addon1_dir).unwrap();
        fs::write(
            addon1_dir.join("addon_one.lua"),
            r#"
            local c = require("config")
            assert(c.value == 42)
            c.value = 100
            mutated_val = c.value
            "#,
        )
        .unwrap();

        // Addon 2
        let addon2_dir = root.join("addon_two");
        fs::create_dir_all(&addon2_dir).unwrap();
        fs::write(
            addon2_dir.join("addon_two.lua"),
            r#"
            local c = require("config")
            -- Must still be 42 in its isolated VM!
            assert(c.value == 42)
            isolated_val = c.value
            "#,
        )
        .unwrap();

        let disc = discover(root);
        assert_eq!(disc.addons.len(), 2);
        let a1 = disc.addons.iter().find(|a| a.id == "addon_one").unwrap();
        let a2 = disc.addons.iter().find(|a| a.id == "addon_two").unwrap();

        let host1 = Host::for_addon(a1).unwrap();
        host1.run_addon(a1).unwrap();
        assert_eq!(host1.lua.globals().get::<i32>("mutated_val").unwrap(), 100);

        let host2 = Host::for_addon(a2).unwrap();
        host2.run_addon(a2).unwrap();
        assert_eq!(host2.lua.globals().get::<i32>("isolated_val").unwrap(), 42);
        // Addon 2 global table must not have mutated_val from Addon 1
        assert_eq!(
            host2.lua.globals().get::<Value>("mutated_val").unwrap(),
            Value::Nil
        );
    }

    #[test]
    fn forge_package_private_shared_precedence_and_init_fallback() {
        let temp = TempDir::new("precedence_and_init");
        let root = temp.path();

        // Shared modules: theme.lua and widgets/init.lua
        fs::create_dir_all(root.join("modules").join("widgets")).unwrap();
        fs::write(
            root.join("modules").join("theme.lua"),
            "return 'shared_theme'",
        )
        .unwrap();
        fs::write(
            root.join("modules").join("widgets").join("init.lua"),
            "return 'shared_widgets_init'",
        )
        .unwrap();

        // Packaged addon with private modules/theme.lua
        let pkg = root.join("my_pack");
        fs::create_dir_all(pkg.join("modules")).unwrap();
        fs::write(
            pkg.join("modules").join("theme.lua"),
            "return 'private_theme'",
        )
        .unwrap();
        fs::write(
            pkg.join("my_pack.lua"),
            r#"
            t = require("theme")
            w = require("widgets")
            "#,
        )
        .unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "my_pack").unwrap();
        let host = Host::for_addon(addon).unwrap();
        host.run_addon(addon).unwrap();

        // Private shadows shared theme
        assert_eq!(
            host.lua.globals().get::<String>("t").unwrap(),
            "private_theme"
        );
        // Init fallback resolves widgets/init.lua
        assert_eq!(
            host.lua.globals().get::<String>("w").unwrap(),
            "shared_widgets_init"
        );
    }

    #[test]
    fn forge_package_cached_mutable_module_survives_frames() {
        let temp = TempDir::new("cached_mutable");
        let root = temp.path();

        fs::create_dir_all(root.join("modules")).unwrap();
        fs::write(
            root.join("modules").join("state.lua"),
            "return { count = 0 }",
        )
        .unwrap();
        fs::write(
            root.join("frame_mod.lua"),
            r#"
            local s = require("state")
            s.count = s.count + 1
            "#,
        )
        .unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // Simulate frame 1
        host.run_addon(addon).unwrap();
        // Simulate frame 2
        host.run_addon(addon).unwrap();

        let count: i32 = host
            .lua
            .load("return require('state').count")
            .eval()
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn forge_package_nil_and_false_reload_standard_behavior() {
        let temp = TempDir::new("nil_false_reload");
        let root = temp.path();

        fs::create_dir_all(root.join("modules")).unwrap();
        fs::write(
            root.join("modules").join("nil_mod.lua"),
            "calls = (calls or 0) + 1; return nil",
        )
        .unwrap();
        fs::write(root.join("main.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // 1. First require returns true and sets package.loaded to true
        let res1: Value = host.lua.load("return require('nil_mod')").eval().unwrap();
        assert_eq!(res1, Value::Boolean(true));
        let loaded_val: Value = host
            .lua
            .load("return package.loaded['nil_mod']")
            .eval()
            .unwrap();
        assert_eq!(loaded_val, Value::Boolean(true));
        let calls1: i32 = host.lua.globals().get("calls").unwrap();
        assert_eq!(calls1, 1);

        // 2. Second require uses cached true value, calls stays 1
        let res2: Value = host.lua.load("return require('nil_mod')").eval().unwrap();
        assert_eq!(res2, Value::Boolean(true));
        let calls2: i32 = host.lua.globals().get("calls").unwrap();
        assert_eq!(calls2, 1);

        // 3. Setting package.loaded to false triggers reload in standard Lua 5.1
        host.lua
            .load("package.loaded['nil_mod'] = false")
            .exec()
            .unwrap();
        let res3: Value = host.lua.load("return require('nil_mod')").eval().unwrap();
        assert_eq!(res3, Value::Boolean(true));
        let calls3: i32 = host.lua.globals().get("calls").unwrap();
        assert_eq!(calls3, 2);
    }

    #[test]
    fn forge_package_cycles_and_error_retry() {
        let temp = TempDir::new("cycles_and_retry");
        let root = temp.path();

        fs::create_dir_all(root.join("modules")).unwrap();
        fs::write(
            root.join("modules").join("cycle_a.lua"),
            "require('cycle_b')",
        )
        .unwrap();
        fs::write(
            root.join("modules").join("cycle_b.lua"),
            "require('cycle_a')",
        )
        .unwrap();

        let flaky_path = root.join("modules").join("flaky.lua");
        fs::write(&flaky_path, "error('intentional runtime failure')").unwrap();

        fs::write(root.join("main.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // 1. Cycle detection
        let cycle_err = host
            .lua
            .load("require('cycle_a')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            cycle_err.contains("require cycle detected: cycle_a -> cycle_b -> cycle_a"),
            "expected cycle chain, got: {cycle_err}"
        );

        // 2. Flaky error cleans loading marker and retries cleanly
        let err1 = host.lua.load("require('flaky')").exec().unwrap_err();
        assert!(err1.to_string().contains("intentional runtime failure"));
        // package.loaded['flaky'] must be nil after failure
        let loaded_flaky: Value = host
            .lua
            .load("return package.loaded['flaky']")
            .eval()
            .unwrap();
        assert_eq!(loaded_flaky, Value::Nil);

        // Update flaky.lua to succeed
        fs::write(&flaky_path, "return 'recovered'").unwrap();
        let res_retry: String = host.lua.load("return require('flaky')").eval().unwrap();
        assert_eq!(res_retry, "recovered");
    }

    #[test]
    fn forge_package_reserved_host_precedence_unsupported_reserved_ffi_bytecode_and_path_rejection()
    {
        let temp = TempDir::new("reserved_and_rejections");
        let root = temp.path();

        fs::create_dir_all(root.join("modules").join("frontiers_forge")).unwrap();
        // Deceptive autocomplete/mock files on disk
        fs::write(
            root.join("modules")
                .join("frontiers_forge")
                .join("util.lua"),
            "error('mock util executed')",
        )
        .unwrap();
        fs::write(
            root.join("modules").join("imgui.lua"),
            "error('mock imgui executed')",
        )
        .unwrap();

        // Bytecode file
        fs::write(
            root.join("modules").join("bytecode_mod.lua"),
            b"\x1bLua\x51\x00\x01\x04\x08\x04\x08\x00",
        )
        .unwrap();

        fs::write(root.join("main.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // 1. frontiers_forge.util returns native table, ignoring mock util.lua on disk
        let is_in_game: i32 = host
            .lua
            .load("return require('frontiers_forge.util').IsInGame()")
            .eval()
            .unwrap();
        assert_eq!(is_in_game, 0);

        // 2. Unsupported frontiers_forge.* fails naming unsupported API
        let ff_err = host
            .lua
            .load("require('frontiers_forge.inventory')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            ff_err.contains("unsupported FrontiersForge API: 'frontiers_forge.inventory'"),
            "got: {ff_err}"
        );

        // 3. imgui without host module fails naming unsupported reserved module (ignores disk imgui.lua)
        let imgui_err = host
            .lua
            .load("require('imgui')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            imgui_err.contains("unsupported reserved module: 'imgui'"),
            "got: {imgui_err}"
        );

        // 4. Exposing host module seam for imgui works
        let custom_imgui = host.lua.create_table().unwrap();
        custom_imgui.set("version", "1.91").unwrap();
        host.register_host_module("imgui", Value::Table(custom_imgui))
            .unwrap();
        let imgui_ver: String = host
            .lua
            .load("return require('imgui').version")
            .eval()
            .unwrap();
        assert_eq!(imgui_ver, "1.91");

        // 5. ffi is unavailable
        let ffi_err = host
            .lua
            .load("require('ffi')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            ffi_err.contains("ffi module is unavailable"),
            "got: {ffi_err}"
        );

        // 6. Bytecode chunk rejected
        let bc_err = host
            .lua
            .load("require('bytecode_mod')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            bc_err.contains("syntax error") || bc_err.contains("cannot load"),
            "got: {bc_err}"
        );

        // 7. Path traversal and malformed module names rejected
        for bad_name in [
            "../escape",
            "foo/bar",
            "foo\\bar",
            "foo..bar",
            ".leading",
            "trailing.",
        ] {
            let p_err = host
                .lua
                .load(format!("require('{bad_name}')"))
                .exec()
                .unwrap_err()
                .to_string();
            assert!(
                p_err.contains("invalid module name"),
                "expected invalid module name for '{bad_name}', got: {p_err}"
            );
        }
    }

    #[test]
    fn forge_package_nested_module_runaway_stops_original_budget() {
        let temp = TempDir::new("runaway_budget");
        let root = temp.path();

        fs::create_dir_all(root.join("modules")).unwrap();
        fs::write(
            root.join("modules").join("runaway.lua"),
            "while true do end",
        )
        .unwrap();

        fs::write(root.join("main.lua"), "require('runaway')").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        let err = host.run_addon(addon).unwrap_err().to_string();
        assert!(
            err.contains("instruction budget exceeded"),
            "expected budget error, got: {err}"
        );

        // Ensure host is not permanently poisoned for subsequent calls
        host.lua.globals().set("answer", 123).unwrap();
        assert_eq!(host.lua.globals().get::<i32>("answer").unwrap(), 123);
    }

    #[test]
    fn forge_package_resource_precedence_and_containment() {
        let temp = TempDir::new("resource_resolution");
        let root = temp.path();

        // Shared resources
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("resources").join("icon.png"), "shared_icon_bytes").unwrap();
        fs::write(
            root.join("resources").join("shared_data.bin"),
            "shared_data_bytes",
        )
        .unwrap();

        // Packaged addon with private resources
        let pkg = root.join("res_pack");
        fs::create_dir_all(pkg.join("resources")).unwrap();
        fs::write(pkg.join("resources").join("icon.png"), "private_icon_bytes").unwrap();
        fs::write(pkg.join("res_pack.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "res_pack").unwrap();
        let host = Host::for_addon(addon).unwrap();

        // 1. Private precedence: icon.png resolves to private directory
        let icon_path = host.resolve_resource("icon.png").unwrap();
        let icon_content = fs::read_to_string(&icon_path).unwrap();
        assert_eq!(icon_content, "private_icon_bytes");

        // 2. Fallback to shared resource: shared_data.bin
        let shared_path = host.resolve_resource("shared_data.bin").unwrap();
        let shared_content = fs::read_to_string(&shared_path).unwrap();
        assert_eq!(shared_content, "shared_data_bytes");

        // 3. Not found
        let not_found_err = host.resolve_resource("nonexistent.png").unwrap_err();
        assert!(matches!(not_found_err, ResourceError::NotFound(_)));

        // 4. Invalid paths: drive letters, colons, traversal, absolute
        for bad in [
            "../escape.txt",
            "C:\\bad.png",
            "/abs.png",
            "\\abs.png",
            "foo/../bar.txt",
        ] {
            let inv_err = host.resolve_resource(bad).unwrap_err();
            assert!(
                matches!(inv_err, ResourceError::InvalidPath(_)),
                "expected InvalidPath for '{bad}', got: {inv_err:?}"
            );
        }

        // 5. Symlink containment check
        let outside = TempDir::new("outside_res");
        let outside_file = outside.path().join("outside.png");
        fs::write(&outside_file, "outside").unwrap();

        if let Err(e) = create_dir_symlink(outside.path(), &pkg.join("resources").join("escaped")) {
            if is_privilege_error(&e) {
                eprintln!("SKIP symlink test: privilege not held ({e})");
                return;
            }
            panic!("symlink_dir failed: {e}");
        }

        let esc_err = host.resolve_resource("escaped/outside.png").unwrap_err();
        assert!(
            matches!(esc_err, ResourceError::EscapesScriptRoot(_)),
            "expected EscapesScriptRoot, got: {esc_err:?}"
        );
    }

    #[test]
    fn forge_package_registered_host_submodules_precedence_and_ffi_lockdown() {
        let temp = TempDir::new("host_submodules");
        let root = temp.path();

        fs::create_dir_all(root.join("modules").join("frontiers_forge")).unwrap();
        fs::write(
            root.join("modules")
                .join("frontiers_forge")
                .join("player.lua"),
            "error('deceptive disk player executed')",
        )
        .unwrap();
        fs::write(root.join("main.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // 1. Register native player table under frontiers_forge.player
        let player_tbl = host.lua.create_table().unwrap();
        player_tbl.set("name", "NativePlayer").unwrap();
        host.register_host_module("frontiers_forge.player", Value::Table(player_tbl))
            .unwrap();

        // 2. Requiring frontiers_forge.player returns registered table, beating deceptive disk file
        let player_name: String = host
            .lua
            .load("return require('frontiers_forge.player').name")
            .eval()
            .unwrap();
        assert_eq!(player_name, "NativePlayer");

        // 3. ffi remains strictly unavailable even if someone tries registering it
        let fake_ffi = host.lua.create_table().unwrap();
        fake_ffi.set("hacked", true).unwrap();
        let _ = host.register_host_module("ffi", Value::Table(fake_ffi));
        let ffi_err = host
            .lua
            .load("require('ffi')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            ffi_err.contains("ffi module is unavailable"),
            "got: {ffi_err}"
        );

        // 4. Unknown reserved module still fails naming unsupported API
        let unk_err = host
            .lua
            .load("require('frontiers_forge.unknown_submodule')")
            .exec()
            .unwrap_err()
            .to_string();
        assert!(
            unk_err.contains("unsupported FrontiersForge API: 'frontiers_forge.unknown_submodule'"),
            "got: {unk_err}"
        );
    }

    #[test]
    fn forge_package_return_overwrites_package_loaded_and_explicit_nil_preserves_nil() {
        let temp = TempDir::new("return_overwrites");
        let root = temp.path();

        fs::create_dir_all(root.join("modules")).unwrap();

        // 1. Module assigns package.loaded[...] = { v = 1 } then returns { v = 2 }
        fs::write(
            root.join("modules").join("override_mod.lua"),
            "package.loaded[...] = { v = 1 }; return { v = 2 }",
        )
        .unwrap();

        // 2. Module assigns package.loaded[...] = nil then returns nil
        fs::write(
            root.join("modules").join("explicit_nil_mod.lua"),
            "package.loaded[...] = nil; return nil",
        )
        .unwrap();

        // 3. Module returns nil without touching package.loaded
        fs::write(
            root.join("modules").join("untouched_nil_mod.lua"),
            "return nil",
        )
        .unwrap();

        fs::write(root.join("main.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // Check 1: Return value must overwrite intermediate package.loaded value
        let val1: i32 = host
            .lua
            .load("return require('override_mod').v")
            .eval()
            .unwrap();
        assert_eq!(
            val1, 2,
            "non-nil loader return must overwrite package.loaded"
        );
        let cached1: i32 = host
            .lua
            .load("return package.loaded['override_mod'].v")
            .eval()
            .unwrap();
        assert_eq!(cached1, 2);

        // Check 2: Stock Lua 5.1 loadlib.c: explicit nil in package.loaded + nil return returns nil
        let val2: Value = host
            .lua
            .load("return require('explicit_nil_mod')")
            .eval()
            .unwrap();
        assert_eq!(
            val2,
            Value::Nil,
            "explicit nil in package.loaded + nil return must return nil"
        );
        let cached2: Value = host
            .lua
            .load("return package.loaded['explicit_nil_mod']")
            .eval()
            .unwrap();
        assert_eq!(cached2, Value::Nil);

        // Check 3: Untouched sentinel + nil return becomes true
        let val3: Value = host
            .lua
            .load("return require('untouched_nil_mod')")
            .eval()
            .unwrap();
        assert_eq!(val3, Value::Boolean(true));
        let cached3: Value = host
            .lua
            .load("return package.loaded['untouched_nil_mod']")
            .eval()
            .unwrap();
        assert_eq!(cached3, Value::Boolean(true));
    }

    #[test]
    fn forge_package_hostile_metatable_does_not_poison_loading_stack() {
        let temp = TempDir::new("stack_poison");
        let root = temp.path();

        fs::create_dir_all(root.join("modules")).unwrap();
        fs::write(root.join("modules").join("valid_mod.lua"), "return 'valid'").unwrap();
        fs::write(root.join("modules").join("fail_store.lua"), "return 'fail'").unwrap();
        fs::write(root.join("main.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();

        // Set metatable on package.loaded that errors when 'fail_store' is set
        host.lua
            .load(
                "local orig_loaded = package.loaded
                 local mt = {
                     __newindex = function(t, k, v)
                         if k == 'fail_store' then
                             error('hostile store failure on ' .. tostring(k))
                         else
                             rawset(t, k, v)
                         end
                     end
                 }
                 setmetatable(orig_loaded, mt)
",
            )
            .exec()
            .unwrap();

        // Attempt require('fail_store') 40 times (exceeding 32 limit if poisoned)
        for _ in 0..40 {
            let err = host.lua.load("require('fail_store')").exec().unwrap_err();
            assert!(
                err.to_string().contains("hostile store failure"),
                "expected hostile error, got: {err}"
            );
        }

        // Requiring valid_mod must succeed without 'nesting depth exceeded' or 'cycle detected'
        let valid_res: String = host.lua.load("return require('valid_mod')").eval().unwrap();
        assert_eq!(valid_res, "valid");
    }
    #[test]
    fn forge_package_require_shared_budget_fixture_cannot_exceed_limit() {
        let temp = TempDir::new("shared_budget");
        let root = temp.path();

        let pkg = root.join("heavy_addon");
        fs::create_dir_all(pkg.join("modules")).unwrap();
        // Submodule consumes 120,000 instructions
        fs::write(
            pkg.join("modules").join("heavy_worker.lua"),
            r#"
            for i = 1, 120000 do end
            return { done = true }
            "#,
        )
        .unwrap();
        // Main addon requires worker, then attempts another 120,000 instructions (exceeding 200,000 total)
        fs::write(
            pkg.join("heavy_addon.lua"),
            r#"
            completed = false
            local worker = require("heavy_worker")
            for i = 1, 120000 do end
            completed = true
            "#,
        )
        .unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "heavy_addon").unwrap();
        let host = Host::for_addon(addon).unwrap();
        let err = host.run_addon(addon).unwrap_err().to_string();
        assert!(err.contains("instruction budget exceeded"), "got: {err}");
        assert_eq!(
            host.lua.globals().get::<Option<bool>>("completed").unwrap(),
            Some(false)
        );
    }

    #[test]
    fn forge_package_require_bit_and_os_cannot_be_shadowed_by_disk() {
        let temp = TempDir::new("anti_shadow_test");
        let root = temp.path();

        let pkg = root.join("shadow_addon");
        fs::create_dir_all(pkg.join("modules")).unwrap();
        // Attempt to shadow 'bit' and 'os' via private module files
        fs::write(
            pkg.join("modules").join("bit.lua"),
            "return { shadow = 'bit' }",
        )
        .unwrap();
        fs::write(
            pkg.join("modules").join("os.lua"),
            "return { shadow = 'os' }",
        )
        .unwrap();
        fs::write(
            pkg.join("shadow_addon.lua"),
            r#"
            b = require("bit")
            o = require("os")
            "#,
        )
        .unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "shadow_addon").unwrap();
        let host = Host::for_addon(addon).unwrap();
        host.run_addon(addon).unwrap();

        // Native modules must be returned, NOT the private shadow files
        let b: Table = host.lua.globals().get("b").unwrap();
        assert!(matches!(
            b.get::<Value>("band").unwrap(),
            Value::Function(_)
        ));
        assert_eq!(b.get::<Value>("shadow").unwrap(), Value::Nil);

        let o: Table = host.lua.globals().get("o").unwrap();
        assert!(matches!(
            o.get::<Value>("clock").unwrap(),
            Value::Function(_)
        ));
        assert_eq!(o.get::<Value>("shadow").unwrap(), Value::Nil);
    }

    #[test]
    fn forge_package_table_identity_and_per_host_isolation() {
        let temp = TempDir::new("identity_isolation");
        let root = temp.path();
        let pkg = root.join("mod_pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("mod_pkg.lua"), "-- empty").unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "mod_pkg").unwrap();

        let host1 = Host::for_addon(addon).unwrap();
        let host2 = Host::for_addon(addon).unwrap();

        // 1. Within one host, require("bit") matches package.loaded["bit"]
        let b1: Table = host1.lua.load("return require('bit')").eval().unwrap();
        let b1_loaded: Table = host1
            .lua
            .load("return package.loaded['bit']")
            .eval()
            .unwrap();
        b1.set("marker", "host1_bit").unwrap();
        assert_eq!(b1_loaded.get::<String>("marker").unwrap(), "host1_bit");

        // 2. Across different hosts, states are strictly isolated
        let b2: Table = host2.lua.load("return require('bit')").eval().unwrap();
        assert_eq!(b2.get::<Value>("marker").unwrap(), Value::Nil);
    }
}
