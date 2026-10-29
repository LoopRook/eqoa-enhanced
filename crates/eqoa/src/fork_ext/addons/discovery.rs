//! Native-only addon discovery engine.
//! Discovers legacy loose and packaged Lua addons, imported child script trees,
//! and native package manifests adhering to schemaVersion: 1.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredAddon {
    pub id: String,
    pub name: String,
    pub entry: PathBuf,
    pub script_root: PathBuf,
    pub package_root: PathBuf,
    pub module_dirs: Vec<PathBuf>,
    pub resource_dirs: Vec<PathBuf>,
    pub api: Option<String>,
    pub requested_capabilities: Vec<String>,
    pub initially_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveryDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Discovery {
    pub addons: Vec<DiscoveredAddon>,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

fn is_support_or_hidden(name: &str) -> bool {
    name.starts_with('.')
        || name.eq_ignore_ascii_case("modules")
        || name.eq_ignore_ascii_case("resources")
        || name.eq_ignore_ascii_case("profiles")
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn deduplicate_dirs(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for dir in dirs {
        if seen.insert(dir.clone()) {
            out.push(dir);
        }
    }
    out
}

struct DiscoveryCtx {
    candidates: Vec<DiscoveredAddon>,
    diagnostics: Vec<DiscoveryDiagnostic>,
}

impl DiscoveryCtx {
    fn add_diagnostic(&mut self, path: PathBuf, message: impl Into<String>) {
        self.diagnostics.push(DiscoveryDiagnostic {
            path,
            message: message.into(),
        });
    }
}

fn resolve_support_dir(
    dir: &Path,
    canonical_root: &Path,
    label: &str,
    ctx: &mut DiscoveryCtx,
) -> Option<PathBuf> {
    if !dir.exists() && !dir.is_symlink() {
        return None;
    }

    match dir.canonicalize() {
        Ok(canon) => {
            if !canon.starts_with(canonical_root) {
                ctx.add_diagnostic(
                    dir.to_path_buf(),
                    format!(
                        "{label} directory escapes canonical root via symlink: {}",
                        canon.display()
                    ),
                );
                None
            } else if !canon.is_dir() {
                ctx.add_diagnostic(
                    dir.to_path_buf(),
                    format!("{label} path is not a directory: {}", canon.display()),
                );
                None
            } else {
                Some(canon)
            }
        }
        Err(e) => {
            ctx.add_diagnostic(
                dir.to_path_buf(),
                format!("Failed to resolve {label} directory: {e}"),
            );
            None
        }
    }
}

pub(crate) fn discover(root: &Path) -> Discovery {
    let mut ctx = DiscoveryCtx {
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };

    let canonical_root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            ctx.add_diagnostic(
                root.to_path_buf(),
                format!(
                    "Addon root directory does not exist or is unreadable: {}",
                    root.display()
                ),
            );
            return Discovery {
                addons: Vec::new(),
                diagnostics: ctx.diagnostics,
            };
        }
    };

    if !canonical_root.is_dir() {
        ctx.add_diagnostic(
            canonical_root,
            format!("Addon root path is not a directory: {}", root.display()),
        );
        return Discovery {
            addons: Vec::new(),
            diagnostics: ctx.diagnostics,
        };
    }

    // Discover directly within the root
    discover_script_root(&canonical_root, &canonical_root, &mut ctx);

    // Look for immediate child directories that contain a scripts/ folder (Layout 3: e.g. root/FrontiersForge/scripts/)
    if let Ok(entries) = fs::read_dir(&canonical_root) {
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    ctx.add_diagnostic(
                        canonical_root.clone(),
                        format!(
                            "Failed to read directory entry in {}: {err}",
                            canonical_root.display()
                        ),
                    );
                    continue;
                }
            };
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                if is_support_or_hidden(&dir_name) {
                    continue;
                }
                let scripts_subdir = path.join("scripts");
                if scripts_subdir.exists() || scripts_subdir.is_symlink() {
                    match scripts_subdir.canonicalize() {
                        Ok(canon_scripts) => {
                            if canon_scripts.starts_with(&canonical_root) {
                                if canon_scripts.is_dir() {
                                    discover_script_root(&canon_scripts, &canon_scripts, &mut ctx);
                                } else {
                                    ctx.add_diagnostic(
                                        scripts_subdir,
                                        "Script path is not a directory",
                                    );
                                }
                            } else {
                                ctx.add_diagnostic(
                                    scripts_subdir,
                                    "Script directory escapes canonical root via symlink",
                                );
                            }
                        }
                        Err(e) => {
                            ctx.add_diagnostic(
                                scripts_subdir,
                                format!("Failed to resolve script directory: {e}"),
                            );
                        }
                    }
                }
            }
        }
    }

    // Process duplicates: duplicate IDs compare case-insensitively across ALL roots.
    // Exclude ALL colliding candidates and report each.
    let mut id_map: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, addon) in ctx.candidates.iter().enumerate() {
        let key = addon.id.to_ascii_lowercase();
        id_map.entry(key).or_default().push(idx);
    }

    let mut valid_addons = Vec::new();
    for (key, indices) in id_map {
        if indices.len() > 1 {
            for &idx in &indices {
                let addon = &ctx.candidates[idx];
                ctx.diagnostics.push(DiscoveryDiagnostic {
                    path: addon.entry.clone(),
                    message: format!(
                        "Duplicate addon ID '{}' (case-insensitive collision for '{}')",
                        addon.id, key
                    ),
                });
            }
        } else {
            valid_addons.push(ctx.candidates[indices[0]].clone());
        }
    }

    // Deterministic sort: addons by id (case-insensitive, then exact), then entry path
    valid_addons.sort_by(|a, b| {
        a.id.to_ascii_lowercase()
            .cmp(&b.id.to_ascii_lowercase())
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.entry.cmp(&b.entry))
    });

    // Deterministic sort: diagnostics by path, then message
    ctx.diagnostics
        .sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.message.cmp(&b.message)));

    Discovery {
        addons: valid_addons,
        diagnostics: ctx.diagnostics,
    }
}

