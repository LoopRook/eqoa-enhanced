//! Plain owned game values and an ordered commit journal. No Lua at mutation sites.
use serde_json::{Value as Json, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    time::Instant,
};

#[derive(Clone, Debug)]
pub(crate) struct View {
    pub in_world: bool,
    pub phase: &'static str,
    pub world_id: Option<i32>,
    pub player: Json,
    pub minimap: Option<super::map_views::Projection>,
    pub map: Json,
    pub map_markers: Vec<Json>,
    pub player_stats: Option<Json>,
    pub entities: Vec<Json>,
    pub entity_slots: Vec<Option<u32>>,
    /// The entities the native minimap draws as dots, at this frame's drawn positions
    /// `(id, x, z)`: what a minimap hover walks without a record per creature.
    pub dots: Vec<(u32, f32, f32)>,
    pub abilities: Vec<Json>,
    pub group: Json,
    pub hotbar: Vec<Json>,
    pub effects: Option<Vec<Json>>,
    /// The four item containers (`eqoa.inventory/bank/loot/merchant.get`), as the client holds
    /// them; `Null` where the container is unavailable (loot and merchant while closed).
    pub inventory: Json,
    pub bank: Json,
    pub loot: Json,
    pub merchant: Json,
    pub quests: Vec<Json>,
    pub mastery: Option<Json>,
    pub mastery_skills: Option<Vec<Json>>,
    pub mastery_node: Option<Json>,
    pub channel: &'static str,
}
impl Default for View {
    fn default() -> Self {
        Self {
            in_world: false,
            phase: "pregame",
            world_id: None,
            player: Json::Null,
            minimap: None,
            map: Json::Null,
            map_markers: Vec::new(),
            player_stats: None,
            entities: Vec::new(),
            entity_slots: Vec::new(),
            dots: Vec::new(),
            abilities: Vec::new(),
            group: json!({"in_group":false,"self_leader":null,"members":[],"raid":[],"invites":[]}),
            hotbar: Vec::new(),
            effects: None,
            inventory: Json::Null,
            bank: Json::Null,
            loot: Json::Null,
            merchant: Json::Null,
            quests: Vec::new(),
            mastery: None,
            mastery_skills: None,
            mastery_node: None,
            channel: "other",
        }
    }
}

/// Which individual Class Mastery counters have been populated by dump or 0x1402 replies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MasteryFlags {
    pub available_spent: bool,
    pub share: bool,
    pub progress: bool,
    pub stats: bool,
    pub skills: bool,
}

impl MasteryFlags {
    pub fn from_dump(&mut self) {
        self.available_spent = true;
        self.share = true;
        self.progress = true;
        self.stats = true;
    }

