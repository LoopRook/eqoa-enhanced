// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! The combat indication adapter; service code only receives plain combat data.
use super::Play;

#[cfg(not(target_arch = "wasm32"))]
#[path = "api_capture.rs"]
mod api_capture;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "api_capture_tests.rs"]
mod native_tests;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use api_capture::{begin_game_event, end_game_event};
#[cfg(target_arch = "wasm32")]
pub(crate) fn begin_game_event(_: &Play, _: &net::session::Event) {}
#[cfg(target_arch = "wasm32")]
pub(crate) fn end_game_event(_: &Play, _: ()) {}
pub(crate) fn begin_iteration(_play: &mut Play, _game_step: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::fork_ext::addons::native_core::iteration(_game_step);
        if _game_step {
            let ready = _play.in_world_now();
            weapon_sets_requests(_play, ready);
            let reqs = crate::fork_ext::addons::actions::take_action_requests();
            if ready && _play.mode == 0 {
                if reqs.assist {
                    _play.assist_target();
                }
                if reqs.summary {
                    _play.trainer_open = true;
                    _play.trainer_from_hud = true;
                    _play.mode = 12;
                }
            }
            if !ready {
                crate::fork_ext::addons::chat::cancel_outgoing_chat(
                    "world became unavailable before queued chat could be sent",
                );
            } else if _play.mode != 60 {
                for msg in crate::fork_ext::addons::chat::take_outgoing_chat() {
                    if !crate::fork_ext::addons::chat::report_route(&msg) {
                        _play.chat_line(&msg);
                    }
                }
            }
        }
    }
}
pub(crate) fn observe_text(_text: &str, _color: [f32; 4]) {
    #[cfg(not(target_arch = "wasm32"))]
    crate::fork_ext::addons::native_core::observe_text(_text, _color);
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "report_state.rs"]
mod report_state;

/// Capture every received 0x00CD before addon frames can coalesce its transitions.
pub(crate) fn ability_lockout(_index: u8, _lockout: i32) {
    #[cfg(not(target_arch = "wasm32"))]
    crate::fork_ext::addons::ability::record_lockout(_index as u32, _lockout.max(0) as u32);
}

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(not(target_arch = "wasm32"))]
static ATLAS_REBUILD_COUNT: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn atlas_rebuild_count() -> u64 {
    ATLAS_REBUILD_COUNT.load(Ordering::Relaxed)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AtlasKey {
    None,
    Hud(usize),
    UiData(usize),
}

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static CACHED_ATLAS_KEY: std::cell::Cell<AtlasKey> = const { std::cell::Cell::new(AtlasKey::None) };
    static CACHED_ATLAS: std::cell::RefCell<Option<std::sync::Arc<crate::fork_ext::addons::icons::SafeAtlas>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn frame_library(
    _play: &Play,
    _lib: &esf::Library,
    _walker: Option<&crate::walker::Walker>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::report::note_state(|| report_state::describe(_play, _walker));
        crate::fork_ext::addons::native_core::CORE.with(|c| {
            let mut c = c.borrow_mut();
            if crate::fork_ext::addons::native_core::capture_requested() && c.words.is_none() {
                c.words = _play.words.clone();
            }
        });
        let current_key = if let Some((vui, _, _)) = &_play.hud {
            AtlasKey::Hud(&vui.textures as *const _ as usize)
        } else if let Some((_, textures)) = &_play.ui_data {
            AtlasKey::UiData(textures as *const _ as usize)
        } else {
            AtlasKey::None
        };

        let prev_key = CACHED_ATLAS_KEY.with(|k| k.get());
        let atlas = if current_key != prev_key {
            CACHED_ATLAS_KEY.with(|k| k.set(current_key));
            let built = match current_key {
                AtlasKey::Hud(_) => {
                    let (vui, _, _) = _play.hud.as_ref().unwrap();
                    let mut entries = Vec::with_capacity(252);
                    for i in 0..252 {
                        entries.push(vui.entry(i));
                    }
                    Some(std::sync::Arc::new(
                        crate::fork_ext::addons::icons::SafeAtlas {
                            entries,
                            textures: vui.textures.clone(),
                        },
                    ))
                }
                AtlasKey::UiData(_) => {
                    let (entries, textures) = _play.ui_data.as_ref().unwrap();
                    Some(std::sync::Arc::new(
                        crate::fork_ext::addons::icons::SafeAtlas {
                            entries: entries.clone(),
                            textures: textures.clone(),
                        },
                    ))
                }
                AtlasKey::None => None,
            };
            ATLAS_REBUILD_COUNT.fetch_add(1, Ordering::Relaxed);
            CACHED_ATLAS.with(|ca| *ca.borrow_mut() = built.clone());
            built
        } else {
            None
        };

        let prefetch_ids = crate::fork_ext::addons::icons::get_cached_or_collect_prefetch_ids(
            &_play.abilities,
            &_play.hud_menus,
            &_play.effects,
            &_play.dump.inventory.items,
        );
        crate::fork_ext::addons::icons::update_live_data(_lib, atlas, &prefetch_ids);
    }
}

pub(crate) fn art_changed() {
    #[cfg(not(target_arch = "wasm32"))]
    crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow_mut().words = None);
    #[cfg(not(target_arch = "wasm32"))]
    crate::fork_ext::addons::icons::notify_art_changed();
}

#[cfg(not(target_arch = "wasm32"))]
impl Play {
    // Same dispatch target as hud_key's gear::MSG arm. Navigation remains native.
    fn forge_use_weapon_set(&mut self, set: i64) -> bool {
        if self.session.self_id.is_none()
            || self.disconnected.is_some()
            || self.quit
            || !matches!(self.mode, 0 | 38)
            || !self.gear.on
            || !(0..crate::gear::ENTRIES as i64).contains(&set)
            || (set != crate::gear::BARE_HANDS && self.gear.empty(set))
        {
            return false;
        }
        self.hud_menus.menus[3].sel = set as i32;
        self.use_gear_set(set);
        true
    }

