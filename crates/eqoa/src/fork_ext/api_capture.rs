//! Native state projection, as a child of Play's adapter (access stays private).
use super::Play;
use crate::fork_ext::addons::native_core::{CORE, View};
use serde_json::{Value as Json, json};

fn vec3(p: [f32; 3]) -> Json {
    json!({"x":p[0],"y":p[1],"z":p[2]})
}
fn stats(s: &[i32; 29]) -> Json {
    json!({"strength":s[0],"stamina":s[1],"agility":s[2],"dexterity":s[3],"wisdom":s[4],"intelligence":s[5],"charisma":s[6]})
}
fn valid_id(id: u32) -> Option<u32> {
    (!matches!(id, 0 | u32::MAX)).then_some(id)
}

#[cfg(test)]
thread_local! {static VIEW_BUILDS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};}
#[cfg(test)]
pub(super) fn take_view_builds() -> usize {
    VIEW_BUILDS.with(|n| n.replace(0))
}

impl Play {
    fn native_location(&self, walker: Option<&crate::walker::Walker>) -> Json {
        let Some(w) = walker else {
            return CORE.with(|c| c.borrow().view.player["location"].clone());
        };
        let Some(g) = w.scene.grid.as_ref() else {
            return Json::Null;
        };
        let cell = g.cell_size.0;
        if !ee::lt(0.0, cell) || (g.cols == 1 && g.rows == 1) {
            return Json::Null;
        }
        let p = w.cam.walk.committed.position;
        let coord = |p: f32| {
            let q = ee::div(p, cell);
            if ee::lt(p, 0.0) {
                q.ceil() as i32
            } else {
                q.floor() as i32
            }
        };
        let col = coord(p.x.0);
        let row = coord(p.z.0);
        let world = self.world_id();
        let bounds = self
            .start
            .as_ref()
            .and_then(|s| s.bounds.get(world as usize).copied().flatten());
        let inside = bounds.is_none_or(|b| {
            col - 1 >= b[0] && col - 1 <= b[1] && row - 1 >= b[2] && row - 1 <= b[3]
        });
        let place = if inside {
            crate::zones::zone(world, row - 1, col - 1).unwrap_or(641)
        } else {
            641
        };
        let local_x = ee::sub(ee::sub(p.x.0, ee::mul(col as f32, cell)), g.origin.x.0);
        let local_z = ee::sub(ee::sub(p.z.0, ee::mul(row as f32, cell)), g.origin.z.0);
        let third = |v: f32| {
            if ee::lt(v, -333.34) {
                0
            } else if ee::le(v, 333.34) {
                1
            } else {
                2
            }
        };
        let compass = [[1426, 1422, 1427], [1425, 1430, 1424], [1428, 1423, 1429]][third(local_z)]
            [third(local_x)];
        let name = self.words.as_ref().and_then(|w| {
            let place = w.get(place as i32, 0);
            let compass = w.get(compass, 0);
            (place != "(null)").then(|| {
                if compass == "(null)" || compass.is_empty() {
                    place.to_string()
                } else {
                    format!("{place} ({compass})")
                }
            })
        });
        json!({"world_id":world,"place_id":place,"compass_id":compass,"name":name,"cell_x":col,"cell_z":row})
    }

    pub(crate) fn in_world_now(&self) -> bool {
        self.session.self_id.is_some()
            && self.disconnected.is_none()
            && !self.quit
            && self.hud_ready()
            && !self.loading_up()
    }

