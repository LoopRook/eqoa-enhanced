//! Creature journal for map search and discovery recording.
//! Compatible with FrontiersForge world_map journal.json format:
//! `{ "<name>": { "<id>": { "world": w, "level": l, "pos": [x, y, z] } } }`

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

pub const UNNAMED_NAME: &str = "(unnamed)";
pub const SAVE_QUIET_SECONDS: u64 = 30;
pub const DEDUPE_RANGE: f32 = 30.0;
pub const DEDUPE_RANGE_SQ: f32 = DEDUPE_RANGE * DEDUPE_RANGE; // 900.0
pub const MAX_IMPORT_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB load cap
pub const MAX_JOURNAL_SIZE: usize = 14 * 1024 * 1024; // 14 MiB save cap
pub const MAX_TOTAL_SPAWNS: usize = 80_000;
pub const MAX_ENTRIES_PER_IMPORT: usize = 50_000;
pub const MAX_NAME_LEN: usize = 64;
pub const MAX_ID_LEN: usize = 20;

#[derive(Clone, Debug, PartialEq)]
pub struct EntityInfo {
    pub world: i32,
    pub level: u8,
    pub pos: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct JournalRecord {
    pub ids: BTreeMap<String, EntityInfo>,
}

#[derive(Clone, Debug)]
pub struct ParsedEntry {
    pub name: String,
    pub id_key: String,
    pub id_num: Option<u32>,
    pub world: i32,
    pub level: u8,
    pub pos: [f32; 3],
}

pub struct Journal {
    records: BTreeMap<String, JournalRecord>,
    known_ids: BTreeSet<u32>,
    loaded: bool,
    dirty: bool,
    disabled: bool,
    revision: u64,
    last_change_time: Option<Instant>,
    custom_path: Option<PathBuf>,
    name_count: usize,
    id_count: usize,
    status: String,
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}

impl Journal {
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            known_ids: BTreeSet::new(),
            loaded: false,
            dirty: false,
            disabled: false,
            revision: 0,
            last_change_time: None,
            custom_path: None,
            name_count: 0,
            id_count: 0,
            status: "new journal".into(),
        }
    }

    pub fn new_empty() -> Self {
        let mut j = Self::new();
        j.loaded = true;
        j
    }

    pub fn new_at_path(path: PathBuf) -> Self {
        let mut j = Self::new();
        j.custom_path = Some(path);
        j.loaded = true;
        j
    }

    /// Map only empty string or None to "(unnamed)" without trimming, matching journal.lua:40-45.
    pub fn name_for(name: Option<&str>) -> String {
        match name {
            Some("") | None => UNNAMED_NAME.to_string(),
            Some(s) => s.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    pub fn counts(&self) -> (usize, usize) {
        (self.name_count, self.id_count)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[allow(dead_code)]
    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn has_creature(&self, name: &str) -> bool {
        self.records.contains_key(name)
    }

    pub fn get_spawn(&self, name: &str, id_key: &str) -> Option<&EntityInfo> {
        self.records.get(name)?.ids.get(id_key)
    }

    pub fn records(&self) -> &BTreeMap<String, JournalRecord> {
        &self.records
    }

    pub fn default_path() -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::fork_ext::report::config_dir().map(|d| d.join("journal.json"))
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    pub fn resolved_path(&self) -> Option<PathBuf> {
        self.custom_path.clone().or_else(Self::default_path)
    }

    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(p) = self.resolved_path() {
            if p.is_file() {
                if let Err(e) = self.load_from_path(&p) {
                    self.disabled = true;
                    crate::say(&format!("Creature journal disabled (load failed): {e}"));
                    self.status = "Journal disabled (corrupted file)".into();
                    set_status_msg(format!("Journal disabled: {e}"));
                }
            }
        }
    }

    pub fn load_from_path(&mut self, path: &Path) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let metadata = std::fs::metadata(path).map_err(|e| format!("cannot read metadata: {e}"))?;
            if metadata.len() > MAX_IMPORT_SIZE {
                self.disabled = true;
                return Err(format!("journal file exceeds 16 MiB: {} bytes", metadata.len()));
            }
            let s = std::fs::read_to_string(path).map_err(|e| {
                self.disabled = true;
                format!("cannot read file: {e}")
            })?;
            if let Err(e) = self.load_from_str(&s) {
                self.disabled = true;
                return Err(e);
            }
            self.loaded = true;
            self.dirty = false;
            self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
            Ok(())
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            Err("filesystem unavailable on wasm".into())
        }
    }

    pub fn load_from_str(&mut self, text: &str) -> Result<(), String> {
        let entries = parse_journal_entries(text, usize::MAX)?;
        let mut records = BTreeMap::<String, JournalRecord>::new();
        let mut known_ids = BTreeSet::new();
        let mut name_count = 0;
        let mut id_count = 0;

        for entry in entries {
            let rec = records.entry(entry.name).or_insert_with(|| {
                name_count += 1;
                JournalRecord::default()
            });
            if !rec.ids.contains_key(&entry.id_key) {
                rec.ids.insert(entry.id_key, EntityInfo {
                    world: entry.world,
                    level: entry.level,
                    pos: entry.pos,
                });
                if let Some(id_num) = entry.id_num {
                    known_ids.insert(id_num);
                }
                id_count += 1;
            }
        }

        self.records = records;
        self.known_ids = known_ids;
        self.name_count = name_count;
        self.id_count = id_count;
        self.dirty = false;
        self.revision += 1;
        Ok(())
    }

    /// Record a discovered NPC. Returns true when something new was recorded.
    /// A known id is never rewritten, and a new id is skipped when the same name
    /// at the same level was already recorded within the dedupe range (30 Euclidean units).
    /// Dedupe matches journal.lua:150 (across worlds).
    pub fn record(
        &mut self,
        name: &str,
        id: u32,
        world: i32,
        level: u8,
        pos: [f32; 3],
        now: Instant,
    ) -> bool {
        if self.disabled || id == 0 {
            return false;
        }
        if self.id_count >= MAX_TOTAL_SPAWNS {
            return false;
        }
        let journal_name = Self::name_for(Some(name));
        let id_key = id.to_string();

        if let Some(record) = self.records.get(&journal_name) {
            // Rule: an id already recorded is never rewritten
            if record.ids.contains_key(&id_key) {
                return false;
            }
            // Rule: a new id is skipped when the same name at the same level was already recorded
            // within the reference's dedupe range (30 Euclidean units in 2D x/z), matching journal.lua:150.
            for info in record.ids.values() {
                if info.level == level {
                    let dx = info.pos[0] - pos[0];
                    let dz = info.pos[2] - pos[2];
                    if dx * dx + dz * dz <= DEDUPE_RANGE_SQ {
                        return false;
                    }
                }
            }
        }

        let record = self.records.entry(journal_name).or_insert_with(|| {
            self.name_count += 1;
            JournalRecord::default()
        });

        record.ids.insert(id_key, EntityInfo { world, level, pos });
        self.known_ids.insert(id);
        self.id_count += 1;
        self.dirty = true;
        self.revision += 1;
        self.last_change_time = Some(now);
        self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
        true
    }

    /// Process active entities, recording valid NPCs while strictly excluding
    /// player characters, corpses, and pets.
    pub fn record_entities(
        &mut self,
        slots: &[Option<net::entity::Entity>],
        me: Option<u32>,
        pet_id: u32,
        world: i32,
        now: Instant,
    ) {
        if self.disabled {
            return;
        }
        self.ensure_loaded();
        for e in slots.iter().flatten() {
            // Fast lookup: skip entities whose id was already recorded
            if self.known_ids.contains(&e.id) {
                continue;
            }
            if Some(e.id) == me || (pet_id != u32::MAX && e.id == pet_id) {
                continue;
            }
            if e.kind != net::entity::TYPE_CREATURE {
                continue;
            }
            let Some(c) = e.creature.as_ref() else {
                continue;
            };
            // Privacy rule: class byte < 2 only (2 = player character, 3 = pet)
            if c.p256 >= 2 {
                continue;
            }
            // Additional player physics and inviteable flags
            if c.p255 != 0 || c.p264 != 0 {
                continue;
            }
            // Filter corpses and inactive slots
            if e.p18 & 2 != 0 || e.p19 == 0 {
                continue;
            }
            let pos = if e.actor_pos.iter().all(|x| x.is_finite()) && e.actor_pos.iter().any(|&x| x != 0.0) {
                e.actor_pos
            } else if e.pos.iter().all(|x| x.is_finite()) {
                e.pos
            } else {
                continue;
            };

            self.record(&c.name, e.id, world, c.level, pos, now);
        }
    }

    /// Prepares a save snapshot if unsaved changes have been quiet for SAVE_QUIET_SECONDS.
    pub fn take_save_snapshot_if_due(&mut self, now: Instant) -> Option<(PathBuf, BTreeMap<String, JournalRecord>)> {
        if self.disabled || !self.dirty {
            return None;
        }
        if let Some(last_change) = self.last_change_time {
            if now.duration_since(last_change).as_secs() >= SAVE_QUIET_SECONDS {
                let path = self.resolved_path()?;
                self.dirty = false;
                return Some((path, self.records.clone()));
            }
        }
        None
    }

    /// Synchronous flush if due, for tests.
    pub fn flush_if_due(&mut self, now: Instant) -> bool {
        if let Some((path, records)) = self.take_save_snapshot_if_due(now) {
            let _ = save_records_to_path(&path, &records);
            true
        } else {
            false
        }
    }

    /// Serializes records into the JSON schema matching world_map/modules/journal.lua.
    pub fn to_json_string(&self) -> Result<String, String> {
        serialize_records_to_json(&self.records)
    }

    /// Synchronous write to disk using a temporary file and rename.
    pub fn save_sync(&mut self) -> Result<(), String> {
        if self.disabled {
            return Err("journal is disabled to protect existing file".into());
        }
        if !self.dirty {
            return Ok(());
        }
        let Some(path) = self.resolved_path() else {
            return Err("no journal path available".into());
        };
        save_records_to_path(&path, &self.records)?;
        self.dirty = false;
        self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save(&mut self) -> Result<(), String> {
        self.save_sync()
    }

    /// Merge parsed entries into this journal. Missing ids added, own data wins on conflict.
    pub fn merge_entries(&mut self, entries: Vec<ParsedEntry>, now: Instant) -> usize {
        if self.disabled {
            return 0;
        }
        let mut added = 0;
        for entry in entries {
            if self.id_count >= MAX_TOTAL_SPAWNS {
                break;
            }
            let record = self.records.entry(entry.name).or_insert_with(|| {
                self.name_count += 1;
                JournalRecord::default()
            });
            if !record.ids.contains_key(&entry.id_key) {
                record.ids.insert(entry.id_key, EntityInfo {
                    world: entry.world,
                    level: entry.level,
                    pos: entry.pos,
                });
                if let Some(id_num) = entry.id_num {
                    self.known_ids.insert(id_num);
                }
                self.id_count += 1;
                added += 1;
            }
        }

        if added > 0 {
            self.dirty = true;
            self.revision += 1;
            self.last_change_time = Some(now);
            self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
        }
        added
    }

    /// Merge another journal file or string into this one. Returns the number of new spawns added.
    pub fn merge(&mut self, text: &str, now: Instant) -> Result<usize, String> {
        if self.disabled {
            return Err("journal is disabled to protect existing file".into());
        }
        let entries = parse_journal_entries(text, MAX_ENTRIES_PER_IMPORT)?;
        let added = self.merge_entries(entries, now);
        if added > 0 {
            let _ = self.save_sync();
        }
        Ok(added)
    }

    /// Export journal to a designated file path.
    #[allow(dead_code)]
    pub fn export_to_file(&self, path: &Path) -> Result<(), String> {
        save_records_to_path(path, &self.records)
    }
}

