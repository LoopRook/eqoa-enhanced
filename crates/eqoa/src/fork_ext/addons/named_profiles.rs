//! Named, process-selected mod profiles. No account or credential input exists here.
use super::{discovery::DiscoveredAddon, storage::Storage};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_LAYOUT: usize = 1024 * 1024;
pub(super) const DEFAULT: &str = "Default";

// Connected character identity survives zone loading and is independent of HP.
// Its only source is Play.player.name / dump.player.class; never Session.user.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CharacterSnapshot {
    pub name: [u8; 24],
    pub len: u8,
    pub class: u8,
}
impl CharacterSnapshot {
    pub fn new(connected: bool, name: &str, class: i64) -> Option<Self> {
        if !connected || name.is_empty() || name.len() > 24 || !(0..=14).contains(&class) {
            return None;
        }
        let mut value = Self {
            len: name.len() as u8,
            class: class as u8,
            ..Default::default()
        };
        value.name[..name.len()].copy_from_slice(name.as_bytes());
        Some(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Character {
    pub server: String,
    pub name: String,
    pub class: u8,
}
impl Character {
    pub fn new(server: &str, value: CharacterSnapshot) -> Result<Self, String> {
        let name = std::str::from_utf8(
            value
                .name
                .get(..value.len as usize)
                .ok_or("Invalid character name")?,
        )
        .map_err(|_| "Invalid character name")?;
        if server.is_empty()
            || server.len() > 255
            || !server
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-_:[]".contains(&b))
            || name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || "_'- ".contains(c))
            || value.class > 14
        {
            return Err(
                "Character profile identity is unavailable; check the configured server".into(),
            );
        }
        Ok(Self {
            server: server.to_ascii_lowercase(),
            name: name.to_lowercase(),
            class: value.class,
        })
    }
}

pub(super) fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 48
        || name.trim() != name
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" _-".contains(&b))
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || name.eq_ignore_ascii_case("characters")
        || device_name(name)
    {
        return Err("Use a profile name of 1-48 letters, numbers, spaces, hyphens or underscores; device names are reserved".into());
    }
    Ok(())
}
fn device_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|p| {
        upper
            .strip_prefix(p)
            .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
    })
}
fn key_ok(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Document {
    pub schema: u32,
    pub enabled: BTreeMap<String, bool>,
    pub addons: BTreeMap<String, Json>,
    pub layout: String,
}
impl Document {
    pub fn empty() -> Self {
        Self {
            schema: 1,
            ..Default::default()
        }
    }
    fn json(&self) -> Json {
        json!({"schemaVersion":self.schema,"enabled":self.enabled,"addons":self.addons,"layout":self.layout})
    }
    fn parse(json: Json) -> Result<Self, String> {
        let obj = json
            .as_object()
            .filter(|v| v.len() == 4)
            .ok_or("Invalid mod profile schema")?;
        if obj.get("schemaVersion").and_then(Json::as_u64) != Some(1) {
            return Err("Unsupported mod profile schema".into());
        }
        let mut doc = Self::empty();
        let enabled = obj
            .get("enabled")
            .and_then(Json::as_object)
            .filter(|v| v.len() <= 128)
            .ok_or("Invalid profile enabled list")?;
        for (key, value) in enabled {
            if !key_ok(key) {
                return Err("Invalid profile addon key".into());
            }
            doc.enabled.insert(
                key.clone(),
                value.as_bool().ok_or("Invalid profile enabled flag")?,
            );
        }
        let addons = obj
            .get("addons")
            .and_then(Json::as_object)
            .filter(|v| v.len() <= 128)
            .ok_or("Invalid profile Save states")?;
        let lua = mlua::Lua::new();
        for (key, value) in addons {
            if !key_ok(key) || value.to_string().len() > 1024 * 1024 {
                return Err("Invalid profile addon state".into());
            }
            super::profile::decode(&lua, value).map_err(|_| "Invalid profile addon Save state")?;
            doc.addons.insert(key.clone(), value.clone());
        }
        doc.layout = obj
            .get("layout")
            .and_then(Json::as_str)
            .filter(|v| v.len() <= MAX_LAYOUT && !v.contains('\0'))
            .ok_or("Invalid profile window layout")?
            .to_owned();
        Ok(doc)
    }
}

#[derive(Default)]
struct Bindings {
    characters: BTreeMap<String, BTreeMap<String, String>>,
    classes: BTreeMap<String, BTreeMap<String, String>>,
}
impl Bindings {
    fn parse(json: Json) -> Result<Self, String> {
        let obj = json
            .as_object()
            .filter(|v| v.len() == 3 && v.get("schemaVersion").and_then(Json::as_u64) == Some(1))
            .ok_or("Invalid character profile assignments")?;
        let mut bindings = Self::default();
        for (field, out) in [
            ("characters", &mut bindings.characters),
            ("classes", &mut bindings.classes),
        ] {
            let servers = obj
                .get(field)
                .and_then(Json::as_object)
                .filter(|v| v.len() <= 128)
                .ok_or("Invalid character profile assignments")?;
            for (server, values) in servers {
                let values = values
                    .as_object()
                    .filter(|v| v.len() <= 1024)
                    .ok_or("Too many character profile assignments")?;
                let mut map = BTreeMap::new();
                for (id, name) in values {
                    if server.is_empty() || server.len() > 255 || id.is_empty() || id.len() > 128 {
                        return Err("Invalid character profile assignments".into());
                    }
                    let name = name.as_str().ok_or("Invalid assigned profile name")?;
                    valid_name(name)?;
                    map.insert(id.clone(), name.to_owned());
                }
                out.insert(server.clone(), map);
            }
        }
        Ok(bindings)
    }
}
#[derive(Clone, Debug)]
pub(super) struct ProfileRow {
    pub name: String,
    pub active: bool,
    pub assigned: bool,
    pub characters: usize,
    pub class_defaults: Vec<u8>,
}
pub(crate) struct Store {
    root: Option<PathBuf>,
}
impl Store {
    pub fn new(config: Option<PathBuf>) -> Self {
        Self {
            root: config.map(|p| p.join("addons").join("profiles")),
        }
    }
    fn root(&self) -> Result<&Path, String> {
        self.root
            .as_deref()
            .ok_or_else(|| "Mod profile configuration directory is unavailable".into())
    }
    pub fn path(&self, name: &str) -> Result<PathBuf, String> {
        valid_name(name)?;
        Ok(self.root()?.join(format!("{name}.json")))
    }
    pub fn read(&self, name: &str) -> Result<Document, String> {
        let data = read_json(&self.path(name)?)?
            .ok_or("Mod profile is missing; the current profile was kept")?;
        Document::parse(data)
    }
    pub fn write(&self, name: &str, doc: &Document) -> Result<(), String> {
        let path = self.path(name)?;
        // Also protect against a file corrupted externally after this process loaded it.
        if read_json(&path)?.is_some() {
            self.read(name)?;
        }
        Document::parse(doc.json())?;
        write_json(&path, &doc.json())
    }
    pub fn names(&self) -> Result<Vec<String>, String> {
        let root = self.root()?;
        if !root.exists() {
            return Ok(vec![]);
        }
        let mut names = Vec::new();
        for e in fs::read_dir(root)
            .map_err(|_| "Cannot list mod profiles")?
            .take(256)
        {
            let e = e.map_err(|_| "Cannot list mod profiles")?;
            let p = e.path();
            if p.extension().is_some_and(|v| v == "json")
                && let Some(name) = p.file_stem().and_then(|s| s.to_str())
                && valid_name(name).is_ok()
            {
                names.push(name.to_owned());
            }
        }
        names.sort_by_key(|v| v.to_ascii_lowercase());
        Ok(names)
    }
    pub fn create(&self, name: &str, doc: &Document) -> Result<(), String> {
        valid_name(name)?;
        // All creators share this lock; an automatic clone never replaces a
        // concurrently-created manual profile with the same name.
        let _lock = self.lock("profiles.lock", "Profile creation is busy; try again")?;
        if self.names()?.iter().any(|v| v.eq_ignore_ascii_case(name)) {
            return Err("That profile name already exists".into());
        }
        self.write(name, doc)
    }
    pub fn delete(&self, name: &str, active: &str) -> Result<(), String> {
        if name.eq_ignore_ascii_case(active) || name.eq_ignore_ascii_case(DEFAULT) {
            return Err("The active profile and Default cannot be deleted".into());
        }
        let path = self.path(name)?;
        self.read(name)?;
        fs::remove_file(path).map_err(|_| "Cannot delete mod profile".into())
    }
    pub fn migrate(
        &self,
        legacy: &Storage,
        addons: &[DiscoveredAddon],
    ) -> Result<Document, String> {
        let path = self.path(DEFAULT)?;
        if path.exists() {
            return self.read(DEFAULT);
        }
        let root = self.root()?;
        fs::create_dir_all(root).map_err(|_| "Cannot create mod profile directory")?;
        // Serialize the complete check/collect/publish operation across clients.
        // Keep the lock file so contenders always lock the same inode. The OS
        // releases the lock even if the migrating client exits unexpectedly.
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("migration.lock"))
            .map_err(|_| "Cannot lock Default profile migration")?;
        if lock.try_lock().is_err() {
            return self.read(DEFAULT).map_err(|_| {
                "Default profile migration is busy; retry loading Default shortly".into()
            });
        }
        // Another client may have finished migration and saved changes since
        // our first check. Only the lock winner may create the missing Default.
        if path.exists() {
            return self.read(DEFAULT);
        }
        if self.root()?.join(".migrated").exists() {
            return Err("Default is missing; restore it or save a new profile".into());
        }
        let mut doc = Document::empty();
        let lua = mlua::Lua::new();
        for addon in addons {
            let key = Storage::key(addon)?;
            doc.enabled.insert(
                key.clone(),
                legacy.enabled(&key)?.unwrap_or(addon.initially_enabled),
            );
            if let Some(value) = legacy.load_profile(&key, &lua)? {
                doc.addons.insert(
                    key,
                    super::profile::encode(value)
                        .map_err(|_| "Cannot migrate legacy addon settings")?,
                );
            }
        }
        let old_ini = self.root()?.parent().unwrap().join("imgui.ini");
        if let Some(bytes) = read_bytes(&old_ini, MAX_LAYOUT)? {
            doc.layout = String::from_utf8(bytes).map_err(|_| "Invalid legacy window layout")?;
        }
        self.write(DEFAULT, &doc)?;
        super::storage::atomic_replace(&self.root()?.join(".migrated"), b"1")
            .map_err(|_| "Cannot mark profile migration complete")?;
        Ok(doc)
    }
    fn bindings(&self) -> Result<Bindings, String> {
        read_json(&self.root()?.join("characters.json"))?
            .map(Bindings::parse)
            .transpose()
            .map(|v| v.unwrap_or_default())
    }
    pub fn resolve(&self, character: &Character) -> Result<(String, bool), String> {
        let bindings = self.bindings()?;
        let explicit = bindings
            .characters
            .get(&character.server)
            .and_then(|m| m.get(&character.name));
        let class = bindings
            .classes
            .get(&character.server)
            .and_then(|m| m.get(&character.class.to_string()));
        // Deleted profiles fall back through class -> Default. Existing damaged
        // files are still read and reported; they are never silently replaced.
        let name = [explicit, class]
            .into_iter()
            .flatten()
            .find(|name| self.path(name).is_ok_and(|p| p.exists()));
        Ok((
            name.cloned().unwrap_or_else(|| DEFAULT.into()),
            name.is_none(),
        ))
    }
    fn lock(&self, filename: &str, busy: &str) -> Result<fs::File, String> {
        let root = self.root()?;
        fs::create_dir_all(root).map_err(|_| "Cannot create mod profile directory")?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(filename))
            .map_err(|_| busy.to_owned())?;
        lock.try_lock().map_err(|_| busy.to_owned())?;
        Ok(lock)
    }
    fn write_bindings(&self, bindings: &Bindings) -> Result<(), String> {
        write_json(
            &self.root()?.join("characters.json"),
            &json!({"schemaVersion":1,"characters":bindings.characters,"classes":bindings.classes}),
        )
    }
    /// Assign an independent copy for unassigned/legacy-Default characters.
    /// The assignment lock covers discovery, creation and read-modify-write.
    pub fn ensure_character(&self, character: &Character) -> Result<(String, bool), String> {
        let _lock = self.lock(
            "characters.lock",
            "Character profile assignments are busy; try again",
        )?;
        let mut bindings = self.bindings()?;
        if let Some(name) = bindings
            .characters
            .get(&character.server)
            .and_then(|m| m.get(&character.name))
        {
            if !name.eq_ignore_ascii_case(DEFAULT) && self.path(name)?.exists() {
                self.read(name)?; // A damaged explicit profile is never replaced.
                return Ok((name.clone(), false));
            }
        }
        let seed = bindings
            .classes
            .get(&character.server)
            .and_then(|m| m.get(&character.class.to_string()))
            .filter(|name| self.path(name).is_ok_and(|p| p.exists()))
            .map(String::as_str)
            .unwrap_or(DEFAULT);
        let document = self.read(seed)?;
        let mut base: String = character
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        base = base.trim().to_owned();
        if let Some(first) = base.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        if valid_name(&base).is_err() || base.eq_ignore_ascii_case(DEFAULT) {
            base = if base.is_empty() {
                "Character".into()
            } else {
                format!("Character {base}")
            };
        }
        base = base
            .chars()
            .take(24)
            .collect::<String>()
            .trim_end()
            .to_owned();
        let server: String = character
            .server
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .take(18)
            .collect();
        let names = self.names()?;
        let mut name = None;
        for collision in 0..128 {
            let candidate = match collision {
                0 => base.clone(),
                1 => format!("{base} {server}"),
                n => format!("{base} {server}-{n}"),
            };
            let candidate = candidate.chars().take(48).collect::<String>();
            valid_name(&candidate)?;
            if !names.iter().any(|v| v.eq_ignore_ascii_case(&candidate)) {
                name = Some(candidate);
                break;
            }
        }
        let name =
            name.ok_or("Too many profiles with this character name; choose a profile manually")?;
        self.create(&name, &document)?;
        bindings
            .characters
            .entry(character.server.clone())
            .or_default()
            .insert(character.name.clone(), name.clone());
        self.write_bindings(&bindings)?;
        Ok((name, true))
    }
    pub fn catalog(
        &self,
        active: &str,
        character: Option<&Character>,
    ) -> Result<Vec<ProfileRow>, String> {
        let bindings = self.bindings()?;
        let assigned = character.and_then(|c| {
            bindings
                .characters
                .get(&c.server)
                .and_then(|m| m.get(&c.name))
        });
        self.names()?
            .into_iter()
            .map(|name| {
                let characters = bindings
                    .characters
                    .values()
                    .flat_map(|m| m.values())
                    .filter(|v| v.eq_ignore_ascii_case(&name))
                    .count();
                let mut class_defaults: Vec<u8> = bindings
                    .classes
                    .iter()
                    .filter(|(server, _)| character.is_none_or(|c| c.server == **server))
                    .flat_map(|(_, m)| m.iter())
                    .filter(|(_, v)| v.eq_ignore_ascii_case(&name))
                    .filter_map(|(k, _)| k.parse::<u8>().ok().filter(|v| *v <= 14))
                    .collect();
                class_defaults.sort();
                class_defaults.dedup();
                Ok(ProfileRow {
                    active: name.eq_ignore_ascii_case(active),
                    assigned: assigned.is_some_and(|v| v.eq_ignore_ascii_case(&name)),
                    name,
                    characters,
                    class_defaults,
                })
            })
            .collect()
    }
    pub fn bind(&self, character: &Character, profile: &str, class: bool) -> Result<(), String> {
        self.read(profile)?;
        let root = self.root()?;
        fs::create_dir_all(root).map_err(|_| "Cannot create mod profile directory")?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("characters.lock"))
            .map_err(|_| "Character profile assignments are busy; try again")?;
        // The operating system releases this nonblocking lock on close or crash.
        // Keep the file: unlinking it could let concurrent clients lock two inodes.
        lock.try_lock()
            .map_err(|_| "Character profile assignments are busy; try again")?;
        // Read afresh under the short lock so two clients assigning characters merge.
        let mut bindings = self.bindings()?;
        let map = if class {
            &mut bindings.classes
        } else {
            &mut bindings.characters
        };
        let id = if class {
            character.class.to_string()
        } else {
            character.name.clone()
        };
        map.entry(character.server.clone())
            .or_default()
            .insert(id, profile.to_owned());
        write_json(
            &root.join("characters.json"),
            &json!({"schemaVersion":1,"characters":bindings.characters,"classes":bindings.classes}),
        )
    }
}

