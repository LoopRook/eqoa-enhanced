// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
// Upstream portions up to 1e23e80 remain MIT (NOTICE).
//! The combat indication adapter; service code only receives plain combat data.
use super::Play;

/// `tlog!("takeover: ...")`: an always-on takeover diagnostic (see `window_takeover::log`).
#[allow(unused_macros)]
macro_rules! tlog {
    ($($a:tt)*) => {
        crate::fork_ext::addons::window_takeover::log(format!($($a)*))
    };
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "api_capture.rs"]
mod api_capture;

#[cfg(not(target_arch = "wasm32"))]
#[path = "a1_play.rs"]
mod a1_play;

#[cfg(not(target_arch = "wasm32"))]
#[path = "a2_play.rs"]
mod a2_play;

#[cfg(not(target_arch = "wasm32"))]
#[path = "a8_play.rs"]
mod a8_play;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "map_mod_tests.rs"]
mod map_mod_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "api_capture_tests.rs"]
mod native_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "a4_hud_tests.rs"]
mod a4_hud_tests;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct CachedCycleView {
    eye: glam::Vec3,
    dir: glam::Vec3,
    vp: glam::Mat4,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
enum PendingNpcwTx {
    LootOne { key: u32, count: i32 },
    LootAll,
    Buy { ware_key: u32, count: i32 },
    Sell { item_key: u32, count: i32 },
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
enum PendingBankTx {
    Move { from_bank: bool, item_key: u32, count: i32 },
    Money { dir: u8, amount: i32 },
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
struct PendingCameraReq {
    mode: Option<i32>,
    heading: Option<f32>,
    pitch: Option<f32>,
    distance: Option<f32>,
}

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static CACHED_CYCLE_VIEW: std::cell::Cell<Option<CachedCycleView>> = const { std::cell::Cell::new(None) };
    static LOOT_BUSY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PENDING_NPCW_TX: std::cell::RefCell<Option<PendingNpcwTx>> = const { std::cell::RefCell::new(None) };
    static PENDING_BANK_TX: std::cell::RefCell<Option<PendingBankTx>> = const { std::cell::RefCell::new(None) };
    static PENDING_CAMERA: std::cell::Cell<Option<PendingCameraReq>> = const { std::cell::Cell::new(None) };
    /// `Play::mode` as the event now being applied found it (L4): a window already open natively
    /// before a packet is never taken over by that packet. -1 outside an event.
    static MODE_BEFORE_EVENT: std::cell::Cell<i32> = const { std::cell::Cell::new(-1) };
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum BrokerReply {
    Assist(u32),
    AbilitySlot { index: u8, slot: u8 },
    AbilityClearBar,
    AbilityMoveIndex { from: u8, to: u8 },
    MasterLoot(bool),
    InvAdd { item_id: i64 },
    InvTook { sound: u8, in_shop: bool },
    LootOpen { mode_before: i32 },
    OpenLootRefused { code: &'static str, reason: String },
    MerchantOpen,
    BankOpen,
    LootOne { confirmed_key: Option<u32> },
    LootAll { corpse_before: Vec<(u32, i64)> },
    LootClose,
    LootBlocked(String, u8),
    TxCancel,
    BankMove {
        is_deposit: bool,
        added_item: Option<(i64, i64)>,
        withdrawn: Option<(u32, i64, i64)>,
    },
    BankMoney,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct GameEventCapture {
    api: api_capture::ChangeCapture,
    broker_reply: Option<BrokerReply>,
    loot_inv_add: Option<(i64, i64)>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn begin_game_event(play: &Play, event: &net::session::Event) -> GameEventCapture {
    MODE_BEFORE_EVENT.with(|m| m.set(play.mode));
    // round A2: what a dialog or offer this packet brings looks like before the game handles it
    a2_play::pre_event(play, event);
    let api = api_capture::begin_game_event(play, event);
    if let net::session::Event::MasterLoot(m) = event {
        crate::fork_ext::addons::action_broker::set_master_loot(m.is_some());
        crate::fork_ext::addons::raid_strips::set_master_loot_detail(m.clone());
    } else if matches!(
        event,
        net::session::Event::Opcode { op: 0x0016 | 0x0018, .. }
            | net::session::Event::WorldEntry { .. }
            | net::session::Event::Teleport { .. }
            | net::session::Event::Disconnected(_)
    ) {
        crate::fork_ext::addons::raid_strips::set_master_loot_detail(None);
        if !matches!(event, net::session::Event::Opcode { .. }) {
            crate::fork_ext::addons::raid_strips::reset();
        }
    }
    let loot_inv_add = match event {
        net::session::Event::InvAdd(item) => Some((item.item.id, item.quantity)),
        _ => None,
    };
    let check_reopen = matches!(
        event,
        net::session::Event::Opcode {
            op: 0x0018 | 0x0085,
            ..
        }
    );
    let broker_reply = if crate::fork_ext::addons::action_broker::has_in_flight() || check_reopen {
        match event {
            net::session::Event::Assist(id) => Some(BrokerReply::Assist(*id)),
            net::session::Event::AbilitySlot { index, slot, .. } => {
                Some(BrokerReply::AbilitySlot {
                    index: *index,
                    slot: *slot,
                })
            }
            net::session::Event::AbilityClearBar { .. } => Some(BrokerReply::AbilityClearBar),
            net::session::Event::AbilityMoveIndex { from, to, .. } => {
                Some(BrokerReply::AbilityMoveIndex {
                    from: *from,
                    to: *to,
                })
            }
            net::session::Event::MasterLoot(m) => Some(BrokerReply::MasterLoot(m.is_some())),
            net::session::Event::InvAdd(item) => Some(BrokerReply::InvAdd {
                item_id: item.item.id,
            }),
            net::session::Event::InvTook { sound, .. } => Some(BrokerReply::InvTook {
                sound: *sound,
                in_shop: play.npcw.shop.id.is_some() && *sound != 0,
            }),
            net::session::Event::Opcode { op: 0x0018, .. } => Some(BrokerReply::LootOpen { mode_before: play.mode }),
            // the live server's answer to a corpse asked for from too far (no 0x0085; owner log 2026-11-18)
            net::session::Event::Chat(text, _)
                if text.trim() == "Out of range." && crate::fork_ext::addons::action_broker::open_loot_in_flight() =>
            {
                Some(BrokerReply::OpenLootRefused { code: "out_of_range", reason: text.trim().to_string() })
            }
            net::session::Event::Opcode { op: 0x00B7, .. } => Some(BrokerReply::MerchantOpen),
            net::session::Event::Opcode { op: 0x124D, .. } => Some(BrokerReply::BankOpen),
            net::session::Event::Opcode { op: 0x0019, data } => {
                let confirmed_key = data.first().copied().and_then(|slot| {
                    play.npcw
                        .corpse
                        .as_ref()
                        .and_then(|c| c.items.get(slot as usize))
                        .map(|it| it.inv_index)
                });
                Some(BrokerReply::LootOne { confirmed_key })
            }
            net::session::Event::Opcode { op: 0x001A, .. } => {
                let corpse_before: Vec<(u32, i64)> = play
                    .npcw
                    .corpse
                    .as_ref()
                    .map(|c| c.items.iter().map(|it| (it.inv_index, it.quantity)).collect())
                    .unwrap_or_default();
                Some(BrokerReply::LootAll { corpse_before })
            }
            net::session::Event::Opcode { op: 0x0016, .. } => Some(BrokerReply::LootClose),
            net::session::Event::Opcode { op: 0x0085, data } => {
                let code = data.first().copied().unwrap_or(0);
                let word_id = match code {
                    1 => Some(673),
                    2 => Some(674),
                    3 => Some(675),
                    _ => None,
                };
                let reason = word_id
                    .and_then(|id| {
                        play.words
                            .as_ref()
                            .map(|w| w.get(id, 0))
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| match code {
                        1 => "That corpse is too far away.".into(),
                        2 => "Someone else is already looting that corpse.".into(),
                        3 => "You cannot loot that corpse.".into(),
                        _ => "Loot blocked by server.".into(),
                    });
                Some(BrokerReply::LootBlocked(reason, code))
            }
            net::session::Event::Opcode { op: 0x00D7, .. } => Some(BrokerReply::TxCancel),
            net::session::Event::Opcode { op: 0x1250, data } => {
                let mut r = net::stream::Reader::new(data);
                let slot = r.u8() as usize;
                let n = r.signed() as i64;
                let withdrawn = (!r.error)
                    .then(|| play.dump.bank.items.items.get(slot))
                    .flatten()
                    .map(|it| (it.inv_index, it.item.id, n));
                Some(BrokerReply::BankMove {
                    is_deposit: false,
                    added_item: None,
                    withdrawn,
                })
            }
            net::session::Event::Opcode { op: 0x1251, data } => {
                let mut r = net::stream::Reader::new(data);
                let item = net::dump::read_net_item(&mut r);
                Some(BrokerReply::BankMove {
                    is_deposit: true,
                    added_item: Some((item.item.id, item.quantity)),
                    withdrawn: None,
                })
            }
            net::session::Event::Opcode { op: 0x1253, .. } => Some(BrokerReply::BankMoney),
            _ => None,
        }
    } else {
        None
    };
    GameEventCapture {
        api,
        broker_reply,
        loot_inv_add,
    }
}

/// Round A2: right after the game handled a packet and before its window state is read (`end_game_event`), a dialog or
/// offer the packet opened or queued is taken for the mod that holds its kind. The one upstream line for A2.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn divert_event(play: &mut Play) {
    a2_play::post_event(play);
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn divert_event(_: &mut Play) {}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn end_game_event(play: &Play, capture: GameEventCapture) {
    MODE_BEFORE_EVENT.with(|m| m.set(-1));
    api_capture::end_game_event(play, capture.api);
    if let Some((item_id, qty)) = capture.loot_inv_add {
        api_capture::items::note_loot_inv_add(item_id, qty);
    }
    match capture.broker_reply {
        Some(BrokerReply::Assist(id)) => {
            crate::fork_ext::addons::action_broker::resolve_server_assist(id);
        }
        Some(BrokerReply::AbilitySlot { index, slot }) => {
            crate::fork_ext::addons::action_broker::resolve_server_ability_slot(index, slot);
        }
        Some(BrokerReply::AbilityClearBar) => {
            crate::fork_ext::addons::action_broker::resolve_server_ability_clear();
        }
        Some(BrokerReply::AbilityMoveIndex { from, to }) => {
            crate::fork_ext::addons::action_broker::resolve_server_ability_move(from, to);
        }
        Some(BrokerReply::MasterLoot(active)) => {
            crate::fork_ext::addons::action_broker::set_master_loot(active);
        }
        Some(BrokerReply::InvAdd { item_id }) => {
            crate::fork_ext::addons::action_broker::mark_loot_inv_added(item_id);
            let inv_total: i64 = play
                .dump
                .inventory
                .items
                .iter()
                .filter(|it| it.item.id == item_id)
                .map(|it| it.quantity)
                .sum();
            crate::fork_ext::addons::action_broker::resolve_server_merchant_buy(
                item_id,
                !play.npcw.shop.pending,
                inv_total,
            );
        }
        Some(BrokerReply::InvTook { sound, in_shop }) => {
            if in_shop || (play.npcw.shop.id.is_some() && sound != 0) {
                crate::fork_ext::addons::window_takeover::touch_hidden_session(
                    crate::fork_ext::addons::window_takeover::TakeoverKind::Merchant,
                );
                crate::fork_ext::addons::action_broker::resolve_server_merchant_sell(
                    !play.npcw.shop.pending,
                    |key| {
                        play.dump
                            .inventory
                            .items
                            .iter()
                            .find(|it| it.inv_index == key)
                            .map_or(0, |it| it.quantity)
                    },
                );
            }
        }
        Some(BrokerReply::LootOne { confirmed_key }) => {
            LOOT_BUSY.with(|b| b.set(false));
            crate::fork_ext::addons::window_takeover::touch_hidden_session(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
            );
            if confirmed_key.is_some()
                || crate::fork_ext::addons::action_broker::is_player_activity(
                    crate::fork_ext::addons::action_broker::Subsystem::Loot,
                )
            {
                crate::fork_ext::addons::action_broker::resolve_server_loot_one(
                    confirmed_key,
                    Some(true),
                    None,
                    |k| {
                        play.npcw
                            .corpse
                            .as_ref()
                            .and_then(|c| c.items.iter().find(|it| it.inv_index == k))
                            .map_or(0, |it| it.quantity)
                    },
                );
            }
        }
        Some(BrokerReply::LootAll { corpse_before }) => {
            LOOT_BUSY.with(|b| b.set(false));
            crate::fork_ext::addons::window_takeover::touch_hidden_session(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
            );
            let corpse_after: Vec<(u32, i64)> = play
                .npcw
                .corpse
                .as_ref()
                .map(|c| c.items.iter().map(|it| (it.inv_index, it.quantity)).collect())
                .unwrap_or_default();
            let state_changed = !corpse_before.is_empty() && corpse_after != corpse_before;
            crate::fork_ext::addons::action_broker::resolve_server_loot_all(
                Some(true),
                None,
                state_changed,
            );
        }
        Some(BrokerReply::OpenLootRefused { code, reason }) => {
            tlog!("takeover: the server refused the corpse open: {reason}");
            let _ = crate::fork_ext::loot_by_id::note_refusal(crate::fork_ext::loot_by_id::Refusal::OutOfRange, open_waits);
            crate::fork_ext::addons::window_takeover::clear_pending_open(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
            );
            crate::fork_ext::addons::action_broker::resolve_server_open_loot_refused(code, &reason);
        }
        // the game turns a corpse's list away when another of its windows is up (`npcw` 0x0018 outside mode 0
        // answers 0x0016): the open is refused at once, never left waiting (upstream's mercenary windows and dye
        // picker, modes 61 to 63, are such windows)
        Some(BrokerReply::LootOpen { mode_before })
            if mode_before != 0 && crate::fork_ext::addons::action_broker::open_loot_in_flight() =>
        {
            tlog!("takeover: corpse list turned away by the game (mode {mode_before}: another window is up); the open is refused");
            crate::fork_ext::loot_by_id::drop_probe();
            crate::fork_ext::addons::window_takeover::clear_pending_open(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
            );
            crate::fork_ext::addons::action_broker::resolve_server_open_loot_refused(
                "busy",
                "another game window was open; the corpse was closed",
            );
        }
        Some(BrokerReply::LootOpen { .. }) => {
            crate::fork_ext::loot_by_id::note_list_reply(api_capture::items::loot_source_named(), open_waits);
            crate::fork_ext::addons::window_takeover::touch_hidden_session(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
            );
            let is_empty = play
                .npcw
                .corpse
                .as_ref()
                .is_none_or(|c| c.items.is_empty());
            let opened_corpse_id = api_capture::items::current_loot_source_id(play);
            crate::fork_ext::addons::action_broker::resolve_server_loot_open(
                opened_corpse_id,
                is_empty,
            );
        }
        Some(BrokerReply::LootClose) => {
            LOOT_BUSY.with(|b| b.set(false));
            crate::fork_ext::addons::action_broker::set_master_loot(false);
            crate::fork_ext::addons::raid_strips::set_master_loot_detail(None);
            crate::fork_ext::addons::action_broker::resolve_server_loot_close();
        }
        Some(BrokerReply::MerchantOpen) => {
            crate::fork_ext::addons::window_takeover::touch_hidden_session(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Merchant,
            );
            crate::fork_ext::addons::action_broker::resolve_server_talk_open(
                crate::fork_ext::addons::action_broker::TalkKind::Merchant,
                play.npcw.shop.id,
            );
        }
        Some(BrokerReply::BankOpen) => {
            crate::fork_ext::addons::window_takeover::touch_hidden_session(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Bank,
            );
            crate::fork_ext::addons::action_broker::resolve_server_talk_open(
                crate::fork_ext::addons::action_broker::TalkKind::Bank,
                None,
            );
        }
        Some(BrokerReply::LootBlocked(reason, code)) => {
            LOOT_BUSY.with(|b| b.set(false));
            use crate::fork_ext::loot_by_id::Refusal;
            let kind = match code {
                1 => Refusal::OutOfRange,
                2 => Refusal::Understood,
                _ => Refusal::Unclear,
            };
            if crate::fork_ext::loot_by_id::note_refusal(kind, open_waits) {
                // the by-id open was refused without a reason: the old request goes out at the next step and the
                // mod's open stays in flight (it ends with that request's answer)
                tlog!("takeover: the server refused the by-id corpse open without a reason ({reason}); asking the old way");
            } else {
                crate::fork_ext::addons::window_takeover::clear_pending_open(
                    crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
                );
                crate::fork_ext::addons::action_broker::resolve_server_loot_blocked(Some(&reason));
            }
        }
        Some(BrokerReply::TxCancel) => {
            LOOT_BUSY.with(|b| b.set(false));
            crate::fork_ext::addons::action_broker::resolve_server_d7();
        }
        Some(BrokerReply::BankMove {
            is_deposit,
            added_item,
            withdrawn,
        }) => {
            crate::fork_ext::addons::window_takeover::touch_hidden_session(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Bank,
            );
            crate::fork_ext::addons::action_broker::resolve_server_bank_move(
                is_deposit,
                play.bank.state == 0,
                added_item,
                withdrawn,
                |key| {
                    play.dump
                        .bank
                        .items
                        .items
                        .iter()
                        .find(|it| it.inv_index == key)
                        .map_or(0, |it| it.quantity)
                },
            );
        }
        Some(BrokerReply::BankMoney) => {
            crate::fork_ext::addons::action_broker::resolve_server_bank_money(
                play.bank.state == 0,
                play.dump.player.p38,
            );
        }
        None => {}
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn begin_game_event(_: &Play, _: &net::session::Event) {}
#[cfg(target_arch = "wasm32")]
pub(crate) fn end_game_event(_: &Play, _: ()) {}
#[cfg(not(target_arch = "wasm32"))]
struct NoopRaster;
#[cfg(not(target_arch = "wasm32"))]
impl vi::ui::Raster for NoopRaster {
    fn blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
    fn stretch_blit(&mut self, _: u32, _: u32, _: [i32; 2], _: [i32; 2], _: [i32; 2], _: [i32; 2], _: i32, _: f32) {}
    fn fill_fan(&mut self, _: [f32; 4], _: [[i32; 2]; 4]) {}
    fn blit_full(&mut self, _: u32, _: u32, _: [i32; 2], _: i32, _: f32) {}
    fn draw_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16]) {}
    fn draw_centered_string(&mut self, _: [i32; 2], _: [f32; 4], _: &[u16], _: u32) {}
    fn string_width(&self, _: &[u16]) -> i32 { 0 }
    fn point_size(&self) -> i32 { 12 }
    fn viewport(&self) -> [i32; 4] { [0, 0, 639, 447] }
    fn set_viewport(&mut self, _: [i32; 4]) {}
}

pub(crate) fn window_taken(play: &Play, mode: i32) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::fork_ext::addons::window_takeover::{self, TakeoverKind};
        if !window_takeover::has_activity() {
            return false;
        }
        let Some(kind) = TakeoverKind::from_mode(mode) else {
            return false;
        };
        // L4: the window was already open natively before this packet: it is the player's
        if MODE_BEFORE_EVENT.with(std::cell::Cell::get) == mode {
            tlog!(
                "takeover: {} window left to the game (already open before the reply)",
                kind.name()
            );
            return false;
        }
        let observed_target = match kind {
            TakeoverKind::Loot => crate::fork_ext::loot_source::peek_single(),
            TakeoverKind::Merchant => play.npcw.shop.id,
            _ => None,
        };
        let fallback_player_target = (play.target != u32::MAX).then_some(play.target);
        let decision = window_takeover::decide_open(
            kind,
            observed_target,
            fallback_player_target,
            play.self_at.map(|(p, _)| p),
        );
        // always on, like the game's own packet lines, so an owner's log explains a window that did or did not show
        match decision {
            window_takeover::OpenDecision::NoHolder => {}
            window_takeover::OpenDecision::Accepted(why) => tlog!(
                "takeover: {} window HIDDEN for the mod ({why}; observed {observed_target:?}, player target {fallback_player_target:?})",
                kind.name()
            ),
            window_takeover::OpenDecision::Declined(why) => tlog!(
                "takeover: {} window left to the game ({why}; observed {observed_target:?}, player target {fallback_player_target:?})",
                kind.name()
            ),
        }
        decision.accepted()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (play, mode);
        false
    }
}

pub(crate) fn bank_taken_tick(play: &mut Play, vui: &crate::ui::Vui) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::fork_ext::addons::window_takeover;
        if play.mode != 0 || !window_takeover::has_activity() {
            return false;
        }
        let Some(banker_id) = window_takeover::hidden_bank_session() else {
            return false;
        };
        let saved_target = play.target;
        if banker_id != u32::MAX {
            play.target = banker_id;
        }
        play.mode = 11;
        let mut noop = NoopRaster;
        play.run_bank(vui, &mut noop);
        play.mode = 0;
        if banker_id != u32::MAX {
            play.target = saved_target;
        }
        true
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (play, vui);
        false
    }
}

pub(crate) fn loot_session_open(play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        (play.mode == 1
            || crate::fork_ext::addons::window_takeover::is_hidden_active(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
            ))
            && play.npcw.corpse.is_some()
    }
    #[cfg(target_arch = "wasm32")]
    {
        play.mode == 1 && play.npcw.corpse.is_some()
    }
}

pub(crate) fn merchant_session_open(play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        (play.mode == 10
            || crate::fork_ext::addons::window_takeover::is_hidden_active(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Merchant,
            ))
            && play.npcw.shop.id.is_some()
    }
    #[cfg(target_arch = "wasm32")]
    {
        play.mode == 10 && play.npcw.shop.id.is_some()
    }
}

pub(crate) fn bank_session_open(play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        play.mode == 11
            || crate::fork_ext::addons::window_takeover::is_hidden_active(
                crate::fork_ext::addons::window_takeover::TakeoverKind::Bank,
            )
    }
    #[cfg(target_arch = "wasm32")]
    {
        play.mode == 11
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn close_hidden_session(
    play: &mut Play,
    kind: crate::fork_ext::addons::window_takeover::TakeoverKind,
    owed: crate::fork_ext::addons::window_takeover::OwedClose,
) {
    use crate::fork_ext::addons::window_takeover::TakeoverKind;
    if !owed.had_session {
        // released with no hidden session up: there is nothing of the mod's to close or to send
        tlog!(
            "takeover: {} released with no hidden session ({}); nothing to close",
            kind.name(),
            owed.reason
        );
        return;
    }
    match kind {
        TakeoverKind::Loot => {
            // one 0x0016 per session, and none when the server already closed the corpse (its own 0x0016 cleared it)
            let sent = play.npcw.corpse.is_some();
            if sent {
                play.session.send_opcode(0x0016);
            }
            tlog!(
                "takeover: loot hidden session closed ({}); 0x0016 {}",
                owed.reason,
                if sent {
                    "sent"
                } else {
                    "not sent: the server had closed the corpse"
                }
            );
            if sent {
                crate::fork_ext::loot_by_id::mark_close("mod");
                let _ = play.npcw_opcode(0x0016, &[]);
            }
            LOOT_BUSY.with(|b| b.set(false));
            crate::fork_ext::addons::action_broker::set_master_loot(false);
            crate::fork_ext::addons::raid_strips::set_master_loot_detail(None);
            crate::fork_ext::loot_source::closed();
        }
        TakeoverKind::Merchant => {
            tlog!("takeover: merchant hidden session closed ({})", owed.reason);
            play.npcw.merchant.visible = false;
            play.npcw.shop = Default::default();
        }
        TakeoverKind::Bank => {
            tlog!("takeover: bank hidden session closed ({})", owed.reason);
            play.bank.state = 0;
        }
        _ => {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_hidden_sessions(play: &mut Play) {
    use crate::fork_ext::addons::window_takeover::{self, TakeoverKind};
    if !window_takeover::has_activity() {
        return;
    }
    let self_pos = play.self_at.map(|(p, _)| p);
    // the opens a mod's request still waits for: a pending claim without one (refused, timed out,
    // cancelled) is dropped, so it can never claim a window the player opens (M2)
    let waiting = crate::fork_ext::addons::action_broker::open_requests_waiting();
    // the server closes a corpse after every take (0x0016) and the holder's batch is waiting for it to be listed
    // again: that close is part of the cycle, not the end of the session
    let reopen_expected = waiting.iter().any(|(_, kinds)| kinds[TakeoverKind::Loot as usize]);
    window_takeover::sync_sessions_with(
        |kind, _target_id, opened_pos| {
            // only the player's own walk since the open ends a session (never how far the corpse or NPC
            // was when it opened: the server decides what it allows, and a far corpse must still show)
            match kind {
                TakeoverKind::Loot if play.npcw.corpse.is_none() && !reopen_expected => {
                    return Err("the server closed the corpse");
                }
                TakeoverKind::Merchant if play.npcw.shop.id.is_none() => {
                    return Err("the merchant session is gone");
                }
                _ => {}
            }
            if let Some(pos) = self_pos {
                if let Some(orig) = opened_pos
                    && ee::lt(625.0, ee_dist_sq(pos, orig))
                {
                    return Err("walked away (more than 25 units)");
                }
            }
            Ok(())
        },
        |vm, kind| {
            waiting
                .iter()
                .any(|(w_vm, kinds)| *w_vm == vm && kinds[kind as usize])
        },
    );
    // M3: an ended session's close waits while a native window is up (it would disturb it) and
    // is sent the moment none is; it is never forgotten
    if play.mode == 0 {
        let owed = window_takeover::take_owed_closes();
        for kind in TakeoverKind::ALL {
            if let Some(o) = owed[kind as usize] {
                close_hidden_session(play, kind, o);
            }
        }
        // M1: the mod is done: the player's own target comes back, unless they chose another
        let restores = window_takeover::take_restorable(|vm| {
            crate::fork_ext::addons::action_broker::vm_is_idle(vm)
        });
        for r in restores {
            let prev_ok = play
                .entities
                .slots
                .iter()
                .flatten()
                .any(|e| e.id == r.prev && play.in_scene(e) && e.p18 & 2 == 0);
            if play.target == r.mod_target && prev_ok {
                play.target_id(r.prev);
            }
        }
    }
}

/// Whether the mod's `open_loot` for `corpse` is still waiting for the server (what a by-id probe may be judged by).
#[cfg(not(target_arch = "wasm32"))]
fn open_waits(vm: u64, corpse: u32) -> bool {
    crate::fork_ext::addons::action_broker::open_loot_waiting(vm, corpse)
}

/// The loot request (0x0015) for `corpse_id` itself, with no target set first: what `Play::attack_press` sends for a
/// targeted corpse, minus the target (`loot_by_id`). The request is noted as the client's own, as `attack_press` notes it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn send_loot_request_by_id(play: &mut Play, corpse_id: u32) {
    crate::fork_ext::loot_requested(corpse_id);
    let mut w = net::stream::Writer::op(0x0015);
    w.u32(corpse_id);
    play.session.send(&w.data);
}

/// The by-id open got no usable answer: ask again the old way (target the corpse, loot, the target comes back later).
#[cfg(not(target_arch = "wasm32"))]
fn retry_open_the_old_way(play: &mut Play, fb: crate::fork_ext::loot_by_id::Fallback) {
    use crate::fork_ext::addons::{action_broker, window_takeover};
    let valid = play
        .entities
        .slots
        .iter()
        .flatten()
        .any(|e| e.id == fb.corpse && play.in_scene(e) && (e.p18 & 1 != 0) && (e.p18 & 2 != 0));
    if !valid || !matches!(play.mode, 0 | 38) || play.is_dead() || play.control_client().casting {
        // nothing can be sent: the mod's open ends now with the reason, and nothing was tried the old way, so the method is
        // undecided again instead of left on a fallback nobody tried
        tlog!("takeover: by-id fallback for corpse {:#x}: the corpse or the player is not ready; the open is refused", fb.corpse);
        crate::fork_ext::loot_by_id::abandon_fallback(fb.corpse);
        window_takeover::clear_pending_open(window_takeover::TakeoverKind::Loot);
        let (code, reason) = if valid {
            ("not_ready", "cannot open corpse right now")
        } else {
            ("not_found", "the corpse is no longer in the scene")
        };
        action_broker::resolve_server_open_loot_refused(code, reason);
        return;
    }
    // the first request's late answer, if any, still names this corpse; nothing is waiting but this one
    crate::fork_ext::loot_source::clear();
    if play.target != fb.corpse {
        let prev = play.target;
        play.target_id(fb.corpse);
        window_takeover::note_retarget(fb.vm, prev, fb.corpse);
    }
    let _ = window_takeover::arm_pending_open(window_takeover::TakeoverKind::Loot, fb.vm, fb.corpse);
    tlog!("takeover: fallback open_loot -> corpse {:#x}, opcode 0x0015 after targeting it", fb.corpse);
    play.attack_press();
    crate::fork_ext::loot_by_id::note_sent_retarget(fb.vm, fb.corpse);
    action_broker::set_open_loot_method(fb.corpse, "retarget");
}

pub(crate) fn begin_iteration(_play: &mut Play, _game_step: bool) {
    // others' power is asked for by upstream's Power Bars row alone (`settings::power_bars`);
    // a second request here would contradict it every frame
    #[cfg(not(target_arch = "wasm32"))]
    {
        sync_hidden_sessions(_play);
        a1_play::tick(_play);
        a2_play::tick(_play);
        a8_play::tick(_play, _game_step);
        api_capture::a5::tick(_play);
        if crate::fork_ext::addons::native_core::capture_requested()
            || crate::fork_ext::addons::window_takeover::has_activity()
        {
            super::raidplay::fork_win::snapshot_raid(_play);
        }
        net::fork_record::drain(&mut _play.session);
        if crate::fork_ext::addons::input::is_text_capture_active() {
            _play.attack_release();
            _play.info_up = false;
            _play.aim_hold = false;
            _play.autorun = false;
            _play.following = false;
        }
        let has_pet = _play.pet_id != u32::MAX;
        let effects_height = if has_pet {
            crate::hud::pet_effects_height(_play.pet_effects().len())
        } else {
            0
        };
        let pet_y = if _play.group.raid.is_empty() {
            93 + ((_play.group.count() as i32 - 1) * 48).max(0)
        } else {
            let below = if has_pet {
                crate::raidlist::PET_H + effects_height
            } else {
                0
            };
            crate::raidlist::Fit::under(_play.group.raid.len(), below).bounds([20, 93])[3] + 2
        };
        // Hud's effect strip begins at y=48; an unbuffed pet panel ends at 46.
        let pet_h = if !has_pet {
            0
        } else if effects_height == 0 {
            46
        } else {
            48 + effects_height
        };
        crate::fork_ext::addons::hud_layout::set_hud_state(
            _play.right_hud_up(),
            _play.right_hud_up(),
            _play.group.in_group && !_play.group.raid.is_empty(),
            pet_y,
            pet_h,
        );
        let snapshot = native_action_snapshot(_play);
        crate::fork_ext::addons::action_broker::update_native_snapshot(snapshot);
        crate::fork_ext::addons::native_core::iteration(_game_step);
        if _game_step {
            // only against pins the map has read: a fresh map after a login holds none yet
            if _play.map.api_waypoints_loaded() {
                crate::fork_ext::addons::map_waypoint::reconcile(|world| _play.map.waypoint(world));
            }
            if let Some(pending) = PENDING_BANK_TX.with(|b| {
                if !bank_session_open(_play) {
                    b.borrow_mut().take()
                } else {
                    None
                }
            }) {
                let kind = match pending {
                    PendingBankTx::Move { .. } => "bank_move",
                    PendingBankTx::Money { .. } => "bank_money",
                };
                crate::fork_ext::addons::action_broker::resolve_in_flight_window_closed(
                    kind,
                    "bank window is not open",
                );
            }
            if let Some(pending) = PENDING_NPCW_TX.with(|p| {
                let mut b = p.borrow_mut();
                match *b {
                    Some(PendingNpcwTx::LootOne { .. })
                        if !loot_session_open(_play) =>
                    {
                        b.take()
                    }
                    Some(PendingNpcwTx::LootAll)
                        if !loot_session_open(_play) =>
                    {
                        b.take()
                    }
                    Some(PendingNpcwTx::Buy { .. } | PendingNpcwTx::Sell { .. })
                        if !merchant_session_open(_play) =>
                    {
                        b.take()
                    }
                    _ => None,
                }
            }) {
                let (kind, msg) = match pending {
                    PendingNpcwTx::LootOne { .. } => ("loot", "corpse window is not open"),
                    PendingNpcwTx::LootAll => ("loot_all", "corpse window is not open"),
                    PendingNpcwTx::Buy { .. } => ("buy", "merchant window is not open"),
                    PendingNpcwTx::Sell { .. } => ("sell", "merchant window is not open"),
                };
                crate::fork_ext::addons::action_broker::resolve_in_flight_window_closed(
                    kind, msg,
                );
            }
            let ready = _play.in_world_now();
            if crate::fork_ext::addons::action_broker::take_loot_timeout_expired() {
                LOOT_BUSY.with(|b| b.set(false));
            }
            // a by-id corpse open that nobody answers, or that the server refused without a reason, goes out again the
            // old way (target the corpse, loot) for the rest of the session
            crate::fork_ext::loot_by_id::step();
            let fallback = crate::fork_ext::loot_by_id::take_retry().or_else(|| {
                crate::fork_ext::loot_by_id::poll_timeout(|vm, corpse| {
                    crate::fork_ext::addons::action_broker::open_loot_waiting(vm, corpse)
                })
            });
            if let Some(fb) = fallback {
                retry_open_the_old_way(_play, fb);
            }
            // a corpse opened by id is not the player's target: it is the one the mod asked for
            let by_id_corpse = crate::fork_ext::addons::window_takeover::by_id_corpse().filter(|id| {
                _play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .any(|e| e.id == *id && e.p18 & 2 != 0)
            });
            let corpse_target = by_id_corpse.or_else(|| {
                (_play.target != 0
                    && _play
                        .entities
                        .slots
                        .iter()
                        .flatten()
                        .any(|e| e.id == _play.target && e.p18 & 2 != 0))
                .then_some(_play.target)
            });
            crate::fork_ext::addons::action_broker::set_corpse_target_id(corpse_target);
            if _play.map.open || _play.mode == 38 {
                crate::fork_ext::addons::action_broker::abort_corpse_reopen(
                    "not_ready",
                    "stopped: map or camera open",
                );
            } else if ready && _play.mode != 0 && _play.mode != 1 {
                crate::fork_ext::addons::action_broker::abort_corpse_reopen(
                    "window_closed",
                    "stopped: non-play mode active",
                );
            } else if ready
                && _play.mode == 0
                && let Some((reopen_vm, reopen_cid)) =
                    crate::fork_ext::addons::action_broker::take_corpse_reopen_ask()
                && corpse_target == Some(reopen_cid)
            {
                if by_id_corpse == Some(reopen_cid) {
                    // the session was opened by id: the corpse is asked for again by id, the target stays as it is
                    let _ = crate::fork_ext::addons::window_takeover::arm_pending_open_by_id(reopen_vm, reopen_cid);
                    send_loot_request_by_id(_play, reopen_cid);
                } else {
                    let _ = crate::fork_ext::addons::window_takeover::arm_pending_open(
                        crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
                        reopen_vm,
                        reopen_cid,
                    );
                    _play.attack_press();
                }
            }
            let wheel = crate::fork_ext::addons::pointer_api::take_wheel();
            if ready && wheel != 0.0 {
                _play.map.mini_wheel(wheel);
            }
            weapon_sets_requests(_play, ready);
            crate::fork_ext::addons::action_broker::step_tick(snapshot, |kind| {
                dispatch_broker_action(_play, kind)
            });
            discard_expired_staged();
            let reqs = crate::fork_ext::addons::actions::take_action_requests();
            drain_released_waypoints(_play);
            if ready {
                if reqs.map && crate::settings::map() && _play.addon_input_available() {
                    _play.map.show();
                }
                if reqs.assist && matches!(_play.mode, 0 | 38) {
                    _play.assist_target();
                }
                if reqs.summary
                    && _play.mode == 0
                    && !_play.map.open
                    && !crate::fork_ext::addons::input::is_text_capture_active()
                    && !_play.search.open
                    && _play.typing.is_none()
                    && _play.death.is_none()
                {
                    _play.trainer_open = true;
                    _play.trainer_from_hud = true;
                    _play.mode = 12;
                }
            }
            if !ready {
                LOOT_BUSY.with(|b| b.set(false));
                PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                PENDING_BANK_TX.with(|b| *b.borrow_mut() = None);
                PENDING_CAMERA.with(|c| c.set(None));
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

/// Called from upstream's `impl Drop for Play`: the session recorder writes
/// what is still queued before the world goes.
pub(super) fn dropped(_play: &mut Play) {
    #[cfg(not(target_arch = "wasm32"))]
    net::fork_record::drain(&mut _play.session);
    // a mod disabled since the last game step: its pins go before the world does (a logout)
    #[cfg(not(target_arch = "wasm32"))]
    drain_released_waypoints(_play);
    // round A1: no mod window, question or controller focus outlives the world
    #[cfg(not(target_arch = "wasm32"))]
    a1_play::play_dropped(_play.quit || _play.disconnected.is_some());
    // round A8: nothing of a mod's crafting window is left to read
    #[cfg(not(target_arch = "wasm32"))]
    a8_play::dropped();
}

/// Round A8: the server's crafting reply (0x1102), offered to a mod's crafting window before the game's own
/// reading, which takes it only while the game's recipe book or experiment is up (`a8_play::reply`). True when a
/// mod's window took it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn craft_reply(play: &mut Play, data: &[u8]) -> bool {
    a8_play::reply(play, data)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn craft_reply(_: &mut Play, _: &[u8]) -> bool {
    false
}

/// Clears the pins disabled mods still own, once the map has read the window's pins; until then the
/// released records wait (for this map's first frame, or the next login's).
#[cfg(not(target_arch = "wasm32"))]
fn drain_released_waypoints(play: &mut Play) {
    if !play.map.api_waypoints_loaded() {
        return;
    }
    for world in crate::fork_ext::addons::map_waypoint::take_released(|world| play.map.waypoint(world)) {
        play.map.api_set_waypoint(world, None);
    }
}

/// Drops staged native sends whose broker request already ended (cancelled, timed out, its batch stopped).
/// Runs each step and again right before the merchant, loot and bank send hooks, so a stop that lands
/// between the step and the native window's frame is not sent anyway.
#[cfg(not(target_arch = "wasm32"))]
fn discard_expired_staged() {
    for expired_kind in crate::fork_ext::addons::action_broker::take_expired_staged_kinds() {
        clear_staged_for_kind(expired_kind);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_staged_for_kind(kind_name: &'static str) {
    match kind_name {
        "loot" => PENDING_NPCW_TX.with(|p| {
            if matches!(*p.borrow(), Some(PendingNpcwTx::LootOne { .. })) {
                *p.borrow_mut() = None;
            }
        }),
        "loot_all" => PENDING_NPCW_TX.with(|p| {
            if matches!(*p.borrow(), Some(PendingNpcwTx::LootAll)) {
                *p.borrow_mut() = None;
            }
        }),
        "buy" => PENDING_NPCW_TX.with(|p| {
            if matches!(*p.borrow(), Some(PendingNpcwTx::Buy { .. })) {
                *p.borrow_mut() = None;
            }
        }),
        "sell" => PENDING_NPCW_TX.with(|p| {
            if matches!(*p.borrow(), Some(PendingNpcwTx::Sell { .. })) {
                *p.borrow_mut() = None;
            }
        }),
        "bank_move" => PENDING_BANK_TX.with(|b| {
            if matches!(*b.borrow(), Some(PendingBankTx::Move { .. })) {
                *b.borrow_mut() = None;
            }
        }),
        "bank_money" => PENDING_BANK_TX.with(|b| {
            if matches!(*b.borrow(), Some(PendingBankTx::Money { .. })) {
                *b.borrow_mut() = None;
            }
        }),
        "camera" => PENDING_CAMERA.with(|c| c.set(None)),
        _ => {}
    }
}

/// Pushes staged broker loot/merchant requests immediately before `npcw.rs`'s native loops.
pub(crate) fn npcw_requests(
    _play: &Play,
    _loot_out: &mut Vec<crate::loot::LootOut>,
    _merch_out: &mut Vec<crate::merchant::MerchantOut>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::fork_ext::addons::action_broker::{
            self, Subsystem, is_master_loot_active, mark_deferred_sent, mark_player_activity,
            resolve_in_flight_window_closed, resolve_staged_refusal,
        };
        if !_loot_out.is_empty() {
            mark_player_activity(Subsystem::Loot);
            for o in _loot_out.iter() {
                match *o {
                    crate::loot::LootOut::Close => {
                        // this list holds only what the player's own use of the game's window asked for: the mod's
                        // staged requests are added after it
                        crate::fork_ext::loot_by_id::mark_close("player");
                        api_capture::items::clear_pending_loot()
                    }
                    crate::loot::LootOut::One(key, n) => {
                        api_capture::items::note_loot_send(_play, Some(key as u32), n as i64, false);
                    }
                    crate::loot::LootOut::All => {
                        api_capture::items::note_loot_send(_play, None, 0, true);
                    }
                    _ => {}
                }
            }
        }
        if !_merch_out.is_empty() {
            mark_player_activity(Subsystem::Merchant);
        }
        discard_expired_staged();
        let Some(req) = PENDING_NPCW_TX.with(|p| *p.borrow()) else {
            return;
        };
        match req {
            PendingNpcwTx::LootOne { key, count } => {
                if !loot_session_open(_play) {
                    PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                    resolve_in_flight_window_closed("loot", "corpse window is not open");
                    return;
                }
                if is_master_loot_active() {
                    PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                    resolve_staged_refusal(
                        "loot",
                        "permission_denied",
                        "corpse is under master loot rules",
                    );
                    return;
                }
                if LOOT_BUSY.with(|b| b.get()) || !_loot_out.is_empty() {
                    return;
                }
                PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                if _play
                    .npcw
                    .corpse
                    .as_ref()
                    .is_some_and(|c| c.items.iter().any(|it| it.inv_index == key))
                {
                    LOOT_BUSY.with(|b| b.set(true));
                    api_capture::items::note_loot_send(_play, Some(key), count as i64, false);
                    _loot_out.push(crate::loot::LootOut::One(key as i32, count));
                    mark_deferred_sent("loot");
                } else {
                    resolve_staged_refusal("loot", "stale_handle", "item is no longer on corpse");
                }
            }
            PendingNpcwTx::LootAll => {
                if !loot_session_open(_play) {
                    PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                    resolve_in_flight_window_closed("loot_all", "corpse window is not open");
                    return;
                }
                if is_master_loot_active() {
                    PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                    resolve_staged_refusal(
                        "loot_all",
                        "permission_denied",
                        "corpse is under master loot rules",
                    );
                    return;
                }
                if LOOT_BUSY.with(|b| b.get()) || !_loot_out.is_empty() {
                    return;
                }
                PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                if _play
                    .npcw
                    .corpse
                    .as_ref()
                    .is_some_and(|c| !c.items.is_empty())
                {
                    LOOT_BUSY.with(|b| b.set(true));
                    api_capture::items::note_loot_send(_play, None, 0, true);
                    _loot_out.push(crate::loot::LootOut::All);
                    mark_deferred_sent("loot_all");
                } else {
                    resolve_staged_refusal("loot_all", "not_found", "corpse has no items to loot");
                }
            }
            PendingNpcwTx::Buy { ware_key, count } => {
                if !merchant_session_open(_play) {
                    PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                    resolve_in_flight_window_closed("buy", "merchant window is not open");
                    return;
                }
                if _play.npcw.shop.pending || !_merch_out.is_empty() {
                    return;
                }
                PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                if let Some(pos) = _play
                    .npcw
                    .shop
                    .wares
                    .items
                    .iter()
                    .position(|it| it.inv_index == ware_key)
                {
                    _merch_out.push(crate::merchant::MerchantOut::Buy(pos as i32, count));
                    mark_deferred_sent("buy");
                    crate::fork_ext::addons::window_takeover::note_merchant_transaction_sent();
                } else {
                    resolve_staged_refusal(
                        "buy",
                        "stale_handle",
                        "ware is no longer offered by merchant",
                    );
                }
            }
            PendingNpcwTx::Sell { item_key, count } => {
                if !merchant_session_open(_play) {
                    PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                    resolve_in_flight_window_closed("sell", "merchant window is not open");
                    return;
                }
                if _play.npcw.shop.pending || !_merch_out.is_empty() {
                    return;
                }
                PENDING_NPCW_TX.with(|p| *p.borrow_mut() = None);
                if let Some(slot) = _play
                    .dump
                    .inventory
                    .items
                    .iter()
                    .position(|it| it.inv_index == item_key && it.quantity >= count as i64)
                {
                    // the native loop (`npcw.rs`) sends only while the stack still holds `count`
                    _merch_out.push(crate::merchant::MerchantOut::Sell(slot as i32, count));
                    mark_deferred_sent("sell");
                    crate::fork_ext::addons::window_takeover::note_merchant_transaction_sent();
                } else {
                    resolve_staged_refusal(
                        "sell",
                        "stale_handle",
                        "item is no longer in inventory or the stack is smaller than requested",
                    );
                }
            }
        }
        let _ = action_broker::has_in_flight;
    }
}

/// Pushes staged broker bank requests immediately before `bankplay.rs`'s native loop.
pub(crate) fn bank_requests(_play: &Play, _out: &mut Vec<crate::bank::BankOut>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::fork_ext::addons::action_broker::{
            Subsystem, mark_deferred_sent, mark_player_activity,
            resolve_in_flight_window_closed, resolve_staged_refusal,
        };
        if !_out.is_empty() {
            mark_player_activity(Subsystem::Bank);
        }
        discard_expired_staged();
        let Some(req) = PENDING_BANK_TX.with(|b| *b.borrow()) else {
            return;
        };
        if !bank_session_open(_play) {
            PENDING_BANK_TX.with(|b| *b.borrow_mut() = None);
            let kind = match req {
                PendingBankTx::Move { .. } => "bank_move",
                PendingBankTx::Money { .. } => "bank_money",
            };
            resolve_in_flight_window_closed(kind, "bank window is not open");
            return;
        }
        if _play.bank.state != 0 || !_out.is_empty() {
            return;
        }
        PENDING_BANK_TX.with(|b| *b.borrow_mut() = None);
        match req {
            PendingBankTx::Move {
                from_bank,
                item_key,
                count,
            } => {
                let list = if from_bank {
                    &_play.dump.bank.items
                } else {
                    &_play.dump.inventory
                };
                if let Some(slot) = list.items.iter().position(|it| it.inv_index == item_key) {
                    _out.push(crate::bank::BankOut::Move(
                        u8::from(from_bank),
                        slot as i32,
                        count,
                    ));
                    mark_deferred_sent("bank_move");
                } else {
                    resolve_staged_refusal(
                        "bank_move",
                        "stale_handle",
                        "item is no longer in source container",
                    );
                }
            }
            PendingBankTx::Money { dir, amount } => {
                _out.push(crate::bank::BankOut::MoveMoney(dir, amount));
                mark_deferred_sent("bank_money");
            }
        }
    }
}

/// A skill the Class Mastery list's reply opened is kept (not dropped for want of the game's window) while a mod
/// holds a Class Mastery window session (round A5; `play.rs` calls this where it drops the node).
pub(crate) fn mastery_node_kept(_play: &Play) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        api_capture::a5::node_kept(_play)
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

/// Applies a one-shot broker camera request through native WalkCamera/camstyle setters.
pub(crate) fn apply_camera(_play: &mut Play, _walker: &mut crate::walker::Walker) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        api_capture::a3::apply_jump(_play, _walker);
        let Some(req) = PENDING_CAMERA.with(|c| c.take()) else {
            return;
        };
        if !matches!(_play.mode, 0 | 38) || _play.is_dead() {
            crate::fork_ext::addons::action_broker::resolve_staged_refusal(
                "camera",
                "busy",
                "camera cannot change in current state",
            );
            return;
        }
        crate::fork_ext::addons::action_broker::mark_deferred_sent("camera");
        if let Some(mode) = req.mode {
            let rc = _play.race_cams.as_ref().and_then(|t| {
                crate::camstyle::race_camera(
                    t,
                    _play.dump.player.race as i32,
                    _play.session.expansion & net::session::EXPANSION_FLAGS,
                )
            });
            crate::camstyle::set_style(&mut _walker.cam, mode, rc, &mut _walker.scene);
        }
        if let Some(pitch) = req.pitch {
            let min_p = _walker.cam.walk.min_pitch.0;
            let max_p = _walker.cam.walk.max_pitch.0;
            let clamped = pitch.clamp(min_p, max_p);
            _walker
                .cam
                .set_orbit_pitch_angle(vi::fpu::F::<vi::fpu::Ee>::new(clamped));
        }
        if let Some(distance) = req.distance {
            _walker
                .cam
                .set_orbit_distance(vi::fpu::F::<vi::fpu::Ee>::new(distance));
        }
        if let Some(heading) = req.heading {
            _walker
                .cam
                .set_orbit_angle(vi::fpu::F::<vi::fpu::Ee>::new(heading));
        }
        crate::fork_ext::addons::action_broker::resolve_server_camera(true, None);
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
            if c.attack_words.is_empty()
                && let Some(start) = _play.start.as_ref()
                && crate::fork_ext::addons::native_core::capture_requested()
            {
                c.attack_words = start.attack_words.clone();
            }
        });
        if crate::fork_ext::addons::native_core::capture_requested()
            && let Some(walker) = _walker
        {
            let (proj, view) = walker.view(4.0 / 3.0);
            CACHED_CYCLE_VIEW.with(|c| {
                c.set(Some(CachedCycleView {
                    eye: walker.eye_position(),
                    dir: glam::Vec3::from(walker.eye.direction.to_array()),
                    vp: proj * view,
                }));
            });
        }
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
fn native_action_snapshot(play: &Play) -> crate::fork_ext::addons::action_broker::NativeSnapshot {
    let loot_open = loot_session_open(play);
    if !loot_open {
        LOOT_BUSY.with(|b| b.set(false));
    }
    crate::fork_ext::addons::action_broker::NativeSnapshot {
        in_world: play.in_world_now(),
        mode: play.mode,
        bank_busy: play.bank.state != 0,
        merchant_pending: play.npcw.shop.pending,
        loot_busy: LOOT_BUSY.with(|b| b.get()),
        inventory_busy: play.waiting,
        merchant_open: merchant_session_open(play),
        bank_open: bank_session_open(play),
        loot_open,
        master_loot_active: crate::fork_ext::addons::action_broker::is_master_loot_active(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn lookup_ability<'a>(
    play: &'a Play,
    spec: &crate::fork_ext::addons::action_broker::AbilityLookup,
) -> Option<&'a net::dump::Ability> {
    if let Some(id) = spec.id
        && let Some(a) = play.abilities.get(&id)
    {
        return Some(a);
    }
    if let Some(idx) = spec.index
        && let Some(a) = play.abilities.values().find(|a| a.display_index == idx)
    {
        return Some(a);
    }
    if let Some(name) = &spec.name
        && let Some(a) = play
            .abilities
            .values()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    {
        return Some(a);
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn ee_dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = ee::sub(a[0], b[0]);
    let dy = ee::sub(a[1], b[1]);
    let dz = ee::sub(a[2], b[2]);
    ee::add(ee::add(ee::mul(dx, dx), ee::mul(dy, dy)), ee::mul(dz, dz))
}

#[cfg(not(target_arch = "wasm32"))]
fn dispatch_broker_action(
    play: &mut Play,
    kind: &crate::fork_ext::addons::action_broker::ActionKind,
) -> crate::fork_ext::addons::action_broker::DispatchOutcome {
    use crate::fork_ext::addons::action_broker::{ActionKind, DispatchOutcome, ServerMatcher};
    if !play.in_world_now() {
        return DispatchOutcome::Refused {
            code: "not_in_world",
            reason: "enter a loaded world first".into(),
        };
    }
    match kind {
        ActionKind::Assist => {
            if play.is_dead() {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot assist while dead".into(),
                };
            }
            if !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: format!("cannot assist while in mode {}", play.mode),
                };
            }
            play.assist_target();
            DispatchOutcome::AwaitingServer {
                matcher: ServerMatcher::Assist,
                detail: None,
            }
        }
        ActionKind::Target(opt_id) => {
            if !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: format!("cannot change target while in mode {}", play.mode),
                };
            }
            match opt_id {
                None => {
                    play.target_id(u32::MAX);
                    DispatchOutcome::Completed { detail: None }
                }
                Some(id) => {
                    let exists = play
                        .entities
                        .slots
                        .iter()
                        .flatten()
                        .any(|e| e.id == *id && play.in_scene(e) && (e.p18 & 1 != 0));
                    if !exists {
                        return DispatchOutcome::Refused {
                            code: "not_found",
                            reason: format!("entity {id} is not targetable in scene"),
                        };
                    }
                    play.target_id(*id);
                    DispatchOutcome::Completed { detail: None }
                }
            }
        }
        ActionKind::CycleTarget { group, filter } => {
            if play.is_dead() {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot cycle target while dead".into(),
                };
            }
            if !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: format!("cannot cycle target while in mode {}", play.mode),
                };
            }
            if let Some(kind) = filter.as_deref()
                && kind == "corpse"
            {
                let mut ids: Vec<u32> = play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .filter(|e| play.in_scene(e) && (e.p18 & 1 != 0) && (e.p18 & 2 != 0))
                    .map(|e| e.id)
                    .collect();
                ids.sort_unstable();
                let next = ids
                    .iter()
                    .copied()
                    .find(|id| *id > play.target)
                    .or_else(|| ids.first().copied());
                play.centre_target(next);
                return DispatchOutcome::Completed { detail: None };
            }
            let view = CACHED_CYCLE_VIEW.with(|c| c.get()).unwrap_or_else(|| {
                let eye = play
                    .self_at
                    .map_or(glam::Vec3::ZERO, |s| glam::Vec3::from(s.0));
                let dir = glam::Vec3::NEG_Z;
                let vp = crate::walker::projection_to(4.0 / 3.0, 300.0)
                    * glam::camera::rh::view::look_to_mat4(eye, dir, glam::Vec3::Y);
                CachedCycleView { eye, dir, vp }
            });
            play.cycle_target(*group, view.eye, view.dir, view.vp, view.vp);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Interact => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot interact right now".into(),
                };
            }
            if play.target == u32::MAX
                || !play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .any(|e| e.id == play.target && play.in_scene(e))
            {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "no valid target to interact with".into(),
                };
            }
            play.interact_with_target();
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::OpenLoot { entity_id, keep_target } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) || play.control_client().casting {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot open corpse right now".into(),
                };
            }
            let corpse_id = match entity_id {
                Some(id) => {
                    let Some(ent) = play
                        .entities
                        .slots
                        .iter()
                        .flatten()
                        .find(|e| e.id == *id && play.in_scene(e) && (e.p18 & 1 != 0))
                    else {
                        return DispatchOutcome::Refused {
                            code: "not_found",
                            reason: format!("corpse {id} is not in scene"),
                        };
                    };
                    if ent.p18 & 2 == 0 {
                        return DispatchOutcome::Refused {
                            code: "invalid_argument",
                            reason: format!("entity {id} is not a corpse"),
                        };
                    }
                    ent.id
                }
                None => {
                    let self_pos = play.self_at.map(|(p, _)| p);
                    let mut best: Option<(u32, f32)> = None;
                    for ent in play.entities.slots.iter().flatten() {
                        if !play.in_scene(ent) || (ent.p18 & 1 == 0) || (ent.p18 & 2 == 0) {
                            continue;
                        }
                        let d2 = self_pos.map_or(0.0, |p| ee_dist_sq(p, ent.actor_pos));
                        if best.is_none_or(|(_, best_d2)| ee::lt(d2, best_d2)) {
                            best = Some((ent.id, d2));
                        }
                    }
                    let Some((id, _)) = best else {
                        return DispatchOutcome::Refused {
                            code: "not_found",
                            reason: "no corpse in scene".into(),
                        };
                    };
                    id
                }
            };
            if *keep_target && crate::fork_ext::loot_by_id::use_by_id() {
                // by id: the request names the corpse and nothing else of the player's changes (target, auto-attack)
                let vm = crate::fork_ext::addons::action_broker::current_dispatch_vm().unwrap_or(0);
                let target_was = play.target;
                let _ = crate::fork_ext::addons::window_takeover::arm_pending_open_by_id(vm, corpse_id);
                crate::fork_ext::loot_by_id::note_sent_by_id(vm, corpse_id, target_was);
                tlog!("takeover: mod open_loot BY ID -> corpse {corpse_id:#x}, opcode 0x0015, target left at {target_was:#x}");
                send_loot_request_by_id(play, corpse_id);
                return DispatchOutcome::AwaitingServer {
                    matcher: crate::fork_ext::addons::action_broker::ServerMatcher::OpenLoot { corpse_id },
                    detail: Some(serde_json::json!({ "entity_id": corpse_id, "method": "keep_target" })),
                };
            }
            if play.target != corpse_id {
                let prev = play.target;
                play.target_id(corpse_id);
                if let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() {
                    crate::fork_ext::addons::window_takeover::note_retarget(vm, prev, corpse_id);
                }
            }
            let still_corpse = play.target == corpse_id
                && play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .any(|e| e.id == corpse_id && play.in_scene(e) && (e.p18 & 1 != 0) && (e.p18 & 2 != 0));
            if !still_corpse {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "target is no longer a valid corpse".into(),
                };
            }
            if let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() {
                let _ = crate::fork_ext::addons::window_takeover::arm_pending_open(
                    crate::fork_ext::addons::window_takeover::TakeoverKind::Loot,
                    vm,
                    corpse_id,
                );
            }
            tlog!("takeover: mod open_loot -> corpse {corpse_id:#x}, opcode 0x0015");
            play.attack_press();
            // keep_target asked for, but this session fell back to the old method: say so in the result
            let detail = if *keep_target {
                crate::fork_ext::loot_by_id::note_sent_retarget(
                    crate::fork_ext::addons::action_broker::current_dispatch_vm().unwrap_or(0),
                    corpse_id,
                );
                serde_json::json!({ "entity_id": corpse_id, "method": "retarget" })
            } else {
                serde_json::json!({ "entity_id": corpse_id })
            };
            DispatchOutcome::AwaitingServer {
                matcher: crate::fork_ext::addons::action_broker::ServerMatcher::OpenLoot {
                    corpse_id,
                },
                detail: Some(detail),
            }
        }
        ActionKind::Talk { entity_id, kind } => {
            use crate::fork_ext::addons::action_broker::TalkKind;
            fn talk_target_matches(
                play: &Play,
                e: &net::entity::Entity,
                kind: TalkKind,
            ) -> bool {
                if !play.in_scene(e) || (e.p18 & 1 == 0) || (e.p18 & 2 != 0) {
                    return false;
                }
                let Some(c) = e.creature.as_ref() else {
                    return false;
                };
                if c.p256 == 2 {
                    return false;
                }
                match kind {
                    TalkKind::Merchant => c.npc_flags & 1 != 0,
                    TalkKind::Bank => c.npc_flags & 2 != 0,
                    TalkKind::AnyFriendly => {
                        (c.npc_flags & 0x80 != 0) || (c.npc_flags & 0b1011 != 0)
                    }
                }
            }
            if play.is_dead() || !matches!(play.mode, 0 | 38) || play.control_client().casting {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot interact right now".into(),
                };
            }
            let npc_id = match entity_id {
                Some(id) => {
                    let Some(ent) = play
                        .entities
                        .slots
                        .iter()
                        .flatten()
                        .find(|e| e.id == *id && play.in_scene(e) && (e.p18 & 1 != 0))
                    else {
                        return DispatchOutcome::Refused {
                            code: "not_found",
                            reason: format!("NPC {id} is not in scene"),
                        };
                    };
                    if !talk_target_matches(play, ent, *kind) {
                        return DispatchOutcome::Refused {
                            code: "invalid_argument",
                            reason: format!("entity {id} does not match requested talk target"),
                        };
                    }
                    ent.id
                }
                None => {
                    let self_pos = play.self_at.map(|(p, _)| p);
                    let mut best: Option<(u32, f32)> = None;
                    for ent in play.entities.slots.iter().flatten() {
                        if !talk_target_matches(play, ent, *kind) {
                            continue;
                        }
                        let d2 = self_pos.map_or(0.0, |p| ee_dist_sq(p, ent.actor_pos));
                        if best.is_none_or(|(_, best_d2)| ee::lt(d2, best_d2)) {
                            best = Some((ent.id, d2));
                        }
                    }
                    let Some((id, _)) = best else {
                        return DispatchOutcome::Refused {
                            code: "not_found",
                            reason: "no matching NPC in scene".into(),
                        };
                    };
                    id
                }
            };
            if play.target != npc_id {
                let prev = play.target;
                play.target_id(npc_id);
                if let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() {
                    crate::fork_ext::addons::window_takeover::note_retarget(vm, prev, npc_id);
                }
            }
            let still_valid = play.target == npc_id
                && play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .any(|e| e.id == npc_id && talk_target_matches(play, e, *kind));
            if !still_valid {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "target is no longer a valid NPC".into(),
                };
            }
            let target_flags = play
                .entities
                .slots
                .iter()
                .flatten()
                .find(|e| e.id == npc_id)
                .and_then(|e| e.creature.as_ref())
                .map_or(0, |c| c.npc_flags);
            if let Some(vm) = crate::fork_ext::addons::action_broker::current_dispatch_vm() {
                use crate::fork_ext::addons::window_takeover::{self, TakeoverKind};
                if (*kind == TalkKind::Merchant
                    || (*kind == TalkKind::AnyFriendly && target_flags & 1 != 0))
                    && window_takeover::arm_pending_open(TakeoverKind::Merchant, vm, npc_id)
                {
                    crate::fork_ext::addons::action_broker::note_dispatch_claim(TakeoverKind::Merchant);
                }
                if (*kind == TalkKind::Bank
                    || (*kind == TalkKind::AnyFriendly && target_flags & 2 != 0))
                    && window_takeover::arm_pending_open(TakeoverKind::Bank, vm, npc_id)
                {
                    crate::fork_ext::addons::action_broker::note_dispatch_claim(TakeoverKind::Bank);
                }
            }
            {
                let op = if target_flags & 1 != 0 {
                    0x004C
                } else if target_flags & 2 != 0 {
                    0x124D
                } else if target_flags & 8 != 0 {
                    0x0061
                } else {
                    0x0004
                };
                tlog!(
                    "takeover: mod talk -> npc {npc_id:#x}, npc_flags {target_flags:#04x}, opcode {op:#06x} (the game's own interact path)"
                );
            }
            play.interact_with_target();
            DispatchOutcome::AwaitingServer {
                matcher: crate::fork_ext::addons::action_broker::ServerMatcher::Talk {
                    npc_id,
                    kind: *kind,
                },
                detail: Some(serde_json::json!({ "entity_id": npc_id })),
            }
        }

        ActionKind::Attack { on } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot toggle attack right now".into(),
                };
            }
            let want_on = match on {
                Some(b) => *b,
                None => !play.auto_attack,
            };
            if !want_on {
                play.auto_attack = false;
                play.attack_release();
                return DispatchOutcome::Completed { detail: None };
            }
            let Some(ent) = play
                .entities
                .slots
                .iter()
                .flatten()
                .find(|e| e.id == play.target && play.in_scene(e))
            else {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "no valid target to attack".into(),
                };
            };
            if Some(ent.id) == play.self_id32()
                || (ent.p18 & 2 != 0)
                || ent
                    .creature
                    .as_ref()
                    .is_none_or(|c| (c.npc_flags & 0x80) != 0)
            {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "current target cannot be attacked".into(),
                };
            }
            play.attack_press();
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Cast(lookup) => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot cast right now".into(),
                };
            }
            if play.control_client().casting {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "already casting".into(),
                };
            }
            let Some(ability) = lookup_ability(play, lookup).cloned() else {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "ability not found in spellbook".into(),
                };
            };
            if ability.valid == 0 || ability.slot < 0 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "ability is not memorized".into(),
                };
            }
            if ability.lockout > 0 {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "ability is on cooldown".into(),
                };
            }
            if ability.power_cost > 0 && (play.stats.stat[9] as i64) < ability.power_cost {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "insufficient power".into(),
                };
            }
            if matches!(ability.scope, 1 | 4) {
                let Some(ent) = play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .find(|e| e.id == play.target && play.in_scene(e))
                else {
                    return DispatchOutcome::Refused {
                        code: "not_ready",
                        reason: "ability requires a target in scene".into(),
                    };
                };
                if ability.scope == 4 && (ent.p18 & 2) == 0 {
                    return DispatchOutcome::Refused {
                        code: "not_ready",
                        reason: "ability requires a corpse target".into(),
                    };
                }
                if ability.range > 0.0
                    && let Some((self_pos, _)) = play.self_at
                {
                    let d2 = ee_dist_sq(self_pos, ent.actor_pos);
                    let max_d2 = ee::mul(ability.range, ability.range);
                    if ee::lt(max_d2, d2) {
                        return DispatchOutcome::Refused {
                            code: "not_ready",
                            reason: "target is out of range".into(),
                        };
                    }
                }
            } else if ability.scope == 3 && play.pet_id == u32::MAX {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "ability requires an active pet".into(),
                };
            }
            play.send_0032(ability.id);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::UseHotbar { bar, slot } => {
            if play.is_dead()
                || !matches!(play.mode, 0 | 38)
                || play.typing.is_some()
                || play.map.open
            {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot use hotbar right now".into(),
                };
            }
            if *bar >= play.ability_bars() as usize {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: format!("hotbar {} is not available", bar + 1),
                };
            }
            let menu_idx = [0usize, 1, 4, 5][*bar];
            let entry = play
                .hud_menus
                .defs
                .get(menu_idx)
                .and_then(|d| d.entries.get(*slot))
                .copied()
                .filter(|e| e.msg != 0 && e.id >= 0 && e.icon2 != -1);
            let Some(entry) = entry else {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "hotbar slot is empty".into(),
                };
            };
            if *bar < 2 {
                play.bar_slot_key(*bar, *slot);
            } else {
                play.hud_menus.menus[menu_idx].sel = *slot as i32;
                play.use_hud_entry(entry.msg, entry.id);
            }
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Chat { text } | ActionKind::Command { text } => {
            if crate::fork_ext::addons::gesture::is_forbidden_report_text(text) {
                return DispatchOutcome::Refused {
                    code: "permission_denied",
                    reason: "synthetic report routes (/bug, #bug, /feature, #feature) are forbidden".into(),
                };
            }
            play.chat_line(text);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Autorun { on } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot toggle autorun right now".into(),
                };
            }
            let want = match on {
                Some(b) => *b,
                None => !play.autorun,
            };
            if play.autorun != want {
                play.toggle_autorun();
            }
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Follow { target_id } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot follow right now".into(),
                };
            }
            if let Some(id) = target_id {
                if Some(*id) == play.self_id32() {
                    return DispatchOutcome::Refused {
                        code: "invalid_argument",
                        reason: "cannot follow yourself".into(),
                    };
                }
                let exists = play
                    .entities
                    .slots
                    .iter()
                    .flatten()
                    .any(|e| e.id == *id && play.in_scene(e));
                if !exists {
                    return DispatchOutcome::Refused {
                        code: "not_found",
                        reason: format!("entity {id} is not in scene"),
                    };
                }
                if play.target != *id {
                    play.target_id(*id);
                }
                if play.following && play.follow_id == *id {
                    return DispatchOutcome::Completed { detail: None };
                }
                play.following = false;
                play.do_action(0x0B0E_0007, 0);
                if !play.following {
                    return DispatchOutcome::Refused {
                        code: "not_ready",
                        reason: "target cannot be followed".into(),
                    };
                }
            } else if play.following {
                // `follow(nil)` only stops a follow: the game's toggle turns it off
                play.do_action(0x0B0E_0007, 0);
            }
            // nothing followed: `follow(nil)` does nothing (never the toggle, which would follow the target)
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::SetWaypoint { vm, addon, world, x, z, label } => {
            if !crate::settings::map() {
                return DispatchOutcome::Refused {
                    code: "unavailable",
                    reason: "Map is turned off".into(),
                };
            }
            if let Err((code, reason)) = play.map.api_validate_waypoint(*world, play.world_id(), [*x, *z]) {
                return DispatchOutcome::Refused { code, reason: reason.into() };
            }
            play.map.api_set_waypoint(*world, Some([*x, *z]));
            crate::fork_ext::addons::map_waypoint::placed(*vm, addon, *world, [*x, *z], label.clone());
            DispatchOutcome::Completed {
                detail: Some(serde_json::json!({"world_id": world, "x": x, "z": z})),
            }
        }
        ActionKind::ClearWaypoint { vm } => {
            if !crate::settings::map() {
                return DispatchOutcome::Refused {
                    code: "unavailable",
                    reason: "Map is turned off".into(),
                };
            }
            play.map.api_load_waypoints();
            match crate::fork_ext::addons::map_waypoint::owned_by(*vm, |world| play.map.waypoint(world)) {
                Some(world) => {
                    play.map.api_set_waypoint(world, None);
                    crate::fork_ext::addons::map_waypoint::cleared(world);
                    DispatchOutcome::Completed {
                        detail: Some(serde_json::json!({"world_id": world})),
                    }
                }
                None => DispatchOutcome::Refused {
                    code: "busy",
                    reason: "the waypoint is not this mod's: the player or another mod moved it, or it was reached"
                        .into(),
                },
            }
        }
        ActionKind::Stop => {
            play.auto_attack = false;
            play.attack_release();
            if play.autorun {
                play.toggle_autorun();
            }
            if play.following {
                play.following = false;
                play.follow_id = u32::MAX;
            }
            play.cast_window = (0, 0);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::OpenWindow { kind, arg } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: format!("cannot open window while in mode {}", play.mode),
                };
            }
            match kind.as_str() {
                "summary" | "trainer" | "skills" => {
                    play.trainer_open = true;
                    play.trainer_from_hud = true;
                    play.mode = 12;
                }
                "map" => {
                    if !crate::settings::map() {
                        return DispatchOutcome::Refused {
                            code: "unsupported",
                            reason: "map is disabled in settings".into(),
                        };
                    }
                    play.map.show();
                }
                "abilities" | "spellbook" => play.start_actions(vec![(0x0801_0000, 0)]),
                "quests" => play.start_actions(vec![(0x0B0A_0000, 0)]),
                "mastery" => play.start_actions(vec![(0x0208_0000, 0)]),
                "factions" => play.start_actions(vec![(0xAA00_0001, 0)]),
                "options" => {
                    if play.start.is_none() || !play.hud_ready() {
                        return DispatchOutcome::Refused {
                            code: "not_ready",
                            reason: "options UI assets not loaded".into(),
                        };
                    }
                    play.start_actions(vec![(0x0B0D_0002, 0)]);
                }
                "friends" | "buddies" => play.start_actions(vec![(0x0B05_0000, 0)]),
                "ignore" => play.start_actions(vec![(0x0B05_3000, 0)]),
                "guild" => {
                    if !play.guild.in_guild {
                        return DispatchOutcome::Refused {
                            code: "not_ready",
                            reason: "character is not in a guild".into(),
                        };
                    }
                    play.do_action(0x0B05_1001, 0);
                }
                "mail" => play.start_actions(vec![(0x0B0B_0002, 0)]),
                "mail_compose" => play.start_actions(vec![(0x0B0B_0000, 0)]),
                "tell" => play.start_actions(vec![(0x0B0D_0000, 0)]),
                "reply" => play.start_actions(vec![(0x0B0D_0001, 0)]),
                "auction" | "auction_browse" | "auction_search" => {
                    if !crate::live::features().auction(play.dump.bank.p78cc) {
                        return DispatchOutcome::Refused {
                            code: "not_ready",
                            reason: "auction is not offered by the current server or bank".into(),
                        };
                    }
                    play.do_action(0xA801_0000, 0);
                }
                "support" => play.start_actions(vec![(0x0B0B_1000, 0)]),
                "support_inbox" => play.start_actions(vec![(0x0B0B_1001, 0)]),
                "recipes" => play.start_actions(vec![(0xA701_0000, *arg)]),
                "experiment" => play.start_actions(vec![(0xA702_0000, 0)]),
                "who" => play.do_action(0x0B03_0004, 0),
                "search" => {
                    if !crate::s2::get().player_search {
                        return DispatchOutcome::Refused {
                            code: "unsupported",
                            reason: "server does not offer player search".into(),
                        };
                    }
                    play.do_action(0xFF00_0001, 0);
                }
                "start" | "menu" => {
                    if play.start.is_none() {
                        return DispatchOutcome::Refused {
                            code: "not_ready",
                            reason: "start menu UI assets not loaded".into(),
                        };
                    }
                    play.open_start_menu(true);
                }
                "inventory" | "bags" => {
                    if play.start.is_none() {
                        return DispatchOutcome::Refused {
                            code: "not_ready",
                            reason: "start menu UI assets not loaded".into(),
                        };
                    }
                    play.open_start_menu(true);
                    play.start_key(13, true);
                    play.start_key(13, false);
                }
                _ => {
                    return DispatchOutcome::Refused {
                        code: "invalid_argument",
                        reason: format!("unknown window kind '{kind}'"),
                    };
                }
            }
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::WeaponSet { set } => {
            if !play.gear.on {
                return DispatchOutcome::Refused {
                    code: "unsupported",
                    reason: "server has not enabled weapon sets".into(),
                };
            }
            if *set != crate::gear::BARE_HANDS && play.gear.empty(*set) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: format!("weapon set {} is empty", set + 1),
                };
            }
            if !play.forge_use_weapon_set(*set) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot switch weapon set right now".into(),
                };
            }
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::PrepareWeaponSet { set, name } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot prepare weapon set right now".into(),
                };
            }
            if !play.gear.on {
                return DispatchOutcome::Refused {
                    code: "unsupported",
                    reason: "server has not enabled weapon sets".into(),
                };
            }
            let has_preparable = play
                .dump
                .inventory
                .items
                .iter()
                .any(|it| it.equip_slot >= 0 && crate::gear::preparable(it.item.wear_slot));
            if !has_preparable {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "no preparable weapon is currently equipped".into(),
                };
            }
            play.item_outs(vec![crate::itemmenu::ItemOut::GearSet(*set, name.clone())]);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::AssignAbility { slot, ability } => {
            if play.is_dead() {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot assign abilities while dead".into(),
                };
            }
            if *slot >= 10 && play.ability_bars() <= 2 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "extended ability bars are not active".into(),
                };
            }
            let Some(found) = lookup_ability(play, ability).cloned() else {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "ability not found in spellbook".into(),
                };
            };
            // The Abilities page's Memorize (`SpellBook::popup_message`): an ability on a slot already is refused
            // (the 0x0033 would toggle it off), one the character has not learned (`p14`, set by 0x00FD) is
            // refused; any other goes onto the slot. `valid` is "on a slot", not "may be memorized".
            if found.valid != 0 && found.slot >= 0 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "ability is already memorized".into(),
                };
            }
            if found.valid == 0 && found.p14 == 0 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "ability is not learned".into(),
                };
            }
            play.book_actions(vec![crate::spellbook::Out::Send0033(
                found.display_index,
                *slot,
            )]);
            DispatchOutcome::AwaitingServer {
                matcher: ServerMatcher::AbilitySlot {
                    slot: *slot as u8,
                    index: found.display_index as u8,
                },
                detail: None,
            }
        }
        ActionKind::UnmemorizeAbility { slot, ability } => {
            let found = if let Some(target_slot) = slot {
                play.abilities
                    .values()
                    .find(|a| a.valid != 0 && a.slot == *target_slot)
                    .cloned()
            } else if let Some(lookup) = ability {
                lookup_ability(play, lookup)
                    .filter(|a| a.valid != 0 && a.slot >= 0)
                    .cloned()
            } else {
                None
            };
            let Some(found) = found else {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "ability or slot is not memorized".into(),
                };
            };
            play.book_actions(vec![crate::spellbook::Out::Send0033(
                found.display_index,
                found.slot,
            )]);
            DispatchOutcome::AwaitingServer {
                matcher: ServerMatcher::AbilitySlot {
                    slot: found.slot as u8,
                    index: found.display_index as u8,
                },
                detail: None,
            }
        }
        ActionKind::UnmemorizeAll => {
            if !play.abilities.values().any(|a| a.slot >= 0) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "no abilities are memorized".into(),
                };
            }
            play.book_actions(vec![crate::spellbook::Out::Send00FE]);
            DispatchOutcome::AwaitingServer {
                matcher: ServerMatcher::AbilityClearBar,
                detail: None,
            }
        }
        ActionKind::ArrangeAbility { from, to } => {
            let Some(from_ab) = lookup_ability(play, from).cloned() else {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "source ability not found in spellbook".into(),
                };
            };
            let Some(to_ab) = lookup_ability(play, to).cloned() else {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "destination ability not found in spellbook".into(),
                };
            };
            if from_ab.display_index == to_ab.display_index {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: "source and destination abilities must differ".into(),
                };
            }
            play.book_actions(vec![crate::spellbook::Out::Send003A(
                from_ab.display_index,
                to_ab.display_index,
            )]);
            DispatchOutcome::AwaitingServer {
                matcher: ServerMatcher::AbilityMoveIndex {
                    from: from_ab.display_index as u8,
                    to: to_ab.display_index as u8,
                },
                detail: None,
            }
        }
        ActionKind::UseItem { item } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38 | 13) || play.waiting {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot use inventory items right now".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item handle is stale or item moved".into(),
                };
            }
            let Some((slot_idx, it)) = play
                .dump
                .inventory
                .items
                .iter()
                .enumerate()
                .find(|(_, it)| it.inv_index == item.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer in inventory".into(),
                };
            };
            let pl = &play.dump.player;
            if !crate::inventory::can_use(it, pl.class, pl.race, pl.level) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: cannot_use_reason(it, pl),
                };
            }
            play.use_item(slot_idx as i32, 0);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Equip { item } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38 | 13) || play.waiting {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot equip items right now".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item handle is stale or item moved".into(),
                };
            }
            let Some((slot_idx, it)) = play
                .dump
                .inventory
                .items
                .iter()
                .enumerate()
                .find(|(_, it)| it.inv_index == item.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer in inventory".into(),
                };
            };
            if it.item.wear_slot < 0 {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: "item is not equippable".into(),
                };
            }
            let pl = &play.dump.player;
            if !crate::inventory::can_use(it, pl.class, pl.race, pl.level) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: cannot_use_reason(it, pl),
                };
            }
            play.use_item(slot_idx as i32, 0);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::SwapItems { a, b } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38 | 13) || play.waiting {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot swap inventory items right now".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(a)
                || !crate::fork_ext::addons::native_items::is_handle_fresh(b)
            {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item handle is stale or item moved".into(),
                };
            }
            let Some((ia, ita)) = play
                .dump
                .inventory
                .items
                .iter()
                .enumerate()
                .find(|(_, it)| it.inv_index == a.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "first item is no longer in inventory".into(),
                };
            };
            let Some((ib, itb)) = play
                .dump
                .inventory
                .items
                .iter()
                .enumerate()
                .find(|(_, it)| it.inv_index == b.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "second item is no longer in inventory".into(),
                };
            };
            if ita.equip_slot >= 0 || itb.equip_slot >= 0 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot swap equipped items".into(),
                };
            }
            play.item_outs(vec![crate::itemmenu::ItemOut::Swap(ia as i32, ib as i32)]);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::QuickSlot { item, slot } => {
            if play.is_dead() || !matches!(play.mode, 0 | 38 | 13) || play.waiting {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot assign quick slot right now".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item handle is stale or item moved".into(),
                };
            }
            let Some((idx, it)) = play
                .dump
                .inventory
                .items
                .iter()
                .enumerate()
                .find(|(_, it)| it.inv_index == item.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer in inventory".into(),
                };
            };
            let fits = if play.gear.on {
                (it.item.p4c == 2 && *slot < 2) || (it.item.p4c == 7 && (2..4).contains(slot))
            } else {
                (it.item.p4c == 2 && *slot < 2)
                    || (it.item.p4c == 7 && *slot == 2)
                    || (crate::inventory::fits_slot3(it) && *slot == 3)
            };
            if !fits {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: format!("item does not fit quick slot {}", *slot + 1),
                };
            }
            play.item_outs(vec![crate::itemmenu::ItemOut::QuickSlot(
                idx as i32,
                *slot as i32,
            )]);
            DispatchOutcome::Completed { detail: None }
        }
        ActionKind::Loot {
            item,
            count,
            identity,
        } => {
            if !loot_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            }
            if crate::fork_ext::addons::action_broker::is_master_loot_active() {
                return DispatchOutcome::Refused {
                    code: "permission_denied",
                    reason: "corpse is under master loot rules".into(),
                };
            }
            if LOOT_BUSY.with(|b| b.get()) || PENDING_NPCW_TX.with(|p| p.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "loot transaction already pending".into(),
                };
            }
            crate::fork_ext::addons::native_core::flush_deferred();
            let resolved_storage;
            let item = if crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                item
            } else if let Some(fresh) = identity.as_ref().and_then(|id| {
                crate::fork_ext::addons::native_items::re_resolve_loot_handle(id, *count)
            }) {
                resolved_storage = fresh;
                &resolved_storage
            } else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "loot handle is stale or corpse changed".into(),
                };
            };
            let Some(corpse) = play.npcw.corpse.as_ref() else {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            };
            let Some(it) = corpse.items.iter().find(|it| it.inv_index == item.inv_index) else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer on corpse".into(),
                };
            };
            let n = count.unwrap_or(it.quantity);
            if n < 1 || n > it.quantity {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: format!("invalid loot count {n} (stack has {})", it.quantity),
                };
            }
            if !has_list_room(&play.dump.inventory, it, n, crate::inventory::pack_cap()) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "inventory is full".into(),
                };
            }
            PENDING_NPCW_TX.with(|p| {
                *p.borrow_mut() = Some(PendingNpcwTx::LootOne {
                    key: it.inv_index,
                    count: n as i32,
                });
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::LootOne {
                    key: it.inv_index,
                    item_id: it.item.id,
                    qty_before: it.quantity,
                    count: n,
                },
                detail: Some(serde_json::json!({"count": n})),
            }
        }
        ActionKind::LootAll => {
            if !loot_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            }
            if crate::fork_ext::addons::action_broker::is_master_loot_active() {
                return DispatchOutcome::Refused {
                    code: "permission_denied",
                    reason: "corpse is under master loot rules".into(),
                };
            }
            if LOOT_BUSY.with(|b| b.get()) || PENDING_NPCW_TX.with(|p| p.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "loot transaction already pending".into(),
                };
            }
            let Some(corpse) = play.npcw.corpse.as_ref() else {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            };
            if corpse.items.is_empty() {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: "corpse has no items to loot".into(),
                };
            }
            let item_ids: Vec<i64> = corpse
                .items
                .iter()
                .filter(|it| it.quantity > 0)
                .map(|it| it.item.id)
                .collect();
            PENDING_NPCW_TX.with(|p| {
                *p.borrow_mut() = Some(PendingNpcwTx::LootAll);
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::LootAll { item_ids },
                detail: None,
            }
        }
        ActionKind::ReadyCheckAnswer { id, ready } => {
            match super::raidplay::fork_win::answer_ready(play, *id, *ready) {
                Ok(()) => DispatchOutcome::Completed {
                    detail: Some(serde_json::json!({"id": id, "ready": ready, "sent": true})),
                },
                Err((code, reason)) => DispatchOutcome::Refused { code, reason },
            }
        }
        ActionKind::LootRoll { id, choice } => {
            let c = match *choice {
                1 => net::raid::Choice::Need,
                2 => net::raid::Choice::Greed,
                _ => net::raid::Choice::Pass,
            };
            match super::raidplay::fork_win::answer_roll(play, *id, c) {
                Ok(()) => {
                    let choice_name = match c {
                        net::raid::Choice::Need => "need",
                        net::raid::Choice::Greed => "greed",
                        net::raid::Choice::Pass => "pass",
                    };
                    DispatchOutcome::Completed {
                        detail: Some(serde_json::json!({"id": id, "choice": choice_name, "sent": true})),
                    }
                }
                Err((code, reason)) => DispatchOutcome::Refused { code, reason },
            }
        }
        ActionKind::LootAssign { item, member_id } => {
            if !loot_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            }
            let Some(ml) = crate::fork_ext::addons::raid_strips::master_loot_detail() else {
                return DispatchOutcome::Refused {
                    code: "permission_denied",
                    reason: "corpse is not under master loot rules".into(),
                };
            };
            if LOOT_BUSY.with(|b| b.get()) || PENDING_NPCW_TX.with(|p| p.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "loot transaction already pending".into(),
                };
            }
            crate::fork_ext::addons::native_core::flush_deferred();
            if !crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "loot handle is stale or corpse changed".into(),
                };
            }
            let Some(corpse) = play.npcw.corpse.as_ref() else {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            };
            let Some(it) = corpse.items.iter().find(|it| it.inv_index == item.inv_index) else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer on corpse".into(),
                };
            };
            if !ml.members.iter().any(|(id, _)| *id == *member_id) {
                return DispatchOutcome::Refused {
                    code: "not_found",
                    reason: format!("member {member_id} is not eligible for master loot on this corpse"),
                };
            }
            play.session.send_raid_command(net::raid::Command::Assign {
                key: it.inv_index,
                id: *member_id,
            });
            DispatchOutcome::Completed {
                detail: Some(serde_json::json!({
                    "inventory_key": it.inv_index,
                    "member_id": member_id,
                    "sent": true
                })),
            }
        }
        ActionKind::LootAbandon { item } => {
            if !loot_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "corpse window is not open".into(),
                };
            }
            if crate::fork_ext::addons::raid_strips::master_loot_detail().is_none() {
                return DispatchOutcome::Refused {
                    code: "permission_denied",
                    reason: "corpse is not under master loot rules".into(),
                };
            }
            if LOOT_BUSY.with(|b| b.get()) || PENDING_NPCW_TX.with(|p| p.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "loot transaction already pending".into(),
                };
            }
            if let Some(spec) = item {
                crate::fork_ext::addons::native_core::flush_deferred();
                if !crate::fork_ext::addons::native_items::is_handle_fresh(spec) {
                    return DispatchOutcome::Refused {
                        code: "stale_handle",
                        reason: "loot handle is stale or corpse changed".into(),
                    };
                }
                let Some(corpse) = play.npcw.corpse.as_ref() else {
                    return DispatchOutcome::Refused {
                        code: "window_closed",
                        reason: "corpse window is not open".into(),
                    };
                };
                if !corpse.items.iter().any(|it| it.inv_index == spec.inv_index) {
                    return DispatchOutcome::Refused {
                        code: "stale_handle",
                        reason: "item is no longer on corpse".into(),
                    };
                }
            }
            play.session.send_raid_command(net::raid::Command::Abandon);
            DispatchOutcome::Completed {
                detail: Some(serde_json::json!({"sent": true})),
            }
        }
        ActionKind::Buy { ware, count } => {
            if !merchant_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "merchant window is not open".into(),
                };
            }
            if play.npcw.shop.pending || PENDING_NPCW_TX.with(|p| p.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "merchant transaction already pending".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(ware) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "merchant ware handle is stale".into(),
                };
            }
            let Some((_, it)) = play
                .npcw
                .shop
                .wares
                .items
                .iter()
                .enumerate()
                .find(|(_, it)| it.inv_index == ware.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "ware is no longer offered by merchant".into(),
                };
            };
            if it.item.max_stack == 0 && *count > 1 {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: "non-stackable ware cannot be bought in quantity > 1".into(),
                };
            }
            if it.item.max_stack > 0 && *count > it.item.max_stack {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: format!("count exceeds max stack ({})", it.item.max_stack),
                };
            }
            let unit_price = crate::merchant::merchant_price(it, play.npcw.shop.buy_rate) as i64;
            let total_price = unit_price.saturating_mul(*count);
            if total_price > play.dump.player.p34 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "not enough tunar".into(),
                };
            }
            if !has_list_room(&play.dump.inventory, it, *count, crate::inventory::pack_cap()) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "inventory is full".into(),
                };
            }
            let qty_before: i64 = play
                .dump
                .inventory
                .items
                .iter()
                .filter(|inv_it| inv_it.item.id == it.item.id)
                .map(|inv_it| inv_it.quantity)
                .sum();
            PENDING_NPCW_TX.with(|p| {
                *p.borrow_mut() = Some(PendingNpcwTx::Buy {
                    ware_key: it.inv_index,
                    count: *count as i32,
                });
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::MerchantBuy {
                    item_id: it.item.id,
                    qty_before,
                    count: *count,
                },
                detail: Some(serde_json::json!({"count": count, "total_price": total_price})),
            }
        }
        ActionKind::Sell { item, count } => {
            if !merchant_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "merchant window is not open".into(),
                };
            }
            if play.npcw.shop.pending || PENDING_NPCW_TX.with(|p| p.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "merchant transaction already pending".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item handle is stale or item moved".into(),
                };
            }
            let Some(it) = play
                .dump
                .inventory
                .items
                .iter()
                .find(|it| it.inv_index == item.inv_index)
            else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer in inventory".into(),
                };
            };
            if it.equip_slot >= 0 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot sell an equipped item".into(),
                };
            }
            if *count < 1 || *count > it.quantity {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: format!("sell count {} exceeds stack size {}", count, it.quantity),
                };
            }
            let unit_price = crate::merchant::sell_price(it, play.npcw.shop.sell_rate) as i64;
            let total_price = unit_price.saturating_mul(*count);
            PENDING_NPCW_TX.with(|p| {
                *p.borrow_mut() = Some(PendingNpcwTx::Sell {
                    item_key: it.inv_index,
                    count: *count as i32,
                });
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::MerchantSell {
                    key: it.inv_index,
                    qty_before: it.quantity,
                    count: *count,
                },
                detail: Some(serde_json::json!({"count": count, "total_price": total_price})),
            }
        }
        ActionKind::BankMove {
            from_bank,
            item,
            count,
        } => {
            if !bank_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "bank window is not open".into(),
                };
            }
            if play.bank.state != 0 || PENDING_BANK_TX.with(|b| b.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "bank transaction already pending".into(),
                };
            }
            if !crate::fork_ext::addons::native_items::is_handle_fresh(item) {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item handle is stale or item moved".into(),
                };
            }
            let src_list = if *from_bank {
                &play.dump.bank.items
            } else {
                &play.dump.inventory
            };
            let Some(it) = src_list.items.iter().find(|it| it.inv_index == item.inv_index) else {
                return DispatchOutcome::Refused {
                    code: "stale_handle",
                    reason: "item is no longer in source container".into(),
                };
            };
            if it.equip_slot >= 0 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "cannot deposit an equipped item".into(),
                };
            }
            if *count < 1 || *count > it.quantity {
                return DispatchOutcome::Refused {
                    code: "invalid_argument",
                    reason: format!("bank move count {} exceeds stack size {}", count, it.quantity),
                };
            }
            let (dst_list, cap) = if *from_bank {
                (&play.dump.inventory, crate::inventory::pack_cap())
            } else {
                (&play.dump.bank.items, crate::inventory::bank_cap())
            };
            if !has_list_room(dst_list, it, *count, cap) {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: if *from_bank {
                        "inventory is full".into()
                    } else {
                        "bank is full".into()
                    },
                };
            }
            PENDING_BANK_TX.with(|b| {
                *b.borrow_mut() = Some(PendingBankTx::Move {
                    from_bank: *from_bank,
                    item_key: it.inv_index,
                    count: *count as i32,
                });
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::BankMove {
                    from_bank: *from_bank,
                    key: it.inv_index,
                    item_id: it.item.id,
                    qty_before: it.quantity,
                    count: *count,
                },
                detail: Some(serde_json::json!({"from_bank": from_bank, "count": count})),
            }
        }
        ActionKind::BankMoney { dir, amount } => {
            if !bank_session_open(play) {
                return DispatchOutcome::Refused {
                    code: "window_closed",
                    reason: "bank window is not open".into(),
                };
            }
            if play.bank.state != 0 || PENDING_BANK_TX.with(|b| b.borrow().is_some()) {
                return DispatchOutcome::Refused {
                    code: "busy",
                    reason: "bank transaction already pending".into(),
                };
            }
            if *dir == 1 && *amount > play.dump.player.p34 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "not enough carried tunar to deposit".into(),
                };
            }
            if *dir == 2 && *amount > play.dump.player.p38 {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "not enough bank tunar to withdraw".into(),
                };
            }
            PENDING_BANK_TX.with(|b| {
                *b.borrow_mut() = Some(PendingBankTx::Money {
                    dir: *dir,
                    amount: *amount as i32,
                });
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::BankMoney {
                    dir: *dir,
                    bank_before: play.dump.player.p38,
                    amount: *amount,
                },
                detail: Some(serde_json::json!({"dir": if *dir == 1 { "deposit" } else { "withdraw" }, "amount": amount})),
            }
        }
        ActionKind::Camera {
            mode,
            heading,
            pitch,
            distance,
        } => {
            if !matches!(play.mode, 0 | 38) || play.is_dead() {
                return DispatchOutcome::Refused {
                    code: "not_ready",
                    reason: "camera cannot change in current state".into(),
                };
            }
            PENDING_CAMERA.with(|c| {
                c.set(Some(PendingCameraReq {
                    mode: *mode,
                    heading: *heading,
                    pitch: *pitch,
                    distance: *distance,
                }));
            });
            DispatchOutcome::Staged {
                matcher: ServerMatcher::Camera,
                detail: None,
            }
        }
        ActionKind::Frame(action) => api_capture::a3::dispatch(play, action),
        ActionKind::A1(action) => a1_play::dispatch(play, action),
        ActionKind::A8(action) => a8_play::dispatch(play, action),
        ActionKind::A5(action) => api_capture::a5::dispatch(play, action),
        ActionKind::DropQuest { .. }
        | ActionKind::MasteryShare { .. }
        | ActionKind::MasteryBrowse { .. }
        | ActionKind::MasteryBuy { .. }
        | ActionKind::MasteryRefund { .. }
        | ActionKind::CloseWindow { .. }
        | ActionKind::SelectRow { .. }
        | ActionKind::ActivateRow { .. }
        | ActionKind::DialogAnswer { .. }
        | ActionKind::InputText { .. }
        | ActionKind::MenuCommand { .. } => dispatch_window_action(play, kind),
    }
}

