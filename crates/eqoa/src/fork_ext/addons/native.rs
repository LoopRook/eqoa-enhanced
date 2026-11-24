//! The versioned native surface. Capability presence is independent of game state.
use super::host::Host;
use mlua::{Function, Lua, MultiValue, Result, Table, Value};
use serde_json::{Value as Json, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Instant};

#[derive(Clone)]
struct Descriptor {
    status: &'static str,
    module: bool,
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
    pub(crate) capabilities: Option<Vec<String>>,
    pub(crate) replaces: Vec<String>,
    /// The mod is a native `eqoa/1` package (a `mod.json`), not a Forge import.
    pub(crate) native_manifest: bool,
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
    /// Last element records of each list call site, by element id (or position without one): a list
    /// that changed in a few elements rebuilds only those.
    list_cache: std::collections::HashMap<String, std::collections::HashMap<u64, (Json, Value)>>,
}

impl Session {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            descriptors: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            pending: Vec::new(),
            start: Instant::now(),
            addon: String::new(),
            capabilities: None,
            replaces: Vec::new(),
            native_manifest: false,
            vm: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            phase: "lua".into(),
            tokens: 32.0,
            token_time: 0.0,
            guard_missing: std::collections::HashMap::new(),
            guard_readonly: None,
            record_cache: std::collections::HashMap::new(),
            list_cache: std::collections::HashMap::new(),
        }
    }
    fn describe(&mut self, path: &str, status: &'static str, round: Option<u8>, reason: &str) {
        self.descriptors.insert(
            path.into(),
            Descriptor {
                status,
                module: false,
                round,
                reason: reason.into(),
            },
        );
    }
    fn describe_module(&mut self, path: &str, round: Option<u8>, reason: &str) {
        self.describe(path, "unsupported", round, reason);
        self.descriptors.get_mut(path).unwrap().module = true;
    }

    fn children<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> {
        self.descriptors
            .range(prefix.to_owned()..)
            .take_while(move |(path, _)| path.starts_with(prefix))
            .filter_map(move |(path, _)| {
                (!path[prefix.len()..].contains('.')).then_some(path.as_str())
            })
    }

    fn status(&self, path: &str) -> Option<&'static str> {
        let descriptor = self.descriptors.get(path)?;
        if !descriptor.module {
            return Some(descriptor.status);
        }
        let prefix = format!("{path}.");
        let mut statuses = self
            .children(&prefix)
            .filter_map(|child| self.status(child));
        let first = statuses.next().unwrap_or(descriptor.status);
        Some(if statuses.all(|status| status == first) {
            first
        } else {
            "partial"
        })
    }

    fn capability(&self, path: &str) -> Json {
        let d = self.descriptors.get(path);
        let status = self.status(path).unwrap_or("unknown");
        let prefix = format!("{path}.");
        json!({"path":path,"status":status,
            "revision":u8::from(matches!(status,"supported"|"partial")),
            "round":d.and_then(|d| d.round), "reason":d.filter(|d| !d.reason.is_empty()).map(|d| &d.reason),
            "children":self.children(&prefix).collect::<Vec<_>>()})
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
pub(super) fn bind_compat(
    lua: &Lua,
    table: &Table,
    path: &str,
    round: u8,
    function: Function,
) -> Result<()> {
    let (_, name) = path.rsplit_once('.').expect("qualified member path");
    table.raw_set(name, function)?;
    describe(lua, path, "supported", Some(round), "");
    Ok(())
}

pub(crate) fn describe_module(lua: &Lua, path: &str, round: Option<u8>, reason: &str) {
    session(lua)
        .borrow_mut()
        .describe_module(path, round, reason);
}

pub(crate) fn init_session(lua: &Lua) {
    lua.set_app_data(Rc::new(RefCell::new(Session::new())));
}

pub(crate) fn status(lua: &Lua, path: &str) -> Option<&'static str> {
    session(lua).borrow().status(path)
}

#[derive(Clone, Copy)]
pub(crate) enum RefusalShape {
    Native,
    ForgeNil,
    ForgeReason,
    ForgeReasonAndNil,
}

