//! Per-VM subscriptions, source cursors and timers; dispatch only at Lua boundaries.
use super::{
    host::Host,
    native::{bind, error, record},
    native_core::{CORE, Event},
};
use mlua::{AnyUserData, Function, MetaMethod, Result, Table, UserData, UserDataMethods, Value};
use serde_json::{Value as Json, json};
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
    buttons: std::collections::BTreeMap<String,Function>,
    pub(super) active: bool,
    pub once_mode: bool,
    pub initialized: bool,
    last_render: Option<Instant>,
    last_settings: Option<Instant>,
    pub received: u64,
    pub dropped: u64,
    delivered: usize,
    pending: std::collections::VecDeque<(Event, Option<mlua::Table>)>,
    pending_bytes: usize,
    commands: std::collections::VecDeque<(Event,u64)>,
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
    "player.stats_changed",
    "target.changed",
    "entity.added",
    "entity.changed",
    "entity.removed",
    "group.changed",
    "map.changed",
    "inventory.changed",
    "bank.changed",
    "loot.changed",
    "merchant.changed",
    "shared.changed",
    "mod.command",
    "guide.lost",
    "pointer.pressed",
    "pointer.wheel",
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
    "action.result",
    "batch.progress",
    "batch.result",
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
        ("player.stats_changed", ""),
        ("target.changed", "before after"),
        ("entity.added", "ref before after changed"),
        ("entity.changed", "ref before after changed"),
        ("entity.removed", "ref before after changed"),
        ("group.changed", "before after changed"),
        ("map.changed", "before after changed"),
        ("inventory.changed", "before after changed cause"),
        ("bank.changed", "before after changed cause"),
        ("loot.changed", "before after changed cause"),
        ("merchant.changed", "before after changed cause"),
        ("shared.changed", "mod path"),
        ("mod.command", "from command args"),
        ("guide.lost", "owner world_id entity_id label reason"),
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
        ("input.key", "key down repeat synthetic text"),
        ("input.pad", "before after"),
        ("pointer.pressed", "x y inside_window button steps"),
        ("pointer.wheel", "x y inside_window button steps"),
        ("resource.ready", "kind handle error request value path"),
        ("resource.failed", "kind handle error request value path"),
        (
            "action.result",
            "request_id kind state phase ok code reason queued_tick dispatched_tick completed_tick detail",
        ),
        (
            "batch.progress",
            "batch_id state ok completed total current_step code reason queued_tick completed_tick failed_step detail",
        ),
        (
            "batch.result",
            "batch_id state ok completed total current_step code reason queued_tick completed_tick failed_step detail",
        ),
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
            4,
            "quests.changed camera.changed windows.changed mastery.changed",
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
        buttons: Default::default(),
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
        commands: Default::default(),
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
            Some(match *name {
                "player.stats_changed" => 3,
                "action.result" | "batch.progress" | "batch.result" => 4,
                _ => 2,
            }),
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
    let polls = super::addon_perf::section("dispatch.polls");
    super::async_json::poll(&host.lua)?;
    super::json_save::poll(&host.lua)?;
    super::player_files::poll(&host.lua)?;
    super::action_broker::poll(&host.lua)?;
    super::native_core::flush_deferred();
    drop(polls);
    let _drain = super::addon_perf::section("dispatch.drain");
    let lua = &host.lua;
    let shared = bus(lua);
    if !host.iteration_in_progress() {
        start_iteration(lua);
    }
    let (batch, gap, commands) = {
        let mut b = shared.borrow_mut();
        b.boundary += 1;
        b.retired.clear();
        let gap = if b.lost.is_empty() {
            None
        } else {
            let lost = std::mem::take(&mut b.lost);
            Some((b.first_lost.take().unwrap_or(0), b.last_lost, lost))
        };
        let command_limit = 64usize.min(256usize.saturating_sub(b.delivered + usize::from(b.once_mode) + if gap.is_some() {2} else {0}));
        let take = command_limit.min(b.commands.len());
        let boundary = b.boundary;
        let commands = b.commands.drain(..take).map(|(mut event,expiry)| {if expiry < boundary {event.expire_player();} event}).collect::<Vec<_>>();
        let limit = 256usize.saturating_sub(
            b.delivered + usize::from(b.once_mode) + commands.len() + if gap.is_some() { 2 } else { 0 },
        );
        let n = b.pending.len().min(limit);
        let batch = b.pending.drain(..n).collect::<Vec<_>>();
        for (e, _) in &batch {
            b.pending_bytes = b.pending_bytes.saturating_sub(e.bytes);
            if e.name == "combat.hit" {
                b.received += 1;
            }
        }
        (batch, gap, commands)
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
    for event in commands { deliver_data(lua, &event, None, None)?; }
    {let mut b=shared.borrow_mut();let boundary=b.boundary;for (event,expiry) in &mut b.commands {if *expiry <= boundary {event.expire_player();}}}
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
        e.data = std::sync::Arc::new(json!({}));
        deliver_timer(lua, &e, &t, table)?;
    }
    Ok(())
}
fn local(name: &str, data: serde_json::Value) -> Event {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.seq += 1;
        Event {
            player_command: false,
            player_action: None,
            name: name.into(),
            data: std::sync::Arc::new(data),
            bytes: 0,
            seq: c.seq,
            time: super::native_core::time_unborrowed(&c),
            tick: c.tick,
            frame: c.frame,
            generation: c.generation,
        }
    })
}
pub(super) fn envelope(lua: &mlua::Lua, e: &Event) -> Result<mlua::Table> {
    let path = format!("eqoa.events.{}.data", e.name);
    let timed = super::addon_perf::section("env.data");
    let data = match lazy_data(lua, e, &path)? {
        Some(data) => data,
        None => record(lua, &path, &e.data)?,
    };
    drop(timed);
    super::record_raw::envelope_table(lua, &e.name, [e.time, e.tick as f64, e.frame as f64, e.generation as f64], e.seq as f64, data)
}

