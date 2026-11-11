//! The versioned native surface. Capability presence is independent of game state.
use super::host::Host;
use mlua::{Function, Lua, MultiValue, Result, Table, Value};
use serde_json::{Value as Json, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Instant};

#[derive(Clone)]
struct Descriptor {
    status: &'static str,
    round: Option<u8>,
    reason: String,
}

#[derive(Default)]
struct Diagnostic {
    count: u64,
    last_emit: f64,
    first_time: f64,
    last_time: f64,
    emitted: u64,
    reason: String,
}

pub(crate) struct Session {
    descriptors: BTreeMap<String, Descriptor>,
    diagnostics: BTreeMap<(String, String), Diagnostic>,
    pending: Vec<String>,
    start: Instant,
    pub(crate) addon: String,
    vm: u64,
    phase: String,
    tokens: f64,
    token_time: f64,
    /// Shared missing-field metatables and the read-only guard, so building a
    /// record allocates only its own tables (no per-record closures).
    guard_missing: std::collections::HashMap<u64, Table>,
    guard_readonly: Option<Function>,
    /// Last record built per call site: records are read-only, so an unchanged
    /// native value is handed back as the same table instead of being rebuilt.
    record_cache: std::collections::HashMap<String, (Json, Table)>,
}

impl Session {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let mut s = Self {
            descriptors: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            pending: Vec::new(),
            start: Instant::now(),
            addon: String::new(),
            vm: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            phase: "lua".into(),
            tokens: 32.0,
            token_time: 0.0,
            guard_missing: std::collections::HashMap::new(),
            guard_readonly: None,
            record_cache: std::collections::HashMap::new(),
        };
        // Normative function index (§2). Registration does not imply implementation.
        for (module, names, round) in [
            ("abilities", "list get find in_range", 2),
            ("actions", "assist target_target toggle_stats_box", 3),
            (
                "actions",
                "status cancel target cycle_target assist interact attack cast use_hotbar use_item equip destroy swap_items quick_slot weapon_set prepare_weapon_set assign_ability unmemorize_ability unmemorize_all arrange_ability loot loot_all buy sell bank_move bank_money chat command move look autorun follow stop camera open_window close_window select_row activate_row dialog_answer input_text drop_quest mastery_share mastery_browse mastery_buy mastery_refund buttons menu_command",
                4,
            ),
            ("bank", "get", 3),
            ("binds", "get register", 3),
            ("camera", "get compass", 4),
            ("capabilities", "get list has", 2),
            ("chat", "history get_channel channel_name", 2),
            ("client", "get display", 2),
            ("combat", "get_counters", 2),
            ("cooldowns", "get", 2),
            ("effects", "list", 2),
            ("entities", "list get find named distance", 2),
            ("events", "on off once after", 2),
            ("files", "read write remove", 2),
            ("files", "list", 4),
            ("fonts", "load default release", 2),
            ("gems", "get is_gem format", 3),
            ("group", "get", 2),
            ("hotbar", "list", 2),
            ("hud", "hide show hidden_by", 2),
            ("hud", "frame layout visible text gauge image panel", 3),
            ("icons", "resource ui info", 2),
            ("icons", "class disposition", 4),
            ("input", "get claim release capture_text rotate_dpad", 2),
            ("input", "bind", 3),
            ("inventory", "get", 3),
            (
                "items",
                "get can_use stat slot_info attack_name stat_name",
                3,
            ),
            ("log", "write", 2),
            ("loot", "get", 3),
            (
                "map",
                "get images load_image set_waypoint clear_waypoint",
                4,
            ),
            ("mastery", "get skills selected", 4),
            (
                "math",
                "bearing degrees radians normalize signed_angle facing is_bit_zero",
                2,
            ),
            ("merchant", "get price", 3),
            ("options", "toggle get set", 3),
            ("player", "get experience_for_level", 2),
            ("player", "stats", 3),
            ("quests", "list get", 4),
            ("settings", "load save on_save on_load", 2),
            ("sound", "load play stop set_volume is_playing release", 2),
            ("target", "get", 2),
            ("text", "localized", 2),
            ("textures", "load rgba release", 2),
            ("windows", "open", 3),
            ("windows", "get find get_window format_hint parse_hint", 4),
        ] {
            s.describe(
                &format!("eqoa.{module}"),
                "not_yet",
                Some(round),
                "implementation pending",
            );
            for name in names.split_whitespace() {
                s.describe(
                    &format!("eqoa.{module}.{name}"),
                    "not_yet",
                    Some(round),
                    "implementation pending",
                );
            }
        }
        for symbol in [
            "eqoa.api",
            "eqoa.revision",
            "eqoa.capabilities",
            "eqoa.capabilities.get",
            "eqoa.capabilities.has",
            "eqoa.capabilities.list",
        ] {
            s.describe(symbol, "supported", Some(2), "");
        }
        for symbol in [
            "frontiers_forge.cave",
            "frontiers_forge.cave.Alloc",
            "ffi",
            "eqoa.textures.formats.jpeg",
        ] {
            let scheduled = symbol.ends_with("jpeg");
            s.describe(
                symbol,
                if scheduled { "not_yet" } else { "unsupported" },
                scheduled.then_some(4),
                if scheduled {
                    "JPEG arrives in round 4"
                } else {
                    "raw memory and FFI are excluded"
                },
            );
        }
        s
    }
    fn describe(&mut self, path: &str, status: &'static str, round: Option<u8>, reason: &str) {
        self.descriptors.insert(
            path.into(),
            Descriptor {
                status,
                round,
                reason: reason.into(),
            },
        );
    }
    fn capability(&self, path: &str) -> Json {
        let d = self.descriptors.get(path);
        json!({"path":path,"status":d.map_or("unknown", |d| d.status),
            "revision":u8::from(d.is_some_and(|d| matches!(d.status,"supported"|"partial"))),
            "round":d.and_then(|d| d.round), "reason":d.filter(|d| !d.reason.is_empty()).map(|d| &d.reason),
            "children": self.descriptors.keys().filter(|p| p.strip_prefix(&format!("{path}.")).is_some_and(|tail| !tail.contains('.'))).collect::<Vec<_>>()})
    }
    fn diagnostic(&mut self, symbol: &str, code: &str, reason: &str) {
        let time = self.start.elapsed().as_secs_f64();
        let key = (symbol.to_string(), code.to_string());
        self.tokens = (self.tokens + (time - self.token_time) * 16.0).min(32.0);
        self.token_time = time;
        let overflow = symbol == "eqoa.diagnostics.overflow";
        if !overflow
            && !self.diagnostics.contains_key(&key)
            && (self.tokens < 1.0 || self.diagnostics.len() >= 255)
        {
            self.diagnostic(
                "eqoa.diagnostics.overflow",
                "quota",
                "new diagnostic keys exceeded the per-VM rate/cache budget",
            );
            return;
        }
        if !overflow && !self.diagnostics.contains_key(&key) {
            self.tokens -= 1.0;
        }
        if !self.diagnostics.contains_key(&key) && self.diagnostics.len() >= 256 {
            if self.pending.len() < 64 {
                self.pending.push(
                    "eqoa/1 diagnostics overflow: 256 distinct keys; further keys suppressed"
                        .into(),
                );
            }
            return;
        }
        let d = self.diagnostics.entry(key).or_default();
        if d.count == 0 {
            d.first_time = time;
            d.reason = reason.chars().take(2048).collect();
        }
        d.last_time = time;
        d.count += 1;
        if (d.count == 1 || time - d.last_emit >= 60.0) && self.pending.len() < 64 {
            self.pending.push(json!({"api":"eqoa/1","addon":self.addon,"vm":self.vm,"symbol":symbol,"phase":self.phase,"code":code,"first_time":d.first_time,"last_time":time,"count":d.count,"message":d.reason}).to_string());
            d.last_emit = time;
            d.emitted = d.count;
        }
    }
}

