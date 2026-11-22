// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Action broker: the per-mod action queue, 30 Hz rate limiting, round-robin
//! fairness, per-subsystem locks, `action.result` lifecycle, and server reply
//! correlation.

use super::{
    gesture,
    host::Host,
    native::{bind, describe, error, record, require_capability, session},
    native_core::CORE,
    native_items::{LootItemIdentity, ResolvedItemSpec, item_identity_of, resolve_item_handle},
};
use mlua::{AnyUserData, Lua, MetaMethod, Result, UserData, UserDataMethods, Value};
use serde_json::{Value as Json, json};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
};

pub(crate) const MAX_QUEUED_PER_VM: usize = 32;
pub(crate) const MAX_DISPATCH_PER_TICK_PER_VM: usize = 8;
pub(crate) const MAX_DISPATCH_PER_TICK_GLOBAL: usize = 64;
pub(crate) const TOKEN_BUCKET_BURST: u32 = 8;
pub(crate) const TOKEN_BUCKET_REFILL_PER_TICK: u32 = 1; // 30 ticks/s * 1 = 30/s
pub(crate) const RATE_LIMIT_WAIT_TICKS: u64 = 30; // 1 s on the 30 Hz clock
pub(crate) const SERVER_TIMEOUT_TICKS: u64 = 300; // 10 s on the 30 Hz clock
pub(crate) const MAX_RETAINED_RESULTS_PER_VM: usize = 256;
pub(crate) const MAX_BATCH_STEPS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Subsystem {
    Bank,
    Merchant,
    Loot,
    Inventory,
}

impl Subsystem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Bank => "bank",
            Self::Merchant => "merchant",
            Self::Loot => "loot",
            Self::Inventory => "inventory",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Bank => 1 << 0,
            Self::Merchant => 1 << 1,
            Self::Loot => 1 << 2,
            Self::Inventory => 1 << 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestId {
    pub vm: u64,
    pub id: u64,
}

impl UserData for RequestId {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other.borrow::<RequestId>().is_ok_and(|o| *this == *o))
        });
        m.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(format!("RequestId({})", this.id))
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BatchId {
    pub vm: u64,
    pub id: u64,
}

impl UserData for BatchId {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other.borrow::<BatchId>().is_ok_and(|o| *this == *o))
        });
        m.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(format!("BatchId({})", this.id))
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AbilityLookup {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub index: Option<i64>,
}