    pub fn apply_subop(&mut self, op: u8) {
        match op {
            1 => self.share = true,
            3 => self.available_spent = true,
            4 => self.progress = true,
            5 => {
                self.available_spent = true;
                self.stats = true;
            }
            6 => self.stats = true,
            8 => self.skills = true,
            10 => {
                self.available_spent = true;
                self.stats = true;
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub(crate) struct Event {
    pub player_command: bool,
    pub player_action: Option<std::sync::Arc<std::sync::atomic::AtomicU8>>,
    pub name: String,
    pub seq: u64,
    pub time: f64,
    pub tick: u64,
    pub frame: u64,
    pub generation: u64,
    /// shared by every VM the event is published to (a copy of it is a count, not a tree)
    pub data: std::sync::Arc<Json>,
    pub bytes: usize,
}

impl Event {
    pub(super) fn expire_player(&mut self) {
        if let Some(action) = self.player_action.take() {
            action.store(2, std::sync::atomic::Ordering::Release);
        }
        self.player_command = false;
    }
}

/// Forge consumers retain only their own source stream, with a local cursor.
#[derive(Default)]
pub(crate) struct RetainedStream {
    pub entries: VecDeque<(u64, Event)>,
    pub next: u64,
    bytes: usize,
}
impl RetainedStream {
    fn push(&mut self, event: &Event) {
        let index = self.next;
        self.next += 1;
        while self.entries.len() >= 1024 || self.bytes.saturating_add(event.bytes) > 2 * 1024 * 1024
        {
            let Some((_, old)) = self.entries.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(old.bytes);
        }
        if event.bytes <= 2 * 1024 * 1024 {
            self.bytes += event.bytes;
            self.entries.push_back((index, event.clone()));
        }
    }
    fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

pub(crate) struct Core {
    pub view: View,
    pub generation: u64,
    pub tick: u64,
    pub frame: u64,
    pub seq: u64,
    pub combat_total: u64,
    pub words: Option<vi::strtab::StringTable>,
    pub events: VecDeque<Event>,
    pub stream_counts: BTreeMap<String, u64>,
    event_bytes: usize,
    motion_tick: u64,
    pending_player_before: Option<Json>,
    pub forge_cooldowns: RetainedStream,
    pub forge_combat: RetainedStream,
    pub chat: VecDeque<Json>,
    pub stats_received: bool,
    pub effects_received: bool,
    pub entity_worlds: BTreeMap<u32, i32>,
    /// Where an entity was last found in `view.entities`: a hint, checked before it is used, so the
    /// list is scanned only when the hint is wrong.
    pub entity_hint: std::collections::HashMap<u32, usize>,
    pub entity_versions: BTreeMap<u32, u64>,
    pub ability_versions: BTreeMap<String, u64>,
    /// Per item list, the incarnation of each stack by its wire key (`native_items`).
    pub item_versions: BTreeMap<&'static str, BTreeMap<u32, u64>>,
    /// Per item list, the revision the container last published with.
    pub item_revisions: BTreeMap<&'static str, u64>,
    pub quest_revision: u64,
    pub mastery_flags: MasteryFlags,
    pub cm_pending_ops: u16,
    /// `gAttackWords` (the start menu's data), for the attack-type names.
    pub attack_words: Vec<(i32, i64)>,
    pub cooldown_edges: BTreeMap<String, (f64, f64, f64)>,
    pub unknown_lockouts: BTreeMap<u8, (f64, f64, f64)>,
    pub disposition_target: Option<u32>,
    pub selected_target: Option<u32>,
    pub text_channel: &'static str,
    next_version: u64,
    started: Instant,
    source_time: Option<f64>,
}
impl Default for Core {
    fn default() -> Self {
        Self {
            view: View::default(),
            generation: 0,
            tick: 0,
            frame: 0,
            seq: 0,
            combat_total: 0,
            words: None,
            events: VecDeque::new(),
            stream_counts: BTreeMap::new(),
            event_bytes: 0,
            motion_tick: u64::MAX,
            pending_player_before: None,
            forge_cooldowns: RetainedStream::default(),
            forge_combat: RetainedStream::default(),
            chat: VecDeque::new(),
            stats_received: false,
            effects_received: false,
            entity_worlds: BTreeMap::new(),
            entity_hint: std::collections::HashMap::new(),
            entity_versions: BTreeMap::new(),
            ability_versions: BTreeMap::new(),
            item_versions: BTreeMap::new(),
            item_revisions: BTreeMap::new(),
            quest_revision: 0,
            mastery_flags: MasteryFlags::default(),
            cm_pending_ops: 0,
            attack_words: Vec::new(),
            cooldown_edges: BTreeMap::new(),
            unknown_lockouts: BTreeMap::new(),
            disposition_target: None,
            selected_target: None,
            text_channel: "system",
            next_version: 0,
            started: Instant::now(),
            source_time: None,
        }
    }
}
thread_local! { pub(crate) static CORE: RefCell<Core> = RefCell::new(Core::default()); }
pub(crate) fn time() -> f64 {
    CORE.with(|c| c.borrow().started.elapsed().as_secs_f64())
}
pub(crate) fn time_unborrowed(c: &Core) -> f64 {
    c.started.elapsed().as_secs_f64()
}
pub(crate) fn presentation() {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.frame += 1;
        PRESENTATION_COUNT.with(|f| f.set(c.frame));
    });
}
pub(crate) fn iteration(game_step: bool) {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.tick += u64::from(game_step);
    });
}
pub(crate) fn emit(name: &str, data: Json) {
    CORE.with(|c| c.borrow_mut().emit(name, data));
}

impl Core {
    pub(crate) fn receipt_time(&self) -> f64 {
        self.source_time
            .unwrap_or_else(|| self.started.elapsed().as_secs_f64())
    }
    pub(crate) fn next_incarnation(&mut self) -> u64 {
        self.next_version += 1;
        self.next_version
    }