pub(crate) type Shared = Rc<RefCell<Session>>;
pub(crate) fn session(lua: &Lua) -> Shared {
    lua.app_data_ref::<Shared>()
        .expect("native API installed")
        .clone()
}

pub(crate) fn diagnose(lua: &Lua, symbol: &str, code: &str, reason: &str) {
    if let Some(s) = lua.app_data_ref::<Shared>() {
        s.borrow_mut().diagnostic(symbol, code, reason);
    }
}
pub(crate) fn describe(
    lua: &Lua,
    path: &str,
    status: &'static str,
    round: Option<u8>,
    reason: &str,
) {
    session(lua)
        .borrow_mut()
        .describe(path, status, round, reason);
}
pub(crate) fn status(lua: &Lua, path: &str) -> Option<&'static str> {
    session(lua)
        .borrow()
        .descriptors
        .get(path)
        .map(|d| d.status)
}
pub(crate) fn annotate_host(host: &Host) -> Result<()> {
    let lua = &host.lua;
    let root: Table = lua.named_registry_value("eqoa_root")?;
    for name in ["WeaponSets", "ChatHistory"] {
        let value: Value = root.raw_get(name)?;
        if !value.is_nil() {
            describe(
                lua,
                &format!("eqoa.{name}"),
                "supported",
                Some(2),
                "permanent Enhanced compatibility alias",
            );
            if let Value::Table(table) = value {
                for pair in table.pairs::<String, Value>() {
                    let (key, _) = pair?;
                    describe(
                        lua,
                        &format!("eqoa.{name}.{key}"),
                        "supported",
                        Some(2),
                        "permanent Enhanced compatibility alias",
                    );
                }
            }
        }
    }
    for (name, value) in host.registered_modules() {
        if name.starts_with("frontiers_forge.") {
            describe(
                lua,
                &name,
                "partial",
                Some(2),
                "native Forge compatibility adapter",
            );
            if let Value::Table(t) = value {
                for pair in t.pairs::<String, Value>() {
                    let (key, _) = pair?;
                    describe(lua, &format!("{name}.{key}"), "supported", Some(2), "");
                }
            }
        }
    }
    for path in [
        "frontiers_forge.chat.CaptureChannelTypeFromGameWindow",
        "frontiers_forge.chat.GetChannelTableInfo",
        "frontiers_forge.chat.DebugDumpFocusChain",
        "frontiers_forge.combat.ReadForeignCave",
        "frontiers_forge.util.utf8_to_utf16",
        "frontiers_forge.util.utf16_to_utf8",
    ] {
        describe(
            lua,
            path,
            "unsupported",
            None,
            "the native client has no matching pointer/config payload",
        );
    }
    for path in [
        "frontiers_forge.player.GetCMs",
        "frontiers_forge.player.GetCMsSpent",
        "frontiers_forge.player.GetCMPct",
        "frontiers_forge.ui.GetClassIconTexture",
        "frontiers_forge.ui.GetDispositionIconTexture",
        "frontiers_forge.ui.DrawClassIcon",
        "frontiers_forge.ui.DrawDispositionIcon",
    ] {
        describe(lua, path, "not_yet", Some(4), "scheduled for round 4");
    }
    let forge: Table = lua.globals().get("UiForge")?;
    for pair in forge.pairs::<String, Value>() {
        let (name, _) = pair?;
        describe(lua, &format!("UiForge.{name}"), "supported", Some(2), "");
    }
    Ok(())
}