impl RefusalShape {
    #[cfg(test)]
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::ForgeNil => "forge_nil",
            Self::ForgeReason => "forge_reason",
            Self::ForgeReasonAndNil => "forge_reason_and_nil",
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Each refusal's declared code and return shape by member path, for the refusal-contract test.
    static DECLARED_REFUSALS: RefCell<std::collections::HashMap<String, (&'static str, RefusalShape)>> =
        RefCell::new(std::collections::HashMap::new());
}

/// The code and return shape `path`'s refusal was declared with on this thread.
#[cfg(test)]
pub(crate) fn declared_refusal(path: &str) -> Option<(&'static str, &'static str)> {
    DECLARED_REFUSALS.with(|d| d.borrow().get(path).map(|(code, shape)| (*code, shape.name())))
}

/// Installs a refusal and its capability from the same declaration. A service
/// installer can later replace the member and mark that real binding supported.
pub(crate) fn refuse_member(
    lua: &Lua,
    table: &Table,
    path: &str,
    code: &'static str,
    round: Option<u8>,
    reason: &'static str,
    shape: RefusalShape,
) -> Result<()> {
    let (module, name) = path.rsplit_once(['.', ':']).expect("qualified member path");
    let symbol = path.to_owned();
    table.raw_set(
        name,
        lua.create_function(move |lua, _: MultiValue| {
            if matches!(shape, RefusalShape::Native) {
                return error(lua, &symbol, code, reason);
            }
            diagnose(lua, &symbol, code, reason);
            match shape {
                RefusalShape::ForgeNil => lua.pack_multi(Value::Nil),
                RefusalShape::ForgeReason => lua.pack_multi((Value::Nil, reason)),
                RefusalShape::ForgeReasonAndNil => lua.pack_multi((Value::Nil, reason, Value::Nil)),
                RefusalShape::Native => unreachable!(),
            }
        })?,
    )?;
    let status = if code == "not_yet" {
        "not_yet"
    } else {
        "unsupported"
    };
    describe(lua, path, status, round, reason);
    #[cfg(test)]
    DECLARED_REFUSALS.with(|d| d.borrow_mut().insert(path.to_owned(), (code, shape)));
    if module == "ImGui" {
        describe(lua, &format!("eqoa.imgui.{name}"), status, round, reason);
        #[cfg(test)]
        DECLARED_REFUSALS.with(|d| d.borrow_mut().insert(format!("eqoa.imgui.{name}"), (code, shape)));
    }
    Ok(())
}

pub(crate) fn refuse_native(
    lua: &Lua,
    module: &str,
    name: &str,
    code: &'static str,
    round: Option<u8>,
    reason: &'static str,
) -> Result<()> {
    let modules: Table = lua.named_registry_value("eqoa_modules")?;
    refuse_member(
        lua,
        &modules.get(module)?,
        &format!("eqoa.{module}.{name}"),
        code,
        round,
        reason,
        RefusalShape::Native,
    )
}
pub(crate) fn capability_grants(caps: &[String], cap: &str) -> bool {
    caps.iter()
        .any(|c| c == cap || (cap.starts_with("eqoa.actions.") && c == "eqoa.actions.*"))
}
pub(crate) fn require_capability(
    lua: &Lua,
    symbol: &str,
    cap: &str,
) -> std::result::Result<(), (&'static str, String)> {
    let s_ref = session(lua);
    let s = s_ref.borrow();
    let Some(caps) = s.capabilities.as_ref() else {
        return Err((
            "not_declared",
            format!("{symbol} requires a 'capabilities' list declaring '{cap}' in mod.json"),
        ));
    };
    if capability_grants(caps, cap) {
        return Ok(());
    }
    Err((
        "permission_denied",
        format!("{symbol} requires capability '{cap}' in mod.json"),
    ))
}
/// What a Forge import can make the game do on its own through the helpers it keeps (Forge chat sends, slash
/// commands and the weapon-set alias). Forge imports declare no capabilities, so these are not gated for them
/// (compatibility first); Mods & Addons labels every Forge import with them instead.
pub(crate) const FORGE_IMPORT_ACTIONS: [&str; 3] =
    ["eqoa.actions.chat", "eqoa.actions.command", "eqoa.actions.weapon_set"];

/// The gate for a legacy alias or Forge helper that acts: a native `eqoa/1` mod (or a test host given
/// capabilities) passes the same capability gate as the typed action `cap`; a Forge import keeps today's
/// behaviour and carries the label instead.
pub(crate) fn require_acting_capability(
    lua: &Lua,
    symbol: &str,
    cap: &str,
) -> std::result::Result<(), (&'static str, String)> {
    let gated = {
        let s = session(lua);
        let s = s.borrow();
        s.native_manifest || s.capabilities.is_some()
    };
    if gated {
        require_capability(lua, symbol, cap)
    } else {
        Ok(())
    }
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
            describe_module(lua, &name, Some(2), "native Forge compatibility adapter");
            if let Value::Table(table) = value {
                for pair in table.pairs::<String, Value>() {
                    let (key, _) = pair?;
                    let path = format!("{name}.{key}");
                    if status(lua, &path).is_none() {
                        describe(lua, &path, "supported", Some(2), "");
                    }
                }
            }
        }
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
    quiet_error(lua, symbol, code, message)
}

pub(super) fn quiet_error(
    lua: &Lua,
    symbol: &str,
    code: &str,
    message: &str,
) -> Result<MultiValue> {
    let e = record(
        lua,
        "eqoa.Error",
        &json!({"code":code,"message":message,"capability":symbol}),
    )?;
    lua.pack_multi((Value::Nil, e))
}

