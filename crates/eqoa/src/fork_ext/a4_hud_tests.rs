// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! Round A4 tests: HUD piece hiding & input suppression (seams 3-6), raid list & strip
//! takeover (seams 7-10), master-loot reads & actions, and the bundled target-frame demo mod.
//! Every test includes an explicit negative control.

use super::*;
use crate::fork_ext::addons::{
    action_broker,
    discovery::{DiscoveredAddon, discover},
    health::InstanceToken,
    host::Host,
    hud_layer,
    hud_layout,
    native_core::{self, CORE, Core},
    native_events,
    raid_strips,
    visibility::{self, HudClaimsRegistry, publish_claims},
    window_takeover::{self, TakeoverKind},
};
use crate::hud::{Hud, HudState, MemberState, PetState, Pick};
use crate::hudlayout::Piece;
use net::dump::{ItemList, NetItem};
use net::raid::{Choice, Command as RaidCmd, MasterLoot, Roll, State as RaidState};
use net::readycheck::Check as ReadyCheck;
use net::session::{Event, RaidMember};
use net::stream::{Reader, Writer};
use std::path::Path;

/// Counting raster for headless (no-GPU) tests of strips and HUD pieces.
#[derive(Default)]
struct CountingRaster {
    blits: usize,
    stretches: usize,
    fills: usize,
    strings: usize,
}
impl CountingRaster {
    fn total(&self) -> usize {
        self.blits + self.stretches + self.fills + self.strings
    }
}
impl vi::ui::Raster for CountingRaster {
    fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {
        self.blits += 1;
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
        self.stretches += 1;
    }
    fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {
        self.fills += 1;
    }
    fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {
        self.blits += 1;
    }
    fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {
        self.strings += 1;
    }
    fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {
        self.strings += 1;
    }
    fn string_width(&self, s: &[u16]) -> i32 {
        s.len() as i32 * 6
    }
    fn point_size(&self) -> i32 {
        12
    }
    fn viewport(&self) -> [i32; 4] {
        [0, 0, 639, 447]
    }
    fn set_viewport(&mut self, _: [i32; 4]) {}
}

fn reset_a4_state() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::set_use_game_windows(false);
    raid_strips::reset();
    publish_claims(&HudClaimsRegistry::new());
    crate::hudlayout::testing_reset();
    hud_layout::set_window_size(640, 448);
    hud_layout::set_hud_state(true, true, false, 93, 0);
}

fn ready_play() -> Play {
    reset_a4_state();
    let mut p = super::tests::play();
    let _ = p.loading.wait.take();
    p.session.record = Some(Vec::new());
    p.session.raid_offer = true;
    p.session.ready_offer = true;
    p.hud = Some((
        crate::ui::Vui::new(Default::default(), Default::default(), vec![]),
        Default::default(),
        Hud::new(),
    ));
    p
}

fn host_with(id: &str, caps: &[&str], replaces: &[&str]) -> Host {
    let host = Host::new().unwrap();
    {
        let sess = crate::fork_ext::addons::native::session(&host.lua);
        let mut s = sess.borrow_mut();
        s.addon = id.to_string();
        s.native_manifest = true;
        s.capabilities = if caps.is_empty() {
            None
        } else {
            Some(caps.iter().map(|c| (*c).to_string()).collect())
        };
        s.replaces = replaces.iter().map(|r| (*r).to_string()).collect();
    }
    host
}

fn test_vui_with_atlas() -> crate::ui::Vui {
    let atlas = (0..260)
        .map(|_| vi::ui::UiTexEntry {
            uv1: [16, 16],
            ..Default::default()
        })
        .collect();
    crate::ui::Vui::new(Default::default(), Default::default(), atlas)
}

fn sample_net_item(id: i64, name: &str, key: u32) -> NetItem {
    let mut w = Writer::default();
    // quantity, cur_hp, unk, equip_slot, quick_slot, key
    w.signed(1).signed(0).signed(0).signed(-1).signed(-1).u32(key);
    for v in [
        id, 100, 0, 4001, 0, -1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, -1, -1, 0, 0, 0, 0,
    ] {
        w.signed(v);
    }
    w.wide(name).wide("a raid drop").signed(0);
    net::dump::read_net_item(&mut Reader::new(&w.data))
}

fn sample_corpse_payload(items: &[NetItem]) -> Vec<u8> {
    let mut w = Writer::default();
    w.signed(items.len() as i64).u32(0);
    for it in items {
        w.signed(it.quantity)
            .signed(it.cur_hp)
            .signed(0)
            .signed(it.equip_slot)
            .signed(it.quick_slot)
            .u32(it.inv_index);
        for v in [
            it.item.id,
            it.item.value,
            0,
            it.item.icon,
            0,
            it.item.wear_slot,
            0,
            0,
            it.item.rent,
            0,
            0,
            0,
            0,
            it.item.level_req,
            it.item.max_stack,
            it.item.max_hp,
            0,
            it.item.class_mask,
            it.item.race_mask,
            it.item.p4c,
            0,
            0,
            it.item.p54,
        ] {
            w.signed(v);
        }
        w.wide(&it.item.name)
            .wide("a raid drop")
            .signed(0);
    }
    w.data
}

fn publish_play(p: &mut Play) {
    let walker = crate::play::Play::bundled_ui_walker();
    super::begin_iteration(p, false);
    let _ = p.forge_snapshot(Some(&walker));
}

fn step_actions(p: &mut Play) {
    let walker = crate::play::Play::bundled_ui_walker();
    super::begin_iteration(p, true);
    let _ = p.forge_snapshot(Some(&walker));
}

// =========================================================================
// 1. HUD piece hiding & click/bounds suppression (all 13 pieces + raid/group)
// =========================================================================

fn attach_hud_visibility(
    host: &Host,
    health_snap: crate::fork_ext::addons::health::HealthSnapshot,
) -> (
    std::rc::Rc<std::cell::RefCell<HudClaimsRegistry>>,
    visibility::VisibilityLease,
) {
    let health = std::rc::Rc::new(std::cell::RefCell::new(
        crate::fork_ext::addons::health::HealthBarClaimsRegistry::new(),
    ));
    let hud_claims = std::rc::Rc::new(std::cell::RefCell::new(HudClaimsRegistry::new()));
    let lease = visibility::VisibilityLease::with_hud_claims(health, hud_claims.clone()).unwrap();
    visibility::install_managed_health_adapter(host, &lease).unwrap();
    lease.update(health_snap, true);
    publish_claims(&hud_claims.borrow());
    (hud_claims, lease)
}