/// The window a window action was queued for: its key, incarnation and row/dialog signature
/// (`window_actions::signature`), all revalidated at dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WinTarget {
    pub key: String,
    pub incarnation: u64,
    pub sig: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TalkKind {
    AnyFriendly,
    Merchant,
    Bank,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ActionKind {
    Assist,
    Target(Option<u32>),
    CycleTarget { group: bool, filter: Option<String> },
    Interact,
    Attack { on: Option<bool> },
    Cast(AbilityLookup),
    UseHotbar { bar: usize, slot: usize },
    Chat { text: String },
    Command { text: String },
    Autorun { on: Option<bool> },
    Follow { target_id: Option<u32> },
    Stop,
    OpenWindow { kind: String, arg: i32 },
    WeaponSet { set: i64 },
    PrepareWeaponSet { set: i32, name: String },
    AssignAbility { slot: i64, ability: AbilityLookup },
    UnmemorizeAbility { slot: Option<i64>, ability: Option<AbilityLookup> },
    UnmemorizeAll,
    ArrangeAbility { from: AbilityLookup, to: AbilityLookup },
    UseItem { item: ResolvedItemSpec },
    Equip { item: ResolvedItemSpec },
    SwapItems { a: ResolvedItemSpec, b: ResolvedItemSpec },
    QuickSlot { item: ResolvedItemSpec, slot: u8 },
    Loot { item: ResolvedItemSpec, count: Option<i64>, identity: Option<LootItemIdentity> },
    LootAll,
    Buy { ware: ResolvedItemSpec, count: i64 },
    Sell { item: ResolvedItemSpec, count: i64 },
    BankMove { from_bank: bool, item: ResolvedItemSpec, count: i64 },
    BankMoney { dir: u8, amount: i64 },
    Camera { mode: Option<i32>, heading: Option<f32>, pitch: Option<f32>, distance: Option<f32> },
    DropQuest { index: usize, title: String, revision: u64, stamp: u32 },
    MasteryShare { percent: i32 },
    MasteryBrowse { skill_id: i32 },
    MasteryBuy { skill_id: i32 },
    MasteryRefund { skill_id: i32 },
    CloseWindow { win: WinTarget },
    SelectRow { win: WinTarget, row: usize },
    ActivateRow { win: WinTarget, row: usize },
    DialogAnswer { win: WinTarget, option: usize },
    /// `grants`: whether the mod holds `eqoa.actions.chat` and `eqoa.actions.command`, taken when queued, so
    /// dispatch can judge the line the chat editor will send then (H1's rule, fix round 3 item 3).
    InputText { win: WinTarget, text: String, submit: bool, grants: LineGrants },
    MenuCommand { win: WinTarget, command: String },
    /// `keep_target`: ask by the corpse's id and leave the player's target alone (`loot_by_id`).
    OpenLoot { entity_id: Option<u32>, keep_target: bool },
    Talk { entity_id: Option<u32>, kind: TalkKind },
    /// `eqoa.map.set_waypoint`: the player's own waypoint of `world` moved to (`x`, `z`) by mod `addon`.
    SetWaypoint { vm: u64, addon: String, world: i32, x: f32, z: f32, label: Option<String> },
    /// `eqoa.map.clear_waypoint`: VM `vm`'s own waypoint taken away.
    ClearWaypoint { vm: u64 },
    /// Round A3 (`frame_actions`): invite the target, the pet's commands, an emote, a jump, a buff taken off.
    Frame(super::frame_actions::FrameAction),
}

impl ActionKind {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::Assist => "assist",
            Self::Target(_) => "target",
            Self::CycleTarget { .. } => "cycle_target",
            Self::Interact => "interact",
            Self::Attack { .. } => "attack",
            Self::Cast(_) => "cast",
            Self::UseHotbar { .. } => "use_hotbar",
            Self::Chat { .. } => "chat",
            Self::Command { .. } => "command",
            Self::Autorun { .. } => "autorun",
            Self::Follow { .. } => "follow",
            Self::Stop => "stop",
            Self::OpenWindow { .. } => "open_window",
            Self::WeaponSet { .. } => "weapon_set",
            Self::PrepareWeaponSet { .. } => "prepare_weapon_set",
            Self::AssignAbility { .. } => "assign_ability",
            Self::UnmemorizeAbility { .. } => "unmemorize_ability",
            Self::UnmemorizeAll => "unmemorize_all",
            Self::ArrangeAbility { .. } => "arrange_ability",
            Self::UseItem { .. } => "use_item",
            Self::Equip { .. } => "equip",
            Self::SwapItems { .. } => "swap_items",
            Self::QuickSlot { .. } => "quick_slot",
            Self::Loot { .. } => "loot",
            Self::LootAll => "loot_all",
            Self::Buy { .. } => "buy",
            Self::Sell { .. } => "sell",
            Self::BankMove { .. } => "bank_move",
            Self::BankMoney { .. } => "bank_money",
            Self::Camera { .. } => "camera",
            Self::DropQuest { .. } => "drop_quest",
            Self::MasteryShare { .. } => "mastery_share",
            Self::MasteryBrowse { .. } => "mastery_browse",
            Self::MasteryBuy { .. } => "mastery_buy",
            Self::MasteryRefund { .. } => "mastery_refund",
            Self::CloseWindow { .. } => "close_window",
            Self::SelectRow { .. } => "select_row",
            Self::ActivateRow { .. } => "activate_row",
            Self::DialogAnswer { .. } => "dialog_answer",
            Self::InputText { .. } => "input_text",
            Self::MenuCommand { .. } => "menu_command",
            Self::OpenLoot { .. } => "open_loot",
            Self::Talk { .. } => "talk",
            Self::SetWaypoint { .. } => "set_waypoint",
            Self::ClearWaypoint { .. } => "clear_waypoint",
            Self::Frame(action) => action.name(),
        }
    }

    pub(crate) const fn subsystem(&self) -> Option<Subsystem> {
        match self {
            Self::UseItem { .. }
            | Self::Equip { .. }
            | Self::SwapItems { .. }
            | Self::QuickSlot { .. } => Some(Subsystem::Inventory),
            Self::Loot { .. } | Self::LootAll => Some(Subsystem::Loot),
            Self::Buy { .. } | Self::Sell { .. } => Some(Subsystem::Merchant),
            Self::BankMove { .. } | Self::BankMoney { .. } => Some(Subsystem::Bank),
            Self::Assist
            | Self::Target(_)
            | Self::CycleTarget { .. }
            | Self::Interact
            | Self::Attack { .. }
            | Self::Cast(_)
            | Self::UseHotbar { .. }
            | Self::Chat { .. }
            | Self::Command { .. }
            | Self::Autorun { .. }
            | Self::Follow { .. }
            | Self::Stop
            | Self::OpenWindow { .. }
            | Self::WeaponSet { .. }
            | Self::PrepareWeaponSet { .. }
            | Self::AssignAbility { .. }
            | Self::UnmemorizeAbility { .. }
            | Self::UnmemorizeAll
            | Self::ArrangeAbility { .. }
            | Self::Camera { .. }
            | Self::DropQuest { .. }
            | Self::MasteryShare { .. }
            | Self::MasteryBrowse { .. }
            | Self::MasteryBuy { .. }
            | Self::MasteryRefund { .. }
            | Self::CloseWindow { .. }
            | Self::SelectRow { .. }
            | Self::ActivateRow { .. }
            | Self::DialogAnswer { .. }
            | Self::InputText { .. }
            | Self::MenuCommand { .. }
            | Self::OpenLoot { .. }
            | Self::Talk { .. }
            | Self::SetWaypoint { .. }
            | Self::ClearWaypoint { .. }
            | Self::Frame(_) => None,
        }
    }

    pub(crate) const fn requires_in_world(&self) -> bool {
        let _ = self;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ServerMatcher {
    Assist,
    AbilitySlot { slot: u8, index: u8 },
    AbilityClearBar,
    AbilityMoveIndex { from: u8, to: u8 },
    LootOne { key: u32, item_id: i64, qty_before: i64, count: i64 },
    LootAll { item_ids: Vec<i64> },
    MerchantBuy { item_id: i64, qty_before: i64, count: i64 },
    MerchantSell { key: u32, qty_before: i64, count: i64 },
    BankMove { from_bank: bool, key: u32, item_id: i64, qty_before: i64, count: i64 },
    BankMoney { dir: u8, bank_before: i64, amount: i64 },
    Camera,
    OpenLoot { corpse_id: u32 },
    Talk { npc_id: u32, kind: TalkKind },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PreEventSnapshot {
    pub target: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NativeSnapshot {
    pub in_world: bool,
    pub mode: i32,
    pub bank_busy: bool,
    pub merchant_pending: bool,
    pub loot_busy: bool,
    pub inventory_busy: bool,
    pub merchant_open: bool,
    pub bank_open: bool,
    pub loot_open: bool,
    pub master_loot_active: bool,
}

impl NativeSnapshot {
    fn subsystem_busy(&self, sub: Subsystem) -> bool {
        match sub {
            Subsystem::Bank => self.bank_busy,
            Subsystem::Merchant => self.merchant_pending,
            Subsystem::Loot => self.loot_busy,
            Subsystem::Inventory => self.inventory_busy,
        }
    }
}

/// The line grants a mod held when it queued text for a chat editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineGrants {
    pub chat: bool,
    pub command: bool,
}

impl LineGrants {
    /// Whether these grants cover a line the game sends: a slash line is a command, any other line chat.
    pub(crate) fn allow(self, line: &str) -> bool {
        if super::chat::line_capability(line) == "eqoa.actions.command" { self.command } else { self.chat }
    }
}

pub(crate) enum DispatchOutcome {
    Completed {
        detail: Option<Json>,
    },
    AwaitingServer {
        matcher: ServerMatcher,
        detail: Option<Json>,
    },
    /// Request was staged for a native UI/queue consumer (`npcw.rs`, `bankplay.rs`, `apply_camera`).
    /// It transitions to `"dispatched"` (`native_sent = true`) only in the frame the native consumer actually sends it.
    Staged {
        matcher: ServerMatcher,
        detail: Option<Json>,
    },
    Refused {
        code: &'static str,
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ActionResultRecord {
    pub id: u64,
    pub vm: u64,
    pub kind: &'static str,
    pub state: &'static str,
    pub phase: &'static str,
    pub ok: Option<bool>,
    pub code: Option<String>,
    pub reason: Option<String>,
    pub queued_tick: u64,
    pub dispatched_tick: Option<u64>,
    pub completed_tick: Option<u64>,
    pub detail: Option<Json>,
}

#[derive(Clone, Debug)]
struct QueuedAction {
    id: u64,
    vm: u64,
    #[allow(dead_code)]
    addon_id: String,
    kind: ActionKind,
    queued_tick: u64,
}

#[derive(Clone, Debug)]
struct InFlightAction {
    id: u64,
    vm: u64,
    kind: &'static str,
    subsystem: Option<Subsystem>,
    matcher: ServerMatcher,
    queued_tick: u64,
    dispatched_tick: u64,
    detail: Option<Json>,
    saw_pending: bool,
    native_sent: bool,
    inv_confirmed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatchWindow {
    Merchant,
    Bank,
    Loot,
}

impl BatchWindow {
    fn is_open(self, snap: &NativeSnapshot) -> bool {
        match self {
            Self::Merchant => snap.merchant_open,
            Self::Bank => snap.bank_open,
            Self::Loot => snap.loot_open,
        }
    }

    const fn closed_reason(self) -> &'static str {
        match self {
            Self::Merchant => "merchant closed",
            Self::Bank => "bank closed",
            Self::Loot => "corpse closed",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BatchResultRecord {
    pub id: u64,
    pub vm: u64,
    pub state: &'static str,
    pub ok: Option<bool>,
    pub completed: usize,
    pub total: usize,
    pub current_step: usize,
    pub code: Option<String>,
    pub reason: Option<String>,
    pub queued_tick: u64,
    pub completed_tick: Option<u64>,
    pub failed_step: Option<usize>,
    pub detail: Option<Json>,
}

#[derive(Clone, Debug)]
pub(crate) struct CorpseReopen {
    pub corpse_id: u32,
    pub asked: bool,
    pub started_tick: u64,
}

#[derive(Clone, Debug)]
struct ActiveBatch {
    id: u64,
    vm: u64,
    steps: VecDeque<ActionKind>,
    total: usize,
    completed: usize,
    current_step: usize,
    waiting_req_id: Option<u64>,
    queued_tick: u64,
    require_window: Option<BatchWindow>,
    rate_wait_start_tick: Option<u64>,
    corpse_target_id: Option<u32>,
    corpse_reopen: Option<CorpseReopen>,
    takeover_kinds: [bool; 3],
    /// Claims a step actually used once dispatched (a `talk` that armed a merchant or bank open): the
    /// game-windows switch stops this batch when it releases one of them (M1).
    used_claims: [bool; 3],
}

#[derive(Default)]
struct VmBroker {
    addon_id: String,
    queued: VecDeque<QueuedAction>,
    in_flight: HashMap<u64, InFlightAction>,
    retained: VecDeque<ActionResultRecord>,
    pending_events: VecDeque<ActionResultRecord>,
    active_batch: Option<ActiveBatch>,
    retained_batches: VecDeque<BatchResultRecord>,
    pending_batch_events: VecDeque<(bool, BatchResultRecord)>,
    /// A stopped batch whose step was already on the wire: its terminal result waits for that step's
    /// server reply (request id, record), so the count it reports includes a late success (M2).
    /// Every one still waiting (fix round 3 item 4b): a later stop never drops an earlier one's result.
    stopped_awaiting: Vec<(u64, BatchResultRecord)>,
    tokens: u32,
    last_refill_tick: u64,
}

impl VmBroker {
    fn new(addon_id: String, now_tick: u64) -> Self {
        Self {
            addon_id,
            queued: VecDeque::new(),
            in_flight: HashMap::new(),
            retained: VecDeque::new(),
            pending_events: VecDeque::new(),
            active_batch: None,
            retained_batches: VecDeque::new(),
            pending_batch_events: VecDeque::new(),
            stopped_awaiting: Vec::new(),
            tokens: TOKEN_BUCKET_BURST,
            last_refill_tick: now_tick,
        }
    }

    fn upsert_batch_retained(&mut self, record: BatchResultRecord) {
        if let Some(existing) = self.retained_batches.iter_mut().find(|r| r.id == record.id) {
            *existing = record;
            return;
        }
        if self.retained_batches.len() >= MAX_RETAINED_RESULTS_PER_VM {
            self.retained_batches.pop_front();
        }
        self.retained_batches.push_back(record);
    }

    fn finish_batch(
        &mut self,
        state: &'static str,
        ok: Option<bool>,
        code: Option<&str>,
        reason: Option<String>,
        now_tick: u64,
        failed_step: Option<usize>,
        emit_event: bool,
    ) -> Option<BatchResultRecord> {
        let batch = self.active_batch.take()?;
        let rec = BatchResultRecord {
            id: batch.id,
            vm: batch.vm,
            state,
            ok,
            completed: batch.completed,
            total: batch.total,
            current_step: batch.current_step,
            code: code.map(str::to_string),
            reason,
            queued_tick: batch.queued_tick,
            completed_tick: Some(now_tick),
            failed_step,
            detail: None,
        };
        self.upsert_batch_retained(rec.clone());
        if emit_event {
            self.pending_batch_events.push_back((true, rec.clone()));
        }
        for kind in super::window_takeover::TakeoverKind::ALL {
            if batch.takeover_kinds[kind as usize] {
                super::window_takeover::end_session_for_vm(batch.vm, kind);
            }
        }
        Some(rec)
    }

    fn emit_batch_progress(&mut self) {
        let Some(batch) = self.active_batch.as_ref() else {
            return;
        };
        let rec = BatchResultRecord {
            id: batch.id,
            vm: batch.vm,
            state: "running",
            ok: None,
            completed: batch.completed,
            total: batch.total,
            current_step: batch.current_step,
            code: None,
            reason: None,
            queued_tick: batch.queued_tick,
            completed_tick: None,
            failed_step: None,
            detail: None,
        };
        self.upsert_batch_retained(rec.clone());
        self.pending_batch_events.push_back((false, rec));
    }

    fn handle_batch_step_resolution(
        &mut self,
        req_id: u64,
        now_tick: u64,
        state: &'static str,
        ok: Option<bool>,
        code: Option<&str>,
        reason: Option<String>,
    ) {
        if let Some(at) = self.stopped_awaiting.iter().position(|(waiting, _)| *waiting == req_id) {
            let (_, mut rec) = self.stopped_awaiting.swap_remove(at);
            if ok == Some(true) {
                rec.completed += 1;
            }
            rec.completed_tick = Some(now_tick);
            self.upsert_batch_retained(rec.clone());
            self.pending_batch_events.push_back((true, rec));
            return;
        }
        let Some(batch) = self.active_batch.as_mut() else {
            return;
        };
        if batch.waiting_req_id != Some(req_id) {
            return;
        }
        batch.waiting_req_id = None;
        if ok == Some(true) {
            batch.completed += 1;
            if batch.completed >= batch.total {
                self.finish_batch("completed", Some(true), None, None, now_tick, None, true);
            } else {
                batch.current_step += 1;
                self.emit_batch_progress();
            }
        } else {
            let failed_at = batch.current_step;
            let term_state = if state == "unknown" {
                "unknown"
            } else {
                "refused"
            };
            let term_ok = if term_state == "unknown" {
                None
            } else {
                Some(false)
            };
            self.finish_batch(
                term_state,
                term_ok,
                code,
                reason,
                now_tick,
                Some(failed_at),
                true,
            );
        }
    }

    fn refill(&mut self, now_tick: u64) {
        if now_tick > self.last_refill_tick {
            let elapsed = now_tick - self.last_refill_tick;
            let add = (elapsed.saturating_mul(TOKEN_BUCKET_REFILL_PER_TICK as u64))
                .min(TOKEN_BUCKET_BURST as u64) as u32;
            self.tokens = (self.tokens + add).min(TOKEN_BUCKET_BURST);
            self.last_refill_tick = now_tick;
        }
    }

    fn upsert_retained(&mut self, record: ActionResultRecord) {
        if let Some(existing) = self.retained.iter_mut().find(|r| r.id == record.id) {
            *existing = record;
            return;
        }
        if self.retained.len() >= MAX_RETAINED_RESULTS_PER_VM {
            self.retained.pop_front();
        }
        self.retained.push_back(record);
    }
}

struct BrokerState {
    tick: u64,
    next_id: u64,
    vm_order: VecDeque<u64>,
    rr_cursor: usize,
    vms: HashMap<u64, VmBroker>,
    native: NativeSnapshot,
    master_loot_active: bool,
    player_activity: u8,
    expired_staged_kinds: Vec<&'static str>,
}

impl Default for BrokerState {
    fn default() -> Self {
        Self {
            tick: 0,
            next_id: 1,
            vm_order: VecDeque::new(),
            rr_cursor: 0,
            vms: HashMap::new(),
            native: NativeSnapshot::default(),
            master_loot_active: false,
            player_activity: 0,
            expired_staged_kinds: Vec::new(),
        }
    }
}

impl BrokerState {
    fn ensure_vm(&mut self, vm: u64, addon_id: &str) -> &mut VmBroker {
        if !self.vms.contains_key(&vm) {
            self.vm_order.push_back(vm);
            self.vms
                .insert(vm, VmBroker::new(addon_id.to_string(), self.tick));
        }
        let entry = self.vms.get_mut(&vm).expect("vm broker exists");
        if entry.addon_id != addon_id {
            entry.addon_id = addon_id.to_string();
        }
        entry
    }

    fn subsystem_busy(&self, sub: Subsystem) -> bool {
        if self.native.subsystem_busy(sub) {
            return true;
        }
        self.vms.values().any(|vm| {
            vm.queued.iter().any(|q| q.kind.subsystem() == Some(sub))
                || vm.in_flight.values().any(|f| f.subsystem == Some(sub))
                || (sub == Subsystem::Loot
                    && vm
                        .active_batch
                        .as_ref()
                        .is_some_and(|b| b.corpse_reopen.is_some()))
        })
    }

    fn enqueue(
        &mut self,
        vm: u64,
        addon_id: &str,
        kind: ActionKind,
    ) -> std::result::Result<RequestId, (&'static str, String)> {
        if kind.requires_in_world() && !self.native.in_world {
            let core_world = CORE.with(|c| c.borrow().view.in_world);
            if !core_world {
                return Err(("not_in_world", "enter a loaded world first".into()));
            }
        }
        if let Some(sub) = kind.subsystem()
            && self.subsystem_busy(sub)
        {
            return Err((
                "busy",
                format!("{} subsystem already has an active request", sub.label()),
            ));
        }
        let now_tick = self.tick;
        let queued_len = self
            .vms
            .get(&vm)
            .map_or(0, |v| v.queued.len());
        if queued_len >= MAX_QUEUED_PER_VM {
            return Err((
                "busy",
                format!("action queue limit ({MAX_QUEUED_PER_VM}) reached"),
            ));
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let vm_state = self.ensure_vm(vm, addon_id);
        let req = QueuedAction {
            id,
            vm,
            addon_id: addon_id.to_string(),
            kind: kind.clone(),
            queued_tick: now_tick,
        };
        vm_state.queued.push_back(req);
        vm_state.upsert_retained(ActionResultRecord {
            id,
            vm,
            kind: kind.name(),
            state: "queued",
            phase: "queued",
            ok: None,
            code: None,
            reason: None,
            queued_tick: now_tick,
            dispatched_tick: None,
            completed_tick: None,
            detail: None,
        });
        Ok(RequestId { vm, id })
    }

    fn enqueue_batch(
        &mut self,
        vm: u64,
        addon_id: &str,
        steps: Vec<ActionKind>,
        require_window: Option<BatchWindow>,
    ) -> std::result::Result<BatchId, (&'static str, String)> {
        if steps.is_empty() {
            return Err(("invalid_argument", "batch must contain at least one step".into()));
        }
        if steps.len() > MAX_BATCH_STEPS {
            return Err((
                "invalid_argument",
                format!("batch exceeds {MAX_BATCH_STEPS} steps"),
            ));
        }
        if self
            .vms
            .get(&vm)
            .is_some_and(|v| v.active_batch.is_some())
        {
            return Err(("busy", "a batch is already running for this addon".into()));
        }
        let inferred_window = require_window.or_else(|| {
            if steps
                .iter()
                .any(|k| matches!(k, ActionKind::Sell { .. } | ActionKind::Buy { .. }))
            {
                Some(BatchWindow::Merchant)
            } else if steps
                .iter()
                .any(|k| matches!(k, ActionKind::BankMove { .. } | ActionKind::BankMoney { .. }))
            {
                Some(BatchWindow::Bank)
            } else if steps
                .iter()
                .any(|k| matches!(k, ActionKind::Loot { .. } | ActionKind::LootAll))
            {
                Some(BatchWindow::Loot)
            } else {
                None
            }
        });
        let now_tick = self.tick;
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let total = steps.len();
        let mut takeover_kinds = [false; 3];
        if inferred_window == Some(BatchWindow::Loot)
            || steps.iter().any(|k| {
                matches!(
                    k,
                    ActionKind::OpenLoot { .. } | ActionKind::Loot { .. } | ActionKind::LootAll
                )
            })
        {
            takeover_kinds[super::window_takeover::TakeoverKind::Loot as usize] = true;
        }
        if inferred_window == Some(BatchWindow::Merchant)
            || steps
                .iter()
                .any(|k| matches!(k, ActionKind::Buy { .. } | ActionKind::Sell { .. }))
        {
            takeover_kinds[super::window_takeover::TakeoverKind::Merchant as usize] = true;
        }
        if inferred_window == Some(BatchWindow::Bank)
            || steps
                .iter()
                .any(|k| matches!(k, ActionKind::BankMove { .. } | ActionKind::BankMoney { .. }))
        {
            takeover_kinds[super::window_takeover::TakeoverKind::Bank as usize] = true;
        }
        let vm_state = self.ensure_vm(vm, addon_id);
        let batch = ActiveBatch {
            id,
            vm,
            steps: VecDeque::from(steps),
            total,
            completed: 0,
            current_step: 1,
            waiting_req_id: None,
            queued_tick: now_tick,
            require_window: inferred_window,
            rate_wait_start_tick: None,
            corpse_target_id: (inferred_window == Some(BatchWindow::Loot))
                .then(|| CURRENT_CORPSE_TARGET.with(|c| c.get()))
                .flatten(),
            corpse_reopen: None,
            takeover_kinds,
            used_claims: [false; 3],
        };
        vm_state.active_batch = Some(batch);
        vm_state.upsert_batch_retained(BatchResultRecord {
            id,
            vm,
            state: "running",
            ok: None,
            completed: 0,
            total,
            current_step: 1,
            code: None,
            reason: None,
            queued_tick: now_tick,
            completed_tick: None,
            failed_step: None,
            detail: None,
        });
        Ok(BatchId { vm, id })
    }

    fn batch_status(
        &self,
        batch: BatchId,
    ) -> std::result::Result<Option<BatchResultRecord>, (&'static str, String)> {
        let Some(vm_state) = self.vms.get(&batch.vm) else {
            return Ok(None);
        };
        Ok(vm_state
            .retained_batches
            .iter()
            .rev()
            .find(|r| r.id == batch.id)
            .cloned())
    }

    fn stop_batch(
        &mut self,
        vm: u64,
        target_id: Option<u64>,
    ) -> std::result::Result<bool, (&'static str, String)> {
        let now_tick = self.tick;
        let Some(vm_state) = self.vms.get_mut(&vm) else {
            return Ok(false);
        };
        let Some(active) = vm_state.active_batch.as_ref() else {
            return Ok(false);
        };
        if target_id.is_some_and(|want| want != active.id) {
            return Ok(false);
        }
        // A step already sent keeps its in-flight record: its server reply still counts (M2). A step only
        // staged for the native send is discarded here, so nothing later consumes it.
        let mut sent_step = None;
        if let Some(req_id) = active.waiting_req_id {
            match vm_state.in_flight.get(&req_id).map(|f| f.native_sent) {
                Some(true) => sent_step = Some(req_id),
                Some(false) => {
                    let staged = vm_state.in_flight.remove(&req_id).expect("checked above");
                    self.expired_staged_kinds.push(staged.kind);
                    let rec = ActionResultRecord {
                        id: staged.id,
                        vm: staged.vm,
                        kind: staged.kind,
                        state: "cancelled",
                        phase: "dispatch",
                        ok: Some(false),
                        code: Some("cancelled".into()),
                        reason: Some("the batch stopped before this step was sent".into()),
                        queued_tick: staged.queued_tick,
                        dispatched_tick: None,
                        completed_tick: Some(now_tick),
                        detail: staged.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                }
                None => {}
            }
        }
        let rec = vm_state.finish_batch(
            "stopped",
            Some(false),
            Some("cancelled"),
            Some("stopped".into()),
            now_tick,
            None,
            sent_step.is_none(),
        );
        if let (Some(req_id), Some(rec)) = (sent_step, rec) {
            vm_state.stopped_awaiting.push((req_id, rec));
        }
        Ok(true)
    }

    /// Stops every running batch whose mod held, in `released`, a claim on a window that batch works in.
    fn stop_batches_of_released_claims(&mut self, released: &[(u64, super::window_takeover::TakeoverKind)]) {
        let stopping: Vec<u64> = self
            .vms
            .iter()
            .filter(|(vm, state)| {
                state.active_batch.as_ref().is_some_and(|batch| {
                    released.iter().any(|(holder, kind)| {
                        holder == *vm
                            && (batch.takeover_kinds[*kind as usize] || batch.used_claims[*kind as usize])
                    })
                })
            })
            .map(|(vm, _)| *vm)
            .collect();
        for vm in stopping {
            let _ = self.stop_batch(vm, None);
        }
    }

    fn status(&self, req: RequestId) -> std::result::Result<Option<ActionResultRecord>, (&'static str, String)> {
        let Some(vm_state) = self.vms.get(&req.vm) else {
            return Ok(None);
        };
        Ok(vm_state.retained.iter().rev().find(|r| r.id == req.id).cloned())
    }

    /// `Some(true)`: cancelled (it was never sent). `Some(false)`: already sent to the server, which cannot be
    /// undone: nothing is cancelled, the request keeps waiting and its real result is delivered (round 7H,
    /// as a stopped batch's sent step is). `None`: not pending.
    fn cancel(&mut self, req: RequestId) -> std::result::Result<Option<bool>, (&'static str, String)> {
        let now_tick = self.tick;
        let Some(vm_state) = self.vms.get_mut(&req.vm) else {
            return Ok(None);
        };
        if let Some(pos) = vm_state.queued.iter().position(|q| q.id == req.id) {
            let item = vm_state.queued.remove(pos).expect("queued item");
            let rec = ActionResultRecord {
                id: item.id,
                vm: item.vm,
                kind: item.kind.name(),
                state: "cancelled",
                phase: "dispatch",
                ok: Some(false),
                code: Some("cancelled".into()),
                reason: Some("cancelled before dispatch".into()),
                queued_tick: item.queued_tick,
                dispatched_tick: None,
                completed_tick: Some(now_tick),
                detail: None,
            };
            vm_state.upsert_retained(rec.clone());
            vm_state.pending_events.push_back(rec);
            return Ok(Some(true));
        }
        if vm_state.in_flight.get(&req.id).is_some_and(|f| f.native_sent) {
            return Ok(Some(false));
        }
        if let Some(inflight) = vm_state.in_flight.remove(&req.id) {
            // staged, not yet sent: the client still holds it, so it is cancelled
            self.expired_staged_kinds.push(inflight.kind);
            let rec = ActionResultRecord {
                id: inflight.id,
                vm: inflight.vm,
                kind: inflight.kind,
                state: "cancelled",
                phase: "dispatch",
                ok: Some(false),
                code: Some("cancelled".into()),
                reason: Some("cancelled before dispatch".into()),
                queued_tick: inflight.queued_tick,
                dispatched_tick: None,
                completed_tick: Some(now_tick),
                detail: inflight.detail,
            };
            let vm_state = self.vms.get_mut(&req.vm).expect("vm exists");
            vm_state.upsert_retained(rec.clone());
            vm_state.pending_events.push_back(rec);
            return Ok(Some(true));
        }
        Ok(None)
    }

    fn clear_vm(&mut self, vm: u64, reason: &str) {
        let now_tick = self.tick;
        self.vm_order.retain(|v| *v != vm);
        if self.vm_order.is_empty() {
            self.rr_cursor = 0;
        } else {
            self.rr_cursor %= self.vm_order.len();
        }
        let Some(vm_state) = self.vms.get_mut(&vm) else {
            return;
        };
        while let Some(item) = vm_state.queued.pop_front() {
            vm_state.upsert_retained(ActionResultRecord {
                id: item.id,
                vm: item.vm,
                kind: item.kind.name(),
                state: "cancelled",
                phase: "dispatch",
                ok: Some(false),
                code: Some("cancelled".into()),
                reason: Some(reason.into()),
                queued_tick: item.queued_tick,
                dispatched_tick: None,
                completed_tick: Some(now_tick),
                detail: None,
            });
        }
        let inflight_ids: Vec<u64> = vm_state.in_flight.keys().copied().collect();
        let mut cleared_staged: Vec<&'static str> = Vec::new();
        for id in inflight_ids {
            if let Some(inflight) = vm_state.in_flight.remove(&id) {
                if !inflight.native_sent {
                    cleared_staged.push(inflight.kind);
                }
                vm_state.upsert_retained(ActionResultRecord {
                    id: inflight.id,
                    vm: inflight.vm,
                    kind: inflight.kind,
                    state: "cancelled",
                    phase: if inflight.native_sent {
                        "server"
                    } else {
                        "dispatch"
                    },
                    ok: Some(false),
                    code: Some("cancelled".into()),
                    reason: Some(reason.into()),
                    queued_tick: inflight.queued_tick,
                    dispatched_tick: inflight.native_sent.then_some(inflight.dispatched_tick),
                    completed_tick: Some(now_tick),
                    detail: inflight.detail,
                });
            }
        }
        if vm_state.active_batch.is_some() {
            let step = vm_state.active_batch.as_ref().map(|b| b.current_step);
            vm_state.finish_batch(
                "cancelled",
                Some(false),
                Some("cancelled"),
                Some(reason.into()),
                now_tick,
                step,
                false,
            );
        }
        vm_state.pending_events.clear();
        vm_state.pending_batch_events.clear();
        self.expired_staged_kinds.extend(cleared_staged);
    }

    fn reset_session(&mut self, reason: &str) {
        let now_tick = self.tick;
        self.native = NativeSnapshot::default();
        self.player_activity = 0;
        self.expired_staged_kinds.clear();
        for vm_state in self.vms.values_mut() {
            if vm_state.active_batch.is_some() {
                let step = vm_state.active_batch.as_ref().map(|b| b.current_step);
                vm_state.finish_batch(
                    "cancelled",
                    Some(false),
                    Some("cancelled"),
                    Some(reason.into()),
                    now_tick,
                    step,
                    true,
                );
            }
            while let Some(item) = vm_state.queued.pop_front() {
                let rec = ActionResultRecord {
                    id: item.id,
                    vm: item.vm,
                    kind: item.kind.name(),
                    state: "cancelled",
                    phase: "dispatch",
                    ok: Some(false),
                    code: Some("cancelled".into()),
                    reason: Some(reason.into()),
                    queued_tick: item.queued_tick,
                    dispatched_tick: None,
                    completed_tick: Some(now_tick),
                    detail: None,
                };
                vm_state.upsert_retained(rec.clone());
                vm_state.pending_events.push_back(rec);
            }
            let inflight_ids: Vec<u64> = vm_state.in_flight.keys().copied().collect();
            for id in inflight_ids {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
                    let rec = ActionResultRecord {
                        id: inflight.id,
                        vm: inflight.vm,
                        kind: inflight.kind,
                        state: "cancelled",
                        phase: "server",
                        ok: None,
                        code: Some("cancelled".into()),
                        reason: Some(reason.into()),
                        queued_tick: inflight.queued_tick,
                        dispatched_tick: Some(inflight.dispatched_tick),
                        completed_tick: Some(now_tick),
                        detail: inflight.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                    vm_state.handle_batch_step_resolution(
                        id,
                        now_tick,
                        "unknown",
                        None,
                        Some("unknown"),
                        Some("server outcome unknown after 10s".into()),
                    );
                }
            }
            vm_state.tokens = TOKEN_BUCKET_BURST;
            vm_state.last_refill_tick = now_tick;
        }
    }

    fn has_in_flight(&self) -> bool {
        self.vms.values().any(|vm| !vm.in_flight.is_empty())
    }

    fn step_tick(
        &mut self,
        snapshot: NativeSnapshot,
        mut dispatch: impl FnMut(&ActionKind) -> DispatchOutcome,
    ) {
        self.tick = self.tick.wrapping_add(1);
        self.native = snapshot;
        let now_tick = self.tick;
        let mut expired_staged: Vec<&'static str> = Vec::new();

        // 1. Staged contention timeouts (`busy`), server reply timeouts (10 s = 300 ticks -> `unknown`) & closed-window aborts.
        for vm_state in self.vms.values_mut() {
            let timed_out_staged: Vec<u64> = vm_state
                .in_flight
                .values()
                .filter(|f| {
                    !f.native_sent && now_tick.saturating_sub(f.queued_tick) >= RATE_LIMIT_WAIT_TICKS
                })
                .map(|f| f.id)
                .collect();
            for id in timed_out_staged {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
                    expired_staged.push(inflight.kind);
                    let reason = format!(
                        "{} stayed busy before request could be sent",
                        inflight.subsystem.map(Subsystem::label).unwrap_or("native")
                    );
                    let rec = ActionResultRecord {
                        id: inflight.id,
                        vm: inflight.vm,
                        kind: inflight.kind,
                        state: "refused",
                        phase: "dispatch",
                        ok: Some(false),
                        code: Some("busy".into()),
                        reason: Some(reason.clone()),
                        queued_tick: inflight.queued_tick,
                        dispatched_tick: None,
                        completed_tick: Some(now_tick),
                        detail: inflight.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                    vm_state.handle_batch_step_resolution(
                        id,
                        now_tick,
                        "refused",
                        Some(false),
                        Some("busy"),
                        Some(reason),
                    );
                }
            }
            let expired: Vec<u64> = vm_state
                .in_flight
                .values()
                .filter(|f| {
                    f.native_sent && now_tick.saturating_sub(f.dispatched_tick) >= SERVER_TIMEOUT_TICKS
                })
                .map(|f| f.id)
                .collect();
            for id in expired {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
                    if inflight.subsystem == Some(Subsystem::Loot) {
                        EXPIRED_LOOT_TIMEOUT.with(|t| t.set(true));
                    }
                    let rec = ActionResultRecord {
                        id: inflight.id,
                        vm: inflight.vm,
                        kind: inflight.kind,
                        state: "unknown",
                        phase: "server",
                        ok: None,
                        code: Some("unknown".into()),
                        reason: Some("server outcome unknown after 10s".into()),
                        queued_tick: inflight.queued_tick,
                        dispatched_tick: Some(inflight.dispatched_tick),
                        completed_tick: Some(now_tick),
                        detail: inflight.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                    vm_state.handle_batch_step_resolution(
                        id,
                        now_tick,
                        "unknown",
                        None,
                        Some("unknown"),
                        Some("server outcome unknown after 10s".into()),
                    );
                }
            }
            // Record observed native pending flags, and fail unconfirmed if pending cleared without reply.
            let mut unconfirmed: Vec<(u64, &'static str)> = Vec::new();
            for inflight in vm_state.in_flight.values_mut() {
                if !inflight.native_sent {
                    continue;
                }
                match inflight.matcher {
                    ServerMatcher::MerchantBuy { .. } | ServerMatcher::MerchantSell { .. } => {
                        if snapshot.merchant_pending {
                            inflight.saw_pending = true;
                        } else if inflight.saw_pending {
                            unconfirmed.push((inflight.id, "merchant did not confirm"));
                        }
                    }
                    ServerMatcher::BankMove { .. } | ServerMatcher::BankMoney { .. } => {
                        if snapshot.bank_busy {
                            inflight.saw_pending = true;
                        } else if inflight.saw_pending {
                            unconfirmed.push((inflight.id, "bank did not confirm"));
                        }
                    }
                    _ => {}
                }
            }
            for (id, reason) in unconfirmed {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
                    let rec = ActionResultRecord {
                        id: inflight.id,
                        vm: inflight.vm,
                        kind: inflight.kind,
                        state: "unknown",
                        phase: "server",
                        ok: None,
                        code: Some("unknown".into()),
                        reason: Some(reason.into()),
                        queued_tick: inflight.queued_tick,
                        dispatched_tick: Some(inflight.dispatched_tick),
                        completed_tick: Some(now_tick),
                        detail: inflight.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                    vm_state.handle_batch_step_resolution(
                        id,
                        now_tick,
                        "unknown",
                        None,
                        Some("unknown"),
                        Some(reason.into()),
                    );
                }
            }
            let closed: Vec<(u64, &'static str)> = vm_state
                .in_flight
                .values()
                .filter_map(|f| match f.matcher {
                    ServerMatcher::MerchantBuy { .. } | ServerMatcher::MerchantSell { .. }
                        if !snapshot.merchant_open =>
                    {
                        Some((f.id, "merchant closed"))
                    }
                    ServerMatcher::BankMove { .. } | ServerMatcher::BankMoney { .. }
                        if !snapshot.bank_open =>
                    {
                        Some((f.id, "bank closed"))
                    }
                    ServerMatcher::LootOne { .. } | ServerMatcher::LootAll { .. }
                        if !snapshot.loot_open =>
                    {
                        Some((f.id, "corpse closed"))
                    }
                    _ => None,
                })
                .collect();
            for (id, reason) in closed {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
                    let native_sent = inflight.native_sent;
                    if !native_sent {
                        expired_staged.push(inflight.kind);
                    }
                    let state = if native_sent { "unknown" } else { "refused" };
                    let ok = if native_sent { None } else { Some(false) };
                    let code = if native_sent {
                        "unknown"
                    } else {
                        "window_closed"
                    };
                    let rec = ActionResultRecord {
                        id: inflight.id,
                        vm: inflight.vm,
                        kind: inflight.kind,
                        state,
                        phase: if native_sent { "server" } else { "dispatch" },
                        ok,
                        code: Some(code.into()),
                        reason: Some(reason.into()),
                        queued_tick: inflight.queued_tick,
                        dispatched_tick: native_sent.then_some(inflight.dispatched_tick),
                        completed_tick: Some(now_tick),
                        detail: inflight.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                    vm_state.handle_batch_step_resolution(
                        id,
                        now_tick,
                        state,
                        ok,
                        Some(code),
                        Some(reason.into()),
                    );
                }
            }
            let reopen_abort = vm_state
                .active_batch
                .as_ref()
                .and_then(|b| b.corpse_reopen.as_ref().map(|r| (b.current_step, r.clone())));
            if let Some((step, reopen)) = reopen_abort {
                let target_now = CURRENT_CORPSE_TARGET.with(|c| c.get());
                if target_now != Some(reopen.corpse_id) {
                    vm_state.finish_batch(
                        "refused",
                        Some(false),
                        Some("window_closed"),
                        Some("corpse target changed".into()),
                        now_tick,
                        Some(step),
                        true,
                    );
                } else if now_tick.saturating_sub(reopen.started_tick) >= SERVER_TIMEOUT_TICKS {
                    vm_state.finish_batch(
                        "unknown",
                        None,
                        Some("unknown"),
                        Some("corpse did not reopen within 10s".into()),
                        now_tick,
                        Some(step),
                        true,
                    );
                }
            } else if let Some(win) = vm_state.active_batch.as_ref().and_then(|b| b.require_window)
                && !win.is_open(&snapshot)
            {
                let step = vm_state.active_batch.as_ref().map(|b| b.current_step);
                if let Some(req_id) = vm_state.active_batch.as_ref().and_then(|b| b.waiting_req_id)
                    && let Some(inflight) = vm_state.in_flight.remove(&req_id)
                    && !inflight.native_sent
                {
                    expired_staged.push(inflight.kind);
                }
                vm_state.finish_batch(
                    "refused",
                    Some(false),
                    Some("window_closed"),
                    Some(win.closed_reason().into()),
                    now_tick,
                    step,
                    true,
                );
            }
        }

        // 2. Refill token buckets & expire requests queued > 1 s (30 ticks) waiting on rate limit.
        for vm_state in self.vms.values_mut() {
            vm_state.refill(now_tick);
            if vm_state
                .active_batch
                .as_ref()
                .is_some_and(|b| b.waiting_req_id.is_none())
            {
                if vm_state.tokens == 0 {
                    let start = vm_state
                        .active_batch
                        .as_mut()
                        .expect("active batch")
                        .rate_wait_start_tick
                        .get_or_insert(now_tick);
                    if now_tick.saturating_sub(*start) > RATE_LIMIT_WAIT_TICKS {
                        let step = vm_state.active_batch.as_ref().map(|b| b.current_step);
                        vm_state.finish_batch(
                            "refused",
                            Some(false),
                            Some("rate_limited"),
                            Some("batch waited more than 1s for rate limit".into()),
                            now_tick,
                            step,
                            true,
                        );
                    }
                } else if let Some(b) = vm_state.active_batch.as_mut() {
                    b.rate_wait_start_tick = None;
                }
            }
            while let Some(front) = vm_state.queued.front() {
                if now_tick.saturating_sub(front.queued_tick) <= RATE_LIMIT_WAIT_TICKS {
                    break;
                }
                let expired = vm_state.queued.pop_front().expect("front exists");
                let rec = ActionResultRecord {
                    id: expired.id,
                    vm: expired.vm,
                    kind: expired.kind.name(),
                    state: "refused",
                    phase: "dispatch",
                    ok: Some(false),
                    code: Some("rate_limited".into()),
                    reason: Some("action waited more than 1s for rate limit".into()),
                    queued_tick: expired.queued_tick,
                    dispatched_tick: None,
                    completed_tick: Some(now_tick),
                    detail: None,
                };
                vm_state.upsert_retained(rec.clone());
                vm_state.pending_events.push_back(rec);
            }
        }

        // 3. Round-robin dispatch across VMs respecting 8/tick/VM, burst 8 (30/s), and 64 global/tick.
        if self.vm_order.is_empty() {
            return;
        }
        let vm_count = self.vm_order.len();
        let mut global_dispatched = 0usize;
        let mut per_vm_dispatched: HashMap<u64, usize> = HashMap::new();
        let mut batch_dispatched_this_tick = std::collections::HashSet::<u64>::new();

        while global_dispatched < MAX_DISPATCH_PER_TICK_GLOBAL {
            let mut progressed = false;
            for offset in 0..vm_count {
                if global_dispatched >= MAX_DISPATCH_PER_TICK_GLOBAL {
                    break;
                }
                let idx = (self.rr_cursor + offset) % vm_count;
                let vm_id = self.vm_order[idx];
                let vm_sent = per_vm_dispatched.get(&vm_id).copied().unwrap_or(0);
                if vm_sent >= MAX_DISPATCH_PER_TICK_PER_VM {
                    continue;
                }
                let next_req = self.vms.get(&vm_id).and_then(|vm_state| {
                    if vm_state.tokens == 0 || vm_state.queued.is_empty() {
                        None
                    } else {
                        vm_state.queued.front().cloned()
                    }
                });
                if let Some(req) = next_req {
                    // Check live subsystem lock before popping/dispatching.
                    let busy_sub = req.kind.subsystem().filter(|sub| {
                        let sub_inflight = self.vms.values().any(|v| {
                            v.in_flight.values().any(|f| f.subsystem == Some(*sub))
                        });
                        self.native.subsystem_busy(*sub) || sub_inflight
                    });
                    if let Some(sub) = busy_sub {
                        let popped = self
                            .vms
                            .get_mut(&vm_id)
                            .and_then(|v| v.queued.pop_front())
                            .expect("front queued");
                        let rec = ActionResultRecord {
                            id: popped.id,
                            vm: popped.vm,
                            kind: popped.kind.name(),
                            state: "refused",
                            phase: "dispatch",
                            ok: Some(false),
                            code: Some("busy".into()),
                            reason: Some(format!("{} subsystem is busy", sub.label())),
                            queued_tick: popped.queued_tick,
                            dispatched_tick: None,
                            completed_tick: Some(now_tick),
                            detail: None,
                        };
                        let vm_state = self.vms.get_mut(&vm_id).expect("vm exists");
                        vm_state.upsert_retained(rec.clone());
                        vm_state.pending_events.push_back(rec);
                        progressed = true;
                    } else {
                        let popped = self
                            .vms
                            .get_mut(&vm_id)
                            .and_then(|v| v.queued.pop_front())
                            .expect("front queued");

                        let _vm_guard = DispatchVmGuard::enter(vm_id);
                        match dispatch(&popped.kind) {
                            DispatchOutcome::Completed { detail } => {
                                let vm_state = self.vms.get_mut(&vm_id).expect("vm exists");
                                vm_state.tokens = vm_state.tokens.saturating_sub(1);
                                let rec = ActionResultRecord {
                                    id: popped.id,
                                    vm: popped.vm,
                                    kind: popped.kind.name(),
                                    state: "completed",
                                    phase: "dispatch",
                                    ok: Some(true),
                                    code: None,
                                    reason: None,
                                    queued_tick: popped.queued_tick,
                                    dispatched_tick: Some(now_tick),
                                    completed_tick: Some(now_tick),
                                    detail,
                                };
                                vm_state.upsert_retained(rec.clone());
                                vm_state.pending_events.push_back(rec);
                                per_vm_dispatched.insert(vm_id, vm_sent + 1);
                                global_dispatched += 1;
                                progressed = true;
                            }
                            DispatchOutcome::AwaitingServer { matcher, detail } => {
                                let vm_state = self.vms.get_mut(&vm_id).expect("vm exists");
                                vm_state.tokens = vm_state.tokens.saturating_sub(1);
                                let rec = ActionResultRecord {
                                    id: popped.id,
                                    vm: popped.vm,
                                    kind: popped.kind.name(),
                                    state: "dispatched",
                                    phase: "dispatch",
                                    ok: None,
                                    code: None,
                                    reason: None,
                                    queued_tick: popped.queued_tick,
                                    dispatched_tick: Some(now_tick),
                                    completed_tick: None,
                                    detail: detail.clone(),
                                };
                                vm_state.upsert_retained(rec.clone());
                                vm_state.pending_events.push_back(rec);
                                vm_state.in_flight.insert(
                                    popped.id,
                                    InFlightAction {
                                        id: popped.id,
                                        vm: popped.vm,
                                        kind: popped.kind.name(),
                                        subsystem: popped.kind.subsystem(),
                                        matcher,
                                        queued_tick: popped.queued_tick,
                                        dispatched_tick: now_tick,
                                        detail,
                                        saw_pending: false,
                                        native_sent: true,
                                        inv_confirmed: false,
                                    },
                                );
                                per_vm_dispatched.insert(vm_id, vm_sent + 1);
                                global_dispatched += 1;
                                progressed = true;
                            }
                            DispatchOutcome::Staged { matcher, detail } => {
                                let vm_state = self.vms.get_mut(&vm_id).expect("vm exists");
                                vm_state.tokens = vm_state.tokens.saturating_sub(1);
                                let rec = ActionResultRecord {
                                    id: popped.id,
                                    vm: popped.vm,
                                    kind: popped.kind.name(),
                                    state: "queued",
                                    phase: "dispatch",
                                    ok: None,
                                    code: None,
                                    reason: None,
                                    queued_tick: popped.queued_tick,
                                    dispatched_tick: None,
                                    completed_tick: None,
                                    detail: detail.clone(),
                                };
                                vm_state.upsert_retained(rec);
                                vm_state.in_flight.insert(
                                    popped.id,
                                    InFlightAction {
                                        id: popped.id,
                                        vm: popped.vm,
                                        kind: popped.kind.name(),
                                        subsystem: popped.kind.subsystem(),
                                        matcher,
                                        queued_tick: popped.queued_tick,
                                        dispatched_tick: now_tick,
                                        detail,
                                        saw_pending: false,
                                        native_sent: false,
                                        inv_confirmed: false,
                                    },
                                );
                                per_vm_dispatched.insert(vm_id, vm_sent + 1);
                                global_dispatched += 1;
                                progressed = true;
                            }
                            DispatchOutcome::Refused { code, reason } => {
                                let vm_state = self.vms.get_mut(&vm_id).expect("vm exists");
                                let rec = ActionResultRecord {
                                    id: popped.id,
                                    vm: popped.vm,
                                    kind: popped.kind.name(),
                                    state: "refused",
                                    phase: "dispatch",
                                    ok: Some(false),
                                    code: Some(code.into()),
                                    reason: Some(reason),
                                    queued_tick: popped.queued_tick,
                                    dispatched_tick: None,
                                    completed_tick: Some(now_tick),
                                    detail: None,
                                };
                                vm_state.upsert_retained(rec.clone());
                                vm_state.pending_events.push_back(rec);
                                per_vm_dispatched.insert(vm_id, vm_sent + 1);
                                global_dispatched += 1;
                                progressed = true;
                            }
                        }
                    }
                }
                if !batch_dispatched_this_tick.contains(&vm_id)
                    && global_dispatched < MAX_DISPATCH_PER_TICK_GLOBAL
                    && per_vm_dispatched.get(&vm_id).copied().unwrap_or(0)
                        < MAX_DISPATCH_PER_TICK_PER_VM
                {
                    let ready_step = self
                        .vms
                        .get(&vm_id)
                        .and_then(|v| {
                            if v.tokens == 0 {
                                return None;
                            }
                            let b = v.active_batch.as_ref()?;
                            if b.waiting_req_id.is_some() || b.corpse_reopen.is_some() {
                                return None;
                            }
                            b.steps.front().cloned()
                        });
                    if let Some(step_kind) = ready_step {
                        batch_dispatched_this_tick.insert(vm_id);
                        if let Some(sub) = step_kind.subsystem() {
                            let sub_inflight = self.vms.values().any(|v| {
                                v.in_flight.values().any(|f| f.subsystem == Some(sub))
                            });
                            if self.native.subsystem_busy(sub) || sub_inflight {
                                if let Some(vm_state) = self.vms.get_mut(&vm_id) {
                                    let step = vm_state.active_batch.as_ref().map(|b| b.current_step);
                                    vm_state.finish_batch(
                                        "refused",
                                        Some(false),
                                        Some("busy"),
                                        Some(format!("{} subsystem is busy", sub.label())),
                                        now_tick,
                                        step,
                                        true,
                                    );
                                }
                                progressed = true;
                                continue;
                            }
                        }
                        let step_req_id = self.next_id;
                        self.next_id = self.next_id.wrapping_add(1).max(1);
                        DISPATCH_CLAIMS.with(|c| c.set([false; 3]));
                        let outcome = {
                            let _vm_guard = DispatchVmGuard::enter(vm_id);
                            dispatch(&step_kind)
                        };
                        let used = DISPATCH_CLAIMS.with(|c| c.replace([false; 3]));
                        let vm_state = self.vms.get_mut(&vm_id).expect("vm exists");
                        if let Some(batch) = vm_state.active_batch.as_mut() {
                            let _ = batch.steps.pop_front();
                            batch.rate_wait_start_tick = None;
                            for (seen, now) in batch.used_claims.iter_mut().zip(used) {
                                *seen |= now;
                            }
                        }
                        match outcome {
                            DispatchOutcome::Completed { detail } => {
                                vm_state.tokens = vm_state.tokens.saturating_sub(1);
                                let rec = ActionResultRecord {
                                    id: step_req_id,
                                    vm: vm_id,
                                    kind: step_kind.name(),
                                    state: "completed",
                                    phase: "dispatch",
                                    ok: Some(true),
                                    code: None,
                                    reason: None,
                                    queued_tick: now_tick,
                                    dispatched_tick: Some(now_tick),
                                    completed_tick: Some(now_tick),
                                    detail,
                                };
                                vm_state.upsert_retained(rec.clone());
                                vm_state.pending_events.push_back(rec);
                                if let Some(batch) = vm_state.active_batch.as_mut() {
                                    batch.completed += 1;
                                    if batch.completed >= batch.total {
                                        vm_state.finish_batch(
                                            "completed",
                                            Some(true),
                                            None,
                                            None,
                                            now_tick,
                                            None,
                                            true,
                                        );
                                    } else {
                                        batch.current_step += 1;
                                        vm_state.emit_batch_progress();
                                    }
                                }
                            }
                            DispatchOutcome::AwaitingServer { matcher, detail } => {
                                vm_state.tokens = vm_state.tokens.saturating_sub(1);
                                let rec = ActionResultRecord {
                                    id: step_req_id,
                                    vm: vm_id,
                                    kind: step_kind.name(),
                                    state: "dispatched",
                                    phase: "dispatch",
                                    ok: None,
                                    code: None,
                                    reason: None,
                                    queued_tick: now_tick,
                                    dispatched_tick: Some(now_tick),
                                    completed_tick: None,
                                    detail: detail.clone(),
                                };
                                vm_state.upsert_retained(rec.clone());
                                vm_state.pending_events.push_back(rec);
                                vm_state.in_flight.insert(
                                    step_req_id,
                                    InFlightAction {
                                        id: step_req_id,
                                        vm: vm_id,
                                        kind: step_kind.name(),
                                        subsystem: step_kind.subsystem(),
                                        matcher,
                                        queued_tick: now_tick,
                                        dispatched_tick: now_tick,
                                        detail,
                                        saw_pending: false,
                                        native_sent: true,
                                        inv_confirmed: false,
                                    },
                                );
                                if let Some(batch) = vm_state.active_batch.as_mut() {
                                    batch.waiting_req_id = Some(step_req_id);
                                    vm_state.emit_batch_progress();
                                }
                            }
                            DispatchOutcome::Staged { matcher, detail } => {
                                vm_state.tokens = vm_state.tokens.saturating_sub(1);
                                let rec = ActionResultRecord {
                                    id: step_req_id,
                                    vm: vm_id,
                                    kind: step_kind.name(),
                                    state: "queued",
                                    phase: "dispatch",
                                    ok: None,
                                    code: None,
                                    reason: None,
                                    queued_tick: now_tick,
                                    dispatched_tick: None,
                                    completed_tick: None,
                                    detail: detail.clone(),
                                };
                                vm_state.upsert_retained(rec);
                                vm_state.in_flight.insert(
                                    step_req_id,
                                    InFlightAction {
                                        id: step_req_id,
                                        vm: vm_id,
                                        kind: step_kind.name(),
                                        subsystem: step_kind.subsystem(),
                                        matcher,
                                        queued_tick: now_tick,
                                        dispatched_tick: now_tick,
                                        detail,
                                        saw_pending: false,
                                        native_sent: false,
                                        inv_confirmed: false,
                                    },
                                );
                                if let Some(batch) = vm_state.active_batch.as_mut() {
                                    batch.waiting_req_id = Some(step_req_id);
                                    vm_state.emit_batch_progress();
                                }
                            }
                            DispatchOutcome::Refused { code, reason } => {
                                let rec = ActionResultRecord {
                                    id: step_req_id,
                                    vm: vm_id,
                                    kind: step_kind.name(),
                                    state: "refused",
                                    phase: "dispatch",
                                    ok: Some(false),
                                    code: Some(code.into()),
                                    reason: Some(reason.clone()),
                                    queued_tick: now_tick,
                                    dispatched_tick: None,
                                    completed_tick: Some(now_tick),
                                    detail: None,
                                };
                                vm_state.upsert_retained(rec.clone());
                                vm_state.pending_events.push_back(rec);
                                let step = vm_state.active_batch.as_ref().map(|b| b.current_step);
                                vm_state.finish_batch(
                                    "refused",
                                    Some(false),
                                    Some(code),
                                    Some(reason),
                                    now_tick,
                                    step,
                                    true,
                                );
                            }
                        }
                        *per_vm_dispatched.entry(vm_id).or_insert(0) += 1;
                        global_dispatched += 1;
                        progressed = true;
                    }
                }
            }
            self.rr_cursor = (self.rr_cursor + 1) % vm_count;
            if !progressed {
                break;
            }
        }
        self.expired_staged_kinds.extend(expired_staged);
    }

    fn mark_deferred_sent(&mut self, kind_name: &'static str) {
        let now_tick = self.tick;
        for vm_state in self.vms.values_mut() {
            if let Some(inflight) = vm_state
                .in_flight
                .values_mut()
                .find(|f| !f.native_sent && f.kind == kind_name)
            {
                inflight.native_sent = true;
                inflight.dispatched_tick = now_tick;
                let rec = ActionResultRecord {
                    id: inflight.id,
                    vm: inflight.vm,
                    kind: inflight.kind,
                    state: "dispatched",
                    phase: "dispatch",
                    ok: None,
                    code: None,
                    reason: None,
                    queued_tick: inflight.queued_tick,
                    dispatched_tick: Some(now_tick),
                    completed_tick: None,
                    detail: inflight.detail.clone(),
                };
                vm_state.upsert_retained(rec.clone());
                vm_state.pending_events.push_back(rec);
                break;
            }
        }
    }

    fn has_request_in(&self, sub: Subsystem) -> bool {
        self.vms
            .values()
            .any(|v| v.in_flight.values().any(|f| f.subsystem == Some(sub)))
    }

    fn prune_player_activity(&mut self) {
        for sub in [
            Subsystem::Bank,
            Subsystem::Merchant,
            Subsystem::Loot,
            Subsystem::Inventory,
        ] {
            if (self.player_activity & sub.bit()) != 0 && !self.has_request_in(sub) {
                self.player_activity &= !sub.bit();
            }
        }
    }

    /// While the player has used `sub` themselves, a reply cannot be attributed: every sent
    /// broker request there resolves `unknown`. Returns true when it did.
    fn resolve_player_activity(&mut self, sub: Subsystem) -> bool {
        if (self.player_activity & sub.bit()) == 0 {
            return false;
        }
        self.resolve_matching_inner(
            |inflight| {
                if inflight.native_sent && inflight.subsystem == Some(sub) {
                    Some((
                        None,
                        Some("unknown"),
                        Some(format!(
                            "you used the {} window yourself; the result cannot be told apart",
                            sub.label()
                        )),
                    ))
                } else {
                    None
                }
            },
            true,
        );
        true
    }

    fn resolve_matching_sent(
        &mut self,
        matches: impl Fn(&InFlightAction) -> Option<(Option<bool>, Option<&'static str>, Option<String>)>,
    ) {
        self.resolve_matching(|inflight| {
            if !inflight.native_sent {
                return None;
            }
            matches(inflight)
        });
    }

    fn resolve_matching(
        &mut self,
        matches: impl Fn(&InFlightAction) -> Option<(Option<bool>, Option<&'static str>, Option<String>)>,
    ) {
        self.resolve_matching_inner(matches, false);
    }

    fn resolve_matching_inner(
        &mut self,
        matches: impl Fn(&InFlightAction) -> Option<(Option<bool>, Option<&'static str>, Option<String>)>,
        all: bool,
    ) {
        let now_tick = self.tick;
        for vm_state in self.vms.values_mut() {
          loop {
            let mut matched: Option<(u64, Option<bool>, Option<&'static str>, Option<String>)> = None;
            for inflight in vm_state.in_flight.values() {
                if let Some((ok, code, reason)) = matches(inflight) {
                    matched = Some((inflight.id, ok, code, reason));
                    break;
                }
            }
            if let Some((id, ok, code, reason)) = matched {
                let inflight = vm_state.in_flight.remove(&id).expect("matched inflight");
                let state = match ok {
                    Some(true) => "completed",
                    Some(false) => "refused",
                    None => "unknown",
                };
                let rec = ActionResultRecord {
                    id: inflight.id,
                    vm: inflight.vm,
                    kind: inflight.kind,
                    state,
                    phase: if inflight.native_sent {
                        "server"
                    } else {
                        "dispatch"
                    },
                    ok,
                    code: code.map(str::to_string),
                    reason,
                    queued_tick: inflight.queued_tick,
                    dispatched_tick: inflight.native_sent.then_some(inflight.dispatched_tick),
                    completed_tick: Some(now_tick),
                    detail: inflight.detail,
                };
                vm_state.upsert_retained(rec.clone());
                vm_state.pending_events.push_back(rec.clone());
                vm_state.handle_batch_step_resolution(
                    id,
                    now_tick,
                    rec.state,
                    rec.ok,
                    rec.code.as_deref(),
                    rec.reason,
                );
                let finished_takeover = match rec.kind {
                    "loot" | "loot_all" => Some(super::window_takeover::TakeoverKind::Loot),
                    "buy" | "sell" => Some(super::window_takeover::TakeoverKind::Merchant),
                    "bank_move" | "bank_money" => Some(super::window_takeover::TakeoverKind::Bank),
                    _ => None,
                };
                if let Some(kind) = finished_takeover
                    && vm_state.active_batch.is_none()
                    && !vm_state.queued.iter().any(|q| q.kind.subsystem() == inflight.subsystem)
                    && !vm_state.in_flight.values().any(|f| f.subsystem == inflight.subsystem)
                {
                    super::window_takeover::end_session_for_vm(inflight.vm, kind);
                }
                if all {
                    continue;
                }
            }
            break;
          }
        }
        self.prune_player_activity();
    }
}

thread_local! {
    static BROKER: RefCell<BrokerState> = RefCell::new(BrokerState::default());
    static MASTER_LOOT_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CURRENT_CORPSE_TARGET: Cell<Option<u32>> = const { Cell::new(None) };
    static EXPIRED_LOOT_TIMEOUT: Cell<bool> = const { Cell::new(false) };
    static DISPATCHING_VM: Cell<Option<u64>> = const { Cell::new(None) };
    /// Claims the action being dispatched armed (indexed by `TakeoverKind`), read back by a batch step.
    static DISPATCH_CLAIMS: Cell<[bool; 3]> = const { Cell::new([false; 3]) };
}

struct DispatchVmGuard(Option<u64>);
impl DispatchVmGuard {
    fn enter(vm: u64) -> Self {
        let prev = DISPATCHING_VM.with(|d| d.replace(Some(vm)));
        Self(prev)
    }
}
impl Drop for DispatchVmGuard {
    fn drop(&mut self) {
        DISPATCHING_VM.with(|d| d.set(self.0));
    }
}

/// The action being dispatched armed `kind`'s claim (a `talk` reaching a merchant or bank, an `open_loot`).
pub(crate) fn note_dispatch_claim(kind: super::window_takeover::TakeoverKind) {
    DISPATCH_CLAIMS.with(|c| {
        let mut used = c.get();
        used[kind as usize] = true;
        c.set(used);
    });
}

pub(crate) fn current_dispatch_vm() -> Option<u64> {
    DISPATCHING_VM.with(Cell::get)
}

pub(crate) fn set_corpse_target_id(id: Option<u32>) {
    CURRENT_CORPSE_TARGET.with(|c| c.set(id));
}

pub(crate) fn take_loot_timeout_expired() -> bool {
    EXPIRED_LOOT_TIMEOUT.with(|t| t.replace(false))
}

pub(crate) fn update_native_snapshot(snapshot: NativeSnapshot) {
    BROKER.with(|b| b.borrow_mut().native = snapshot);
}

/// The player's own native output was seen for `sub`. Only matters while a broker request of that
/// subsystem is staged or in flight; the flag stays until none is (never per frame or game step).
pub(crate) fn mark_player_activity(sub: Subsystem) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        if b.has_request_in(sub) {
            b.player_activity |= sub.bit();
        }
    });
}

pub(crate) fn is_player_activity(sub: Subsystem) -> bool {
    BROKER.with(|b| (b.borrow().player_activity & sub.bit()) != 0)
}

pub(crate) fn mark_deferred_sent(kind_name: &'static str) {
    BROKER.with(|b| b.borrow_mut().mark_deferred_sent(kind_name));
}

pub(crate) fn take_expired_staged_kinds() -> Vec<&'static str> {
    BROKER.with(|b| std::mem::take(&mut b.borrow_mut().expired_staged_kinds))
}

pub(crate) fn resolve_staged_refusal(kind_name: &'static str, code: &'static str, reason: &str) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            if !inflight.native_sent && inflight.kind == kind_name {
                Some((Some(false), Some(code), Some(reason.to_string())))
            } else {
                None
            }
        });
    });
}

pub(crate) fn step_tick(
    snapshot: NativeSnapshot,
    dispatch: impl FnMut(&ActionKind) -> DispatchOutcome,
) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.step_tick(snapshot, dispatch);
        b.prune_player_activity();
    });
}

pub(crate) fn has_in_flight() -> bool {
    BROKER.with(|b| b.borrow().has_in_flight())
}

pub(crate) fn clear_vm(vm: u64, reason: &str) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.clear_vm(vm, reason);
        b.prune_player_activity();
    });
}