fn discover_script_root(script_root: &Path, canonical_root: &Path, ctx: &mut DiscoveryCtx) {
    let entries = match fs::read_dir(script_root) {
        Ok(e) => e,
        Err(err) => {
            ctx.add_diagnostic(
                script_root.to_path_buf(),
                format!(
                    "Failed to read script directory {}: {err}",
                    script_root.display()
                ),
            );
            return;
        }
    };

    let mut subdirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                ctx.add_diagnostic(
                    script_root.to_path_buf(),
                    format!(
                        "Failed to read directory entry in {}: {err}",
                        script_root.display()
                    ),
                );
                continue;
            }
        };
        let p = entry.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.is_file() {
            files.push(p);
        }
    }

    // Sort entries to process deterministically
    subdirs.sort();
    files.sort();

    // 1. Loose root/*.lua
    for file in files {
        if let Some(ext) = file.extension() {
            if ext == "lua" {
                let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                if is_support_or_hidden(&stem) {
                    continue;
                }
                let canonical_entry = match file.canonicalize() {
                    Ok(p) => p,
                    Err(e) => {
                        ctx.add_diagnostic(
                            file,
                            format!("Failed to canonicalize loose script: {e}"),
                        );
                        continue;
                    }
                };

                if !canonical_entry.starts_with(canonical_root) {
                    ctx.add_diagnostic(
                        file,
                        "Loose script file escapes canonical root via symlink",
                    );
                    continue;
                }

                let mut module_dirs = Vec::new();
                let mut resource_dirs = Vec::new();

                if let Some(m) = resolve_support_dir(
                    &script_root.join("modules"),
                    canonical_root,
                    "Shared modules",
                    ctx,
                ) {
                    module_dirs.push(m);
                }

                if let Some(r) = resolve_support_dir(
                    &script_root.join("resources"),
                    canonical_root,
                    "Shared resources",
                    ctx,
                ) {
                    resource_dirs.push(r);
                }

                ctx.candidates.push(DiscoveredAddon {
                    id: stem.to_string(),
                    name: stem.to_string(),
                    entry: canonical_entry,
                    script_root: script_root.to_path_buf(),
                    package_root: script_root.to_path_buf(),
                    module_dirs: deduplicate_dirs(module_dirs),
                    resource_dirs: deduplicate_dirs(resource_dirs),
                    api: None,
                    requested_capabilities: Vec::new(),
                    initially_enabled: false,
                });
            }
        }
    }

    // 2. Subdirectories: Native package or Legacy package
    for subdir in subdirs {
        let dir_name = subdir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if is_support_or_hidden(&dir_name) {
            continue;
        }

        // Layout 3 check: if this subdir itself contains scripts/, skip treating it as a package here.
        // It is handled by the outer child scripts/ loop.
        let scripts_sub = subdir.join("scripts");
        if scripts_sub.exists() || scripts_sub.is_symlink() {
            continue;
        }

        let canonical_pkg = match subdir.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                ctx.add_diagnostic(
                    subdir,
                    format!("Failed to canonicalize package directory: {e}"),
                );
                continue;
            }
        };

        if !canonical_pkg.starts_with(canonical_root) {
            ctx.add_diagnostic(
                subdir,
                "Package directory escapes canonical root via symlink",
            );
            continue;
        }

        let manifest_path = canonical_pkg.join("mod.json");
        if manifest_path.is_file() {
            // Layout 4: Native package manifest
            parse_native_manifest(
                &manifest_path,
                &canonical_pkg,
                script_root,
                canonical_root,
                ctx,
            );
        } else {
            // Layout 2: Legacy package directory (precedence: <package>.lua, main.lua, init.lua)
            parse_legacy_package(&canonical_pkg, script_root, canonical_root, &dir_name, ctx);
        }
    }
}