/// Round 6 window and quest actions, dispatched through the native key, edit, class-mastery and
/// quest-journal paths. Windows are revalidated against the live window list (key, incarnation,
/// row/dialog signature) and must hold the input focus; no packet is built here except the
/// game's own `Quests::send_remove` and `Part::{select,browse,send02,send03}` requests.
#[cfg(not(target_arch = "wasm32"))]
fn dispatch_window_action(
    play: &mut Play,
    kind: &crate::fork_ext::addons::action_broker::ActionKind,
) -> crate::fork_ext::addons::action_broker::DispatchOutcome {
    use crate::fork_ext::addons::action_broker::{ActionKind, DispatchOutcome, WinTarget};
    use crate::fork_ext::addons::window_actions::{report_guard, signature};
    use serde_json::{Value as Json, json};
    let refuse = |code: &'static str, reason: &str| DispatchOutcome::Refused { code, reason: reason.to_string() };
    let keys_sent = |n: usize| DispatchOutcome::Completed { detail: Some(json!({"keys": n, "via": "native key presses"})) };
    // the report confirmation and editor never take synthetic input, whatever the mode says
    let report_owned = (play.mode == 14 && play.mbox.p2 == 0xB0B_1002)
        || (play.mode == 2 && play.edit.purpose == 0xB0B_1002)
        || matches!(play.mode, 31 | 32);
    let window_op = !matches!(
        kind,
        ActionKind::DropQuest { .. }
            | ActionKind::MasteryShare { .. }
            | ActionKind::MasteryBrowse { .. }
            | ActionKind::MasteryBuy { .. }
            | ActionKind::MasteryRefund { .. }
    );
    if report_owned && window_op {
        return refuse("permission_denied", "the report window does not take synthetic input");
    }
    // the live window: same key, same incarnation, same rows, holding the focus
    let live = |play: &Play, win: &WinTarget| -> std::result::Result<Json, DispatchOutcome> {
        if !win.key.starts_with("mode:") || win.key.starts_with("mode:other") {
            return Err(refuse("unsupported", "only mode windows take window actions"));
        }
        let state = api_capture::r6a::windows_json(play);
        let Some(w) = state["windows"]
            .as_array()
            .and_then(|l| l.iter().find(|w| w["_key"].as_str() == Some(win.key.as_str())))
            .cloned()
        else {
            return Err(refuse("stale_handle", "the window is closed"));
        };
        let version = crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow().window_versions.get(&win.key).copied());
        if version != Some(win.incarnation) || signature(&w) != win.sig {
            return Err(refuse("stale_handle", "the window changed since the handle was taken"));
        }
        if w["focused"] != true {
            return Err(refuse("busy", "the window does not hold the input focus"));
        }
        if !play.key_queue.is_empty() || play.held.is_some() {
            return Err(refuse("busy", "an earlier key sequence is still being delivered"));
        }
        Ok(w)
    };
    // Windows act only in the exact state their exported rows describe. Where a sub-dialog,
    // page or popup lives in private state this client does not expose (bank page, merchant
    // quantity/MBox/inspect, spellbook popup, boxed quest prompt, mastery focus) the action is
    // refused rather than guessed (see round-6b-seams.md for the read-only accessors it needs).
    let gate = |play: &Play, op: &str| -> std::result::Result<(), DispatchOutcome> {
        let rows_op = matches!(op, "select" | "activate" | "menu");
        let ok = match play.mode {
            1 => rows_op || op == "close",
            13 => rows_op || op == "close",
            7 => true,
            9 | 14 => !rows_op,
            2 => op == "close",
            12 => op == "select",
            _ => false,
        };
        if !ok {
            return Err(refuse("unsupported", "this window's sub-state is not readable, so the action is refused rather than guessed"));
        }
        if matches!(play.mode, 7 | 9 | 14) && play.hud.is_none() {
            return Err(refuse("not_ready", "the dialog is not drawn yet"));
        }
        if play.mode == 1 && (crate::fork_ext::addons::action_broker::is_master_loot_active() || LOOT_BUSY.with(|b| b.get())) {
            return Err(refuse("busy", "a loot request or master-loot choice is open"));
        }
        if play.bank.state != 0 || play.npcw.shop.pending || play.waiting || play.cm.pending != 0 {
            return Err(refuse("busy", "a native transaction is pending"));
        }
        Ok(())
    };
    // the keys that move a list selection from `from` to `to` (1-based), then `then`
    let walk = |from: Option<u64>, to: usize, then: &[i32]| -> std::result::Result<Vec<i32>, DispatchOutcome> {
        let Some(from) = from else {
            return Err(refuse("not_ready", "the window has no current selection to move from"));
        };
        let (from, to) = (from as i64, to as i64);
        let mut keys: Vec<i32> = std::iter::repeat_n(if to > from { 139 } else { 138 }, (to - from).unsigned_abs() as usize).collect();
        keys.extend_from_slice(then);
        Ok(keys)
    };
    match kind {
        ActionKind::DropQuest { index, title, revision, stamp } => {
            let current = crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow().quest_revision);
            let now_title = play
                .quests
                .titles
                .get(index.wrapping_sub(1))
                .map(|t| String::from_utf16_lossy(&t[..t.iter().position(|&c| c == 0).unwrap_or(t.len())]));
            if current != *revision || play.quests.stamp != *stamp || now_title.as_deref() != Some(title.as_str()) {
                return refuse("stale_handle", "the quest journal changed since the handle was taken");
            }
            // round A1: never at once. The question goes to the player; their own Yes sends it (`a1_play::tick`)
            a1_play::ask_drop_quest(*index, title, *stamp, *revision)
        }
        ActionKind::MasteryShare { percent } => {
            let ready = crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow().mastery_flags.share);
            if !ready {
                return refuse("not_ready", "class mastery has not been received yet");
            }
            match play.cm.select(*percent) {
                Some(m) => {
                    play.session.send(&m);
                    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "percent": percent})) }
                }
                None => refuse("busy", "a class mastery request is still pending"),
            }
        }
        ActionKind::MasteryBrowse { skill_id } | ActionKind::MasteryBuy { skill_id } | ActionKind::MasteryRefund { skill_id } => {
            let ready = crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow().mastery_flags.skills);
            // round A5: the game's window, or a mod's "mastery" window session, which has its list fetched
            if !api_capture::a5::mastery_open(play) || !ready {
                return refuse("not_ready", "the class mastery window (the game's, or a mod's window session of kind 'mastery') must be open with its list loaded");
            }
            if play.cm.pending != 0 {
                return refuse("busy", "a class mastery request is still pending");
            }
            let msg = match kind {
                ActionKind::MasteryBrowse { .. } => {
                    if play.cm.find(*skill_id).is_none() {
                        return refuse("not_found", "no such skill in the loaded list");
                    }
                    play.cm.browse(*skill_id)
                }
                _ => {
                    let Some(node) = play.cm.node.clone().filter(|n| n.x08 == *skill_id) else {
                        return refuse("not_ready", "inspect the skill (mastery_browse) before buying or refunding it");
                    };
                    // the live skill list decides owned/known, never the inspected node's copy
                    let Some(skill) = play.cm.find(*skill_id).map(|k| play.cm.skills[k].clone()) else {
                        return refuse("not_found", "no such skill in the loaded list");
                    };
                    if matches!(kind, ActionKind::MasteryRefund { .. }) {
                        if !skill.on {
                            return refuse("not_ready", "the skill is not owned");
                        }
                        // round A5: never at once. The question goes to the player; their own Yes sends it
                        return api_capture::a5::ask_mastery(play, node.x08, true, node.x10, node.x0c);
                    } else {
                        if skill.on || skill.known == 0 {
                            return refuse("not_ready", "the skill is owned or not yet buyable");
                        }
                        if node.x10 == crate::classmastery::MASTER_CLASS && play.cm.refuses_master_class(node.x08, node.x0c) {
                            play.warn(1840);
                            return refuse("not_ready", "another Master Class is already owned");
                        }
                        return api_capture::a5::ask_mastery(play, node.x08, false, node.x10, node.x0c);
                    }
                }
            };
            match msg {
                Some(m) => {
                    play.session.send(&m);
                    DispatchOutcome::Completed { detail: Some(json!({"sent": true, "skill_id": skill_id})) }
                }
                None => refuse("busy", "a class mastery request is still pending"),
            }
        }
        ActionKind::CloseWindow { win } => match live(play, win).and_then(|w| gate(play, "close").map(|_| w)) {
            Ok(_) => {
                play.press(&[132]);
                keys_sent(1)
            }
            Err(e) => e,
        },
        ActionKind::SelectRow { win, row } | ActionKind::ActivateRow { win, row } => {
            let w = match live(play, win) {
                Ok(w) => w,
                Err(e) => return e,
            };
            let activate = matches!(kind, ActionKind::ActivateRow { .. });
            if let Err(e) = gate(play, if activate { "activate" } else { "select" }) {
                return e;
            }
            let r = &w["rows"][*row - 1];
            if play.mode == 12 {
                // the trainer's stats and details step exactly one row per key, clamped; from
                // Train/Details an Up restores the old stat, so those rows are not a start or target
                let (sel, t_mode) = play.trainer.read_state();
                let limit = match t_mode {
                    0 => 7,
                    3 => 9,
                    _ => return refuse("not_ready", "the trainer's cursor is on its buttons"),
                };
                if activate || *row > limit || sel < 0 || sel >= limit as i32 {
                    return refuse("unsupported", "only a stat or detail row can be selected in the trainer");
                }
            }
            if activate && r["enabled"] != true {
                return refuse("not_ready", "the row is not enabled");
            }
            if report_guard(&w, r["command"].as_str(), None).is_some() {
                return refuse("permission_denied", "report routes are forbidden");
            }
            let then: &[i32] = if matches!(kind, ActionKind::ActivateRow { .. }) { &[134] } else { &[] };
            match walk(w["selected_row"].as_u64(), *row, then) {
                Ok(keys) => {
                    play.press(&keys);
                    keys_sent(keys.len())
                }
                Err(e) => e,
            }
        }
        ActionKind::MenuCommand { win, command } => {
            let w = match live(play, win) {
                Ok(w) => w,
                Err(e) => return e,
            };
            if let Err(e) = gate(play, "menu") {
                return e;
            }
            if report_guard(&w, Some(command), None).is_some() {
                return refuse("permission_denied", "report routes are forbidden");
            }
            let Some(row) = w["rows"]
                .as_array()
                .and_then(|r| r.iter().position(|x| x["command"].as_str() == Some(command.as_str()) && x["enabled"] == true))
            else {
                return refuse("unsupported", "the command is not an enabled row of this window");
            };
            match walk(w["selected_row"].as_u64(), row + 1, &[134]) {
                Ok(keys) => {
                    play.press(&keys);
                    keys_sent(keys.len())
                }
                Err(e) => e,
            }
        }
        ActionKind::DialogAnswer { win, option } => {
            let w = match live(play, win) {
                Ok(w) => w,
                Err(e) => return e,
            };
            if let Err(e) = gate(play, "answer") {
                return e;
            }
            // The game's box answers Yes for its OK button and for Yes on a Yes/No box (`MBox::key`):
            // that is accepting, which a mod may do only for the prompts listed as answerable.
            if play.mode == 14 {
                use crate::fork_ext::addons::window_actions::{PROMPT_CONFIRM_CONTRACT, prompt_answerable};
                let accepts = play.mbox.kind == 0 || (play.mbox.kind == 1 && *option == 1);
                if accepts && !prompt_answerable(play.mbox.p1, play.mbox.p2) {
                    return refuse("not_yet", PROMPT_CONFIRM_CONTRACT);
                }
            }
            let keys = match (play.mode, w["dialog"]["kind"].as_str()) {
                (9, _) if *option == 1 => Ok(vec![134]),
                (7, _) => walk(w["selected_row"].as_u64(), *option, &[134]),
                (14, Some("confirm")) => match w["dialog"]["style"].as_str() {
                    Some("yes_no") => match w["dialog"]["selection"].as_u64() {
                        Some(cur) => {
                            let mut k = Vec::new();
                            if cur as usize != *option {
                                k.push(if *option == 1 { 136 } else { 137 });
                            }
                            k.push(134);
                            Ok(k)
                        }
                        None => Err(refuse("not_ready", "the confirm box has no current selection")),
                    },
                    _ => Ok(vec![134]),
                },
                _ => Err(refuse("unsupported", "this dialog takes no answer")),
            };
            match keys {
                Ok(keys) => {
                    play.press(&keys);
                    keys_sent(keys.len())
                }
                Err(e) => e,
            }
        }
        ActionKind::InputText { win, text, submit, grants } => {
            let w = match live(play, win) {
                Ok(w) => w,
                Err(e) => return e,
            };
            if play.mode != 2 || w["dialog"]["kind"] != "input" {
                return refuse("unsupported", "the window has no native edit field");
            }
            if play.edit.password {
                return refuse("permission_denied", "password fields never take synthetic input");
            }
            if report_guard(&w, None, Some(text)).is_some() {
                return refuse("permission_denied", "synthetic report routes are forbidden");
            }
            if crate::editbox::paste_chars(text, true) != *text {
                return refuse("invalid_argument", "the text has characters the game's keyboard cannot type");
            }
            // What the open box sends on submit (`EDIT_BOXES`, `Play::edit_done`) decides what more it needs,
            // judged now, submitted or left to send. The chat editor's line: the Who list's Tell sends
            // `/t <name> <text>`, otherwise the text itself.
            {
                use crate::fork_ext::addons::window_actions::{EditBoxNeed, edit_box_allows, edit_box_need};
                let need = match edit_box_need(play.edit.purpose) {
                    None => return refuse("unsupported", "this text box is not one a mod can fill"),
                    Some(EditBoxNeed::Refused(code)) => return refuse(code, "this text box is never filled by a mod"),
                    Some(need) => need,
                };
                let line = match (&play.who_tell, need) {
                    (Some(name), EditBoxNeed::Line) => format!("/t {name} {text}"),
                    _ => text.clone(),
                };
                if !text.is_empty() && !edit_box_allows(need, *grants, &line) {
                    return refuse(
                        "permission_denied",
                        match need {
                            EditBoxNeed::Chat => "this box sends chat to a channel: requires eqoa.actions.chat",
                            EditBoxNeed::Line if !line.starts_with('/') => "the editor would send chat: requires eqoa.actions.chat",
                            _ => "this box sends a command: requires eqoa.actions.command",
                        },
                    );
                }
            }
            let old = play.edit.text.clone();
            let now = play.session.now() as i64;
            play.edit.clear();
            let typed = play.edit.paste(text, now);
            if typed != text.chars().count() {
                play.edit.set_text(&old);
                return refuse("out_of_range", "the text does not fit the editor's length or rules");
            }
            if *submit {
                let purpose = play.edit.purpose;
                play.edit_done(purpose);
            }
            DispatchOutcome::Completed { detail: Some(json!({"typed": typed, "submitted": submit})) }
        }
        _ => refuse("unsupported", "not a window action"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn cannot_use_reason(it: &net::dump::NetItem, pl: &net::dump::Player) -> String {
    let c = 1i64 << pl.class.clamp(0, 31);
    let r = 1i64 << pl.race.clamp(0, 31);
    if it.item.class_mask & c == 0 {
        "wrong class".into()
    } else if it.item.race_mask & r == 0 {
        "wrong race".into()
    } else if pl.level < it.item.level_req {
        format!("requires level {}", it.item.level_req)
    } else {
        "cannot use item".into()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn has_list_room(
    list: &net::dump::ItemList,
    item: &net::dump::NetItem,
    count: i64,
    cap: usize,
) -> bool {
    if item.item.max_stack > 1
        && list.items.iter().any(|existing| {
            existing.item.id == item.item.id
                && existing.quantity + count <= item.item.max_stack
        })
    {
        return true;
    }
    list.items.len() < cap
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
                    || crate::fork_ext::addons::input::is_text_capture_active()
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
        let timed_build = crate::fork_ext::addons::addon_perf::section("snapshot.build");
        let key = api_capture::updates::key(self, walker);
        if let Some(mut snapshot) = api_capture::updates::cached(key) {
            drop(timed_build);
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

            let cm_flags =
                crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow().mastery_flags);
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
                cms: if cm_flags.available_spent {
                    self.cm.m.x0c.max(0) as u32
                } else {
                    0
                },
                cms_spent: if cm_flags.available_spent {
                    self.cm.m.x10.max(0) as u32
                } else {
                    0
                },
                cm_pct: if cm_flags.share {
                    self.cm.m.x08.clamp(0, 100) as u32
                } else {
                    0
                },
                cm_available: cm_flags.available_spent,
                cm_pct_available: cm_flags.share,
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
                || crate::fork_ext::addons::input::is_text_capture_active()
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
        let layer_dirty = crate::fork_ext::addons::hud_layer::take_dirty();
        hud_changed || vis_changed || layer_dirty || options_closed || step || loading_changed
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
    pub fn entity_name(&self, id: u32) -> Option<String> {
        if id == 0 || id == u32::MAX {
            return None;
        }
        self.entities
            .slots
            .iter()
            .flatten()
            .find(|e| e.id == id)
            .and_then(|e| e.creature.as_ref())
            .map(|c| c.name.clone())
    }
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
    /// Hide UI (`hideui`): a click on the compass opens the map only while
    /// the HUD is shown; hidden, no HUD button is there and the click is the
    /// world's. The control: shown, the same click opens the map.
    #[test]
    fn a_hidden_hud_does_not_open_the_map_from_the_compass() {
        let mut p = super::super::commands::play();
        p.hud = Some((
            crate::ui::Vui::new(Default::default(), Default::default(), Vec::new()),
            crate::ui::UiAssets::default(),
            crate::hud::Hud::new(),
        ));
        let compass = crate::mouse::Click::Target(39.0, 54.0);
        crate::hideui::toggle();
        assert!(crate::hideui::hidden());
        assert!(!p.point(compass, (640, 448), false), "hidden: the world's click");
        assert!(!p.map.open);
        crate::hideui::toggle();
        assert!(p.point(compass, (640, 448), false), "the control: shown, the HUD's");
        assert!(p.map.open, "and the compass opens the map");
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
    /// The real path: a player's F3 filing on a server that offered bug
    /// reports sends the note first, and it names EQOA Enhanced. The control:
    /// a server that offered none gets no note at all.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn the_note_on_the_wire_is_labelled() {
        let mut p = play();
        p.session.record = Some(Vec::new());
        let notes = |p: &mut crate::play::Play| -> Vec<String> {
            p.session
                .record
                .as_mut()
                .map(std::mem::take)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, d)| d)
                .filter(|d| d[..2] == [0x0F, 0x7E])
                .map(|d| String::from_utf8_lossy(&d[10..]).into_owned())
                .collect()
        };
        p.send_bug_note();
        assert!(notes(&mut p).is_empty(), "control: no offer, no note");
        p.session.bug_reports = true;
        p.send_bug_note();
        let sent = notes(&mut p);
        assert_eq!(sent.len(), 1);
        assert!(
            sent[0].starts_with("build: ")
                && sent[0].contains(&format!(
                    "\nclient: {}\n",
                    crate::fork_ext::report_label::enhanced_label()
                )),
            "{}",
            sent[0]
        );
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
            ability: false,
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

        // Simulate opening Start Menu -> Options -> Extended -> Enhanced (Play.mode == 34),
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

        crate::fork_ext::addons::input::testing_text_capture(true);
        assert!(
            p.forge_snapshot(None).ui_busy,
            "mod text capture must report ui_busy == true"
        );
        crate::fork_ext::addons::input::reset_input_state();

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

    #[test]
    fn round7e_mgr2_the_manager_key_is_blocked_while_loading_and_while_the_map_is_up() {
        let mut p = play();
        // a fresh play is on the loading screen
        assert!(p.loading.up());
        assert!(p.manager_key_blocked(), "no manager window over the loading screen");
        let _ = p.loading.wait.take();
        assert!(!p.manager_key_blocked(), "in the world it is usable");
        p.map.show();
        assert!(p.manager_key_blocked(), "the full map owns the screen: the window would open unseen");
        // an ordinary mod key's rule is not changed by this
        let ordinary = p.addon_input_available();
        assert!(ordinary, "the map does not stop ordinary mod keys (unchanged)");
        p.mode = 34;
        assert!(!p.addon_input_available(), "a game window still does (unchanged)");
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Play {
    /// The Mods & Addons key works only where its window would be shown and usable: not on the
    /// loading screen, not with the full map up (the window would open unseen behind it), not
    /// while the world changes. (Character select, and a game window or typing, are not plain play
    /// at all.) This does NOT change `addon_input_available`, which ordinary mod keys use.
    pub(crate) fn manager_key_blocked(&mut self) -> bool {
        self.loading.up() || self.map.open || self.world_change.is_some()
    }
    pub(crate) fn addon_input_available(&mut self) -> bool {
        matches!(self.mode, 0 | 38)
            && !self.window_up()
            && !self.search.open
            && !self.info_wanted()
            && self.info_window.open <= 0.0
            && !self.keyboard_typing()
            && !self.is_dead()
    }
    pub(crate) fn draw_map_addon_hud(&self, ui: &mut render::ui::Ui) {
        if !self.map.open || self.loading.up() {
            return;
        }
        if let Some((vui, assets, _)) = &self.hud {
            let mut r = assets.raster(ui);
            crate::fork_ext::addons::hud_layer::draw(vui, &mut r);
        }
    }
}

#[cfg(test)]
impl Play {
    pub(crate) fn test_map_search_surface(&mut self, open: bool) {
        self.map.open = open;
    }
}