    pub fn emit(&mut self, name: &str, data: Json) {
        let _timed = super::addon_perf::section("events.emit");
        match self.stream_counts.get_mut(name) {
            Some(n) => *n += 1,
            None => {
                self.stream_counts.insert(name.into(), 1);
            }
        }
        if name == "combat.hit" {
            self.combat_total += 1;
        }
        self.seq += 1;
        let bytes = json_len(&data) + 128;
        while self.events.len() >= 2048 || self.event_bytes.saturating_add(bytes) > 4 * 1024 * 1024
        {
            let Some(old) = self.events.pop_front() else {
                break;
            };
            self.event_bytes = self.event_bytes.saturating_sub(old.bytes);
        }
        if bytes > 4 * 1024 * 1024 {
            return;
        }
        self.event_bytes += bytes;
        let event = Event {
            player_command: false,
            player_action: ((name == "input.key"
                && data["down"] == true
                && data["synthetic"] != true
                && data["repeat"] != true)
                || (name == "pointer.pressed" && data["inside_window"] == true))
                .then(|| std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0))),
            name: name.into(),
            seq: self.seq,
            time: self
                .source_time
                .unwrap_or_else(|| self.started.elapsed().as_secs_f64()),
            tick: self.tick,
            frame: self.frame,
            generation: self.generation,
            data: std::sync::Arc::new(data),
            bytes,
        };
        if event.name.starts_with("cooldown.") {
            self.forge_cooldowns.push(&event);
        }
        if event.name == "combat.hit" {
            self.forge_combat.push(&event);
        }
        super::native_events::publish(&event);
        self.events.push_back(event);
    }
    pub(crate) fn player_changed(&mut self, before: Json) {
        if !super::native_events::interested("player.changed") {
            self.pending_player_before = None;
            return;
        }
        let motion = [
            "position",
            "velocity",
            "heading",
            "speed",
            "horizontal_speed",
            "location",
        ];
        let non_motion = before
            .as_object()
            .into_iter()
            .flat_map(|v| v.keys())
            .chain(
                self.view
                    .player
                    .as_object()
                    .into_iter()
                    .flat_map(|v| v.keys()),
            )
            .any(|key| !motion.contains(&key.as_str()) && before[key] != self.view.player[key]);
        if non_motion || self.motion_tick != self.tick {
            let before = self.pending_player_before.take().unwrap_or(before);
            self.emit_player_change(before);
        } else if self.pending_player_before.is_none() {
            self.pending_player_before = Some(before);
        }
    }
    fn emit_player_change(&mut self, before: Json) {
        self.motion_tick = self.tick;
        let changed = changed_fields(&before, &self.view.player);
        self.emit(
            "player.changed",
            json!({"before":before,"after":self.view.player,"changed":changed}),
        );
    }
    pub(crate) fn flush_player_motion(&mut self) {
        if !super::native_events::interested("player.changed") {
            self.pending_player_before = None;
        } else if self.motion_tick != self.tick {
            if let Some(before) = self.pending_player_before.take() {
                self.emit_player_change(before);
            }
        }
    }
    pub fn reset(&mut self, reason: &str) {
        super::input::reset_input_state();
        if self.view.in_world {
            self.emit(
                "world.leaving",
                json!({"reason":reason,"world_id":self.view.world_id}),
            );
        }
        self.items_reset_events();
        self.generation += 1;
        self.pending_player_before = None;
        self.motion_tick = u64::MAX;
        self.forge_cooldowns.clear();
        self.forge_combat.clear();
        self.entity_worlds.clear();
        self.entity_hint.clear();
        self.entity_versions.clear();
        self.ability_versions.clear();
        self.item_versions.clear();
        self.item_revisions.clear();
        self.quest_revision = 0;
        self.mastery_flags = MasteryFlags::default();
        self.cm_pending_ops = 0;
        self.cooldown_edges.clear();
        self.unknown_lockouts.clear();
        self.disposition_target = None;
        self.selected_target = None;
        self.stats_received = false;
        self.effects_received = false;
        self.words = None;
        self.chat.clear();
        self.view = View::default();
        super::guides::update(self);
        self.emit("state.reset", json!({"in_world":false,"initial":false}));
    }
    pub fn publish(&mut self, mut view: View) {
        let target = view.player["target_id"].as_u64().map(|id| id as u32);
        if target != self.selected_target {
            self.disposition_target = None;
            self.selected_target = target;
            for e in &mut view.entities {
                e["disposition"] = Json::Null;
                e["disposition_name"] = Json::Null;
            }
        }
        let entered = view.in_world && !self.view.in_world;
        if !view.in_world && self.view.in_world {
            self.reset(if view.phase == "disconnected" {
                "disconnect"
            } else {
                "zone"
            });
        }
        if entered && self.generation == 0 {
            self.generation = 1;
        }
        self.entity_versions.retain(|id, _| {
            view.entities
                .iter()
                .any(|r| r["id"].as_u64() == Some(*id as u64))
        });
        for r in &mut view.entities {
            let id = r["id"].as_u64().unwrap_or(0) as u32;
            let version = *self.entity_versions.entry(id).or_insert_with(|| {
                self.next_version += 1;
                self.next_version
            });
            r["ref"] = json!({"$ref":"entity","key":id.to_string(),"generation":self.generation,"incarnation":version});
        }
        self.ability_versions
            .retain(|id, _| view.abilities.iter().any(|r| r["id"].as_str() == Some(id)));
        for r in &mut view.abilities {
            let id = r["id"].as_str().unwrap_or("").to_string();
            let version = *self.ability_versions.entry(id.clone()).or_insert_with(|| {
                self.next_version += 1;
                self.next_version
            });
            r["ref"] = json!({"$ref":"ability","key":id,"generation":self.generation,"incarnation":version});
        }
        let refs = view
            .entities
            .iter()
            .filter_map(|e| Some((e["id"].as_u64()?, e["ref"].clone())))
            .collect::<BTreeMap<_, _>>();
        let entity_ref = |id: &Json| {
            id.as_u64()
                .and_then(|id| refs.get(&id))
                .cloned()
                .unwrap_or(Json::Null)
        };
        if view.player.is_object() {
            view.player["entity"] = entity_ref(&view.player["id"]);
            view.player["mastery"] = view.mastery.clone().unwrap_or(Json::Null);
        }
        let quests_same = view.quests.len() == self.view.quests.len()
            && view
                .quests
                .iter()
                .zip(&self.view.quests)
                .all(|(a, b)| a["index"] == b["index"] && a["title"] == b["title"]);
        if !quests_same || self.quest_revision == 0 {
            self.quest_revision += 1;
        }
        for q in &mut view.quests {
            let key = q["index"].as_u64().unwrap_or(0).to_string();
            q["ref"] = json!({"$ref":"quest","key":key,"generation":self.generation,"incarnation":self.quest_revision});
        }
        if let Some(members) = view.group["members"].as_array_mut() {
            for m in members {
                m["entity"] = entity_ref(&m["id"]);
            }
        }
        for bar in &mut view.hotbar {
            if let Some(slots) = bar["slots"].as_array_mut() {
                for slot in slots {
                    if let Some(id) = slot["ability_id"].as_str() {
                        slot["ability"] = view
                            .abilities
                            .iter()
                            .find(|r| r["id"].as_str() == Some(id))
                            .map(|r| r["ref"].clone())
                            .unwrap_or(Json::Null);
                    }
                    slot.as_object_mut().unwrap().remove("ability_id");
                }
            }
        }
        if entered {
            if self.generation == 0 {
                self.generation = 1;
            }
            self.emit(
                "world.entered",
                json!({"world_id":view.world_id,"initial":false}),
            );
            self.emit("state.reset", json!({"in_world":true,"initial":false}));
        }
        let mut before = self.view.map.clone();
        let mut after = view.map.clone();
        if let Some(m) = before.as_object_mut() {
            m.remove("revision");
        }
        if let Some(m) = after.as_object_mut() {
            m.remove("revision");
        }
        let revision = self.view.map["revision"].as_u64().unwrap_or(0);
        if view.map.is_object() {
            view.map["revision"] = json!(revision + u64::from(before != after));
        }
        if before != after {
            self.emit("map.changed", json!({"before":self.view.map,"after":view.map,"changed":changed_fields(&before,&after)}));
        }
        let item_changes = self.items_prepare(&mut view, entered);
        let previous = std::mem::replace(&mut self.view, view);
        self.items_emit(item_changes);
        super::guides::update(self);
        if self.view.in_world && previous.player_stats != self.view.player_stats {
            self.emit("player.stats_changed", json!({}));
        }
        if !entered && self.view.in_world {
            if previous.quests != self.view.quests {
                self.emit(
                    "quests.changed",
                    json!({"before":previous.quests,"after":self.view.quests,"changed":["records"]}),
                );
            }
            if previous.mastery != self.view.mastery {
                let before = previous.mastery.as_ref().unwrap_or(&Json::Null);
                let after = self.view.mastery.as_ref().unwrap_or(&Json::Null);
                self.emit(
                    "mastery.changed",
                    json!({"before":previous.mastery,"after":self.view.mastery,"changed":changed_fields(before,after)}),
                );
            }
        }

        if !entered && self.view.in_world {
            if previous.player["target_id"] != self.view.player["target_id"] {
                let before = previous
                    .entities
                    .iter()
                    .find(|e| e["id"] == previous.player["target_id"])
                    .map(|e| e["ref"].clone());
                let after = self
                    .view
                    .entities
                    .iter()
                    .find(|e| e["id"] == self.view.player["target_id"])
                    .map(|e| e["ref"].clone());
                self.emit("target.changed", json!({"before":before,"after":after}));
            }
            if previous.player != self.view.player {
                self.player_changed(previous.player);
            }
        }
    }
}
pub(crate) fn changed_fields(before: &Json, after: &Json) -> Vec<String> {
    if before.is_array() || after.is_array() {
        return vec!["records".into()];
    }
    // the keys whose value differs (a missing one reads as null), sorted
    let empty = serde_json::Map::new();
    let (b, a) = (before.as_object().unwrap_or(&empty), after.as_object().unwrap_or(&empty));
    let mut keys: Vec<&String> = a.iter().filter(|(k, v)| b.get(*k).unwrap_or(&Json::Null) != *v).map(|(k, _)| k).collect();
    keys.extend(b.iter().filter(|(k, v)| !a.contains_key(*k) && !v.is_null()).map(|(k, _)| k));
    keys.sort_unstable();
    keys.into_iter().cloned().collect()
}