pub(crate) fn drain(lua: &Lua) -> Vec<String> {
    lua.app_data_ref::<Shared>()
        .map(|s| std::mem::take(&mut s.borrow_mut().pending))
        .unwrap_or_default()
}
pub(crate) fn finish(lua: &Lua) -> Vec<String> {
    let shared = session(lua);
    let mut s = shared.borrow_mut();
    let addon = s.addon.clone();
    let vm = s.vm;
    let mut final_messages = Vec::new();
    for ((symbol, code), d) in &mut s.diagnostics {
        if d.count > d.emitted {
            final_messages.push(json!({"api":"eqoa/1","addon":addon,"vm":vm,"symbol":symbol,"phase":"disable","code":code,"first_time":d.first_time,"last_time":d.last_time,"count":d.count,"message":d.reason}).to_string());
            d.emitted = d.count;
        }
    }
    final_messages.extend(std::mem::take(&mut s.pending));
    final_messages
}

pub(crate) fn error(lua: &Lua, symbol: &str, code: &str, message: &str) -> Result<MultiValue> {
    if !matches!(
        code,
        "not_in_world" | "unavailable" | "not_found" | "busy" | "stale_handle"
    ) {
        diagnose(lua, symbol, code, message);
    }
    let e = record(
        lua,
        "eqoa.Error",
        &json!({"code":code,"message":message,"capability":symbol}),
    )?;
    lua.pack_multi((Value::Nil, e))
}

pub(crate) fn record(lua: &Lua, path: &str, value: &Json) -> Result<Value> {
    Ok(match value {
        Json::Null => Value::Nil,
        Json::Bool(v) => Value::Boolean(*v),
        Json::Number(v) => Value::Number(
            v.as_f64()
                .ok_or_else(|| mlua::Error::runtime("invalid native number"))?,
        ),
        Json::String(v) => Value::String(lua.create_string(v)?),
        Json::Array(values) => {
            let table = lua.create_table()?;
            for (i, v) in values.iter().enumerate() {
                table.raw_set(i + 1, record(lua, path, v)?)?;
            }
            Value::Table(table)
        }
        Json::Object(fields) if fields.contains_key("$ref") => {
            super::native_state::make_ref(lua, value)?
        }
        Json::Object(fields) => {
            let backing = lua.create_table()?;
            for (k, v) in fields {
                backing.set(
                    k.as_str(),
                    record(lua, nested_type(path, k).unwrap_or(path), v)?,
                )?;
            }
            Value::Table(guard_keyed(
                lua,
                backing,
                path,
                signature(path, true, fields.keys().map(String::as_str)),
                || Some(fields.keys().cloned().collect()),
            )?)
        }
    })
}

fn signature<'a>(path: &str, has_keys: bool, keys: impl Iterator<Item = &'a str>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    has_keys.hash(&mut h);
    for k in keys {
        k.hash(&mut h);
    }
    h.finish()
}

