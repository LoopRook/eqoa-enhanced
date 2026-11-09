//! Per-VM subscriptions, source cursors and timers; dispatch only at Lua boundaries.
use super::{
    host::Host,
    native::{bind, error, record},
    native_core::{CORE, Event},
};
use mlua::{AnyUserData, Function, MetaMethod, Result, UserData, UserDataMethods, Value};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

thread_local! {static NEXT_VM:Cell<u64>=const {Cell::new(1)};}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    pub vm: u64,
    pub id: u64,
    pub kind: &'static str,
}
impl UserData for Token {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other.borrow::<Token>().is_ok_and(|o| *o == *this))
        });
    }
}
struct Subscription {
    token: Token,
    name: String,
    callback: Function,
    once: bool,
    deadline: Option<Instant>,
    not_before: u64,
}
pub(crate) struct Bus {
    pub vm: u64,
    diagnostics: super::native::DiagnosticSink,
    next: u64,
    subscriptions: Vec<Subscription>,
    active: bool,
    pub once_mode: bool,
    pub initialized: bool,
    last_render: Option<Instant>,
    last_settings: Option<Instant>,
    pub received: u64,
    pub dropped: u64,
    delivered: usize,
    pending: std::collections::VecDeque<(Event, Option<mlua::Table>)>,
    pending_bytes: usize,
    retired: Vec<mlua::Table>,
    lost: std::collections::BTreeMap<String, u64>,
    first_lost: Option<u64>,
    last_lost: u64,
    boundary: u64,
}
type Shared = Rc<RefCell<Bus>>;
pub(crate) fn bus(lua: &mlua::Lua) -> Shared {
    lua.app_data_ref::<Shared>()
        .expect("event bus installed")
        .clone()
}
pub(crate) fn token(lua: &mlua::Lua, kind: &'static str) -> Token {
    let shared = bus(lua);
    let mut b = shared.borrow_mut();
    b.next += 1;
    Token {
        vm: b.vm,
        id: b.next,
        kind,
    }
}
pub(crate) fn valid_token(lua: &mlua::Lua, v: &Value, kind: &str) -> Option<Token> {
    let r = v.as_userdata()?.borrow::<Token>().ok()?;
    (r.vm == bus(lua).borrow().vm && r.kind == kind).then(|| r.clone())
}

