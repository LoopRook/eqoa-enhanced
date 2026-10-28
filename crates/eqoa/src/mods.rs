//! Drag-and-drop mod engine and companion manager for EQOA Native Rust.
//! Discovers mods in the `./mods/` directory and manages companion tools
//! like the EQOA Combat Parser.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
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
static RUNNING_PARSER: Mutex<Option<Child>> = Mutex::new(None);

/// The path to the `mods/` directory beside the executable or in current directory.
pub fn mods_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("mods");
            if p.is_dir() {
                return p;
            }
        }
    }
    PathBuf::from("mods")
}

/// Ensures the `mods/` directory exists and creates default starter templates.
pub fn ensure_mods_dir() {
    let dir = mods_dir();
    let _ = fs::create_dir_all(&dir);

    // Create a helpful README explaining drag-and-drop modding.
    let readme = dir.join("README.txt");
    if !readme.exists() {
        let text = "\
EQOA Native Rust - Mod Directory
================================

Drop your mod folders in this directory!
Each mod can be a folder containing a `mod.json` file.

Example folder structure:
  mods/
    my_custom_ui/
      mod.json
      (textures, resources...)
    eqoa_parser/
      mod.json

You can toggle mods on or off directly inside the game from:
Options -> Extended -> Mods
";
        let _ = fs::write(readme, text);
    }

    // Create starter entry for EQOA Combat Parser companion
    let parser_dir = dir.join("eqoa_parser");
    let parser_json = parser_dir.join("mod.json");
    if !parser_json.exists() {
        let _ = fs::create_dir_all(&parser_dir);
        let parser_manifest = "\
{\n\
  \"id\": \"eqoa_parser\",\n\
  \"name\": \"EQOA Combat Parser\",\n\
  \"version\": \"1.0.0\",\n\
  \"author\": \"LoopRook\",\n\
  \"description\": \"Real-time DPS meter and combat parser hooked directly into client telemetry without admin elevation.\",\n\
  \"enabled\": true,\n\
  \"command\": \"python\",\n\
  \"args\": [\"-m\", \"eqoa_parser.app\", \"--client\"]\n\
}\n";
        let _ = fs::write(parser_json, parser_manifest);
    }
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
        let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let manifest_file = path.join("mod.json");

        if manifest_file.exists() {
            if let Ok(content) = fs::read_to_string(&manifest_file) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    let id = v.get("id").and_then(|s| s.as_str()).unwrap_or(folder_name).to_string();
                    let name = v.get("name").and_then(|s| s.as_str()).unwrap_or(folder_name).to_string();
                    let version = v.get("version").and_then(|s| s.as_str()).unwrap_or("1.0.0").to_string();
                    let author = v.get("author").and_then(|s| s.as_str()).unwrap_or("Community").to_string();
                    let description = v.get("description").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    let enabled = v.get("enabled").and_then(|b| b.as_bool()).unwrap_or(true);
                    let command = v.get("command").and_then(|s| s.as_str()).map(|s| s.to_string());
                    let args = v.get("args")
                        .and_then(|a| a.as_array())
                        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
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

/// Checks whether the combat parser companion is currently running.
pub fn is_parser_running() -> bool {
    let mut lock = RUNNING_PARSER.lock().unwrap();
    if let Some(child) = lock.as_mut() {
        match child.try_wait() {
            Ok(None) => true,
            _ => {
                *lock = None;
                false
            }
        }
    } else {
        false
    }
}

/// Spawns the EQOA Combat Parser companion if not already running.
pub fn launch_parser() -> bool {
    let mut lock = RUNNING_PARSER.lock().unwrap();
    if let Some(child) = lock.as_mut() {
        if let Ok(None) = child.try_wait() {
            return true; // Already running
        }
    }

    // Try windowed pythonw first (no console window flash), then python
    let candidates = [
        ("C:\\Python314\\pythonw.exe", vec!["-m", "eqoa_parser", "--client"]),
        ("pythonw", vec!["-m", "eqoa_parser", "--client"]),
        ("C:\\Python314\\python.exe", vec!["-m", "eqoa_parser", "--client"]),
        ("python", vec!["-m", "eqoa_parser", "--client"]),
    ];

    let repo_dir = Path::new("C:\\Users\\hocka\\Github Projects\\EQOA Parser");

    for (cmd, args) in candidates {
        let mut command = Command::new(cmd);
        command.args(&args);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW prevents command prompt flash if python.exe is used
            command.creation_flags(0x08000000);
        }

        if repo_dir.is_dir() {
            command.current_dir(repo_dir);
            command.env("PYTHONPATH", repo_dir);
        }

        if let Ok(child) = command.spawn() {
            *lock = Some(child);
            return true;
        }
    }

    false
}