fn parse_native_manifest(
    manifest_path: &Path,
    package_root: &Path,
    script_root: &Path,
    canonical_root: &Path,
    ctx: &mut DiscoveryCtx,
) {
    let content = match fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(e) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Failed to read manifest mod.json: {e}"),
            );
            return;
        }
    };

    let raw: serde_json::Value = match serde_json::from_str(&content) {
        Ok(serde_json::Value::Object(m)) => serde_json::Value::Object(m),
        Ok(_) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Manifest mod.json must be a JSON object",
            );
            return;
        }
        Err(e) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Malformed manifest mod.json: {e}"),
            );
            return;
        }
    };

    // Check companion manifest: has command and NO runtime
    if raw.get("command").is_some() && raw.get("runtime").is_none() {
        // Companion manifest (e.g. parser). Skip cleanly without error.
        return;
    }

    // Check schemaVersion: must be integer 1
    match raw.get("schemaVersion") {
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(1) => {}
        Some(v) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Unsupported schemaVersion: {v}; expected 1"),
            );
            return;
        }
        None => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Manifest missing required 'schemaVersion' (expected 1)",
            );
            return;
        }
    }

    // Check runtime: must be "lua"
    match raw.get("runtime") {
        Some(serde_json::Value::String(r)) if r == "lua" => {}
        Some(r) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Unsupported manifest runtime: {r}; expected 'lua'"),
            );
            return;
        }
        None => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Manifest missing required 'runtime' field (expected 'lua')",
            );
            return;
        }
    }

    // Check api: must be "eqoa/1"
    match raw.get("api") {
        Some(serde_json::Value::String(a)) if a == "eqoa/1" => {}
        Some(a) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Unsupported manifest API: {a}; expected 'eqoa/1'"),
            );
            return;
        }
        None => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Manifest missing required 'api' field (expected 'eqoa/1')",
            );
            return;
        }
    }

    // Check id: nonempty ASCII letters, digits, _, ., -
    let id = match raw.get("id") {
        Some(serde_json::Value::String(s)) if is_valid_id(s) => s.clone(),
        Some(s) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Invalid manifest 'id': {s:?}; must be non-empty ASCII letters, digits, _, ., or -"),
            );
            return;
        }
        None => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Manifest missing required 'id' field",
            );
            return;
        }
    };

    // Check name: optional string, defaults to id
    let name = match raw.get("name") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Invalid manifest 'name': {v:?}; expected string"),
            );
            return;
        }
        None => id.clone(),
    };

    // Check entry: nonempty relative contained path
    let entry_str = match raw.get("entry") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => s,
        Some(_) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Invalid manifest 'entry': must be a non-empty relative string path",
            );
            return;
        }
        None => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                "Manifest missing required 'entry' field",
            );
            return;
        }
    };

    let entry_path = Path::new(entry_str);
    if entry_path.is_absolute()
        || entry_str.starts_with('/')
        || entry_str.starts_with('\\')
        || entry_str.contains(':')
    {
        ctx.add_diagnostic(
            manifest_path.to_path_buf(),
            format!("Manifest entry '{entry_str}' must be a relative path"),
        );
        return;
    }

    // Reject parent directory traversal
    for comp in entry_path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Manifest entry '{entry_str}' contains parent directory traversal ('..')"),
            );
            return;
        }
    }

    let full_entry = package_root.join(entry_path);
    let canonical_entry = match full_entry.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Manifest entry file '{entry_str}' could not be resolved: {e}"),
            );
            return;
        }
    };

    if !canonical_entry.starts_with(package_root) || !canonical_entry.starts_with(canonical_root) {
        ctx.add_diagnostic(
            manifest_path.to_path_buf(),
            format!("Manifest entry file '{entry_str}' escapes package root"),
        );
        return;
    }

    if !canonical_entry.is_file() {
        ctx.add_diagnostic(
            manifest_path.to_path_buf(),
            format!("Manifest entry '{entry_str}' is not a file"),
        );
        return;
    }

    // Capabilities
    let mut requested_capabilities = Vec::new();
    if let Some(caps) = raw.get("capabilities") {
        match caps {
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let serde_json::Value::String(s) = item {
                        requested_capabilities.push(s.clone());
                    } else {
                        ctx.add_diagnostic(
                            manifest_path.to_path_buf(),
                            "Manifest 'capabilities' must be an array of strings",
                        );
                        return;
                    }
                }
            }
            _ => {
                ctx.add_diagnostic(
                    manifest_path.to_path_buf(),
                    "Manifest 'capabilities' must be an array",
                );
                return;
            }
        }
    }

    // Enabled
    let initially_enabled = match raw.get("enabled") {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(v) => {
            ctx.add_diagnostic(
                manifest_path.to_path_buf(),
                format!("Invalid manifest 'enabled': {v:?}; expected boolean"),
            );
            return;
        }
        None => false,
    };

    // Module / Resource directories: private first, then shared script-root
    let mut module_dirs = Vec::new();
    let mut resource_dirs = Vec::new();

    if let Some(m) = resolve_support_dir(
        &package_root.join("modules"),
        canonical_root,
        "Package modules",
        ctx,
    ) {
        module_dirs.push(m);
    }
    if let Some(m) = resolve_support_dir(
        &script_root.join("modules"),
        canonical_root,
        "Shared modules",
        ctx,
    ) {
        module_dirs.push(m);
    }

    if let Some(r) = resolve_support_dir(
        &package_root.join("resources"),
        canonical_root,
        "Package resources",
        ctx,
    ) {
        resource_dirs.push(r);
    }
    if let Some(r) = resolve_support_dir(
        &script_root.join("resources"),
        canonical_root,
        "Shared resources",
        ctx,
    ) {
        resource_dirs.push(r);
    }

    ctx.candidates.push(DiscoveredAddon {
        id,
        name,
        entry: canonical_entry,
        script_root: script_root.to_path_buf(),
        package_root: package_root.to_path_buf(),
        module_dirs: deduplicate_dirs(module_dirs),
        resource_dirs: deduplicate_dirs(resource_dirs),
        api: Some("eqoa/1".to_string()),
        requested_capabilities,
        initially_enabled,
    });
}