    pub(crate) fn forge_snapshot(
        &self,
        walker: Option<&crate::walker::Walker>,
    ) -> crate::fork_ext::addons::host::Snapshot {
        if !crate::fork_ext::addons::native_core::capture_requested() {
            api_capture::updates::inactive();
            let connected =
                self.session.self_id.is_some() && self.disconnected.is_none() && !self.quit;
            let ready = self.in_world_now();
            let start_menu = ready
                && (is_menu_mode(self.mode)
                    || self.map.open
                    || self.start.as_ref().is_some_and(|s| s.menu.visible));
            let ui_mode = if !ready {
                0
            } else if self.mode != 0 {
                self.mode as u32
            } else if self.start.as_ref().is_some_and(|s| s.menu.visible) {
                13
            } else {
                0
            };
            let ui_busy = ready
                && (ui_mode != 0
                    || start_menu
                    || self.map.open
                    || self.map.search.open
                    || self.search.open
                    || self.typing.is_some());
            return crate::fork_ext::addons::host::Snapshot {
                capture_pending: true,
                character: crate::fork_ext::addons::named_profiles::CharacterSnapshot::new(
                    connected,
                    &self.player.name,
                    self.dump.player.class,
                ),
                in_game: ready,
                start_menu,
                ui_mode,
                ui_busy,
                health: self.forge_health.snapshot(connected, ready),
                ..Default::default()
            };
        }
        api_capture::updates::refresh(self, walker);
        let key = api_capture::updates::key(self, walker);
        if let Some(mut snapshot) = api_capture::updates::cached(key) {
            snapshot.input = crate::fork_ext::addons::input::live_snapshot();
            return snapshot;
        }
        let connected = self.session.self_id.is_some() && self.disconnected.is_none() && !self.quit;
        let hud_ready = self.hud_ready() && !self.loading_up();
        let in_game = connected && hud_ready;

        let player = if in_game {
            let mut name_buf = [0u8; 24];
            let name_bytes = self.player.name.as_bytes();
            let copy_len = name_bytes.len().min(24);
            name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

            let (position, velocity, heading_rad) = if let Some(w) = walker {
                let pos = w.cam.walk.phys.position;
                let vel = w.cam.walk.phys.velocity;
                let h_rad = w.cam.walk.phys.angles.x.0;
                (
                    [pos.x.0, pos.y.0, pos.z.0],
                    [vel.x.0, vel.y.0, vel.z.0],
                    h_rad,
                )
            } else if let Some((at, _)) = self.self_at {
                (at, [0.0, 0.0, 0.0], 0.0)
            } else {
                ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.0)
            };

            let target_entity_id = if self.target == u32::MAX {
                0
            } else {
                self.target
            };
            let pet_entity_id = if self.pet_id == u32::MAX {
                None
            } else {
                Some(self.pet_id)
            };

            let cur_pwr = self.stats.stat.get(9).copied().unwrap_or(0).max(0) as u32;
            let max_pwr = self.stats.stat.get(10).copied().unwrap_or(0).max(0) as u32;
            let base_pwr = self.stats.base.get(10).copied().unwrap_or(0).max(0) as u32;

            let class_id = self.dump.player.class.clamp(0, 14) as u32;
            let race_id = self.dump.player.race.clamp(0, 9) as u32;

            let location = if let Some(w) = walker {
                let g = w.scene.grid.as_ref();
                let p = w.cam.walk.committed.position;
                let world = self.world_id();
                let cols = g.map_or(1, |g| g.cols);
                let rows = g.map_or(1, |g| g.rows);
                let cell = g.map_or(1.0, |g| g.cell_size.0);
                let origin = g.map_or([0.0; 3], |g| [g.origin.x.0, g.origin.y.0, g.origin.z.0]);
                let bounds = self
                    .start
                    .as_ref()
                    .and_then(|s| s.bounds.get(world as usize).copied().flatten());

                if cols == 1 && rows == 1 {
                    let mut loc =
                        crate::fork_ext::addons::player::PlayerLocationSnapshot::default();
                    loc.place[..3].copy_from_slice(b"***");
                    loc.place_len = 3;
                    loc.name[..3].copy_from_slice(b"***");
                    loc.name_len = 3;
                    loc.valid = true;
                    loc
                } else if cell > 0.0 {
                    let col = if p.x.0 < 0.0 {
                        (p.x.0 / cell).ceil() as i32
                    } else {
                        (p.x.0 / cell).floor() as i32
                    };
                    let row = if p.z.0 < 0.0 {
                        (p.z.0 / cell).ceil() as i32
                    } else {
                        (p.z.0 / cell).floor() as i32
                    };
                    let in_cell_x = p.x.0 - col as f32 * cell - origin[0];
                    let in_cell_z = p.z.0 - row as f32 * cell - origin[2];
                    let r_idx = row - 1;
                    let c_idx = col - 1;

                    let zone_id = if let Some(b) = bounds {
                        if c_idx < b[0] || b[1] < c_idx || r_idx < b[2] || b[3] < r_idx {
                            641
                        } else {
                            crate::zones::zone(world, r_idx, c_idx).unwrap_or(641)
                        }
                    } else {
                        641
                    };

                    let place_str = self
                        .words
                        .as_ref()
                        .map(|w| w.get(zone_id as i32, 0))
                        .unwrap_or("");
                    const DIRS: [[i32; 3]; 3] =
                        [[1426, 1422, 1427], [1425, 1430, 1424], [1428, 1423, 1429]];
                    let third = |v: f32| {
                        if v < -333.34 {
                            0
                        } else if v <= 333.34 {
                            1
                        } else {
                            2
                        }
                    };
                    let dir_id = DIRS[third(in_cell_z)][third(in_cell_x)];
                    let compass_str = self.words.as_ref().map(|w| w.get(dir_id, 0)).unwrap_or("");
                    let name_str = if compass_str.is_empty() {
                        place_str.to_string()
                    } else {
                        format!("{place_str} ({compass_str})")
                    };

                    let mut loc =
                        crate::fork_ext::addons::player::PlayerLocationSnapshot::default();
                    let p_bytes = place_str.as_bytes();
                    let p_len = p_bytes.len().min(64);
                    loc.place[..p_len].copy_from_slice(&p_bytes[..p_len]);
                    loc.place_len = p_len as u8;

                    let c_bytes = compass_str.as_bytes();
                    let c_len = c_bytes.len().min(16);
                    loc.compass[..c_len].copy_from_slice(&c_bytes[..c_len]);
                    loc.compass_len = c_len as u8;

                    let n_bytes = name_str.as_bytes();
                    let n_len = n_bytes.len().min(80);
                    loc.name[..n_len].copy_from_slice(&n_bytes[..n_len]);
                    loc.name_len = n_len as u8;

                    loc.cell_x = col;
                    loc.cell_z = row;
                    loc.valid = true;
                    loc
                } else {
                    crate::fork_ext::addons::player::PlayerLocationSnapshot::default()
                }
            } else {
                crate::fork_ext::addons::player::PlayerLocationSnapshot::default()
            };

            crate::fork_ext::addons::player::PlayerSnapshot {
                name: name_buf,
                name_len: copy_len as u8,
                level: self.dump.player.level.max(0) as u32,
                exp: self.dump.player.exp.max(0) as u32,
                exp_debt: self.dump.player.p2c.max(0) as u32,
                current_hp: self
                    .forge_health
                    .snapshot(connected, hud_ready)
                    .current_hp
                    .max(0) as u32,
                max_hp: self
                    .forge_health
                    .snapshot(connected, hud_ready)
                    .max_hp
                    .max(0) as u32,
                base_hp: self.stats.base.get(8).copied().unwrap_or(0).max(0) as u32,
                current_pwr: cur_pwr,
                max_pwr: max_pwr,
                base_pwr: base_pwr,
                total_str: self.stats.stat.get(0).copied().unwrap_or(0).max(0) as u32,
                total_sta: self.stats.stat.get(1).copied().unwrap_or(0).max(0) as u32,
                total_agi: self.stats.stat.get(2).copied().unwrap_or(0).max(0) as u32,
                total_dex: self.stats.stat.get(3).copied().unwrap_or(0).max(0) as u32,
                total_wis: self.stats.stat.get(4).copied().unwrap_or(0).max(0) as u32,
                total_int: self.stats.stat.get(5).copied().unwrap_or(0).max(0) as u32,
                total_cha: self.stats.stat.get(6).copied().unwrap_or(0).max(0) as u32,
                base_str: self.stats.base.get(0).copied().unwrap_or(0).max(0) as u32,
                base_sta: self.stats.base.get(1).copied().unwrap_or(0).max(0) as u32,
                base_agi: self.stats.base.get(2).copied().unwrap_or(0).max(0) as u32,
                base_dex: self.stats.base.get(3).copied().unwrap_or(0).max(0) as u32,
                base_wis: self.stats.base.get(4).copied().unwrap_or(0).max(0) as u32,
                base_int: self.stats.base.get(5).copied().unwrap_or(0).max(0) as u32,
                base_cha: self.stats.base.get(6).copied().unwrap_or(0).max(0) as u32,
                ac: self.stats.stat.get(14).copied().unwrap_or(0).max(0) as u32,
                poison_resist: self.stats.stat.get(22).copied().unwrap_or(0).max(0) as u32,
                disease_resist: self.stats.stat.get(23).copied().unwrap_or(0).max(0) as u32,
                fire_resist: self.stats.stat.get(24).copied().unwrap_or(0).max(0) as u32,
                cold_resist: self.stats.stat.get(25).copied().unwrap_or(0).max(0) as u32,
                lightning_resist: self.stats.stat.get(26).copied().unwrap_or(0).max(0) as u32,
                arcane_resist: self.stats.stat.get(27).copied().unwrap_or(0).max(0) as u32,
                cms: 0,
                cms_spent: 0,
                cm_pct: 0,
                position,
                velocity,
                heading_rad,
                entity_id: self.self_id32().unwrap_or(0),
                target_entity_id,
                pet_entity_id,
                is_auto_attacking: self.auto_attack,
                is_dead: self.death.is_some(),
                class_id,
                race_id,
                location,
                available: true,
            }
        } else {
            crate::fork_ext::addons::player::PlayerSnapshot::default()
        };

        let group = if in_game && self.group.in_group {
            let mut members = [crate::fork_ext::addons::group::GroupMemberSnapshot::default();
                crate::fork_ext::addons::group::MAX_GROUP_MEMBERS];
            let count = self
                .group
                .members
                .len()
                .min(crate::fork_ext::addons::group::MAX_GROUP_MEMBERS);
            for (i, m) in self.group.members.iter().take(count).enumerate() {
                let mut name_buf = [0u8; 24];
                let bytes = m.name.as_bytes();
                let copy_len = bytes.len().min(24);
                name_buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                members[i] = crate::fork_ext::addons::group::GroupMemberSnapshot {
                    entity_id: m.id,
                    name: name_buf,
                    name_len: copy_len as u8,
                    active: m.active,
                    hp_known: m.hp_known != 0,
                    percent_hp_255: m.hp,
                    position: m.pos,
                };
            }
            crate::fork_ext::addons::group::GroupSnapshot {
                in_group: true,
                is_leader: self
                    .group
                    .members
                    .first()
                    .is_some_and(|m| Some(m.id) == self.self_id32()),
                member_count: count as u8,
                members,
            }
        } else {
            crate::fork_ext::addons::group::GroupSnapshot::default()
        };

