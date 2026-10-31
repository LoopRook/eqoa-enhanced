//! Drag-and-drop mod engine and companion manager for EQOA Native Rust.
//! Discovers mods beside the executable and manages companion tools
//! like the EQOA Combat Parser.

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Child, Command};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

/// One mod discovered in the `mods/` directory.
#[derive(Clone, Debug, PartialEq)]
pub struct ModManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub enabled: bool,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub path: PathBuf,
}

/// Active combat parser child process, if started through the client.
#[cfg(not(target_arch = "wasm32"))]
static RUNNING_PARSER: Mutex<Option<Child>> = Mutex::new(None);

/// Native mods always live beside the executable, even before the folder exists.
pub fn mods_dir() -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|exe| native_mods_dir(&exe))
        {
            Ok(path) => path,
            Err(error) => {
                crate::say(&format!("Cannot locate the client mods folder: {error}"));
                // An exceptional executable lookup failure must not select cwd
                // assets. Use an explicit private fallback and show the error.
                std::env::temp_dir().join("eqoa-mods-unavailable")
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    PathBuf::from("mods")
}

#[cfg(not(target_arch = "wasm32"))]
fn native_mods_dir(executable: &Path) -> Result<PathBuf, String> {
    if !executable.is_absolute() {
        return Err("executable path must be absolute".into());
    }
    let beside = executable
        .parent()
        .ok_or("executable has no parent")?
        .join("mods");
    Ok(beside)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod portable_root_tests {
    #[test]
    fn forge_mods_native_root_is_executable_relative_without_existing_folder() {
        let parent = std::env::temp_dir().join("eqoa-root-selector-nonexistent");
        assert_eq!(
            super::native_mods_dir(&parent.join("client.exe")).unwrap(),
            parent.join("mods")
        );
        assert!(super::native_mods_dir(std::path::Path::new("client.exe")).is_err());
    }
}

const MODS_README: &str = include_str!("../../../../mods/README.txt");

/// Ensures the `mods/` directory exists natively if missing.
/// Only when it was just created, writes a starter `README.txt` inside.
/// Never overwrites an existing `README.txt` or touches existing contents.
/// Failures (e.g. read-only folder) are logged once and ignored, never fatal.
pub fn auto_create_mods_dir() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static INITIALIZED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if INITIALIZED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let dir = mods_dir();
        auto_create_mods_dir_at(&dir);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn auto_create_mods_dir_at(dir: &Path) {
    if dir.exists() {
        return;
    }
    match fs::create_dir_all(dir) {
        Ok(()) => {
            let readme = dir.join("README.txt");
            if !readme.exists() {
                if let Err(e) = fs::write(&readme, MODS_README) {
                    crate::say(&format!("Could not write mods/README.txt: {e}"));
                }
            }
        }
        Err(e) => {
            crate::say(&format!(
                "Could not create mods directory {}: {e}",
                dir.display()
            ));
        }
    }
}

/// Ensures the `mods/` directory exists and creates default starter templates.
pub fn ensure_mods_dir() {
    auto_create_mods_dir();
}

/// Opens the `mods/` folder in the system file manager.
pub fn open_mods_folder() {
    ensure_mods_dir();
    let dir = mods_dir();
    #[cfg(windows)]
    {
        let _ = Command::new("explorer.exe").arg(&dir).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(&dir).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(&dir).spawn();
    }
}

/// Discovers all mods located in `mods/`.
pub fn list_mods() -> Vec<ModManifest> {
    ensure_mods_dir();
    let dir = mods_dir();
    let mut mods = Vec::new();

    let Ok(entries) = fs::read_dir(&dir) else {
        return mods;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let manifest_file = path.join("mod.json");

        if manifest_file.exists() {
            if let Ok(content) = fs::read_to_string(&manifest_file) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    let id = v
                        .get("id")
                        .and_then(|s| s.as_str())
                        .unwrap_or(folder_name)
                        .to_string();
                    let name = v
                        .get("name")
                        .and_then(|s| s.as_str())
                        .unwrap_or(folder_name)
                        .to_string();
                    let version = v
                        .get("version")
                        .and_then(|s| s.as_str())
                        .unwrap_or("1.0.0")
                        .to_string();
                    let author = v
                        .get("author")
                        .and_then(|s| s.as_str())
                        .unwrap_or("Community")
                        .to_string();
                    let description = v
                        .get("description")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    let enabled = v.get("enabled").and_then(|b| b.as_bool()).unwrap_or(true);
                    let command = v
                        .get("command")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    let args = v
                        .get("args")
                        .and_then(|a| a.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    mods.push(ModManifest {
                        id,
                        name,
                        version,
                        author,
                        description,
                        enabled,
                        command,
                        args,
                        path,
                    });
                    continue;
                }
            }
        }

        // Mod without mod.json: fallback representation
        mods.push(ModManifest {
            id: folder_name.to_string(),
            name: folder_name.replace('_', " "),
            version: "1.0.0".to_string(),
            author: "Local".to_string(),
            description: "Custom local mod package".to_string(),
            enabled: true,
            command: None,
            args: Vec::new(),
            path,
        });
    }

    mods
}

/// Checks the child owned by this client; a separate launcher owns its own child.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_parser_running() -> bool {
    let Ok(mut lock) = RUNNING_PARSER.lock() else {
        return false;
    };
    if let Some(child) = lock.as_mut() {
        if matches!(child.try_wait(), Ok(None)) {
            return true;
        }
        *lock = None;
    }
    false
}