#[test]
fn a4_hud_hide_tokens_roundtrip_and_all_token() {
    let mut p = crate::play::Play::bundled_ui_fixture();
    let walker = crate::play::Play::bundled_ui_walker();
    reset_a4_state();
    super::begin_iteration(&mut p, false);
    let snap = p.forge_snapshot(Some(&walker)).managed();
    let host = host_with("hud_tokens_test", &[], &[]);
    let (hud_claims, _lease) = attach_hud_visibility(&host, snap.health);

    let tokens = [
        "player",
        "raid_list",
        "quest_tracker",
        "dps_meter",
        "breath",
        "spell_bars",
        "utility_bars",
    ];
    for tok in tokens {
        hud_layout::set_hud_state(true, true, tok == "raid_list", 93, 0);
        // Negative control: visible before hiding, hidden_by empty.
        publish_claims(&hud_claims.borrow());
        let before: (bool, usize) = host
            .lua
            .load(format!(
                "local v = eqoa.hud.visible({tok:?}); local hb = assert(eqoa.hud.hidden_by({tok:?})); return v, #hb"
            ))
            .eval()
            .unwrap();
        assert!(before.0, "{tok}: negative control should start visible");
        assert_eq!(before.1, 0, "{tok}: negative control hidden_by should be empty");

        let holder: String = host
            .lua
            .load(format!(
                r#"
                local t, err = eqoa.hud.hide({tok:?})
                assert(t and not err, "hide failed for {tok}: " .. tostring(err and err.message))
                _G._tok = t
                local hb = assert(eqoa.hud.hidden_by({tok:?}))
                return hb[1] or ""
                "#
            ))
            .eval()
            .unwrap();
        publish_claims(&hud_claims.borrow());
        let vis_hidden: bool = host
            .lua
            .load(format!("return eqoa.hud.visible({tok:?})"))
            .eval()
            .unwrap();
        assert!(!vis_hidden, "{tok}: should be hidden after eqoa.hud.hide");
        assert!(!holder.is_empty(), "{tok}: hidden_by should name holder");

        let ok: bool = host
            .lua
            .load(r#"local ok, err = eqoa.hud.show(_G._tok); assert(ok and not err); return ok"#)
            .eval()
            .unwrap();
        publish_claims(&hud_claims.borrow());
        let (vis_back, hb_back): (bool, usize) = host
            .lua
            .load(format!(
                "local v = eqoa.hud.visible({tok:?}); local hb = assert(eqoa.hud.hidden_by({tok:?})); return v, #hb"
            ))
            .eval()
            .unwrap();
        assert!(ok && vis_back && hb_back == 0, "{tok}: show must restore visibility");
    }

    // "all" preserves its pre-A4 meaning: hides the original 14 HUD leaves, and must NOT hide
    // raid_list, quest_tracker, dps_meter, breath, or minimap.
    hud_layout::set_hud_state(true, true, true, 93, 0);
    host.lua
        .load(r#"_G._all_tok = assert(eqoa.hud.hide("all"))"#)
        .exec()
        .unwrap();
    publish_claims(&hud_claims.borrow());
    let (all_vis, health_vis, raid_vis, quest_vis, meter_vis, breath_vis, mini_vis): (
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
    ) = host
        .lua
        .load(
            r#"
            return eqoa.hud.visible("all"),
                   eqoa.hud.visible("health"),
                   eqoa.hud.visible("raid_list"),
                   eqoa.hud.visible("quest_tracker"),
                   eqoa.hud.visible("dps_meter"),
                   eqoa.hud.visible("breath"),
                   eqoa.hud.visible("minimap")
            "#,
        )
        .eval()
        .unwrap();
    let (raid_hb, spell_hb, util_hb): (usize, usize, usize) = host
        .lua
        .load(
            r#"
            return #assert(eqoa.hud.hidden_by("raid_list")),
                   #assert(eqoa.hud.hidden_by("spell_bars")),
                   #assert(eqoa.hud.hidden_by("utility_bars"))
            "#,
        )
        .eval()
        .unwrap();
    host.lua.load(r#"assert(eqoa.hud.show(_G._all_tok))"#).exec().unwrap();
    publish_claims(&hud_claims.borrow());
    assert!(!all_vis && !health_vis, "all must still hide its original 14 leaves");
    assert!(
        raid_vis && quest_vis && meter_vis && breath_vis && mini_vis,
        "all must NOT hide the 6 A4 pieces or minimap (eqoa/1 additive-only rule)"
    );
    assert_eq!(
        (raid_hb, spell_hb, util_hb),
        (0, 0, 0),
        "all must not register hidden_by claims on raid_list/spell_bars/utility_bars"
    );

    // "all_pieces" covers every HUD piece except minimap.
    host.lua
        .load(r#"_G._allp_tok = assert(eqoa.hud.hide("all_pieces"))"#)
        .exec()
        .unwrap();
    publish_claims(&hud_claims.borrow());
    let (allp_vis, raid_vis, quest_vis, meter_vis, breath_vis, spell_vis, util_vis, mini_vis): (
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
    ) = host
        .lua
        .load(
            r#"
            return eqoa.hud.visible("all_pieces"),
                   eqoa.hud.visible("raid_list"),
                   eqoa.hud.visible("quest_tracker"),
                   eqoa.hud.visible("dps_meter"),
                   eqoa.hud.visible("breath"),
                   eqoa.hud.visible("spell_bars"),
                   eqoa.hud.visible("utility_bars"),
                   eqoa.hud.visible("minimap")
            "#,
        )
        .eval()
        .unwrap();
    host.lua.load(r#"assert(eqoa.hud.show(_G._allp_tok))"#).exec().unwrap();
    publish_claims(&hud_claims.borrow());
    assert!(
        !allp_vis && !raid_vis && !quest_vis && !meter_vis && !breath_vis && !spell_vis && !util_vis,
        "all_pieces must hide every HUD piece except minimap"
    );
    assert!(mini_vis, "all_pieces must NOT hide minimap");

    // Unknown token refused with invalid_argument.
    let code: String = host
        .lua
        .load(r#"local t, e = eqoa.hud.hide("not_a_real_piece"); return e and e.code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(code, "invalid_argument");
    native_events::clear(&host);
}

#[test]
fn a4_every_hud_piece_drops_quads_bounds_and_clicks_when_hidden() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut ui) = crate::ui::tests::layer() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    reset_a4_state();

    let quad_of = |r: [f32; 4]| render::ui::Quad {
        tex: 0,
        blend: 3,
        pos: [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]],
        uv: [[0.0; 2]; 4],
        color: [1.0; 4],
        clip: None,
    };

    // Part A: Every Piece (all 13, including Group as plain group AND as raid list) drops its
    // drawn quads and Move HUD bounds through hudlayout::begin/end (and seal() for SpellBars / UtilityBars).
    let cases: &[(Piece, &str, bool, bool)] = &[
        (Piece::Player, "player", false, false),
        (Piece::Group, "group_panel", false, false),
        (Piece::Group, "raid_list", true, false),
        (Piece::Pet, "pet", false, false),
        (Piece::Target, "target", false, false),
        (Piece::Minimap, "minimap", false, false),
        (Piece::Quests, "quest_tracker", false, false),
        (Piece::Meter, "dps_meter", false, false),
        (Piece::Effects, "effects", false, false),
        (Piece::Chat, "chat", false, false),
        (Piece::Breath, "breath", false, false),
        (Piece::QuickMenus, "hotbar", false, false),
        (Piece::SpellBars, "spell_bars", false, true),
        (Piece::UtilityBars, "utility_bars", false, true),
    ];

    let visible_quads = |ui: &render::ui::Ui| -> usize {
        ui.quads
            .iter()
            .filter(|q| {
                q.color[3] > 0.0
                    && q.clip
                        .map_or(true, |[x0, y0, x1, y1]| x1 > x0 && y1 > y0)
            })
            .count()
    };

    for &(piece, token, in_raid, uses_seal) in cases {
        hud_layout::set_hud_state(true, true, in_raid, 93, 0);

        // Negative control: unhidden -> quads stay visible, Move HUD notes the piece.
        publish_claims(&HudClaimsRegistry::new());
        crate::hudlayout::frame_begin();
        ui.begin((640, 448));
        ui.anchor = render::ui::Anchor::Left;
        ui.vertical = render::ui::Vertical::Top;
        let mut m = crate::hudlayout::begin(&mut ui, piece);
        ui.push(quad_of([20.0, 20.0, 120.0, 60.0]));
        if uses_seal {
            m = crate::hudlayout::seal(&mut ui, m);
            // Simulate draw_name emitted after seal().
            ui.push(quad_of([20.0, 62.0, 100.0, 76.0]));
        }
        crate::hudlayout::end(&mut ui, m);
        ui.end();
        crate::hudlayout::frame_end();
        assert_eq!(
            visible_quads(&ui),
            if uses_seal { 2 } else { 1 },
            "{piece:?} ({token}): negative control must keep drawn quads"
        );
        assert!(
            crate::hudlayout::drawn(piece).is_some(),
            "{piece:?} ({token}): negative control must note Move HUD bounds"
        );

        // Hidden via token: quads blanked in place (0 visible quads, including post-seal draw_name)
        // and Move HUD not noted.
        let mut reg = HudClaimsRegistry::new();
        let owner = InstanceToken::next().unwrap();
        let mut ok = false;
        for leaf in visibility::native_leaves(token).unwrap() {
            reg.disable(leaf, owner, "test_hider");
            ok = true;
        }
        assert!(ok, "token {token} must resolve to leaves");
        publish_claims(&reg);

        crate::hudlayout::frame_begin();
        ui.begin((640, 448));
        ui.anchor = render::ui::Anchor::Left;
        ui.vertical = render::ui::Vertical::Top;
        let mut m = crate::hudlayout::begin(&mut ui, piece);
        ui.push(quad_of([20.0, 20.0, 120.0, 60.0]));
        if uses_seal {
            m = crate::hudlayout::seal(&mut ui, m);
            ui.push(quad_of([20.0, 62.0, 100.0, 76.0]));
        }
        crate::hudlayout::end(&mut ui, m);
        ui.end();
        crate::hudlayout::frame_end();
        assert_eq!(
            visible_quads(&ui),
            0,
            "{piece:?} ({token}): hidden piece must leave 0 visible quads"
        );
        assert!(
            crate::hudlayout::drawn(piece).is_none(),
            "{piece:?} ({token}): hidden piece must not register Move HUD bounds via end/seal"
        );

        // Cross-check Group vs Raid independence.
        if piece == Piece::Group {
            hud_layout::set_hud_state(true, true, !in_raid, 93, 0);
            ui.begin((640, 448));
            let m = crate::hudlayout::begin(&mut ui, Piece::Group);
            ui.push(quad_of([20.0, 20.0, 120.0, 60.0]));
            crate::hudlayout::end(&mut ui, m);
            ui.end();
            assert_eq!(
                visible_quads(&ui),
                1,
                "hiding {token} must not hide the sibling group/raid mode"
            );
        }
    }

    // Part B: End-to-end Hud::draw click areas (Hud::pick) for every Pick variant.
    publish_claims(&HudClaimsRegistry::new());
    let vui = test_vui_with_atlas();
    let assets = crate::ui::UiAssets::default();

    let make_state = |raid: bool| HudState {
        has_self: true,
        self_hp: 200,
        power: 50,
        max_power: 100,
        level: 10,
        exp: 100,
        target_name: Some("a goblin".into()),
        target_hp: 150,
        pet: Some(PetState {
            name: "a wolf".into(),
            creature: Some((200, false, false)),
            effects: vec![1, 2],
            power: Some(120),
            ..Default::default()
        }),
        group_count: if raid { 0 } else { 2 },
        group: if raid {
            vec![]
        } else {
            vec![MemberState {
                id: 77,
                name: "Ally".into(),
                hp: Some((220, false, false)),
                ..Default::default()
            }]
        },
        raid: if raid {
            vec![crate::raidlist::Row {
                id: 88,
                name: "Raider".into(),
                hp: 200,
                power: Some(180),
                is_target: false,
                seen: true,
                me: false,
                marks: crate::raidlist::LEADER,
                class: 0,
                group: 1,
                dragged: false,
                merc: false,
            }]
        } else {
            vec![]
        },
        minimap: true,
        tracker: vec![net::session::TrackedQuest {
            name: "Slay Rats".into(),
            goals: vec![],
            ability: false,
        }],
        meter: Some(crate::dpsmeter::Shown {
            view: crate::dpsmeter::View::Fight,
            title: "DPS".into(),
            secs: 20,
            dps: 500.0,
            rows: (0..10)
                .map(|i| crate::dpsmeter::Row {
                    rank: i + 1,
                    name: format!("P{i}"),
                    class: 0,
                    damage: 1000 - i as u64 * 10,
                    pet: 0,
                    dps: 50.0,
                    share: 0.5,
                    bar: 0.5,
                    own: i == 0,
                    left: false,
                })
                .collect(),
            pet: None,
            first: 0,
        }),
        ..Default::default()
    };

    // Negative control (unhidden, plain group): every pick hits.
    let mut hud = Hud::new();
    let s_group = make_state(false);
    ui.begin((640, 448));
    hud.draw(&vui, &mut assets.raster(&mut ui), &s_group);
    let me_pt = [50.0, 40.0];
    let member_pt = [60.0, 110.0];
    let pet_pt = [60.0, 155.0];
    let mini_f = crate::map::mini::drawn(crate::settings::minimap_scroll());
    let mini_pt = [
        (mini_f[0] + mini_f[2]) as f32 * 0.5,
        (mini_f[1] + mini_f[3]) as f32 * 0.5,
    ];
    let meter_shown = s_group.meter.as_ref().unwrap();
    let col = crate::tracker::COLUMN;
    let meter_panel = crate::dpsmeter::panel(meter_shown, (col[3] - col[1]) / 2);
    let meter_view_pt = [
        (meter_panel.view[0] + meter_panel.view[2]) as f32 * 0.5,
        (meter_panel.view[1] + meter_panel.view[3]) as f32 * 0.5,
    ];
    assert_eq!(hud.pick(me_pt), Some(Pick::Me), "negative control: player click");
    assert_eq!(hud.pick(member_pt), Some(Pick::Member(77)), "negative control: group click");
    assert_eq!(hud.pick(pet_pt), Some(Pick::Pet), "negative control: pet click");
    assert_eq!(hud.pick(mini_pt), Some(Pick::Minimap), "negative control: minimap click");
    assert!(
        matches!(hud.pick(meter_view_pt), Some(Pick::Meter(_))),
        "negative control: meter click"
    );

    // Hide all_pieces + minimap -> every click area is gone, while Pick::Report stays unaffected.
    let mut reg = HudClaimsRegistry::new();
    let owner = InstanceToken::next().unwrap();
    for tok in ["all_pieces", "minimap"] {
        for leaf in visibility::native_leaves(tok).unwrap() {
            reg.disable(leaf, owner, "hide_everything");
        }
    }
    publish_claims(&reg);
    ui.begin((640, 448));
    hud.draw(&vui, &mut assets.raster(&mut ui), &s_group);
    assert_eq!(hud.pick(me_pt), None, "hidden player must have no click area");
    assert_eq!(hud.pick(member_pt), None, "hidden group must have no click area");
    assert_eq!(hud.pick(pet_pt), None, "hidden pet must have no click area");
    assert_eq!(hud.pick(mini_pt), None, "hidden minimap must have no click area");
    assert_eq!(hud.pick(meter_view_pt), None, "hidden meter must have no click area");
    assert!(
        !crate::hud::fork_hud::skip_pick(Pick::Report),
        "Report button must never be suppressed"
    );

    // Raid list click area: visible hits Pick::Raid(88), hidden returns None.
    publish_claims(&HudClaimsRegistry::new());
    let s_raid = make_state(true);
    ui.begin((640, 448));
    hud.draw(&vui, &mut assets.raster(&mut ui), &s_raid);
    let raid_pt = [60.0, 114.0];
    assert!(
        matches!(hud.pick(raid_pt), Some(Pick::Raid(88) | Pick::RaidGroup(1))),
        "negative control: raid list click at {raid_pt:?}, got {:?}",
        hud.pick(raid_pt)
    );
    let mut raid_reg = HudClaimsRegistry::new();
    raid_reg.disable("raid_list", owner, "raid_mod");
    publish_claims(&raid_reg);
    ui.begin((640, 448));
    hud.draw(&vui, &mut assets.raster(&mut ui), &s_raid);
    assert_eq!(hud.pick(raid_pt), None, "hidden raid list must have no click area");

    // UtilityBars vs SpellBars independent click/visibility check.
    let mut menus = crate::hudmenu::HudMenus::default();
    menus.stacked = true;
    publish_claims(&HudClaimsRegistry::new());
    assert!(crate::hud::fork_hud::hud_menu_visible(&menus));
    assert!(crate::hud::fork_hud::utility_bar_visible());

    let mut sp_reg = HudClaimsRegistry::new();
    sp_reg.disable("spell_bars", owner, "bars_mod");
    publish_claims(&sp_reg);
    assert!(!crate::hud::fork_hud::hud_menu_visible(&menus), "spell_bars hides stacked spell bars");
    assert!(crate::hud::fork_hud::utility_bar_visible(), "spell_bars leaves utility_bars visible");

    let mut ut_reg = HudClaimsRegistry::new();
    ut_reg.disable("utility_bars", owner, "bars_mod");
    publish_claims(&ut_reg);
    assert!(crate::hud::fork_hud::hud_menu_visible(&menus), "utility_bars leaves stacked spell bars visible");
    assert!(!crate::hud::fork_hud::utility_bar_visible(), "utility_bars hides utility bars");
    assert_eq!(
        crate::hud::fork_hud::utility_point(&mut menus, [50.0, 50.0]),
        None,
        "hidden utility bars take no clicks"
    );

    publish_claims(&HudClaimsRegistry::new());
}

// =========================================================================
// 2. Wheel captures (raid_wheel & meter_wheel)
// =========================================================================

#[test]
fn a4_wheel_captures_stop_when_raid_list_or_dps_meter_hidden() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut ui) = crate::ui::tests::layer() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    let mut p = ready_play();
    let vui = test_vui_with_atlas();
    let assets = crate::ui::UiAssets::default();

    // Build a raid list tall enough to require scrolling (`raid_more > 0`)
    // and a DPS meter with enough rows to require scrolling (`room > 0`).
    let raid_rows: Vec<crate::raidlist::Row> = (1..=80u32)
        .map(|i| crate::raidlist::Row {
            id: i,
            name: format!("Raider{i}"),
            hp: 200,
            power: Some(150),
            is_target: false,
            seen: true,
            me: false,
            marks: 0,
            class: 0,
            group: 0,
            dragged: false,
            merc: false,
        })
        .collect();
    let meter_rows: Vec<crate::dpsmeter::Row> = (1..=20u32)
        .map(|i| crate::dpsmeter::Row {
            rank: i as usize,
            name: format!("Dps{i}"),
            class: 0,
            damage: 5000 - i as u64 * 100,
            pet: 0,
            dps: 100.0,
            share: 0.5,
            bar: 0.5,
            own: i == 1,
            left: false,
        })
        .collect();
    let s = HudState {
        raid: raid_rows,
        meter: Some(crate::dpsmeter::Shown {
            view: crate::dpsmeter::View::Fight,
            title: "DPS".into(),
            secs: 50,
            dps: 1000.0,
            rows: meter_rows,
            pet: None,
            first: 0,
        }),
        ..Default::default()
    };

    // Draw once into play's Hud so raid_panel and meter_panel are populated.
    let mut hud = Hud::new();
    ui.begin((640, 448));
    hud.draw(&vui, &mut assets.raster(&mut ui), &s);

    let raid_pt = [50.0, 130.0];
    let col = crate::tracker::COLUMN;
    let mp = crate::dpsmeter::panel(s.meter.as_ref().unwrap(), col[3] - col[1]);
    let meter_win = (
        ((mp.rect[0] + mp.rect[2]) as f64) * 0.5,
        ((mp.rect[1] + mp.rect[3]) as f64) * 0.5,
    );

    // Negative control: unhidden -> raid_wheel scrolls!
    assert!(
        hud.raid_wheel(raid_pt, -1.0),
        "negative control: visible raid list must capture wheel"
    );
    // Hide raid_list -> raid_wheel returns false (does not capture).
    let mut reg = HudClaimsRegistry::new();
    let owner = InstanceToken::next().unwrap();
    reg.disable("raid_list", owner, "raid_mod");
    publish_claims(&reg);
    assert!(
        !hud.raid_wheel(raid_pt, -1.0),
        "hidden raid list must not capture wheel"
    );

    // Put hud into Play and test Play::meter_wheel.
    publish_claims(&HudClaimsRegistry::new());
    p.mode = 0;
    p.meter.first = 0;
    p.hud = Some((vui, assets, hud));

    // Negative control: unhidden -> meter_wheel returns true and advances meter.first.
    assert!(
        p.meter_wheel(meter_win, (640, 448), -1.0),
        "negative control: visible DPS meter must capture wheel"
    );
    assert_eq!(p.meter.first, 1, "negative control: meter scrolled to row 1");

    // Hide dps_meter -> meter_wheel returns false and meter.first does not move.
    let mut mreg = HudClaimsRegistry::new();
    mreg.disable("dps_meter", owner, "meter_mod");
    publish_claims(&mreg);
    assert!(
        !p.meter_wheel(meter_win, (640, 448), -1.0),
        "hidden DPS meter must not capture wheel"
    );
    assert_eq!(p.meter.first, 1, "hidden DPS meter must not change scroll offset");

    // Release -> captures again.
    publish_claims(&HudClaimsRegistry::new());
    assert!(
        p.meter_wheel(meter_win, (640, 448), -1.0),
        "released DPS meter must capture wheel again"
    );
    assert_eq!(p.meter.first, 2);
}

// =========================================================================
// 3. Raid strips (seams 7-10), timers, takeover rules, eqoa.raid & actions
// =========================================================================

#[test]
fn a4_raid_strips_takeover_suppression_timers_and_recovery() {
    let mut p = ready_play();
    let vui = test_vui_with_atlas();

    // Set up a ready check and a loot roll (each 60s so neither expires before we advance `ends`).
    p.ready_check(ReadyCheck {
        id: 101,
        asker: "Leader".into(),
        seconds: 60,
    });
    let drop_item = sample_net_item(9001, "Dragon Scale", 55);
    p.raid_roll(Roll {
        id: 202,
        seconds: 60,
        may_need: true,
        item: Some(Box::new(drop_item.clone())),
    });
    assert!(p.raid.ready.is_open() && p.raid.rolls.is_open());

    // Negative control (unclaimed): run_rolls draws, rolls_up is true, roll_key & roll_point respond.
    let mut r_unclaimed = CountingRaster::default();
    p.run_rolls(&vui, &mut r_unclaimed);
    assert!(
        r_unclaimed.total() > 0,
        "negative control: unclaimed strips must draw"
    );
    assert!(p.rolls_up(), "negative control: unclaimed rolls_up is true");
    // Right arrow focuses ready check strip.
    assert!(
        p.roll_key(18, true),
        "negative control: unclaimed roll_key takes focus"
    );
    assert!(p.raid.ready.focused());
    p.roll_let_go();

    // Takeover rules: scope="own" refused, missing replaces refused, scope="all" with replaces succeeds,
    // second holder gets busy.
    let h_bad = host_with("bad_strip_mod", &["eqoa.windows.takeover"], &[]);
    let (own_code, no_rep_code): (String, String) = h_bad
        .lua
        .load(
            r#"
            local _, e1 = eqoa.windows.takeover("raid_ready", { scope = "own" })
            local _, e2 = eqoa.windows.takeover("raid_ready", { scope = "all" })
            return e1 and e1.code or "", e2 and e2.code or ""
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(own_code, "invalid_argument", "strips reject scope='own'");
    assert_eq!(no_rep_code, "not_declared", "strips require replaces declaration");

    let h_ready = host_with("ready_mod", &["eqoa.windows.takeover"], &["raid_ready"]);
    let h_roll = host_with("roll_mod", &["eqoa.windows.takeover"], &["raid_roll"]);
    let h_rival = host_with("rival_mod", &["eqoa.windows.takeover"], &["raid_ready"]);

    // Claim ONLY raid_ready first: ready check suppressed, loot roll still draws and takes clicks!
    let ok: bool = h_ready
        .lua
        .load(r#"return assert(eqoa.windows.takeover("raid_ready", { scope = "all" }))"#)
        .eval()
        .unwrap();
    assert!(ok);

    // Rival cannot claim raid_ready while ready_mod holds it.
    let busy_code: String = h_rival
        .lua
        .load(r#"local _, e = eqoa.windows.takeover("raid_ready", { scope = "all" }); return e and e.code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(busy_code, "busy");

    // Verify eqoa.windows.taken() reports raid_ready = true, raid_roll = false, and holder id.
    let (t_ready, t_roll, holder): (bool, bool, String) = h_ready
        .lua
        .load(
            r#"
            local t = eqoa.windows.taken()
            return t.raid_ready, t.raid_roll, t.holders.raid_ready or ""
            "#,
        )
        .eval()
        .unwrap();
    assert!(t_ready && !t_roll && holder == "ready_mod");

    let mut r_half = CountingRaster::default();
    p.run_rolls(&vui, &mut r_half);
    assert!(r_half.total() > 0, "unclaimed loot roll strip still draws");
    assert!(p.rolls_up(), "rolls_up still true because loot roll is unclaimed");

    // Claim raid_roll too -> BOTH strips suppressed!
    let ok2: bool = h_roll
        .lua
        .load(r#"return assert(eqoa.windows.takeover("raid_roll", { scope = "all" }))"#)
        .eval()
        .unwrap();
    assert!(ok2);

    let mut r_both = CountingRaster::default();
    p.run_rolls(&vui, &mut r_both);
    assert_eq!(
        r_both.total(),
        0,
        "when both raid_ready and raid_roll are taken, run_rolls must emit 0 draw calls"
    );
    assert!(!p.rolls_up(), "rolls_up must be false when all open strips are taken");

    // Keys and clicks must do nothing and send nothing while taken.
    p.session.record.as_mut().unwrap().clear();
    assert!(!p.roll_key(18, true), "taken strips take no focus key");
    assert!(!p.roll_key(13, true), "taken strips take no confirm key");
    assert!(!p.roll_point([320.0, 200.0]), "taken ready strip takes no clicks");
    assert!(!p.roll_point([320.0, 260.0]), "taken roll strip takes no clicks");
    assert!(
        p.session.record.as_ref().unwrap().is_empty(),
        "no wire packets sent while strips are taken"
    );

    // Timers STILL RUN while hidden: move deadlines past ends + 3000 ms relative to session.now() and run_rolls.
    let now = p.session.now() as i64;
    p.raid.ready.open.as_mut().unwrap().ends = now - 5000;
    p.raid.rolls.open[0].ends = now - 5000;
    let mut r_expired = CountingRaster::default();
    p.run_rolls(&vui, &mut r_expired);
    assert_eq!(r_expired.total(), 0);
    assert!(
        !p.raid.ready.is_open(),
        "ready check timer must still expire while hidden"
    );
    assert!(
        !p.raid.rolls.is_open(),
        "loot roll timer must still expire while hidden"
    );

    // Re-open both strips, then simulate holder disappearing (VM cleared / disabled):
    // native strips must draw and take keys again immediately!
    p.ready_check(ReadyCheck {
        id: 102,
        asker: "Leader".into(),
        seconds: 60,
    });
    p.raid_roll(Roll {
        id: 203,
        seconds: 60,
        may_need: true,
        item: Some(Box::new(drop_item.clone())),
    });
    // Holder goes away (disabled/errored -> native_events::clear calls window_takeover::clear_vm).
    native_events::clear(&h_ready);
    native_events::clear(&h_roll);
    assert!(!window_takeover::is_taken(TakeoverKind::RaidReady));
    assert!(!window_takeover::is_taken(TakeoverKind::RaidRoll));

    let mut r_back = CountingRaster::default();
    p.run_rolls(&vui, &mut r_back);
    assert!(
        r_back.total() > 0,
        "after holders go away, native strips must draw again"
    );
    assert!(p.rolls_up());
    assert!(p.roll_key(18, true), "after holders go away, native roll_key works again");
    p.roll_let_go();

    // Player's "Use the game's windows" emergency switch releases strip claims and refuses new ones.
    let h_sw = host_with("switch_mod", &["eqoa.windows.takeover"], &["raid_ready"]);
    assert!(h_sw
        .lua
        .load(r#"return assert(eqoa.windows.takeover("raid_ready", { scope = "all" }))"#)
        .eval::<bool>()
        .unwrap());
    assert!(window_takeover::is_taken(TakeoverKind::RaidReady));
    window_takeover::set_use_game_windows(true);
    assert!(!window_takeover::is_taken(TakeoverKind::RaidReady));
    let unavail: String = h_sw
        .lua
        .load(r#"local _, e = eqoa.windows.takeover("raid_ready", { scope = "all" }); return e and e.code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(unavail, "unavailable");
    window_takeover::set_use_game_windows(false);

    native_events::clear(&h_bad);
    native_events::clear(&h_rival);
    native_events::clear(&h_sw);
}

#[test]
fn a4_raid_reads_and_ready_roll_actions_match_native_wire_bytes() {
    let mut p = ready_play();
    p.raid_state(RaidState {
        leader: 10,
        helper: 11,
        assist: 12,
        rule: net::raid::RULE_MASTER_LOOTER,
        flags: 0x01 | 0x02, // in_raid + leads (bit 0 = in_combat, bit 1 = leads)
    });
    p.group.raid = vec![
        RaidMember {
            id: 10,
            name: "Lead".into(),
            class: 0,
            level: 50,
            hp: 255,
            power: 128,
            group: 1,
            merc: false,
        },
        RaidMember {
            id: 12,
            name: "Tank".into(),
            class: 0,
            level: 50,
            hp: 128,
            power: 64,
            group: 2,
            merc: false,
        },
    ];
    p.ready_check(ReadyCheck {
        id: 501,
        asker: "Lead".into(),
        seconds: 20,
    });
    p.raid_roll(Roll {
        id: 601,
        seconds: 30,
        may_need: true,
        item: Some(Box::new(sample_net_item(777, "Aegis", 12))),
    });
    p.raid_roll(Roll {
        id: 602,
        seconds: 30,
        may_need: false,
        item: Some(Box::new(sample_net_item(778, "Cloth Hat", 13))),
    });
    publish_play(&mut p);

    // Negative control: mod without capability is refused at call time.
    let h_nocap = host_with("nocap_raid", &[], &[]);
    let (c1, c2): (String, String) = h_nocap
        .lua
        .load(
            r#"
            local _, e1 = eqoa.actions.ready_check_answer(501, true)
            local _, e2 = eqoa.actions.loot_roll(601, "need")
            return e1.code, e2.code
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!((c1.as_str(), c2.as_str()), ("not_declared", "not_declared"));

    // Read raid state + session-shaped ready_check and loot_rolls.
    let h = host_with(
        "raid_ui",
        &["eqoa.actions.ready_check_answer", "eqoa.actions.loot_roll"],
        &[],
    );
    let (in_raid, rule, members_n, rc_kind, rc_origin, rc_id, roll_kind, roll_n, no_need_choices): (
        bool,
        String,
        usize,
        String,
        String,
        u32,
        String,
        usize,
        usize,
    ) = h
        .lua
        .load(
            r#"
            local r = assert(eqoa.raid.get())
            local rc = assert(eqoa.raid.ready_check())
            local rl = assert(eqoa.raid.loot_rolls())
            assert(r.members[1].leader == true and r.members[2].assist == true)
            assert(rc.revision >= 1 and rl.revision >= 1)
            return r.in_raid, r.loot_rule, #r.members,
                   rc.kind, rc.origin, rc.data.id,
                   rl.kind, #rl.data.rolls, #rl.data.rolls[2].choices
            "#,
        )
        .eval()
        .unwrap();
    assert!(in_raid);
    assert_eq!(rule, "master_looter");
    assert_eq!(members_n, 2);
    assert_eq!(rc_kind, "raid_ready");
    assert_eq!(rc_origin, "server");
    assert_eq!(rc_id, 501);
    assert_eq!(roll_kind, "raid_roll");
    assert_eq!(roll_n, 2);
    assert_eq!(no_need_choices, 2, "may_need=false offers only greed & pass");

    // Negative control: Need on may_need=false roll is refused with not_ready, sends nothing.
    p.session.record.as_mut().unwrap().clear();
    h.lua
        .load(r#"req_noneed = assert(eqoa.actions.loot_roll(602, "need"))"#)
        .exec()
        .unwrap();
    step_actions(&mut p);
    let (st, code): (String, String) = h
        .lua
        .load(r#"local s = eqoa.actions.status(req_noneed); return s.state, s.code or "" "#)
        .eval()
        .unwrap();
    assert_eq!((st.as_str(), code.as_str()), ("refused", "not_ready"));
    assert!(p.session.record.as_ref().unwrap().is_empty());

    // Negative control: bad choice string refused immediately with invalid_argument.
    let bad_choice: String = h
        .lua
        .load(r#"local _, e = eqoa.actions.loot_roll(601, "steal"); return e.code"#)
        .eval()
        .unwrap();
    assert_eq!(bad_choice, "invalid_argument");

    // Positive control: answer ready check (true) and loot roll (need) -> exact wire bytes!
    h.lua
        .load(
            r#"
            req_rc = assert(eqoa.actions.ready_check_answer(501, true))
            req_rl = assert(eqoa.actions.loot_roll(601, "need"))
            "#,
        )
        .exec()
        .unwrap();
    step_actions(&mut p);

    let (st_rc, st_rl): (String, String) = h
        .lua
        .load(r#"return eqoa.actions.status(req_rc).state, eqoa.actions.status(req_rl).state"#)
        .eval()
        .unwrap();
    assert_eq!((st_rc.as_str(), st_rl.as_str()), ("completed", "completed"));
    assert!(!p.raid.ready.is_open(), "answering ready check clears strip");
    assert_eq!(p.raid.rolls.open.len(), 1, "answering roll 601 removes it");

    let sent = p.session.record.as_ref().unwrap();
    assert_eq!(sent.len(), 2, "two wire packets sent: {sent:?}");
    assert_eq!(sent[0].1, net::readycheck::reply(501, true), "ready check reply wire bytes");
    assert_eq!(sent[1].1, net::raid::roll_choice(601, Choice::Need), "loot roll choice wire bytes");

    // Negative control: answering already-closed ready check returns not_found.
    h.lua
        .load(r#"req_gone = assert(eqoa.actions.ready_check_answer(501, false))"#)
        .exec()
        .unwrap();
    step_actions(&mut p);
    let gone_code: String = h
        .lua
        .load(r#"return eqoa.actions.status(req_gone).code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(gone_code, "not_found");

    native_events::clear(&h_nocap);
    native_events::clear(&h);
}

// =========================================================================
// 4. Master-loot Container.master_loot read & loot_assign / loot_abandon
// =========================================================================

#[test]
fn a4_master_loot_read_actions_and_refusals() {
    let mut p = ready_play();
    let inv_item = sample_net_item(100, "Ration", 5);
    p.dump.inventory = ItemList {
        p04: 0,
        items: vec![inv_item],
    };
    publish_play(&mut p);

    let h_nocap = host_with("ml_nocap", &[], &[]);
    let h = host_with(
        "ml_mod",
        &["eqoa.actions.loot_assign", "eqoa.actions.loot_abandon"],
        &[],
    );

    // Negative control 1: no capability -> not_declared.
    let nc: String = h_nocap
        .lua
        .load(r#"local _, e = eqoa.actions.loot_abandon(); return e.code"#)
        .eval()
        .unwrap();
    assert_eq!(nc, "not_declared");

    // Negative control 2: no corpse open -> window_closed.
    h.lua
        .load(r#"r_closed = assert(eqoa.actions.loot_abandon())"#)
        .exec()
        .unwrap();
    step_actions(&mut p);
    let code_closed: String = h
        .lua
        .load(r#"return eqoa.actions.status(r_closed).code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(code_closed, "window_closed");

    // Open corpse WITHOUT master loot -> loot_abandon & loot_assign refused with permission_denied.
    let drop_item = sample_net_item(5000, "Boss Sword", 42);
    let payload = sample_corpse_payload(&[drop_item]);
    assert!(p.npcw_opcode(0x0018, &payload));
    publish_play(&mut p);

    // Also test non-loot ItemRef (from inventory) refused with invalid_argument.
    let non_loot_code: String = h
        .lua
        .load(
            r#"
            local inv = assert(eqoa.inventory.get())
            local _, e = eqoa.actions.loot_assign(inv.items[1].ref, 10)
            return e and e.code or ""
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(non_loot_code, "invalid_argument");

    h.lua
        .load(
            r#"
            local loot = assert(eqoa.loot.get())
            assert(loot.master_loot == nil, "no master loot before 0x7E47")
            r_noml_assign = assert(eqoa.actions.loot_assign(loot.items[1].ref, 10))
            "#,
        )
        .exec()
        .unwrap();
    step_actions(&mut p);
    h.lua
        .load(r#"r_noml_abandon = assert(eqoa.actions.loot_abandon())"#)
        .exec()
        .unwrap();
    step_actions(&mut p);
    let (c_a, c_b): (String, String) = h
        .lua
        .load(
            r#"
            return eqoa.actions.status(r_noml_assign).code or "",
                   eqoa.actions.status(r_noml_abandon).code or ""
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(
        (c_a.as_str(), c_b.as_str()),
        ("permission_denied", "permission_denied")
    );

    // Enable master loot via real Event::MasterLoot(Some(...)).
    let ml = MasterLoot {
        seconds: 120,
        members: vec![(10, "Leader".into()), (20, "Wizard".into())],
    };
    let ev_on = Event::MasterLoot(Some(ml));
    let cap_on = super::begin_game_event(&p, &ev_on);
    super::end_game_event(&p, cap_on);
    publish_play(&mut p);

    // Verify Container.master_loot is populated cleanly.
    let (secs, m_count, m2_id, m2_name): (u32, usize, u32, String) = h
        .lua
        .load(
            r#"
            local loot = assert(eqoa.loot.get())
            local ml = assert(loot.master_loot)
            return ml.seconds, #ml.members, ml.members[2].id, ml.members[2].name
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!((secs, m_count, m2_id, m2_name.as_str()), (120, 2, 20, "Wizard"));

    // Negative control 3: ineligible member id -> refused with not_found, sends nothing.
    p.session.record.as_mut().unwrap().clear();
    h.lua
        .load(
            r#"
            local loot = assert(eqoa.loot.get())
            r_inelig = assert(eqoa.actions.loot_assign(loot.items[1].ref, 999))
            "#,
        )
        .exec()
        .unwrap();
    step_actions(&mut p);
    let inelig_code: String = h
        .lua
        .load(r#"return eqoa.actions.status(r_inelig).code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(inelig_code, "not_found");
    assert!(p.session.record.as_ref().unwrap().is_empty());

    // Positive control: loot_assign to eligible member 20 -> sends Command::Assign { key: 42, id: 20 }!
    h.lua
        .load(
            r#"
            local loot = assert(eqoa.loot.get())
            saved_ref = loot.items[1].ref
            r_assign = assert(eqoa.actions.loot_assign(saved_ref, 20))
            "#,
        )
        .exec()
        .unwrap();
    step_actions(&mut p);
    let assign_state: String = h
        .lua
        .load(r#"return eqoa.actions.status(r_assign).state"#)
        .eval()
        .unwrap();
    assert_eq!(assign_state, "completed");
    // 2026-11-25: the game's window asks "Assign <item> to <name>?" first, so the client does: nothing is sent until the
    // player's own Yes (negative control: the old behaviour sent at once, with `record` holding the Assign here)
    assert!(p.session.record.as_ref().unwrap().is_empty(), "asking assigns nothing");
    let q = crate::fork_ext::addons::confirm::current_for_test().expect("the assign question is up");
    assert_eq!(q.text, "Assign Boss Sword to Wizard?");
    assert!(!q.yes, "starts on No");
    assert_eq!(q.act.kind(), "loot_assign");
    {
        use winit::keyboard::{Key, NamedKey};
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::ArrowRight)));
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::Enter)));
    }
    step_actions(&mut p);
    {
        let sent = p.session.record.as_ref().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1, RaidCmd::Assign { key: 42, id: 20 }.bytes());
    }

    // Positive control: loot_abandon -> sends Command::Abandon!
    p.session.record.as_mut().unwrap().clear();
    h.lua
        .load(r#"r_abandon = assert(eqoa.actions.loot_abandon())"#)
        .exec()
        .unwrap();
    step_actions(&mut p);
    let abandon_state: String = h
        .lua
        .load(r#"return eqoa.actions.status(r_abandon).state"#)
        .eval()
        .unwrap();
    assert_eq!(abandon_state, "completed");
    assert!(p.session.record.as_ref().unwrap().is_empty(), "asking abandons nothing");
    let q = crate::fork_ext::addons::confirm::current_for_test().expect("the abandon question is up");
    assert!(q.text.starts_with("Abandon this corpse's loot?") && !q.yes);
    // No sends nothing
    {
        use winit::keyboard::{Key, NamedKey};
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::Escape)));
    }
    step_actions(&mut p);
    assert!(p.session.record.as_ref().unwrap().is_empty(), "No abandons nothing");
    // Yes, but master loot is no longer ours by the time of the answer: nothing is sent
    h.lua.load(r#"r_abandon2 = assert(eqoa.actions.loot_abandon())"#).exec().unwrap();
    step_actions(&mut p);
    {
        use winit::keyboard::{Key, NamedKey};
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::ArrowRight)));
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::Enter)));
    }
    crate::fork_ext::addons::raid_strips::set_master_loot_detail(None);
    step_actions(&mut p);
    assert!(p.session.record.as_ref().unwrap().is_empty(), "a Yes after master loot ended sends nothing");
    crate::fork_ext::addons::raid_strips::set_master_loot_detail(Some(MasterLoot {
        seconds: 120,
        members: vec![(10, "Leader".into()), (20, "Wizard".into())],
    }));
    // Yes with master loot still ours: the Abandon goes out
    h.lua.load(r#"r_abandon3 = assert(eqoa.actions.loot_abandon())"#).exec().unwrap();
    step_actions(&mut p);
    {
        use winit::keyboard::{Key, NamedKey};
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::ArrowRight)));
        assert!(crate::fork_ext::addons::confirm::key(&Key::Named(NamedKey::Enter)));
    }
    step_actions(&mut p);
    {
        let sent = p.session.record.as_ref().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1, RaidCmd::Abandon.bytes());
    }

    // Negative control 4: after corpse closes, saved_ref is stale (stale_handle).
    assert!(p.npcw_opcode(0x0016, &[]));
    let ev_off = Event::MasterLoot(None);
    let cap_off = super::begin_game_event(&p, &ev_off);
    super::end_game_event(&p, cap_off);
    publish_play(&mut p);
    let stale_code: String = h
        .lua
        .load(r#"local _, e = eqoa.actions.loot_assign(saved_ref, 20); return e and e.code or "" "#)
        .eval()
        .unwrap();
    assert_eq!(stale_code, "stale_handle");

    native_events::clear(&h_nocap);
    native_events::clear(&h);
}

// =========================================================================
// 5. Bundled demo mod: eqoa_target_frame_demo
// =========================================================================

#[test]
fn a4_target_frame_demo_mod_off_by_default_and_replaces_target_when_enabled() {
    crate::fork_ext::addons::big_stack(|| {
        let bundled_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods");
        let found = discover(&bundled_root);
        let demo: DiscoveredAddon = found
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_target_frame_demo")
            .expect("eqoa_target_frame_demo must be discovered under bundled-mods/");

        // Negative control: OFF by default, discloses HUD replacement clearly.
        assert!(
            !demo.initially_enabled,
            "eqoa_target_frame_demo must be OFF by default"
        );
        assert_eq!(demo.api.as_deref(), Some("eqoa/1"));
        assert_eq!(demo.extensions.replaces, vec!["hud.target".to_string()]);
        let lines = crate::fork_ext::addons::runtime::disclosure_lines(&demo);
        assert!(
            lines.iter().any(|l| l.contains("Replaces HUD: Target frame")),
            "disclosure must name Target frame: {lines:?}"
        );

        let mut p = crate::play::Play::bundled_ui_fixture();
        let walker = crate::play::Play::bundled_ui_walker();
        reset_a4_state();

        // While disabled (not loaded), native target piece is untouched and visible.
        assert!(
            !visibility::effective_visibility().target,
            "negative control: disabled demo mod changes nothing"
        );

        // Enable in world: loads cleanly with zero diagnostics, hides "target", draws replacement.
        let host = Host::for_addon(&demo).unwrap();
        let (hud_claims, lease) = attach_hud_visibility(&host, Default::default());
        let entry = host.compile_addon_entry(&demo).unwrap();
        native_events::clear(&host);
        hud_layer::clear_all();

        let mut diag = Vec::new();
        for frame in 0..3 {
            let snap = p.forge_snapshot(Some(&walker)).managed();
            super::begin_iteration(&mut p, false);
            host.begin_iteration();
            host.snapshot.set(snap);
            lease.update(snap.health, true);
            host.frame_entry(&entry).unwrap();
            host.finish_frame(frame == 0).unwrap();
            host.end_iteration();
            publish_claims(&hud_claims.borrow());
            diag.extend(
                host.drain_logs()
                    .unwrap()
                    .into_iter()
                    .filter(|l| !l.starts_with("eqoa/1 [info] ") && !l.starts_with("eqoa/1 [debug] ")),
            );
        }
        assert!(diag.is_empty(), "demo mod must run without warnings/errors: {diag:?}");
        assert!(
            visibility::effective_visibility().target,
            "enabled demo mod must hide native target piece"
        );
        let holder: String = host
            .lua
            .load(r#"local hb = assert(eqoa.hud.hidden_by("target")); return hb[1] or "" "#)
            .eval()
            .unwrap();
        assert_eq!(holder, "eqoa_target_frame_demo");

        // Disabling (clearing lease & VM) restores native target visibility immediately.
        lease.clear();
        publish_claims(&hud_claims.borrow());
        native_events::clear(&host);
        assert!(
            !visibility::effective_visibility().target,
            "disabling demo mod must restore native target visibility"
        );
    });
}

// =========================================================================
// 6. Fix round 1 regressions: H1 widescreen/tall Ui::end safety & M2 bar layouts
// =========================================================================

#[test]
fn a4_hidden_pieces_preserve_ui_indices_on_widescreen_and_tall_windows() {
    let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut ui) = crate::ui::tests::layer() else {
        eprintln!("SKIP: no graphics adapter");
        return;
    };
    reset_a4_state();

    let quad_of = |r: [f32; 4], blend: u8| render::ui::Quad {
        tex: 0,
        blend,
        pos: [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]],
        uv: [[0.0; 2]; 4],
        color: [1.0; 4],
        clip: None,
    };

    let cases: &[(Piece, &str, bool, bool)] = &[
        (Piece::Player, "player", false, false),
        (Piece::Group, "group_panel", false, false),
        (Piece::Group, "raid_list", true, false),
        (Piece::Pet, "pet", false, false),
        (Piece::Target, "target", false, false),
        (Piece::Minimap, "minimap", false, false),
        (Piece::Quests, "quest_tracker", false, false),
        (Piece::Meter, "dps_meter", false, false),
        (Piece::Effects, "effects", false, false),
        (Piece::Chat, "chat", false, false),
        (Piece::Breath, "breath", false, false),
        (Piece::QuickMenus, "hotbar", false, false),
        (Piece::SpellBars, "spell_bars", false, true),
        (Piece::UtilityBars, "utility_bars", false, true),
    ];

    // Check both widescreen (16:9 -> side > 0) and tall portrait (top > 0).
    for (label, size) in [("16:9 widescreen", (1280u32, 720u32)), ("tall portrait", (480u32, 900u32))] {
        let side = render::ui::side(size);
        let top = render::ui::top(size);
        if label.starts_with("16:9") {
            assert!(side > 0.0, "16:9 window must have side > 0");
        } else {
            assert!(top > 0.0, "tall window must have top > 0");
        }

        // Negative control A: the old `ui.quads.truncate(from)` panics in `Ui::end()` because
        // `sided` / `risen` still hold indices >= quads.len().
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.begin(size);
            ui.anchor = render::ui::Anchor::Left;
            ui.vertical = render::ui::Vertical::Top;
            let from = ui.quads.len();
            ui.push(quad_of([20.0, 20.0, 120.0, 60.0], 3));
            ui.push(quad_of([20.0, 64.0, 120.0, 100.0], 3));
            ui.quads.truncate(from);
            ui.end();
        }))
        .is_err();
        assert!(
            panicked,
            "{label}: negative control — truncating ui.quads must panic in Ui::end()"
        );

        // Negative control B: truncating a middle piece and pushing a later anchored quad leaves
        // a stale index pointing at the later quad, so an opaque full-width backdrop double-shifts it.
        ui.begin(size);
        ui.anchor = render::ui::Anchor::Left;
        ui.vertical = render::ui::Vertical::Top;
        ui.push(quad_of([20.0, 20.0, 100.0, 60.0], 3)); // index 0: piece A
        ui.push(quad_of([20.0, 20.0, 100.0, 60.0], 3)); // index 1: piece B (visible baseline)
        ui.anchor = render::ui::Anchor::Middle;
        ui.vertical = render::ui::Vertical::Middle;
        ui.push(quad_of([0.0, 0.0, 640.0, 448.0], 0)); // full-width opaque backdrop
        ui.end();
        let baseline_b_pos = ui.quads[1].pos;

        ui.begin(size);
        ui.anchor = render::ui::Anchor::Left;
        ui.vertical = render::ui::Vertical::Top;
        let from_a = ui.quads.len();
        ui.push(quad_of([20.0, 20.0, 100.0, 60.0], 3)); // index 0 in sided/risen
        ui.quads.truncate(from_a); // buggy truncate leaves stale (0, d)
        ui.push(quad_of([20.0, 20.0, 100.0, 60.0], 3)); // now at index 0 -> gets TWO (0, d) entries!
        ui.anchor = render::ui::Anchor::Middle;
        ui.vertical = render::ui::Vertical::Middle;
        ui.push(quad_of([0.0, 0.0, 640.0, 448.0], 0));
        ui.end();
        assert_ne!(
            ui.quads[0].pos, baseline_b_pos,
            "{label}: negative control — truncate must corrupt/double-shift the later quad"
        );

        // Now verify our real `piece_hidden` fix for EVERY HUD piece in both Scenario A (last piece)
        // and Scenario B (middle piece between two visible pieces + full-width opaque backdrop).
        let owner = InstanceToken::next().unwrap();
        for &(piece, token, in_raid, uses_seal) in cases {
            hud_layout::set_hud_state(true, true, in_raid, 93, 0);

            // Baseline with nothing hidden (before + middle + after + opaque backdrop).
            publish_claims(&HudClaimsRegistry::new());
            ui.begin(size);
            ui.anchor = render::ui::Anchor::Left;
            ui.vertical = render::ui::Vertical::Top;
            ui.push(quad_of([10.0, 10.0, 80.0, 40.0], 3)); // idx 0: before
            let mut m = crate::hudlayout::begin(&mut ui, piece);
            let mid_start = ui.quads.len();
            ui.push(quad_of([20.0, 80.0, 140.0, 120.0], 3));
            if uses_seal {
                m = crate::hudlayout::seal(&mut ui, m);
                ui.push(quad_of([20.0, 122.0, 120.0, 136.0], 3));
            }
            crate::hudlayout::end(&mut ui, m);
            let mid_end = ui.quads.len();
            ui.anchor = render::ui::Anchor::Right;
            ui.vertical = render::ui::Vertical::Bottom;
            let after_idx = ui.quads.len();
            ui.push(quad_of([520.0, 380.0, 620.0, 420.0], 3)); // after
            ui.anchor = render::ui::Anchor::Middle;
            ui.vertical = render::ui::Vertical::Middle;
            ui.push(quad_of([0.0, 0.0, 640.0, 448.0], 0)); // opaque backdrop
            ui.end();
            let expect_before = (ui.quads[0].pos, ui.quads[0].clip);
            let expect_after = (ui.quads[after_idx].pos, ui.quads[after_idx].clip);

            // Hide `piece` and run Scenario B (middle piece + opaque backdrop).
            let mut reg = HudClaimsRegistry::new();
            for leaf in visibility::native_leaves(token).unwrap() {
                reg.disable(leaf, owner, "widescreen_test");
            }
            publish_claims(&reg);

            ui.begin(size);
            ui.anchor = render::ui::Anchor::Left;
            ui.vertical = render::ui::Vertical::Top;
            ui.push(quad_of([10.0, 10.0, 80.0, 40.0], 3)); // idx 0: before
            let mut m = crate::hudlayout::begin(&mut ui, piece);
            ui.push(quad_of([20.0, 80.0, 140.0, 120.0], 3));
            if uses_seal {
                m = crate::hudlayout::seal(&mut ui, m);
                ui.push(quad_of([20.0, 122.0, 120.0, 136.0], 3));
            }
            crate::hudlayout::end(&mut ui, m);
            ui.anchor = render::ui::Anchor::Right;
            ui.vertical = render::ui::Vertical::Bottom;
            ui.push(quad_of([520.0, 380.0, 620.0, 420.0], 3)); // after
            ui.anchor = render::ui::Anchor::Middle;
            ui.vertical = render::ui::Vertical::Middle;
            ui.push(quad_of([0.0, 0.0, 640.0, 448.0], 0));
            ui.end();

            assert_eq!(
                (ui.quads[0].pos, ui.quads[0].clip),
                expect_before,
                "{label} {piece:?}: before-piece quad must be unmoved"
            );
            assert_eq!(
                (ui.quads[after_idx].pos, ui.quads[after_idx].clip),
                expect_after,
                "{label} {piece:?}: after-piece quad must be unmoved (no double-shift)"
            );
            for q in &ui.quads[mid_start..mid_end] {
                assert_eq!(q.color[3], 0.0, "{label} {piece:?}: hidden quad alpha must be 0");
                let [x0, y0, x1, y1] = q.clip.expect("hidden quad must carry an empty clip");
                assert!(
                    x1 <= x0 || y1 <= y0,
                    "{label} {piece:?}: hidden quad clip must stay empty after Ui::end(), got {:?}",
                    q.clip
                );
            }

            // Scenario A: hidden piece is the LAST thing drawn before Ui::end() (no trailing quads).
            ui.begin(size);
            ui.anchor = render::ui::Anchor::Right;
            ui.vertical = render::ui::Vertical::Top;
            let mut m = crate::hudlayout::begin(&mut ui, piece);
            ui.push(quad_of([500.0, 20.0, 620.0, 90.0], 3));
            if uses_seal {
                m = crate::hudlayout::seal(&mut ui, m);
                ui.push(quad_of([500.0, 92.0, 600.0, 106.0], 3));
            }
            crate::hudlayout::end(&mut ui, m);
            ui.end(); // must NOT panic
            for q in &ui.quads {
                assert_eq!(q.color[3], 0.0, "{label} {piece:?} last: hidden quad alpha must be 0");
                let [x0, y0, x1, y1] = q.clip.expect("hidden quad must carry an empty clip");
                assert!(x1 <= x0 || y1 <= y0, "{label} {piece:?} last: clip must stay empty");
            }
        }
    }

    publish_claims(&HudClaimsRegistry::new());
}

#[test]
fn a4_spell_and_utility_bars_hide_draw_and_clicks_in_both_layouts() {
    reset_a4_state();
    let vui = test_vui_with_atlas();
    let owner = InstanceToken::next().unwrap();

    let mut menus = crate::hudmenu::HudMenus::default();
    let mut abilities = std::collections::BTreeMap::new();
    for slot in 0..5 {
        abilities.insert(
            slot + 1,
            net::dump::Ability {
                id: slot + 1,
                valid: 1,
                slot,
                icon_front: 10,
                ..Default::default()
            },
        );
    }
    let bars = crate::hudmenu::Bars {
        abilities: 2,
        gear: true,
    };
    menus.build(&abilities, &ItemList::default(), &Default::default(), bars);
    let no_name = |_: u32, _: i64| Vec::new();

    // --- Layout 1: Non-stacked (classic carousel) ---
    menus.stacked = false;
    let carousel_click_pt = |menus: &crate::hudmenu::HudMenus| -> [f32; 2] {
        let k = menus.sel.clamp(0, 5) as usize;
        // HudMenu::new places every carousel menu at [430, 25].
        let at = [430.0f32, 25.0f32];
        if matches!(k, 2 | 3) {
            [at[0] + 175.0, at[1] + 10.0]
        } else {
            [at[0] + 170.0, at[1] + 4.0]
        }
    };

    // Negative control (unhidden): all 6 carousel menus draw and accept clicks.
    publish_claims(&HudClaimsRegistry::new());
    for sel in [0, 1, 2, 3, 4, 5] {
        menus.sel = sel;
        let mut r = CountingRaster::default();
        crate::hud::fork_hud::draw_hud_menus(&mut menus, &vui, &mut r, 0, &no_name);
        assert!(r.total() > 0, "carousel sel={sel}: negative control must draw");
        assert!(
            crate::hud::fork_hud::hud_menu_visible(&menus),
            "carousel sel={sel}: negative control must be visible"
        );
        let pt = carousel_click_pt(&menus);
        assert!(
            menus.point(pt).is_some(),
            "carousel sel={sel}: negative control click must hit"
        );
    }

    // Hide spell_bars in carousel layout: menus 0, 1, 4, 5 hidden (draw=0, clicks gated off);
    // utility menus 2 (items) and 3 (gear) stay visible and clickable.
    let mut sp_reg = HudClaimsRegistry::new();
    sp_reg.disable("spell_bars", owner, "bars_mod");
    publish_claims(&sp_reg);
    for sel in [0, 1, 4, 5] {
        menus.sel = sel;
        let mut r = CountingRaster::default();
        crate::hud::fork_hud::draw_hud_menus(&mut menus, &vui, &mut r, 0, &no_name);
        assert_eq!(r.total(), 0, "carousel sel={sel}: spell_bars must suppress drawing");
        assert!(
            !crate::hud::fork_hud::hud_menu_visible(&menus),
            "carousel sel={sel}: spell_bars must gate clicks & right-click autocast off"
        );
    }
    for sel in [2, 3] {
        menus.sel = sel;
        let mut r = CountingRaster::default();
        crate::hud::fork_hud::draw_hud_menus(&mut menus, &vui, &mut r, 0, &no_name);
        assert!(r.total() > 0, "carousel sel={sel}: spell_bars must leave utility menu drawn");
        assert!(
            crate::hud::fork_hud::hud_menu_visible(&menus),
            "carousel sel={sel}: spell_bars must leave utility menu clickable"
        );
    }

    // Hide utility_bars in carousel layout: menus 2 and 3 hidden (draw=0, clicks gated off);
    // spell menus 0, 1, 4, 5 stay visible and clickable.
    let mut ut_reg = HudClaimsRegistry::new();
    ut_reg.disable("utility_bars", owner, "bars_mod");
    publish_claims(&ut_reg);
    for sel in [2, 3] {
        menus.sel = sel;
        let mut r = CountingRaster::default();
        crate::hud::fork_hud::draw_hud_menus(&mut menus, &vui, &mut r, 0, &no_name);
        assert_eq!(r.total(), 0, "carousel sel={sel}: utility_bars must suppress drawing");
        assert!(
            !crate::hud::fork_hud::hud_menu_visible(&menus),
            "carousel sel={sel}: utility_bars must gate clicks off"
        );
    }
    for sel in [0, 1, 4, 5] {
        menus.sel = sel;
        let mut r = CountingRaster::default();
        crate::hud::fork_hud::draw_hud_menus(&mut menus, &vui, &mut r, 0, &no_name);
        assert!(r.total() > 0, "carousel sel={sel}: utility_bars must leave spell menu drawn");
        assert!(
            crate::hud::fork_hud::hud_menu_visible(&menus),
            "carousel sel={sel}: utility_bars must leave spell menu clickable"
        );
    }

    // --- Layout 2: Stacked spell bars + utility bars ---
    menus.stacked = true;
    menus.sel = 0;
    publish_claims(&HudClaimsRegistry::new());
    let mut r_sp = CountingRaster::default();
    menus.draw_stacked(&vui, &mut r_sp, 350, 400, bars, crate::hudmenu::Layout::Stacked);
    let mut r_ut = CountingRaster::default();
    menus.draw_utility(
        &vui,
        &mut r_ut,
        bars,
        crate::hudmenu::UtilityLayout::UpperLeft,
        crate::hudmenu::UPPER_LEFT_AT,
    );
    assert!(r_sp.total() > 0 && r_ut.total() > 0, "stacked negative control draws both");
    let ut_pt = [
        crate::hudmenu::UPPER_LEFT_AT[0] as f32 + 10.0,
        crate::hudmenu::UPPER_LEFT_AT[1] as f32 + 10.0,
    ];
    assert!(
        crate::hud::fork_hud::utility_point(&mut menus, ut_pt).is_some(),
        "stacked negative control: utility_point must hit when visible"
    );

    // Hide spell_bars in stacked layout: spell bars gated off, utility bars still visible & clickable.
    publish_claims(&sp_reg);
    assert!(!crate::hud::fork_hud::hud_menu_visible(&menus));
    assert!(crate::hud::fork_hud::utility_bar_visible());
    assert!(crate::hud::fork_hud::utility_point(&mut menus, ut_pt).is_some());

    // Hide utility_bars in stacked layout: utility bars gated off & clicks return None, spell bars visible.
    publish_claims(&ut_reg);
    assert!(crate::hud::fork_hud::hud_menu_visible(&menus));
    assert!(!crate::hud::fork_hud::utility_bar_visible());
    assert_eq!(crate::hud::fork_hud::utility_point(&mut menus, ut_pt), None);

    publish_claims(&HudClaimsRegistry::new());
}
