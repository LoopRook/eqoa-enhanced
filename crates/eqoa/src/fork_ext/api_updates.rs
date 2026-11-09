//! Typed, bounded source deltas; JSON conversion occurs only at an addon boundary.
use super::*;
use crate::fork_ext::addons::{
    native_core::{self, Core},
    native_events,
};
use std::cell::{Cell, RefCell};

#[derive(Clone, Debug, PartialEq)]
struct EntityData {
    id: u32,
    name: String,
    level: Option<u8>,
    position: [f32; 3],
    hp: u8,
    code: Option<u8>,
    target: Option<u32>,
    flags: Option<u8>,
    disposition: Option<u8>,
    world: i32,
}
impl EntityData {
    fn read(p: &Play, e: &net::entity::Entity) -> Self {
        let creature = e.creature.as_ref();
        Self {
            id: e.id,
            name: creature.map_or("", |c| c.name.as_str()).into(),
            level: creature.map(|c| c.level),
            position: e.pos,
            hp: e.p19,
            code: creature.map(|c| c.p256),
            target: creature.and_then(|c| valid_id(c.target)),
            flags: creature.map(|c| c.flags262),
            disposition: (e.id == p.target
                && CORE.with(|c| c.borrow().disposition_target == Some(p.target)))
            .then_some(p.target_info)
            .flatten(),
            world: e.world as i32,
        }
    }
    fn value(&self) -> Json {
        json!({"id":self.id,"name":self.name,"level":self.level,"position":vec3(self.position),"hp_fraction":(self.hp!=0).then(||ee::div(self.hp as f32,255.)),"kind":match self.code{Some(2)=>"player",Some(0|1)=>"npc",Some(3)=>"pet",_=>"other"},"class_code":self.code,"target_id":self.target,"status_flags":self.flags,"poisoned":self.flags.map(|f|f&4!=0),"diseased":self.flags.map(|f|f&8!=0),"disposition":self.disposition,"disposition_name":self.disposition.and_then(|d|match d{0..=1=>Some("Hostile"),2..=3=>Some("Neutral"),4..=6=>Some("Friendly"),_=>None})})
    }
}
#[derive(Clone)]
struct PlayerData {
    id: Option<u32>,
    name: String,
    player: net::dump::Player,
    stats: net::dump::Stats,
    known: bool,
    health: crate::fork_ext::addons::health::HealthSnapshot,
    target: Option<u32>,
    pet: Option<u32>,
    auto: bool,
    dead: bool,
    fighting: Option<bool>,
}
impl PlayerData {
    fn read(p: &Play) -> Self {
        let id = p.self_id32();
        let health = p
            .forge_health
            .snapshot(p.session.self_id.is_some(), p.in_world_now());
        Self {
            id,
            name: p.player.name.clone(),
            player: p.dump.player.clone(),
            stats: p.stats,
            known: CORE.with(|c| c.borrow().stats_received) || health.available,
            health,
            target: valid_id(p.target),
            pet: valid_id(p.pet_id),
            auto: p.auto_attack,
            dead: p.death.is_some(),
            fighting: id
                .and_then(|id| p.entities.slots.iter().flatten().find(|e| e.id == id))
                .and_then(|e| e.creature.as_ref())
                .map(|c| c.p263 != 0),
        }
    }
    fn value(&self, c: &Core) -> Json {
        let p = &self.player;
        let s = &self.stats;
        let h = self.health;
        let known = self.known;
        let mut value = json!({"id":self.id,"name":self.name,"class_id":p.class,"class_name":usize::try_from(p.class).ok().and_then(|i|crate::fork_ext::addons::player::CLASSES.get(i)),"race_id":p.race,"race_name":usize::try_from(p.race).ok().and_then(|i|crate::fork_ext::addons::player::RACES.get(i)),"level":p.level,"experience":p.exp,"experience_debt":p.p2c,"hp":known.then(||json!({"current":if h.available{h.current_hp}else{s.stat[7]},"maximum":if h.available{h.max_hp}else{s.stat[8]},"base":s.base[8]})),"power":known.then(||json!({"current":s.stat[9],"maximum":s.stat[10],"base":s.base[10]})),"attributes":known.then(||json!({"base":stats(&s.base),"total":stats(&s.stat)})),"armor":known.then_some(s.stat[14]),"resists":known.then(||json!({"base":40+s.stat[4].div_euclid(7),"poison":s.stat[22],"disease":s.stat[23],"fire":s.stat[24],"cold":s.stat[25],"lightning":s.stat[26],"arcane":s.stat[27]})),"target_id":self.target,"pet_id":self.pet,"auto_attacking":self.auto,"dead":self.dead,"has_pet":self.pet.is_some(),"fighting":self.fighting,"mastery":null});
        for field in [
            "position",
            "velocity",
            "heading",
            "speed",
            "horizontal_speed",
            "location",
            "entity",
        ] {
            value[field] = c.view.player[field].clone();
        }
        value
    }
}
fn interested(name: &str) -> bool {
    native_events::interested(name)
}
fn ref_for(c: &Core, id: &Json) -> Json {
    c.view
        .entities
        .iter()
        .find(|e| e["id"] == *id)
        .map_or(Json::Null, |e| e["ref"].clone())
}
fn patch_player(c: &mut Core, mut after: Json) {
    after["entity"] = ref_for(c, &after["id"]);
    if after == c.view.player {
        return;
    }
    let target_changed = after["target_id"] != c.view.player["target_id"];
    let before = std::mem::replace(&mut c.view.player, after);
    if target_changed {
        if c.selected_target != c.view.player["target_id"].as_u64().map(|id| id as u32) {
            c.disposition_target = None;
        }
        if let Some(old) = c
            .view
            .entities
            .iter_mut()
            .find(|e| e["id"] == before["target_id"])
        {
            old["disposition"] = Json::Null;
            old["disposition_name"] = Json::Null;
        }
        c.selected_target = c.view.player["target_id"].as_u64().map(|id| id as u32);
        if interested("target.changed") {
            c.emit("target.changed",json!({"before":ref_for(c,&before["target_id"]),"after":ref_for(c,&c.view.player["target_id"])}));
        }
    }
    if interested("player.changed") {
        let after = c.view.player.clone();
        let changed = crate::fork_ext::addons::native_core::changed_fields(&before, &after);
        c.emit(
            "player.changed",
            json!({"before":before,"after":after,"changed":changed}),
        );
    }
}
fn patch_entity(c: &mut Core, slot: usize, entity: Option<EntityData>) {
    while c.view.entity_slots.len() <= slot {
        c.view.entity_slots.push(None);
    }
    let old_id = c.view.entity_slots[slot];
    let new_id = entity.as_ref().map(|e| e.id);
    if old_id != new_id {
        if let Some(old) = old_id {
            c.entity_versions.remove(&old);
            c.entity_worlds.remove(&old);
            if let Some(index) = c
                .view
                .entities
                .iter()
                .position(|e| e["id"].as_u64() == Some(old as u64))
            {
                let before = c.view.entities.remove(index);
                if interested("entity.removed") {
                    c.emit(
                        "entity.removed",
                        json!({"ref":before["ref"],"before":before,"after":null,"changed":[]}),
                    );
                }
            }
        }
        c.view.entity_slots[slot] = new_id;
    }
    let Some(entity) = entity else {
        return;
    };
    c.entity_worlds.insert(entity.id, entity.world);
    let mut after = entity.value();
    let version = if let Some(v) = c.entity_versions.get(&entity.id) {
        *v
    } else {
        let v = c.next_incarnation();
        c.entity_versions.insert(entity.id, v);
        v
    };
    after["ref"] = json!({"$ref":"entity","key":entity.id.to_string(),"generation":c.generation,"incarnation":version});
    let index = c
        .view
        .entities
        .iter()
        .position(|e| e["id"].as_u64() == Some(entity.id as u64));
    let before = if let Some(index) = index {
        if c.view.entities[index] == after {
            return;
        }
        Some(std::mem::replace(
            &mut c.view.entities[index],
            after.clone(),
        ))
    } else {
        c.view.entities.push(after.clone());
        None
    };
    let name = if before.is_some() {
        "entity.changed"
    } else {
        "entity.added"
    };
    if interested(name) {
        let changed = crate::fork_ext::addons::native_core::changed_fields(
            before.as_ref().unwrap_or(&Json::Null),
            &after,
        );
        c.emit(
            name,
            json!({"ref":after["ref"],"before":before,"after":after,"changed":changed}),
        );
    }
    let old_group = interested("group.changed").then(|| c.view.group.clone());
    if let Some(members) = c.view.group["members"].as_array_mut() {
        for member in members {
            if member["id"].as_u64() == Some(entity.id as u64) {
                member["position"] = vec3(entity.position);
                member["world_id"] = json!(entity.world);
                member["entity"] = after["ref"].clone();
            }
        }
    }
    if let Some(before) = old_group {
        if before != c.view.group {
            c.emit(
                "group.changed",
                json!({"before":before,"after":c.view.group,"changed":["members"]}),
            );
        }
    }
    if c.view.player["id"].as_u64() == Some(entity.id as u64) {
        c.view.player["entity"] = after["ref"].clone();
    }
}
fn ability_value(a: &net::dump::Ability) -> Json {
    json!({"id":a.id.to_string(),"name":a.name,"description":a.desc,"display_key":a.display_index,"spellbook_slot":(a.slot>=0).then(||a.slot+1),"usable":a.valid!=0,"level":a.level,"range":a.range,"cast_time":a.cast_time,"power_cost":a.power_cost,"icon_background":(a.icon_back>=0).then_some(a.icon_back),"icon_foreground":(a.icon_front>=0).then_some(a.icon_front),"scope":a.scope,"cooldown":a.cooldown,"equipment_mask":a.equip_req,"lockout":ee::div(a.lockout as f32,1000.)})
}
fn group_value(group: &crate::group::Group, me: Option<u32>, c: &Core) -> Json {
    json!({"in_group":group.in_group,"self_leader":group.in_group.then(||group.members.first().is_some_and(|m|Some(m.id)==me)),"members":if group.in_group{group.members.iter().map(|m|{let actor=c.view.entities.iter().find(|e|e["id"].as_u64()==Some(m.id as u64));json!({"id":m.id,"name":m.name,"active":m.active,"hp_fraction":(m.hp_known!=0).then(||ee::div(m.hp as f32,255.)),"position":actor.map(|e|e["position"].clone()).or_else(||m.active.then(||vec3(m.pos))),"world_id":actor.and_then(|_|c.entity_worlds.get(&m.id).copied()).or_else(||m.active.then_some(m.x)),"entity":actor.map(|e|e["ref"].clone())})}).collect::<Vec<_>>()}else{Vec::new()},"raid":group.raid.iter().map(|m|json!({"id":m.id,"name":m.name,"hp_fraction":ee::div(m.hp as f32,255.),"power_fraction":ee::div(m.power as f32,255.)})).collect::<Vec<_>>(),"invites":group.invites.iter().map(|i|json!({"id":i.id,"name":i.name})).collect::<Vec<_>>()})
}
fn patch_section(c: &mut Core, name: &str, after: Json) {
    let target = match name {
        "group.changed" => &mut c.view.group,
        _ => return,
    };
    if *target == after {
        return;
    }
    let before = std::mem::replace(target, after.clone());
    if interested(name) {
        let changed = crate::fork_ext::addons::native_core::changed_fields(&before, &after);
        c.emit(
            name,
            json!({"before":before,"after":after,"changed":changed}),
        );
    }
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FrameKey([u64; 32]);
thread_local! {static KEY:Cell<Option<FrameKey>>=const{Cell::new(None)};static SNAPSHOT:RefCell<Option<(FrameKey,u64,crate::fork_ext::addons::host::Snapshot)>>=const{RefCell::new(None)};static REVISION:Cell<u64>=const{Cell::new(0)};static LIVE:Cell<bool>=const{Cell::new(false)};}
pub(crate) fn key(play: &Play, walker: Option<&crate::walker::Walker>) -> FrameKey {
    let mut v = [0; 32];
    v[..12].copy_from_slice(&[
        play.self_id32().unwrap_or(0) as u64,
        play.target as u64,
        play.pet_id as u64,
        play.auto_attack as u64,
        play.death.is_some() as u64,
        play.mode as u64,
        play.channel as u64,
        play.hud_menus.sel as u64,
        play.in_world_now() as u64,
        play.dump.player.level as u64,
        play.dump.player.exp as u64,
        play.dump.player.p2c as u64,
    ]);
    for (i, m) in play.hud_menus.menus.iter().enumerate() {
        v[12 + i] = m.sel as u64;
    }
    if let Some(w) = walker {
        let p = w.cam.walk.phys.position;
        let speed = w.cam.walk.phys.velocity;
        v[18..25].copy_from_slice(&[
            p.x.0.to_bits() as u64,
            p.y.0.to_bits() as u64,
            p.z.0.to_bits() as u64,
            speed.x.0.to_bits() as u64,
            speed.y.0.to_bits() as u64,
            speed.z.0.to_bits() as u64,
            w.cam.walk.phys.angles.x.0.to_bits() as u64,
        ]);
    }
    v[26] = play.typing.is_some() as u64;
    v[27] = play.map.open as u64;
    v[28] = play.map.search.open as u64;
    v[29] = play.search.open as u64;
    v[30] = play.start.as_ref().is_some_and(|s| s.menu.visible) as u64;
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for d in &play.hud_menus.defs {
        d.count.hash(&mut h);
        for e in &d.entries {
            e.msg.hash(&mut h);
            e.id.hash(&mut h);
            e.icon2.hash(&mut h);
        }
    }
    let bars = play.hud_bars();
    bars.abilities.hash(&mut h);
    bars.gear.hash(&mut h);
    v[31] = h.finish();
    FrameKey(v)
}
pub(crate) fn inactive() {
    LIVE.with(|l| l.set(false));
    KEY.with(|k| k.set(None));
    SNAPSHOT.with(|s| *s.borrow_mut() = None);
    native_core::discard_deferred();
}
pub(crate) fn revision() -> u64 {
    REVISION.with(|r| r.get().wrapping_add(CORE.with(|c| c.borrow().seq)))
}
pub(crate) fn dirty() {
    REVISION.with(|r| r.set(r.get().wrapping_add(1)));
}
pub(crate) fn cached(key: FrameKey) -> Option<crate::fork_ext::addons::host::Snapshot> {
    SNAPSHOT.with(|s| {
        s.borrow()
            .as_ref()
            .filter(|(k, r, _)| *k == key && *r == revision())
            .map(|(_, _, snapshot)| *snapshot)
    })
}
pub(crate) fn cache(key: FrameKey, snapshot: crate::fork_ext::addons::host::Snapshot) {
    SNAPSHOT.with(|s| *s.borrow_mut() = Some((key, revision(), snapshot)));
}
pub(crate) fn refresh(play: &Play, walker: Option<&crate::walker::Walker>) {
    thread_local! {static EPOCH:Cell<u64>=const{Cell::new(u64::MAX)};}
    let epoch_changed =
        EPOCH.with(|e| e.replace(native_core::capture_epoch())) != native_core::capture_epoch();
    let first = LIVE.with(|l| !l.replace(true)) || epoch_changed;
    #[cfg(test)]
    let manual = native_core::fixture_requested();
    #[cfg(not(test))]
    let manual = false;
    if first || manual || CORE.with(|c| c.borrow().view.in_world != play.in_world_now()) {
        native_core::flush_deferred();
        let view = play.native_view(walker);
        CORE.with(|c| c.borrow_mut().publish(view, false));
        CORE.with(|c| {
            c.borrow_mut().entity_worlds = play
                .entities
                .slots
                .iter()
                .flatten()
                .map(|e| (e.id, e.world as i32))
                .collect()
        });
        KEY.with(|k| k.set(Some(key(play, walker))));
        dirty();
        return;
    }
    if !play.in_world_now() {
        native_core::flush_deferred();
        return;
    }
    native_core::flush_deferred();
    let now = key(play, walker);
    let previous = KEY.with(|k| k.replace(Some(now)));
    if previous != Some(now) {
        if previous.is_none_or(|k| k.0[7..18] != now.0[7..18] || k.0[31] != now.0[31]) {
            let bars = hotbars(play);
            CORE.with(|c| patch_hotbars(&mut c.borrow_mut(), bars));
        }
        let player = PlayerData::read(play);
        CORE.with(|c| {
            let mut c = c.borrow_mut();
            let mut after = player.value(&c);
            if let Some(w) = walker {
                let p = w.cam.walk.phys.position;
                let v = w.cam.walk.phys.velocity;
                let xz = ee::add(ee::mul(v.x.0, v.x.0), ee::mul(v.z.0, v.z.0));
                after["position"] = vec3([p.x.0, p.y.0, p.z.0]);
                after["velocity"] = vec3([v.x.0, v.y.0, v.z.0]);
                after["heading"] =
                    json!(w.cam.walk.phys.angles.x.0.rem_euclid(std::f32::consts::TAU));
                after["speed"] = json!(ee::sqrt(ee::add(xz, ee::mul(v.y.0, v.y.0))));
                after["horizontal_speed"] = json!(ee::sqrt(xz));
                after["location"] = play.native_location(walker);
            }
            c.view.channel = match play.channel {
                0 => "say",
                2 => "shout",
                3 => "group",
                4 => "guild",
                _ => "other",
            };
            patch_player(&mut c, after);
        });
        dirty();
    }
}

// Called after a packet mutation. Only the affected typed records are copied.
pub(crate) fn capture(play: &Play, event: &Kind, cooldown: Option<(i64, f64)>) {
    dirty();
    match event {
        Kind::Entity(slot) => {
            let slot = *slot;
            let entity = play
                .entities
                .slots
                .get(slot)
                .and_then(|e| e.as_ref())
                .map(|e| EntityData::read(play, e));
            let player = entity
                .as_ref()
                .filter(|e| Some(e.id) == play.self_id32())
                .map(|_| PlayerData::read(play));
            native_core::defer(
                512 + entity.as_ref().map_or(0, |e| e.name.len()),
                move |c| patch_entity(c, slot, entity),
            );
            if let Some(player) = player {
                native_core::defer(2048 + player.name.len(), move |c| {
                    let value = player.value(c);
                    patch_player(c, value);
                });
            }
        }
        Kind::Player => {
            let player = PlayerData::read(play);
            native_core::defer(2048 + player.name.len(), move |c| {
                let value = player.value(c);
                patch_player(c, value)
            });
        }
        Kind::Group => {
            let mut group = crate::group::Group::default();
            group.in_group = play.group.in_group;
            group.leader = play.group.leader;
            group.members = play.group.members.clone();
            group.raid = play.group.raid.clone();
            group.invites = play.group.invites.clone();
            let me = play.self_id32();
            native_core::defer(
                2048 + group.members.iter().map(|m| m.name.len()).sum::<usize>()
                    + group.raid.len() * 256,
                move |c| {
                    let value = group_value(&group, me, c);
                    patch_section(c, "group.changed", value)
                },
            );
        }
        Kind::Ability(id) => {
            if let Some(ability) = play.abilities.get(id) {
                let ability = ability.clone();
                let bytes = ability.name.len() + ability.desc.len() + 512;
                native_core::defer(bytes, move |c| {
                    let id = ability.id.to_string();
                    let before = interested("abilities.changed").then(|| c.view.abilities.clone());
                    let version = c.next_incarnation();
                    c.ability_versions.insert(id.clone(), version);
                    let mut value = ability_value(&ability);
                    value["ref"] = json!({"$ref":"ability","key":id,"generation":c.generation,"incarnation":version});
                    if let Some(index) = c
                        .view
                        .abilities
                        .iter()
                        .position(|a| a["id"].as_str() == Some(&id))
                    {
                        c.view.abilities[index] = value;
                    } else {
                        c.view.abilities.push(value);
                        c.view.abilities.sort_by_key(|a| {
                            a["id"].as_str().and_then(|id| id.parse::<i64>().ok())
                        });
                    }
                    if let Some(before) = before {
                        c.emit(
                            "abilities.changed",
                            json!({"before":before,"after":c.view.abilities,"changed":["records"]}),
                        );
                    }
                });
            }
            let bars = hotbars(play);
            native_core::defer(8192, move |c| patch_hotbars(c, bars));
        }
        Kind::Abilities => {
            let records = play
                .abilities
                .values()
                .map(|a| (a.id, a.display_index, a.slot, a.valid))
                .collect::<Vec<_>>();
            native_core::defer(records.len() * 32, move |c| {
                let before = interested("abilities.changed").then(|| c.view.abilities.clone());
                c.view.abilities.retain(|a| {
                    records.iter().any(|r| {
                        a["id"].as_str().and_then(|id| id.parse::<i64>().ok()) == Some(r.0)
                    })
                });
                c.ability_versions
                    .retain(|id, _| records.iter().any(|r| id.parse::<i64>().ok() == Some(r.0)));
                for (id, key, slot, valid) in records {
                    if let Some(a) = c.view.abilities.iter_mut().find(|a| {
                        a["id"].as_str().and_then(|id| id.parse::<i64>().ok()) == Some(id)
                    }) {
                        a["display_key"] = json!(key);
                        a["spellbook_slot"] = json!((slot >= 0).then_some(slot + 1));
                        a["usable"] = json!(valid != 0);
                    }
                }
                if let Some(before) = before {
                    if before != c.view.abilities {
                        c.emit(
                            "abilities.changed",
                            json!({"before":before,"after":c.view.abilities,"changed":["records"]}),
                        );
                    }
                }
            });
            let bars = hotbars(play);
            native_core::defer(8192, move |c| patch_hotbars(c, bars));
        }
        Kind::Effects => {
            let effects = (0..play.effects.count.clamp(0, 8) as usize)
                .map(|i| (play.effects.icon[i], play.effects.name[i].clone()))
                .collect::<Vec<_>>();
            native_core::defer(4096, move |c| {
                let value=Some(effects.into_iter().enumerate().map(|(i,(icon,name))|json!({"index":i+1,"icon":(icon!=u32::MAX).then_some(icon),"name":String::from_utf16_lossy(&name)})).collect::<Vec<_>>());
                let before = std::mem::replace(&mut c.view.effects, value);
                if interested("effects.changed") && before != c.view.effects {
                    c.emit(
                        "effects.changed",
                        json!({"before":before,"after":c.view.effects,"changed":["records"]}),
                    );
                }
            });
        }
        _ => {}
    }
    if let Some((id, old)) = cooldown {
        if let Some(a) = play.abilities.get(&id) {
            let lockout = ee::div(a.lockout as f32, 1000.) as f64;
            let key = a.display_index;
            if old != lockout {
                native_core::defer(256, move |c| {
                    let id = id.to_string();
                    if let Some(value) = c
                        .view
                        .abilities
                        .iter_mut()
                        .find(|a| a["id"].as_str() == Some(&id))
                    {
                        value["lockout"] = json!(lockout);
                    }
                    let now = c.receipt_time();
                    if lockout == 0. {
                        c.cooldown_edges.remove(&id);
                    } else {
                        let start = c.cooldown_edges.get(&id).map_or(now, |e| e.0);
                        c.cooldown_edges.insert(id.clone(), (start, now, lockout));
                    }
                    let ability = c
                        .view
                        .abilities
                        .iter()
                        .find(|a| a["id"].as_str() == Some(&id))
                        .map(|a| a["ref"].clone());
                    c.emit(if lockout==0.{"cooldown.ended"}else if old==0.{"cooldown.started"}else{"cooldown.updated"},json!({"ability":ability,"ability_id":id,"display_key":key,"lockout":lockout}));
                });
            }
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Entity(usize),
    Player,
    Group,
    Ability(i64),
    Abilities,
    Effects,
    Legacy,
    Other,
}
pub(crate) fn kind(play: &Play, event: &net::session::Event) -> Kind {
    use net::session::Event::*;
    match event {
        State { channel: 0x42, .. } | LevelExp { .. } | PlayerP2C(_) | Pet(_) | Assist(_) => {
            Kind::Player
        }
        TargetInfo { .. } => play
            .entities
            .slots
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| e.id == play.target))
            .map_or(Kind::Other, Kind::Entity),
        State { channel: 0x43, .. } => Kind::Effects,
        State { channel: 0x40, .. }
        | GroupRoster { .. }
        | RaidRoster { .. }
        | GroupInvite { .. }
        | GroupInviteGone(_)
        | GroupRemoved(_)
        | GroupDisbanded => Kind::Group,
        State { channel, .. } => {
            net::entity::slot_of_channel(*channel).map_or(Kind::Other, Kind::Entity)
        }
        AbilityAdd { ability, .. } => Kind::Ability(ability.id),
        AbilitySlot { .. }
        | AbilityMoveSlot { .. }
        | AbilityMoveIndex { .. }
        | AbilityRemove { .. }
        | AbilityClearBar { .. }
        | AbilityUsable { .. } => Kind::Abilities,
        InvAdd(_)
        | InvTook { .. }
        | InvMove { .. }
        | InvUse { .. }
        | InvEquip { .. }
        | InvUnequip(_)
        | QuickSlot { .. }
        | GearSet { .. }
        | ItemHp { .. }
        | Opcode { .. } => Kind::Legacy,
        _ => Kind::Other,
    }
}

pub(crate) fn fingerprint(play: &Play, kind: Kind) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match kind {
        Kind::Entity(slot) => {
            if let Some(Some(e)) = play.entities.slots.get(slot) {
                e.id.hash(&mut h);
                e.world.hash(&mut h);
                e.pos.map(f32::to_bits).hash(&mut h);
                e.p19.hash(&mut h);
                if let Some(c) = &e.creature {
                    c.name.hash(&mut h);
                    c.level.hash(&mut h);
                    c.p256.hash(&mut h);
                    c.target.hash(&mut h);
                    c.flags262.hash(&mut h);
                    c.p263.hash(&mut h);
                }
                play.target_info.hash(&mut h);
                CORE.with(|c| c.borrow().disposition_target.hash(&mut h));
            }
        }
        Kind::Player => {
            play.dump.player.level.hash(&mut h);
            play.dump.player.exp.hash(&mut h);
            play.dump.player.p2c.hash(&mut h);
            play.stats.stat.hash(&mut h);
            play.stats.base.hash(&mut h);
            play.target.hash(&mut h);
            play.pet_id.hash(&mut h);
            CORE.with(|c| c.borrow().stats_received.hash(&mut h));
        }
        Kind::Group => {
            play.group.in_group.hash(&mut h);
            for m in &play.group.members {
                m.id.hash(&mut h);
                m.name.hash(&mut h);
                m.active.hash(&mut h);
                m.hp_known.hash(&mut h);
                m.hp.hash(&mut h);
                m.x.hash(&mut h);
                m.pos.map(f32::to_bits).hash(&mut h);
            }
            for m in &play.group.raid {
                m.id.hash(&mut h);
                m.name.hash(&mut h);
                m.hp.hash(&mut h);
                m.power.hash(&mut h);
            }
            for i in &play.group.invites {
                i.id.hash(&mut h);
                i.name.hash(&mut h);
            }
        }
        Kind::Effects => {
            play.effects.count.hash(&mut h);
            play.effects.icon.hash(&mut h);
            for n in &play.effects.name {
                n.hash(&mut h);
            }
            CORE.with(|c| c.borrow().effects_received.hash(&mut h));
        }
        Kind::Ability(id) => id.hash(&mut h),
        Kind::Abilities => {
            for a in play.abilities.values() {
                a.id.hash(&mut h);
                a.display_index.hash(&mut h);
                a.slot.hash(&mut h);
                a.valid.hash(&mut h);
                a.lockout.hash(&mut h);
            }
        }
        Kind::Other | Kind::Legacy => {}
    }
    h.finish()
}