        let entities = if in_game {
            let mut snap_entities = [crate::fork_ext::addons::entity::EntitySnapshot::default();
                crate::fork_ext::addons::entity::MAX_ENTITIES];
            let record_of = |e: &net::entity::Entity| {
                let mut name_buf = [0u8; 24];
                let mut name_len = 0u8;
                let mut level = 0u8;
                let mut target_id = 0u32;
                let mut class_byte = 0u8;
                let mut status_flags = 0u8;
                if let Some(c) = &e.creature {
                    let bytes = c.name.as_bytes();
                    let copy_len = bytes.len().min(24);
                    name_buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    name_len = copy_len as u8;
                    level = c.level;
                    target_id = c.target;
                    class_byte = c.p256;
                    status_flags = c.flags262;
                }
                let is_target = self.target != u32::MAX && self.target != 0 && e.id == self.target;
                let disposition = if is_target { self.target_info } else { None };
                crate::fork_ext::addons::entity::EntitySnapshot {
                    id: e.id,
                    level,
                    health_percent_255: e.p19,
                    position: e.pos,
                    name: name_buf,
                    name_len,
                    target_id,
                    class_byte,
                    status_flags,
                    disposition,
                    is_populated: e.id != 0 && level > 0,
                }
            };
            for i in 0..crate::fork_ext::addons::entity::RETAIL_RECORDS {
                if let Some(Some(e)) = self.entities.slots.get(i) {
                    snap_entities[i] = record_of(e);
                }
            }
            // Slot 0 invariant: 0th index is always the player in FrontiersForge.
            // If slot 0 is not populated or has id 0, synthesize slot 0 from player state:
            if !snap_entities[0].is_populated {
                let player_id = self.self_id32().unwrap_or(0);
                let hp_pct_255 = if player.max_hp > 0 {
                    ((player.current_hp as u64 * 255) / player.max_hp as u64).min(255) as u8
                } else {
                    0
                };
                let is_target =
                    self.target != u32::MAX && self.target != 0 && player_id == self.target;
                let disposition = if is_target { self.target_info } else { None };
                snap_entities[0] = crate::fork_ext::addons::entity::EntitySnapshot {
                    id: player_id,
                    level: player.level.min(255) as u8,
                    health_percent_255: hp_pct_255,
                    position: player.position,
                    name: player.name,
                    name_len: player.name_len,
                    target_id: player.target_entity_id,
                    class_byte: 2, // 2 is player character
                    status_flags: 0,
                    disposition,
                    is_populated: player_id != 0 && player.level > 0,
                };
            }
            // A server keeping more than the game's 24 slots (NPC/PC Display)
            // can put whom the HUD follows past them, where the game's list
            // never reaches: the records after the 24 hold those, the target
            // first, so a mod's target box, pet or group frame finds them by id.
            let mut next = crate::fork_ext::addons::entity::RETAIL_RECORDS;
            let group = self.group.members[..self.group.count()]
                .iter()
                .map(|m| m.id);
            let tot_id = self
                .entities
                .slots
                .iter()
                .flatten()
                .find(|e| e.id == self.target)
                .and_then(|e| e.creature.as_ref())
                .map(|c| c.target)
                .filter(|&t| t != 0 && t != u32::MAX && t != self.target);
            let followed = [Some(self.target), Some(self.pet_id), self.self_id32()]
                .into_iter()
                .flatten()
                .chain(group)
                .chain(tot_id);
            for id in followed {
                if next == snap_entities.len() {
                    break;
                }
                if id == 0 || id == u32::MAX || snap_entities[..next].iter().any(|r| r.id == id) {
                    continue;
                }
                if let Some(e) = self.entities.slots.iter().flatten().find(|e| e.id == id) {
                    snap_entities[next] = record_of(e);
                    next += 1;
                }
            }
            crate::fork_ext::addons::entity::EntitiesSnapshot {
                count: 24,
                entities: snap_entities,
            }
        } else {
            crate::fork_ext::addons::entity::EntitiesSnapshot::default()
        };

        let abilities = if in_game {
            let mut snap_abilities = [crate::fork_ext::addons::ability::AbilitySnapshot::default();
                crate::fork_ext::addons::ability::MAX_ABILITIES];
            let mut ability_count = 0usize;
            for a in self.abilities.values() {
                if ability_count >= crate::fork_ext::addons::ability::MAX_ABILITIES {
                    break;
                }
                let mut name_buf = [0u8; 32];
                let n_bytes = a.name.as_bytes();
                let n_len = n_bytes.len().min(32);
                name_buf[..n_len].copy_from_slice(&n_bytes[..n_len]);

                let mut desc_buf = [0u8; 64];
                let d_bytes = a.desc.as_bytes();
                let d_len = d_bytes.len().min(64);
                desc_buf[..d_len].copy_from_slice(&d_bytes[..d_len]);

                let bg_ref = if a.icon_back != 0 && a.icon_back != -1 {
                    (a.icon_back as i32) as u32
                } else {
                    0
                };
                let fg_ref = if a.icon_front != 0 && a.icon_front != -1 {
                    (a.icon_front as i32) as u32
                } else {
                    0
                };

                snap_abilities[ability_count] = crate::fork_ext::addons::ability::AbilitySnapshot {
                    id: a.id as u32,
                    display_index: a.display_index.max(0) as u32,
                    spellbook_slot: a.slot.max(0) as u32,
                    level: a.level.max(0) as u32,
                    range: a.range,
                    cast_time: a.cast_time.max(0) as u32,
                    power_cost: a.power_cost.max(0) as u32,
                    icon_background_ref: bg_ref,
                    icon_foreground_ref: fg_ref,
                    scope: a.scope.max(0) as u32,
                    cooldown_seconds: a.cooldown.max(0) as u32,
                    cooldown_lockout_ms: a.lockout.max(0) as u32,
                    equip_requirements: a.equip_req.max(0) as u32,
                    name: name_buf,
                    name_len: n_len as u8,
                    description: desc_buf,
                    desc_len: d_len as u8,
                    valid: a.valid != 0,
                };
                ability_count += 1;
            }

            let mut snap_items = [crate::fork_ext::addons::ability::ItemSnapshot::default();
                crate::fork_ext::addons::ability::MAX_ITEM_RECORDS];
            for (k, it) in self
                .dump
                .inventory
                .items
                .iter()
                .enumerate()
                .take(crate::fork_ext::addons::ability::MAX_ITEM_RECORDS)
            {
                let mut name_buf = [0u8; 32];
                let n_bytes = it.item.name.as_bytes();
                let n_len = n_bytes.len().min(32);
                name_buf[..n_len].copy_from_slice(&n_bytes[..n_len]);

                let mut desc_buf = [0u8; 64];
                let d_bytes = it.item.desc.as_bytes();
                let d_len = d_bytes.len().min(64);
                desc_buf[..d_len].copy_from_slice(&d_bytes[..d_len]);

                let icon_raw = crate::inventory::item_icon(it, net::session::EXPANSION_FLAGS);
                let icon_ref = if icon_raw != 0 && icon_raw != -1 {
                    (icon_raw as i32) as u32
                } else {
                    0
                };

                snap_items[k] = crate::fork_ext::addons::ability::ItemSnapshot {
                    name: name_buf,
                    name_len: n_len as u8,
                    description: desc_buf,
                    desc_len: d_len as u8,
                    icon_ref,
                    slot_id: (k + 1) as u32,
                    populated: true,
                };
            }

            let mut bars = [crate::fork_ext::addons::ability::AbilityBarSnapshot::default();
                crate::fork_ext::addons::ability::MAX_BARS];
            let selected_bar = (self.hud_menus.sel.clamp(0, 2)) as u8;

            for b in 0..3 {
                let def = &self.hud_menus.defs[b];
                let slot_count = (def.count.clamp(0, 5)) as u8;
                let selected_slot = (self.hud_menus.menus[b].sel.clamp(0, 4)) as u8;
                let mut slots =
                    [crate::fork_ext::addons::ability::AbilityBarSlotSnapshot::default();
                        crate::fork_ext::addons::ability::MAX_BAR_SLOTS];

                for (s_idx, entry) in def.entries.iter().take(5).enumerate() {
                    let (source_type, source_index, ability_icon_ref) = if entry.msg == 0x8060000 {
                        let raw_idx = snap_abilities[..ability_count]
                            .iter()
                            .position(|ab| ab.id == entry.id as u32)
                            .map(|idx| (idx + 1) as u32)
                            .unwrap_or(0);
                        let icon_ref = if raw_idx > 0 && (raw_idx as usize) <= ability_count {
                            snap_abilities[raw_idx as usize - 1].icon_foreground_ref as i32
                        } else {
                            -1
                        };
                        (0x8060000, raw_idx, icon_ref)
                    } else if entry.msg == 0x2030000 {
                        let item_idx = entry.id.max(0) as usize;
                        let icon_ref =
                            if item_idx < snap_items.len() && snap_items[item_idx].populated {
                                snap_items[item_idx].icon_ref as i32
                            } else {
                                -1
                            };
                        (0x2030000, item_idx as u32, icon_ref)
                    } else {
                        (0, 0, -1)
                    };
                    slots[s_idx] = crate::fork_ext::addons::ability::AbilityBarSlotSnapshot {
                        source_type,
                        source_index,
                        ability_icon_ref,
                    };
                }

                bars[b] = crate::fork_ext::addons::ability::AbilityBarSnapshot {
                    slot_count,
                    selected_slot,
                    slots,
                };
            }

            // Diagnostic slot change telemetry (throttled: emit once when slot layout changes)
            static PREV_SLOTS: std::sync::Mutex<Option<[[(u32, u32, [u8; 32], u8); 5]; 3]>> =
                std::sync::Mutex::new(None);
            let mut current_slots = [[(0u32, 0u32, [0u8; 32], 0u8); 5]; 3];
            for b in 0..3 {
                for s in 0..5 {
                    let slot = &bars[b].slots[s];
                    if slot.source_type == 0x8060000
                        && slot.source_index >= 1
                        && (slot.source_index as usize) <= ability_count
                    {
                        let ab = &snap_abilities[slot.source_index as usize - 1];
                        current_slots[b][s] = (
                            ab.icon_background_ref,
                            ab.icon_foreground_ref,
                            ab.name,
                            ab.name_len,
                        );
                    } else if slot.source_type == 0x2030000
                        && (slot.source_index as usize) < snap_items.len()
                    {
                        let it = &snap_items[slot.source_index as usize];
                        if it.populated {
                            current_slots[b][s] = (0, it.icon_ref, it.name, it.name_len);
                        }
                    }
                }
            }
            if let Ok(mut prev_guard) = PREV_SLOTS.lock() {
                if prev_guard.as_ref() != Some(&current_slots) {
                    for b in 0..3 {
                        for s in 0..bars[b].slot_count as usize {
                            let (bg, fg, name_bytes, name_len) = current_slots[b][s];
                            let name = if name_len > 0 {
                                String::from_utf8_lossy(&name_bytes[..name_len as usize])
                                    .to_string()
                            } else {
                                "empty".to_string()
                            };
                            eprintln!(
                                "[hotbar] bar {b} slot {s} -> (bg: 0x{bg:08X}, fg: 0x{fg:08X}) {name}"
                            );
                        }
                    }
                    *prev_guard = Some(current_slots);
                }
            }

            crate::fork_ext::addons::ability::AbilitiesSnapshot {
                ability_count: ability_count as u16,
                selected_bar,
                abilities: snap_abilities,
                bars,
                items: snap_items,
            }
        } else {
            crate::fork_ext::addons::ability::AbilitiesSnapshot::default()
        };