/// The length `serde_json` writes for `value`, worked out without writing it (every event's size is
/// counted for the queue limits, and an entity event is two whole records).
pub(crate) fn json_len(value: &Json) -> usize {
    use std::fmt::Write;
    struct Count(usize);
    impl Write for Count {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0 += s.len();
            Ok(())
        }
    }
    fn string(s: &str) -> usize {
        2 + s
            .bytes()
            .map(|b| match b {
                b'"' | b'\\' | 0x08 | 0x09 | 0x0a | 0x0c | 0x0d => 2,
                0..=0x1f => 6,
                _ => 1,
            })
            .sum::<usize>()
    }
    match value {
        Json::Null => 4,
        Json::Bool(true) => 4,
        Json::Bool(false) => 5,
        Json::Number(n) => {
            let mut c = Count(0);
            let _ = write!(c, "{n}");
            c.0
        }
        Json::String(s) => string(s),
        Json::Array(items) => 2 + items.len().saturating_sub(1) + items.iter().map(json_len).sum::<usize>(),
        Json::Object(fields) => {
            2 + fields.len().saturating_sub(1) + fields.iter().map(|(k, v)| string(k) + 1 + json_len(v)).sum::<usize>()
        }
    }
}

pub(crate) fn observe_text(text: &str, color: [f32; 4]) {
    if !capture_requested() {
        return;
    }
    let mut end = text.len().min(8192);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let text = text[..end].to_owned();
    let channel = CORE.with(|c| c.borrow().text_channel);
    defer(text.len() + 256, move |c| {
        let message = json!({"seq":c.seq+1,"time":c.source_time.unwrap_or_else(||time_unborrowed(c)),"text":text,"color":{"r":color[0],"g":color[1],"b":color[2],"a":color[3]},"channel":channel});
        if c.chat.len() == 1000 {
            c.chat.pop_front();
        }
        c.chat.push_back(message.clone());
        c.emit("chat.message", message);
    });
}
pub(crate) fn observe_hit(attacker: u32, defender: u32, amount: i32, color: Option<[u8; 3]>) {
    defer(256, move |c| {
        let me = c.view.player["id"].as_u64();
        let pet = c.view.player["pet_id"].as_u64();
        c.emit("combat.hit",json!({"attacker_id":attacker,"defender_id":defender,"amount":amount.unsigned_abs(),"kind":if amount>0 {"heal"}else{"damage"},"color":color,"outgoing":me==Some(attacker as u64),"incoming":me==Some(defender as u64),"from_pet":pet==Some(attacker as u64),"to_pet":pet==Some(defender as u64)}));
    });
}