pub(crate) fn record(lua: &Lua, path: &str, value: &Json) -> Result<Value> {
    if matches!(value, Json::Array(_)) || matches!(value, Json::Object(f) if !f.contains_key("$ref")) {
        if let Some(built) = super::record_raw::build(lua, path, value)? {
            return Ok(built);
        }
    }
    record_slow(lua, path, value)
}

/// `record` one Lua call at a time (the build `record_raw` does in a single protected call).
pub(super) fn record_slow(lua: &Lua, path: &str, value: &Json) -> Result<Value> {
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
                table.raw_set(i + 1, record_slow(lua, path, v)?)?;
            }
            Value::Table(table)
        }
        Json::Object(fields) if fields.contains_key("$ref") => {
            super::native_state::make_ref(lua, value)?
        }
        Json::Object(fields) => {
            // Leaves go onto the stack as they are, with no Lua value made for them first
            // (a record is built per entity per frame and per `entity.*` event).
            let backing = lua.create_table_with_capacity(0, fields.len())?;
            for (k, v) in fields {
                match v {
                    Json::Null => {}
                    Json::Bool(b) => backing.raw_set(k.as_str(), *b)?,
                    Json::Number(n) => backing.raw_set(
                        k.as_str(),
                        n.as_f64()
                            .ok_or_else(|| mlua::Error::runtime("invalid native number"))?,
                    )?,
                    Json::String(s) => backing.raw_set(k.as_str(), s.as_str())?,
                    _ => backing.raw_set(
                        k.as_str(),
                        record_slow(lua, nested_type(path, k).unwrap_or(path), v)?,
                    )?,
                }
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

/// FNV-1a over the path and the keys (a record is shaped per object built, so this is hot).
pub(super) fn signature<'a>(path: &str, has_keys: bool, keys: impl Iterator<Item = &'a str>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        h = (h ^ 0xff).wrapping_mul(0x0100_0000_01b3);
    };
    feed(path.as_bytes());
    feed(&[has_keys as u8]);
    for k in keys {
        feed(k.as_bytes());
    }
    h
}

/// `record`, reusing the previous result of `slot` while the native value is unchanged.
///
/// Lists (the entity and ability records, map markers) are the expensive case: one full
/// record per element with its guarded tables. They are cached per element (`record_list_cached`),
/// so a caller that reads the list every frame (a minimap hover, a search) pays only the
/// comparison for the elements that did not change. A list is handed out as a fresh table each
/// call, as before, so a caller may insert, remove or reorder its own copy; its elements are the
/// shared read-only records.
pub(crate) fn record_cached(lua: &Lua, slot: &str, path: &str, value: &Json) -> Result<Value> {
    if let Json::Array(items) = value {
        return record_list_cached(lua, slot, path, items);
    }
    if !matches!(value, Json::Object(_)) {
        return record(lua, path, value);
    }
    let session_rc = session(lua);
    let compared = super::addon_perf::section("record.compare");
    if let Ok(s) = session_rc.try_borrow() {
        if let Some((json, table)) = s.record_cache.get(slot) {
            if json == value {
                drop(compared);
                return Ok(Value::Table(table.clone()));
            }
        }
    }
    drop(compared);
    let _built = super::addon_perf::section("record.build");
    let built = record(lua, path, value)?;
    let Value::Table(table) = &built else {
        return Ok(built);
    };
    if let Ok(mut s) = session_rc.try_borrow_mut() {
        s.record_cache
            .insert(slot.to_string(), (value.clone(), table.clone()));
    }
    Ok(Value::Table(table.clone()))
}

/// The key a list element is remembered by: its `id` when it has a whole-number one, else its position.
fn element_key(item: &Json, index: usize) -> u64 {
    item.get("id").and_then(Json::as_u64).unwrap_or((1 << 63) | index as u64)
}

/// `record_cached` for a list: a fresh top-level table of element records, an element unchanged since
/// the last call keeping the record it had (read-only and shared, as a whole cached list's were), the
/// changed and new ones built. A list in which a few creatures moved rebuilds a few records, not all.
fn record_list_cached(lua: &Lua, slot: &str, path: &str, items: &[Json]) -> Result<Value> {
    let _timed = super::addon_perf::section("record.list");
    let session_rc = session(lua);
    let mut previous = session_rc
        .try_borrow_mut()
        .ok()
        .and_then(|mut s| s.list_cache.remove(slot))
        .unwrap_or_default();
    let table = lua.create_table_with_capacity(items.len(), 0)?;
    let mut kept = std::collections::HashMap::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let key = element_key(item, i);
        let (json, record) = match previous.remove(&key) {
            Some((json, record)) if json == *item => (json, record),
            _ => (item.clone(), record(lua, path, item)?),
        };
        table.raw_set(i + 1, record.clone())?;
        kept.insert(key, (json, record));
    }
    if let Ok(mut s) = session_rc.try_borrow_mut() {
        s.list_cache.insert(slot.to_string(), kept);
    }
    Ok(Value::Table(table))
}