/// Serializes records to JSON string formatted to match FrontiersForge reference schema.
pub fn serialize_records_to_json(records: &BTreeMap<String, JournalRecord>) -> Result<String, String> {
    let mut root = serde_json::Map::new();
    for (name, rec) in records {
        let mut ids_map = serde_json::Map::new();
        for (id_key, info) in &rec.ids {
            let mut info_map = serde_json::Map::new();
            info_map.insert("world".to_string(), serde_json::Value::Number(info.world.into()));
            info_map.insert("level".to_string(), serde_json::Value::Number(info.level.into()));
            let pos_arr = vec![
                serde_json::Number::from_f64(info.pos[0] as f64).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                serde_json::Number::from_f64(info.pos[1] as f64).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                serde_json::Number::from_f64(info.pos[2] as f64).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
            ];
            info_map.insert("pos".to_string(), serde_json::Value::Array(pos_arr));
            ids_map.insert(id_key.clone(), serde_json::Value::Object(info_map));
        }
        root.insert(name.clone(), serde_json::Value::Object(ids_map));
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(root))
        .map_err(|e| format!("failed to serialize: {e}"))?;
    if text.len() > MAX_JOURNAL_SIZE {
        return Err("journal serialized size exceeds 14 MiB limit".into());
    }
    Ok(text)
}

