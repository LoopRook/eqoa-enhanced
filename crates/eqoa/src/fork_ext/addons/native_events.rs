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
    next: u64,
    subscriptions: Vec<Subscription>,
    cursor: u64,
    pub once_mode: bool,
    pub initialized: bool,
    last_render: Option<Instant>,
    last_settings: Option<Instant>,
    pub received: u64,
    pub dropped: u64,
    combat_total: u64,
    stream_counts: std::collections::BTreeMap<String, u64>,
    resources: Vec<(Event, mlua::Table)>,
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
    let (cursor, combat_total) = CORE.with(|c| {
        let c = c.borrow();
        (c.seq, c.combat_total)
    });
    lua.set_app_data(Rc::new(RefCell::new(Bus {
        vm,
        next: 0,
        subscriptions: Vec::new(),
        cursor,
        once_mode,
        initialized: false,
        last_render: None,
        last_settings: None,
        received: 0,
        dropped: 0,
        combat_total,
        stream_counts: CORE.with(|c| c.borrow().stream_counts.clone()),
        resources: Vec::new(),
        boundary: 0,
    })));
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
    let lua = &host.lua;
    let shared = bus(lua);
    let cursor = shared.borrow().cursor;
    shared.borrow_mut().boundary += 1;
    let (mut events, last, total, counts) = CORE.with(|c| {
        let c = c.borrow();
        (
            c.events
                .iter()
                .filter(|e| e.seq > cursor)
                .cloned()
                .collect::<Vec<_>>(),
            c.seq,
            c.combat_total,
            c.stream_counts.clone(),
        )
    });
    if events.len() > 256 {
        events.drain(..events.len() - 256);
    }
    let delivered = events.iter().fold(
        std::collections::BTreeMap::<String, u64>::new(),
        |mut counts, e| {
            *counts.entry(e.name.clone()).or_default() += 1;
            counts
        },
    );
    let mut lost = 0;
    let mut streams = Vec::new();
    for (stream, total) in &counts {
        let previous = shared
            .borrow()
            .stream_counts
            .get(stream)
            .copied()
            .unwrap_or(0);
        let count = total
            .saturating_sub(previous)
            .saturating_sub(delivered.get(stream).copied().unwrap_or(0));
        if count > 0 {
            lost += count;
            streams.push(stream.clone());
        }
    }
    shared.borrow_mut().stream_counts = counts;
    if lost > 0 {
        deliver(
            lua,
            &local(
                "events.dropped",
                json!({"first_seq":cursor+1,"last_seq":events.first().map_or(last,|e|e.seq.saturating_sub(1)),"count":lost,"streams":streams}),
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
    {
        let mut b = shared.borrow_mut();
        b.cursor = last;
        let hits = events.iter().filter(|e| e.name == "combat.hit").count() as u64;
        b.received += hits;
        b.dropped += total.saturating_sub(b.combat_total).saturating_sub(hits);
        b.combat_total = total;
    }
    let mut merged = events.into_iter().map(|e| (e, None)).collect::<Vec<_>>();
    merged.extend(
        std::mem::take(&mut shared.borrow_mut().resources)
            .into_iter()
            .map(|(e, data)| (e, Some(data))),
    );
    merged.sort_by_key(|(e, _)| e.seq);
    for (e, data) in merged {
        deliver_data(lua, &e, None, data)?;
    }
    let due = shared
        .borrow()
        .subscriptions
        .iter()
        .filter(|s| {
            s.deadline.is_some_and(|d| Instant::now() >= d)
                && s.not_before <= shared.borrow().boundary
        })
        .map(|s| s.token.clone())
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
    b.resources.clear();
    b.initialized = false;
    b.last_render = None;
    b.cursor = CORE.with(|c| c.borrow().seq);
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
    let b = bus(lua);
    if b.borrow().resources.len() >= 256 {
        super::native::diagnose(
            lua,
            "eqoa.events.resource",
            "quota",
            "resource completion queue exceeded 256 events",
        );
        return Ok(());
    }
    let data = lua.create_table()?;
    data.set("kind", kind)?;
    data.set("handle", handle)?;
    if let Some(message) = failure {
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
    b.borrow_mut()
        .resources
        .push((local(name, json!({})), data));
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