/// `guard` for a record whose known keys are a fixed list (no list is built unless a shape is new).
pub(super) fn guard_lazy(lua: &Lua, backing: Table, path: &str, keys: &[&str]) -> Result<Table> {
    let sig = signature(path, true, keys.iter().copied());
    guard_keyed(lua, backing, path, sig, || Some(keys.iter().map(|s| s.to_string()).collect()))
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
    // The common case, a shape seen before: one borrow, the shared tables used by reference.
    if let Some(s) = session_rc.as_ref().and_then(|rc| rc.try_borrow().ok())
        && let (Some(missing), Some(readonly)) = (s.guard_missing.get(&sig), s.guard_readonly.as_ref())
    {
        return guard_proxy(lua, backing, missing, readonly);
    }
    let (missing, readonly) = guard_parts(lua, sig, path, known)?;
    guard_proxy(lua, backing, &missing, &readonly)
}

/// The shared pieces of a guarded record of shape `sig`: the missing-field metatable (made the
/// first time the shape is seen) and the read-only guard.
pub(super) fn guard_parts(
    lua: &Lua,
    sig: u64,
    path: &str,
    known: impl FnOnce() -> Option<Vec<String>>,
) -> Result<(Table, Function)> {
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
    Ok((missing, readonly))
}

/// The read-only proxy over `backing`: reads fall to the backing table, whose own metatable
/// reports a missing field; writes raise.
fn guard_proxy(lua: &Lua, backing: Table, missing: &Table, readonly: &Function) -> Result<Table> {
    let proxy = lua.create_table()?;
    let mt = lua.create_table_with_capacity(0, 3)?;
    backing.set_metatable(Some(missing.clone()))?;
    mt.raw_set("__index", backing)?;
    mt.raw_set("__newindex", readonly)?;
    mt.raw_set("__metatable", false)?;
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
    if !cfg!(windows)
        && (module == "files"
            || module == "sound"
            || matches!((module, name), ("textures", "load") | ("fonts", "load")))
    {
        let round = declared_round(lua, &symbol);
        let reason = if module == "sound" {
            "no native sound backend on this platform"
        } else {
            "secure file backend is not implemented on this platform"
        };
        return refuse_member(
            lua,
            &table,
            &symbol,
            "unsupported",
            round,
            reason,
            RefusalShape::Native,
        );
    }
    table.set(
        name,
        lua.create_function(move |lua, args: MultiValue| {
            if matches!(
                symbol.as_str(),
                "eqoa.client.get"
                    | "eqoa.client.display"
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
                    | "eqoa.chat.get_channel"
                    | "eqoa.combat.get_counters"
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
            let _timed = super::addon_perf::api(&symbol);
            super::native_core::flush_deferred();
            match function.call::<MultiValue>(args) {
                Ok(value) => Ok(value),
                Err(e) if fatal_memory(&e) => Err(e),
                Err(e) => error(lua, &symbol, call_error_code(&e), &e.to_string()),
            }
        })?,
    )?;
    register_descriptor(lua, module, name);
    Ok(())
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
            let _timed = super::addon_perf::api(&symbol);
            super::native_core::flush_deferred();
            match f(lua, args) {
                Ok(value) => Ok(value),
                Err(e) if fatal_memory(&e) => Err(e),
                Err(e) => error(lua, &symbol, call_error_code(&e), &e.to_string()),
            }
        })?,
    )?;
    register_descriptor(lua, module, name);
    Ok(())
}

fn declared_round(lua: &Lua, path: &str) -> Option<u8> {
    session(lua)
        .borrow()
        .descriptors
        .get(path)
        .and_then(|d| d.round)
        .or(Some(2))
}