thread_local! {
    static MANAGER_DEMAND: std::cell::Cell<Option<u64>> = const {std::cell::Cell::new(None)};
    static PRESENTATION_COUNT: std::cell::Cell<u64> = const {std::cell::Cell::new(0)};
    #[cfg(test)]
    static FIXTURE_DEMAND: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};
}
pub(crate) fn manager_demand(wanted: bool) {
    MANAGER_DEMAND
        .with(|d| d.set(wanted.then(|| PRESENTATION_COUNT.with(|f| f.get().saturating_add(1)))));
}
#[cfg(test)]
pub(crate) fn fixture_demand(wanted: bool) {
    FIXTURE_DEMAND.with(|d| d.set(wanted));
}
pub(crate) fn capture_requested() -> bool {
    #[cfg(test)]
    if FIXTURE_DEMAND.with(|d| d.get()) {
        return true;
    }
    MANAGER_DEMAND.with(|d| {
        let wanted = d
            .get()
            .is_some_and(|until| PRESENTATION_COUNT.with(|f| f.get() <= until));
        if !wanted {
            d.set(None);
        }
        wanted
    }) || super::native_events::has_consumers()
}

// Source sites enqueue only compact owned Rust values. Serialization runs at Lua boundaries.
type PendingApply = Box<dyn FnOnce(&mut Core)>;
struct Pending {
    apply: PendingApply,
    time: f64,
    generation: u64,
    tick: u64,
    frame: u64,
    bytes: usize,
}
#[derive(Default)]
struct Deferred {
    queue: VecDeque<Pending>,
    bytes: usize,
    lost: u64,
    flushing: bool,
}
thread_local! {static DEFERRED:RefCell<Deferred>=RefCell::new(Deferred::default());}
pub(crate) fn defer(bytes: usize, apply: impl FnOnce(&mut Core) + 'static) {
    if !capture_requested() {
        return;
    }
    let (time, tick, frame, generation) = CORE.with(|c| {
        let c = c.borrow();
        (time_unborrowed(&c), c.tick, c.frame, c.generation)
    });
    let dropped = DEFERRED.with(|d| {
        let mut d = d.borrow_mut();
        let before = d.lost;
        while d.queue.len() >= 8192 || d.bytes.saturating_add(bytes) > 32 * 1024 * 1024 {
            let Some(old) = d.queue.pop_front() else {
                break;
            };
            d.bytes = d.bytes.saturating_sub(old.bytes);
            d.lost += 1;
            CAPTURE_EPOCH.with(|e| e.set(e.get().wrapping_add(1)));
        }
        if bytes > 32 * 1024 * 1024 {
            d.lost += 1;
            CAPTURE_EPOCH.with(|e| e.set(e.get().wrapping_add(1)));
            return d.lost - before;
        }
        d.bytes += bytes;
        d.queue.push_back(Pending {
            apply: Box::new(apply),
            time,
            generation,
            tick,
            frame,
            bytes,
        });
        d.lost - before
    });
    if dropped != 0 {
        super::native_events::capture_diagnostic(
            "quota",
            &format!(
                "source.capture overflow discarded {dropped} pending deltas in this batch; current-state bootstrap required"
            ),
        );
    }
}