struct Bar {
    index: usize,
    kind: &'static str,
    selected: bool,
    selected_slot: Option<i32>,
    slots: Vec<(crate::hudmenu::Entry, Option<i64>)>,
}
fn hotbars(p: &Play) -> Vec<Bar> {
    let bars = p.hud_bars();
    [0, 1, 4, 5, 2, 3]
        .into_iter()
        .filter(|k| match *k {
            0 | 2 => true,
            1 => bars.abilities >= 2,
            3 => bars.gear,
            4 => bars.abilities >= 3,
            5 => bars.abilities >= 4,
            _ => false,
        })
        .enumerate()
        .map(|(i, k)| {
            let d = &p.hud_menus.defs[k];
            let m = &p.hud_menus.menus[k];
            Bar {
                index: i + 1,
                kind: match k {
                    2 => "items",
                    3 => "weapon_sets",
                    _ => "abilities",
                },
                selected: p.hud_menus.sel == k as i32,
                selected_slot: (m.sel >= 0 && m.sel < d.count).then_some(m.sel + 1),
                slots: d
                    .entries
                    .iter()
                    .take(d.count.max(0) as usize)
                    .map(|e| {
                        (
                            *e,
                            (e.msg == 0x8060000)
                                .then(|| p.abilities.get(&e.id).map(|a| a.id))
                                .flatten(),
                        )
                    })
                    .collect(),
            }
        })
        .collect()
}
fn patch_hotbars(c: &mut Core, bars: Vec<Bar>) {
    let after=bars.into_iter().map(|b|json!({"index":b.index,"kind":b.kind,"selected":b.selected,"selected_slot":b.selected_slot,"slots":b.slots.into_iter().enumerate().map(|(i,(e,id))|{let kind=match e.msg{0=>"empty",0x8060000=>"ability",0x2030000=>"item",crate::gear::MSG=>"weapon_set",_=>"command"};let ability=id.and_then(|id|c.view.abilities.iter().find(|a|a["id"].as_str().is_some_and(|v|v.parse::<i64>().ok()==Some(id)))).map(|a|a["ref"].clone());json!({"index":i+1,"kind":kind,"ability":ability,"item":null,"weapon_set":(kind=="weapon_set").then_some(e.id+1),"command":null,"icon":null,"ui_icon":(e.icon2>=0).then_some(e.icon2)})}).collect::<Vec<_>>()})).collect::<Vec<_>>();
    let before = std::mem::replace(&mut c.view.hotbar, after);
    if interested("hotbar.changed") && before != c.view.hotbar {
        c.emit(
            "hotbar.changed",
            json!({"before":before,"after":c.view.hotbar,"changed":["records"]}),
        );
    }
}