fn register_descriptor(lua: &Lua, module: &str, name: &str) {
    let path = format!("eqoa.{module}.{name}");
    describe(lua, &path, "supported", declared_round(lua, &path), "");
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    let s_ref = session(lua);
    let mut s = s_ref.borrow_mut();
    s.addon = host.addon_id();
    s.capabilities = host.requested_capabilities().map(<[String]>::to_vec);
    s.replaces = host.replaced_windows().to_vec();
    s.native_manifest = host.native_manifest();
    drop(s);
    let root: Table = lua.globals().get("eqoa").unwrap_or(lua.create_table()?);
    lua.set_named_registry_value("eqoa_root", root.clone())?;
    root.set("api", "eqoa/1")?;
    root.set("revision", 1)?;
    let modules = lua.create_table()?;
    // Planned native entry points carry a callable refusal until their service binds them.
    for (module, names, round) in [
        ("abilities", "list get find in_range", 2),
        ("actions", "assist target_target toggle_stats_box", 3),
        (
            "actions",
            "status cancel batch stop_batch batch_status target cycle_target interact attack cast use_hotbar use_item equip swap_items quick_slot weapon_set prepare_weapon_set assign_ability unmemorize_ability unmemorize_all arrange_ability open_loot talk loot loot_all buy sell bank_move bank_money chat command autorun follow stop camera open_window close_window select_row activate_row dialog_answer input_text drop_quest mastery_share mastery_browse mastery_buy mastery_refund menu_command",
            6,
        ),
        (
            "actions",
            "ready_check_answer loot_roll loot_assign loot_abandon",
            7,
        ),
        ("bank", "get", 5),
        ("binds", "get register", 3),
        ("camera", "get compass", 6),
        ("capabilities", "get list has", 2),
        ("chat", "history get_channel channel_name", 2),
        ("client", "get display", 2),
        ("combat", "get_counters", 2),
        ("cooldowns", "get", 2),
        ("effects", "list", 2),
        ("entities", "list get find named distance", 2),
        ("entities", "pick_dot dots", 4),
        ("events", "on off once after", 2),
        ("files", "read write remove", 2),
        ("files", "load_json save_json_async import export", 4),
        ("files", "list", 7),
        ("fonts", "load default release", 2),
        ("gems", "get is_gem format", 5),
        ("group", "get", 2),
        ("hotbar", "list", 2),
        ("hud", "hide show hidden_by", 2),
        ("hud", "frame layout visible text gauge image panel", 3),
        ("icons", "resource ui info", 2),
        ("icons", "class disposition", 6),
        ("input", "get claim release capture_text rotate_dpad", 2),
        ("input", "bind", 3),
        ("input", "pointer claim_pointer minimap_wheel", 4),
        ("inventory", "get", 5),
        (
            "items",
            "get can_use stat slot_info attack_name stat_name",
            5,
        ),
        ("log", "write", 2),
        ("loot", "get", 5),
        ("map", "get open markers view set_guide clear_guide", 4),
        ("map", "set_waypoint clear_waypoint", 7),
        ("map", "terrain", 8),
        ("mastery", "get skills selected", 6),
        ("mods", "send", 4),
        (
            "math",
            "bearing degrees radians normalize signed_angle facing is_bit_zero",
            2,
        ),
        ("merchant", "get price", 5),
        ("options", "toggle get set", 3),
        ("options", "button", 4),
        ("player", "get experience_for_level", 2),
        ("player", "stats", 3),
        ("quests", "list get", 6),
        ("raid", "get ready_check loot_rolls", 7),
        ("settings", "load save on_save on_load", 2),
        ("sound", "load play stop set_volume is_playing release", 2),
        ("target", "get", 2),
        ("text", "localized", 2),
        ("textures", "load rgba release", 2),
        ("windows", "open", 3),
        (
            "windows",
            "get find get_window format_hint parse_hint takeover release taken",
            6,
        ),
        // round A3, last so that no older module takes its round from these
        ("casts", "list get", 8),
        ("actions", "group_invite pet_attack pet_back_off emote jump remove_effect", 8),
        ("effects", "buffs pet", 8),
        ("group", "raid_roles", 8),
        ("player", "training breath", 8),
        ("quests", "goals", 8),
        ("target", "info", 8),
        // round A8: crafting
        ("craft", "get", 8),
        (
            "actions",
            "craft_list craft_inspect craft_tray_add craft_tray_remove craft_tray_clear craft_fill craft_combine craft_keep",
            8,
        ),
        // round A10: mail and the auction
        ("mail", "get", 8),
        ("auction", "get", 8),
        (
            "actions",
            "mail_refresh mail_read mail_delete mail_take mail_send auction_search auction_page auction_my_lots auction_my_bids auction_bidders auction_bid auction_buyout auction_sell",
            8,
        ),
        // round A7: community (friends, ignore, guild, who, group search)
        ("social", "friends ignored friend_request", 8),
        ("guild", "get invite", 8),
        ("who", "get request", 8),
        ("lfg", "get search register unregister", 8),
        (
            "actions",
            "friend_add friend_remove friend_accept friend_refuse ignore_add ignore_remove guild_invite guild_promote guild_demote guild_kick guild_set_motd guild_accept guild_refuse",
            8,
        ),
        // round A5
        ("factions", "list", 8),
        ("combat", "record", 8),
        ("actions", "train_stats request_factions request_combat_record", 8),
        // round A12: world overlays
        ("camera", "project project_entity", 8),
    ] {
        let module_path = format!("eqoa.{module}");
        let backing = match modules.get::<Value>(module)? {
            Value::Table(table) => table,
            _ => {
                let table = lua.create_table()?;
                modules.set(module, table.clone())?;
                root.set(module, guard(lua, table.clone(), &module_path, None)?)?;
                describe_module(lua, &module_path, Some(round), "");
                table
            }
        };
        for name in names.split_whitespace() {
            refuse_member(
                lua,
                &backing,
                &format!("{module_path}.{name}"),
                "unsupported",
                Some(round),
                "this function has no implementation on this host",
                RefusalShape::Native,
            )?;
        }
    }
    lua.set_named_registry_value("eqoa_modules", modules)?;
    describe(lua, "eqoa.api", "supported", Some(2), "");
    describe(lua, "eqoa.revision", "supported", Some(2), "");
    describe(
        lua,
        "ffi",
        "unsupported",
        None,
        "raw memory and FFI are excluded",
    );
    describe(
        lua,
        "eqoa.textures.formats.jpeg",
        "unsupported",
        None,
        "the client decodes PNG only; convert JPEG to PNG or pass RGBA pixels (a capability flag, probed with eqoa.capabilities.has; there is no eqoa.textures.formats table)",
    );
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
        lua.create_function(|lua, path: String| Ok(status(lua, &path) == Some("supported")))?,
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
    refuse_native(
        lua,
        "fonts",
        "game",
        "unsupported",
        Some(7),
        "game bitmap font cannot be represented by the existing ImGui font service",
    )?;
    bind(lua,"client","get",lua.create_function(|lua,()| record(lua,"eqoa.client.Client",&json!({"in_world":false,"phase":"pregame","generation":0,"tick":0,"frame":0,"time":0})))?)?;
    refuse_native(
        lua,
        "player",
        "get",
        "not_in_world",
        Some(2),
        "enter a loaded world first",
    )?;
    refuse_native(
        lua,
        "hud",
        "hide",
        "not_in_world",
        Some(2),
        "enter a loaded world first",
    )?;
    refuse_native(
        lua,
        "hud",
        "hidden_by",
        "unavailable",
        Some(2),
        "no visibility lease on this host",
    )?;
    refuse_native(
        lua,
        "input",
        "get",
        "unavailable",
        Some(2),
        "native input service is not installed on this host",
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
    super::action_broker::install(host)?;
    super::native_items::install(host)?;
    super::native_r6a::install(host)?;
    super::native_a3::install(host)?;
    super::native_a1::install(host)?;
    super::native_a6::install(host)?;
    super::native_a8::install(host)?;
    super::native_a12::install(host)?;
    super::native_a10::install(host)?;
    super::native_a7::install(host)?;
    super::native_a5::install(host)?;
    super::frame_actions::install(host)?;
    super::raid_strips::install(host)?;
    super::map_views::install(host)?;
    super::map_terrain::install(host)?;
    super::guides::install(host)?;
    super::pointer_api::install(host)?;
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
    // the table itself, as `eqoa.items.slots` is (round 7H)
    describe(lua, "eqoa.abilities.scope", "supported", Some(2), "");
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
    describe(lua, "eqoa.entities.status", "supported", Some(2), "");
    super::file_io::install_native(host)?;
    super::async_json::install(host)?;
    super::json_save::install(host)?;
    super::exchange::register(lua);
    super::exchange::install(host)?;
    super::player_files::install(host)?;
    super::option_buttons::install(host)?;
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

pub(super) fn nested_type(path: &str, key: &str) -> Option<&'static str> {
    match (path, key) {
        ("eqoa.player.Player", "hp" | "power") => Some("eqoa.player.Vital"),
        ("eqoa.player.Player", "attributes") => Some("eqoa.player.Attributes"),
        ("eqoa.player.Attributes", "base" | "total") => Some("eqoa.player.SevenStats"),
        ("eqoa.player.Player", "resists") => Some("eqoa.player.Resists"),
        ("eqoa.player.Player", "location") => Some("eqoa.player.Location"),
        ("eqoa.player.Player", "mastery") => Some("eqoa.mastery.Mastery"),
        (_, "position" | "actor_position" | "velocity") => Some("eqoa.Vec3"),
        ("eqoa.map.MapState", "search_anchor") => Some("eqoa.hud.Rect"),
        ("eqoa.group.Group", "members") => Some("eqoa.group.Member"),
        ("eqoa.group.Group", "raid") => Some("eqoa.group.RaidMember"),
        ("eqoa.group.Group", "invites") => Some("eqoa.group.Invite"),
        ("eqoa.quests.QuestGoals", "goals") => Some("eqoa.quests.QuestGoal"),
        ("eqoa.player.Training", "caps") => Some("eqoa.player.SevenStats"),
        ("eqoa.factions.FactionPage", "entries") => Some("eqoa.factions.Faction"),
        ("eqoa.combat.CombatRecord", "rows") => Some("eqoa.combat.CombatRow"),
        ("eqoa.target.TargetInfo", "consider") => Some("eqoa.target.Consider"),
        ("eqoa.target.Consider", "color") => Some("eqoa.Color"),
        ("eqoa.items.Container", "items") => Some("eqoa.items.Item"),
        ("eqoa.items.Container", "merchant") => Some("eqoa.items.MerchantState"),
        ("eqoa.items.Container", "source") => Some("eqoa.items.LootSource"),
        ("eqoa.items.Container", "master_loot") => Some("eqoa.items.MasterLoot"),
        ("eqoa.items.MasterLoot", "members") => Some("eqoa.items.MasterLootMember"),
        ("eqoa.items.Container", "view") => Some("eqoa.items.ContainerView"),
        ("eqoa.raid.RaidState", "members") => Some("eqoa.raid.RaidMember"),
        ("eqoa.raid.RaidState", "ready_check") => Some("eqoa.raid.ReadyCheckSession"),
        ("eqoa.raid.RaidState", "loot_rolls") => Some("eqoa.raid.LootRollSession"),
        ("eqoa.raid.ReadyCheckSession", "data") => Some("eqoa.raid.ReadyCheckData"),
        ("eqoa.raid.LootRollSession", "data") => Some("eqoa.raid.LootRollData"),
        ("eqoa.raid.LootRollData", "rolls") => Some("eqoa.raid.LootRollItem"),
        ("eqoa.items.ContainerView", "tabs") => Some("eqoa.items.Tab"),
        ("eqoa.items.MerchantState", "options") => Some("eqoa.items.MerchantOption"),
        ("eqoa.items.Item", "stats") => Some("eqoa.items.ItemStat"),
        ("eqoa.hotbar.Hotbar", "slots") => Some("eqoa.hotbar.HotbarSlot"),
        ("eqoa.chat.ChatMessage", "color") => Some("eqoa.Color"),
        ("eqoa.chat.ChatMessage", "links") => Some("eqoa.chat.ChatLink"),
        ("eqoa.windows.Session", "data") => Some("eqoa.windows.SessionData"),
        ("eqoa.windows.SessionData", "entries") => Some("eqoa.windows.StartEntry"),
        ("eqoa.windows.StartEntry", "entries") => Some("eqoa.windows.StartEntry"),
        ("eqoa.windows.SessionData", "links") => Some("eqoa.windows.ChatLink"),
        ("eqoa.windows.SessionData", "offer") => Some("eqoa.windows.LinkOffer"),
        ("eqoa.chat.ChatTabs", "tabs") => Some("eqoa.chat.ChatTab"),
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
        ("eqoa.hud.Layout", "minimap") => Some("eqoa.hud.Rect"),
        ("eqoa.hud.Layout", "pet") => Some("eqoa.hud.SinglePanelLayout"),
        (
            "eqoa.hud.PlayerLayout"
            | "eqoa.hud.TargetLayout"
            | "eqoa.hud.GroupMemberLayout"
            | "eqoa.hud.SinglePanelLayout",
            "panel" | "health" | "power" | "exp",
        ) => Some("eqoa.hud.Rect"),
        ("eqoa.windows.Windows", "windows") => Some("eqoa.windows.Window"),
        ("eqoa.windows.Window", "rows") => Some("eqoa.windows.MenuRow"),
        ("eqoa.windows.Window", "hints") => Some("eqoa.windows.ButtonHint"),
        ("eqoa.windows.Window", "dialog") => Some("eqoa.windows.Dialog"),
        ("eqoa.craft.State", "skills") => Some("eqoa.craft.Skill"),
        ("eqoa.craft.State", "recipes") => Some("eqoa.craft.Recipe"),
        ("eqoa.craft.State", "detail") => Some("eqoa.craft.Detail"),
        ("eqoa.craft.State", "tray") => Some("eqoa.craft.TrayItem"),
        ("eqoa.craft.State", "items") => Some("eqoa.craft.Item"),
        ("eqoa.craft.State", "question") => Some("eqoa.craft.Question"),
        ("eqoa.craft.State", "notice") => Some("eqoa.craft.Notice"),
        ("eqoa.craft.Detail", "ingredients") => Some("eqoa.craft.Ingredient"),
        ("eqoa.craft.Detail", "tools") => Some("eqoa.craft.Tool"),
        ("eqoa.mail.State", "entries") => Some("eqoa.mail.Entry"),
        ("eqoa.mail.State", "message") => Some("eqoa.mail.Message"),
        ("eqoa.mail.State", "pack") => Some("eqoa.mail.PackItem"),
        ("eqoa.auction.State", "results") => Some("eqoa.auction.Lot"),
        ("eqoa.auction.State", "lots") => Some("eqoa.auction.Own"),
        ("eqoa.auction.State", "bids") => Some("eqoa.auction.Bid"),
        ("eqoa.auction.State", "bidders") => Some("eqoa.auction.Bidder"),
        ("eqoa.auction.State", "sellable") => Some("eqoa.auction.Sellable"),
        ("eqoa.guild.Guild", "members") => Some("eqoa.guild.GuildMember"),
        ("eqoa.who.WhoState", "entries") => Some("eqoa.who.WhoEntry"),
        ("eqoa.lfg.LfgState", "entries") => Some("eqoa.lfg.LfgEntry"),
        _ => None,
    }
}
fn install_fields(lua: &Lua) {
    for (path, fields) in [
        ("eqoa.Error", "code message capability retry_after"),
        (
            "eqoa.map.Marker",
            "kind name x z level_min level_max label step",
        ),
        (
            "eqoa.map.MapState",
            "enabled open minimap markers_on world_id world_name zone bounds revision waypoint search_anchor",
        ),
        ("eqoa.map.Bounds", "x_min z_min x_max z_max"),
        ("eqoa.map.Waypoint", "world_id x z label owner"),
        ("eqoa.input.Pointer", "x y inside_window"),
        // what `eqoa.options.button` returns (round 4; described in round 7H)
        ("eqoa.options.Button", "id label item_id"),
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
            "entity id name class_id class_name race_id race_name level experience experience_debt hp_fraction hp power attributes armor resists position velocity speed horizontal_speed heading target_id pet_id auto_attacking dead has_pet fighting location mastery",
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
            "ref id name level position actor_position hp_fraction power kind world_id entity_type is_creature corpse search_flags npc_flags class_code target_id status_flags poisoned diseased disposition disposition_name head_mark merc_tag",
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
            "ref id name description display_key spellbook_slot usable learned level range cast_time power_cost icon_background icon_foreground scope cooldown equipment_mask lockout",
        ),
        ("eqoa.abilities.RangeResult", "in_range distance"),
        (
            "eqoa.hotbar.Hotbar",
            "index kind selected selected_slot slots",
        ),
        (
            "eqoa.hotbar.HotbarSlot",
            "index kind ability item weapon_set command icon ui_icon",
        ),
        (
            "eqoa.cooldowns.Cooldown",
            "active lockout started_at estimated_remaining",
        ),
        ("eqoa.effects.Effect", "index icon name"),
        (
            "eqoa.chat.ChatMessage",
            "seq time text color channel channel_code sender outgoing kind links",
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
        ("eqoa.hud.Layout", "player group_members target pet minimap"),
        ("eqoa.hud.PlayerLayout", "panel health power exp"),
        ("eqoa.hud.TargetLayout", "panel health power"),
        ("eqoa.hud.GroupMemberLayout", "index panel health power"),
        ("eqoa.hud.SinglePanelLayout", "panel"),
        ("eqoa.options.Toggle", "id label default value item_id"),
    ] {
        let round = if path.starts_with("eqoa.map.") || matches!(path, "eqoa.input.Pointer" | "eqoa.options.Button") {
            Some(4)
        } else if path.starts_with("eqoa.hud.")
            || path.starts_with("eqoa.options.")
            || matches!(path, "eqoa.player.Stats" | "eqoa.player.StatItem")
        {
            Some(3)
        } else {
            Some(2)
        };
        describe(lua, path, "supported", round, "");
        for field in fields.split_whitespace() {
            if let Some(reason) = unprojected_field(path, field) {
                describe(lua, &format!("{path}.{field}"), "unsupported", None, reason);
                continue;
            }
            let field_round = if matches!(
                (path, field),
                ("eqoa.entities.Entity", "actor_position") | ("eqoa.map.MapState", "search_anchor")
            ) {
                Some(4)
            } else if matches!(
                (path, field),
                ("eqoa.entities.Entity", "npc_flags") | ("eqoa.player.Player", "mastery")
            ) {
                Some(6)
            } else if (path, field) == ("eqoa.hotbar.HotbarSlot", "item") {
                Some(5)
            } else if matches!((path, field), ("eqoa.entities.Entity", "head_mark" | "merc_tag")) {
                Some(8)
            } else if (path, field) == ("eqoa.abilities.Ability", "learned") {
                Some(8)
            } else if matches!(
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
}

pub(super) fn unprojected_field(path: &str, field: &str) -> Option<&'static str> {
    match (path, field) {
        ("eqoa.Error", "retry_after") => {
            Some("no refusal in this client knows how long to wait: rate_limited and quota carry no wait")
        }
        (
            "eqoa.actions.BatchResult" | "eqoa.events.batch.progress.data" | "eqoa.events.batch.result.data",
            "detail",
        ) => Some("a batch reports what each step did in that step's action.result"),
        ("eqoa.chat.ChatMessage" | "eqoa.events.chat.message.data", "channel_code" | "sender" | "outgoing") => {
            Some("the chat observer retains only text, color and the channel name")
        }
        ("eqoa.hotbar.HotbarSlot", "command") => Some("native hotbar rows do not expose command text"),
        ("eqoa.hotbar.HotbarSlot", "icon") => Some("use ui_icon or the referenced ability or item icon"),
        _ => None,
    }
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