fn source_loss(c: &mut Core, count: u64, stream: &str) {
    if count == 0 {
        return;
    }
    let first = c.seq + 1;
    let last = c.seq + count;
    c.emit(
        "events.dropped",
        json!({"count":count,"streams":[stream],"first_seq":first,"last_seq":last}),
    );
}
fn generation_loss(c: &mut Core, count: u64) {
    if count != 0 {
        super::native_events::capture_diagnostic(
            "cancelled",
            &format!(
                "source.generation discarded {count} pending deltas after the world generation changed"
            ),
        );
        source_loss(c, count, "source.generation");
    }
}
pub(crate) fn flush_deferred() {
    let _timed = super::addon_perf::section("deferred.flush");
    let batch = DEFERRED.with(|d| {
        let mut d = d.borrow_mut();
        if d.flushing {
            return None;
        }
        d.flushing = true;
        d.bytes = 0;
        Some((std::mem::take(&mut d.queue), std::mem::take(&mut d.lost)))
    });
    let Some((batch, lost)) = batch else { return };
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        let saved = (c.tick, c.frame, c.source_time);
        if lost != 0 {
            source_loss(&mut c, lost, "source.capture");
            let world = c.view.in_world;
            c.emit("state.reset", json!({"in_world":world,"initial":false}));
        }
        let stale = batch
            .iter()
            .filter(|p| p.generation != c.generation)
            .count() as u64;
        generation_loss(&mut c, stale);
        for pending in batch {
            if pending.generation != c.generation {
                continue;
            }
            c.tick = pending.tick;
            c.frame = pending.frame;
            c.source_time = Some(pending.time);
            (pending.apply)(&mut c);
        }
        super::guides::update(&mut c);
        (c.tick, c.frame, c.source_time) = saved;
    });
    DEFERRED.with(|d| d.borrow_mut().flushing = false);
}
#[cfg(test)]
pub(crate) fn deferred_len() -> usize {
    DEFERRED.with(|d| d.borrow().queue.len())
}
pub(crate) fn discard_deferred() {
    let has_queue = DEFERRED.with(|d| !d.borrow().queue.is_empty());
    let stale = if has_queue {
        let generation = CORE.with(|c| c.borrow().generation);
        DEFERRED.with(|d| {
            d.borrow()
                .queue
                .iter()
                .filter(|p| p.generation != generation)
                .count() as u64
        })
    } else {
        0
    };
    DEFERRED.with(|d| {
        let mut d = d.borrow_mut();
        d.queue.clear();
        d.bytes = 0;
        d.lost = 0;
    });
    if stale != 0 {
        CORE.with(|c| generation_loss(&mut c.borrow_mut(), stale));
    }
}