    pub(crate) fn native_view(&self, walker: Option<&crate::walker::Walker>) -> View {
        #[cfg(test)]
        VIEW_BUILDS.with(|n| n.set(n.get() + 1));
        let connected = self.session.self_id.is_some() && self.disconnected.is_none() && !self.quit;
        let ready = self.in_world_now();
        let phase = if self.disconnected.is_some() || self.quit {
            "disconnected"
        } else if ready {
            "world"
        } else {
            "loading"
        };
        if !ready {
            return View {
                phase,
                ..View::default()
            };
        }
        let (stats_known, effects_known, previous) = CORE.with(|c| {
            let c = c.borrow();
            (c.stats_received, c.effects_received, c.view.player.clone())
        });
        let health = self.forge_health.snapshot(connected, ready);
        let stats_known = stats_known || health.available;
        let id = self.self_id32();
        let class = self.dump.player.class;
        let race = self.dump.player.race;
        let hp=stats_known.then(||json!({"current":if health.available{health.current_hp}else{self.stats.stat[7]},"maximum":if health.available{health.max_hp}else{self.stats.stat[8]},"base":self.stats.base[8]}));
        let power=stats_known.then(||json!({"current":self.stats.stat[9],"maximum":self.stats.stat[10],"base":self.stats.base[10]}));
        let (position, velocity, heading, speed, horizontal_speed) = if let Some(w) = walker {
            let p = w.cam.walk.phys.position;
            let v = w.cam.walk.phys.velocity;
            let v = [v.x.0, v.y.0, v.z.0];
            let xz = ee::add(ee::mul(v[0], v[0]), ee::mul(v[2], v[2]));
            (
                vec3([p.x.0, p.y.0, p.z.0]),
                vec3(v),
                json!(w.cam.walk.phys.angles.x.0.rem_euclid(std::f32::consts::TAU)),
                json!(ee::sqrt(ee::add(xz, ee::mul(v[1], v[1])))),
                json!(ee::sqrt(xz)),
            )
        } else {
            (
                previous["position"].clone(),
                previous["velocity"].clone(),
                previous["heading"].clone(),
                previous["speed"].clone(),
                previous["horizontal_speed"].clone(),
            )
        };
        let entities=self.entities.slots.iter().flatten().map(|e|{
            let c=e.creature.as_ref();let code=c.map(|c|c.p256);
            let disposition=(e.id==self.target && CORE.with(|c|c.borrow().disposition_target==Some(self.target))).then_some(self.target_info).flatten();
            json!({"id":e.id,"name":c.map_or("",|c|c.name.as_str()),"level":c.map(|c|c.level),"position":vec3(e.pos),"hp_fraction":(e.p19!=0).then(||ee::div(e.p19 as f32,255.0)),"kind":match code{Some(2)=>"player",Some(0|1)=>"npc",Some(3)=>"pet",_=>"other"},"class_code":code,"target_id":c.and_then(|c|valid_id(c.target)),"status_flags":c.map(|c|c.flags262),"poisoned":c.map(|c|c.flags262&4!=0),"diseased":c.map(|c|c.flags262&8!=0),"disposition":disposition,"disposition_name":disposition.and_then(|d|match d{0..=1=>Some("Hostile"),2..=3=>Some("Neutral"),4..=6=>Some("Friendly"),_=>None})})
        }).collect::<Vec<_>>();
        let fighting = id
            .and_then(|id| self.entities.slots.iter().flatten().find(|e| e.id == id))
            .and_then(|e| e.creature.as_ref())
            .map(|c| c.p263 != 0);
        let attributes = stats_known
            .then(|| json!({"base":stats(&self.stats.base),"total":stats(&self.stats.stat)}));
        let resists=stats_known.then(||json!({"base":40+self.stats.stat[4].div_euclid(7),"poison":self.stats.stat[22],"disease":self.stats.stat[23],"fire":self.stats.stat[24],"cold":self.stats.stat[25],"lightning":self.stats.stat[26],"arcane":self.stats.stat[27]}));
        let player = json!({"id":id,"name":self.player.name,"class_id":class,"class_name":usize::try_from(class).ok().and_then(|k|crate::fork_ext::addons::player::CLASSES.get(k)),"race_id":race,"race_name":usize::try_from(race).ok().and_then(|k|crate::fork_ext::addons::player::RACES.get(k)),"level":self.dump.player.level,"experience":self.dump.player.exp,"experience_debt":self.dump.player.p2c,"hp":hp,"power":power,"attributes":attributes,"armor":stats_known.then_some(self.stats.stat[14]),"resists":resists,"position":position,"velocity":velocity,"heading":heading,"speed":speed,"horizontal_speed":horizontal_speed,"target_id":valid_id(self.target),"pet_id":valid_id(self.pet_id),"auto_attacking":self.auto_attack,"dead":self.death.is_some(),"has_pet":valid_id(self.pet_id).is_some(),"fighting":fighting,"location":self.native_location(walker),"mastery":null});
        let members = if self.group.in_group {
            self.group.members.iter().map(|m|{
            let actor=self.entities.slots.iter().flatten().find(|e|e.id==m.id);
            json!({"id":m.id,"name":m.name,"active":m.active,"hp_fraction":(m.hp_known!=0).then(||ee::div(m.hp as f32,255.0)),"position":actor.map(|e|vec3(e.pos)).or_else(||m.active.then(||vec3(m.pos))),"world_id":actor.map(|e|e.world as i32).or_else(||m.active.then_some(m.x))})
        }).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let group = json!({"in_group":self.group.in_group,"self_leader":self.group.in_group.then(||self.group.members.first().is_some_and(|m|Some(m.id)==id)),"members":members,"raid":self.group.raid.iter().map(|m|json!({"id":m.id,"name":m.name,"hp_fraction":ee::div(m.hp as f32,255.0),"power_fraction":ee::div(m.power as f32,255.0)})).collect::<Vec<_>>(),"invites":self.group.invites.iter().map(|i|json!({"id":i.id,"name":i.name})).collect::<Vec<_>>()});
        let abilities=self.abilities.values().map(|a|json!({"id":a.id.to_string(),"name":a.name,"description":a.desc,"display_key":a.display_index,"spellbook_slot":(a.slot>=0).then(||a.slot+1),"usable":a.valid!=0,"level":a.level,"range":a.range,"cast_time":a.cast_time,"power_cost":a.power_cost,"icon_background":(a.icon_back>=0).then_some(a.icon_back),"icon_foreground":(a.icon_front>=0).then_some(a.icon_front),"scope":a.scope,"cooldown":a.cooldown,"equipment_mask":a.equip_req,"lockout":ee::div(a.lockout as f32,1000.0)})).collect();
        let bars = self.hud_bars();
        let hotbar=[0,1,4,5,2,3].into_iter().filter(|k|match *k {0|2=>true,1=>bars.abilities>=2,3=>bars.gear,4=>bars.abilities>=3,5=>bars.abilities>=4,_=>false}).enumerate().map(|(index,k)|{
            let def=&self.hud_menus.defs[k];let menu=&self.hud_menus.menus[k];
            let slots=def.entries.iter().take(def.count.max(0) as usize).enumerate().map(|(i,e)|{
                let kind=match e.msg {0=>"empty",0x8060000=>"ability",0x2030000=>"item",crate::gear::MSG=>"weapon_set",_=>"command"};
                json!({"index":i+1,"kind":kind,"ability_id":(kind=="ability").then(||self.abilities.values().find(|a|a.slot==e.id).map(|a|a.id.to_string())).flatten(),"item":null,"weapon_set":(kind=="weapon_set").then_some(e.id+1),"command":null,"icon":null,"ui_icon":(e.icon2>=0).then_some(e.icon2)})
            }).collect::<Vec<_>>();
            json!({"index":index+1,"kind":match k {2=>"items",3=>"weapon_sets",_=>"abilities"},"selected":self.hud_menus.sel==k as i32,"selected_slot":(menu.sel>=0 && menu.sel<def.count).then_some(menu.sel+1),"slots":slots})
        }).collect();
        let effects=effects_known.then(||(0..self.effects.count.clamp(0,8) as usize).map(|i|json!({"index":i+1,"icon":(self.effects.icon[i]!=u32::MAX).then_some(self.effects.icon[i]),"name":String::from_utf16_lossy(&self.effects.name[i])})).collect());
        View {
            in_world: true,
            phase,
            world_id: Some(self.world_id() as i32),
            player,
            entities,
            entity_slots: self
                .entities
                .slots
                .iter()
                .map(|e| e.as_ref().map(|e| e.id))
                .collect(),
            abilities,
            group,
            hotbar,
            effects,
            channel: match self.channel {
                0 => "say",
                2 => "shout",
                3 => "group",
                4 => "guild",
                _ => "other",
            },
        }
    }
}

pub(crate) struct ChangeCapture {
    pub cooldown_before: Option<(i64, f64)>,
    pub interested: bool,
    pub kind: updates::Kind,
    pub before: u64,
    pub previous_channel: &'static str,
}
impl Drop for ChangeCapture {
    fn drop(&mut self) {
        if self.interested {
            CORE.with(|c| c.borrow_mut().text_channel = self.previous_channel);
        }
    }
}
pub(crate) fn begin_game_event(play: &Play, event: &net::session::Event) -> ChangeCapture {
    use net::session::Event;
    let interested = crate::fork_ext::addons::native_core::capture_requested();
    if !interested {
        updates::inactive();
        CORE.with(|c| {
            let mut c = c.borrow_mut();
            match event {
                Event::State {
                    channel: 0x42,
                    data,
                } if data.len() >= 233 => c.stats_received = true,
                Event::State {
                    channel: 0x43,
                    data,
                } if data.len() >= 4 => c.effects_received = true,
                Event::WorldEntry { .. } | Event::Teleport { .. } | Event::Disconnected(_) => {
                    c.stats_received = false;
                    c.effects_received = false;
                    c.generation += 1;
                    c.entity_versions.clear();
                    c.ability_versions.clear();
                    c.cooldown_edges.clear();
                    c.entity_worlds.clear();
                    c.view.in_world = false;
                    c.words = None;
                    c.chat.clear();
                    c.selected_target = None;
                    c.disposition_target = None;
                }
                _ => {}
            }
        });
        return ChangeCapture {
            interested: false,
            kind: updates::Kind::Other,
            before: 0,
            cooldown_before: None,
            previous_channel: "system",
        };
    }
    let kind = updates::kind(play, event);
    let before = updates::fingerprint(play, kind);
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        match event {
            Event::TargetInfo { .. } => {
                c.selected_target = valid_id(play.target);
                c.disposition_target = c.selected_target;
            }
            Event::Teleport { .. } => c.reset("zone"),

            Event::WorldEntry { player, .. } => c.reset(if player.world == play.player.world {
                "respawn"
            } else {
                "zone"
            }),
            Event::Disconnected(_) => c.reset("disconnect"),
            Event::State {
                channel: 0x42,
                data,
            } if data.len() >= 233 => c.stats_received = true,
            Event::State {
                channel: 0x43,
                data,
            } if data.len() >= 4 => c.effects_received = true,
            _ => {}
        }
    });
    ChangeCapture {
        interested,
        kind,
        before,
        previous_channel: CORE.with(|c| {
            let mut c = c.borrow_mut();
            let old = c.text_channel;
            c.text_channel = match event {
                Event::GroupChat { .. } => "group",
                Event::Chat(..) => "unknown",
                _ => "system",
            };
            old
        }),
        cooldown_before: match event {
            Event::AbilityAdd { ability, .. } => Some((
                ability.id,
                play.abilities
                    .get(&ability.id)
                    .map_or(0.0, |a| ee::div(a.lockout as f32, 1000.0) as f64),
            )),
            Event::AbilityLockout { index, .. } => play
                .abilities
                .values()
                .find(|a| a.display_index == *index as i64)
                .map(|a| (a.id, ee::div(a.lockout as f32, 1000.0) as f64)),
            _ => None,
        },
    }
}
pub(crate) fn end_game_event(play: &Play, capture: ChangeCapture) {
    if !capture.interested {
        return;
    }
    if matches!(capture.kind, updates::Kind::Legacy) {
        updates::dirty();
    }
    if updates::fingerprint(play, capture.kind) != capture.before
        || matches!(capture.kind, updates::Kind::Ability(_))
        || capture.cooldown_before.is_some_and(|(id, old)| {
            play.abilities
                .get(&id)
                .is_some_and(|a| ee::div(a.lockout as f32, 1000.) as f64 != old)
        })
    {
        updates::capture(play, &capture.kind, capture.cooldown_before);
    }
}

#[path = "api_updates.rs"]
pub(super) mod updates;