/// `record`, reusing the previous result of `slot` while the native value is unchanged.
pub(crate) fn record_cached(lua: &Lua, slot: &str, path: &str, value: &Json) -> Result<Value> {
    if !matches!(value, Json::Object(_)) {
        return record(lua, path, value);
    }
    let session_rc = session(lua);
    if let Ok(s) = session_rc.try_borrow() {
        if let Some((json, table)) = s.record_cache.get(slot) {
            if json == value {
                return Ok(Value::Table(table.clone()));
            }
        }
    }
    let built = record(lua, path, value)?;
    if let (Value::Table(table), Ok(mut s)) = (&built, session_rc.try_borrow_mut()) {
        s.record_cache
            .insert(slot.to_string(), (value.clone(), table.clone()));
    }
    Ok(built)
}

pub(crate) fn guard(
    lua: &Lua,
    backing: Table,
    path: &str,
    known: Option<Vec<String>>,
) -> Result<Table> {
    let sig = signature(
        path,
        known.is_some(),
        known.iter().flatten().map(String::as_str),
    );
    guard_keyed(lua, backing, path, sig, || known)
}

/// `sig` identifies (path, known keys); records of one shape share one
/// missing-field metatable, built the first time the shape is seen.
fn guard_keyed(
    lua: &Lua,
    backing: Table,
    path: &str,
    sig: u64,
    known: impl FnOnce() -> Option<Vec<String>>,
) -> Result<Table> {
    // A bare Lua (no session) or a session in use higher up the stack just
    // builds uncached: the cache is an optimisation, never a requirement.
    let session_rc = lua.app_data_ref::<Shared>().map(|s| s.clone());
    let (cached_missing, cached_readonly) = session_rc
        .as_ref()
        .and_then(|rc| rc.try_borrow().ok())
        .map_or((None, None), |s| {
            (s.guard_missing.get(&sig).cloned(), s.guard_readonly.clone())
        });
    // Let Lua resolve present fields directly. Only a missing field crosses
    // into Rust, where unsupported/unknown fields retain their diagnostics.
    // The callback must not capture backing: a record can contain Lua cycles,
    // and an mlua Table captured by a Rust closure roots them in the registry.
    let missing = match cached_missing {
        Some(t) => t,
        None => {
            let known = known();
            let prefix = path.to_string();
            let missing = lua.create_table()?;
            missing.set(
                "__index",
                lua.create_function(move |lua, (_table, key): (Table, String)| {
                    let symbol = format!("{prefix}.{key}");
                    let s = session(lua);
                    let descriptor = s.borrow().descriptors.get(&symbol).cloned();
                    if descriptor
                        .as_ref()
                        .is_some_and(|d| matches!(d.status, "not_yet" | "unsupported"))
                        || (descriptor.is_none()
                            && !known.as_ref().is_some_and(|keys| keys.contains(&key)))
                    {
                        let code = descriptor.as_ref().map_or("unknown_symbol", |d| d.status);
                        diagnose(
                            lua,
                            &symbol,
                            code,
                            descriptor
                                .as_ref()
                                .map_or("unknown field; consult eqoa.capabilities", |d| {
                                    d.reason.as_str()
                                }),
                        );
                    }
                    Ok(Value::Nil)
                })?,
            )?;
            if let Some(mut s) = session_rc.as_ref().and_then(|rc| rc.try_borrow_mut().ok()) {
                s.guard_missing.insert(sig, missing.clone());
            }
            missing
        }
    };
    let readonly = match cached_readonly {
        Some(f) => f,
        None => {
            let f = lua.create_function(|_, _: MultiValue| -> Result<()> {
                Err(mlua::Error::runtime(
                    "eqoa records and modules are read-only",
                ))
            })?;
            if let Some(mut s) = session_rc.as_ref().and_then(|rc| rc.try_borrow_mut().ok()) {
                s.guard_readonly = Some(f.clone());
            }
            f
        }
    };
    let proxy = lua.create_table()?;
    let mt = lua.create_table_with_capacity(0, 3)?;
    backing.set_metatable(Some(missing))?;
    mt.set("__index", backing)?;
    mt.set("__newindex", readonly)?;
    mt.set("__metatable", false)?;
    proxy.set_metatable(Some(mt))?;
    Ok(proxy)
}

fn call_error_code(error: &mlua::Error) -> &'static str {
    use mlua::Error;
    match error {
        Error::CallbackError { cause, .. } | Error::WithContext { cause, .. } => {
            call_error_code(cause)
        }
        Error::BadArgument { .. } | Error::FromLuaConversionError { .. } => "invalid_argument",
        Error::UserDataDestructed | Error::UserDataTypeMismatch => "stale_handle",
        Error::ExternalError(error) => {
            error
                .downcast_ref::<std::io::Error>()
                .map_or("failed", |error| match error.kind() {
                    std::io::ErrorKind::NotFound => "not_found",
                    std::io::ErrorKind::PermissionDenied => "permission_denied",
                    _ => "io_error",
                })
        }
        _ => "failed",
    }
}