/// Writes serialized records to a designated path atomically via a .tmp file.
pub fn save_records_to_path(path: &Path, records: &BTreeMap<String, JournalRecord>) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let text = serialize_records_to_json(records)?;
        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, text).map_err(|e| format!("cannot write tmp journal: {e}"))?;
        std::fs::rename(&tmp_path, path).map_err(|e| format!("cannot rename journal: {e}"))?;
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, records);
        Ok(())
    }
}

/// Parse and validate journal JSON according to size, level, name, and id bounds.
pub fn parse_journal_entries(text: &str, cap_entries: usize) -> Result<Vec<ParsedEntry>, String> {
    let val: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("failed to parse: {e}"))?;
    let serde_json::Value::Object(map) = val else {
        return Err("expected JSON object at root".into());
    };

    let mut out = Vec::new();
    for (name, ids_val) in map {
        // Cap name at 64 characters, reject control characters
        if name.len() > MAX_NAME_LEN || name.chars().any(|c| c.is_control()) {
            continue;
        }
        let Some(ids_map) = ids_val.as_object() else {
            continue;
        };
        for (id_key, info_val) in ids_map {
            if out.len() >= cap_entries {
                return Ok(out);
            }
            // Cap id at 20 characters, digits only
            if id_key.len() > MAX_ID_LEN || !id_key.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Some(info_obj) = info_val.as_object() else {
                continue;
            };
            let world = info_obj.get("world").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            // Reject level outside 1-60 without wrapping
            let Some(level_u64) = info_obj.get("level").and_then(|v| v.as_u64()) else {
                continue;
            };
            if !(1..=60).contains(&level_u64) {
                continue;
            }
            let level = level_u64 as u8;
            let Some(pos_arr) = info_obj.get("pos").and_then(|v| v.as_array()) else {
                continue;
            };
            if pos_arr.len() < 3 {
                continue;
            }
            let (Some(x), Some(y), Some(z)) = (
                pos_arr[0].as_f64(),
                pos_arr[1].as_f64(),
                pos_arr[2].as_f64(),
            ) else {
                continue;
            };
            let pos = [x as f32, y as f32, z as f32];
            if !pos.iter().all(|v| v.is_finite()) {
                continue;
            }
            let id_num = id_key.parse::<u32>().ok();
            out.push(ParsedEntry {
                name: name.clone(),
                id_key: id_key.clone(),
                id_num,
                world,
                level,
                pos,
            });
        }
    }
    Ok(out)
}