/// A change event's data (`ref`, `before`, `after`, `changed`) as a read-only record whose fields
/// are built when a handler first reads them: the first read builds the field as `record` makes it,
/// keeps it in the data (so it is the same value every time), and a missing field is diagnosed as
/// before. A creature moving 25 times a frame is delivered with two full records each; the journal
/// reads one field of one. The `before` and `after` records are the same, a field at a time.
fn lazy_data(lua: &mlua::Lua, e: &Event, path: &str) -> Result<Option<Value>> {
    let Json::Object(fields) = &*e.data else {
        return Ok(None);
    };
    if !(fields.contains_key("before") && fields.contains_key("after")) {
        return Ok(None);
    }
    Ok(lazy_fields(lua, path, &e.data, None)?.map(Value::Table))
}

/// The object `at` in `root` (or `root` itself) as a read-only record whose fields are built from the
/// native value when first read. None when it is not a plain object (a `$ref`, an array: those are
/// built whole by `record`).
fn lazy_fields(lua: &mlua::Lua, path: &str, root: &std::sync::Arc<Json>, at: Option<&str>) -> Result<Option<Table>> {
    let object = match at {
        Some(key) => &root[key],
        None => &**root,
    };
    let Json::Object(fields) = object else {
        return Ok(None);
    };
    if fields.contains_key("$ref") {
        return Ok(None);
    }
    let sig = super::native::signature(path, true, fields.keys().map(String::as_str));
    let (missing, readonly) = super::native::guard_parts(lua, sig, path, || Some(fields.keys().cloned().collect()))?;
    let (root, path, at) = (root.clone(), path.to_owned(), at.map(str::to_owned));
    let lazy = lua.create_function(move |lua, (table, key): (Table, Value)| {
        if let Value::String(k) = &key {
            let k = k.to_str()?;
            let object = match &at {
                Some(at) => &root[at.as_str()],
                None => &*root,
            };
            if let Some(value) = object.get(&*k).filter(|v| !v.is_null()) {
                let nested = if at.is_none() && (&*k == "before" || &*k == "after") { lazy_fields(lua, &path, &root, Some(&k))? } else { None };
                let built = match nested {
                    Some(table) => Value::Table(table),
                    None => record(lua, super::native::nested_type(&path, &k).unwrap_or(&path), value)?,
                };
                table.raw_set(&*k, built.clone())?;
                return Ok(built);
            }
        }
        missing.get::<Function>("__index")?.call::<Value>((table, key))
    })?;
    super::record_raw::lazy_object(lua, lazy, &readonly).map(Some)
}