/// An owned diagnostic route that native consumers can use without evaluating Lua.
#[derive(Clone)]
pub(crate) struct DiagnosticSink(std::rc::Weak<RefCell<Session>>);
impl DiagnosticSink {
    pub(crate) fn record(&self, symbol: &str, code: &str, reason: &str) {
        if let Some(session) = self.0.upgrade() {
            session.borrow_mut().diagnostic(symbol, code, reason);
        }
    }
}
pub(crate) fn diagnostic_sink(lua: &Lua) -> DiagnosticSink {
    DiagnosticSink(Rc::downgrade(&session(lua)))
}

pub(crate) fn bind(lua: &Lua, module: &str, name: &str, function: Function) -> Result<()> {
    let modules: Table = lua.named_registry_value("eqoa_modules")?;
    let table: Table = modules.get(module)?;
    let symbol = format!("eqoa.{module}.{name}");
    let blocked = !cfg!(windows)
        && (module == "files"
            || module == "sound"
            || matches!((module, name), ("textures", "load") | ("fonts", "load")));
    table.set(
        name,
        lua.create_function(move |lua, args: MultiValue| {
            if blocked {
                return error(
                    lua,
                    &symbol,
                    "unsupported",
                    "secure file or sound backend is not implemented on this platform",
                );
            }
            if matches!(
                symbol.as_str(),
                "eqoa.client.get"
                    | "eqoa.client.display"
                    | "eqoa.client.time"
                    | "eqoa.player.get"
                    | "eqoa.player.stats"
                    | "eqoa.actions.assist"
                    | "eqoa.actions.target_target"
                    | "eqoa.actions.toggle_stats_box"
                    | "eqoa.target.get"
                    | "eqoa.group.get"
                    | "eqoa.entities.list"
                    | "eqoa.abilities.list"
                    | "eqoa.hotbar.list"
                    | "eqoa.effects.list"
                    | "eqoa.chat.channel"
                    | "eqoa.combat.counters"
                    | "eqoa.input.get"
                    | "eqoa.fonts.default"
                    | "eqoa.settings.load"
            ) && !args.is_empty()
            {
                return error(
                    lua,
                    &symbol,
                    "invalid_argument",
                    "this function takes no arguments",
                );
            }
            super::native_core::flush_deferred();
            match function.call::<MultiValue>(args) {
                Ok(value) => Ok(value),
                Err(e) if fatal_memory(&e) => Err(e),
                Err(e) => error(lua, &symbol, call_error_code(&e), &e.to_string()),
            }
        })?,
    )?;
    register_descriptor(lua, module, name, blocked)
}

/// `bind` for hot, never-blocked calls: the body runs inside the one Lua
/// callback instead of being called back through a second wrapped function.
pub(crate) fn bind_fn<A, F>(lua: &Lua, module: &str, name: &str, f: F) -> Result<()>
where
    A: mlua::FromLuaMulti,
    F: Fn(&Lua, A) -> Result<MultiValue> + 'static,
{
    let modules: Table = lua.named_registry_value("eqoa_modules")?;
    let table: Table = modules.get(module)?;
    let symbol = format!("eqoa.{module}.{name}");
    table.set(
        name,
        lua.create_function(move |lua, args: A| {
            super::native_core::flush_deferred();
            match f(lua, args) {
                Ok(value) => Ok(value),
                Err(e) if fatal_memory(&e) => Err(e),
                Err(e) => error(lua, &symbol, call_error_code(&e), &e.to_string()),
            }
        })?,
    )?;
    register_descriptor(lua, module, name, false)
}