// Global active journal singleton
static GLOBAL_JOURNAL: Mutex<Option<Journal>> = Mutex::new(None);
static SAVE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static STATUS_MSG: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static LAST_SCAN: Mutex<Option<Instant>> = Mutex::new(None);

pub fn with<R>(f: impl FnOnce(&mut Journal) -> R) -> R {
    let mut lock = GLOBAL_JOURNAL.lock().unwrap();
    let j = lock.get_or_insert_with(Journal::new);
    f(j)
}

pub fn reset_for_test() {
    let mut lock = GLOBAL_JOURNAL.lock().unwrap();
    *lock = None;
    SAVE_IN_FLIGHT.store(false, Ordering::SeqCst);
    let mut s = STATUS_MSG.lock().unwrap();
    *s = None;
    let mut ls = LAST_SCAN.lock().unwrap();
    *ls = None;
}

pub fn set_status_msg(msg: String) {
    let mut lock = STATUS_MSG.lock().unwrap();
    *lock = Some((msg, Instant::now()));
}

pub fn status_msg() -> Option<String> {
    let lock = STATUS_MSG.lock().unwrap();
    let (msg, when) = lock.as_ref()?;
    if when.elapsed().as_secs() < 6 {
        Some(msg.clone())
    } else {
        None
    }
}

pub fn revision() -> u64 {
    with(|j| j.revision())
}

struct SaveInFlightGuard;
impl Drop for SaveInFlightGuard {
    fn drop(&mut self) {
        SAVE_IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

fn spawn_save_worker(path: PathBuf, records: BTreeMap<String, JournalRecord>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if SAVE_IN_FLIGHT.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        std::thread::Builder::new()
            .name("journal-save".into())
            .spawn(move || {
                let _guard = SaveInFlightGuard;
                if let Err(e) = save_records_to_path(&path, &records) {
                    crate::say(&format!("Failed to save journal: {e}"));
                }
            })
            .ok();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, records);
    }
}

/// One-line hook called each frame from Play::map_frame.
/// Throttled to ~1 Hz matching world_map.lua:1060.
pub fn record_entities(
    slots: &[Option<net::entity::Entity>],
    me: Option<u32>,
    pet_id: u32,
    world: i32,
) {
    let now = Instant::now();
    {
        let mut last = LAST_SCAN.lock().unwrap();
        if let Some(prev) = *last {
            if now.duration_since(prev).as_secs_f32() < 1.0 {
                return;
            }
        }
        *last = Some(now);
    }

    let save_snapshot = with(|j| {
        j.record_entities(slots, me, pet_id, world, now);
        j.take_save_snapshot_if_due(now)
    });

    if let Some((path, records)) = save_snapshot {
        spawn_save_worker(path, records);
    }
}

/// Clean exit hook called from report::clean_exit.
pub fn clean_exit() {
    with(|j| {
        if !j.disabled && j.has_unsaved_changes() {
            let _ = j.save_sync();
        }
    });
}

/// Current creature and spawn counts.
pub fn counts() -> (usize, usize) {
    with(|j| {
        j.ensure_loaded();
        j.counts()
    })
}

/// Current journal status string.
#[allow(dead_code)]
pub fn status() -> String {
    with(|j| {
        j.ensure_loaded();
        j.status().to_string()
    })
}