/// The event as a handler gets it: its envelope behind the read-only guard, the data as `lazy_data`
/// or `record` makes it. The names whose data the guard also covers (`readonly_envelope`) and an
/// event delivered with its data made already take the longer way.
pub(super) fn envelope_readonly(lua: &mlua::Lua, e: &Event) -> Result<mlua::Table> {
    const KEYS: [&str; 7] = ["name", "seq", "time", "tick", "frame", "generation", "data"];
    let path = format!("eqoa.events.{}.data", e.name);
    let timed = super::addon_perf::section("env.data");
    let data = match lazy_data(lua, e, &path)? {
        Some(data) => data,
        None => record(lua, &path, &e.data)?,
    };
    drop(timed);
    let _timed = super::addon_perf::section("env.readonly");
    let sig = super::native::signature("eqoa.events.Event", true, KEYS.into_iter());
    let guard = super::native::guard_parts(lua, sig, "eqoa.events.Event", || Some(KEYS.iter().map(|k| k.to_string()).collect()))?;
    super::record_raw::event_proxy(lua, &e.name, [e.time, e.tick as f64, e.frame as f64, e.generation as f64], e.seq as f64, data, guard)
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
    let _timed = super::addon_perf::section("events.deliver");
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
    if candidates.is_empty() && e.name != "option.activated" {
        // nothing here listens for it: it counted against the frame's delivery budget and that is all
        return Ok(());
    }
    let direct = e.name == "option.activated" || (e.name == "input.key" && e.data["down"] == true && e.data["synthetic"] != true && e.data["repeat"] != true)
        || (e.name == "pointer.pressed" && e.data["inside_window"] == true);
    let _player = super::exchange::player_scope(shared.borrow().vm, direct, e.player_command, e.player_action.clone());
    if e.name == "option.activated" {
        let callback = shared.borrow().buttons.get(e.data["id"].as_str().unwrap_or("")).cloned();
        if let Some(callback) = callback {
            if let Err(err) = callback.call::<()>(()) {
                if super::native::fatal_memory(&err) { return Err(err); }
                super::native::diagnose(lua,"eqoa.options.button","failed",&err.to_string());
            }
        }
        return Ok(());
    }
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
        let timed = super::addon_perf::section("deliver.envelope");
        let envelope_ready = if data.is_none()
            && !matches!(
                e.name.as_str(),
                "timer"
                    | "resource.ready"
                    | "resource.failed"
                    | "action.result"
                    | "batch.progress"
                    | "batch.result"
            )
        {
            envelope_readonly(lua, e)?
        } else {
            let event = envelope(lua, e)?;
            if let Some(data) = &data {
                event.set("data", data.clone())?;
            }
            let timed_ro = super::addon_perf::section("env.readonly");
            let ready = readonly_envelope(lua, event)?;
            drop(timed_ro);
            ready
        };
        drop(timed);
        let (gesture_vm, gesture_addon) = (
            shared.borrow().vm,
            super::native::session(lua).borrow().addon.clone(),
        );
        let _gesture = if matches!(e.name.as_str(), "render" | "settings.render") {
            Some(super::gesture::enter_render_scope(gesture_vm, &gesture_addon))
        } else if e.name == "pointer.pressed"
            && e.data["inside_window"] == true
            && e.data["synthetic"] != true
        {
            let x = e.data["x"].as_f64().unwrap_or(f64::NAN);
            let y = e.data["y"].as_f64().unwrap_or(f64::NAN);
            super::pointer_api::point_in_vm_claim(gesture_vm, x, y).then(|| {
                super::gesture::enter_gesture(
                    gesture_vm,
                    &gesture_addon,
                    super::gesture::GestureKind::PointerClaim,
                    true,
                )
            })
        } else {
            None
        };
        let _phase = super::native::phase(lua, &e.name);
        let _drawing =
            super::bridge::drawing(matches!(e.name.as_str(), "render" | "settings.render"));
        let _handler = super::addon_perf::section("deliver.handler");
        if let Err(err) = call.call::<()>(envelope_ready) {
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
    super::player_files::close(&host.lua);
    super::json_save::close(&host.lua);
    super::async_json::close(&host.lua);
    if host.lua.app_data_ref::<Shared>().is_none() {
        return;
    }
    super::input::native::release_vm(&host.lua);
    let vm = bus(&host.lua).borrow().vm;
    super::map_views::release_vm(vm);
    super::guides::release_vm(vm);
    super::pointer_api::release_vm(vm);
    super::chat::cancel_vm(vm);
    super::weapon_sets::cancel_vm(vm);
    super::action_broker::clear_vm(vm, "addon disabled or reloaded");
    let b = bus(&host.lua);
    let mut b = b.borrow_mut();
    b.subscriptions.clear();
    b.buttons.clear();
    b.pending.clear();
    b.commands.clear();
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

pub(super) fn readonly_envelope(lua: &mlua::Lua, event: mlua::Table) -> Result<mlua::Table> {
    let name: String = event.get("name")?;
    let data: mlua::Table = event.get("data")?;
    if name == "action.result" {
        event.set(
            "data",
            super::native::guard_lazy(
                lua,
                data,
                "eqoa.actions.ActionResult",
                super::action_broker::ACTION_RESULT_KEYS,
            )?,
        )?;
    } else if matches!(name.as_str(), "batch.progress" | "batch.result") {
        event.set(
            "data",
            super::native::guard_lazy(
                lua,
                data,
                "eqoa.actions.BatchResult",
                super::action_broker::BATCH_RESULT_KEYS,
            )?,
        )?;
    } else if matches!(
        name.as_str(),
        "timer" | "resource.ready" | "resource.failed"
    ) {
        event.set(
            "data",
            super::native::guard_lazy(
                lua,
                data,
                &format!("eqoa.events.{name}.data"),
                &["token", "kind", "handle", "request", "value", "path", "error"],
            )?,
        )?;
    }
    super::native::guard_lazy(lua, event, "eqoa.events.Event", &["name", "seq", "time", "tick", "frame", "generation", "data"])
}

pub(crate) fn prime(host: &Host) {
    let shared = bus(&host.lua);
    let mut b = shared.borrow_mut();
    b.pending.clear();
    b.commands.clear();
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
                if b.active
                    && (event.name != "guide.lost" || event.data["owner"].as_u64() == Some(b.vm))
                {
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

pub(crate) fn json_resource(
    lua: &mlua::Lua,
    request: u64,
    value: Option<mlua::Value>,
    failure: Option<&str>,
) -> Result<()> {
    let data = lua.create_table()?;
    data.set("kind", "json")?;
    data.set("request", request)?;
    if let Some(value) = value {
        data.set("value", value.clone())?;
        data.set("handle", value)?;
    }
    if let Some(message) = failure {
        data.set("error",record(lua,"eqoa.Error",&json!({"code":"failed","message":message.chars().take(2048).collect::<String>(),"capability":"eqoa.files.load_json"}))?)?;
    }
    let mut event = local(
        if failure.is_some() {
            "resource.failed"
        } else {
            "resource.ready"
        },
        json!({}),
    );
    event.bytes = 256; // Bounded table reference; its storage is charged to the opt-in JSON VM heap.
    enqueue(&mut bus(lua).borrow_mut(), event, Some(data));
    Ok(())
}

pub(crate) fn json_saved(lua: &mlua::Lua, request: u64, failure: Option<&str>) -> Result<()> {
    let data = lua.create_table()?;
    data.set("kind", "json_saved")?;
    data.set("request", request)?;
    if let Some(message) = failure {
        data.set("error",record(lua,"eqoa.Error",&json!({"code":"failed","message":message.chars().take(2048).collect::<String>(),"capability":"eqoa.files.save_json_async"}))?)?;
    }
    let mut event = local(
        if failure.is_some() {
            "resource.failed"
        } else {
            "resource.ready"
        },
        json!({}),
    );
    event.bytes = 256;
    enqueue(&mut bus(lua).borrow_mut(), event, Some(data));
    Ok(())
}

pub(super) fn weak_bus(lua: &mlua::Lua) -> Option<std::rc::Weak<RefCell<Bus>>> {
    lua.app_data_ref::<Shared>().map(|b| Rc::downgrade(&b))
}
pub(super) fn shared_changed(owner: &str, path: &str) {
    let mut event = local("shared.changed", json!({"mod":owner,"path":path}));
    event.bytes = owner.len() + path.len() + 64;
    publish(&event);
}

pub(super) fn command(target: &Rc<RefCell<Bus>>, data: serde_json::Value, player: impl FnOnce() -> Option<std::sync::Arc<std::sync::atomic::AtomicU8>>) -> std::result::Result<(), ()> {
    let pending = target.borrow();
    let sender = data["from"].as_str().unwrap_or("");
    if !pending.active || pending.commands.len()>=64
        || pending.commands.iter().filter(|(e,_)|e.data["from"].as_str()==Some(sender)).count()>=4 {return Err(())}
    let expiry=pending.boundary+1;drop(pending);
    let mut event=local("mod.command",data);event.player_action=player();event.player_command=event.player_action.is_some();
    event.bytes=event.data.to_string().len();
    // Commands never enter or evict the native/resource event queue.
    target.borrow_mut().commands.push_back((event,expiry));Ok(())
}

pub(super) fn file_action(lua: &mlua::Lua, data: serde_json::Value, failed: bool) -> Result<()> {
    let table=lua.create_table()?;
    for (key,value) in data.as_object().expect("file action record") {
        table.set(key.as_str(),record(lua,if key=="error" {"eqoa.Error"} else {"eqoa.files.Result"},value)?)?;
    }
    let mut event=local(if failed {"resource.failed"} else {"resource.ready"},json!({}));
    event.bytes=data.to_string().len();
    enqueue(&mut bus(lua).borrow_mut(),event,Some(table));
    Ok(())
}
pub(super) fn action_result(lua: &mlua::Lua, data: mlua::Table) -> Result<()> {
    let mut event = local("action.result", json!({}));
    event.bytes = 256;
    enqueue(&mut bus(lua).borrow_mut(), event, Some(data));
    Ok(())
}
pub(super) fn batch_event(lua: &mlua::Lua, name: &'static str, data: mlua::Table) -> Result<()> {
    let mut event = local(name, json!({}));
    event.bytes = 256;
    enqueue(&mut bus(lua).borrow_mut(), event, Some(data));
    Ok(())
}

pub(super) fn register_option(lua: &mlua::Lua,id: &str,callback: Function)->bool {
    let bus=bus(lua);let mut bus=bus.borrow_mut();
    if bus.buttons.len()>=32 && !bus.buttons.contains_key(id) {return false}
    bus.buttons.insert(id.to_string(),callback);true
}
pub(super) fn activate_option(bus: &Rc<RefCell<Bus>>,id: &str)->bool {
    let mut bus=bus.borrow_mut();
    if !bus.active || !bus.buttons.contains_key(id) || bus.pending.len()>=64 {return false}
    let mut event=local("option.activated",json!({"id":id}));event.player_action=Some(std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0)));event.bytes=id.len()+64;
    enqueue(&mut bus,event,None);true
}