pub(crate) fn reset_session(reason: &str) {
    MASTER_LOOT_ACTIVE.with(|m| m.set(false));
    CURRENT_CORPSE_TARGET.with(|c| c.set(None));
    EXPIRED_LOOT_TIMEOUT.with(|t| t.set(false));
    BROKER.with(|b| b.borrow_mut().reset_session(reason));
}

pub(crate) fn resolve_server_assist(target_id: u32) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            if !matches!(inflight.matcher, ServerMatcher::Assist) {
                return None;
            }
            if target_id == u32::MAX {
                Some((Some(false), Some("refused"), Some("there is no target to assist".into())))
            } else {
                Some((Some(true), None, None))
            }
        });
    });
}

pub(crate) fn resolve_server_ability_slot(index: u8, slot: u8) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            let ServerMatcher::AbilitySlot { slot: expected_slot, index: expected_index } =
                inflight.matcher
            else {
                return None;
            };
            if slot == expected_slot || index == expected_index {
                Some((Some(true), None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_ability_clear() {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            matches!(inflight.matcher, ServerMatcher::AbilityClearBar)
                .then_some((Some(true), None, None))
        });
    });
}

pub(crate) fn set_master_loot(active: bool) {
    MASTER_LOOT_ACTIVE.with(|m| m.set(active));
}