/// Export to <desktop>/EQOA-Creature-Journal.json and open explorer.
/// Snapshots under mutex and writes on a worker thread.
pub fn export_to_desktop() -> Result<PathBuf, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (export_path, snapshot) = with(|j| {
            j.ensure_loaded();
            let dest_dir = crate::fork_ext::report::desktop_dir()
                .or_else(crate::fork_ext::report::config_dir)
                .unwrap_or_else(|| PathBuf::from("."));
            let export_path = dest_dir.join("EQOA-Creature-Journal.json");
            (export_path, j.records.clone())
        });

        let export_path_clone = export_path.clone();
        std::thread::Builder::new()
            .name("journal-export".into())
            .spawn(move || {
                let res = (|| -> Result<(), String> {
                    let text = serialize_records_to_json(&snapshot)?;
                    if let Some(parent) = export_path_clone.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::write(&export_path_clone, text)
                        .map_err(|e| format!("cannot export journal: {e}"))?;
                    crate::fork_ext::report::reveal_in_file_manager(&export_path_clone);
                    Ok(())
                })();
                match res {
                    Ok(()) => {
                        let fname = export_path_clone.file_name().and_then(|n| n.to_str()).unwrap_or("journal");
                        set_status_msg(format!("Saved to {fname}"));
                    }
                    Err(e) => {
                        crate::say(&format!("Export failed: {e}"));
                        set_status_msg(format!("Export: {e}"));
                    }
                }
            })
            .map_err(|e| format!("cannot spawn export thread: {e}"))?;

        Ok(export_path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Export unavailable on wasm".into())
    }
}

