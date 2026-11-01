//! Creature journal for map search and discovery recording.
//! Compatible with FrontiersForge world_map journal.json format:
//! `{ "<name>": { "<id>": { "world": w, "level": l, "pos": [x, y, z] } } }`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

pub const UNNAMED_NAME: &str = "(unnamed)";
pub const SAVE_QUIET_SECONDS: u64 = 30;
pub const DEDUPE_RANGE: f32 = 30.0;
pub const DEDUPE_RANGE_SQ: f32 = DEDUPE_RANGE * DEDUPE_RANGE; // 900.0
pub const MAX_IMPORT_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB

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

pub struct Journal {
    records: BTreeMap<String, JournalRecord>,
    loaded: bool,
    dirty: bool,
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
            loaded: false,
            dirty: false,
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

    pub fn name_for(name: Option<&str>) -> String {
        match name {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => UNNAMED_NAME.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    pub fn counts(&self) -> (usize, usize) {
        (self.name_count, self.id_count)
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
                let _ = self.load_from_path(&p);
            }
        }
    }

    pub fn load_from_path(&mut self, path: &Path) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let metadata = std::fs::metadata(path).map_err(|e| format!("cannot read metadata: {e}"))?;
            if metadata.len() > MAX_IMPORT_SIZE {
                return Err(format!("journal file exceeds 16 MiB: {} bytes", metadata.len()));
            }
            let s = std::fs::read_to_string(path).map_err(|e| format!("cannot read file: {e}"))?;
            self.load_from_str(&s)?;
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
        let val: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("failed to parse journal: {e}"))?;
        let serde_json::Value::Object(map) = val else {
            return Err("expected root JSON object".into());
        };

        let mut records = BTreeMap::new();
        let mut name_count = 0;
        let mut id_count = 0;

        for (name, ids_val) in map {
            let Some(ids_map) = ids_val.as_object() else {
                continue;
            };
            let mut rec = JournalRecord::default();
            for (id_key, info_val) in ids_map {
                let Some(info_obj) = info_val.as_object() else {
                    continue;
                };
                let world = info_obj.get("world").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let level = info_obj.get("level").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
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
                rec.ids.insert(id_key.clone(), EntityInfo { world, level, pos });
                id_count += 1;
            }
            if !rec.ids.is_empty() {
                name_count += 1;
                records.insert(name, rec);
            }
        }

        self.records = records;
        self.name_count = name_count;
        self.id_count = id_count;
        Ok(())
    }

    /// Record a discovered NPC. Returns true when something new was recorded.
    /// A known id is never rewritten, and a new id is skipped when the same name
    /// at the same level was already recorded within the dedupe range (30 Euclidean units).
    pub fn record(
        &mut self,
        name: &str,
        id: u32,
        world: i32,
        level: u8,
        pos: [f32; 3],
        now: Instant,
    ) -> bool {
        if id == 0 {
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
            // within the reference's dedupe range (30 Euclidean units in 2D x/z).
            for info in record.ids.values() {
                if info.world == world && info.level == level {
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
        self.id_count += 1;
        self.dirty = true;
        self.last_change_time = Some(now);
        self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
        true
    }

    /// Process a frame of active entities, recording valid NPCs while strictly excluding
    /// player characters, corpses, and the player's active pet.
    pub fn record_entities(
        &mut self,
        slots: &[Option<net::entity::Entity>],
        me: Option<u32>,
        pet_id: u32,
        world: i32,
        now: Instant,
    ) {
        self.ensure_loaded();
        for e in slots.iter().flatten() {
            if Some(e.id) == me || (pet_id != u32::MAX && e.id == pet_id) {
                continue;
            }
            if e.kind != net::entity::TYPE_CREATURE {
                continue;
            }
            let Some(c) = e.creature.as_ref() else {
                continue;
            };
            // Privacy rule: NEVER record player characters.
            // p255 != 0: player walk physics
            // p264 != 0: inviteable player
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
        self.flush_if_due(now);
    }

    /// Flushes to disk if unsaved discoveries have been quiet for SAVE_QUIET_SECONDS (30s).
    pub fn flush_if_due(&mut self, now: Instant) {
        if self.dirty {
            if let Some(last_change) = self.last_change_time {
                if now.duration_since(last_change).as_secs() >= SAVE_QUIET_SECONDS {
                    let _ = self.save();
                }
            }
        }
    }

    /// Serializes records into the JSON schema matching world_map/modules/journal.lua.
    pub fn to_json_string(&self) -> Result<String, String> {
        let mut root = serde_json::Map::new();
        for (name, rec) in &self.records {
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
        serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|e| format!("failed to serialize: {e}"))
    }

    /// Atomic write to disk using a temporary file and rename.
    pub fn save(&mut self) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.dirty {
                return Ok(());
            }
            let Some(path) = self.resolved_path() else {
                return Err("no journal path available".into());
            };

            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let text = self.to_json_string()?;
            let tmp_path = path.with_extension("tmp");
            std::fs::write(&tmp_path, text).map_err(|e| format!("cannot write tmp journal: {e}"))?;
            std::fs::rename(&tmp_path, &path).map_err(|e| format!("cannot rename journal: {e}"))?;

            self.dirty = false;
            self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
            Ok(())
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.dirty = false;
            Ok(())
        }
    }

    /// Merge another journal file or string into this one. Only names and ids not already
    /// present are added; own data always wins on conflict. Returns the number of new spawns added.
    pub fn merge(&mut self, text: &str, now: Instant) -> Result<usize, String> {
        let val: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("failed to parse: {e}"))?;
        let serde_json::Value::Object(map) = val else {
            return Err("expected JSON object at root".into());
        };

        let mut added = 0;
        for (name, ids_val) in map {
            let Some(ids_map) = ids_val.as_object() else {
                continue;
            };
            for (id_key, info_val) in ids_map {
                let Some(info_obj) = info_val.as_object() else {
                    continue;
                };
                let world = info_obj.get("world").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let level = info_obj.get("level").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
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

                let record = self.records.entry(name.clone()).or_insert_with(|| {
                    self.name_count += 1;
                    JournalRecord::default()
                });
                // Merge rule: only missing ids added, own data wins on conflict
                if !record.ids.contains_key(id_key) {
                    record.ids.insert(id_key.clone(), EntityInfo { world, level, pos });
                    self.id_count += 1;
                    added += 1;
                }
            }
        }

        if added > 0 {
            self.dirty = true;
            self.last_change_time = Some(now);
            self.status = format!("{} creatures, {} spawns", self.name_count, self.id_count);
            let _ = self.save();
        }
        Ok(added)
    }

    /// Export journal to a designated file path.
    pub fn export_to_file(&self, path: &Path) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let text = self.to_json_string()?;
            std::fs::write(path, text).map_err(|e| format!("cannot export journal: {e}"))?;
            Ok(())
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            Err("export unavailable on wasm".into())
        }
    }
}