        let chat = if in_game {
            crate::fork_ext::addons::chat::snapshot_chat(&self.chat, self.session.now())
        } else {
            crate::fork_ext::addons::chat::ChatSnapshot::default()
        };

        let combat = if in_game {
            crate::fork_ext::addons::combat::snapshot_combat_events()
        } else {
            crate::fork_ext::addons::combat::CombatSnapshot::default()
        };

        // The full-screen map counts as a menu: on the PS2 it is a page of the start menu.
        let start_menu = in_game
            && (is_menu_mode(self.mode)
                || self.start.as_ref().is_some_and(|s| s.menu.visible)
                || self.map.open);
        let ui_mode = if !in_game {
            0
        } else if self.mode != 0 {
            self.mode as u32
        } else if self.start.as_ref().is_some_and(|s| s.menu.visible) {
            0x0D
        } else {
            0
        };
        let ui_busy = in_game
            && (ui_mode != 0
                || start_menu
                || self.map.open
                || self.map.search.open
                || self.search.open
                || self.typing.is_some());

        let mut snapshot = crate::fork_ext::addons::host::Snapshot {
            capture_pending: false,
            character: crate::fork_ext::addons::named_profiles::CharacterSnapshot::new(
                connected,
                &self.player.name,
                self.dump.player.class,
            ),
            in_game,
            start_menu,
            ui_mode,
            ui_busy,
            health: self.forge_health.snapshot(connected, hud_ready),
            player,
            group,
            entities,
            abilities,
            weapon_sets: self
                .hud
                .as_ref()
                .map(|(vui, _, _)| {
                    crate::fork_ext::addons::weapon_sets::WeaponSetsSnapshot::from_native(
                        &self.gear,
                        &self.dump.inventory,
                        &self.hud_menus,
                        in_game,
                        &vui.words,
                        vui.language,
                    )
                })
                .unwrap_or_default(),
            chat,
            combat,
            input: crate::fork_ext::addons::input::live_snapshot(),
            effects: crate::fork_ext::addons::effects::EffectsSnapshot::from_native(&self.effects),
            journal: crate::fork_ext::addons::quest_log::JournalSnapshot::from_titles(
                in_game,
                &self.quests.titles,
            ),
            chat_history: crate::fork_ext::chat_history::snapshot(
                in_game && self.mode == 33 && !crate::hud::fork_hud::chat_window_visible(),
            ),
        };
        crate::fork_ext::addons::native_core::CORE.with(|c| {
            crate::fork_ext::addons::native_projection::apply(&mut snapshot, &c.borrow().view)
        });
        api_capture::updates::cache(key, snapshot);
        snapshot
    }
}

fn is_menu_mode(mode: i32) -> bool {
    matches!(
        mode,
        1    // Corpse loot window (VIWndLoot)
        | 4  // GM Goto window (VIWndGoto, adminplay.rs:7,78,263; not trainer.rs:364 internal sub-mode)
        | 5  // Spellbook / abilities (VIWndSpellBook)
        | 6  // Recipe book (VIWndRecipeBook)
        | 10 // Merchant (VIWndMerchant)
        | 11 // Bank (VIWndBank)
        | 12 // Trainer / Character Summary (VIWndTrainer)
        | 13 // Start menu (VIWndMainMenu)
        | 14 // Confirm / message box (VIWndMBox)
        | 15 // Trade window (VIWndTrade)
        | 16 // Trade ask
        | 17 // Buddy request
        | 18 // Buddy list (VIWndBuddy)
        | 19 // Blacksmith (VIWndBlacksmith, support.rs:3,37,121)
        | 20 // Guild invitation
        | 21 // Guild roster (VIWndGuild)
        | 22 // Quest journal (VIWndQuestLog)
        | 23 // Mail compose
        | 24 // Mail inbox
        | 26 // Ignore list (VIWndIgnoreList)
        | 27 // GM Buff PC window (VIWndBuffPC, adminplay.rs:6,78,190)
        | 28 // Mail tell
        | 29 // Mail reply
        | 31 // CS request form (support.rs:4,37)
        | 32 // CS inbox (support.rs:4,37)
        | 34 // Options (VIWndOptions)
        | 35 // Who list (VIWndWhoList)
        | 37 // Tradeskills experiment (VIWndExperiment)
        | 39 // Auction search (VIWndAuctionSearch)
        | 40 // Class Mastery (VIWndClassMastery)
        | 41 // Factions (VIWndFaction)
        | 42 // Live group search (VIWndGroupSearch)
        | 43 // Duel invite (duelplay.rs:3,31)
        | 44 // Duel window / wager (VIWndDuel, duelplay.rs:3,31)
    )
}

