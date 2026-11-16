// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Action broker: gesture-gated queue, 30 Hz rate limiting, round-robin
//! fairness, per-subsystem locks, `action.result` lifecycle, and server reply
//! correlation.

use super::{
    gesture::{self, GestureToken},
    host::Host,
    native::{bind, describe, error, record, session},
    native_core::CORE,
};
use mlua::{AnyUserData, Lua, MetaMethod, Result, UserData, UserDataMethods, Value};
use serde_json::{Value as Json, json};
use std::{
    cell::RefCell,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AbilityLookup {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub index: Option<i64>,
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
        }
    }

    pub(crate) const fn subsystem(&self) -> Option<Subsystem> {
        match self {
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
            | Self::ArrangeAbility { .. } => None,
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

pub(crate) enum DispatchOutcome {
    Completed {
        detail: Option<Json>,
    },
    AwaitingServer {
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
    #[allow(dead_code)]
    gesture_serial: u64,
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
}

#[derive(Default)]
struct VmBroker {
    addon_id: String,
    queued: VecDeque<QueuedAction>,
    in_flight: HashMap<u64, InFlightAction>,
    retained: VecDeque<ActionResultRecord>,
    pending_events: VecDeque<ActionResultRecord>,
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
            tokens: TOKEN_BUCKET_BURST,
            last_refill_tick: now_tick,
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
        })
    }

    fn enqueue(
        &mut self,
        token: GestureToken,
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
        let vm = token.vm;
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
        let vm_state = self.ensure_vm(vm, &token.addon_id);
        let req = QueuedAction {
            id,
            vm,
            addon_id: token.addon_id,
            kind: kind.clone(),
            queued_tick: now_tick,
            gesture_serial: token.serial,
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

    fn status(&self, req: RequestId) -> std::result::Result<Option<ActionResultRecord>, (&'static str, String)> {
        let Some(vm_state) = self.vms.get(&req.vm) else {
            return Ok(None);
        };
        Ok(vm_state.retained.iter().rev().find(|r| r.id == req.id).cloned())
    }

    fn cancel(&mut self, req: RequestId) -> std::result::Result<bool, (&'static str, String)> {
        let now_tick = self.tick;
        let Some(vm_state) = self.vms.get_mut(&req.vm) else {
            return Ok(false);
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
            return Ok(true);
        }
        if let Some(inflight) = vm_state.in_flight.remove(&req.id) {
            let rec = ActionResultRecord {
                id: inflight.id,
                vm: inflight.vm,
                kind: inflight.kind,
                state: "cancelled",
                phase: "server",
                ok: Some(false),
                code: Some("cancelled".into()),
                reason: Some("cancelled while awaiting server reply".into()),
                queued_tick: inflight.queued_tick,
                dispatched_tick: Some(inflight.dispatched_tick),
                completed_tick: Some(now_tick),
                detail: inflight.detail,
            };
            vm_state.upsert_retained(rec.clone());
            vm_state.pending_events.push_back(rec);
            return Ok(true);
        }
        Ok(false)
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
        for id in inflight_ids {
            if let Some(inflight) = vm_state.in_flight.remove(&id) {
                vm_state.upsert_retained(ActionResultRecord {
                    id: inflight.id,
                    vm: inflight.vm,
                    kind: inflight.kind,
                    state: "cancelled",
                    phase: "server",
                    ok: Some(false),
                    code: Some("cancelled".into()),
                    reason: Some(reason.into()),
                    queued_tick: inflight.queued_tick,
                    dispatched_tick: Some(inflight.dispatched_tick),
                    completed_tick: Some(now_tick),
                    detail: inflight.detail,
                });
            }
        }
        vm_state.pending_events.clear();
    }

    fn reset_session(&mut self, reason: &str) {
        let now_tick = self.tick;
        self.native = NativeSnapshot::default();
        for vm_state in self.vms.values_mut() {
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

        // 1. Server reply timeouts (10 s = 300 ticks -> `unknown`).
        for vm_state in self.vms.values_mut() {
            let expired: Vec<u64> = vm_state
                .in_flight
                .values()
                .filter(|f| now_tick.saturating_sub(f.dispatched_tick) >= SERVER_TIMEOUT_TICKS)
                .map(|f| f.id)
                .collect();
            for id in expired {
                if let Some(inflight) = vm_state.in_flight.remove(&id) {
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
                }
            }
        }

        // 2. Refill token buckets & expire requests queued > 1 s (30 ticks) waiting on rate limit.
        for vm_state in self.vms.values_mut() {
            vm_state.refill(now_tick);
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
                let next_req = {
                    let Some(vm_state) = self.vms.get(&vm_id) else {
                        continue;
                    };
                    if vm_state.tokens == 0 || vm_state.queued.is_empty() {
                        continue;
                    }
                    vm_state.queued.front().cloned()
                };
                let Some(req) = next_req else {
                    continue;
                };
                // Check live subsystem lock before popping/dispatching.
                if let Some(sub) = req.kind.subsystem() {
                    let sub_inflight = self.vms.values().any(|v| {
                        v.in_flight.values().any(|f| f.subsystem == Some(sub))
                    });
                    if self.native.subsystem_busy(sub) || sub_inflight {
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
                        continue;
                    }
                }

                let popped = self
                    .vms
                    .get_mut(&vm_id)
                    .and_then(|v| v.queued.pop_front())
                    .expect("front queued");

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
                            ok: Some(true),
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
            self.rr_cursor = (self.rr_cursor + 1) % vm_count;
            if !progressed {
                break;
            }
        }
    }

    fn resolve_matching(
        &mut self,
        matches: impl Fn(&InFlightAction) -> Option<(bool, Option<&'static str>, Option<String>)>,
    ) {
        let now_tick = self.tick;
        for vm_state in self.vms.values_mut() {
            let mut matched: Option<(u64, bool, Option<&'static str>, Option<String>)> = None;
            for inflight in vm_state.in_flight.values() {
                if let Some((ok, code, reason)) = matches(inflight) {
                    matched = Some((inflight.id, ok, code, reason));
                    break;
                }
            }
            if let Some((id, ok, code, reason)) = matched {
                let inflight = vm_state.in_flight.remove(&id).expect("matched inflight");
                let rec = ActionResultRecord {
                    id: inflight.id,
                    vm: inflight.vm,
                    kind: inflight.kind,
                    state: if ok { "completed" } else { "refused" },
                    phase: "server",
                    ok: Some(ok),
                    code: code.map(str::to_string),
                    reason,
                    queued_tick: inflight.queued_tick,
                    dispatched_tick: Some(inflight.dispatched_tick),
                    completed_tick: Some(now_tick),
                    detail: inflight.detail,
                };
                vm_state.upsert_retained(rec.clone());
                vm_state.pending_events.push_back(rec);
            }
        }
    }
}

thread_local! {
    static BROKER: RefCell<BrokerState> = RefCell::new(BrokerState::default());
}

pub(crate) fn update_native_snapshot(snapshot: NativeSnapshot) {
    BROKER.with(|b| b.borrow_mut().native = snapshot);
}

pub(crate) fn step_tick(
    snapshot: NativeSnapshot,
    dispatch: impl FnMut(&ActionKind) -> DispatchOutcome,
) {
    BROKER.with(|b| b.borrow_mut().step_tick(snapshot, dispatch));
}

pub(crate) fn has_in_flight() -> bool {
    BROKER.with(|b| b.borrow().has_in_flight())
}

pub(crate) fn clear_vm(vm: u64, reason: &str) {
    BROKER.with(|b| b.borrow_mut().clear_vm(vm, reason));
}

pub(crate) fn reset_session(reason: &str) {
    BROKER.with(|b| b.borrow_mut().reset_session(reason));
}

pub(crate) fn resolve_server_assist(target_id: u32) {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            if !matches!(inflight.matcher, ServerMatcher::Assist) {
                return None;
            }
            if target_id == u32::MAX {
                Some((false, Some("refused"), Some("there is no target to assist".into())))
            } else {
                Some((true, None, None))
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
                Some((true, None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn resolve_server_ability_clear() {
    BROKER.with(|b| {
        b.borrow_mut().resolve_matching(|inflight| {
            matches!(inflight.matcher, ServerMatcher::AbilityClearBar).then_some((true, None, None))
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
                Some((true, None, None))
            } else {
                None
            }
        });
    });
}

pub(crate) fn poll(lua: &Lua) -> Result<()> {
    let vm = super::native_events::bus(lua).borrow().vm;
    let events = BROKER.with(|b| {
        let mut b = b.borrow_mut();
        b.vms
            .get_mut(&vm)
            .map(|v| v.pending_events.drain(..).collect::<Vec<_>>())
            .unwrap_or_default()
    });
    for rec in events {
        let tbl = action_result_raw_table(lua, &rec)?;
        super::native_events::action_result(lua, tbl)?;
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
    let vm = super::native_events::bus(lua).borrow().vm;
    let addon = session(lua).borrow().addon.clone();
    let token = match gesture::take_single_token(vm, &addon) {
        Ok(tok) => tok,
        Err((code, msg)) => return error(lua, symbol, code, &msg),
    };
    let kind = match build() {
        Ok(kind) => kind,
        Err((code, msg)) => return error(lua, symbol, code, &msg),
    };
    match BROKER.with(|b| b.borrow_mut().enqueue(token, kind)) {
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
    if text.is_empty() || text.len() > 255 {
        return Err(("invalid_argument", "message length must be 1..=255 bytes".into()));
    }
    if text.contains('\0') || text.contains('\r') || text.contains('\n') {
        return Err(("invalid_argument", "message cannot contain control newlines or NUL".into()));
    }
    let normalized = if command_mode && !text.starts_with('/') && !text.starts_with('#') {
        format!("/{text}")
    } else {
        text
    };
    if gesture::is_forbidden_report_text(&normalized) {
        return Err((
            "permission_denied",
            "synthetic report routes (/bug, #bug, /feature, #feature) are forbidden".into(),
        ));
    }
    Ok(normalized)
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

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    describe(
        lua,
        "eqoa.actions.RequestId",
        "supported",
        Some(4),
        "opaque action request handle",
    );
    describe(
        lua,
        "eqoa.actions.ActionResult",
        "supported",
        Some(4),
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
            Some(4),
            "ActionResult field",
        );
    }

    describe(
        lua,
        "eqoa.actions.destroy",
        "unsupported",
        None,
        "not offered before the PR (owner)",
    );
    for name in ["move", "look", "buttons"] {
        describe(
            lua,
            &format!("eqoa.actions.{name}"),
            "not_yet",
            Some(4),
            "continuous controls come after the PR",
        );
    }
    for name in [
        "drop_quest",
        "mastery_share",
        "mastery_browse",
        "mastery_buy",
        "mastery_refund",
        "close_window",
        "select_row",
        "activate_row",
        "dialog_answer",
        "input_text",
        "menu_command",
    ] {
        describe(
            lua,
            &format!("eqoa.actions.{name}"),
            "not_yet",
            Some(4),
            "arrives with the window/quest read API",
        );
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
                    "request_id must be a RequestId handle",
                );
            };
            let Ok(req) = ud.borrow::<RequestId>() else {
                return error(
                    lua,
                    "eqoa.actions.status",
                    "invalid_argument",
                    "request_id must be a RequestId handle",
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
                    "request_id must be a RequestId handle",
                );
            };
            let Ok(req) = ud.borrow::<RequestId>() else {
                return error(
                    lua,
                    "eqoa.actions.cancel",
                    "invalid_argument",
                    "request_id must be a RequestId handle",
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
                Ok(true) => lua.pack_multi((true, Value::Nil)),
                Ok(false) => error(
                    lua,
                    "eqoa.actions.cancel",
                    "not_found",
                    "request is no longer pending",
                ),
                Err((code, msg)) => error(lua, "eqoa.actions.cancel", code, &msg),
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

    describe(
        lua,
        "eqoa.actions.camera",
        "not_yet",
        Some(4),
        "requires &mut Walker hook in Play::frame (proposed in round-6b-seams.md)",
    );

    install_non_item_actions(lua)?;

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
                    Some(k @ ("hostile" | "friendly" | "corpse")) => (false, Some(k.to_string())),
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
        lua.create_function(|lua, text: Value| {
            enqueue_lua_with(lua, "eqoa.actions.chat", || {
                Ok(ActionKind::Chat {
                    text: validate_chat_string(text, false)?,
                })
            })
        })?,
    )?;

    bind(
        lua,
        "actions",
        "command",
        lua.create_function(|lua, cmd: Value| {
            enqueue_lua_with(lua, "eqoa.actions.command", || {
                Ok(ActionKind::Command {
                    text: validate_chat_string(cmd, true)?,
                })
            })
        })?,
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

#[cfg(test)]
pub(crate) fn reset_all_for_test() {
    BROKER.with(|b| *b.borrow_mut() = BrokerState::default());
}

#[cfg(test)]
pub(crate) fn queued_count_for_test() -> usize {
    BROKER.with(|b| b.borrow().vms.values().map(|v| v.queued.len()).sum())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::{
        gesture::{
            self, GestureKind, arm_imgui_click, enter_gesture, enter_render_scope,
            is_forbidden_report_text,
        },
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
        let token = gesture::take_single_token(vm, addon_id)?;
        BROKER.with(|b| b.borrow_mut().enqueue(token, kind))
    }

    #[test]
    fn round6b_broker_gesture_gate_enforces_three_sources_and_one_action_per_gesture() {
        reset_all_for_test();
        crate::fork_ext::bindpage::reset_in_memory();
        CORE.with(|c| {
            let mut core = Core::default();
            core.view.in_world = true;
            *c.borrow_mut() = core;
        });

        let host = Host::new().unwrap();
        let vm = crate::fork_ext::addons::native_events::bus(&host.lua)
            .borrow()
            .vm;

        // Outside gesture -> needs_player_action + diagnostic
        host.execute(
            r#"
            events_seen = {}
            eqoa.events.on("action.result", function(e)
                events_seen[#events_seen + 1] = e.data
            end)
            local r, err = eqoa.actions.assist()
            assert(r == nil and err.code == "needs_player_action")
            eqoa.input.bind({ id = "go", label = "Go", default_key = "k" }, function()
                key_req, key_err = eqoa.actions.assist()
                second_req, second_err = eqoa.actions.assist()
            end)
            eqoa.events.on("pointer.pressed", function()
                ptr_req, ptr_err = eqoa.actions.assist()
            end)
        "#,
        )
        .unwrap();
        let logs = host.drain_logs().unwrap();
        assert!(
            logs.iter()
                .any(|line| line.contains("eqoa.actions.assist")
                    && line.contains("needs_player_action")),
            "missing gesture must record visible diagnostic: {logs:?}"
        );

        // Synthetic key down must NOT grant gesture
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                json!({"key":"k","down":true,"repeat":false,"synthetic":true,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
        host.execute("assert(key_req == nil)")
            .unwrap();

        // Physical ModKey gesture grants 1 action; 2nd fails with needs_player_action
        CORE.with(|c| {
            c.borrow_mut().emit(
                "input.key",
                json!({"key":"k","down":true,"repeat":false,"synthetic":false,"text":""}),
            )
        });
        native_events::dispatch(&host).unwrap();
        host.execute(
            "assert(key_req ~= nil and key_err == nil); assert(second_req == nil and second_err.code == 'needs_player_action')",
        )
        .unwrap();

        // Pointer claim gate: unclaimed click rejected; claimed click accepted
        CORE.with(|c| {
            c.borrow_mut().frame = 10;
            c.borrow_mut().emit(
                "pointer.pressed",
                json!({"x":50.0,"y":50.0,"inside_window":true,"button":"left","steps":0}),
            );
        });
        native_events::dispatch(&host).unwrap();
        host.execute("assert(ptr_req == nil and ptr_err.code == 'needs_player_action')")
            .unwrap();

        crate::fork_ext::addons::pointer_api::position([50.0, 50.0], true, (800, 600));
        host.execute("assert(eqoa.input.claim_pointer{x=40,y=40,width=30,height=30})")
            .unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "pointer.pressed",
                json!({"x":50.0,"y":50.0,"inside_window":true,"button":"left","steps":0}),
            );
        });
        native_events::dispatch(&host).unwrap();
        host.execute("assert(ptr_req ~= nil and ptr_err == nil)")
            .unwrap();

        // ImGui render scope: unarmed fails, armed after click succeeds once then refuses second
        {
            let _render = enter_render_scope(vm, "test");
            let err = enqueue_raw(vm, "test", ActionKind::Assist).unwrap_err();
            assert_eq!(err.0, "needs_player_action");
            arm_imgui_click();
            let first = enqueue_raw(vm, "test", ActionKind::Assist);
            assert!(first.is_ok());
            let second = enqueue_raw(vm, "test", ActionKind::Assist).unwrap_err();
            assert_eq!(second.0, "needs_player_action");
        }
        // Outside render scope again -> rejected
        let after = enqueue_raw(vm, "test", ActionKind::Assist).unwrap_err();
        assert_eq!(after.0, "needs_player_action");
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
        assert!(BROKER.with(|b| b.borrow_mut().cancel(ids[31])).unwrap());
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
}
