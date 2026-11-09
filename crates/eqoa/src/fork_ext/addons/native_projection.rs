//! Forge's bounded legacy records, projected from the native core.
use super::{host::Snapshot, native_core::View};
use serde_json::Value;
fn u(v: &Value) -> u32 {
    v.as_i64().unwrap_or(0).max(0) as u32
}
fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}
fn vector(v: &Value) -> [f32; 3] {
    [f(&v["x"]), f(&v["y"]), f(&v["z"])]
}
fn text<const N: usize>(v: &Value) -> ([u8; N], u8) {
    let text = v.as_str().unwrap_or("");
    let mut n = text.len().min(N);
    while !text.is_char_boundary(n) {
        n -= 1;
    }
    let mut bytes = [0; N];
    bytes[..n].copy_from_slice(&text.as_bytes()[..n]);
    (bytes, n as u8)
}
pub(crate) fn apply(s: &mut Snapshot, v: &View) {
    if !v.in_world {
        return;
    }
    let p = &v.player;
    (s.player.name, s.player.name_len) = text(&p["name"]);
    s.player.level = u(&p["level"]);
    s.player.exp = u(&p["experience"]);
    s.player.exp_debt = u(&p["experience_debt"]);
    s.player.entity_id = u(&p["id"]);
    s.player.target_entity_id = u(&p["target_id"]);
    s.player.pet_entity_id = p["pet_id"].as_u64().map(|n| n as u32);
    s.player.class_id = u(&p["class_id"]);
    s.player.race_id = u(&p["race_id"]);
    s.player.position = vector(&p["position"]);
    s.player.velocity = vector(&p["velocity"]);
    s.player.heading_rad = f(&p["heading"]);
    s.player.current_hp = u(&p["hp"]["current"]);
    s.player.max_hp = u(&p["hp"]["maximum"]);
    s.player.base_hp = u(&p["hp"]["base"]);
    s.player.current_pwr = u(&p["power"]["current"]);
    s.player.max_pwr = u(&p["power"]["maximum"]);
    s.player.base_pwr = u(&p["power"]["base"]);
    s.player.ac = u(&p["armor"]);
    s.player.is_auto_attacking = p["auto_attacking"].as_bool().unwrap_or(false);
    s.player.is_dead = p["dead"].as_bool().unwrap_or(false);
    s.player.available = true;
    let total = &p["attributes"]["total"];
    let base = &p["attributes"]["base"];
    s.player.total_str = u(&total["strength"]);
    s.player.total_sta = u(&total["stamina"]);
    s.player.total_agi = u(&total["agility"]);
    s.player.total_dex = u(&total["dexterity"]);
    s.player.total_wis = u(&total["wisdom"]);
    s.player.total_int = u(&total["intelligence"]);
    s.player.total_cha = u(&total["charisma"]);
    s.player.base_str = u(&base["strength"]);
    s.player.base_sta = u(&base["stamina"]);
    s.player.base_agi = u(&base["agility"]);
    s.player.base_dex = u(&base["dexterity"]);
    s.player.base_wis = u(&base["wisdom"]);
    s.player.base_int = u(&base["intelligence"]);
    s.player.base_cha = u(&base["charisma"]);
    s.player.poison_resist = u(&p["resists"]["poison"]);
    s.player.disease_resist = u(&p["resists"]["disease"]);
    s.player.fire_resist = u(&p["resists"]["fire"]);
    s.player.cold_resist = u(&p["resists"]["cold"]);
    s.player.lightning_resist = u(&p["resists"]["lightning"]);
    s.player.arcane_resist = u(&p["resists"]["arcane"]);
    s.group = super::group::GroupSnapshot::default();
    s.group.in_group = v.group["in_group"].as_bool().unwrap_or(false);
    s.group.is_leader = v.group["self_leader"].as_bool().unwrap_or(false);
    if let Some(members) = v.group["members"].as_array() {
        s.group.member_count = members.len().min(super::group::MAX_GROUP_MEMBERS) as u8;
        for (i, m) in members.iter().take(s.group.members.len()).enumerate() {
            let member = &mut s.group.members[i];
            member.entity_id = u(&m["id"]);
            (member.name, member.name_len) = text(&m["name"]);
            member.active = m["active"].as_bool().unwrap_or(false);
            member.hp_known = m["hp_fraction"].is_number();
            member.percent_hp_255 = ee::mul(f(&m["hp_fraction"]), 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            member.position = vector(&m["position"]);
        }
    }
    // Preserve retail slot indexes and Forge's synthesized self record; native list is unbounded.
    s.entities = super::entity::EntitiesSnapshot::default();
    let put = |s: &mut super::entity::EntitySnapshot, e: &Value| {
        s.id = u(&e["id"]);
        s.level = u(&e["level"]) as u8;
        s.health_percent_255 = ee::mul(f(&e["hp_fraction"]), 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        s.position = vector(&e["position"]);
        (s.name, s.name_len) = text(&e["name"]);
        s.target_id = u(&e["target_id"]) & 0xff;
        s.class_byte = u(&e["class_code"]) as u8;
        s.status_flags = u(&e["status_flags"]) as u8;
        s.disposition = e["disposition"].as_u64().map(|d| d as u8);
        s.is_populated = s.id != 0 && s.level > 0;
    };
    for i in 0..24 {
        if let Some(Some(id)) = v.entity_slots.get(i) {
            if let Some(e) = v
                .entities
                .iter()
                .find(|e| e["id"].as_u64() == Some(*id as u64))
            {
                put(&mut s.entities.entities[i], e);
            }
        }
    }
    if !s.entities.entities[0].is_populated {
        let me = s.player.entity_id;
        if let Some(e) = v
            .entities
            .iter()
            .find(|e| e["id"].as_u64() == Some(me as u64))
        {
            put(&mut s.entities.entities[0], e);
        } else {
            let e = &mut s.entities.entities[0];
            e.id = me;
            e.name = s.player.name;
            e.name_len = s.player.name_len;
            e.level = s.player.level as u8;
            e.position = s.player.position;
            e.class_byte = 2;
            e.is_populated = me != 0 && e.level > 0;
        }
    }
    let mut next = 24;
    for e in &v.entities {
        let id = u(&e["id"]);
        if next == s.entities.entities.len() {
            break;
        }
        if s.entities.entities.iter().any(|r| r.id == id) {
            continue;
        }
        if id == s.player.target_entity_id
            || Some(id) == s.player.pet_entity_id
            || v.group["members"]
                .as_array()
                .is_some_and(|m| m.iter().any(|m| u(&m["id"]) == id))
        {
            put(&mut s.entities.entities[next], e);
            next += 1;
        }
    }
    s.entities.count = next as u8;
    s.abilities.ability_count = v.abilities.len().min(s.abilities.abilities.len()) as u16;
    for (i, a) in v
        .abilities
        .iter()
        .take(s.abilities.abilities.len())
        .enumerate()
    {
        let out = &mut s.abilities.abilities[i];
        out.id = a["id"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0) as u32;
        out.display_index = u(&a["display_key"]);
        out.spellbook_slot = a["spellbook_slot"]
            .as_u64()
            .map_or(u32::MAX, |n| n.saturating_sub(1) as u32);
        out.valid = a["usable"].as_bool().unwrap_or(false);
        out.level = u(&a["level"]);
        out.range = f(&a["range"]);
        out.cast_time = u(&a["cast_time"]);
        out.power_cost = u(&a["power_cost"]);
        out.icon_background_ref = a["icon_background"].as_u64().map_or(u32::MAX, |n| n as u32);
        out.icon_foreground_ref = a["icon_foreground"].as_u64().map_or(u32::MAX, |n| n as u32);
        out.scope = u(&a["scope"]);
        out.cooldown_seconds = u(&a["cooldown"]);
        out.cooldown_lockout_ms = ee::mul(f(&a["lockout"]), 1000.0).round() as u32;
        out.equip_requirements = u(&a["equipment_mask"]);
        (out.name, out.name_len) = text(&a["name"]);
        (out.description, out.desc_len) = text(&a["description"]);
    }
}