pub(super) fn health_hidden(play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        play.forge_health
            .snapshot(
                play.session.self_id.is_some() && play.disconnected.is_none() && !play.quit,
                play.in_world_now(),
            )
            .is_available()
            && crate::fork_ext::addons::visibility::published_health_hidden()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = play;
        false
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn weapon_sets_requests(play: &mut Play, ready: bool) {
    let pending = crate::fork_ext::addons::weapon_sets::take_pending();
    if ready {
        for set in pending {
            play.forge_use_weapon_set(set);
        }
    }
}

pub(super) fn health_redraw(play: &mut Play, step: bool, loading_changed: bool) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let options_closed =
            if crate::fork_ext::addons::take_close_options_on_open() && play.mode == 34 {
                play.mode = 0;
                play.apply_camera = true;
                play.sound_ops.push(crate::options::OptionsUp::PreviewEnd);
                true
            } else {
                false
            };
        let ready = play
            .forge_health
            .snapshot(
                play.session.self_id.is_some() && play.disconnected.is_none() && !play.quit,
                play.in_world_now(),
            )
            .is_available();
        crate::fork_ext::addons::visibility::set_play_ready(ready);
        let hidden = health_hidden(play);
        let vis = if ready {
            crate::fork_ext::addons::visibility::published_visibility()
        } else {
            Default::default()
        };
        let hud_changed = play.forge_health.hud_changed(hidden);
        let vis_changed = play.forge_health.vis_changed(vis);
        hud_changed || vis_changed || options_closed || step || loading_changed
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = play;
        step || loading_changed
    }
}
use crate::fork_ext::telemetry::{CombatEvent, CombatKind};
struct CombatSnapshot {
    actor: String,
    target: String,
    amount: u32,
    kind: CombatKind,
}
impl CombatSnapshot {
    fn event(&self, ts: f64) -> CombatEvent<'_> {
        CombatEvent {
            ts,
            actor: &self.actor,
            target: &self.target,
            amount: self.amount,
            ability: "melee",
            kind: self.kind,
        }
    }
}
fn snapshot(play: &Play, attacker: u32, amount: i32, defender: u32) -> CombatSnapshot {
    let me = play.self_id32();
    let pet = (play.pet_id != u32::MAX).then_some(play.pet_id);
    let name = |id| {
        if Some(id) == me {
            "you".to_string()
        } else if Some(id) == pet {
            "pet".to_string()
        } else {
            play.entity_name(id)
                .unwrap_or_else(|| format!("{id:#010x}"))
        }
    };
    let kind = if amount < 0 {
        CombatKind::Heal
    } else if Some(attacker) == me || Some(attacker) == pet {
        CombatKind::Damage
    } else {
        CombatKind::DamageTaken
    };
    CombatSnapshot {
        actor: name(attacker),
        target: name(defender),
        amount: amount.unsigned_abs(),
        kind,
    }
}
impl Play {
    pub(super) fn fork_combat_event(
        &self,
        _attacker: u32,
        _amount: i32,
        _defender: u32,
        _color: Option<[u8; 3]>,
    ) {
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::combat::record_combat_event(_attacker, _amount, _defender);
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::native_core::observe_hit(_attacker, _defender, _amount, _color);
        // Unit tests never emit to a real companion. The browser has no native socket
        // or SystemTime clock; neither is touched by this hook there.
        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
        {
            let event = snapshot(self, _attacker, _amount, _defender);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64());
            crate::fork_ext::telemetry::combat_event(&event.event(ts));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn weapon_sets_use_matches_native_carousel_and_preserves_input() {
        let mut native = play();
        let mut addon = play();
        for p in [&mut native, &mut addon] {
            p.gear.set(1, "set".into(), 10, u32::MAX);
            p.session.record = Some(Vec::new());
            p.hud_menus
                .build(&p.abilities, &p.dump.inventory, &p.gear, p.hud_bars());
        }
        native.hud_menus.sel = 3;
        native.hud_menus.menus[3].sel = 1;
        native.hud_key(crate::hudmenu::Key::Use);
        let host = crate::fork_ext::addons::host::Host::new().unwrap();
        let mut snapshot = crate::fork_ext::addons::host::Snapshot::default();
        snapshot.weapon_sets =
            crate::fork_ext::addons::weapon_sets::WeaponSetsSnapshot::from_native(
                &addon.gear,
                &addon.dump.inventory,
                &addon.hud_menus,
                true,
                &Default::default(),
                0,
            );
        host.snapshot.set(snapshot);
        host.execute("assert(eqoa.WeaponSets.Use(1))").unwrap();
        weapon_sets_requests(&mut addon, true);
        assert_eq!(addon.session.record, native.session.record);
        assert_eq!(addon.session.record.as_ref().unwrap()[0].1, [0x22, 0, 1]);
        assert_eq!(addon.hud_menus.menus[3].sel, 1);
        addon.hud_menus.sel = 3;
        addon.hud_key(crate::hudmenu::Key::Down);
        assert_eq!(
            addon.hud_menus.menus[3].sel, 2,
            "native navigation still moves empty slots"
        );
        for i in [-1, 0, 5] {
            assert!(!addon.forge_use_weapon_set(i));
        }
        addon.mode = 34;
        assert!(!addon.forge_use_weapon_set(1));
        addon.mode = 38;
        assert!(addon.forge_use_weapon_set(4));
        addon.gear.on = false;
        assert!(!addon.forge_use_weapon_set(4));
        assert_eq!(addon.session.record.as_ref().unwrap().len(), 2);
    }
    /// R2 (mode 33) scrolls the chat history for an addon-owned chat window:
    /// the snapshot reports the lines and the HUD carousel keeps its ability
    /// bar on screen. Without a claim both stay stock.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_chat_history_r2_scrolls_claimed_chat_and_keeps_ability_bar() {
        use crate::fork_ext::addons::{
            health::InstanceToken,
            visibility::{HudClaimsRegistry, publish_claims},
        };
        struct Discard;
        impl vi::ui::Raster for Discard {
            fn blit(
                &mut self,
                _: u32,
                _: u32,
                _: [i32; 2],
                _: [i32; 2],
                _: [i32; 2],
                _: i32,
                _: f32,
            ) {
            }
            fn stretch_blit(
                &mut self,
                _: u32,
                _: u32,
                _: [i32; 2],
                _: [i32; 2],
                _: [i32; 2],
                _: [i32; 2],
                _: i32,
                _: f32,
            ) {
            }
            fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {}
            fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
            fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {}
            fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {}
            fn string_width(&self, _: &[u16]) -> i32 {
                0
            }
            fn point_size(&self) -> i32 {
                12
            }
            fn viewport(&self) -> [i32; 4] {
                [0, 0, 639, 447]
            }
            fn set_viewport(&mut self, _: [i32; 4]) {}
        }

        crate::fork_ext::chat_history::reset();
        let mut p = play();
        p.loading.wait.take();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        p.mode = 33;

        // Stock: R2 belongs to the native window; mode 33 keeps the carousel
        // up either way.
        publish_claims(&HudClaimsRegistry::new());
        let stock = p.forge_snapshot(None).chat_history;
        assert!(!stock.active, "stock mode 33 scrolls the native chat");
        assert!(
            crate::hud::fork_hud::chat_history_keeps_ability_bar(p.mode),
            "stock mode 33 keeps the ability bar up"
        );

        // Wereoxxs claims the chat window, so the HUD keeps drawing.
        let mut reg = HudClaimsRegistry::new();
        reg.disable("chat_window", InstanceToken::next().unwrap(), "wereoxxs_ui");
        publish_claims(&reg);
        let claimed = p.forge_snapshot(None).chat_history;
        assert!(claimed.active, "claimed chat follows R2");
        assert_eq!(claimed.total, 0, "no invented scroll");
        assert!(
            crate::hud::fork_hud::chat_history_keeps_ability_bar(p.mode),
            "the ability bar stays up while the claimed chat is scrolled"
        );
        assert!(
            crate::hud::fork_hud::ability_bar_shown(false, p.mode),
            "the carousel's mode list takes the claimed chat too"
        );

        // The native key path taps the stock scroll: one step, then repeats on
        // the stock 0.45 s / 0.05 s clock the draw wrapper advances.
        let atlas = (0..252)
            .map(|_| vi::ui::UiTexEntry {
                uv1: [16, 16],
                ..Default::default()
            })
            .collect();
        let vui = crate::ui::Vui::new(Default::default(), Default::default(), atlas);
        let mut wnd = crate::chat::WndChat::new();
        let draw = |wnd: &mut crate::chat::WndChat, dt: f32| {
            crate::hud::fork_hud::draw_chat_window(wnd, &vui, &mut Discard, 0, dt);
        };
        p.chat_history_key(19, true);
        draw(&mut wnd, 0.44);
        assert_eq!(
            p.forge_snapshot(None).chat_history.total,
            1,
            "up steps back once at once"
        );
        draw(&mut wnd, 0.01);
        assert_eq!(
            p.forge_snapshot(None).chat_history.total,
            2,
            "and repeats after 0.45 s"
        );
        p.chat_history_key(19, false);
        draw(&mut wnd, 1.0);
        assert_eq!(
            p.forge_snapshot(None).chat_history.total,
            2,
            "release stops the repeat"
        );
        p.chat_history_key(20, true);
        assert_eq!(
            p.forge_snapshot(None).chat_history.total,
            1,
            "down steps forward"
        );
        draw(&mut wnd, 0.40);
        assert_eq!(
            p.forge_snapshot(None).chat_history.total,
            1,
            "no repeat before 0.45 s"
        );
        p.chat_history_key(20, false);

        // Negative controls: another mode ignores the key, and releasing the
        // claim restores stock hiding and stock-only scrolling.
        p.mode = 0;
        assert!(!p.forge_snapshot(None).chat_history.active);
        p.mode = 33;
        p.chat_history_key(150, true);
        assert_eq!(
            p.forge_snapshot(None).chat_history.total,
            1,
            "non-scroll keys pass through"
        );
        publish_claims(&HudClaimsRegistry::new());
        assert!(!p.forge_snapshot(None).chat_history.active);
        assert!(
            crate::hud::fork_hud::chat_history_keeps_ability_bar(p.mode),
            "mode 33 keeps the carousel without a claim"
        );
        crate::fork_ext::chat_history::reset();
    }
    pub(super) fn play() -> Play {
        play_as("test")
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn play_as(account: &str) -> Play {
        crate::fork_ext::addons::native_core::fixture_demand(true);
        struct Nowhere;
        impl net::session::Datagrams for Nowhere {
            fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
            fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
                None
            }
        }
        let mut session = net::session::Session::connect_with(
            Box::new(Nowhere),
            ([127, 0, 0, 1], 5000),
            ([127, 0, 0, 1], 10070),
            account,
            [0; 32],
        )
        .unwrap();
        session.self_id = Some(0x100);
        Play::new(session, net::session::Player::default(), Box::default())
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn mod_profiles_world_entry_uses_character_not_account_and_survives_loading() {
        use crate::fork_ext::addons::named_profiles::{Character, Document, Store};
        let account = "ACCOUNT_CANARY_NOT_A_CHARACTER";
        let mut p = play_as(account);
        p.player.name = "Ari".into();
        p.dump.player.class = 3;
        // The game has a connected character before HUD/HP/loading are ready.
        let snapshot = p.forge_snapshot(None);
        assert!(!snapshot.in_game);
        let identity = Character::new("server.example", snapshot.character.unwrap()).unwrap();
        assert_eq!(identity.name, "ari");
        assert_eq!(identity.class, 3);
        let root =
            std::env::temp_dir().join(format!("eqoa-profile-identity-{}", std::process::id()));
        let store = Store::new(Some(root.clone()));
        // Test reruns may reuse the process id, so a valid existing fixture is fine.
        if store.read("Healer").is_err() {
            store.create("Healer", &Document::empty()).unwrap();
        }
        store.bind(&identity, "Healer", false).unwrap();
        let bytes = std::fs::read_to_string(root.join("addons/profiles/characters.json")).unwrap();
        assert!(!bytes.contains(account));
        assert!(bytes.contains("ari"));
        p.session.self_id = None;
        assert!(p.forge_snapshot(None).character.is_none());
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_modern_journal_snapshot_uses_retail_titles_and_clears_on_disconnect() {
        let mut p = play();
        p.loading.wait.take();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        p.quests.add("Bat Eyes".encode_utf16().collect());
        p.quests.add("Caf\u{e9}".encode_utf16().collect());
        p.session.quest_progress.push(net::session::TrackedQuest {
            name: "Wrong tracker title".into(),
            goals: vec![],
        });
        let journal = p.forge_snapshot(None).journal;
        assert_eq!(journal.count, 2);
        assert_eq!(
            &journal.titles[0][..8],
            "Bat Eyes".encode_utf16().collect::<Vec<_>>().as_slice()
        );
        p.quests.remove(0);
        let journal = p.forge_snapshot(None).journal;
        assert_eq!(journal.count, 1);
        assert_eq!(
            &journal.titles[0][..4],
            "Caf\u{e9}".encode_utf16().collect::<Vec<_>>().as_slice()
        );
        p.session.self_id = None;
        assert_eq!(p.forge_snapshot(None).journal.count, 0);
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_health_play_snapshot_and_actual_hud_cache_predicate_follow_publication() {
        use crate::fork_ext::addons::visibility::{publish, published_health_hidden};
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                publish(false);
            }
        }
        let _reset = Reset;
        publish(false);
        let mut p = play();
        let initial_loading = p.loading.wait.take(); // explicit entered-world fixture
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        let mut bytes = vec![0; 233];
        bytes[28..32].copy_from_slice(&0_i32.to_le_bytes());
        bytes[32..36].copy_from_slice(&100_i32.to_le_bytes());
        assert!(!p.forge_snapshot(None).managed().in_game);
        p.forge_health.receive(&bytes);
        assert!(
            p.forge_snapshot(None).managed().in_game,
            "zero HP remains an in-game death"
        );
        assert!(!health_redraw(&mut p, false, false));
        publish(true);
        assert!(published_health_hidden());
        assert!(health_hidden(&p));
        assert!(
            health_redraw(&mut p, false, false),
            "claim transition must invalidate cached quads without game step"
        );
        assert!(
            !health_redraw(&mut p, false, false),
            "unchanged claim can replay cache"
        );
        p.session.self_id = None;
        assert!(!p.forge_snapshot(None).managed().in_game);
        assert!(
            health_redraw(&mut p, false, false),
            "connect gap restores health despite previously published claim"
        );
        p.session.self_id = Some(0x100);
        p.forge_health.reset();
        assert!(
            !p.forge_snapshot(None).managed().in_game,
            "same id with reset has no fresh receipt"
        );
        p.forge_health.receive(&bytes);
        assert!(health_redraw(&mut p, false, false));
        publish(false);
        assert!(health_redraw(&mut p, false, false));
        let mut reg = crate::fork_ext::addons::visibility::HudClaimsRegistry::new();
        let tok = crate::fork_ext::addons::health::InstanceToken::next().unwrap();
        reg.disable("compass_face", tok, "test");
        crate::fork_ext::addons::visibility::publish_claims(&reg);
        assert!(
            health_redraw(&mut p, false, false),
            "non-health PublishedVisibility change must invalidate cached HUD quads"
        );
        assert!(
            !health_redraw(&mut p, false, false),
            "unchanged non-health visibility can replay cache"
        );
        publish(false);
        assert!(
            health_redraw(&mut p, false, false),
            "releasing non-health visibility claim invalidates cached HUD quads"
        );
        assert!(health_redraw(&mut p, true, false));
        assert!(health_redraw(&mut p, false, true));
        p.disconnected = Some(0);
        assert!(!p.forge_snapshot(None).managed().in_game);
        p.disconnected = None;
        p.quit = true;
        assert!(!p.forge_snapshot(None).managed().in_game);
        p.quit = false;
        p.loading.wait = initial_loading;
        assert!(
            !p.forge_snapshot(None).managed().in_game,
            "loading must not display health overlay"
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_play_click_gates_hidden_compass_ability_bar_and_chat_while_keys_and_typing_work() {
        use crate::fork_ext::addons::{
            health::InstanceToken,
            visibility::{HudClaimsRegistry, publish, publish_claims},
        };
        use crate::mouse::Click;
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                publish(false);
            }
        }
        let _reset = Reset;
        publish(false);

        let mut p = play();
        let _ = p.loading.wait.take();
        let make_vui = || {
            let atlas = (0..252)
                .map(|_| vi::ui::UiTexEntry {
                    uv1: [16, 16],
                    ..Default::default()
                })
                .collect();
            crate::ui::Vui::new(Default::default(), Default::default(), atlas)
        };
        let vui = make_vui();
        p.hud = Some((make_vui(), Default::default(), crate::hud::Hud::new()));
        p.hud_menus.build(
            &Default::default(),
            &Default::default(),
            &Default::default(),
            Default::default(),
        );
        p.hud_menus.sel = 0;

        let mut wnd = crate::chat::WndChat::new();
        wnd.mode = 1;
        wnd.text
            .add_text(&crate::ui::wide("hello"), [1.0; 4], 0, |_| 0);
        struct DummyRaster;
        impl vi::ui::Raster for DummyRaster {
            fn blit(
                &mut self,
                _: u32,
                _: u32,
                _: [i32; 2],
                _: [i32; 2],
                _: [i32; 2],
                _: i32,
                _: f32,
            ) {
            }
            fn stretch_blit(
                &mut self,
                _: u32,
                _: u32,
                _: [i32; 2],
                _: [i32; 2],
                _: [i32; 2],
                _: [i32; 2],
                _: i32,
                _: f32,
            ) {
            }
            fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {}
            fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
            fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {}
            fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {}
            fn string_width(&self, _: &[u16]) -> i32 {
                0
            }
            fn point_size(&self) -> i32 {
                12
            }
            fn viewport(&self) -> [i32; 4] {
                [0, 0, 639, 447]
            }
            fn set_viewport(&mut self, _: [i32; 4]) {}
        }
        wnd.draw(&vui, &mut DummyRaster, 0, 0.0);
        assert!(wnd.contains([422.0, 420.0]));
        p.wnd_chat = Some(wnd);

        let size = (1280, 720);
        let (side, up) = (render::ui::side(size), render::ui::top(size));
        let compass_win = render::ui::window_from_frame(size, [39.0 - side, 54.0 - up]);
        let menu_at = [430, 25];
        let ability_win = render::ui::window_from_frame(
            size,
            [
                menu_at[0] as f32 + 10.0 + side,
                menu_at[1] as f32 + 10.0 - up,
            ],
        );
        let chat_win = render::ui::window_from_frame(size, [422.0 + side, 420.0 + up]);

        let mut reg = HudClaimsRegistry::new();
        let tok = InstanceToken::next().unwrap();
        for el in [
            "compass_face",
            "compass_back",
            "hotbar_draw_compact",
            "hotbar_draw_expanded",
            "chat_window",
        ] {
            reg.disable(el, tok, "test");
        }
        publish_claims(&reg);

        // Clicks on hidden compass, ability bar, and chat window do nothing
        assert!(
            !p.point(
                Click::Target(compass_win[0] as f64, compass_win[1] as f64),
                size,
                false
            ),
            "click on hidden compass must do nothing"
        );
        assert!(!p.map.open);

        assert!(
            !p.point(
                Click::Target(ability_win[0] as f64, ability_win[1] as f64),
                size,
                false
            ),
            "click on hidden ability bar must do nothing"
        );
        assert_eq!(p.hud_menus.sel, 0);

        assert!(
            !p.point(
                Click::Target(chat_win[0] as f64, chat_win[1] as f64),
                size,
                false
            ),
            "click on hidden chat window must do nothing"
        );
        assert_eq!(p.mode, 0);

        // Keyboard/pad hotbar keys and chat typing still work while hidden
        p.hud_key(crate::hudmenu::Key::Right);
        assert_ne!(
            p.hud_menus.sel, 0,
            "pad/keyboard hotbar keys still work while hidden"
        );
        p.hud_menus.sel = 0;

        p.open_edit(0xB0F0000, 128, -1, false);
        assert_eq!(
            p.mode, 2,
            "chat typing still opens while chat window is hidden"
        );
        p.mode = 0;

        // Unhidden: clicks on compass, ability bar, and chat window work
        publish(false);

        assert!(
            p.point(
                Click::Target(compass_win[0] as f64, compass_win[1] as f64),
                size,
                false
            ),
            "click on unhidden compass opens map"
        );
        assert!(p.map.open);
        p.map.open = false;

        assert!(
            p.point(
                Click::Target(ability_win[0] as f64, ability_win[1] as f64),
                size,
                false
            ),
            "click on unhidden ability bar switches menu"
        );
        assert_ne!(p.hud_menus.sel, 0);
        p.hud_menus.sel = 0;

        assert!(
            p.point(
                Click::Target(chat_win[0] as f64, chat_win[1] as f64),
                size,
                false
            ),
            "click on unhidden chat window opens chat line"
        );
        assert_eq!(p.mode, 2);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_addon_manager_open_from_options_exits_mode_34_and_restores_all_hud_elements_and_connect_gap_cache()
     {
        use crate::fork_ext::addons::{
            health::InstanceToken,
            visibility::{HudClaimsRegistry, publish, publish_claims},
        };
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                let _ = crate::fork_ext::addons::take_open_request();
                publish(false);
            }
        }
        let _reset = Reset;
        let _ = crate::fork_ext::addons::take_open_request();
        publish(false);

        let mut p = play();
        let _ = p.loading.wait.take();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        let mut bytes = vec![0; 233];
        bytes[28..32].copy_from_slice(&100_i32.to_le_bytes());
        bytes[32..36].copy_from_slice(&100_i32.to_le_bytes());
        p.forge_health.receive(&bytes);
        assert!(!health_redraw(&mut p, false, false));

        // Simulate opening Start Menu -> Options -> Extended -> Mods & Addons (Play.mode == 34),
        // where ability bars and active effects are suppressed by Play::draw_hud's mode checks.
        p.mode = 34;
        p.apply_camera = false;
        p.sound_ops.clear();
        assert!(!crate::influence::shown(p.mode));
        assert!(!((p.mode == 0 || p.mode == 30 || p.mode == 38) && !p.search.open));

        // Activating "Addon Manager" (option item 1053 -> request_open) must request the overlay
        // AND exit mode 34 on the next HUD redraw check so ability bars and active effects are no longer stuck hidden.
        crate::fork_ext::addons::request_open();
        assert!(
            health_redraw(&mut p, false, false),
            "opening Addon Manager from mode 34 must invalidate HUD cache and exit VIWndOptions"
        );
        assert_eq!(
            p.mode, 0,
            "Addon Manager activation must exit mode 34 to mode 0"
        );
        assert!(p.apply_camera, "exiting mode 34 must restore camera");
        assert!(
            p.sound_ops
                .iter()
                .any(|op| matches!(op, crate::options::OptionsUp::PreviewEnd)),
            "exiting mode 34 must end options sound preview"
        );
        assert!(crate::influence::shown(p.mode));
        assert!((p.mode == 0 || p.mode == 30 || p.mode == 38) && !p.search.open);
        assert!(crate::fork_ext::addons::take_open_request());

        // Now verify that when all non-health HUD elements have published claims and Play hits a
        // connect gap / unready frame before Runtime::draw runs, fork_hud visibility predicates
        // agree with health_redraw and report visible so ui.keep() never caches stale hidden quads.
        let mut reg = HudClaimsRegistry::new();
        let tok = InstanceToken::next().unwrap();
        for el in [
            "health_bar",
            "power_bar",
            "main_exp_bar",
            "secondary_exp_bar",
            "compass_face",
            "compass_back",
            "target_nameplate",
            "group_member_panel",
            "group_compass_marks",
            "pet_panel",
            "chat_window",
            "hotbar_draw_compact",
            "hotbar_draw_expanded",
            "active_effects",
        ] {
            reg.disable(el, tok, "test");
        }
        publish_claims(&reg);
        assert!(health_redraw(&mut p, false, false));

        let hud_state = crate::hud::HudState::default();
        assert!(!crate::hud::fork_hud::compass_visible(&hud_state));
        assert!(!crate::hud::fork_hud::group_compass_marks_visible(
            &hud_state
        ));
        assert!(!crate::hud::fork_hud::target_visible(&hud_state));
        assert!(!crate::hud::fork_hud::group_visible(&hud_state));
        assert!(!crate::hud::fork_hud::pet_visible(&hud_state));
        assert!(!crate::hud::fork_hud::power_bar_visible(&hud_state));
        assert!(!crate::hud::fork_hud::main_exp_bar_visible(&hud_state));
        assert!(!crate::hud::fork_hud::secondary_exp_bar_visible(&hud_state));
        assert!(!crate::hud::fork_hud::exp_bar_visible(&hud_state));
        assert!(!crate::hud::fork_hud::chat_window_visible());
        assert!(!crate::hud::fork_hud::ability_bar_visible());
        assert!(!crate::hud::fork_hud::active_effects_visible());

        // Connect gap occurs while PUBLISHED_VIS still holds claims:
        p.session.self_id = None;
        assert!(
            health_redraw(&mut p, false, false),
            "connect gap must invalidate HUD cache for all elements"
        );
        assert!(crate::hud::fork_hud::compass_visible(&hud_state));
        assert!(crate::hud::fork_hud::group_compass_marks_visible(
            &hud_state
        ));
        assert!(crate::hud::fork_hud::target_visible(&hud_state));
        assert!(crate::hud::fork_hud::group_visible(&hud_state));
        assert!(crate::hud::fork_hud::pet_visible(&hud_state));
        assert!(crate::hud::fork_hud::power_bar_visible(&hud_state));
        assert!(crate::hud::fork_hud::main_exp_bar_visible(&hud_state));
        assert!(crate::hud::fork_hud::secondary_exp_bar_visible(&hud_state));
        assert!(crate::hud::fork_hud::exp_bar_visible(&hud_state));
        assert!(crate::hud::fork_hud::chat_window_visible());
        assert!(crate::hud::fork_hud::ability_bar_visible());
        assert!(crate::hud::fork_hud::active_effects_visible());

        // Reconnect restores readiness and reapplies active claims:
        p.session.self_id = Some(0x100);
        p.forge_health.receive(&bytes);
        assert!(health_redraw(&mut p, false, false));
        assert!(!crate::hud::fork_hud::compass_visible(&hud_state));
        assert!(!crate::hud::fork_hud::ability_bar_visible());
        assert!(!crate::hud::fork_hud::active_effects_visible());
        assert!(!crate::hud::fork_hud::chat_window_visible());
    }