pub(crate) fn is_master_loot_active() -> bool {
    MASTER_LOOT_ACTIVE.with(|m| m.get())
}

pub(crate) fn mark_loot_inv_added(added_item_id: i64) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        for vm in b.vms.values_mut() {
            for inflight in vm.in_flight.values_mut() {
                if !inflight.native_sent {
                    continue;
                }
                match inflight.matcher {
                    ServerMatcher::LootOne { item_id, .. } if item_id == added_item_id => {
                        inflight.inv_confirmed = true;
                    }
                    ServerMatcher::LootAll { ref item_ids }
                        if item_ids.contains(&added_item_id) =>
                    {
                        inflight.inv_confirmed = true;
                    }
                    _ => {}
                }
            }
        }
    });
}

/// For each mod VM, the window kinds (indexed by `TakeoverKind`) whose open it still waits for:
/// an `open_loot` or `talk` request in flight, or a batch's corpse reopen. A pending takeover claim
/// outlives none of these (refused, timed out, stopped).
pub(crate) fn open_requests_waiting() -> Vec<(u64, [bool; 3])> {
    BROKER.with(|b| {
        let b = b.borrow();
        let mut out = Vec::new();
        for (vm, state) in &b.vms {
            let mut kinds = [false; 3];
            for f in state.in_flight.values() {
                match f.matcher {
                    ServerMatcher::OpenLoot { .. } => kinds[0] = true,
                    ServerMatcher::Talk { kind, .. } => match kind {
                        TalkKind::Merchant => kinds[1] = true,
                        TalkKind::Bank => kinds[2] = true,
                        TalkKind::AnyFriendly => {
                            kinds[1] = true;
                            kinds[2] = true;
                        }
                    },
                    _ => {}
                }
            }
            if state
                .active_batch
                .as_ref()
                .is_some_and(|batch| batch.corpse_reopen.is_some())
            {
                kinds[0] = true;
            }
            if kinds.iter().any(|&k| k) {
                out.push((*vm, kinds));
            }
        }
        out
    })
}

/// Nothing of this mod VM's is queued, in flight or running as a batch.
pub(crate) fn vm_is_idle(vm: u64) -> bool {
    BROKER.with(|b| {
        b.borrow().vms.get(&vm).is_none_or(|s| {
            s.queued.is_empty() && s.in_flight.is_empty() && s.active_batch.is_none()
        })
    })
}

/// The mod's `open_loot` for `corpse_id` is still waiting for the server (in flight and sent).
pub(crate) fn open_loot_waiting(vm: u64, corpse_id: u32) -> bool {
    BROKER.with(|b| {
        b.borrow().vms.get(&vm).is_some_and(|v| {
            v.in_flight.values().any(|f| {
                f.native_sent
                    && matches!(f.matcher, ServerMatcher::OpenLoot { corpse_id: c } if c == corpse_id)
            }) || v.active_batch.as_ref().is_some_and(|batch| {
                batch.corpse_reopen.as_ref().is_some_and(|r| r.asked && r.corpse_id == corpse_id)
            })
        })
    })
}

/// Which method the open of `corpse_id` ended up using, written into the in-flight request's `detail.method`, so the
/// result the mod reads says what the client did (`"keep_target"` or, after a fallback, `"retarget"`).
pub(crate) fn set_open_loot_method(corpse_id: u32, method: &str) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        for v in b.vms.values_mut() {
            for f in v.in_flight.values_mut() {
                if matches!(f.matcher, ServerMatcher::OpenLoot { corpse_id: c } if c == corpse_id) {
                    match f.detail.as_mut().and_then(Json::as_object_mut) {
                        Some(o) => {
                            o.insert("method".into(), Json::from(method));
                        }
                        None => f.detail = Some(serde_json::json!({ "entity_id": corpse_id, "method": method })),
                    }
                }
            }
        }
    });
}

pub(crate) fn take_corpse_reopen_ask() -> Option<(u64, u32)> {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        for vm in b.vms.values_mut() {
            if let Some(batch) = vm.active_batch.as_mut()
                && let Some(reopen) = batch.corpse_reopen.as_mut()
                && !reopen.asked
            {
                reopen.asked = true;
                return Some((batch.vm, reopen.corpse_id));
            }
        }
        None
    })
}

pub(crate) fn abort_corpse_reopen(code: &'static str, reason: &str) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        let now_tick = b.tick;
        for vm_state in b.vms.values_mut() {
            let Some(step) = vm_state
                .active_batch
                .as_ref()
                .filter(|batch| batch.corpse_reopen.is_some())
                .map(|batch| batch.current_step)
            else {
                continue;
            };
            vm_state.finish_batch(
                "refused",
                Some(false),
                Some(code),
                Some(reason.into()),
                now_tick,
                Some(step),
                true,
            );
        }
    });
}

pub(crate) fn resolve_server_loot_close() {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.native.loot_busy = false;
        let now_tick = b.tick;
        if b.resolve_player_activity(Subsystem::Loot) {
            return;
        }
        let vm_ids: Vec<u64> = b.vms.keys().copied().collect();
        for vm_id in vm_ids {
            let Some(vm_state) = b.vms.get_mut(&vm_id) else {
                continue;
            };
            let matches_ids: Vec<u64> = vm_state
                .in_flight
                .iter()
                .filter_map(|(&id, f)| {
                    (f.native_sent
                        && matches!(f.matcher, ServerMatcher::LootOne { .. } | ServerMatcher::LootAll { .. }))
                    .then_some(id)
                })
                .collect();
            for id in matches_ids {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
                    let ok = inflight.inv_confirmed.then_some(true);
                    let state = if ok == Some(true) { "completed" } else { "unknown" };
                    let code = (ok != Some(true)).then_some("unknown");
                    let reason = (ok != Some(true)).then(|| "corpse closed".to_string());
                    let rec = ActionResultRecord {
                        id: inflight.id,
                        vm: inflight.vm,
                        kind: inflight.kind,
                        state,
                        phase: "server",
                        ok,
                        code: code.map(str::to_string),
                        reason: reason.clone(),
                        queued_tick: inflight.queued_tick,
                        dispatched_tick: Some(inflight.dispatched_tick),
                        completed_tick: Some(now_tick),
                        detail: inflight.detail,
                    };
                    vm_state.upsert_retained(rec.clone());
                    vm_state.pending_events.push_back(rec);
                    let is_loot_one = matches!(inflight.matcher, ServerMatcher::LootOne { .. });
                    vm_state.handle_batch_step_resolution(
                        id,
                        now_tick,
                        state,
                        ok,
                        code,
                        reason,
                    );
                    if ok == Some(true)
                        && is_loot_one
                        && let Some(batch) = vm_state.active_batch.as_mut()
                        && !batch.steps.is_empty()
                        && matches!(batch.steps.front(), Some(ActionKind::Loot { .. }))
                        && let Some(cid) = CURRENT_CORPSE_TARGET
                            .with(|c| c.get())
                            .or(batch.corpse_target_id)
                    {
                        batch.corpse_target_id = Some(cid);
                        batch.corpse_reopen = Some(CorpseReopen {
                            corpse_id: cid,
                            asked: false,
                            started_tick: now_tick,
                        });
                    }
                }
            }
        }
    });
}

pub(crate) fn resolve_server_loot_open(opened_corpse_id: Option<u32>, is_empty: bool) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        let now_tick = b.tick;
        if let Some(cid) = opened_corpse_id {
            CURRENT_CORPSE_TARGET.with(|c| c.set(Some(cid)));
        }
        b.resolve_matching(|inflight| {
            let ServerMatcher::OpenLoot { corpse_id } = inflight.matcher else {
                return None;
            };
            if opened_corpse_id.is_some() && opened_corpse_id != Some(corpse_id) {
                return None;
            }
            Some((Some(true), None, None))
        });
        for vm_state in b.vms.values_mut() {
            let Some(reopen) = vm_state
                .active_batch
                .as_ref()
                .and_then(|batch| batch.corpse_reopen.clone())
            else {
                continue;
            };
            let step = vm_state.active_batch.as_ref().map(|batch| batch.current_step);
            if is_empty {
                vm_state.finish_batch(
                    "refused",
                    Some(false),
                    Some("not_found"),
                    Some("corpse has no items".into()),
                    now_tick,
                    step,
                    true,
                );
            } else if opened_corpse_id.is_some() && opened_corpse_id != Some(reopen.corpse_id) {
                vm_state.finish_batch(
                    "refused",
                    Some(false),
                    Some("stale_handle"),
                    Some("corpse changed".into()),
                    now_tick,
                    step,
                    true,
                );
            } else if let Some(batch) = vm_state.active_batch.as_mut() {
                batch.corpse_reopen = None;
            }
        }
    });
}

pub(crate) fn resolve_server_loot_blocked(reason: Option<&str>) {
    let text = reason.unwrap_or("loot blocked by server");
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        let now_tick = b.tick;
        for vm_state in b.vms.values_mut() {
            if vm_state
                .active_batch
                .as_ref()
                .is_some_and(|batch| batch.corpse_reopen.is_some())
            {
                let step = vm_state.active_batch.as_ref().map(|batch| batch.current_step);
                vm_state.finish_batch(
                    "refused",
                    Some(false),
                    Some("refused"),
                    Some(text.to_string()),
                    now_tick,
                    step,
                    true,
                );
            }
        }
        b.resolve_matching(|inflight| {
            matches!(
                inflight.matcher,
                ServerMatcher::OpenLoot { .. } | ServerMatcher::Talk { .. }
            )
            .then(|| (Some(false), Some("refused"), Some(text.to_string())))
        });
    });
    resolve_server_loot_one(None, Some(false), Some(text), |_| 0);
    resolve_server_loot_all(Some(false), Some(text), false);
}

/// A corpse open that did not happen: the server said so in chat ("Out of range."), or the game turned the corpse's list
/// away itself (another window was up, so it answered with 0x0016). The waiting `open_loot` requests and a batch's
/// re-ask of its corpse end as refused with `reason` at once, never left to the 10 s timeout. Takes in flight are not
/// touched (they have their own answers).
pub(crate) fn resolve_server_open_loot_refused(code: &'static str, reason: &str) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        let now_tick = b.tick;
        for vm_state in b.vms.values_mut() {
            if vm_state
                .active_batch
                .as_ref()
                .is_some_and(|batch| batch.corpse_reopen.as_ref().is_some_and(|r| r.asked))
            {
                let step = vm_state.active_batch.as_ref().map(|batch| batch.current_step);
                vm_state.finish_batch(
                    "refused",
                    Some(false),
                    Some(code),
                    Some(reason.to_string()),
                    now_tick,
                    step,
                    true,
                );
            }
        }
        b.resolve_matching(|inflight| {
            (inflight.native_sent && matches!(inflight.matcher, ServerMatcher::OpenLoot { .. }))
                .then(|| (Some(false), Some(code), Some(reason.to_string())))
        });
    });
}

/// An `open_loot` request is waiting for the server's answer.
pub(crate) fn open_loot_in_flight() -> bool {
    BROKER.with(|b| {
        b.borrow().vms.values().any(|v| {
            v.in_flight
                .values()
                .any(|f| f.native_sent && matches!(f.matcher, ServerMatcher::OpenLoot { .. }))
                || v
                    .active_batch
                    .as_ref()
                    .is_some_and(|batch| batch.corpse_reopen.as_ref().is_some_and(|r| r.asked))
        })
    })
}

pub(crate) fn resolve_server_talk_open(opened: TalkKind, opened_npc_id: Option<u32>) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            let ServerMatcher::Talk { npc_id, kind } = inflight.matcher else {
                return None;
            };
            if (kind == TalkKind::AnyFriendly || kind == opened)
                && opened_npc_id.is_none_or(|id| id == npc_id)
            {
                Some((Some(true), None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_loot_one(
    confirmed_key: Option<u32>,
    ok: Option<bool>,
    reason: Option<&str>,
    corpse_qty_of: impl Fn(u32) -> i64,
) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.native.loot_busy = false;
        if b.resolve_player_activity(Subsystem::Loot) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let ServerMatcher::LootOne { key, qty_before, count, .. } = inflight.matcher else {
                return None;
            };
            if let Some(ck) = confirmed_key
                && ck != key
            {
                return None;
            }
            match ok {
                Some(true) => {
                    let after = corpse_qty_of(key);
                    let exact = if count >= qty_before {
                        after == 0
                    } else {
                        after == qty_before - count
                    };
                    if exact {
                        Some((Some(true), None, None))
                    } else {
                        Some((
                            None,
                            Some("unknown"),
                            Some(format!(
                                "loot count mismatch (expected {}, corpse stack {} -> {})",
                                count, qty_before, after
                            )),
                        ))
                    }
                }
                Some(false) => Some((
                    Some(false),
                    Some("refused"),
                    Some(reason.unwrap_or("loot refused by server").to_string()),
                )),
                None => Some((
                    None,
                    Some("unknown"),
                    Some(reason.unwrap_or("loot outcome unknown").to_string()),
                )),
            }
        });
    });
}

pub(crate) fn resolve_server_loot_all(
    ok: Option<bool>,
    reason: Option<&str>,
    state_changed: bool,
) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.native.loot_busy = false;
        if b.resolve_player_activity(Subsystem::Loot) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            if !matches!(inflight.matcher, ServerMatcher::LootAll { .. }) {
                return None;
            }
            match ok {
                Some(true) if state_changed => Some((Some(true), None, None)),
                Some(true) => Some((
                    None,
                    Some("unknown"),
                    Some("loot_all did not change corpse state".into()),
                )),
                Some(false) => Some((
                    Some(false),
                    Some("refused"),
                    Some(reason.unwrap_or("loot_all refused by server").to_string()),
                )),
                None => Some((
                    None,
                    Some("unknown"),
                    Some(reason.unwrap_or("loot outcome unknown").to_string()),
                )),
            }
        });
    });
}

