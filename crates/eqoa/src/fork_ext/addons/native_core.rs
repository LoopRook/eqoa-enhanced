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
    pub entities: Vec<Json>,
    pub entity_slots: Vec<Option<u32>>,
    pub abilities: Vec<Json>,
    pub group: Json,
    pub hotbar: Vec<Json>,
    pub effects: Option<Vec<Json>>,
    pub channel: &'static str,
}
impl Default for View {
    fn default() -> Self {
        Self {
            in_world: false,
            phase: "pregame",
            world_id: None,
            player: Json::Null,
            entities: Vec::new(),
            entity_slots: Vec::new(),
            abilities: Vec::new(),
            group: json!({"in_group":false,"self_leader":null,"members":[],"raid":[],"invites":[]}),
            hotbar: Vec::new(),
            effects: None,
            channel: "other",
        }
    }
}

#[derive(Clone)]
pub(crate) struct Event {
    pub name: String,
    pub seq: u64,
    pub time: f64,
    pub tick: u64,
    pub frame: u64,
    pub generation: u64,
    pub data: Json,
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
    pub chat: VecDeque<Json>,
    pub stats_received: bool,
    pub effects_received: bool,
    pub entity_versions: BTreeMap<u32, u64>,
    pub ability_versions: BTreeMap<String, u64>,
    next_version: u64,
    started: Instant,
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
            chat: VecDeque::new(),
            stats_received: false,
            effects_received: false,
            entity_versions: BTreeMap::new(),
            ability_versions: BTreeMap::new(),
            next_version: 0,
            started: Instant::now(),
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
pub(crate) fn iteration(game_step: bool) {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.frame += 1;
        c.tick += u64::from(game_step);
    });
}
pub(crate) fn emit(name: &str, data: Json) {
    CORE.with(|c| c.borrow_mut().emit(name, data));
}

impl Core {
    pub fn emit(&mut self, name: &str, data: Json) {
        if name == "combat.hit" {
            self.combat_total += 1;
        }
        self.seq += 1;
        if self.events.len() == 2048 {
            self.events.pop_front();
        }
        self.events.push_back(Event {
            name: name.into(),
            seq: self.seq,
            time: self.started.elapsed().as_secs_f64(),
            tick: self.tick,
            frame: self.frame,
            generation: self.generation,
            data,
        });
    }
    pub fn reset(&mut self, reason: &str) {
        super::input::reset_input_state();
        if self.view.in_world {
            self.emit(
                "world.leaving",
                json!({"reason":reason,"world_id":self.view.world_id}),
            );
        }
        self.generation += 1;
        self.entity_versions.clear();
        self.ability_versions.clear();
        self.stats_received = false;
        self.effects_received = false;
        self.chat.clear();
        self.view = View::default();
        self.emit("state.reset", json!({"in_world":false,"initial":false}));
    }
    pub fn publish(&mut self, mut view: View, events: bool) {
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
        let entities = view.entities.clone();
        let entity_ref = |id: &Json| {
            entities
                .iter()
                .find(|r| r["id"] == *id)
                .map(|r| r["ref"].clone())
                .unwrap_or(Json::Null)
        };
        if view.player.is_object() {
            view.player["entity"] = entity_ref(&view.player["id"]);
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
        } else if events && view.in_world {
            for (name, before, after) in [
                (
                    "player.changed",
                    self.view.player.clone(),
                    view.player.clone(),
                ),
                ("group.changed", self.view.group.clone(), view.group.clone()),
                (
                    "abilities.changed",
                    json!(self.view.abilities),
                    json!(view.abilities),
                ),
                (
                    "hotbar.changed",
                    json!(self.view.hotbar),
                    json!(view.hotbar),
                ),
                (
                    "effects.changed",
                    json!(self.view.effects),
                    json!(view.effects),
                ),
            ] {
                if before != after {
                    let changed = changed_fields(&before, &after);
                    self.emit(
                        name,
                        json!({"before":before,"after":after,"changed":changed}),
                    );
                }
            }
            if self.view.player["target_id"] != view.player["target_id"] {
                let before = self
                    .view
                    .entities
                    .iter()
                    .find(|r| r["id"] == self.view.player["target_id"])
                    .map(|r| r["ref"].clone());
                self.emit(
                    "target.changed",
                    json!({"before":before,"after":entity_ref(&view.player["target_id"])}),
                );
            }
            let old = self.view.entities.clone();
            for before in &old {
                if !view.entities.iter().any(|r| r["ref"] == before["ref"]) {
                    self.emit(
                        "entity.removed",
                        json!({"ref":before["ref"],"before":before,"after":null,"changed":[]}),
                    );
                }
            }
            for after in &view.entities {
                let before = old.iter().find(|r| r["ref"] == after["ref"]);
                if before != Some(after) {
                    self.emit(if before.is_some() {"entity.changed"} else {"entity.added"},json!({"ref":after["ref"],"before":before,"after":after,"changed":changed_fields(before.unwrap_or(&Json::Null),after)}));
                }
            }
        }
        self.view = view;
    }
}
fn changed_fields(before: &Json, after: &Json) -> Vec<String> {
    if before.is_array() || after.is_array() {
        return vec!["records".into()];
    }
    let mut keys = std::collections::BTreeSet::new();
    for value in [before, after] {
        if let Some(o) = value.as_object() {
            keys.extend(o.keys().cloned());
        }
    }
    keys.into_iter().filter(|k| before[k] != after[k]).collect()
}

pub(crate) fn observe_text(text: &str, color: [f32; 4]) {
    CORE.with(|c| { let mut c=c.borrow_mut(); let message=json!({"seq":c.seq+1,"time":c.started.elapsed().as_secs_f64(),"text":text.chars().take(8192).collect::<String>(),"color":{"r":color[0],"g":color[1],"b":color[2],"a":color[3]},"channel":"system"});
        if c.chat.len()==1000 {c.chat.pop_front();} c.chat.push_back(message.clone()); c.emit("chat.message",message);
    });
}
pub(crate) fn observe_hit(attacker: u32, defender: u32, amount: i32, color: Option<[u8; 3]>) {
    CORE.with(|c| {let mut c=c.borrow_mut(); let me=c.view.player["id"].as_u64();let pet=c.view.player["pet_id"].as_u64();
        c.emit("combat.hit",json!({"attacker_id":attacker,"defender_id":defender,"amount":amount.unsigned_abs(),"kind":if amount>0 {"heal"}else{"damage"},"color":color,"outgoing":me==Some(attacker as u64),"incoming":me==Some(defender as u64),"from_pet":pet==Some(attacker as u64),"to_pet":pet==Some(defender as u64)}));
    });
}