    #[test]
    fn self_pet_and_other_attribution_keep_the_existing_parser_contract() {
        let mut p = play();
        p.pet_id = 0x200;
        for (attacker, amount, defender, actor, target, kind) in [
            (0x100, 7, 0x300, "you", "0x00000300", CombatKind::Damage),
            (0x200, 8, 0x100, "pet", "you", CombatKind::Damage),
            (
                0x300,
                9,
                0x200,
                "0x00000300",
                "pet",
                CombatKind::DamageTaken,
            ),
            (0x300, -9, 0x100, "0x00000300", "you", CombatKind::Heal),
        ] {
            let event = snapshot(&p, attacker, amount, defender);
            assert_eq!(
                (&*event.actor, &*event.target, event.kind),
                (actor, target, kind)
            );
            assert_eq!(event.amount, amount.unsigned_abs());
        }
        p.pet_id = u32::MAX;
        assert_eq!(
            snapshot(&p, u32::MAX, 7, 0x100).kind,
            CombatKind::DamageTaken
        );
    }
    #[test]
    fn combat_snapshot_serializes_entity_names_and_minimum_signed_amount() {
        let mut p = play();
        p.entities.slots = vec![Some(net::entity::Entity {
            id: 0x300,
            creature: Some(net::entity::Creature {
                name: "A \"quoted\" name\\tail".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        })];
        let event = snapshot(&p, 0x300, i32::MIN, 0x100);
        let wire = crate::fork_ext::telemetry::json(&event.event(1.25));
        let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(value["actor"], "A \"quoted\" name\\tail");
        assert_eq!(value["amount"], 2147483648u64);
        assert_eq!(value["kind"], "heal");
        assert_eq!(value["ability"], "melee");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_play_steady_frame_does_not_rebuild_atlas() {
        let mut p = play();
        let vui = crate::ui::Vui::new(
            Default::default(),
            Default::default(),
            vec![vi::ui::UiTexEntry::default(); 252],
        );
        p.hud = Some((vui, Default::default(), crate::hud::Hud::new()));

        let lib = esf::Library::new();

        frame_library(&p, &lib, None);
        let count1 = atlas_rebuild_count();

        for _ in 0..10 {
            frame_library(&p, &lib, None);
        }
        let count2 = atlas_rebuild_count();

        assert_eq!(
            count1, count2,
            "steady frames must not rebuild the HUD atlas or clone TEXTURES.TXT"
        );
    }
    /// A server keeping more than the game's 24 slots (NPC/PC Display) puts
    /// the target and the pet past them: the addons' records 24 and up hold
    /// them, so a mod's target box finds what the player targeted; anyone
    /// else past slot 23 stays out, and one already in the 24 is not repeated.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_snapshot_keeps_the_target_past_the_games_24_slots() {
        let mut p = play();
        let _ = p.loading.wait.take();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        let mut bytes = vec![0; 233];
        bytes[28..32].copy_from_slice(&100_i32.to_le_bytes());
        bytes[32..36].copy_from_slice(&100_i32.to_le_bytes());
        p.forge_health.receive(&bytes);
        let creature = |id: u32, name: &str| net::entity::Entity {
            id,
            creature: Some(net::entity::Creature {
                name: name.into(),
                level: 18,
                ..Default::default()
            }),
            ..Default::default()
        };
        p.entities.slots = vec![None; 40];
        p.entities.slots[0] = Some(creature(0x100, "Zaptir"));
        for i in 1..24 {
            p.entities.slots[i] = Some(creature(0x200 + i as u32, "near"));
        }
        p.entities.slots[30] = Some(creature(0x18ade, "desert madman"));
        p.entities.slots[33] = Some(creature(0x18da0, "Primal Spirit"));
        p.entities.slots[35] = Some(creature(0x18bf9, "Gharj"));
        p.target = 0x18ade;
        p.pet_id = 0x18da0;
        let snap = p.forge_snapshot(None);
        assert!(snap.in_game);
        let ids: Vec<u32> = snap.entities.entities.iter().map(|e| e.id).collect();
        let at = |id: u32| ids.iter().position(|&i| i == id);
        let t = at(0x18ade).expect("the target past slot 23 is in the addons' records");
        let e = snap.entities.entities[t];
        assert_eq!(&e.name[..e.name_len as usize], b"desert madman");
        assert!(at(0x18da0).is_some(), "the pet too");
        assert_eq!(at(0x18bf9), None, "no one the HUD does not follow");
        // control: a target inside the 24 keeps its slot and is not repeated
        p.target = 0x205;
        let snap = p.forge_snapshot(None);
        assert_eq!(
            snap.entities
                .entities
                .iter()
                .filter(|e| e.id == 0x205)
                .count(),
            1
        );
        assert_eq!(snap.entities.entities[5].id, 0x205);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_snapshot_menu_mode_semantics_match_reference() {
        let mut p = play();
        let _ = p.loading.wait.take();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));
        let mut bytes = vec![0; 233];
        bytes[28..32].copy_from_slice(&100_i32.to_le_bytes());
        bytes[32..36].copy_from_slice(&100_i32.to_le_bytes());
        p.forge_health.receive(&bytes);
        assert!(p.forge_snapshot(None).in_game);

        // 1. Mode 0: Normal gameplay (nothing open)
        p.mode = 0;
        let snap = p.forge_snapshot(None);
        assert!(!snap.start_menu);
        assert_eq!(snap.ui_mode, 0); // normal
        assert!(!snap.ui_busy);

        // 1b. Busy states in normal mode (matching play.rs:3950: map, map search, search, typing)
        p.map.open = true;
        assert!(
            p.forge_snapshot(None).ui_busy,
            "map.open must report ui_busy == true"
        );
        assert!(
            p.forge_snapshot(None).start_menu,
            "the full-screen map must pause addons like a menu"
        );
        p.map.open = false;

        p.map.search.open = true;
        assert!(
            p.forge_snapshot(None).ui_busy,
            "map.search.open must report ui_busy == true"
        );
        p.map.search.open = false;

        p.search.open = true;
        assert!(
            p.forge_snapshot(None).ui_busy,
            "search.open must report ui_busy == true"
        );
        p.search.open = false;

        p.typing = Some(String::from("test input"));
        assert!(
            p.forge_snapshot(None).ui_busy,
            "typing.is_some() must report ui_busy == true"
        );
        p.typing = None;

        // 2. Start menu and all sub-screen modes (spellbook, auctions, trainer, quests, options, etc.)
        let menu_modes = [
            (1, 1, "loot"),
            (4, 4, "gm_goto"),
            (5, 5, "spellbook"),
            (6, 6, "recipes"),
            (10, 10, "merchant"),
            (11, 11, "bank"),
            (12, 12, "trainer"),
            (13, 13, "start_menu"),
            (14, 14, "confirm"),
            (15, 15, "trade"),
            (16, 16, "trade_ask"),
            (17, 17, "buddy_request"),
            (18, 18, "buddy_list"),
            (19, 19, "blacksmith"),
            (20, 20, "guild_invite"),
            (21, 21, "guild_roster"),
            (22, 22, "quest_log"),
            (23, 23, "mail_compose"),
            (24, 24, "mail_inbox"),
            (26, 26, "ignore_list"),
            (27, 27, "gm_buff_pc"),
            (28, 28, "mail_tell"),
            (29, 29, "mail_reply"),
            (31, 31, "cs_request"),
            (32, 32, "cs_inbox"),
            (34, 34, "options"),
            (35, 35, "who_list"),
            (37, 37, "experiment"),
            (39, 39, "auction_search"),
            (40, 40, "class_mastery"),
            (41, 41, "factions"),
            (42, 42, "group_search"),
            (43, 43, "duel_invite"),
            (44, 44, "duel_window"),
        ];

        for (mode, expected_ui_mode, name) in menu_modes {
            p.mode = mode;
            let snap = p.forge_snapshot(None);
            assert!(
                snap.start_menu,
                "mode {mode} ({name}) must report start_menu == true so addons pause"
            );
            assert_eq!(
                snap.ui_mode, expected_ui_mode,
                "mode {mode} ({name}) must report exact ui_mode"
            );
            assert!(
                snap.ui_busy,
                "mode {mode} ({name}) must report ui_busy == true"
            );
        }

        // 4. Non-menu modes: input, dialog answers (7), text dialog (9), context menu, r2 chat, fishing, manual camera
        let non_menu_modes = [
            (2, 2, "input"),
            (7, 7, "dialog_answers"),
            (9, 9, "text_dialog"),
            (30, 30, "context_menu"),
            (33, 33, "r2_chat"),
            (36, 36, "fishing"),
            (38, 38, "manual_camera"),
        ];

        for (mode, expected_ui_mode, name) in non_menu_modes {
            p.mode = mode;
            let snap = p.forge_snapshot(None);
            assert!(
                !snap.start_menu,
                "non-menu mode {mode} ({name}) must not report start_menu == true"
            );
            assert_eq!(
                snap.ui_mode, expected_ui_mode,
                "non-menu mode {mode} ({name}) must report exact ui_mode"
            );
            assert!(
                snap.ui_busy,
                "non-menu mode {mode} ({name}) owns input, must report ui_busy == true"
            );
        }

        // 5. Independent validation: scan client source files for all mode assignments
        // and assert that every assigned mode is classified either as a menu mode or as one of the
        // known non-menu modes (2, 30, 33, 36, 38, or 0 normal). Any newly added mode will fail here.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let src_dir = manifest_dir.join("src");
        let mut discovered_modes = std::collections::BTreeSet::new();

        for entry in std::fs::read_dir(&src_dir).expect("src dir must be readable") {
            let entry = entry.expect("valid dir entry");
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
                let content = std::fs::read_to_string(&path).expect("file readable");
                if content.contains("impl Play") || content.contains("impl super::Play") {
                    for line in content.lines() {
                        if let Some(pos) = line.find("self.mode = ") {
                            let rest = &line[pos + "self.mode = ".len()..];
                            let num_str: String =
                                rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                            if let Ok(m) = num_str.parse::<i32>() {
                                discovered_modes.insert(m);
                            }
                        }
                        for prefix in &["open_panel(", "open_guild(", "open_support(", "open_mail("]
                        {
                            let mut cur = line;
                            while let Some(pos) = cur.find(prefix) {
                                let rest = &cur[pos + prefix.len()..];
                                let num_str: String =
                                    rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                                if let Ok(m) = num_str.parse::<i32>() {
                                    discovered_modes.insert(m);
                                }
                                cur = rest;
                            }
                        }
                    }
                }
            }
        }

        let known_non_menu: std::collections::BTreeSet<i32> =
            [0, 2, 7, 9, 30, 33, 36, 38].into_iter().collect();
        for mode in discovered_modes {
            let is_non_menu = known_non_menu.contains(&mode);
            let is_menu = is_menu_mode(mode);
            assert!(
                is_menu || is_non_menu,
                "Mode {mode} assigned in client is not classified as menu or non-menu mode!"
            );
        }

        // 6. Out of game: everything defaults to inactive
        p.session.self_id = None;
        let snap = p.forge_snapshot(None);
        assert!(!snap.in_game);
        assert!(!snap.start_menu);
        assert_eq!(snap.ui_mode, 0);
        assert!(!snap.ui_busy);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_minimap_stays_up_in_chat_history_mode() {
        use crate::fork_ext::addons::health::InstanceToken;
        use crate::fork_ext::addons::visibility::{HudClaimsRegistry, publish_claims};

        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                publish_claims(&HudClaimsRegistry::new());
                let _ = crate::settings::set_minimap(true);
            }
        }
        let _reset = Reset;
        publish_claims(&HudClaimsRegistry::new());
        let _ = crate::settings::set_minimap(true);