// Global active journal singleton
static GLOBAL_JOURNAL: Mutex<Option<Journal>> = Mutex::new(None);

pub fn with<R>(f: impl FnOnce(&mut Journal) -> R) -> R {
    let mut lock = GLOBAL_JOURNAL.lock().unwrap();
    let j = lock.get_or_insert_with(Journal::new);
    f(j)
}

pub fn reset_for_test() {
    let mut lock = GLOBAL_JOURNAL.lock().unwrap();
    *lock = None;
}

/// One-line hook called each frame from Play::map_frame.
pub fn record_entities(
    slots: &[Option<net::entity::Entity>],
    me: Option<u32>,
    pet_id: u32,
    world: i32,
) {
    let now = Instant::now();
    with(|j| {
        j.record_entities(slots, me, pet_id, world, now);
    });
}

/// Clean exit hook called from report::clean_exit.
pub fn clean_exit() {
    with(|j| {
        if j.has_unsaved_changes() {
            let _ = j.save();
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
pub fn export_to_desktop() -> Result<PathBuf, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        with(|j| {
            j.ensure_loaded();
            let dest_dir = crate::fork_ext::report::desktop_dir()
                .or_else(crate::fork_ext::report::config_dir)
                .unwrap_or_else(|| PathBuf::from("."));
            let export_path = dest_dir.join("EQOA-Creature-Journal.json");
            j.export_to_file(&export_path)?;
            crate::fork_ext::report::reveal_in_file_manager(&export_path);
            Ok(export_path)
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Export unavailable on wasm".into())
    }
}

/// Import from a chosen file or fallback to <desktop>/EQOA-Creature-Journal.json.
pub fn import_interactive() -> Result<usize, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let picked = rfd::FileDialog::new()
            .set_title("Import Creature Journal")
            .add_filter("JSON", &["json"])
            .pick_file();

        let path = picked.or_else(|| {
            let desktop = crate::fork_ext::report::desktop_dir()?;
            let fallback = desktop.join("EQOA-Creature-Journal.json");
            if fallback.is_file() {
                Some(fallback)
            } else {
                None
            }
        }).ok_or_else(|| "No file selected or found on Desktop".to_string())?;

        let metadata = std::fs::metadata(&path).map_err(|e| format!("cannot inspect file: {e}"))?;
        if metadata.len() > MAX_IMPORT_SIZE {
            return Err(format!("File exceeds 16 MiB: {} bytes", metadata.len()));
        }

        let content = std::fs::read_to_string(&path).map_err(|e| format!("cannot read file: {e}"))?;
        let now = Instant::now();
        with(|j| {
            j.ensure_loaded();
            j.merge(&content, now)
        })
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
        assert_eq!(j.counts(), (1, 2));
        assert!(j.has_creature("(unnamed)"));
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