#[cfg(test)]
pub(crate) fn fixture_requested() -> bool {
    FIXTURE_DEMAND.with(|d| d.get())
}

thread_local! {static CAPTURE_EPOCH:std::cell::Cell<u64>=const{std::cell::Cell::new(0)};}
pub(crate) fn capture_epoch() -> u64 {
    CAPTURE_EPOCH.with(|e| e.get())
}
pub(crate) fn invalidate_capture() {
    CAPTURE_EPOCH.with(|e| e.set(e.get().wrapping_add(1)));
    discard_deferred();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.pending_player_before = None;
        c.motion_tick = u64::MAX;
        c.unknown_lockouts.clear();
    });
}

pub(crate) fn unknown_lockout(key: u8, lockout: f64) {
    defer(256, move |c| {
        let previous = c.unknown_lockouts.get(&key).copied();
        let old = previous.map_or(0., |v| v.2);
        if old == lockout {
            return;
        }
        let now = c.receipt_time();
        if lockout == 0. {
            c.unknown_lockouts.remove(&key);
        } else {
            c.unknown_lockouts
                .insert(key, (previous.map_or(now, |v| v.0), now, lockout));
        }
        c.emit(
            if lockout == 0. {
                "cooldown.ended"
            } else if old == 0. {
                "cooldown.started"
            } else {
                "cooldown.updated"
            },
            json!({"ability":null,"ability_id":null,"display_key":key,"lockout":lockout}),
        );
    });
}