fn read_bytes(path: &Path, max: usize) -> Result<Option<Vec<u8>>, String> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err("Linked profile files are not supported".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err("Linked profile files are not supported".into());
            }
        }
    }
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Cannot read mod profile file".into()),
    };
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read mod profile file")?;
    if bytes.len() > max {
        return Err("Mod profile file exceeds its size limit".into());
    }
    Ok(Some(bytes))
}
fn read_json(path: &Path) -> Result<Option<Json>, String> {
    read_bytes(path, MAX_BYTES)?
        .map(|b| {
            serde_json::from_slice(&b).map_err(|_| "Cannot read damaged mod profile JSON".into())
        })
        .transpose()
}
fn write_json(path: &Path, value: &Json) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "Cannot encode mod profile")?;
    if bytes.len() > MAX_BYTES {
        return Err("Mod profile exceeds 16 MiB".into());
    }
    super::storage::atomic_replace(path, &bytes)
        .map_err(|_| "Cannot save mod profile atomically".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "eqoa-named-profiles-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn mod_profiles_names_reject_paths_devices_and_reserved_metadata() {
        for name in [
            "",
            "..",
            "../Tank",
            "a/b",
            "a\\b",
            "CON",
            "con.txt",
            "NuL",
            "COM1",
            "lpt9",
            "COM¹",
            "CONOUT$",
            "characters",
            " Tank",
            "Tank ",
            "a:b",
        ] {
            assert!(valid_name(name).is_err(), "accepted {name}");
        }
        assert!(valid_name(&"a".repeat(49)).is_err());
        for name in [
            "Default",
            "Healer",
            "Tank 2",
            "Multibox_2",
            "Raid-Healer",
            "COM0",
        ] {
            valid_name(name).unwrap();
        }
    }
    #[test]
    fn mod_profiles_corrupt_file_is_never_overwritten_or_deleted() {
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        store.create("Tank", &Document::empty()).unwrap();
        let path = store.path("Tank").unwrap();
        for bytes in [
            b"{broken".as_slice(),
            br#"{"schemaVersion":999}"#.as_slice(),
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(store.read("Tank").is_err());
            assert!(store.write("Tank", &Document::empty()).is_err());
            assert!(store.delete("Tank", "Default").is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::write(&path, b"null").unwrap();
        assert!(
            store
                .migrate(&Storage::new(Some(scratch.0.clone())), &[])
                .is_ok()
        );
        let default = store.path(DEFAULT).unwrap();
        fs::write(&default, b"broken").unwrap();
        assert!(
            store
                .migrate(&Storage::new(Some(scratch.0.clone())), &[])
                .is_err()
        );
        assert_eq!(fs::read(default).unwrap(), b"broken");
    }
    fn character(server: &str, name: &str, class: i64) -> Character {
        Character::new(
            server,
            CharacterSnapshot::new(true, name, class.into()).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn mod_profiles_character_and_class_bindings_use_server_and_never_account() {
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        for name in [DEFAULT, "Healer", "Tank"] {
            store.create(name, &Document::empty()).unwrap();
        }
        let a = character("server.example", "Ari", 3);
        let b = character("server.example", "Bea", 3);
        assert_eq!(store.resolve(&a).unwrap(), ("Default".into(), true));
        store.bind(&a, "Healer", true).unwrap();
        assert_eq!(store.resolve(&b).unwrap(), ("Healer".into(), false));
        store.bind(&a, "Tank", false).unwrap();
        assert_eq!(store.resolve(&a).unwrap(), ("Tank".into(), false));
        assert_eq!(
            store
                .resolve(&character("other.example", "Ari", 3))
                .unwrap()
                .0,
            "Default"
        );
        // The session's account/password are deliberately outside the identity API.
        let account = "account_DO_NOT_PERSIST";
        let password = "password_DO_NOT_PERSIST";
        let bytes = fs::read_to_string(store.root().unwrap().join("characters.json")).unwrap();
        assert!(!bytes.contains(account) && !bytes.contains(password));
        let data: Json = serde_json::from_str(&bytes).unwrap();
        assert_eq!(data["characters"], json!({"server.example":{"ari":"Tank"}}));
        assert_eq!(data["classes"], json!({"server.example":{"3":"Healer"}}));
        assert!(
            Character::new(
                "account:password@host",
                CharacterSnapshot::new(true, "Ari", 3).unwrap()
            )
            .is_err()
        );
        let second = Store::new(Some(scratch.0.clone()));
        second.bind(&b, "Healer", false).unwrap();
        assert_eq!(
            store.resolve(&a).unwrap().0,
            "Tank",
            "another client's binding must retain Ari"
        );
        assert_eq!(store.resolve(&b).unwrap().0, "Healer");
    }
    #[test]
    fn mod_profiles_delete_protects_active_default_and_case_collisions() {
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        store.create(DEFAULT, &Document::empty()).unwrap();
        store.create("Healer", &Document::empty()).unwrap();
        assert!(store.create("healer", &Document::empty()).is_err());
        assert!(store.delete(DEFAULT, "Healer").is_err());
        assert!(store.delete("Healer", "healer").is_err());
        store.delete("Healer", DEFAULT).unwrap();
        assert_eq!(store.names().unwrap(), vec![DEFAULT]);
    }
    #[test]
    fn mod_profiles_binding_lock_releases_and_deleted_assignments_fall_back() {
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        for name in [DEFAULT, "Healer"] {
            store.create(name, &Document::empty()).unwrap();
        }
        let c = character("server.example", "Ari", 3);
        // An existing lock file from a prior process must not block assignments.
        let lock_path = store.root().unwrap().join("characters.lock");
        fs::write(&lock_path, b"").unwrap();
        store.bind(&c, "Healer", false).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        lock.try_lock().unwrap();
        assert!(store.bind(&c, DEFAULT, false).is_err());
        drop(lock);
        store.bind(&c, "Healer", false).unwrap();
        store.delete("Healer", DEFAULT).unwrap();
        assert_eq!(store.resolve(&c).unwrap(), (DEFAULT.into(), true));
    }
    #[test]
    fn mod_profiles_migrate_default_once_without_removing_legacy_files() {
        let scratch = Scratch::new();
        let mods = scratch.0.join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("example.lua"), "").unwrap();
        let addon = super::super::discovery::discover(&mods).addons.remove(0);
        let legacy = Storage::new(Some(scratch.0.clone()));
        let key = Storage::key(&addon).unwrap();
        let lua = mlua::Lua::new();
        legacy
            .save_profile(&key, lua.load("return {size=42}").eval().unwrap())
            .unwrap();
        legacy.set_enabled(&key, true).unwrap();
        let ini = scratch.0.join("addons/imgui.ini");
        fs::write(&ini, "[Window][Orb]\nPos=10,20\n").unwrap();
        let store = Store::new(Some(scratch.0.clone()));
        let doc = store.migrate(&legacy, &[addon.clone()]).unwrap();
        assert_eq!(doc.enabled.get(&key), Some(&true));
        assert_eq!(doc.layout, "[Window][Orb]\nPos=10,20\n");
        assert_eq!(
            doc.addons[&key],
            super::super::profile::encode(lua.load("return {size=42}").eval().unwrap()).unwrap()
        );
        assert!(legacy.profile_path(&key).unwrap().is_file());
        assert!(scratch.0.join("addons/state.json").is_file());
        assert!(ini.is_file());
        legacy.set_enabled(&key, false).unwrap();
        assert_eq!(
            store.migrate(&legacy, &[addon]).unwrap().enabled.get(&key),
            Some(&true)
        );
    }
    #[test]
    fn mod_profiles_concurrent_migration_keeps_the_winners_saved_default() {
        let scratch = Scratch::new();
        let first = Store::new(Some(scratch.0.clone()));
        let second = Store::new(Some(scratch.0.clone()));
        fs::create_dir_all(first.root().unwrap()).unwrap();
        // Client one is still collecting legacy data. Client two must not publish.
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(first.root().unwrap().join("migration.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        let legacy = Storage::new(Some(scratch.0.clone()));
        assert!(
            second.migrate(&legacy, &[]).is_err(),
            "a second migration must wait for the winner"
        );
        assert!(!first.path(DEFAULT).unwrap().exists());
        let mut saved = Document::empty();
        saved.layout = "[Window][Changed after migration]\nPos=77,88\n\n".into();
        first.write(DEFAULT, &saved).unwrap();
        let bytes = fs::read(first.path(DEFAULT).unwrap()).unwrap();
        drop(lock);
        assert_eq!(second.migrate(&legacy, &[]).unwrap(), saved);
        assert_eq!(fs::read(first.path(DEFAULT).unwrap()).unwrap(), bytes);
    }
    #[test]
    fn mod_profiles_concurrent_character_bindings_keep_both_clients_entries() {
        use std::sync::{Arc, Barrier};
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        for name in ["Healer", "Tank"] {
            store.create(name, &Document::empty()).unwrap();
        }
        let barrier = Arc::new(Barrier::new(2));
        let threads = [("Ari", "Healer"), ("Bea", "Tank")].map(|(name, profile)| {
            let root = scratch.0.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let store = Store::new(Some(root));
                let identity = character("server.example", name, 3);
                barrier.wait();
                for _ in 0..100 {
                    match store.bind(&identity, profile, false) {
                        Ok(()) => return,
                        Err(e) if e.contains("busy") => {
                            std::thread::sleep(std::time::Duration::from_millis(10))
                        }
                        Err(e) => panic!("unexpected binding error: {e}"),
                    }
                }
                panic!("binding stayed busy");
            })
        });
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(
            store
                .resolve(&character("server.example", "Ari", 3))
                .unwrap()
                .0,
            "Healer"
        );
        assert_eq!(
            store
                .resolve(&character("server.example", "Bea", 3))
                .unwrap()
                .0,
            "Tank"
        );
    }
    #[test]
    fn mod_profiles_auto_names_handle_devices_unicode_and_server_collisions() {
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        store.create(DEFAULT, &Document::empty()).unwrap();
        assert_eq!(
            store
                .ensure_character(&character("one.example", "Ari", 3))
                .unwrap()
                .0,
            "Ari"
        );
        assert_eq!(
            store
                .ensure_character(&character("other.example", "Ari", 3))
                .unwrap()
                .0,
            "Ari other-example"
        );
        for name in ["CON", "Default", "\u{03b1}"] {
            let created = store
                .ensure_character(&character("one.example", name, 3))
                .unwrap()
                .0;
            valid_name(&created).unwrap();
            assert!(!created.eq_ignore_ascii_case(DEFAULT));
        }
    }
    #[test]
    fn mod_profiles_concurrent_auto_creation_has_one_character_assignment() {
        use std::sync::{Arc, Barrier};
        let scratch = Scratch::new();
        let store = Store::new(Some(scratch.0.clone()));
        store.create(DEFAULT, &Document::empty()).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let threads = [0, 1].map(|_| {
            let root = scratch.0.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let store = Store::new(Some(root));
                barrier.wait();
                for _ in 0..100 {
                    match store.ensure_character(&character("server.example", "Ari", 3)) {
                        Ok((name, _)) => return name,
                        Err(e) if e.contains("busy") => {
                            std::thread::sleep(std::time::Duration::from_millis(10))
                        }
                        Err(e) => panic!("unexpected automatic creation failure: {e}"),
                    }
                }
                panic!("automatic creation stayed busy");
            })
        });
        for thread in threads {
            assert_eq!(thread.join().unwrap(), "Ari");
        }
        assert_eq!(store.names().unwrap(), ["Ari", "Default"]);
        let path = store.path("Ari").unwrap();
        fs::write(&path, b"broken").unwrap();
        assert!(
            store
                .ensure_character(&character("server.example", "Ari", 3))
                .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), b"broken");
    }
}