const EVENTS: &[&str] = &[
    "enabled",
    "disabling",
    "render",
    "settings.render",
    "timer",
    "world.leaving",
    "world.entered",
    "state.reset",
    "player.changed",
    "target.changed",
    "entity.added",
    "entity.changed",
    "entity.removed",
    "group.changed",
    "abilities.changed",
    "hotbar.changed",
    "cooldown.started",
    "cooldown.updated",
    "cooldown.ended",
    "effects.changed",
    "chat.message",
    "chat.history_scroll",
    "combat.hit",
    "input.key",
    "input.pad",
    "events.dropped",
    "resource.ready",
    "resource.failed",
];
fn install_event_fields(lua: &mlua::Lua) {
    for (name, fields) in [
        ("render", "dt"),
        ("settings.render", "dt"),
        ("timer", "token"),
        ("world.leaving", "reason world_id"),
        ("world.entered", "world_id initial"),
        ("state.reset", "in_world initial"),
        ("player.changed", "before after changed"),
        ("target.changed", "before after"),
        ("entity.added", "ref before after changed"),
        ("entity.changed", "ref before after changed"),
        ("entity.removed", "ref before after changed"),
        ("group.changed", "before after changed"),
        ("abilities.changed", "before after changed"),
        ("hotbar.changed", "before after changed"),
        ("effects.changed", "before after changed"),
        ("cooldown.started", "ability ability_id display_key lockout"),
        ("cooldown.updated", "ability ability_id display_key lockout"),
        ("cooldown.ended", "ability ability_id display_key lockout"),
        (
            "chat.message",
            "seq time text color channel channel_code sender outgoing",
        ),
        ("chat.history_scroll", "active steps"),
        (
            "combat.hit",
            "attacker_id defender_id amount kind color outgoing incoming from_pet to_pet",
        ),
        ("input.key", "key down repeat synthetic"),
        ("input.pad", "before after"),
        ("resource.ready", "kind handle error"),
        ("resource.failed", "kind handle error"),
        ("events.dropped", "first_seq last_seq count streams"),
    ] {
        for field in fields.split_whitespace() {
            super::native::describe(
                lua,
                &format!("eqoa.events.{name}.data.{field}"),
                "supported",
                Some(2),
                "",
            );
        }
    }
    for (round, names) in [
        (
            3,
            "inventory.changed bank.changed loot.changed merchant.changed",
        ),
        (
            4,
            "quests.changed camera.changed windows.changed map.changed mastery.changed action.result",
        ),
    ] {
        for name in names.split_whitespace() {
            super::native::describe(
                lua,
                &format!("eqoa.events.{name}"),
                "not_yet",
                Some(round),
                "scheduled for a later round",
            );
        }
    }
}
pub(crate) fn install(host: &Host, once_mode: bool) -> Result<()> {
    let lua = &host.lua;
    let vm = NEXT_VM.with(|id| {
        let next = id.get();
        id.set(next + 1);
        next
    });
    let shared = Rc::new(RefCell::new(Bus {
        vm,
        diagnostics: super::native::diagnostic_sink(lua),
        next: 0,
        subscriptions: Vec::new(),
        active: true,
        once_mode,
        initialized: false,
        last_render: None,
        last_settings: None,
        received: 0,
        dropped: 0,
        delivered: 0,
        pending: Default::default(),
        pending_bytes: 0,
        retired: Vec::new(),
        lost: Default::default(),
        first_lost: None,
        last_lost: 0,
        boundary: 0,
    }));
    REGISTRY.with(|registry| {
        let mut r = registry.borrow_mut();
        r.retain(|w| w.strong_count() > 0);
        r.push(Rc::downgrade(&shared));
    });
    lua.set_app_data(shared);
    for name in EVENTS {
        super::native::describe(
            lua,
            &format!("eqoa.events.{name}"),
            "supported",
            Some(2),
            "",
        );
    }
    for (name, once) in [("on", false), ("once", true)] {
        install_event_fields(lua);
        bind(
            lua,
            "events",
            name,
            lua.create_function(move |lua, (name, callback): (String, Function)| {
                let symbol = format!("eqoa.events.{name}");
                if !EVENTS.contains(&name.as_str()) {
                    return error(
                        lua,
                        &symbol,
                        super::native::status(lua, &symbol).unwrap_or("unknown_symbol"),
                        "event source is not implemented",
                    );
                }
                if name == "render" && !bus(lua).borrow().once_mode {
                    return error(
                        lua,
                        "eqoa.events.on",
                        "invalid_argument",
                        "frame-mode entry is already the render callback; use entryMode once",
                    );
                }
                if bus(lua).borrow().subscriptions.len() >= 128 {
                    return error(lua, "eqoa.events.on", "quota", "128 subscription limit");
                }
                let t = token(lua, "event");
                bus(lua).borrow_mut().subscriptions.push(Subscription {
                    token: t.clone(),
                    name,
                    callback,
                    once,
                    deadline: None,
                    not_before: 0,
                });
                lua.pack_multi((lua.create_userdata(t)?, Value::Nil))
            })?,
        )?;
    }
    bind(
        lua,
        "events",
        "off",
        lua.create_function(|lua, value: Value| {
            let Some(t) = valid_token(lua, &value, "event") else {
                return error(
                    lua,
                    "eqoa.events.off",
                    "stale_handle",
                    "token belongs to another VM or service",
                );
            };
            let shared = bus(lua);
            let mut b = shared.borrow_mut();
            let old = b.subscriptions.len();
            b.subscriptions.retain(|s| s.token != t);
            lua.pack_multi((old != b.subscriptions.len(), Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "events",
        "after",
        lua.create_function(|lua, (seconds, callback): (Value, Function)| {
            let Some(seconds) = super::native_state::number(&seconds).filter(|s| *s >= 0.0) else {
                return error(
                    lua,
                    "eqoa.events.after",
                    "invalid_argument",
                    "delay must be finite and nonnegative",
                );
            };
            let Some(deadline) = Instant::now().checked_add(
                std::time::Duration::try_from_secs_f64(seconds).map_err(mlua::Error::external)?,
            ) else {
                return error(
                    lua,
                    "eqoa.events.after",
                    "out_of_range",
                    "timer deadline exceeds host clock",
                );
            };
            if bus(lua).borrow().subscriptions.len() >= 128 {
                return error(lua, "eqoa.events.after", "quota", "128 subscription limit");
            }
            let t = token(lua, "event");
            let not_before = bus(lua).borrow().boundary + 1;
            bus(lua).borrow_mut().subscriptions.push(Subscription {
                token: t.clone(),
                name: "timer".into(),
                callback,
                once: true,
                deadline: Some(deadline),
                not_before,
            });
            lua.pack_multi((lua.create_userdata(t)?, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "combat",
        "get_counters",
        lua.create_function(|lua, ()| {
            let b = bus(lua);
            let b = b.borrow();
            record(
                lua,
                "eqoa.combat.Counters",
                &json!({"received":b.received,"dropped":b.dropped}),
            )
        })?,
    )?;
    Ok(())
}
pub(crate) fn dispatch(host: &Host) -> Result<()> {
    super::native_core::flush_deferred();
    let lua = &host.lua;
    let shared = bus(lua);
    if !host.iteration_in_progress() {
        start_iteration(lua);
    }
    let (batch, gap) = {
        let mut b = shared.borrow_mut();
        b.boundary += 1;
        b.retired.clear();
        let gap = if b.lost.is_empty() {
            None
        } else {
            let lost = std::mem::take(&mut b.lost);
            Some((b.first_lost.take().unwrap_or(0), b.last_lost, lost))
        };
        let limit = 256usize.saturating_sub(
            b.delivered + usize::from(b.once_mode) + if gap.is_some() { 2 } else { 0 },
        );
        let n = b.pending.len().min(limit);
        let batch = b.pending.drain(..n).collect::<Vec<_>>();
        for (e, _) in &batch {
            b.pending_bytes = b.pending_bytes.saturating_sub(e.bytes);
            if e.name == "combat.hit" {
                b.received += 1;
            }
        }
        (batch, gap)
    };
    if let Some((first, last, streams)) = gap {
        let count = streams.values().sum::<u64>();
        let names = streams.keys().cloned().collect::<Vec<_>>();
        deliver(
            lua,
            &local(
                "events.dropped",
                json!({"first_seq":first,"last_seq":last,"count":count,"streams":names}),
            ),
            None,
        )?;
        let world = CORE.with(|c| c.borrow().view.in_world);
        deliver(
            lua,
            &local("state.reset", json!({"in_world":world,"initial":false})),
            None,
        )?;
    }
    for (event, data) in batch {
        deliver_data(lua, &event, None, data)?;
    }
    let timer_budget = {
        let b = shared.borrow();
        256usize.saturating_sub(b.delivered + usize::from(b.once_mode))
    };
    let due = shared
        .borrow()
        .subscriptions
        .iter()
        .filter(|s| {
            s.deadline.is_some_and(|d| Instant::now() >= d)
                && s.not_before <= shared.borrow().boundary
        })
        .map(|s| s.token.clone())
        .take(timer_budget)
        .collect::<Vec<_>>();
    for t in due {
        let mut e = local("timer", json!({}));
        let table = lua.create_table()?;
        table.set("token", lua.create_userdata(t.clone())?)?;
        e.data = json!({});
        deliver_timer(lua, &e, &t, table)?;
    }
    Ok(())
}
fn local(name: &str, data: serde_json::Value) -> Event {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.seq += 1;
        Event {
            name: name.into(),
            data,
            bytes: 0,
            seq: c.seq,
            time: super::native_core::time_unborrowed(&c),
            tick: c.tick,
            frame: c.frame,
            generation: c.generation,
        }
    })
}
fn envelope(lua: &mlua::Lua, e: &Event) -> Result<mlua::Table> {
    let table = lua.create_table()?;
    table.set("name", e.name.as_str())?;
    table.set("seq", e.seq)?;
    table.set("time", e.time)?;
    table.set("tick", e.tick)?;
    table.set("frame", e.frame)?;
    table.set("generation", e.generation)?;
    table.set(
        "data",
        record(lua, &format!("eqoa.events.{}.data", e.name), &e.data)?,
    )?;
    Ok(table)
}
fn deliver(lua: &mlua::Lua, e: &Event, only: Option<&Token>) -> Result<()> {
    deliver_data(lua, e, only, None)
}
fn deliver_data(
    lua: &mlua::Lua,
    e: &Event,
    only: Option<&Token>,
    data: Option<mlua::Table>,
) -> Result<()> {
    let shared = bus(lua);
    {
        let mut b = shared.borrow_mut();
        if b.delivered >= 256 {
            return Ok(());
        }
        b.delivered += 1;
    }
    let candidates = shared
        .borrow()
        .subscriptions
        .iter()
        .filter(|s| s.name == e.name && s.deadline.is_none() && only.is_none_or(|t| *t == s.token))
        .map(|s| s.token.clone())
        .collect::<Vec<_>>();
    for t in candidates {
        let call = {
            let mut b = shared.borrow_mut();
            let Some(i) = b.subscriptions.iter().position(|s| s.token == t) else {
                continue;
            };
            let s = &b.subscriptions[i];
            let callback = s.callback.clone();
            if s.once {
                b.subscriptions.remove(i);
            }
            callback
        };
        let event = envelope(lua, e)?;
        if let Some(data) = &data {
            event.set("data", data.clone())?;
        }
        let _phase = super::native::phase(lua, &e.name);
        let _drawing =
            super::bridge::drawing(matches!(e.name.as_str(), "render" | "settings.render"));
        if let Err(err) = call.call::<()>(readonly_envelope(lua, event)?) {
            if super::native::fatal_memory(&err) {
                return Err(err);
            }
            super::native::diagnose(
                lua,
                &format!("eqoa.events.{}", e.name),
                "failed",
                &err.to_string(),
            );
            shared.borrow_mut().subscriptions.retain(|s| s.token != t);
        }
    }
    Ok(())
}
fn deliver_timer(lua: &mlua::Lua, e: &Event, t: &Token, data: mlua::Table) -> Result<()> {
    let shared = bus(lua);
    let sub = {
        let mut b = shared.borrow_mut();
        b.subscriptions
            .iter()
            .position(|s| s.token == *t)
            .map(|i| b.subscriptions.remove(i))
    };
    if let Some(sub) = sub {
        {
            let mut b = shared.borrow_mut();
            if b.delivered >= 256 {
                return Ok(());
            }
            b.delivered += 1;
        }
        let event = envelope(lua, e)?;
        event.set("data", data)?;
        let _phase = super::native::phase(lua, "timer");
        let _drawing = super::bridge::drawing(false);
        if let Err(e) = sub.callback.call::<()>(readonly_envelope(lua, event)?) {
            if super::native::fatal_memory(&e) {
                return Err(e);
            }
            super::native::diagnose(lua, "eqoa.events.timer", "failed", &e.to_string());
        }
    }
    Ok(())
}
pub(crate) fn lifecycle(host: &Host, name: &str) -> Result<()> {
    deliver(&host.lua, &local(name, json!({})), None)
}
pub(crate) fn settings_render(host: &Host) -> Result<()> {
    let b = bus(&host.lua);
    let now = Instant::now();
    let dt = b
        .borrow_mut()
        .last_settings
        .replace(now)
        .map_or(0.0, |last| now.duration_since(last).as_secs_f64());
    deliver(&host.lua, &local("settings.render", json!({"dt":dt})), None)
}
pub(crate) fn initial(host: &Host) -> Result<()> {
    let (world, id) = CORE.with(|c| {
        let c = c.borrow();
        (c.view.in_world, c.view.world_id)
    });
    if world {
        deliver(
            &host.lua,
            &local("world.entered", json!({"world_id":id,"initial":true})),
            None,
        )?;
        deliver(
            &host.lua,
            &local("state.reset", json!({"in_world":true,"initial":true})),
            None,
        )?;
    }
    Ok(())
}
pub(crate) fn render(host: &Host) -> Result<()> {
    let b = bus(&host.lua);
    if !b.borrow().once_mode {
        return Ok(());
    }
    let now = Instant::now();
    let dt = b
        .borrow_mut()
        .last_render
        .replace(now)
        .map_or(0.0, |last| now.duration_since(last).as_secs_f64());
    deliver(&host.lua, &local("render", json!({"dt":dt})), None)
}
pub(crate) fn clear(host: &Host) {
    if host.lua.app_data_ref::<Shared>().is_none() {
        return;
    }
    super::input::native::release_vm(&host.lua);
    let vm = bus(&host.lua).borrow().vm;
    super::chat::cancel_vm(vm);
    super::weapon_sets::cancel_vm(vm);
    let b = bus(&host.lua);
    let mut b = b.borrow_mut();
    b.subscriptions.clear();
    b.pending.clear();
    b.pending_bytes = 0;
    b.retired.clear();
    b.lost.clear();
    b.first_lost = None;
    b.active = false;
    b.initialized = false;
    b.last_render = None;
    drop(b);
    if !has_consumers() {
        super::native_core::invalidate_capture();
    }
}
pub(crate) fn resource(
    lua: &mlua::Lua,
    kind: &str,
    handle: Value,
    failure: Option<&str>,
) -> Result<()> {
    if lua.app_data_ref::<Shared>().is_none() {
        return Ok(());
    }
    super::native_core::flush_deferred();
    let b = bus(lua);
    b.borrow_mut().retired.clear();
    let data = lua.create_table()?;
    data.set("kind", kind)?;
    data.set("handle", handle)?;
    if let Some(message) = failure {
        let message = message.chars().take(2048).collect::<String>();
        data.set(
            "error",
            record(
                lua,
                "eqoa.Error",
                &json!({"code":"failed","message":message,"capability":format!("eqoa.{kind}.load")}),
            )?,
        )?;
    }
    let name = if failure.is_some() {
        "resource.failed"
    } else {
        "resource.ready"
    };
    let mut event = local(name, json!({}));
    event.bytes = 256 + failure.map_or(0, |s| s.len().min(8192));
    enqueue(&mut b.borrow_mut(), event, Some(data));
    Ok(())
}

fn readonly_envelope(lua: &mlua::Lua, event: mlua::Table) -> Result<mlua::Table> {
    let name: String = event.get("name")?;
    let data: mlua::Table = event.get("data")?;
    if matches!(
        name.as_str(),
        "timer" | "resource.ready" | "resource.failed"
    ) {
        event.set(
            "data",
            super::native::guard(
                lua,
                data,
                &format!("eqoa.events.{name}.data"),
                Some(vec![
                    "token".into(),
                    "kind".into(),
                    "handle".into(),
                    "error".into(),
                ]),
            )?,
        )?;
    }
    super::native::guard(
        lua,
        event,
        "eqoa.events.Event",
        Some(
            ["name", "seq", "time", "tick", "frame", "generation", "data"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
    )
}

pub(crate) fn prime(host: &Host) {
    let shared = bus(&host.lua);
    let mut b = shared.borrow_mut();
    b.pending.clear();
    b.pending_bytes = 0;
    b.retired.clear();
    b.lost.clear();
    b.first_lost = None;
    b.active = true;
}

thread_local! {static REGISTRY:RefCell<Vec<std::rc::Weak<RefCell<Bus>>>>=const{RefCell::new(Vec::new())};}
// Called from the native source only. It does not evaluate Lua or release Lua values.
pub(crate) fn publish(event: &Event) {
    REGISTRY.with(|registry| {
        for weak in registry.borrow().iter() {
            if let Some(shared) = weak.upgrade() {
                let mut b = shared.borrow_mut();
                if b.active {
                    enqueue(&mut b, event.clone(), None);
                }
            }
        }
    });
}
fn enqueue(b: &mut Bus, event: Event, data: Option<mlua::Table>) {
    b.pending_bytes = b.pending_bytes.saturating_add(event.bytes);
    b.pending.push_back((event, data));
    while b.pending.len() > 1024 || b.pending_bytes > 2 * 1024 * 1024 {
        let Some((old, data)) = b.pending.pop_front() else {
            break;
        };
        b.pending_bytes = b.pending_bytes.saturating_sub(old.bytes);
        b.first_lost = Some(b.first_lost.map_or(old.seq, |first| first.min(old.seq)));
        b.last_lost = b.last_lost.max(old.seq);
        *b.lost.entry(old.name.clone()).or_default() += 1;
        if old.name == "combat.hit" {
            b.dropped += 1;
        }
        if let Some(data) = data {
            b.retired.push(data);
        }
    }
}

pub(crate) fn start_iteration(lua: &mlua::Lua) {
    if let Some(b) = lua.app_data_ref::<Shared>() {
        b.borrow_mut().delivered = 0;
    }
}

pub(crate) fn active(lua: &mlua::Lua) -> bool {
    lua.app_data_ref::<Shared>()
        .is_some_and(|b| b.borrow().active)
}

/// Native producers have no work when all instantiated VMs are inactive.
pub(crate) fn has_consumers() -> bool {
    REGISTRY.with(|r| {
        r.borrow()
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .any(|b| b.borrow().active)
    })
}

pub(crate) fn interested(name: &str) -> bool {
    REGISTRY.with(|r| {
        r.borrow()
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .any(|b| {
                let b = b.borrow();
                b.active && b.subscriptions.iter().any(|s| s.name == name)
            })
    })
}

// Source-capture failures remain visible through retained VMs even after disable.
// This records diagnostics only: no Lua executes, and the manager drains the sink.
pub(crate) fn capture_diagnostic(code: &str, reason: &str) {
    REGISTRY.with(|registry| {
        for shared in registry.borrow().iter().filter_map(std::rc::Weak::upgrade) {
            shared
                .borrow()
                .diagnostics
                .record("eqoa.events", code, reason);
        }
    });
}