pub(crate) fn resolve_server_merchant_buy(
    added_item_id: i64,
    pending_cleared: bool,
    inv_total_for_item: i64,
) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        if pending_cleared {
            b.native.merchant_pending = false;
        }
        if b.resolve_player_activity(Subsystem::Merchant) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let ServerMatcher::MerchantBuy {
                item_id,
                qty_before,
                count,
            } = inflight.matcher
            else {
                return None;
            };
            if !pending_cleared || added_item_id != item_id {
                return None;
            }
            if inv_total_for_item >= qty_before.saturating_add(count) {
                Some((Some(true), None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_merchant_sell(
    pending_cleared: bool,
    inv_qty_for_key: impl Fn(u32) -> i64,
) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        if pending_cleared {
            b.native.merchant_pending = false;
        }
        if b.resolve_player_activity(Subsystem::Merchant) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let ServerMatcher::MerchantSell {
                key,
                qty_before,
                count,
            } = inflight.matcher
            else {
                return None;
            };
            if !pending_cleared {
                return None;
            }
            let after = inv_qty_for_key(key);
            if after <= qty_before.saturating_sub(count) {
                Some((Some(true), None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_merchant(is_buy: bool, ok: bool, reason: Option<&str>) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.native.merchant_pending = false;
        if b.resolve_player_activity(Subsystem::Merchant) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let matched = if is_buy {
                matches!(inflight.matcher, ServerMatcher::MerchantBuy { .. })
            } else {
                matches!(inflight.matcher, ServerMatcher::MerchantSell { .. })
            };
            if !matched {
                return None;
            }
            if ok {
                Some((Some(true), None, None))
            } else {
                Some((
                    Some(false),
                    Some("refused"),
                    Some(reason.unwrap_or("merchant transaction refused").to_string()),
                ))
            }
        });
    });
}

pub(crate) fn resolve_server_bank_move(
    is_deposit_opcode: bool,
    pending_cleared: bool,
    added_item: Option<(i64, i64)>,
    withdrawn: Option<(u32, i64, i64)>,
    bank_qty_for_key: impl Fn(u32) -> i64,
) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        if pending_cleared {
            b.native.bank_busy = false;
        }
        if b.resolve_player_activity(Subsystem::Bank) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let ServerMatcher::BankMove {
                from_bank,
                key,
                item_id,
                qty_before,
                count,
            } = inflight.matcher
            else {
                return None;
            };
            if !pending_cleared {
                return None;
            }
            if is_deposit_opcode && !from_bank {
                if added_item == Some((item_id, count)) {
                    Some((Some(true), None, None))
                } else if added_item.is_some_and(|(id, _)| id == item_id) {
                    Some((
                        None,
                        Some("unknown"),
                        Some("bank deposit quantity did not match request".into()),
                    ))
                } else {
                    None
                }
            } else if !is_deposit_opcode && from_bank {
                if withdrawn == Some((key, item_id, count))
                    && bank_qty_for_key(key) == qty_before.saturating_sub(count)
                {
                    Some((Some(true), None, None))
                } else if withdrawn.is_some_and(|(k, id, _)| k == key && id == item_id) {
                    Some((
                        None,
                        Some("unknown"),
                        Some("bank withdraw quantity did not match request".into()),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_bank_money(pending_cleared: bool, bank_after: i64) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        if pending_cleared {
            b.native.bank_busy = false;
        }
        if b.resolve_player_activity(Subsystem::Bank) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let ServerMatcher::BankMoney {
                dir,
                bank_before,
                amount,
            } = inflight.matcher
            else {
                return None;
            };
            if !pending_cleared {
                return None;
            }
            let expected = match dir {
                1 => bank_after >= bank_before.saturating_add(amount),
                2 => bank_after <= bank_before.saturating_sub(amount),
                _ => bank_after != bank_before,
            };
            if expected {
                Some((Some(true), None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_bank(is_money: bool, ok: bool, reason: Option<&str>) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.native.bank_busy = false;
        if b.resolve_player_activity(Subsystem::Bank) {
            return;
        }
        b.resolve_matching_sent(|inflight| {
            let matched = if is_money {
                matches!(inflight.matcher, ServerMatcher::BankMoney { .. })
            } else {
                matches!(inflight.matcher, ServerMatcher::BankMove { .. })
            };
            if !matched {
                return None;
            }
            if ok {
                Some((Some(true), None, None))
            } else {
                Some((
                    Some(false),
                    Some("refused"),
                    Some(reason.unwrap_or("bank transaction refused").to_string()),
                ))
            }
        });
    });
}

pub(crate) fn resolve_server_camera(ok: bool, reason: Option<&str>) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching_sent(|inflight| {
            if !matches!(inflight.matcher, ServerMatcher::Camera) {
                return None;
            }
            if ok {
                Some((Some(true), None, None))
            } else {
                Some((
                    Some(false),
                    Some("busy"),
                    Some(reason.unwrap_or("camera is locked").to_string()),
                ))
            }
        });
    });
}

pub(crate) fn resolve_in_flight_window_closed(kind_name: &'static str, reason: &str) {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.resolve_matching(|inflight| {
            if inflight.kind == kind_name {
                Some((Some(false), Some("window_closed"), Some(reason.to_string())))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_d7() {
    BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.native.loot_busy = false;
        b.native.merchant_pending = false;
        b.native.bank_busy = false;
        b.native.inventory_busy = false;
        for sub in [Subsystem::Loot, Subsystem::Merchant, Subsystem::Bank] {
            b.resolve_player_activity(sub);
        }
        b.resolve_matching_sent(|inflight| {
            if matches!(
                inflight.matcher,
                ServerMatcher::LootOne { .. }
                    | ServerMatcher::LootAll { .. }
                    | ServerMatcher::MerchantBuy { .. }
                    | ServerMatcher::MerchantSell { .. }
                    | ServerMatcher::BankMove { .. }
                    | ServerMatcher::BankMoney { .. }
            ) {
                Some((
                    Some(false),
                    Some("refused"),
                    Some("server cancelled or refused transaction".into()),
                ))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_ability_move(from: u8, to: u8) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            let ServerMatcher::AbilityMoveIndex {
                from: ef,
                to: et,
            } = inflight.matcher
            else {
                return None;
            };
            if from == ef && to == et {
                Some((Some(true), None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn poll(lua: &Lua) -> Result<()> {
    let vm = super::native_events::bus(lua).borrow().vm;
    let (events, batch_events) = BROKER.with(|b| {
        let mut b = b.borrow_mut();
        match b.vms.get_mut(&vm) {
            Some(v) => (
                v.pending_events.drain(..).collect::<Vec<_>>(),
                v.pending_batch_events.drain(..).collect::<Vec<_>>(),
            ),
            None => (Vec::new(), Vec::new()),
        }
    });
    for rec in events {
        let tbl = action_result_raw_table(lua, &rec)?;
        super::native_events::action_result(lua, tbl)?;
    }
    for (is_result, rec) in batch_events {
        let tbl = batch_result_raw_table(lua, &rec)?;
        super::native_events::batch_event(
            lua,
            if is_result { "batch.result" } else { "batch.progress" },
            tbl,
        )?;
    }
    Ok(())
}

pub(crate) const ACTION_RESULT_KEYS: &[&str] = &[
    "request_id",
    "kind",
    "state",
    "phase",
    "ok",
    "code",
    "reason",
    "queued_tick",
    "dispatched_tick",
    "completed_tick",
    "detail",
];

pub(crate) const BATCH_RESULT_KEYS: &[&str] = &[
    "batch_id",
    "state",
    "ok",
    "completed",
    "total",
    "current_step",
    "code",
    "reason",
    "queued_tick",
    "completed_tick",
    "failed_step",
    "detail",
];

fn batch_result_raw_table(lua: &Lua, rec: &BatchResultRecord) -> Result<mlua::Table> {
    let tbl = lua.create_table()?;
    tbl.raw_set(
        "batch_id",
        lua.create_userdata(BatchId {
            vm: rec.vm,
            id: rec.id,
        })?,
    )?;
    tbl.raw_set("state", rec.state)?;
    if let Some(ok) = rec.ok {
        tbl.raw_set("ok", ok)?;
    }
    tbl.raw_set("completed", rec.completed)?;
    tbl.raw_set("total", rec.total)?;
    tbl.raw_set("current_step", rec.current_step)?;
    if let Some(code) = &rec.code {
        tbl.raw_set("code", code.as_str())?;
    }
    if let Some(reason) = &rec.reason {
        tbl.raw_set("reason", reason.as_str())?;
    }
    tbl.raw_set("queued_tick", rec.queued_tick)?;
    if let Some(completed) = rec.completed_tick {
        tbl.raw_set("completed_tick", completed)?;
    }
    if let Some(failed_step) = rec.failed_step {
        tbl.raw_set("failed_step", failed_step)?;
    }
    if let Some(detail) = &rec.detail {
        tbl.raw_set(
            "detail",
            record(lua, "eqoa.actions.BatchResult.detail", detail)?,
        )?;
    }
    Ok(tbl)
}

fn batch_result_to_table(lua: &Lua, rec: &BatchResultRecord) -> Result<mlua::Table> {
    let tbl = batch_result_raw_table(lua, rec)?;
    super::native::guard_lazy(lua, tbl, "eqoa.actions.BatchResult", BATCH_RESULT_KEYS)
}

fn action_result_raw_table(lua: &Lua, rec: &ActionResultRecord) -> Result<mlua::Table> {
    let tbl = lua.create_table()?;
    tbl.raw_set(
        "request_id",
        lua.create_userdata(RequestId {
            vm: rec.vm,
            id: rec.id,
        })?,
    )?;
    tbl.raw_set("kind", rec.kind)?;
    tbl.raw_set("state", rec.state)?;
    tbl.raw_set("phase", rec.phase)?;
    if let Some(ok) = rec.ok {
        tbl.raw_set("ok", ok)?;
    }
    if let Some(code) = &rec.code {
        tbl.raw_set("code", code.as_str())?;
    }
    if let Some(reason) = &rec.reason {
        tbl.raw_set("reason", reason.as_str())?;
    }
    tbl.raw_set("queued_tick", rec.queued_tick)?;
    if let Some(dispatched) = rec.dispatched_tick {
        tbl.raw_set("dispatched_tick", dispatched)?;
    }
    if let Some(completed) = rec.completed_tick {
        tbl.raw_set("completed_tick", completed)?;
    }
    if let Some(detail) = &rec.detail {
        tbl.raw_set(
            "detail",
            record(lua, "eqoa.actions.ActionResult.detail", detail)?,
        )?;
    }
    Ok(tbl)
}

fn action_result_to_table(lua: &Lua, rec: &ActionResultRecord) -> Result<mlua::Table> {
    let tbl = action_result_raw_table(lua, rec)?;
    super::native::guard_lazy(lua, tbl, "eqoa.actions.ActionResult", ACTION_RESULT_KEYS)
}

pub(crate) fn enqueue_lua_with(
    lua: &Lua,
    symbol: &str,
    build: impl FnOnce() -> std::result::Result<ActionKind, (&'static str, String)>,
) -> Result<mlua::MultiValue> {
    enqueue_lua_with_grant(lua, symbol, symbol, build)
}

fn enqueue_lua_with_grant(
    lua: &Lua,
    symbol: &str,
    capability: &str,
    build: impl FnOnce() -> std::result::Result<ActionKind, (&'static str, String)>,
) -> Result<mlua::MultiValue> {
    let vm = super::native_events::bus(lua).borrow().vm;
    let addon = session(lua).borrow().addon.clone();
    if let Err((code, msg)) = require_capability(lua, symbol, capability) {
        return error(lua, symbol, code, &msg);
    }
    let kind = match build() {
        Ok(kind) => kind,
        Err((code, msg)) => return error(lua, symbol, code, &msg),
    };
    match BROKER.with(|b| b.borrow_mut().enqueue(vm, &addon, kind)) {
        Ok(req) => lua.pack_multi((lua.create_userdata(req)?, Value::Nil)),
        Err((code, msg)) => error(lua, symbol, code, &msg),
    }
}

pub(crate) fn enqueue_lua(
    lua: &Lua,
    symbol: &str,
    kind: ActionKind,
) -> Result<mlua::MultiValue> {
    enqueue_lua_with(lua, symbol, move || Ok(kind))
}

/// Queues a map alias (`eqoa.map.set_waypoint` / `clear_waypoint`, spec §7): the same request and result
/// contract as `eqoa.actions.*`. The map binding checks the waypoint grant and world readiness.
pub(crate) fn enqueue_map_lua(
    lua: &Lua,
    symbol: &str,
    build: impl FnOnce(u64, &str) -> std::result::Result<ActionKind, (&'static str, String)>,
) -> Result<mlua::MultiValue> {
    let vm = super::native_events::bus(lua).borrow().vm;
    let addon = session(lua).borrow().addon.clone();
    let kind = match build(vm, &addon) {
        Ok(kind) => kind,
        Err((code, msg)) => return error(lua, symbol, code, &msg),
    };
    match BROKER.with(|b| b.borrow_mut().enqueue(vm, &addon, kind)) {
        Ok(req) => lua.pack_multi((lua.create_userdata(req)?, Value::Nil)),
        Err((code, msg)) => error(lua, symbol, code, &msg),
    }
}

fn current_generation() -> u64 {
    CORE.with(|c| c.borrow().generation)
}

fn parse_entity_id(arg: Value) -> std::result::Result<Option<u32>, (&'static str, String)> {
    match arg {
        Value::Nil => Ok(None),
        Value::Integer(n) => {
            if !(0..u32::MAX as i64).contains(&n) {
                return Err(("invalid_argument", "entity id out of range".into()));
            }
            Ok(Some(n as u32))
        }
        Value::Number(n) => {
            if !n.is_finite() || n < 0.0 || n >= u32::MAX as f64 || n.fract() != 0.0 {
                return Err(("invalid_argument", "entity id must be a whole integer".into()));
            }
            Ok(Some(n as u32))
        }
        Value::UserData(ud) => {
            let Ok(r) = ud.borrow::<super::native_state::Ref>() else {
                return Err(("invalid_argument", "entity handle must be an entity ref or id".into()));
            };
            if r.kind != "entity" {
                return Err(("invalid_argument", format!("expected entity ref, got {}", r.kind)));
            }
            let cur_gen = current_generation();
            if r.generation != 0 && cur_gen != 0 && r.generation != cur_gen {
                return Err(("stale_handle", "entity reference is from an older session generation".into()));
            }
            let id = r
                .key
                .parse::<u32>()
                .map_err(|_| ("invalid_argument", "malformed entity ref key".into()))?;
            Ok(Some(id))
        }
        _ => Err(("invalid_argument", "entity must be an entity ref, integer id, or nil".into())),
    }
}

fn parse_ability_lookup(
    arg: Value,
    allow_display_index: bool,
) -> std::result::Result<AbilityLookup, (&'static str, String)> {
    match arg {
        Value::Integer(n) => {
            if n < 0 {
                return Err(("invalid_argument", "ability id or index must be non-negative".into()));
            }
            Ok(if allow_display_index {
                AbilityLookup {
                    id: Some(n),
                    name: None,
                    index: Some(n),
                }
            } else {
                AbilityLookup {
                    id: Some(n),
                    name: None,
                    index: None,
                }
            })
        }
        Value::Number(n) => {
            if !n.is_finite() || n < 0.0 || n.fract() != 0.0 {
                return Err(("invalid_argument", "ability id or index must be a whole non-negative integer".into()));
            }
            let n = n as i64;
            Ok(if allow_display_index {
                AbilityLookup {
                    id: Some(n),
                    name: None,
                    index: Some(n),
                }
            } else {
                AbilityLookup {
                    id: Some(n),
                    name: None,
                    index: None,
                }
            })
        }
        Value::String(s) => {
            let text = s
                .to_str()
                .map_err(|_| ("invalid_argument", "ability string must be UTF-8".into()))?
                .trim()
                .to_string();
            if text.is_empty() {
                return Err(("invalid_argument", "ability name cannot be empty".into()));
            }
            if let Ok(id) = text.parse::<i64>() {
                Ok(AbilityLookup {
                    id: Some(id),
                    name: Some(text),
                    index: None,
                })
            } else {
                Ok(AbilityLookup {
                    id: None,
                    name: Some(text),
                    index: None,
                })
            }
        }
        Value::UserData(ud) => {
            let Ok(r) = ud.borrow::<super::native_state::Ref>() else {
                return Err(("invalid_argument", "ability must be an ability ref, id, or name".into()));
            };
            if r.kind != "ability" {
                return Err(("invalid_argument", format!("expected ability ref, got {}", r.kind)));
            }
            let cur_gen = current_generation();
            if r.generation != 0 && cur_gen != 0 && r.generation != cur_gen {
                return Err(("stale_handle", "ability reference is from an older session generation".into()));
            }
            let id = r
                .key
                .parse::<i64>()
                .map_err(|_| ("invalid_argument", "malformed ability ref key".into()))?;
            Ok(AbilityLookup {
                id: Some(id),
                name: None,
                index: None,
            })
        }
        _ => Err(("invalid_argument", "ability must be an ability ref, id, or name".into())),
    }
}

fn validate_chat_string(raw: Value, command_mode: bool) -> std::result::Result<String, (&'static str, String)> {
    let Value::String(s) = raw else {
        return Err(("invalid_argument", "message text must be a string".into()));
    };
    let text = s
        .to_str()
        .map_err(|_| ("invalid_argument", "message text must be UTF-8".into()))?
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(("invalid_argument", "message text is empty".into()));
    }
    if text.contains('\0') || text.contains('\r') || text.contains('\n') {
        return Err(("invalid_argument", "message cannot contain control newlines or NUL".into()));
    }
    let normalized = if command_mode && !text.starts_with('/') && !text.starts_with('#') {
        format!("/{text}")
    } else {
        text
    };
    if normalized.encode_utf16().count() >= 128 {
        return Err(("invalid_argument", "message must be shorter than 128 UTF-16 units".into()));
    }
    if gesture::is_forbidden_report_text(&normalized) {
        return Err((
            "permission_denied",
            "synthetic report routes (/bug, #bug, /feature, #feature) are forbidden".into(),
        ));
    }
    Ok(normalized)
}

fn enqueue_chat_line(lua: &Lua, text: Value, command: bool) -> Result<mlua::MultiValue> {
    let symbol = if command { "eqoa.actions.command" } else { "eqoa.actions.chat" };
    let text = match validate_chat_string(text, command) {
        Ok(text) => text,
        Err((code, reason)) => return error(lua, symbol, code, &reason),
    };
    let capability = super::chat::line_capability(&text);
    enqueue_lua_with_grant(lua, symbol, capability, || {
        Ok(if command { ActionKind::Command { text } } else { ActionKind::Chat { text } })
    })
}

fn parse_weapon_set(raw: Value) -> std::result::Result<i64, (&'static str, String)> {
    match raw {
        Value::Integer(n) if (1..=5).contains(&n) => Ok(n - 1),
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && (1.0..=5.0).contains(&n) => {
            Ok((n as i64) - 1)
        }
        Value::String(s) => {
            let s = s.to_str().map_err(|_| ("invalid_argument", "invalid set".into()))?;
            if s.eq_ignore_ascii_case("bare_hands") || s.eq_ignore_ascii_case("bare hands") {
                Ok(crate::gear::BARE_HANDS)
            } else {
                Err(("invalid_argument", "weapon set must be 1..=5 or 'bare_hands'".into()))
            }
        }
        _ => Err(("invalid_argument", "weapon set must be 1..=5 or 'bare_hands'".into())),
    }
}

fn parse_batch_window(raw: &str) -> std::result::Result<BatchWindow, (&'static str, String)> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "merchant" => Ok(BatchWindow::Merchant),
        "bank" => Ok(BatchWindow::Bank),
        "loot" | "corpse" => Ok(BatchWindow::Loot),
        other => Err((
            "invalid_argument",
            format!("unknown batch require_window '{other}'"),
        )),
    }
}

fn step_field(tbl: &mlua::Table, key: &str, pos: Option<usize>) -> Value {
    if let Ok(v) = tbl.get::<Value>(key)
        && !v.is_nil()
    {
        return v;
    }
    if let Some(i) = pos
        && let Ok(v) = tbl.get::<Value>(i)
    {
        return v;
    }
    Value::Nil
}

fn step_int(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(n) => Some(*n),
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 => Some(*n as i64),
        _ => None,
    }
}

/// `open_loot`'s `keep_target` option: a boolean, nil meaning false.
fn parse_keep_target(v: Value) -> std::result::Result<bool, (&'static str, String)> {
    match v {
        Value::Nil => Ok(false),
        Value::Boolean(b) => Ok(b),
        _ => Err(("invalid_argument", "open_loot keep_target must be a boolean".into())),
    }
}

fn parse_batch_step(raw: Value) -> std::result::Result<ActionKind, (&'static str, String)> {
    let Value::Table(t) = raw else {
        return Err(("invalid_argument", "each batch step must be a table".into()));
    };
    let kind_val = match t.get::<Value>("kind") {
        Ok(v) if !v.is_nil() => v,
        _ => t.get::<Value>("action").unwrap_or(Value::Nil),
    };
    let Value::String(kind_str) = kind_val else {
        return Err(("invalid_argument", "batch step requires a 'kind' string".into()));
    };
    let kind = kind_str
        .to_str()
        .map_err(|_| ("invalid_argument", "step kind must be UTF-8".into()))?
        .trim()
        .to_ascii_lowercase();
    match kind.as_str() {
        "assist" | "target_target" => Ok(ActionKind::Assist),
        "interact" => Ok(ActionKind::Interact),
        "open_loot" => Ok(ActionKind::OpenLoot {
            entity_id: parse_entity_id(step_field(&t, "entity_id", Some(1)))?,
            keep_target: parse_keep_target(step_field(&t, "keep_target", None))?,
        }),
        "talk" => {
            let entity_id = parse_entity_id(step_field(&t, "entity_id", Some(1)))?;
            let kind = match step_field(&t, "kind_filter", None) {
                Value::Nil => TalkKind::AnyFriendly,
                Value::String(s) => match s.to_str().ok().as_deref() {
                    Some("merchant") => TalkKind::Merchant,
                    Some("bank") => TalkKind::Bank,
                    Some(other) => {
                        return Err((
                            "invalid_argument",
                            format!("talk kind_filter must be 'merchant' or 'bank', got '{other}'"),
                        ));
                    }
                    None => return Err(("invalid_argument", "talk kind_filter must be UTF-8".into())),
                },
                _ => return Err(("invalid_argument", "talk kind_filter must be a string".into())),
            };
            Ok(ActionKind::Talk { entity_id, kind })
        }
        "loot_all" => Ok(ActionKind::LootAll),
        "stop" => Ok(ActionKind::Stop),
        "unmemorize_all" => Ok(ActionKind::UnmemorizeAll),
        "target" => Ok(ActionKind::Target(parse_entity_id(step_field(
            &t,
            "target",
            Some(1),
        ))?)),
        "attack" => {
            let on = match step_field(&t, "on", Some(1)) {
                Value::Nil => None,
                Value::Boolean(b) => Some(b),
                _ => return Err(("invalid_argument", "attack 'on' must be boolean".into())),
            };
            Ok(ActionKind::Attack { on })
        }
        "autorun" => {
            let on = match step_field(&t, "on", Some(1)) {
                Value::Nil => None,
                Value::Boolean(b) => Some(b),
                _ => return Err(("invalid_argument", "autorun 'on' must be boolean".into())),
            };
            Ok(ActionKind::Autorun { on })
        }
        "use_item" => {
            let item = step_field(&t, "item", Some(1));
            let spec = resolve_item_handle(&item)?;
            if spec.list != "inventory" {
                return Err((
                    "invalid_argument",
                    format!("use_item requires an inventory item handle, got '{}'", spec.list),
                ));
            }
            Ok(ActionKind::UseItem { item: spec })
        }
        "equip" => {
            let item = step_field(&t, "item", Some(1));
            let spec = resolve_item_handle(&item)?;
            if spec.list != "inventory" {
                return Err((
                    "invalid_argument",
                    format!("equip requires an inventory item handle, got '{}'", spec.list),
                ));
            }
            Ok(ActionKind::Equip { item: spec })
        }
        "quick_slot" => {
            let item = step_field(&t, "item", Some(1));
            let spec = resolve_item_handle(&item)?;
            if spec.list != "inventory" {
                return Err((
                    "invalid_argument",
                    "quick_slot requires an inventory item handle".into(),
                ));
            }
            let Some(slot) = step_int(&step_field(&t, "slot", Some(2))) else {
                return Err(("invalid_argument", "quick_slot must be 1..=4".into()));
            };
            if !(1..=4).contains(&slot) {
                return Err(("invalid_argument", "quick_slot must be 1..=4".into()));
            }
            Ok(ActionKind::QuickSlot {
                item: spec,
                slot: (slot - 1) as u8,
            })
        }
        "loot" => {
            let item = step_field(&t, "item", Some(1));
            let spec = resolve_item_handle(&item)?;
            if spec.list != "loot" {
                return Err((
                    "invalid_argument",
                    format!("loot requires a loot item handle, got '{}'", spec.list),
                ));
            }
            let count_v = step_field(&t, "count", Some(2));
            let count = if count_v.is_nil() {
                None
            } else {
                let Some(n) = step_int(&count_v).filter(|n| *n >= 1) else {
                    return Err(("invalid_argument", "loot count must be >= 1".into()));
                };
                Some(n)
            };
            let identity = item_identity_of(&item);
            Ok(ActionKind::Loot { item: spec, count, identity })
        }
        "buy" => {
            let ware = step_field(&t, "ware", None);
            let ware = if ware.is_nil() {
                step_field(&t, "item", Some(1))
            } else {
                ware
            };
            let spec = resolve_item_handle(&ware)?;
            if spec.list != "merchant" {
                return Err((
                    "invalid_argument",
                    format!("buy requires a merchant ware handle, got '{}'", spec.list),
                ));
            }
            let count_v = step_field(&t, "count", Some(2));
            let count = if count_v.is_nil() {
                1
            } else {
                let Some(n) = step_int(&count_v).filter(|n| *n >= 1) else {
                    return Err(("invalid_argument", "buy count must be >= 1".into()));
                };
                n
            };
            Ok(ActionKind::Buy { ware: spec, count })
        }
        "sell" => {
            let item = step_field(&t, "item", Some(1));
            let spec = resolve_item_handle(&item)?;
            if spec.list != "inventory" {
                return Err((
                    "invalid_argument",
                    format!("sell requires an inventory item handle, got '{}'", spec.list),
                ));
            }
            let count_v = step_field(&t, "count", Some(2));
            let count = if count_v.is_nil() {
                1
            } else {
                let Some(n) = step_int(&count_v).filter(|n| *n >= 1) else {
                    return Err(("invalid_argument", "sell count must be >= 1".into()));
                };
                n
            };
            Ok(ActionKind::Sell { item: spec, count })
        }
        "bank_move" => {
            let item = step_field(&t, "item", Some(1));
            let spec = resolve_item_handle(&item)?;
            let handle_bank = match spec.list {
                "bank" => true,
                "inventory" => false,
                other => {
                    return Err((
                        "invalid_argument",
                        format!("bank_move requires an inventory or bank handle, got '{other}'"),
                    ));
                }
            };
            let dir_v = step_field(&t, "from", None);
            if let Value::String(s) = dir_v {
                let d = s
                    .to_str()
                    .map_err(|_| ("invalid_argument", "invalid direction".into()))?;
                let want_bank = match d.trim().to_ascii_lowercase().as_str() {
                    "bank" | "withdraw" => true,
                    "inventory" | "bags" | "deposit" => false,
                    other => {
                        return Err((
                            "invalid_argument",
                            format!("bank_move direction must be 'inventory' or 'bank', got '{other}'"),
                        ));
                    }
                };
                if want_bank != handle_bank {
                    return Err((
                        "invalid_argument",
                        "bank_move direction does not match handle container".into(),
                    ));
                }
            }
            let count_v = step_field(&t, "count", Some(2));
            let count = if count_v.is_nil() {
                1
            } else {
                let Some(n) = step_int(&count_v).filter(|n| *n >= 1) else {
                    return Err(("invalid_argument", "bank_move count must be >= 1".into()));
                };
                n
            };
            Ok(ActionKind::BankMove {
                from_bank: handle_bank,
                item: spec,
                count,
            })
        }
        "bank_money" => {
            let dir_v = step_field(&t, "direction", Some(1));
            let Value::String(s) = dir_v else {
                return Err(("invalid_argument", "bank_money requires 'deposit' or 'withdraw'".into()));
            };
            let d = s
                .to_str()
                .map_err(|_| ("invalid_argument", "invalid direction".into()))?;
            let dir = match d.trim().to_ascii_lowercase().as_str() {
                "deposit" => 1u8,
                "withdraw" => 2u8,
                other => {
                    return Err((
                        "invalid_argument",
                        format!("bank_money direction must be 'deposit' or 'withdraw', got '{other}'"),
                    ));
                }
            };
            let Some(amount) = step_int(&step_field(&t, "amount", Some(2))).filter(|n| *n >= 1) else {
                return Err(("invalid_argument", "bank_money amount must be >= 1".into()));
            };
            Ok(ActionKind::BankMoney { dir, amount })
        }
        "chat" => Ok(ActionKind::Chat {
            text: validate_chat_string(step_field(&t, "text", Some(1)), false)?,
        }),
        "command" => Ok(ActionKind::Command {
            text: validate_chat_string(step_field(&t, "text", Some(1)), true)?,
        }),
        "cast" => Ok(ActionKind::Cast(parse_ability_lookup(
            step_field(&t, "ability", Some(1)),
            false,
        )?)),
        "weapon_set" => Ok(ActionKind::WeaponSet {
            set: parse_weapon_set(step_field(&t, "set", Some(1)))?,
        }),
        other => Err((
            "invalid_argument",
            format!("unsupported batch action kind '{other}'"),
        )),
    }
}

fn parse_batch_spec(
    raw: &Value,
) -> std::result::Result<(Vec<ActionKind>, Option<BatchWindow>), (&'static str, String)> {
    let Value::Table(t) = raw else {
        return Err(("invalid_argument", "batch spec must be a table".into()));
    };
    let require_window = match t.get::<Value>("require_window") {
        Ok(Value::String(s)) => {
            let text = s
                .to_str()
                .map_err(|_| ("invalid_argument", "require_window must be UTF-8".into()))?;
            Some(parse_batch_window(&text)?)
        }
        Ok(Value::Nil) | Err(_) => None,
        Ok(_) => {
            return Err((
                "invalid_argument",
                "require_window must be 'merchant', 'bank', or 'loot'".into(),
            ));
        }
    };
    let steps_tbl = match t.get::<Value>("steps") {
        Ok(Value::Table(sub)) => sub,
        Ok(Value::Nil) | Err(_) => t.clone(),
        Ok(_) => {
            return Err(("invalid_argument", "batch 'steps' must be a list table".into()));
        }
    };
    let len = steps_tbl.raw_len();
    if len == 0 {
        return Err(("invalid_argument", "batch must contain at least one step".into()));
    }
    if len > MAX_BATCH_STEPS {
        return Err((
            "invalid_argument",
            format!("batch exceeds {MAX_BATCH_STEPS} steps"),
        ));
    }
    let mut out = Vec::with_capacity(len);
    for i in 1..=len {
        let step_val: Value = steps_tbl
            .raw_get(i)
            .map_err(|_| ("invalid_argument", "malformed batch step".into()))?;
        out.push(parse_batch_step(step_val)?);
    }
    Ok((out, require_window))
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    describe(
        lua,
        "eqoa.actions.RequestId",
        "supported",
        Some(6),
        "opaque action request handle",
    );
    describe(
        lua,
        "eqoa.actions.ActionResult",
        "supported",
        Some(6),
        "action status and result record",
    );
    for field in [
        "request_id",
        "kind",
        "state",
        "phase",
        "ok",
        "code",
        "reason",
        "queued_tick",
        "dispatched_tick",
        "completed_tick",
        "detail",
    ] {
        describe(
            lua,
            &format!("eqoa.actions.ActionResult.{field}"),
            "supported",
            Some(6),
            "ActionResult field",
        );
    }

    describe(
        lua,
        "eqoa.actions.BatchId",
        "supported",
        Some(6),
        "opaque action batch handle",
    );
    describe(
        lua,
        "eqoa.actions.BatchResult",
        "supported",
        Some(6),
        "batch progress and completion record",
    );
    for field in BATCH_RESULT_KEYS {
        let path = format!("eqoa.actions.BatchResult.{field}");
        match super::native::unprojected_field("eqoa.actions.BatchResult", field) {
            Some(reason) => describe(lua, &path, "unsupported", None, reason),
            None => describe(lua, &path, "supported", Some(6), "BatchResult field"),
        }
    }

    super::native::refuse_native(
        lua,
        "actions",
        "destroy",
        "unsupported",
        None,
        "excluded until after the upstream PR by the mod API plan",
    )?;
    for name in ["move", "look", "buttons"] {
        super::native::refuse_native(
            lua,
            "actions",
            name,
            "not_yet",
            Some(6),
            "continuous controls come after the upstream PR (mod API plan, round 8)",
        )?;
    }
    bind(
        lua,
        "actions",
        "status",
        lua.create_function(|lua, arg: Value| {
            let caller_vm = super::native_events::bus(lua).borrow().vm;
            let Value::UserData(ud) = arg else {
                return error(
                    lua,
                    "eqoa.actions.status",
                    "invalid_argument",
                    "id must be a RequestId or BatchId handle",
                );
            };
            if let Ok(batch) = ud.borrow::<BatchId>() {
                let batch = *batch;
                if batch.vm != caller_vm {
                    return error(
                        lua,
                        "eqoa.actions.status",
                        "permission_denied",
                        "batch handle belongs to another addon",
                    );
                }
                return match BROKER.with(|b| b.borrow().batch_status(batch)) {
                    Ok(Some(rec)) => {
                        let tbl = batch_result_to_table(lua, &rec)?;
                        lua.pack_multi((Value::Table(tbl), Value::Nil))
                    }
                    Ok(None) => error(
                        lua,
                        "eqoa.actions.status",
                        "not_found",
                        "batch result not found or evicted",
                    ),
                    Err((code, msg)) => error(lua, "eqoa.actions.status", code, &msg),
                };
            }
            let Ok(req) = ud.borrow::<RequestId>() else {
                return error(
                    lua,
                    "eqoa.actions.status",
                    "invalid_argument",
                    "id must be a RequestId or BatchId handle",
                );
            };
            let req = *req;
            if req.vm != caller_vm {
                return error(
                    lua,
                    "eqoa.actions.status",
                    "permission_denied",
                    "request handle belongs to another addon",
                );
            }
            match BROKER.with(|b| b.borrow().status(req)) {
                Ok(Some(rec)) => {
                    let tbl = action_result_to_table(lua, &rec)?;
                    lua.pack_multi((Value::Table(tbl), Value::Nil))
                }
                Ok(None) => error(
                    lua,
                    "eqoa.actions.status",
                    "not_found",
                    "request result not found or evicted",
                ),
                Err((code, msg)) => error(lua, "eqoa.actions.status", code, &msg),
            }
        })?,
    )?;

    bind(
        lua,
        "actions",
        "cancel",
        lua.create_function(|lua, arg: Value| {
            let caller_vm = super::native_events::bus(lua).borrow().vm;
            let Value::UserData(ud) = arg else {
                return error(
                    lua,
                    "eqoa.actions.cancel",
                    "invalid_argument",
                    "id must be a RequestId or BatchId handle",
                );
            };
            if let Ok(batch) = ud.borrow::<BatchId>() {
                let batch = *batch;
                if batch.vm != caller_vm {
                    return error(
                        lua,
                        "eqoa.actions.cancel",
                        "permission_denied",
                        "batch handle belongs to another addon",
                    );
                }
                return match BROKER.with(|b| b.borrow_mut().stop_batch(caller_vm, Some(batch.id))) {
                    Ok(true) => lua.pack_multi((true, Value::Nil)),
                    Ok(false) => error(
                        lua,
                        "eqoa.actions.cancel",
                        "not_found",
                        "batch is no longer running",
                    ),
                    Err((code, msg)) => error(lua, "eqoa.actions.cancel", code, &msg),
                };
            }
            let Ok(req) = ud.borrow::<RequestId>() else {
                return error(
                    lua,
                    "eqoa.actions.cancel",
                    "invalid_argument",
                    "id must be a RequestId or BatchId handle",
                );
            };
            let req = *req;
            if req.vm != caller_vm {
                return error(
                    lua,
                    "eqoa.actions.cancel",
                    "permission_denied",
                    "request handle belongs to another addon",
                );
            }
            match BROKER.with(|b| b.borrow_mut().cancel(req)) {
                Ok(Some(cancelled)) => lua.pack_multi((cancelled, Value::Nil)),
                Ok(None) => error(
                    lua,
                    "eqoa.actions.cancel",
                    "not_found",
                    "request is no longer pending",
                ),
                Err((code, msg)) => error(lua, "eqoa.actions.cancel", code, &msg),
            }
        })?,
    )?;

    bind(
        lua,
        "actions",
        "batch",
        lua.create_function(|lua, spec: Value| {
            let vm = super::native_events::bus(lua).borrow().vm;
            let addon = session(lua).borrow().addon.clone();
            if let Err((code, msg)) = require_capability(lua, "eqoa.actions.batch", "eqoa.actions.batch") {
                return error(lua, "eqoa.actions.batch", code, &msg);
            }
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.actions.batch",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let (steps, require_window) = match parse_batch_spec(&spec) {
                Ok(parsed) => parsed,
                Err((code, msg)) => return error(lua, "eqoa.actions.batch", code, &msg),
            };
            for step in &steps {
                let cap = match step {
                    ActionKind::Chat { text } | ActionKind::Command { text } => {
                        super::chat::line_capability(text).to_owned()
                    }
                    _ => format!("eqoa.actions.{}", step.name()),
                };
                if let Err((code, msg)) = require_capability(lua, "eqoa.actions.batch", &cap) {
                    return error(lua, "eqoa.actions.batch", code, &msg);
                }
            }
            match BROKER.with(|b| b.borrow_mut().enqueue_batch(vm, &addon, steps, require_window)) {
                Ok(batch_id) => lua.pack_multi((lua.create_userdata(batch_id)?, Value::Nil)),
                Err((code, msg)) => error(lua, "eqoa.actions.batch", code, &msg),
            }
        })?,
    )?;

    bind(
        lua,
        "actions",
        "stop_batch",
        lua.create_function(|lua, arg: Option<Value>| {
            let caller_vm = super::native_events::bus(lua).borrow().vm;
            let target_id = match arg {
                None | Some(Value::Nil) => None,
                Some(Value::UserData(ud)) => {
                    let Ok(batch) = ud.borrow::<BatchId>() else {
                        return error(
                            lua,
                            "eqoa.actions.stop_batch",
                            "invalid_argument",
                            "batch_id must be a BatchId handle",
                        );
                    };
                    if batch.vm != caller_vm {
                        return error(
                            lua,
                            "eqoa.actions.stop_batch",
                            "permission_denied",
                            "batch handle belongs to another addon",
                        );
                    }
                    Some(batch.id)
                }
                Some(_) => {
                    return error(
                        lua,
                        "eqoa.actions.stop_batch",
                        "invalid_argument",
                        "batch_id must be a BatchId handle",
                    );
                }
            };
            match BROKER.with(|b| b.borrow_mut().stop_batch(caller_vm, target_id)) {
                Ok(stopped) => lua.pack_multi((stopped, Value::Nil)),
                Err((code, msg)) => error(lua, "eqoa.actions.stop_batch", code, &msg),
            }
        })?,
    )?;

    bind(
        lua,
        "actions",
        "batch_status",
        lua.create_function(|lua, arg: Value| {
            let caller_vm = super::native_events::bus(lua).borrow().vm;
            let Value::UserData(ud) = arg else {
                return error(
                    lua,
                    "eqoa.actions.batch_status",
                    "invalid_argument",
                    "batch_id must be a BatchId handle",
                );
            };
            let Ok(batch) = ud.borrow::<BatchId>() else {
                return error(
                    lua,
                    "eqoa.actions.batch_status",
                    "invalid_argument",
                    "batch_id must be a BatchId handle",
                );
            };
            let batch = *batch;
            if batch.vm != caller_vm {
                return error(
                    lua,
                    "eqoa.actions.batch_status",
                    "permission_denied",
                    "batch handle belongs to another addon",
                );
            }
            match BROKER.with(|b| b.borrow().batch_status(batch)) {
                Ok(Some(rec)) => {
                    let tbl = batch_result_to_table(lua, &rec)?;
                    lua.pack_multi((Value::Table(tbl), Value::Nil))
                }
                Ok(None) => error(
                    lua,
                    "eqoa.actions.batch_status",
                    "not_found",
                    "batch result not found or evicted",
                ),
                Err((code, msg)) => error(lua, "eqoa.actions.batch_status", code, &msg),
            }
        })?,
    )?;

    for action_name in ["assist", "target_target"] {
        let sym = format!("eqoa.actions.{action_name}");
        bind(
            lua,
            "actions",
            action_name,
            lua.create_function(move |lua, ()| {
                enqueue_lua(lua, &sym, ActionKind::Assist)
            })?,
        )?;
    }

    bind(
        lua,
        "actions",
        "camera",
        lua.create_function(|lua, opts: Option<Value>| {
            enqueue_lua_with(lua, "eqoa.actions.camera", || {
                let Some(Value::Table(t)) = opts else {
                    return Err((
                        "invalid_argument",
                        "camera requires an options table with heading, pitch, distance, or mode".into(),
                    ));
                };
                let parse_opt_f32 = |key: &str| -> std::result::Result<Option<f32>, (&'static str, String)> {
                    match t.get::<Value>(key) {
                        Ok(Value::Nil) => Ok(None),
                        Ok(Value::Integer(n)) => Ok(Some(n as f32)),
                        Ok(Value::Number(n)) if n.is_finite() => Ok(Some(n as f32)),
                        _ => Err(("invalid_argument", format!("camera.{key} must be a finite number"))),
                    }
                };
                let heading = parse_opt_f32("heading")?;
                let pitch = parse_opt_f32("pitch")?;
                let distance = parse_opt_f32("distance")?;
                if let Some(d) = distance
                    && d < 0.0
                {
                    return Err(("invalid_argument", "camera.distance must be non-negative".into()));
                }
                let mode = match t.get::<Value>("mode") {
                    Ok(Value::Nil) => None,
                    Ok(Value::Integer(m)) if (0..=3).contains(&m) => Some(m as i32),
                    Ok(Value::Number(m)) if m.is_finite() && m.fract() == 0.0 && (0.0..=3.0).contains(&m) => {
                        Some(m as i32)
                    }
                    _ => return Err(("invalid_argument", "camera.mode must be 0, 1, 2, or 3".into())),
                };
                if mode.is_none() && heading.is_none() && pitch.is_none() && distance.is_none() {
                    return Err((
                        "invalid_argument",
                        "camera requires at least one of heading, pitch, distance, or mode".into(),
                    ));
                }
                Ok(ActionKind::Camera {
                    mode,
                    heading,
                    pitch,
                    distance,
                })
            })
        })?,
    )?;
    install_non_item_actions(lua)?;
    install_item_actions(lua)?;
    super::window_actions::install(lua)?;

    Ok(())
}

fn install_item_actions(lua: &Lua) -> Result<()> {
    bind(
        lua,
        "actions",
        "use_item",
        lua.create_function(|lua, item: Value| {
            enqueue_lua_with(lua, "eqoa.actions.use_item", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "inventory" {
                    return Err((
                        "invalid_argument",
                        format!("use_item requires an inventory item handle, got '{}'", spec.list),
                    ));
                }
                Ok(ActionKind::UseItem { item: spec })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "equip",
        lua.create_function(|lua, item: Value| {
            enqueue_lua_with(lua, "eqoa.actions.equip", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "inventory" {
                    return Err((
                        "invalid_argument",
                        format!("equip requires an inventory item handle, got '{}'", spec.list),
                    ));
                }
                Ok(ActionKind::Equip { item: spec })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "swap_items",
        lua.create_function(|lua, (a, b): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.swap_items", || {
                let sa = resolve_item_handle(&a)?;
                let sb = resolve_item_handle(&b)?;
                if sa.list != "inventory" || sb.list != "inventory" {
                    return Err((
                        "invalid_argument",
                        "swap_items requires two inventory item handles".into(),
                    ));
                }
                if sa.inv_index == sb.inv_index {
                    return Err((
                        "invalid_argument",
                        "swap_items requires two distinct item stacks".into(),
                    ));
                }
                Ok(ActionKind::SwapItems { a: sa, b: sb })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "quick_slot",
        lua.create_function(|lua, (item, slot): (Value, i64)| {
            enqueue_lua_with(lua, "eqoa.actions.quick_slot", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "inventory" {
                    return Err((
                        "invalid_argument",
                        "quick_slot requires an inventory item handle".into(),
                    ));
                }
                if !(1..=4).contains(&slot) {
                    return Err(("invalid_argument", "quick_slot must be 1..=4".into()));
                }
                Ok(ActionKind::QuickSlot {
                    item: spec,
                    slot: (slot - 1) as u8,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "loot",
        lua.create_function(|lua, (item, count): (Value, Option<i64>)| {
            enqueue_lua_with(lua, "eqoa.actions.loot", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "loot" {
                    return Err((
                        "invalid_argument",
                        format!("loot requires a loot item handle, got '{}'", spec.list),
                    ));
                }
                if let Some(n) = count
                    && n < 1
                {
                    return Err(("invalid_argument", "loot count must be >= 1".into()));
                }
                Ok(ActionKind::Loot { item: spec, count, identity: None })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "loot_all",
        lua.create_function(|lua, ()| enqueue_lua(lua, "eqoa.actions.loot_all", ActionKind::LootAll))?,
    )?;

    bind(
        lua,
        "actions",
        "buy",
        lua.create_function(|lua, (ware, count): (Value, Option<i64>)| {
            enqueue_lua_with(lua, "eqoa.actions.buy", || {
                let spec = resolve_item_handle(&ware)?;
                if spec.list != "merchant" {
                    return Err((
                        "invalid_argument",
                        format!("buy requires a merchant ware handle, got '{}'", spec.list),
                    ));
                }
                let count = count.unwrap_or(1);
                if count < 1 {
                    return Err(("invalid_argument", "buy count must be >= 1".into()));
                }
                Ok(ActionKind::Buy { ware: spec, count })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "sell",
        lua.create_function(|lua, (item, count): (Value, Option<i64>)| {
            enqueue_lua_with(lua, "eqoa.actions.sell", || {
                let spec = resolve_item_handle(&item)?;
                if spec.list != "inventory" {
                    return Err((
                        "invalid_argument",
                        format!("sell requires an inventory item handle, got '{}'", spec.list),
                    ));
                }
                let count = count.unwrap_or(1);
                if count < 1 {
                    return Err(("invalid_argument", "sell count must be >= 1".into()));
                }
                Ok(ActionKind::Sell { item: spec, count })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "bank_move",
        lua.create_function(|lua, args: mlua::MultiValue| {
            enqueue_lua_with(lua, "eqoa.actions.bank_move", || match args.len() {
                1 | 2 if !args[0].is_string() => {
                    let (item, count): (Value, Option<i64>) = lua
                        .unpack_multi(args)
                        .map_err(|_| ("invalid_argument", "expected (item, count?)".into()))?;
                    let spec = resolve_item_handle(&item)?;
                    let from_bank = match spec.list {
                        "bank" => true,
                        "inventory" => false,
                        other => {
                            return Err((
                                "invalid_argument",
                                format!("bank_move requires an inventory or bank handle, got '{other}'"),
                            ));
                        }
                    };
                    let count = count.unwrap_or(1);
                    if count < 1 {
                        return Err(("invalid_argument", "bank_move count must be >= 1".into()));
                    }
                    Ok(ActionKind::BankMove {
                        from_bank,
                        item: spec,
                        count,
                    })
                }
                2 | 3 => {
                    let (dir, item, count): (String, Value, Option<i64>) = lua
                        .unpack_multi(args)
                        .map_err(|_| ("invalid_argument", "expected (from, item, count?)".into()))?;
                    let spec = resolve_item_handle(&item)?;
                    let want_bank = match dir.trim().to_ascii_lowercase().as_str() {
                        "bank" | "withdraw" => true,
                        "inventory" | "bags" | "deposit" => false,
                        other => {
                            return Err((
                                "invalid_argument",
                                format!("bank_move direction must be 'inventory' or 'bank', got '{other}'"),
                            ));
                        }
                    };
                    let handle_bank = match spec.list {
                        "bank" => true,
                        "inventory" => false,
                        other => {
                            return Err((
                                "invalid_argument",
                                format!("bank_move requires inventory or bank handle, got '{other}'"),
                            ));
                        }
                    };
                    if want_bank != handle_bank {
                        return Err((
                            "invalid_argument",
                            "bank_move direction does not match handle container".into(),
                        ));
                    }
                    let count = count.unwrap_or(1);
                    if count < 1 {
                        return Err(("invalid_argument", "bank_move count must be >= 1".into()));
                    }
                    Ok(ActionKind::BankMove {
                        from_bank: want_bank,
                        item: spec,
                        count,
                    })
                }
                _ => Err(("invalid_argument", "bank_move expects (item, count?) or (from, item, count?)".into())),
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "bank_money",
        lua.create_function(|lua, (dir_val, amount): (Value, i64)| {
            enqueue_lua_with(lua, "eqoa.actions.bank_money", || {
                let dir = match dir_val {
                    Value::String(s) => {
                        let s = s
                            .to_str()
                            .map_err(|_| ("invalid_argument", "direction must be UTF-8".into()))?;
                        match s.trim().to_ascii_lowercase().as_str() {
                            "deposit" => 1u8,
                            "withdraw" => 2u8,
                            other => {
                                return Err((
                                    "invalid_argument",
                                    format!("bank_money direction must be 'deposit' or 'withdraw', got '{other}'"),
                                ));
                            }
                        }
                    }
                    Value::Integer(1) => 1u8,
                    Value::Integer(2) => 2u8,
                    _ => {
                        return Err((
                            "invalid_argument",
                            "bank_money direction must be 'deposit' or 'withdraw'".into(),
                        ));
                    }
                };
                if amount <= 0 {
                    return Err(("invalid_argument", "bank_money amount must be > 0".into()));
                }
                Ok(ActionKind::BankMoney { dir, amount })
            })
        })?,
    )?;

    Ok(())
}

fn install_non_item_actions(lua: &Lua) -> Result<()> {
    bind(
        lua,
        "actions",
        "target",
        lua.create_function(|lua, arg: Value| {
            enqueue_lua_with(lua, "eqoa.actions.target", || {
                Ok(ActionKind::Target(parse_entity_id(arg)?))
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "cycle_target",
        lua.create_function(|lua, filter: Option<String>| {
            enqueue_lua_with(lua, "eqoa.actions.cycle_target", || {
                let (group, norm) = match filter.as_deref().map(str::trim) {
                    None | Some("") | Some("any") | Some("nearby") => (false, None),
                    Some("group") => (true, Some("group".to_string())),
                    Some("corpse") => (false, Some("corpse".to_string())),
                    // The game's next-target key has no hostile or friendly form (upstream
                    // `Play::cycle_target` takes only "group or not"): refused, never cycled unfiltered.
                    Some(k @ ("hostile" | "friendly")) => {
                        return Err((
                            "unsupported",
                            format!("the game has no '{k}' target cycling; use 'group', 'corpse' or no filter"),
                        ));
                    }
                    Some(other) => {
                        return Err((
                            "invalid_argument",
                            format!("unsupported cycle_target filter '{other}'"),
                        ));
                    }
                };
                Ok(ActionKind::CycleTarget { group, filter: norm })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "interact",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.interact", ActionKind::Interact)
        })?,
    )?;

    bind(
        lua,
        "actions",
        "open_loot",
        lua.create_function(|lua, (entity, options): (Option<Value>, Option<Value>)| {
            enqueue_lua_with(lua, "eqoa.actions.open_loot", || {
                let entity_id = match entity {
                    None | Some(Value::Nil) => None,
                    Some(v) => parse_entity_id(v)?,
                };
                let keep_target = match options {
                    None | Some(Value::Nil) => false,
                    Some(Value::Table(t)) => parse_keep_target(t.get::<Value>("keep_target").unwrap_or(Value::Nil))?,
                    Some(_) => {
                        return Err(("invalid_argument", "open_loot options must be a table or nil".into()));
                    }
                };
                Ok(ActionKind::OpenLoot { entity_id, keep_target })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "talk",
        lua.create_function(|lua, (entity, options): (Option<Value>, Option<Value>)| {
            enqueue_lua_with(lua, "eqoa.actions.talk", || {
                let entity_id = match entity {
                    None | Some(Value::Nil) => None,
                    Some(v) => parse_entity_id(v)?,
                };
                let kind = match options {
                    None | Some(Value::Nil) => TalkKind::AnyFriendly,
                    Some(Value::Table(t)) => match t.get::<Value>("kind") {
                        Ok(Value::Nil) | Err(_) => TalkKind::AnyFriendly,
                        Ok(Value::String(s)) => match s.to_str().ok().as_deref() {
                            Some("merchant") => TalkKind::Merchant,
                            Some("bank") => TalkKind::Bank,
                            Some(other) => {
                                return Err((
                                    "invalid_argument",
                                    format!("talk options.kind must be 'merchant' or 'bank', got '{other}'"),
                                ));
                            }
                            None => return Err(("invalid_argument", "talk options.kind must be UTF-8".into())),
                        },
                        Ok(_) => return Err(("invalid_argument", "talk options.kind must be a string".into())),
                    },
                    Some(_) => return Err(("invalid_argument", "talk options must be a table or nil".into())),
                };
                Ok(ActionKind::Talk { entity_id, kind })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "attack",
        lua.create_function(|lua, on: Option<bool>| {
            enqueue_lua(lua, "eqoa.actions.attack", ActionKind::Attack { on })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "cast",
        lua.create_function(|lua, ability: Value| {
            enqueue_lua_with(lua, "eqoa.actions.cast", || {
                Ok(ActionKind::Cast(parse_ability_lookup(ability, false)?))
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "use_hotbar",
        lua.create_function(|lua, args: mlua::MultiValue| {
            enqueue_lua_with(lua, "eqoa.actions.use_hotbar", || match args.len() {
                1 => {
                    let s: i64 = lua
                        .unpack_multi(args)
                        .map_err(|_| ("invalid_argument", "slot must be 1..=20".into()))?;
                    if !(1..=20).contains(&s) {
                        return Err(("invalid_argument", "hotbar slot must be 1..=20".into()));
                    }
                    Ok(ActionKind::UseHotbar {
                        bar: ((s - 1) / 5) as usize,
                        slot: ((s - 1) % 5) as usize,
                    })
                }
                2 => {
                    let (bar, slot): (i64, i64) = lua
                        .unpack_multi(args)
                        .map_err(|_| ("invalid_argument", "bar and slot must be integers".into()))?;
                    if !(1..=4).contains(&bar) || !(1..=5).contains(&slot) {
                        return Err(("invalid_argument", "hotbar (bar, slot) must be 1..=4, 1..=5".into()));
                    }
                    Ok(ActionKind::UseHotbar {
                        bar: (bar - 1) as usize,
                        slot: (slot - 1) as usize,
                    })
                }
                _ => Err(("invalid_argument", "use_hotbar expects (slot) or (bar, slot)".into())),
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "chat",
        lua.create_function(|lua, (text, channel): (Value, Value)| {
            if !channel.is_nil() {
                return error(
                    lua,
                    "eqoa.actions.chat",
                    "unsupported",
                    "use eqoa.actions.command with the channel's slash command to select a channel",
                );
            }
            enqueue_chat_line(lua, text, false)
        })?,
    )?;

    bind(
        lua,
        "actions",
        "command",
        lua.create_function(|lua, cmd: Value| enqueue_chat_line(lua, cmd, true))?,
    )?;

    bind(
        lua,
        "actions",
        "autorun",
        lua.create_function(|lua, on: Option<bool>| {
            enqueue_lua(lua, "eqoa.actions.autorun", ActionKind::Autorun { on })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "follow",
        lua.create_function(|lua, entity: Value| {
            enqueue_lua_with(lua, "eqoa.actions.follow", || {
                Ok(ActionKind::Follow {
                    target_id: parse_entity_id(entity)?,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "stop",
        lua.create_function(|lua, ()| enqueue_lua(lua, "eqoa.actions.stop", ActionKind::Stop))?,
    )?;

    bind(
        lua,
        "actions",
        "open_window",
        lua.create_function(|lua, (kind, arg): (Value, Option<i32>)| {
            enqueue_lua_with(lua, "eqoa.actions.open_window", || {
                let Value::String(s) = kind else {
                    return Err(("invalid_argument", "window kind must be a string".into()));
                };
                let kind = s
                    .to_str()
                    .map_err(|_| ("invalid_argument", "window kind must be UTF-8".into()))?
                    .trim()
                    .to_ascii_lowercase();
                if !matches!(
                    kind.as_str(),
                    "summary"
                        | "trainer"
                        | "skills"
                        | "map"
                        | "inventory"
                        | "bags"
                        | "start"
                        | "menu"
                        | "abilities"
                        | "spellbook"
                        | "quests"
                        | "mastery"
                        | "factions"
                        | "options"
                        | "friends"
                        | "buddies"
                        | "ignore"
                        | "guild"
                        | "mail"
                        | "mail_compose"
                        | "tell"
                        | "reply"
                        | "auction"
                        | "auction_browse"
                        | "auction_search"
                        | "support"
                        | "support_inbox"
                        | "who"
                        | "search"
                        | "recipes"
                        | "experiment"
                ) {
                    return Err((
                        "invalid_argument",
                        format!("unknown or unsupported window kind '{kind}'"),
                    ));
                }
                Ok(ActionKind::OpenWindow {
                    kind,
                    arg: arg.unwrap_or(0),
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "weapon_set",
        lua.create_function(|lua, set: Value| {
            enqueue_lua_with(lua, "eqoa.actions.weapon_set", || {
                Ok(ActionKind::WeaponSet {
                    set: parse_weapon_set(set)?,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "prepare_weapon_set",
        lua.create_function(|lua, (set, name): (i64, String)| {
            enqueue_lua_with(lua, "eqoa.actions.prepare_weapon_set", || {
                if !(1..=crate::gear::SETS as i64).contains(&set) {
                    return Err(("invalid_argument", "weapon set slot must be 1..=4".into()));
                }
                let trimmed = name.trim().to_string();
                if trimmed.is_empty()
                    || trimmed.chars().count() > crate::gear::NAME_MAX
                    || trimmed.chars().any(char::is_control)
                {
                    return Err((
                        "invalid_argument",
                        format!(
                            "weapon set name must be 1..={} non-control characters",
                            crate::gear::NAME_MAX
                        ),
                    ));
                }
                Ok(ActionKind::PrepareWeaponSet {
                    set: (set - 1) as i32,
                    name: trimmed,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "assign_ability",
        lua.create_function(|lua, (slot, ability): (i64, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.assign_ability", || {
                if !(1..=20).contains(&slot) {
                    return Err(("invalid_argument", "hotbar slot must be 1..=20".into()));
                }
                Ok(ActionKind::AssignAbility {
                    slot: slot - 1,
                    ability: parse_ability_lookup(ability, false)?,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "unmemorize_ability",
        lua.create_function(|lua, target: Value| {
            enqueue_lua_with(lua, "eqoa.actions.unmemorize_ability", || match target {
                Value::Integer(n) if (1..=20).contains(&n) => Ok(ActionKind::UnmemorizeAbility {
                    slot: Some(n - 1),
                    ability: None,
                }),
                Value::Number(n)
                    if n.is_finite() && n.fract() == 0.0 && (1.0..=20.0).contains(&n) =>
                {
                    Ok(ActionKind::UnmemorizeAbility {
                        slot: Some((n as i64) - 1),
                        ability: None,
                    })
                }
                other => Ok(ActionKind::UnmemorizeAbility {
                    slot: None,
                    ability: Some(parse_ability_lookup(other, false)?),
                }),
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "unmemorize_all",
        lua.create_function(|lua, ()| {
            enqueue_lua(lua, "eqoa.actions.unmemorize_all", ActionKind::UnmemorizeAll)
        })?,
    )?;

    bind(
        lua,
        "actions",
        "arrange_ability",
        lua.create_function(|lua, (from, to): (Value, Value)| {
            enqueue_lua_with(lua, "eqoa.actions.arrange_ability", || {
                Ok(ActionKind::ArrangeAbility {
                    from: parse_ability_lookup(from, true)?,
                    to: parse_ability_lookup(to, true)?,
                })
            })
        })?,
    )?;

    Ok(())
}

/// The player turned on "Use the game's windows": batches that worked through a claim it released stop.
pub(crate) fn stop_batches_of_released_claims(released: &[(u64, super::window_takeover::TakeoverKind)]) {
    if released.is_empty() {
        return;
    }
    BROKER.with(|b| b.borrow_mut().stop_batches_of_released_claims(released));
}

#[cfg(test)]
pub(crate) fn reset_all_for_test() {
    MASTER_LOOT_ACTIVE.with(|m| m.set(false));
    CURRENT_CORPSE_TARGET.with(|c| c.set(None));
    EXPIRED_LOOT_TIMEOUT.with(|t| t.set(false));
    BROKER.with(|b| *b.borrow_mut() = BrokerState::default());
}

#[cfg(test)]
pub(crate) fn queued_count_for_test() -> usize {
    BROKER.with(|b| b.borrow().vms.values().map(|v| v.queued.len()).sum())
}

#[cfg(test)]
pub(crate) fn queued_kinds_for_test() -> Vec<&'static str> {
    BROKER.with(|b| {
        b.borrow()
            .vms
            .values()
            .flat_map(|v| v.queued.iter().map(|q| q.kind.name()))
            .collect()
    })
}

#[cfg(test)]
pub(crate) fn dispatch_all_for_test() -> Vec<&'static str> {
    let snap = NativeSnapshot {
        in_world: true,
        mode: 0,
        ..Default::default()
    };
    let mut dispatched = Vec::new();
    BROKER.with(|b| {
        b.borrow_mut().step_tick(snap, |kind| {
            dispatched.push(kind.name());
            DispatchOutcome::Completed { detail: None }
        });
    });
    dispatched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::{
        gesture::{GestureKind, enter_gesture, is_forbidden_report_text},
        native_core::{CORE, Core},
        native_events,
    };
    use serde_json::json;

    #[derive(Clone)]
    struct TestSubsystemAction;

    fn enqueue_raw(
        vm: u64,
        addon_id: &str,
        kind: ActionKind,
    ) -> std::result::Result<RequestId, (&'static str, String)> {
        BROKER.with(|b| b.borrow_mut().enqueue(vm, addon_id, kind))
    }

    fn cap_host(id: &str, caps: &[&str]) -> Host {
        let root = std::env::temp_dir();
        Host::for_addon(&super::super::discovery::DiscoveredAddon {
            id: id.into(),
            name: id.into(),
            entry: root.join("main.lua"),
            script_root: root.clone(),
            package_root: root.clone(),
            module_dirs: vec![root.clone()],
            resource_dirs: vec![root],
            api: Some("eqoa/1".into()),
            entry_mode: super::super::discovery::EntryMode::Once,
            capabilities_declared: true,
            requested_capabilities: caps.iter().map(|s| (*s).to_string()).collect(),
            extensions: Default::default(),
            initially_enabled: true,
            mods_root: None,
        })
        .unwrap()
    }

    #[test]
    fn round65_broker_allows_autonomous_actions_with_capability_and_refuses_without() {
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| {
            let mut core = Core::default();
            core.view.in_world = true;
            *c.borrow_mut() = core;
        });

        // Negative control: mod without eqoa.actions.* or eqoa.actions.assist is refused + diagnosed.
        let denied = cap_host("r65_denied", &["eqoa.player.get"]);
        denied
            .execute(
                r#"
            local r, err = eqoa.actions.assist()
            assert(r == nil and err.code == "permission_denied")
            local b, berr = eqoa.actions.batch({ { kind = "assist" } })
            assert(b == nil and berr.code == "permission_denied")
        "#,
            )
            .unwrap();
        let logs = denied.drain_logs().unwrap();
        assert!(
            logs.iter()
                .any(|line| line.contains("eqoa.actions.assist")
                    && line.contains("permission_denied")),
            "undeclared action capability must record visible diagnostic: {logs:?}"
        );

        // Specific action capability grants that action outside any gesture, and refuses others.
        let specific = cap_host("r65_specific", &["eqoa.actions.assist"]);
        specific
            .execute(
                r#"
            local r, err = eqoa.actions.assist()
            assert(r ~= nil and err == nil)
            local r2, err2 = eqoa.actions.assist()
            assert(r2 ~= nil and err2 == nil)
            local bad, berr = eqoa.actions.interact()
            assert(bad == nil and berr.code == "permission_denied")
        "#,
            )
            .unwrap();

        // Wildcard capability grants actions and batches autonomously from render/event callbacks.
        let host = cap_host("r65_wild", &["eqoa.actions.*"]);
        let vm = crate::fork_ext::addons::native_events::bus(&host.lua)
            .borrow()
            .vm;
        host.execute(
            r#"
            local r, err = eqoa.actions.assist()
            assert(r ~= nil and err == nil)
            eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function()
                key_req, key_err = eqoa.actions.assist()
                second_req, second_err = eqoa.actions.assist()
            end)
        "#,
        )
        .unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
        host.execute(
            "assert(key_req ~= nil and key_err == nil); assert(second_req ~= nil and second_err == nil)",
        )
        .unwrap();
        assert!(enqueue_raw(vm, "r65_wild", ActionKind::Assist).is_ok());

        // Synthetic report guard still holds even with eqoa.actions.*.
        host.execute(
            r#"
            local bug_r, bug_e = eqoa.actions.command("/bug test")
            assert(bug_r == nil and bug_e.code == "permission_denied")
        "#,
        )
        .unwrap();
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
    }

    #[test]
    fn round65_undeclared_capabilities_refuses_actions_and_takeover_with_not_declared() {
        reset_all_for_test();
        super::super::window_takeover::reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| {
            let mut core = Core::default();
            core.view.in_world = true;
            *c.borrow_mut() = core;
        });

        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("eqoa-r65-not-declared-{uniq}"));
        let mod_no_caps = root.join("no_caps");
        let mod_explicit = root.join("explicit_caps");
        std::fs::create_dir_all(&mod_no_caps).unwrap();
        std::fs::create_dir_all(&mod_explicit).unwrap();
        std::fs::write(mod_no_caps.join("main.lua"), "").unwrap();
        std::fs::write(mod_explicit.join("main.lua"), "").unwrap();
        std::fs::write(
            mod_no_caps.join("mod.json"),
            r#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"no_caps","name":"No Caps","entry":"main.lua","entryMode":"once"}"#,
        )
        .unwrap();
        std::fs::write(
            mod_explicit.join("mod.json"),
            r#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"explicit_caps","name":"Explicit Caps","entry":"main.lua","entryMode":"once","capabilities":["eqoa.actions.assist","eqoa.windows.takeover"]}"#,
        )
        .unwrap();

        let disc = super::super::discovery::discover(&root);
        let a_no_caps = disc
            .addons
            .iter()
            .find(|a| a.id == "no_caps")
            .expect("discovered mod without capabilities");
        let a_explicit = disc
            .addons
            .iter()
            .find(|a| a.id == "explicit_caps")
            .expect("discovered mod with explicit capabilities");
        assert!(!a_no_caps.capabilities_declared);
        assert_eq!(a_no_caps.declared_capabilities(), None);
        assert_eq!(
            super::super::runtime::autonomous_action_label(
                a_no_caps.declared_capabilities().unwrap_or(&[])
            ),
            None
        );
        assert!(a_explicit.capabilities_declared);
        assert_eq!(
            super::super::runtime::autonomous_action_label(
                a_explicit.declared_capabilities().unwrap_or(&[])
            )
            .as_deref(),
            Some("Actions: can act on its own (assist)")
        );

        let h_no_caps = Host::for_addon(a_no_caps).unwrap();
        h_no_caps
            .execute(
                r#"
            -- Reads keep their normal behavior.
            local _, r_err = eqoa.player.get()
            assert(r_err == nil or r_err.code ~= "not_declared")
            -- Actions and takeover refuse visibly with not_declared.
            local r1, e1 = eqoa.actions.assist()
            assert(r1 == nil and e1.code == "not_declared", "assist without capabilities list must refuse with not_declared")
            local r2, e2 = eqoa.actions.open_loot()
            assert(r2 == nil and e2.code == "not_declared")
            local r3, e3 = eqoa.actions.talk()
            assert(r3 == nil and e3.code == "not_declared")
            local r4, e4 = eqoa.actions.batch({ { kind = "assist" } })
            assert(r4 == nil and e4.code == "not_declared")
            local r5, e5 = eqoa.windows.takeover("loot")
            assert(r5 == nil and e5.code == "not_declared", "takeover without capabilities list must refuse with not_declared")
        "#,
            )
            .unwrap();
        let logs = h_no_caps.drain_logs().unwrap();
        assert!(
            logs.iter().any(|l| l.contains("not_declared") && l.contains("eqoa.actions.assist")),
            "expected visible not_declared diagnostic for assist: {logs:?}"
        );
        assert!(
            logs.iter().any(|l| l.contains("not_declared") && l.contains("eqoa.windows.takeover")),
            "expected visible not_declared diagnostic for takeover: {logs:?}"
        );

        // Bare Host::new() (no mod.json) also refuses actions and takeover with not_declared.
        let bare = Host::new().unwrap();
        bare.execute(
            r#"
            local r, e = eqoa.actions.assist()
            assert(r == nil and e.code == "not_declared")
            local t, te = eqoa.windows.takeover("loot")
            assert(t == nil and te.code == "not_declared")
        "#,
        )
        .unwrap();

        // Explicit mod.json grants only declared capabilities; other actions return permission_denied.
        let h_explicit = Host::for_addon(a_explicit).unwrap();
        h_explicit
            .execute(
                r#"
            local r_ok, e_ok = eqoa.actions.assist()
            assert(r_ok ~= nil and e_ok == nil)
            local t_ok, te_ok = eqoa.windows.takeover("loot")
            assert(t_ok == true and te_ok == nil)
            local r_no, e_no = eqoa.actions.open_loot()
            assert(r_no == nil and e_no.code == "permission_denied")
        "#,
            )
            .unwrap();

        let _ = std::fs::remove_dir_all(&root);
        reset_all_for_test();
        super::super::window_takeover::reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
    }

    #[test]
    fn round65_multiple_listeners_can_dispatch_actions_autonomously() {
        use super::super::native_events;
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| c.borrow_mut().view.in_world = true);

        let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
        crate::fork_ext::addons::pointer_api::position([40.0, 40.0], true, (800, 600));
        host.execute(
            r#"
            p_res, k_res = {}, {}
            assert(eqoa.input.claim_pointer({ x = 0, y = 0, width = 200, height = 200 }))
            eqoa.events.on("pointer.pressed", function()
                local req, err = eqoa.actions.assist()
                p_res[#p_res + 1] = { ok = req ~= nil, code = err and err.code }
            end)
            eqoa.events.on("pointer.pressed", function()
                local req, err = eqoa.actions.assist()
                p_res[#p_res + 1] = { ok = req ~= nil, code = err and err.code }
            end)
            eqoa.input.bind({ id = "b1", label = "Bind 1", default_key = "y" }, function()
                local req, err = eqoa.actions.assist()
                k_res[#k_res + 1] = { ok = req ~= nil, code = err and err.code }
            end)
            eqoa.input.bind({ id = "b2", label = "Bind 2", default_key = "9" }, function()
                local req, err = eqoa.actions.assist()
                k_res[#k_res + 1] = { ok = req ~= nil, code = err and err.code }
            end)
        "#,
        )
        .unwrap();

        CORE.with(|c| {
            c.borrow_mut().emit(
                "pointer.pressed",
                json!({"x":40.0,"y":40.0,"inside_window":true,"button":"left","steps":0}),
            );
        });
        native_events::dispatch(&host).unwrap();
        host.execute("assert(#p_res == 2 and p_res[1].ok == true and p_res[2].ok == true)")
            .unwrap();

        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                json!({"key":"y","down":true,"repeat":false,"synthetic":false,"text":""}),
            );
            c.borrow_mut().emit(
                "input.key",
                json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
            );
        });
        native_events::dispatch(&host).unwrap();
        host.execute("assert(#k_res == 2 and k_res[1].ok == true and k_res[2].ok == true)")
            .unwrap();

        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
    }

    #[test]
    fn round6b_broker_rate_limits_fairness_retention_cancel_and_reset() {
        reset_all_for_test();
        CORE.with(|c| c.borrow_mut().view.in_world = true);
        let snap = NativeSnapshot {
            in_world: true,
            mode: 0,
            ..Default::default()
        };

        // 1. Burst 8/tick, refill 1/tick (30/s), retain 256, queue cap 32, cancel & clear_vm
        let mut ids = Vec::new();
        for _ in 0..32 {
            let _g = enter_gesture(1, "mod_a", GestureKind::ModKey, true);
            ids.push(enqueue_raw(1, "mod_a", ActionKind::Assist).unwrap());
        }
        {
            let _g = enter_gesture(1, "mod_a", GestureKind::ModKey, true);
            let overflow = enqueue_raw(1, "mod_a", ActionKind::Assist).unwrap_err();
            assert_eq!(overflow.0, "busy");
        }
        // Cancel #32 before dispatch
        assert_eq!(BROKER.with(|b| b.borrow_mut().cancel(ids[31])).unwrap(), Some(true));
        assert_eq!(
            BROKER.with(|b| b.borrow().status(ids[31]).unwrap()).unwrap().state,
            "cancelled"
        );

        let mut dispatched = 0usize;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 8, "burst allows 8 dispatches on first tick");

        let mut next_tick = 0usize;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                next_tick += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(next_tick, 1, "token bucket refills 1 token per 30 Hz tick (30/s)");

        // 2. Round-robin fairness & global 64/tick cap across 10 mods (10 * 8 = 80 > 64) -> >1s rate_limited
        reset_all_for_test();
        let mut per_mod_reqs: Vec<Vec<RequestId>> = Vec::new();
        for m in 1..=10u64 {
            let mut list = Vec::new();
            for _ in 0..8 {
                let _g = enter_gesture(m, &format!("m{m}"), GestureKind::ModKey, true);
                list.push(enqueue_raw(m, &format!("m{m}"), ActionKind::Assist).unwrap());
            }
            per_mod_reqs.push(list);
        }
        let mut per_mod_dispatched: HashMap<u64, usize> = HashMap::new();
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| DispatchOutcome::Completed { detail: None });
        });
        for (idx, list) in per_mod_reqs.iter().enumerate() {
            let count = list
                .iter()
                .filter(|id| {
                    BROKER
                        .with(|b| b.borrow().status(**id).unwrap())
                        .is_some_and(|r| r.state == "completed")
                })
                .count();
            per_mod_dispatched.insert(idx as u64 + 1, count);
        }
        let total: usize = per_mod_dispatched.values().sum();
        assert_eq!(total, 64, "global cap must stop at 64/tick");
        assert!(
            per_mod_dispatched.values().all(|&c| (6..=8).contains(&c)),
            "round-robin fairness shares the 64-slot tick across all 10 mods: {per_mod_dispatched:?}"
        );

        // Hold one mod throttled (> 30 ticks without refill) -> `rate_limited`
        reset_all_for_test();
        let held_req = {
            let _g = enter_gesture(77, "mod_rate", GestureKind::ModKey, true);
            enqueue_raw(77, "mod_rate", ActionKind::Assist).unwrap()
        };
        for _ in 0..=RATE_LIMIT_WAIT_TICKS {
            BROKER.with(|b| {
                let mut b = b.borrow_mut();
                let next_tick = b.tick + 1;
                let vm = b.vms.get_mut(&77).unwrap();
                vm.tokens = 0;
                vm.last_refill_tick = next_tick;
                b.step_tick(snap, |_| DispatchOutcome::Completed { detail: None });
            });
        }
        let rec = BROKER.with(|b| b.borrow().status(held_req).unwrap()).unwrap();
        assert_eq!(rec.state, "refused");
        assert_eq!(rec.code.as_deref(), Some("rate_limited"));

        // 3. Retention cap 256 per VM and reset_session clears without replay
        reset_all_for_test();
        let mut all_ids = Vec::new();
        for _ in 0..260 {
            let _g = enter_gesture(9, "ret", GestureKind::ModKey, true);
            let id = enqueue_raw(9, "ret", ActionKind::Assist).unwrap();
            all_ids.push(id);
            BROKER.with(|b| {
                b.borrow_mut()
                    .step_tick(snap, |_| DispatchOutcome::Completed { detail: None });
            });
        }
        assert!(BROKER.with(|b| b.borrow().status(all_ids[0]).unwrap()).is_none());
        assert!(BROKER.with(|b| b.borrow().status(all_ids[3]).unwrap()).is_none());
        assert!(BROKER.with(|b| b.borrow().status(all_ids[4]).unwrap()).is_some());
        assert!(BROKER.with(|b| b.borrow().status(all_ids[259]).unwrap()).is_some());

        // Zone/reset clears queued actions so next tick dispatches nothing
        let queued_before_zone = {
            let _g = enter_gesture(9, "ret", GestureKind::ModKey, true);
            enqueue_raw(9, "ret", ActionKind::Assist).unwrap()
        };
        reset_session("zone");
        assert_eq!(
            BROKER
                .with(|b| b.borrow().status(queued_before_zone).unwrap())
                .unwrap()
                .state,
            "cancelled"
        );
        let mut replayed = 0;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                replayed += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(replayed, 0, "cleared actions must never replay after zone/reset");
        reset_all_for_test();
    }

    #[test]
    fn round6b_broker_server_timeout_and_subsystem_locks() {
        reset_all_for_test();
        CORE.with(|c| c.borrow_mut().view.in_world = true);
        let snap = NativeSnapshot {
            in_world: true,
            mode: 0,
            ..Default::default()
        };

        // 1. In-flight action times out after 300 ticks (10 s on 30 Hz clock) -> `unknown`
        let req = {
            let _g = enter_gesture(11, "srv", GestureKind::ModKey, true);
            enqueue_raw(11, "srv", ActionKind::Assist).unwrap()
        };
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| DispatchOutcome::AwaitingServer {
                matcher: ServerMatcher::Assist,
                detail: None,
            });
        });
        assert_eq!(
            BROKER.with(|b| b.borrow().status(req).unwrap()).unwrap().state,
            "dispatched"
        );
        for _ in 0..(SERVER_TIMEOUT_TICKS - 1) {
            BROKER.with(|b| {
                b.borrow_mut()
                    .step_tick(snap, |_| DispatchOutcome::Completed { detail: None });
            });
        }
        assert_eq!(
            BROKER.with(|b| b.borrow().status(req).unwrap()).unwrap().state,
            "dispatched",
            "still waiting before 300th tick"
        );
        BROKER.with(|b| {
            b.borrow_mut()
                .step_tick(snap, |_| DispatchOutcome::Completed { detail: None });
        });
        let timed_out = BROKER.with(|b| b.borrow().status(req).unwrap()).unwrap();
        assert_eq!(timed_out.state, "unknown");
        assert_eq!(timed_out.phase, "server");
        assert_eq!(timed_out.ok, None);
        assert_eq!(timed_out.code.as_deref(), Some("unknown"));

        // 2. Native busy snapshot blocks subsystem
        BROKER.with(|b| {
            let mut b = b.borrow_mut();
            b.native.bank_busy = true;
            assert!(b.subsystem_busy(Subsystem::Bank));
            assert!(!b.subsystem_busy(Subsystem::Merchant));
            b.native.bank_busy = false;
            b.native.merchant_pending = true;
            assert!(b.subsystem_busy(Subsystem::Merchant));
        });
        reset_all_for_test();
    }

    #[test]
    fn round6b_broker_synthetic_report_guard_rejects_routes() {
        for forbidden in [
            "/bug",
            "  /bug stuck here",
            "#bug",
            "#BUG report",
            "/feature add pony",
            "#feature idea",
            "/say /bug test",
            "/sa #bug",
            "/tell Alice /bug hidden",
            "/t Alice #bug",
            "/w Bob #feature",
            "/g /bug",
            "/group #bug",
            "/p /bug",
            "/gu #bug",
            "/shout /bug",
            "/r #bug",
        ] {
            assert!(
                is_forbidden_report_text(forbidden),
                "expected synthetic report route to be blocked: {forbidden:?}"
            );
        }
        for allowed in [
            "/say hello there",
            "/tell Alice found a beetle bug near Qeynos",
            "/g /assist Bob",
            "just plain chat with #hashtags",
        ] {
            assert!(
                !is_forbidden_report_text(allowed),
                "expected normal message to be allowed: {allowed:?}"
            );
        }
    }

    #[test]
    fn round6b_capabilities_and_descriptors_cover_actions_result_and_events() {
        reset_all_for_test();
        let host = Host::new().unwrap();
        let supported = [
            "eqoa.actions.status",
            "eqoa.actions.cancel",
            "eqoa.actions.RequestId",
            "eqoa.actions.ActionResult",
            "eqoa.events.action.result",
            "eqoa.actions.target",
            "eqoa.actions.cycle_target",
            "eqoa.actions.assist",
            "eqoa.actions.target_target",
            "eqoa.actions.interact",
            "eqoa.actions.attack",
            "eqoa.actions.cast",
            "eqoa.actions.use_hotbar",
            "eqoa.actions.chat",
            "eqoa.actions.command",
            "eqoa.actions.autorun",
            "eqoa.actions.follow",
            "eqoa.actions.stop",
            "eqoa.actions.camera",
            "eqoa.actions.open_window",
            "eqoa.actions.weapon_set",
            "eqoa.actions.prepare_weapon_set",
            "eqoa.actions.assign_ability",
            "eqoa.actions.unmemorize_ability",
            "eqoa.actions.unmemorize_all",
            "eqoa.actions.arrange_ability",
            "eqoa.actions.use_item",
            "eqoa.actions.equip",
            "eqoa.actions.swap_items",
            "eqoa.actions.quick_slot",
            "eqoa.actions.loot",
            "eqoa.actions.loot_all",
            "eqoa.actions.buy",
            "eqoa.actions.sell",
            "eqoa.actions.bank_move",
            "eqoa.actions.bank_money",
        ];
        for sym in supported {
            let status: String = host
                .lua
                .load(format!("return assert(eqoa.capabilities.get('{sym}')).status"))
                .eval()
                .unwrap();
            assert_eq!(status, "supported", "{sym} must be supported");
        }
        for field in ACTION_RESULT_KEYS {
            let sym = format!("eqoa.actions.ActionResult.{field}");
            let status: String = host
                .lua
                .load(format!("return assert(eqoa.capabilities.get('{sym}')).status"))
                .eval()
                .unwrap();
            assert_eq!(status, "supported", "{sym} must be supported");
        }
        let destroy_status: String = host
            .lua
            .load("return assert(eqoa.capabilities.get('eqoa.actions.destroy')).status")
            .eval()
            .unwrap();
        assert_eq!(destroy_status, "unsupported");
        for not_yet in [
            "eqoa.actions.move",
            "eqoa.actions.look",
            "eqoa.actions.buttons",
        ] {
            let status: String = host
                .lua
                .load(format!("return assert(eqoa.capabilities.get('{not_yet}')).status"))
                .eval()
                .unwrap();
            assert_eq!(status, "not_yet", "{not_yet} must be not_yet");
        }
        reset_all_for_test();
    }

    #[test]
    fn round6c_batch_gesture_single_batch_refuses_second_serial_execution_and_capabilities() {
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| {
            let mut core = Core::default();
            core.view.in_world = true;
            *c.borrow_mut() = core;
        });

        let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();

        // Capabilities for Part C batch symbols & events
        for sym in [
            "eqoa.actions.batch",
            "eqoa.actions.stop_batch",
            "eqoa.actions.batch_status",
            "eqoa.actions.BatchId",
            "eqoa.actions.BatchResult",
            "eqoa.events.batch.progress",
            "eqoa.events.batch.result",
        ] {
            let status: String = host
                .lua
                .load(format!("return assert(eqoa.capabilities.get('{sym}')).status"))
                .eval()
                .unwrap();
            assert_eq!(status, "supported", "{sym} must be supported");
        }
        for field in BATCH_RESULT_KEYS {
            let sym = format!("eqoa.actions.BatchResult.{field}");
            let status: String = host
                .lua
                .load(format!("return assert(eqoa.capabilities.get('{sym}')).status"))
                .eval()
                .unwrap();
            // a batch never fills detail: each step reports in its own action.result (round 7C fix M1)
            let expected = if *field == "detail" { "unsupported" } else { "supported" };
            assert_eq!(status, expected, "{sym} must be {expected}");
        }

        // Batch starts autonomously; 2nd batch while 1st runs -> busy; 3-step serial execution waits for server reply.
        host.execute(
            r#"
            bprog, bres = {}, {}
            eqoa.events.on("batch.progress", function(e) bprog[#bprog + 1] = e.data end)
            eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
            eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function()
                if mode == "first" then
                    b1, err1 = eqoa.actions.batch({
                        { kind = "assist" },
                        { kind = "assist" },
                        { kind = "assist" },
                    })
                elseif mode == "second_while_running" then
                    b2, err2 = eqoa.actions.batch({ { kind = "assist" } })
                end
            end)
            mode = "first"
        "#,
        )
        .unwrap();

        let press_k = || {
            CORE.with(|c| {
                c.borrow_mut().emit(
                    "input.key",
                    json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
                )
            });
            native_events::dispatch(&host).unwrap();
        };
        press_k();
        host.execute(
            r#"
            assert(b1 ~= nil and err1 == nil)
            local st = assert(eqoa.actions.batch_status(b1))
            assert(st.state == "running" and st.completed == 0 and st.total == 3)
            assert(eqoa.actions.status(b1).state == "running")
            mode = "second_while_running"
        "#,
        )
        .unwrap();

        press_k();
        host.execute("assert(b2 == nil and err2.code == 'busy')").unwrap();

        let snap = NativeSnapshot {
            in_world: true,
            mode: 0,
            ..Default::default()
        };
        // Tick 1: dispatches step 1 ONLY (awaits server); step 2 must NOT run yet.
        let mut dispatched = 0usize;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                dispatched += 1;
                DispatchOutcome::AwaitingServer {
                    matcher: ServerMatcher::Assist,
                    detail: None,
                }
            });
        });
        assert_eq!(dispatched, 1, "only step 1 dispatches while awaiting server");
        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 1, "step 2 must wait until step 1 server reply arrives");

        // Resolve step 1 -> tick advances to step 2 (sync completed) -> tick 3 finishes step 3!
        resolve_server_assist(101);
        native_events::dispatch(&host).unwrap();
        host.execute("assert(#bprog >= 1 and bprog[#bprog].completed == 1 and bprog[#bprog].state == 'running')").unwrap();

        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 2);
        native_events::dispatch(&host).unwrap();
        host.execute("assert(bprog[#bprog].completed == 2 and #bres == 0)").unwrap();

        BROKER.with(|b| {
            b.borrow_mut().step_tick(snap, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 3);
        native_events::dispatch(&host).unwrap();
        host.execute(
            r#"
            assert(#bres == 1)
            assert(bres[1].batch_id == b1)
            assert(bres[1].state == "completed" and bres[1].ok == true)
            assert(bres[1].completed == 3 and bres[1].total == 3)
            assert(eqoa.actions.batch_status(b1).state == "completed")
        "#,
        )
        .unwrap();

        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
    }

    /// Fix round 3 item 4b: two batches of one mod stopped while each had a step on the wire both report,
    /// each once, when their replies land; the second stop does not lose the first's result.
    #[test]
    fn round7c_every_stopped_batch_reports_its_late_reply() {
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| {
            let mut core = Core::default();
            core.view.in_world = true;
            *c.borrow_mut() = core;
        });
        let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
        host.execute(
            r#"
            bres = {}
            eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
            eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function()
                batches[#batches + 1] = assert(eqoa.actions.batch({ steps = { { kind = "assist" }, { kind = "assist" } } }))
            end)
            batches = {}
        "#,
        )
        .unwrap();
        let in_world = NativeSnapshot { in_world: true, mode: 0, ..Default::default() };
        for _ in 0..2 {
            CORE.with(|c| {
                c.borrow_mut().emit(
                    "input.key",
                    json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
                )
            });
            native_events::dispatch(&host).unwrap();
            BROKER.with(|b| {
                b.borrow_mut().step_tick(in_world, &mut |_: &ActionKind| DispatchOutcome::AwaitingServer {
                    matcher: ServerMatcher::Assist,
                    detail: None,
                })
            });
            host.execute("assert(eqoa.actions.stop_batch(batches[#batches]))").unwrap();
        }
        native_events::dispatch(&host).unwrap();
        host.execute("assert(#bres == 0, 'each result waits for its reply: ' .. #bres)").unwrap();
        resolve_server_assist(7);
        resolve_server_assist(7);
        native_events::dispatch(&host).unwrap();
        host.execute(
            r#"
            assert(#bres == 2, 'both stopped batches report: ' .. #bres)
            for i, r in ipairs(bres) do
                assert(r.state == 'stopped' and r.completed == 1, i .. ': ' .. r.state .. ' ' .. r.completed)
            end
        "#,
        )
        .unwrap();
        reset_all_for_test();
    }

    #[test]
    fn round6c_batch_stops_on_refusal_window_close_stop_call_and_zoning_without_replay() {
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| {
            let mut core = Core::default();
            core.view.in_world = true;
            *c.borrow_mut() = core;
        });

        let host = Host::with_test_capabilities(&["eqoa.actions.*"]).unwrap();
        host.execute(
            r#"
            bres = {}
            eqoa.events.on("batch.result", function(e) bres[#bres + 1] = e.data end)
            eqoa.input.bind({ id = "go", label = "Go", default_key = "9" }, function()
                last_b, last_err = eqoa.actions.batch({
                    steps = {
                        { kind = "assist" },
                        { kind = "assist" },
                        { kind = "assist" },
                    },
                    require_window = req_win,
                })
            end)
        "#,
        )
        .unwrap();

        let press_k = |req_win: &str| {
            host.execute(req_win).unwrap();
            CORE.with(|c| {
                c.borrow_mut().emit(
                    "input.key",
                    json!({"key":"9","down":true,"repeat":false,"synthetic":false,"text":""}),
                )
            });
            native_events::dispatch(&host).unwrap();
        };

        let open_merch = NativeSnapshot {
            in_world: true,
            mode: 10,
            merchant_open: true,
            ..Default::default()
        };
        let closed_merch = NativeSnapshot {
            in_world: true,
            mode: 0,
            merchant_open: false,
            ..Default::default()
        };

        // 1. Stop on dispatch refusal at step 2 -> step 3 NEVER runs.
        press_k("req_win = nil");
        let mut dispatched = 0usize;
        BROKER.with(|b| {
            let mut b = b.borrow_mut();
            let mut step_fn = |_: &ActionKind| {
                dispatched += 1;
                if dispatched == 1 {
                    DispatchOutcome::Completed { detail: None }
                } else {
                    DispatchOutcome::Refused {
                        code: "not_ready",
                        reason: "inventory is full".into(),
                    }
                }
            };
            b.step_tick(open_merch, &mut step_fn);
            b.step_tick(open_merch, &mut step_fn);
            b.step_tick(open_merch, &mut step_fn);
        });
        assert_eq!(dispatched, 2, "step 3 must never run after refusal at step 2");
        native_events::dispatch(&host).unwrap();
        host.execute(
            "assert(bres[#bres].state == 'refused' and bres[#bres].ok == false and bres[#bres].completed == 1 and bres[#bres].total == 3 and bres[#bres].failed_step == 2 and bres[#bres].reason == 'inventory is full')",
        )
        .unwrap();

        // 2. Stop on window close (merchant closed after step 1) + never replay after reopening!
        press_k("req_win = 'merchant'");
        dispatched = 0;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(open_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
            b.borrow_mut().step_tick(closed_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
            b.borrow_mut().step_tick(open_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 1, "reopening merchant must not replay remaining steps");
        native_events::dispatch(&host).unwrap();
        host.execute(
            "assert(bres[#bres].state == 'refused' and bres[#bres].completed == 1 and bres[#bres].reason == 'merchant closed')",
        )
        .unwrap();

        // 3. Explicit stop_batch stops immediately and cancels remaining steps.
        press_k("req_win = 'merchant'");
        dispatched = 0;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(open_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        host.execute("local ok, e = eqoa.actions.stop_batch(last_b); assert(ok == true and e == nil)").unwrap();
        BROKER.with(|b| {
            b.borrow_mut().step_tick(open_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 1);
        native_events::dispatch(&host).unwrap();
        host.execute(
            "assert(bres[#bres].state == 'stopped' and bres[#bres].ok == false and bres[#bres].completed == 1 and bres[#bres].reason == 'stopped')",
        )
        .unwrap();

        // 4. Zoning (reset_session) stops immediately and never replays afterward.
        press_k("req_win = nil");
        dispatched = 0;
        BROKER.with(|b| {
            b.borrow_mut().step_tick(open_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        reset_session("zone");
        BROKER.with(|b| {
            b.borrow_mut().step_tick(open_merch, |_| {
                dispatched += 1;
                DispatchOutcome::Completed { detail: None }
            });
        });
        assert_eq!(dispatched, 1, "zoning must never replay remaining steps");
        native_events::dispatch(&host).unwrap();
        host.execute(
            "assert(bres[#bres].state == 'cancelled' and bres[#bres].ok == false and bres[#bres].completed == 1 and bres[#bres].reason == 'zone')",
        )
        .unwrap();

        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
    }
}