#[cfg(target_arch = "wasm32")]
pub fn is_parser_running() -> bool {
    false
}

/// Compatibility for callers that only need success/failure.
pub fn launch_parser() -> bool {
    launch_parser_result().is_ok()
}

/// Launch exactly the manifest command and args, reporting configuration failures.
#[cfg(not(target_arch = "wasm32"))]
pub fn launch_parser_result() -> Result<(), String> {
    let mut lock = RUNNING_PARSER
        .lock()
        .map_err(|_| "Parser process lock is unavailable.".to_string())?;
    if let Some(child) = lock.as_mut() {
        if matches!(child.try_wait(), Ok(None)) {
            return Ok(());
        }
    }
    *lock = None;
    ensure_mods_dir();
    let manifest = mods_dir().join("eqoa_parser/mod.json");
    let plan = parser_plan(&manifest, std::env::var_os("EQOA_PARSER_DIR").as_deref())?;
    let mut command = Command::new(&plan.command);
    command.args(&plan.args).current_dir(&plan.cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW, including python.exe
    }
    let child = command.spawn().map_err(|e| {
        format!(
            "Cannot launch parser command {}: {e}",
            plan.command.display()
        )
    })?;
    *lock = Some(child);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn launch_parser_result() -> Result<(), String> {
    Err("Combat parser launching is available in the native client only.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct ParserLaunch {
    command: PathBuf,
    args: Vec<String>,
    cwd: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
fn parser_plan(
    manifest_path: &Path,
    configured: Option<&std::ffi::OsStr>,
) -> Result<ParserLaunch, String> {
    let manifest_path = fs::canonicalize(manifest_path).map_err(|e| {
        format!(
            "Cannot read parser manifest {}: {e}",
            manifest_path.display()
        )
    })?;
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Cannot read parser manifest: {e}"))?;
    let manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid parser manifest: {e}"))?;
    if manifest.get("id").and_then(|v| v.as_str()) != Some("eqoa_parser") {
        return Err("Parser manifest id must be eqoa_parser.".to_string());
    }
    if manifest
        .get("enabled")
        .is_some_and(|v| v.as_bool() != Some(true))
    {
        return Err("The parser is disabled in mod.json.".to_string());
    }
    let command = manifest
        .get("command")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Parser manifest command must be a nonempty string.".to_string())?;
    let args = match manifest.get("args") {
        None => Vec::new(),
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "Parser manifest args must be an array of strings.".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("Parser manifest args must be an array of strings.".to_string()),
    };
    let configured = configured
        .filter(|v| !v.to_string_lossy().trim().is_empty())
        .ok_or_else(|| {
            "Set EQOA_PARSER_DIR to the parser source or installation directory.".to_string()
        })?;
    let folder = manifest_path
        .parent()
        .ok_or_else(|| "Parser manifest has no directory.".to_string())?;
    let base = folder
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "Parser manifest is outside the mods directory.".to_string())?;
    let mut cwd = PathBuf::from(configured);
    if !cwd.is_absolute() {
        cwd = base.join(cwd);
    }
    if !cwd.is_dir() {
        return Err(format!(
            "EQOA_PARSER_DIR is not a directory: {}",
            cwd.display()
        ));
    }
    let cwd = fs::canonicalize(cwd).map_err(|e| format!("Cannot access EQOA_PARSER_DIR: {e}"))?;
    let search_path: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let command = find_command(command, folder, &search_path).ok_or_else(|| {
        format!("Parser command was not found beside mod.json or on PATH: {command}")
    })?;
    Ok(ParserLaunch { command, args, cwd })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Fixture {
        root: PathBuf,
        manifest: PathBuf,
        cwd: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "eqoa-mod-launch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let folder = root.join("mods/eqoa_parser");
            let cwd = root.join("parser source");
            fs::create_dir_all(&folder).unwrap();
            fs::create_dir_all(&cwd).unwrap();
            let this = Self {
                root,
                manifest: folder.join("mod.json"),
                cwd,
            };
            this.write(serde_json::json!({"id":"eqoa_parser", "enabled":true, "command":std::env::current_exe().unwrap(), "args":["-m", "eqoa_parser", "--client", "a \"quoted\" arg"]}));
            this
        }
        fn write(&self, value: serde_json::Value) {
            fs::write(&self.manifest, value.to_string()).unwrap();
        }
        fn plan(&self) -> Result<ParserLaunch, String> {
            parser_plan(&self.manifest, Some(self.cwd.as_os_str()))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn manifest_command_args_and_working_directory_drive_launch() {
        let f = Fixture::new();
        let plan = f.plan().unwrap();
        assert_eq!(plan.command, std::env::current_exe().unwrap());
        assert_eq!(
            plan.args,
            ["-m", "eqoa_parser", "--client", "a \"quoted\" arg"]
        );
        assert_eq!(plan.cwd, fs::canonicalize(&f.cwd).unwrap());
        assert!(
            parser_plan(&f.manifest, None)
                .unwrap_err()
                .contains("EQOA_PARSER_DIR")
        );
    }

    #[test]
    fn launch_rejects_missing_disabled_and_malformed_manifest() {
        let f = Fixture::new();
        fs::remove_file(&f.manifest).unwrap();
        assert!(f.plan().is_err());
        f.write(
            serde_json::json!({"id":"eqoa_parser", "enabled":false,"command":"python", "args":[]}),
        );
        assert!(f.plan().unwrap_err().contains("disabled"));
        f.write(serde_json::json!({"id":"eqoa_parser", "command":"python", "args":[12]}));
        assert!(f.plan().unwrap_err().contains("args"));
    }

    #[test]
    fn relative_command_and_directory_resolve_from_installation() {
        let f = Fixture::new();
        let program = f.manifest.parent().unwrap().join("companion.exe");
        fs::write(&program, "test fixture").unwrap();
        f.write(serde_json::json!({"id":"eqoa_parser", "command":"./companion.exe", "args":[]}));
        let plan = parser_plan(&f.manifest, Some(std::ffi::OsStr::new("parser source"))).unwrap();
        assert_eq!(
            fs::canonicalize(plan.command).unwrap(),
            fs::canonicalize(program).unwrap()
        );
        assert_eq!(plan.cwd, fs::canonicalize(&f.cwd).unwrap());
        f.write(serde_json::json!({"id":"eqoa_parser", "command":"./missing.exe", "args":[]}));
        assert!(f.plan().unwrap_err().contains("command"));
    }

    #[test]
    fn bare_commands_fall_back_to_search_path() {
        let f = Fixture::new();
        let executable = std::env::current_exe().unwrap();
        let name = executable.file_name().unwrap().to_str().unwrap();
        assert_eq!(
            find_command(
                name,
                f.manifest.parent().unwrap(),
                &[executable.parent().unwrap().to_path_buf()]
            )
            .unwrap(),
            executable
        );
        assert!(
            find_command(
                "eqoa-deliberately-missing",
                f.manifest.parent().unwrap(),
                &[]
            )
            .is_none()
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn find_command(command: &str, directory: &Path, search_path: &[PathBuf]) -> Option<PathBuf> {
    let program = Path::new(command);
    if program.is_absolute() {
        return program.is_file().then(|| program.to_path_buf());
    }
    if command.contains('/') || command.contains('\\') {
        let path = directory.join(program);
        return path.is_file().then_some(path);
    }
    for folder in std::iter::once(directory).chain(search_path.iter().map(PathBuf::as_path)) {
        let path = folder.join(command);
        if path.is_file() {
            return Some(path);
        }
        #[cfg(windows)]
        if path.extension().is_none() {
            let extensions =
                std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
            for extension in extensions.split(';').filter(|s| !s.is_empty()) {
                let candidate = folder.join(format!("{command}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod auto_create_tests {
    use super::*;

    #[test]
    fn forge_mods_auto_create_fresh_dir() {
        let parent = std::env::temp_dir().join(format!("eqoa-mods-fresh-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        let mods = parent.join("mods");
        assert!(!mods.exists());

        auto_create_mods_dir_at(&mods);
        assert!(mods.is_dir(), "mods folder must be created");
        let readme = mods.join("README.txt");
        assert!(readme.is_file(), "README.txt must be written in newly created folder");
        let text = fs::read_to_string(&readme).unwrap();
        assert!(text.contains("Options -> Extended -> Mods & Addons -> Addon Manager"));
        assert!(text.contains("addons_error.log"));

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn forge_mods_existing_dir_untouched() {
        let parent = std::env::temp_dir().join(format!("eqoa-mods-exist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        let mods = parent.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let my_mod = mods.join("my_mod.lua");
        fs::write(&my_mod, "-- existing mod code").unwrap();
        let custom_readme = mods.join("README.txt");
        fs::write(&custom_readme, "custom player readme").unwrap();

        auto_create_mods_dir_at(&mods);
        assert_eq!(
            fs::read_to_string(&custom_readme).unwrap(),
            "custom player readme",
            "existing README.txt must not be overwritten"
        );
        assert_eq!(
            fs::read_to_string(&my_mod).unwrap(),
            "-- existing mod code",
            "existing files in mods folder must remain untouched"
        );

        // Also verify existing folder without README does NOT have README injected
        let mods2 = parent.join("mods2");
        fs::create_dir_all(&mods2).unwrap();
        auto_create_mods_dir_at(&mods2);
        assert!(
            !mods2.join("README.txt").exists(),
            "pre-existing folder without README must not have README created"
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn forge_mods_unwritable_location_no_panic() {
        let parent = std::env::temp_dir().join(format!("eqoa-mods-invalid-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();
        let file = parent.join("blocked");
        fs::write(&file, "blocking file").unwrap();
        // Creating a directory path through an existing regular file fails in create_dir_all
        let bad_path = file.join("mods");
        auto_create_mods_dir_at(&bad_path); // Must log and not panic!
        assert!(!bad_path.exists());

        let _ = fs::remove_dir_all(&parent);
    }
}