fn register_descriptor(lua: &Lua, module: &str, name: &str, blocked: bool) -> Result<()> {
    let mut s = session(lua).borrow().descriptors.clone();
    let path = format!("eqoa.{module}.{name}");
    let round = s.get(&path).and_then(|d| d.round).or(Some(2));
    s.insert(
        path,
        Descriptor {
            status: if blocked { "unsupported" } else { "supported" },
            round,
            reason: if blocked {
                "secure file or sound backend is not implemented on this platform".into()
            } else {
                String::new()
            },
        },
    );
    let children = s
        .iter()
        .filter(|(p, _)| {
            p.starts_with(&format!("eqoa.{module}.")) && !p[6 + module.len()..].contains('.')
        })
        .map(|(_, d)| d.status)
        .collect::<Vec<_>>();
    let status = if children.iter().all(|s| *s == "unsupported") {
        "unsupported"
    } else if children.iter().all(|s| *s == "supported") {
        "supported"
    } else {
        "partial"
    };
    let mod_path = format!("eqoa.{module}");
    let mod_round = s.get(&mod_path).and_then(|d| d.round).or(Some(2));
    s.insert(
        mod_path,
        Descriptor {
            status,
            round: mod_round,
            reason: String::new(),
        },
    );
    session(lua).borrow_mut().descriptors = s;
    Ok(())
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    lua.set_app_data(Rc::new(RefCell::new(Session::new())));
    session(lua).borrow_mut().addon = host.addon_id();
    let root: Table = lua.globals().get("eqoa").unwrap_or(lua.create_table()?);
    lua.set_named_registry_value("eqoa_root", root.clone())?;
    root.set("api", "eqoa/1")?;
    root.set("revision", 1)?;
    let modules = lua.create_table()?;
    let descriptors = session(lua).borrow().descriptors.clone();
    for (path, d) in &descriptors {
        let Some(tail) = path.strip_prefix("eqoa.") else {
            continue;
        };
        let Some((module, name)) = tail.split_once('.') else {
            continue;
        };
        if name.contains('.') {
            continue;
        }
        let backing = match modules.get::<Value>(module)? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table()?;
                modules.set(module, t.clone())?;
                root.set(
                    module,
                    guard(lua, t.clone(), &format!("eqoa.{module}"), None)?,
                )?;
                t
            }
        };
        let symbol = path.clone();
        let code = d.status;
        let reason = d.reason.clone();
        backing.set(
            name,
            lua.create_function(move |lua, _: MultiValue| error(lua, &symbol, code, &reason))?,
        )?;
    }
    lua.set_named_registry_value("eqoa_modules", modules)?;
    bind(
        lua,
        "capabilities",
        "get",
        lua.create_function(|lua, path: String| {
            record(
                lua,
                "eqoa.capabilities.Capability",
                &session(lua).borrow().capability(&path),
            )
        })?,
    )?;
    bind(
        lua,
        "capabilities",
        "has",
        lua.create_function(|lua, path: String| {
            Ok(session(lua)
                .borrow()
                .descriptors
                .get(&path)
                .is_some_and(|d| d.status == "supported"))
        })?,
    )?;
    bind(
        lua,
        "capabilities",
        "list",
        lua.create_function(|lua, prefix: Option<String>| {
            let s = session(lua);
            let s = s.borrow();
            let prefix = prefix.unwrap_or_default();
            let t = lua.create_table()?;
            for p in s.descriptors.keys().filter(|p| {
                prefix.is_empty()
                    || p.as_str() == prefix
                    || p.strip_prefix(&prefix)
                        .is_some_and(|tail| tail.starts_with('.'))
            }) {
                t.raw_push(record(
                    lua,
                    "eqoa.capabilities.Capability",
                    &s.capability(p),
                )?)?;
            }
            Ok(t)
        })?,
    )?;
    bind(lua,"client","get",lua.create_function(|lua,()| record(lua,"eqoa.client.Client",&json!({"in_world":false,"phase":"pregame","generation":0,"tick":0,"frame":0,"time":0})))?)?;
    bind(
        lua,
        "player",
        "get",
        lua.create_function(|lua, ()| {
            error(
                lua,
                "eqoa.player.get",
                "not_in_world",
                "enter a loaded world first",
            )
        })?,
    )?;
    bind(
        lua,
        "hud",
        "hide",
        lua.create_function(|lua, _: Value| {
            error(
                lua,
                "eqoa.hud.hide",
                "not_in_world",
                "enter a loaded world first",
            )
        })?,
    )?;
    bind(
        lua,
        "hud",
        "hidden_by",
        lua.create_function(|lua, _: Value| lua.create_table())?,
    )?;
    bind(
        lua,
        "input",
        "get",
        lua.create_function(|lua, ()| {
            record(
                lua,
                "eqoa.input.InputState",
                &json!({"focused":false,"pad":null,"text_capture":false}),
            )
        })?,
    )?;
    let root = guard(lua, root, "eqoa", None)?;
    lua.globals().set("eqoa", root.clone())?;
    host.register_host_module("eqoa", Value::Table(root.clone()))?;
    for module in lua
        .named_registry_value::<Table>("eqoa_modules")?
        .pairs::<String, Table>()
    {
        let (name, _) = module?;
        host.register_host_module(format!("eqoa.{name}"), root.get(name)?)?;
    }
    super::native_events::install(host, host.once_mode())?;
    install_fields(lua);
    super::native_state::install(host)?;
    let modules: Table = lua.named_registry_value("eqoa_modules")?;
    let scope = lua.create_table()?;
    for (id, name) in ["self", "target", "group", "pet", "corpse", "unknown"]
        .into_iter()
        .enumerate()
    {
        scope.set(name, id)?;
        describe(
            lua,
            &format!("eqoa.abilities.scope.{name}"),
            "supported",
            Some(2),
            "",
        );
    }
    modules
        .get::<Table>("abilities")?
        .set("scope", guard(lua, scope, "eqoa.abilities.scope", None)?)?;
    let status = lua.create_table()?;
    for (name, id) in [("nameplate_hidden", 1), ("poisoned", 4), ("diseased", 8)] {
        status.set(name, id)?;
        describe(
            lua,
            &format!("eqoa.entities.status.{name}"),
            "supported",
            Some(2),
            "",
        );
    }
    modules
        .get::<Table>("entities")?
        .set("status", guard(lua, status, "eqoa.entities.status", None)?)?;
    super::file_io::install_native(host)?;
    super::native_settings::install(host)?;
    bind(
        lua,
        "log",
        "write",
        lua.create_function(|lua, (level, message): (Value, Value)| {
            let Some(level) = level
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
                .filter(|s| matches!(s.as_str(), "debug" | "info" | "warn" | "error"))
            else {
                return error(
                    lua,
                    "eqoa.log.write",
                    "invalid_argument",
                    "level must be debug, info, warn, or error",
                );
            };
            let Some(message) = message.as_string().and_then(|s| {
                s.to_str()
                    .ok()
                    .map(|s| s.chars().take(2048).collect::<String>())
            }) else {
                return error(
                    lua,
                    "eqoa.log.write",
                    "invalid_argument",
                    "message must be a string",
                );
            };
            let messages: Table = lua.named_registry_value("forge_log")?;
            if messages.raw_len() >= 64 {
                return error(lua, "eqoa.log.write", "quota", "local log queue full");
            }
            messages.raw_push(format!("eqoa/1 [{level}] {message}"))?;
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    annotate_host(host)?;
    Ok(())
}

fn nested_type(path: &str, key: &str) -> Option<&'static str> {
    match (path, key) {
        ("eqoa.player.Player", "hp" | "power") => Some("eqoa.player.Vital"),
        ("eqoa.player.Player", "attributes") => Some("eqoa.player.Attributes"),
        ("eqoa.player.Attributes", "base" | "total") => Some("eqoa.player.SevenStats"),
        ("eqoa.player.Player", "resists") => Some("eqoa.player.Resists"),
        ("eqoa.player.Player", "location") => Some("eqoa.player.Location"),
        (_, "position" | "velocity") => Some("eqoa.Vec3"),
        ("eqoa.group.Group", "members") => Some("eqoa.group.Member"),
        ("eqoa.group.Group", "raid") => Some("eqoa.group.RaidMember"),
        ("eqoa.group.Group", "invites") => Some("eqoa.group.Invite"),
        ("eqoa.hotbar.Hotbar", "slots") => Some("eqoa.hotbar.HotbarSlot"),
        ("eqoa.chat.ChatMessage", "color") => Some("eqoa.Color"),
        ("eqoa.input.InputState", "pad") => Some("eqoa.input.Pad"),
        ("eqoa.input.Pad", "left" | "right") => Some("eqoa.Vec2"),
        ("eqoa.input.Pad", "raw_left" | "raw_right") => Some("eqoa.input.RawAxes"),
        ("eqoa.player.Stats", "rows" | "columns") => Some("eqoa.player.StatItem"),
        ("eqoa.player.Stats", "resists") => Some("eqoa.player.Stats"),
        (
            "eqoa.player.Stats",
            "strength" | "stamina" | "agility" | "dexterity" | "wisdom" | "intelligence"
            | "charisma" | "armor" | "max_hp" | "hp_max" | "max_power" | "power" | "fire" | "cold"
            | "lightning" | "arcane" | "poison" | "disease",
        ) => Some("eqoa.player.StatItem"),
        ("eqoa.hud.Layout", "player") => Some("eqoa.hud.PlayerLayout"),
        ("eqoa.hud.Layout", "group_members") => Some("eqoa.hud.GroupMemberLayout"),
        ("eqoa.hud.Layout", "target") => Some("eqoa.hud.TargetLayout"),
        ("eqoa.hud.Layout", "pet") => Some("eqoa.hud.SinglePanelLayout"),
        (
            "eqoa.hud.PlayerLayout"
            | "eqoa.hud.TargetLayout"
            | "eqoa.hud.GroupMemberLayout"
            | "eqoa.hud.SinglePanelLayout",
            "panel" | "health" | "power" | "exp",
        ) => Some("eqoa.hud.Rect"),
        _ => None,
    }
}
fn install_fields(lua: &Lua) {
    for (path, fields) in [
        ("eqoa.Error", "code message capability retry_after"),
        ("eqoa.Vec2", "x y"),
        ("eqoa.Vec3", "x y z"),
        ("eqoa.Color", "r g b a"),
        (
            "eqoa.capabilities.Capability",
            "path status revision round reason children",
        ),
        (
            "eqoa.client.Client",
            "in_world phase generation tick frame time world_id",
        ),
        ("eqoa.client.DisplayMetrics", "width height render_scale"),
        (
            "eqoa.player.Player",
            "entity id name class_id class_name race_id race_name level experience experience_debt hp_fraction hp power attributes armor resists position velocity speed horizontal_speed heading target_id pet_id auto_attacking dead has_pet fighting location",
        ),
        ("eqoa.player.Vital", "current maximum base"),
        ("eqoa.player.Attributes", "base total"),
        (
            "eqoa.player.SevenStats",
            "strength stamina agility dexterity wisdom intelligence charisma",
        ),
        (
            "eqoa.player.Resists",
            "base poison disease fire cold lightning arcane",
        ),
        (
            "eqoa.player.Location",
            "world_id place_id compass_id name cell_x cell_z",
        ),
        (
            "eqoa.player.Stats",
            "rows columns resists strength stamina agility dexterity wisdom intelligence charisma armor max_hp hp_max max_power power fire cold lightning arcane poison disease",
        ),
        (
            "eqoa.player.StatItem",
            "name short word stat_id current base min max without_min without_max change effect_change shift",
        ),
        (
            "eqoa.entities.Entity",
            "ref id name level position hp_fraction power kind class_code target_id status_flags poisoned diseased disposition disposition_name",
        ),
        (
            "eqoa.group.Group",
            "in_group self_leader members raid invites",
        ),
        (
            "eqoa.group.Member",
            "entity id name active hp_fraction power position world_id",
        ),
        (
            "eqoa.group.RaidMember",
            "id name hp_fraction power_fraction",
        ),
        ("eqoa.group.Invite", "id name"),
        (
            "eqoa.abilities.Ability",
            "ref id name description display_key spellbook_slot usable level range cast_time power_cost icon_background icon_foreground scope cooldown equipment_mask lockout",
        ),
        ("eqoa.abilities.RangeResult", "in_range distance"),
        (
            "eqoa.hotbar.Hotbar",
            "index kind selected selected_slot slots",
        ),
        (
            "eqoa.hotbar.HotbarSlot",
            "index kind ability weapon_set command icon ui_icon",
        ),
        (
            "eqoa.cooldowns.Cooldown",
            "active lockout started_at estimated_remaining",
        ),
        ("eqoa.effects.Effect", "index icon name"),
        (
            "eqoa.chat.ChatMessage",
            "seq time text color channel channel_code sender outgoing",
        ),
        (
            "eqoa.combat.CombatHit",
            "attacker_id defender_id amount kind color outgoing incoming from_pet to_pet",
        ),
        ("eqoa.combat.Counters", "received dropped"),
        ("eqoa.input.InputState", "focused pad text_capture"),
        ("eqoa.input.Pad", "buttons left right raw_left raw_right"),
        ("eqoa.input.RawAxes", "x y"),
        (
            "eqoa.events.Event",
            "name seq time tick frame generation data",
        ),
        ("eqoa.textures.TextureInfo", "texture width height"),
        ("eqoa.fonts.FontInfo", "font status reason"),
        (
            "eqoa.hud.Frame",
            "width height offset_x offset_y scale_x scale_y side top window_width window_height",
        ),
        (
            "eqoa.hud.Rect",
            "frame_x frame_y frame_width frame_height x y width height",
        ),
        ("eqoa.hud.Layout", "player group_members target pet"),
        ("eqoa.hud.PlayerLayout", "panel health power exp"),
        ("eqoa.hud.TargetLayout", "panel health power"),
        ("eqoa.hud.GroupMemberLayout", "index panel health power"),
        ("eqoa.hud.SinglePanelLayout", "panel"),
        ("eqoa.options.Toggle", "id label default value item_id"),
    ] {
        let round = if path.starts_with("eqoa.hud.")
            || path.starts_with("eqoa.options.")
            || matches!(path, "eqoa.player.Stats" | "eqoa.player.StatItem")
        {
            Some(3)
        } else {
            Some(2)
        };
        describe(lua, path, "supported", round, "");
        for field in fields.split_whitespace() {
            let field_round = if matches!(
                (path, field),
                ("eqoa.entities.Entity", "power") | ("eqoa.group.Member", "power")
            ) {
                Some(3)
            } else {
                round
            };
            describe(
                lua,
                &format!("{path}.{field}"),
                "supported",
                field_round,
                "",
            );
        }
    }
    describe(
        lua,
        "eqoa.player.Player.mastery",
        "not_yet",
        Some(4),
        "Class Mastery arrives in round 4",
    );
    describe(
        lua,
        "eqoa.hotbar.HotbarSlot.item",
        "not_yet",
        Some(3),
        "item handles arrive in round 3",
    );
}

pub(crate) fn fatal_memory(error: &mlua::Error) -> bool {
    match error {
        mlua::Error::MemoryError(_) => true,
        mlua::Error::CallbackError { cause, .. } => fatal_memory(cause),
        _ => false,
    }
}

pub(crate) struct Phase {
    session: Rc<RefCell<Session>>,
    old: String,
}
impl Drop for Phase {
    fn drop(&mut self) {
        self.session.borrow_mut().phase = self.old.clone();
    }
}
pub(crate) fn phase(lua: &Lua, name: &str) -> Phase {
    let session = session(lua);
    let old = std::mem::replace(&mut session.borrow_mut().phase, name.into());
    Phase { session, old }
}