/// Import from a chosen file or fallback to <desktop>/EQOA-Creature-Journal.json.
/// Runs file dialog, read, and parsing on worker thread without holding mutex.
pub fn import_interactive() -> Result<usize, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::Builder::new()
            .name("journal-import".into())
            .spawn(move || {
                let picked = rfd::FileDialog::new()
                    .set_title("Import Creature Journal")
                    .add_filter("JSON", &["json"])
                    .pick_file();

                let path = match picked.or_else(|| {
                    let desktop = crate::fork_ext::report::desktop_dir()?;
                    let fallback = desktop.join("EQOA-Creature-Journal.json");
                    if fallback.is_file() {
                        Some(fallback)
                    } else {
                        None
                    }
                }) {
                    Some(p) => p,
                    None => return,
                };

                let res = (|| -> Result<usize, String> {
                    let metadata = std::fs::metadata(&path).map_err(|e| format!("cannot inspect file: {e}"))?;
                    if metadata.len() > MAX_IMPORT_SIZE {
                        return Err(format!("File exceeds 16 MiB: {} bytes", metadata.len()));
                    }
                    let content = std::fs::read_to_string(&path).map_err(|e| format!("cannot read file: {e}"))?;
                    let entries = parse_journal_entries(&content, MAX_ENTRIES_PER_IMPORT)?;
                    let now = Instant::now();
                    let (added, save_snapshot) = with(|j| {
                        j.ensure_loaded();
                        let added = j.merge_entries(entries, now);
                        let snapshot = if added > 0 { j.take_save_snapshot_if_due(now) } else { None };
                        (added, snapshot)
                    });
                    if let Some((path, records)) = save_snapshot {
                        spawn_save_worker(path, records);
                    }
                    Ok(added)
                })();

                match res {
                    Ok(added) => {
                        crate::say(&format!("Imported {added} spawns from {}", path.display()));
                        set_status_msg(format!("Imported {added} spawns"));
                    }
                    Err(e) => {
                        crate::say(&format!("Import failed: {e}"));
                        set_status_msg(format!("Import: {e}"));
                    }
                }
            })
            .map_err(|e| format!("cannot spawn import thread: {e}"))?;

        Ok(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Import unavailable on wasm".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_journal_dedupe() {
        let now = Instant::now();
        let mut j = Journal::new_empty();

        // 1. Initial record
        assert!(j.record("Grass Snake", 100, 0, 3, [100.0, 0.0, 100.0], now));
        assert_eq!(j.counts(), (1, 1));

        // 2. Same id is skipped
        assert!(!j.record("Grass Snake", 100, 0, 3, [100.0, 0.0, 100.0], now));
        assert_eq!(j.counts(), (1, 1));

        // 3. Same name + level within 30 units (Euclidean) is skipped
        // dx = 15, dz = 15 -> dx*dx + dz*dz = 450 <= 900
        assert!(!j.record("Grass Snake", 101, 0, 3, [115.0, 0.0, 115.0], now));
        assert_eq!(j.counts(), (1, 1));

        // 4. Same name + DIFFERENT level within 30 units records a new entry
        assert!(j.record("Grass Snake", 102, 0, 4, [115.0, 0.0, 115.0], now));
        assert_eq!(j.counts(), (1, 2));

        // 5. Same name + same level > 30 units records a new entry
        // dx = 40, dz = 0 -> dx*dx + dz*dz = 1600 > 900
        assert!(j.record("Grass Snake", 103, 0, 3, [140.0, 0.0, 100.0], now));
        assert_eq!(j.counts(), (1, 3));
    }

    #[test]
    fn test_dedupe_across_worlds() {
        let now = Instant::now();
        let mut j = Journal::new_empty();

        // Recorded in world 0
        assert!(j.record("Hill Giant", 200, 0, 35, [10.0, 0.0, 10.0], now));
        assert_eq!(j.counts(), (1, 1));

        // Same name and level within 30 units in world 1: skipped matching journal.lua:150
        assert!(!j.record("Hill Giant", 201, 1, 35, [15.0, 0.0, 15.0], now));
        assert_eq!(j.counts(), (1, 1));
    }

    #[test]
    fn test_journal_privacy_player_never_recorded() {
        let now = Instant::now();
        let mut j = Journal::new_empty();
        let me = 0x100;
        let pet_id = 0x200;

        let mut slots = Vec::new();
        // 1. Local player
        slots.push(Some(net::entity::Entity {
            id: me,
            kind: net::entity::TYPE_CREATURE,
            actor_pos: [10.0, 0.0, 10.0],
            creature: Some(net::entity::Creature {
                name: "HeroPlayer".into(),
                level: 50,
                p255: 1,
                ..Default::default()
            }),
            ..Default::default()
        }));
        // 2. Remote player character with p255 = 1 (walk physics)
        slots.push(Some(net::entity::Entity {
            id: 0x101,
            kind: net::entity::TYPE_CREATURE,
            actor_pos: [20.0, 0.0, 20.0],
            creature: Some(net::entity::Creature {
                name: "OtherPlayer".into(),
                level: 45,
                p255: 1,
                ..Default::default()
            }),
            ..Default::default()
        }));
        // 3. Remote player character with p264 = 1 (inviteable player)
        slots.push(Some(net::entity::Entity {
            id: 0x102,
            kind: net::entity::TYPE_CREATURE,
            actor_pos: [25.0, 0.0, 25.0],
            creature: Some(net::entity::Creature {
                name: "InviteablePlayer".into(),
                level: 30,
                p255: 0,
                p264: 1,
                ..Default::default()
            }),
            ..Default::default()
        }));
        // 4. Player's pet
        slots.push(Some(net::entity::Entity {
            id: pet_id,
            kind: net::entity::TYPE_CREATURE,
            actor_pos: [30.0, 0.0, 30.0],
            creature: Some(net::entity::Creature {
                name: "Familiar".into(),
                level: 20,
                p255: 0,
                p264: 0,
                ..Default::default()
            }),
            ..Default::default()
        }));
        // 5. Valid NPC
        slots.push(Some(net::entity::Entity {
            id: 0x300,
            kind: net::entity::TYPE_CREATURE,
            p19: 1,
            actor_pos: [40.0, 0.0, 40.0],
            creature: Some(net::entity::Creature {
                name: "Kodiak Bear".into(),
                level: 15,
                p255: 0,
                p264: 0,
                npc_flags: 1,
                ..Default::default()
            }),
            ..Default::default()
        }));

        j.record_entities(&slots, Some(me), pet_id, 0, now);

        assert_eq!(j.counts(), (1, 1));
        assert!(j.has_creature("Kodiak Bear"));
        assert!(!j.has_creature("HeroPlayer"));
        assert!(!j.has_creature("OtherPlayer"));
        assert!(!j.has_creature("InviteablePlayer"));
        assert!(!j.has_creature("Familiar"));
    }

    #[test]
    fn test_privacy_excludes_class_2_and_3_with_p255_p264_zero() {
        let now = Instant::now();
        let mut j = Journal::new_empty();

        let mut slots = Vec::new();
        // Remote player character with p256 = 2, but p255 = 0, p264 = 0
        slots.push(Some(net::entity::Entity {
            id: 0x401,
            kind: net::entity::TYPE_CREATURE,
            p19: 1,
            actor_pos: [10.0, 0.0, 10.0],
            creature: Some(net::entity::Creature {
                name: "StealthyPlayer".into(),
                level: 50,
                p255: 0,
                p264: 0,
                p256: 2, // Class 2 = player
                ..Default::default()
            }),
            ..Default::default()
        }));
        // Remote pet with p256 = 3, but p255 = 0, p264 = 0
        slots.push(Some(net::entity::Entity {
            id: 0x402,
            kind: net::entity::TYPE_CREATURE,
            p19: 1,
            actor_pos: [20.0, 0.0, 20.0],
            creature: Some(net::entity::Creature {
                name: "PlayerPet".into(),
                level: 40,
                p255: 0,
                p264: 0,
                p256: 3, // Class 3 = pet
                ..Default::default()
            }),
            ..Default::default()
        }));
        // Valid NPC with p256 = 1 (< 2)
        slots.push(Some(net::entity::Entity {
            id: 0x403,
            kind: net::entity::TYPE_CREATURE,
            p19: 1,
            actor_pos: [30.0, 0.0, 30.0],
            creature: Some(net::entity::Creature {
                name: "Forest Wolf".into(),
                level: 8,
                p255: 0,
                p264: 0,
                p256: 1, // Class 1 = NPC
                npc_flags: 1,
                ..Default::default()
            }),
            ..Default::default()
        }));

        j.record_entities(&slots, None, u32::MAX, 0, now);

        assert_eq!(j.counts(), (1, 1));
        assert!(j.has_creature("Forest Wolf"));
        assert!(!j.has_creature("StealthyPlayer"), "class 2 player must never be recorded");
        assert!(!j.has_creature("PlayerPet"), "class 3 pet must never be recorded");
    }

    #[test]
    fn test_journal_merge_rules() {
        let now = Instant::now();
        let mut j = Journal::new_empty();
        j.record("Snake", 1, 0, 2, [10.0, 0.0, 10.0], now);

        let friend_json = r#"{
            "Snake": {
                "1": { "world": 0, "level": 5, "pos": [999.0, 0.0, 999.0] },
                "2": { "world": 0, "level": 3, "pos": [20.0, 0.0, 20.0] }
            },
            "Bear": {
                "3": { "world": 0, "level": 10, "pos": [50.0, 0.0, 50.0] }
            }
        }"#;

        let added = j.merge(friend_json, now).expect("merge succeeds");
        assert_eq!(added, 2);
        assert_eq!(j.counts(), (2, 3));

        // Conflict check: own data wins on conflict for id 1!
        let snake_1 = j.get_spawn("Snake", "1").expect("id 1 exists");
        assert_eq!(snake_1.level, 2);
        assert_eq!(snake_1.pos, [10.0, 0.0, 10.0]);

        // Missing entries added
        let snake_2 = j.get_spawn("Snake", "2").expect("id 2 exists");
        assert_eq!(snake_2.level, 3);
        assert_eq!(snake_2.pos, [20.0, 0.0, 20.0]);

        let bear_3 = j.get_spawn("Bear", "3").expect("id 3 exists");
        assert_eq!(bear_3.level, 10);
        assert_eq!(bear_3.pos, [50.0, 0.0, 50.0]);
    }

    #[test]
    fn test_merge_bounds_and_caps() {
        let now = Instant::now();
        let mut j = Journal::new_empty();

        let long_name = "A".repeat(65);
        let long_id = "1".repeat(21);

        let bad_json = format!(r#"{{
            "ValidNpc": {{
                "1": {{ "world": 0, "level": 10, "pos": [1.0, 2.0, 3.0] }},
                "2": {{ "world": 0, "level": 0, "pos": [1.0, 2.0, 3.0] }},
                "3": {{ "world": 0, "level": 61, "pos": [1.0, 2.0, 3.0] }},
                "4": {{ "world": 0, "level": 257, "pos": [1.0, 2.0, 3.0] }},
                "{long_id}": {{ "world": 0, "level": 15, "pos": [1.0, 2.0, 3.0] }},
                "abc": {{ "world": 0, "level": 15, "pos": [1.0, 2.0, 3.0] }}
            }},
            "{long_name}": {{
                "10": {{ "world": 0, "level": 20, "pos": [1.0, 2.0, 3.0] }}
            }},
            "Bad\u0000Name": {{
                "11": {{ "world": 0, "level": 20, "pos": [1.0, 2.0, 3.0] }}
            }}
        }}"#);

        let added = j.merge(&bad_json, now).expect("merge completes");
        assert_eq!(added, 1, "only the single valid entry (id 1) should be accepted");
        assert_eq!(j.counts(), (1, 1));
        assert!(j.has_creature("ValidNpc"));
        assert!(!j.has_creature(&long_name));
        assert!(!j.has_creature("Bad\u{0000}Name"));
    }

    #[test]
    fn test_corrupt_journal_disabled_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("eqoa-journal-corrupt-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("journal.json");

        // Write corrupt JSON
        let corrupt_content = "{ not valid json ";
        std::fs::write(&path, corrupt_content).unwrap();

        let mut j = Journal::new_at_path(path.clone());
        let res = j.load_from_path(&path);
        assert!(res.is_err());
        assert!(j.is_disabled(), "journal must be disabled after corrupted load");

        // Attempting to record or save must fail or not modify the file
        assert!(!j.record("TestMonster", 999, 0, 10, [0.0, 0.0, 0.0], Instant::now()));
        assert!(j.save_sync().is_err());

        // File on disk must remain unchanged!
        let disk_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(disk_content, corrupt_content, "corrupted file must never be overwritten");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_journal_round_trip_reference_schema() {
        let fixture = r#"{
  "(unnamed)": {
    "99999": {
      "level": 1,
      "pos": [
        1.0,
        2.0,
        3.0
      ],
      "world": 0
    }
  },
  "Grass Snake": {
    "1234567": {
      "level": 3,
      "pos": [
        100.5,
        20.0,
        -300.25
      ],
      "world": 0
    }
  }
}"#;

        let now = Instant::now();
        let mut j = Journal::new_empty();
        j.merge(fixture, now).expect("valid fixture");
        assert_eq!(j.counts(), (2, 2));

        let exported = j.to_json_string().expect("serialize");
        let mut j2 = Journal::new_empty();
        j2.merge(&exported, now).expect("re-parse");
        assert_eq!(j2.counts(), (2, 2));

        assert_eq!(j.get_spawn("(unnamed)", "99999"), j2.get_spawn("(unnamed)", "99999"));
        assert_eq!(j.get_spawn("Grass Snake", "1234567"), j2.get_spawn("Grass Snake", "1234567"));
    }

    #[test]
    fn test_journal_nameless_creatures() {
        let now = Instant::now();
        let mut j = Journal::new_empty();
        assert!(j.record("", 501, 0, 5, [10.0, 0.0, 10.0], now));
        assert!(j.record("   ", 502, 0, 5, [50.0, 0.0, 50.0], now));
        // Empty string maps to (unnamed), whitespace is preserved without trimming
        assert_eq!(j.counts(), (2, 2));
        assert!(j.has_creature("(unnamed)"));
        assert!(j.has_creature("   "));
    }

    #[test]
    fn test_journal_search_finds_journal_only_creature() {
        reset_for_test();
        with(|j| {
            j.loaded = true;
            j.record("ZzxMystic Beast", 5555, 0, 42, [500.0, 10.0, 500.0], Instant::now());
        });

        let mut s = crate::map::search::Search::default();
        // Search by name
        s.query = "ZzxMystic".into();
        let res = s.results();
        assert!(res.iter().any(|r| r.name == "ZzxMystic Beast"), "found by name");

        // Search by level
        s.query = "42".into();
        let res = s.results();
        assert!(res.iter().any(|r| r.name == "ZzxMystic Beast"), "found by level");

        // Search by location
        s.query = "Tunaria".into();
        let res = s.results();
        assert!(res.iter().any(|r| r.name == "ZzxMystic Beast"), "found by location");

        reset_for_test();
    }

    #[test]
    fn test_search_cache_invalidated_by_journal_revision() {
        reset_for_test();
        let mut s = crate::map::search::Search::default();
        s.query = "RareWolf".into();

        // 1. Initial query: empty
        let initial = s.results();
        assert!(!initial.iter().any(|r| r.name == "RareWolf"));

        // 2. Discover new entity in journal (bumps revision)
        with(|j| {
            j.loaded = true;
            j.record("RareWolf", 7777, 0, 25, [100.0, 0.0, 100.0], Instant::now());
        });

        // 3. Next results() call automatically invalidates cached results and finds creature
        let updated = s.results();
        assert!(updated.iter().any(|r| r.name == "RareWolf"), "cache must invalidate when journal revision bumps");

        reset_for_test();
    }

    #[test]
    fn test_fast_id_lookup_skips_known_entities() {
        let now = Instant::now();
        let mut j = Journal::new_empty();
        assert!(j.record("Cave Bat", 999, 0, 5, [10.0, 0.0, 10.0], now));
        assert!(j.known_ids.contains(&999));

        let mut slots = Vec::new();
        slots.push(Some(net::entity::Entity {
            id: 999,
            kind: net::entity::TYPE_CREATURE,
            p19: 1,
            actor_pos: [100.0, 0.0, 100.0],
            creature: Some(net::entity::Creature {
                name: "Changed Bat Name".into(),
                level: 5,
                ..Default::default()
            }),
            ..Default::default()
        }));

        // Processing slots skips id 999 on fast path
        j.record_entities(&slots, None, u32::MAX, 0, now);
        assert_eq!(j.counts(), (1, 1));
        assert!(j.has_creature("Cave Bat"));
        assert!(!j.has_creature("Changed Bat Name"));
    }

    #[test]
    fn test_journal_no_disk_io_until_quiet_period() {
        let dir = std::env::temp_dir().join(format!("eqoa-journal-io-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let journal_file = dir.join("journal.json");

        let t0 = Instant::now();
        let mut j = Journal::new_at_path(journal_file.clone());

        // Record 100 entities in memory
        for i in 1..=100 {
            j.record("Spawn", i, 0, 10, [i as f32 * 50.0, 0.0, 0.0], t0);
        }
        assert_eq!(j.counts(), (1, 100));

        // File must NOT exist yet (0 file writes)
        assert!(!journal_file.exists(), "no writes while recording");

        // Flush at t0 + 10s: quiet period 30s not reached
        let t_10s = t0 + Duration::from_secs(10);
        j.flush_if_due(t_10s);
        assert!(!journal_file.exists(), "no writes before quiet period");

        // Flush at t0 + 31s: quiet period reached
        let t_31s = t0 + Duration::from_secs(31);
        j.flush_if_due(t_31s);
        assert!(journal_file.exists(), "file written after quiet period");
        assert!(!j.has_unsaved_changes());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_journal_import_size_cap() {
        let dir = std::env::temp_dir().join(format!("eqoa-journal-cap-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let big_file = dir.join("big.json");
        let file = std::fs::File::create(&big_file).unwrap();
        file.set_len(16 * 1024 * 1024 + 1).unwrap();

        let mut j = Journal::new();
        let res = j.load_from_path(&big_file);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("16 MiB"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