fn parse_legacy_package(
    package_root: &Path,
    script_root: &Path,
    canonical_root: &Path,
    dir_name: &str,
    ctx: &mut DiscoveryCtx,
) {
    // Precedence: 1. <package>.lua, 2. main.lua, 3. init.lua
    let named_entry = package_root.join(format!("{dir_name}.lua"));
    let main_entry = package_root.join("main.lua");
    let init_entry = package_root.join("init.lua");

    let entry_file = if named_entry.is_file() {
        Some(named_entry)
    } else if main_entry.is_file() {
        Some(main_entry)
    } else if init_entry.is_file() {
        Some(init_entry)
    } else {
        None
    };

    let entry_file = match entry_file {
        Some(f) => f,
        None => return, // Not an addon directory
    };

    let canonical_entry = match entry_file.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            ctx.add_diagnostic(
                entry_file,
                format!("Failed to canonicalize package entry: {e}"),
            );
            return;
        }
    };

    if !canonical_entry.starts_with(package_root) || !canonical_entry.starts_with(canonical_root) {
        ctx.add_diagnostic(
            entry_file,
            "Package entry file escapes package root via symlink",
        );
        return;
    }

    let mut module_dirs = Vec::new();
    let mut resource_dirs = Vec::new();

    if let Some(m) = resolve_support_dir(
        &package_root.join("modules"),
        canonical_root,
        "Package modules",
        ctx,
    ) {
        module_dirs.push(m);
    }
    if let Some(m) = resolve_support_dir(
        &script_root.join("modules"),
        canonical_root,
        "Shared modules",
        ctx,
    ) {
        module_dirs.push(m);
    }

    if let Some(r) = resolve_support_dir(
        &package_root.join("resources"),
        canonical_root,
        "Package resources",
        ctx,
    ) {
        resource_dirs.push(r);
    }
    if let Some(r) = resolve_support_dir(
        &script_root.join("resources"),
        canonical_root,
        "Shared resources",
        ctx,
    ) {
        resource_dirs.push(r);
    }

    ctx.candidates.push(DiscoveredAddon {
        id: dir_name.to_string(),
        name: dir_name.to_string(),
        entry: canonical_entry,
        script_root: script_root.to_path_buf(),
        package_root: package_root.to_path_buf(),
        module_dirs: deduplicate_dirs(module_dirs),
        resource_dirs: deduplicate_dirs(resource_dirs),
        api: None,
        requested_capabilities: Vec::new(),
        initially_enabled: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("eqoa_test_{}_{}", name, std::process::id()));
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

    #[test]
    fn addon_discovery_missing_root_returns_diagnostic() {
        let temp = TempDir::new("missing_root");
        let non_existent = temp.path().join("does_not_exist");
        let discovery = discover(&non_existent);
        assert!(discovery.addons.is_empty());
        assert_eq!(discovery.diagnostics.len(), 1);
        assert!(
            discovery.diagnostics[0].message.contains("not exist")
                || discovery.diagnostics[0].message.contains("missing")
        );
    }

    #[test]
    fn addon_discovery_loose_file_precedence_and_shared_dirs() {
        let temp = TempDir::new("loose_and_package");
        let root = temp.path();

        // 1. Loose script
        fs::write(root.join("screen_grid.lua"), "-- screen grid").unwrap();
        // Shared modules and resources
        fs::create_dir_all(root.join("modules")).unwrap();
        fs::create_dir_all(root.join("resources")).unwrap();

        // 2. Package with <name>.lua
        let pkg1 = root.join("my_pack");
        fs::create_dir_all(&pkg1).unwrap();
        fs::write(pkg1.join("my_pack.lua"), "-- my pack").unwrap();
        fs::write(pkg1.join("main.lua"), "-- main fallback").unwrap(); // Precedence should pick my_pack.lua
        fs::create_dir_all(pkg1.join("modules")).unwrap();

        // 3. Package with main.lua
        let pkg2 = root.join("main_pack");
        fs::create_dir_all(&pkg2).unwrap();
        fs::write(pkg2.join("main.lua"), "-- main pack").unwrap();

        // 4. Package with init.lua
        let pkg3 = root.join("init_pack");
        fs::create_dir_all(&pkg3).unwrap();
        fs::write(pkg3.join("init.lua"), "-- init pack").unwrap();

        // Ignored support dirs
        let ignored_profiles = root.join("profiles");
        fs::create_dir_all(&ignored_profiles).unwrap();
        fs::write(ignored_profiles.join("profiles.lua"), "-- not a mod").unwrap();

        let disc = discover(root);
        assert!(
            disc.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            disc.diagnostics
        );
        assert_eq!(disc.addons.len(), 4);

        let ids: Vec<&str> = disc.addons.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["init_pack", "main_pack", "my_pack", "screen_grid"]
        );

        let my_pack = disc.addons.iter().find(|a| a.id == "my_pack").unwrap();
        assert!(my_pack.entry.ends_with("my_pack.lua"));
        assert_eq!(my_pack.module_dirs.len(), 2);
        assert!(
            my_pack.module_dirs[0].ends_with("my_pack/modules")
                || my_pack.module_dirs[0].ends_with("my_pack\\modules")
        );
        assert!(my_pack.module_dirs[1].ends_with("modules"));
        assert_eq!(my_pack.resource_dirs.len(), 1);
        assert!(my_pack.resource_dirs[0].ends_with("resources"));
    }

    #[test]
    fn addon_discovery_child_scripts_directory_imported() {
        let temp = TempDir::new("child_scripts");
        let root = temp.path();

        let ff_scripts = root.join("FrontiersForge").join("scripts");
        fs::create_dir_all(&ff_scripts).unwrap();
        fs::create_dir_all(ff_scripts.join("modules")).unwrap();
        fs::create_dir_all(ff_scripts.join("resources")).unwrap();
        fs::create_dir_all(ff_scripts.join("profiles")).unwrap();

        fs::write(ff_scripts.join("ff_example.lua"), "-- ff example").unwrap();
        let arpg = ff_scripts.join("arpg_ui");
        fs::create_dir_all(&arpg).unwrap();
        fs::write(arpg.join("arpg_ui.lua"), "-- arpg").unwrap();

        let disc = discover(root);
        assert!(
            disc.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            disc.diagnostics
        );
        assert_eq!(disc.addons.len(), 2);
        let ids: Vec<&str> = disc.addons.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["arpg_ui", "ff_example"]);
        assert_eq!(
            disc.addons[0].script_root,
            ff_scripts.canonicalize().unwrap()
        );
    }

    #[test]
    fn addon_discovery_native_manifest_success_and_companion_skip() {
        let temp = TempDir::new("manifests");
        let root = temp.path();

        // Valid native manifest
        let native_dir = root.join("native_mod");
        fs::create_dir_all(&native_dir).unwrap();
        fs::write(native_dir.join("entry.lua"), "-- entry").unwrap();
        fs::write(
            native_dir.join("mod.json"),
            r#"{
                "schemaVersion": 1,
                "runtime": "lua",
                "api": "eqoa/1",
                "id": "native_one",
                "name": "Native One Addon",
                "entry": "entry.lua",
                "capabilities": ["hud", "combat"],
                "enabled": true
            }"#,
        )
        .unwrap();

        // Existing companion manifest (parser): command, no runtime -> skipped cleanly
        let companion_dir = root.join("eqoa_parser");
        fs::create_dir_all(&companion_dir).unwrap();
        fs::write(
            companion_dir.join("mod.json"),
            r#"{
                "id": "eqoa_parser",
                "name": "EQOA Combat Parser",
                "command": "python",
                "args": ["-m", "eqoa_parser"]
            }"#,
        )
        .unwrap();

        let disc = discover(root);
        assert!(
            disc.diagnostics.is_empty(),
            "diagnostics: {:?}",
            disc.diagnostics
        );
        assert_eq!(disc.addons.len(), 1);
        let addon = &disc.addons[0];
        assert_eq!(addon.id, "native_one");
        assert_eq!(addon.name, "Native One Addon");
        assert_eq!(addon.api.as_deref(), Some("eqoa/1"));
        assert_eq!(addon.requested_capabilities, vec!["hud", "combat"]);
        assert!(addon.initially_enabled);
    }

    #[test]
    fn addon_discovery_manifest_diagnostics_and_path_traversal() {
        let temp = TempDir::new("manifest_errors");
        let root = temp.path();

        // 1. Malformed JSON
        let bad_json = root.join("bad_json");
        fs::create_dir_all(&bad_json).unwrap();
        fs::write(bad_json.join("mod.json"), "{ invalid json").unwrap();

        // 2. Unsupported schemaVersion
        let bad_schema = root.join("bad_schema");
        fs::create_dir_all(&bad_schema).unwrap();
        fs::write(
            bad_schema.join("mod.json"),
            r#"{"schemaVersion": 2, "runtime": "lua", "api": "eqoa/1", "id": "bad2", "entry": "e.lua"}"#,
        )
        .unwrap();

        // 3. Path traversal entry
        let bad_path = root.join("bad_path");
        fs::create_dir_all(&bad_path).unwrap();
        fs::write(
            bad_path.join("mod.json"),
            r#"{"schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": "bad3", "entry": "../outside.lua"}"#,
        )
        .unwrap();

        // 4. Valid neighbor
        fs::write(root.join("good_neighbor.lua"), "-- good").unwrap();

        let disc = discover(root);
        assert_eq!(disc.addons.len(), 1);
        assert_eq!(disc.addons[0].id, "good_neighbor");
        assert_eq!(disc.diagnostics.len(), 3);
    }

    #[test]
    fn addon_discovery_duplicate_ids_excluded() {
        let temp = TempDir::new("duplicates");
        let root = temp.path();

        // Duplicate case-insensitive IDs across roots
        fs::write(root.join("test_mod.lua"), "-- loose").unwrap();
        let pkg = root.join("Test_Mod");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("Test_Mod.lua"), "-- pkg").unwrap();

        // Valid neighbor
        fs::write(root.join("unique_mod.lua"), "-- unique").unwrap();

        let disc = discover(root);
        assert_eq!(disc.addons.len(), 1);
        assert_eq!(disc.addons[0].id, "unique_mod");
        // Both colliding candidates reported as diagnostics
        assert_eq!(disc.diagnostics.len(), 2);
        assert!(
            disc.diagnostics
                .iter()
                .all(|d| d.message.contains("Duplicate") || d.message.contains("collision"))
        );
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
    fn addon_discovery_dangling_and_escaping_support_dirs_emit_diagnostics() {
        let temp = TempDir::new("dangling_support");
        let root = temp.path();

        // Target directory outside root
        let outside = TempDir::new("outside_target");
        let outside_dir = outside.path().join("outside_modules");
        fs::create_dir_all(&outside_dir).unwrap();

        let pkg = root.join("escaping_pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("escaping_pkg.lua"), "-- script").unwrap();

        // 1. Symlinked modules escaping root
        if let Err(e) = create_dir_symlink(&outside_dir, &pkg.join("modules")) {
            if is_privilege_error(&e) {
                eprintln!("SKIP symlink test: privilege not held ({e})");
                return;
            }
            panic!("symlink_dir failed: {e}");
        }

        // 2. Dangling symlink for resources
        let nonexistent = root.join("nonexistent_target");
        if let Err(e) = create_dir_symlink(&nonexistent, &pkg.join("resources")) {
            if is_privilege_error(&e) {
                eprintln!("SKIP symlink test: privilege not held ({e})");
                return;
            }
            panic!("symlink_dir failed: {e}");
        }

        // 3. Valid neighbor
        fs::write(root.join("valid_neighbor.lua"), "-- valid").unwrap();

        let disc = discover(root);
        assert_eq!(disc.addons.len(), 2); // Both addons discovered, but escaping/dangling dirs omitted from dirs list
        let escaping_addon = disc.addons.iter().find(|a| a.id == "escaping_pkg").unwrap();
        assert!(
            escaping_addon.module_dirs.is_empty(),
            "escaping module dir must not be accepted"
        );
        assert!(
            escaping_addon.resource_dirs.is_empty(),
            "dangling resource dir must not be accepted"
        );

        // Diagnostics emitted for escaping modules and dangling resources
        assert!(
            disc.diagnostics.len() >= 2,
            "expected diagnostics for escaping and dangling symlinks, got: {:?}",
            disc.diagnostics
        );
        assert!(
            disc.diagnostics
                .iter()
                .any(|d| d.message.contains("escapes") || d.message.contains("symlink"))
        );
        assert!(
            disc.diagnostics
                .iter()
                .any(|d| d.message.contains("Failed to resolve")
                    || d.message.contains("canonicalize")
                    || d.message.contains("dangling")
                    || d.message.contains("unreadable"))
        );
    }

    #[test]
    fn addon_discovery_cross_scriptroot_symlink_stays_excluded() {
        let temp = TempDir::new("cross_scriptroot");
        let root = temp.path();

        // Child tree A
        let ff_a = root.join("TreeA").join("scripts");
        fs::create_dir_all(ff_a.join("modules")).unwrap();
        let pkg_a = ff_a.join("mod_a");
        fs::create_dir_all(&pkg_a).unwrap();
        fs::write(pkg_a.join("mod_a.lua"), "-- mod a").unwrap();

        // Child tree B with modules
        let ff_b = root.join("TreeB").join("scripts");
        let b_modules = ff_b.join("modules");
        fs::create_dir_all(&b_modules).unwrap();

        // Symlink in TreeA pointing to TreeB modules
        if let Err(e) = create_dir_symlink(&b_modules, &pkg_a.join("modules")) {
            if is_privilege_error(&e) {
                eprintln!("SKIP symlink test: privilege not held ({e})");
                return;
            }
            panic!("symlink_dir failed: {e}");
        }

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "mod_a").unwrap();
        // Cross-scriptroot link to TreeB must be excluded from TreeA's module_dirs!
        assert!(
            !addon.module_dirs.iter().any(|p| p.starts_with(&ff_b)),
            "cross-scriptroot module dir must stay excluded"
        );
        assert!(
            disc.diagnostics
                .iter()
                .any(|d| d.message.contains("escapes")),
            "diagnostic must be emitted for cross-scriptroot escape"
        );
    }

    #[test]
    fn addon_discovery_dangling_child_scripts_directory_emits_diagnostic() {
        let temp = TempDir::new("dangling_scripts");
        let root = temp.path();

        let child = root.join("BrokenTree");
        fs::create_dir_all(&child).unwrap();
        let nonexistent = temp.path().join("nowhere");
        if let Err(e) = create_dir_symlink(&nonexistent, &child.join("scripts")) {
            if is_privilege_error(&e) {
                eprintln!("SKIP symlink test: privilege not held ({e})");
                return;
            }
            panic!("symlink_dir failed: {e}");
        }

        // Valid neighbor survives
        fs::write(root.join("survivor.lua"), "-- survivor").unwrap();

        let disc = discover(root);
        assert_eq!(disc.addons.len(), 1);
        assert_eq!(disc.addons[0].id, "survivor");
        assert!(
            disc.diagnostics.iter().any(|d| d.path.ends_with("scripts")),
            "expected diagnostic for dangling scripts dir, got: {:?}",
            disc.diagnostics
        );
    }

    #[test]
    fn addon_discovery_legacy_names_with_spaces_and_unicode_preserved() {
        let temp = TempDir::new("legacy_opaque_names");
        let root = temp.path();

        // Legacy loose addon with Unicode
        fs::write(root.join("日本語_overlay.lua"), "-- japanese").unwrap();

        // Legacy packaged addon with spaces and symbols
        let pkg = root.join("My Custom UI (v2)");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("My Custom UI (v2).lua"), "-- custom ui").unwrap();

        let disc = discover(root);
        assert!(
            disc.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            disc.diagnostics
        );
        assert_eq!(disc.addons.len(), 2);
        let ids: Vec<&str> = disc.addons.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["My Custom UI (v2)", "日本語_overlay"]);
    }

    #[test]
    fn addon_discovery_manifest_schema_type_and_absolute_path_failures() {
        let temp = TempDir::new("schema_type_failures");
        let root = temp.path();

        // 1. Invalid schemaVersion string
        let p1 = root.join("bad_schema_str");
        fs::create_dir_all(&p1).unwrap();
        fs::write(p1.join("mod.json"), r#"{"schemaVersion": "1", "runtime": "lua", "api": "eqoa/1", "id": "p1", "entry": "e.lua"}"#).unwrap();

        // 2. Invalid runtime type (number)
        let p2 = root.join("bad_runtime_type");
        fs::create_dir_all(&p2).unwrap();
        fs::write(p2.join("mod.json"), r#"{"schemaVersion": 1, "runtime": 123, "api": "eqoa/1", "id": "p2", "entry": "e.lua"}"#).unwrap();

        // 3. Unknown runtime string ("python" with schemaVersion 1)
        let p3 = root.join("unknown_runtime");
        fs::create_dir_all(&p3).unwrap();
        fs::write(p3.join("mod.json"), r#"{"schemaVersion": 1, "runtime": "python", "api": "eqoa/1", "id": "p3", "entry": "e.lua"}"#).unwrap();

        // 4. Invalid API string
        let p4 = root.join("bad_api");
        fs::create_dir_all(&p4).unwrap();
        fs::write(p4.join("mod.json"), r#"{"schemaVersion": 1, "runtime": "lua", "api": "eqoa/2", "id": "p4", "entry": "e.lua"}"#).unwrap();

        // 5. Invalid capabilities (non-string element)
        let p5 = root.join("bad_caps");
        fs::create_dir_all(&p5).unwrap();
        fs::write(p5.join("entry.lua"), "-- e").unwrap();
        fs::write(p5.join("mod.json"), r#"{"schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": "p5", "entry": "entry.lua", "capabilities": [123]}"#).unwrap();

        // 6. Invalid native ID characters (spaces not allowed in native manifest ID)
        let p6 = root.join("bad_id");
        fs::create_dir_all(&p6).unwrap();
        fs::write(p6.join("entry.lua"), "-- e").unwrap();
        fs::write(p6.join("mod.json"), r#"{"schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": "bad id with spaces", "entry": "entry.lua"}"#).unwrap();

        // 7. Windows drive path entry
        let p7 = root.join("bad_drive_path");
        fs::create_dir_all(&p7).unwrap();
        fs::write(p7.join("mod.json"), r#"{"schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": "p7", "entry": "C:\\bad.lua"}"#).unwrap();

        // 8. UNC path entry
        let p8 = root.join("bad_unc_path");
        fs::create_dir_all(&p8).unwrap();
        fs::write(p8.join("mod.json"), r#"{"schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": "p8", "entry": "\\\\server\\bad.lua"}"#).unwrap();

        // Valid neighbor survives
        fs::write(root.join("survivor.lua"), "-- survivor").unwrap();

        let disc = discover(root);
        assert_eq!(disc.addons.len(), 1);
        assert_eq!(disc.addons[0].id, "survivor");
        assert_eq!(disc.diagnostics.len(), 8);
    }

    #[test]
    fn addon_discovery_hidden_and_support_dirs_skipped() {
        let temp = TempDir::new("hidden_dirs");
        let root = temp.path();

        // Dot-prefixed hidden directories
        let dot_git = root.join(".git");
        fs::create_dir_all(&dot_git).unwrap();
        fs::write(dot_git.join(".git.lua"), "-- git").unwrap();

        let dot_hidden = root.join(".hidden_addon");
        fs::create_dir_all(&dot_hidden).unwrap();
        fs::write(dot_hidden.join("main.lua"), "-- hidden").unwrap();

        // Support dirs
        let mods_dir = root.join("modules");
        fs::create_dir_all(&mods_dir).unwrap();
        fs::write(mods_dir.join("helper.lua"), "-- helper").unwrap();

        let res_dir = root.join("resources");
        fs::create_dir_all(&res_dir).unwrap();
        fs::write(res_dir.join("main.lua"), "-- res").unwrap();

        let prof_dir = root.join("profiles");
        fs::create_dir_all(&prof_dir).unwrap();
        fs::write(prof_dir.join("init.lua"), "-- profile").unwrap();

        let disc = discover(root);
        assert!(disc.addons.is_empty());
        assert!(disc.diagnostics.is_empty());
    }
}