        let mut p = play();
        let _ = p.loading.wait.take();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
            Default::default(),
            crate::hud::Hud::new(),
        ));

        // 1. In mode 0 (plain play), minimap is up
        p.mode = 0;
        assert!(p.minimap_up(), "minimap must be up in mode 0");

        // 2. In mode 33 (chat history / R2) without chat claimed, the minimap
        // stays up: the native history box (about y 336..444) is below it.
        p.mode = 33;
        assert!(crate::hud::fork_hud::chat_window_visible());
        assert!(crate::hud::fork_hud::chat_history_keeps_right_hud(33));
        assert!(
            p.minimap_up(),
            "minimap must stay up in mode 33 with the stock chat"
        );

        // 3. Claim the chat window (e.g. Wereoxxs UI or FrontiersForge mod)
        let mut reg = HudClaimsRegistry::new();
        let token = InstanceToken::next().unwrap();
        reg.disable("chat_window", token, "wereoxxs_ui");
        publish_claims(&reg);
        assert!(!crate::hud::fork_hud::chat_window_visible());

        // In mode 33 with chat claimed, the minimap stays up too.
        assert!(
            p.minimap_up(),
            "minimap must stay up in mode 33 when chat window is claimed by an addon"
        );

        // Negative controls:
        // Even with chat claimed, opening a menu (mode 1) hides minimap
        p.mode = 1;
        assert!(!crate::hud::fork_hud::chat_history_keeps_right_hud(1));
        assert!(
            !p.minimap_up(),
            "menu mode must hide minimap even if chat is claimed"
        );

        // Map open hides minimap
        p.mode = 33;
        p.map.open = true;
        assert!(!p.minimap_up(), "map.open must hide minimap in mode 33");
        p.map.open = false;

        // Search open hides minimap
        p.search.open = true;
        assert!(!p.minimap_up(), "search.open must hide minimap in mode 33");
        p.search.open = false;

        // Turning minimap off hides minimap
        let _ = crate::settings::set_minimap(false);
        assert!(
            !p.minimap_up(),
            "minimap setting off must hide minimap in mode 33"
        );
        let _ = crate::settings::set_minimap(true);
        assert!(
            p.minimap_up(),
            "restoring minimap setting restores minimap in mode 33"
        );

        // 4. Releasing the chat claim changes nothing: mode 33 keeps the
        // minimap with the stock chat too.
        publish_claims(&HudClaimsRegistry::new());
        assert!(crate::hud::fork_hud::chat_window_visible());
        assert!(
            p.minimap_up(),
            "releasing the chat claim keeps the minimap up in mode 33"
        );
    }
}

