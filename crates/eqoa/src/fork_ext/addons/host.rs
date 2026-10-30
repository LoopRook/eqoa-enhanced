use super::discovery::DiscoveredAddon;
use mlua::chunk::ChunkMode;
use mlua::{Function, HookTriggers, Lua, Result, StdLib, Table, Value, VmState};
use std::collections::HashMap;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct Snapshot {
    pub in_game: bool,
    pub start_menu: bool,
    pub health: super::health::HealthSnapshot,
    pub player: super::player::PlayerSnapshot,
    pub group: super::group::GroupSnapshot,
    pub entities: super::entity::EntitiesSnapshot,
}
impl Snapshot {
    pub fn managed(mut self) -> Self {
        if !self.in_game {
            self.health = super::health::HealthSnapshot::unavailable();
            self.player = super::player::PlayerSnapshot::default();
            self.group = super::group::GroupSnapshot::default();
            self.entities = super::entity::EntitiesSnapshot::default();
        }
        self.in_game &= self.health.is_available();
        self
    }
}

pub(crate) struct Host {
    pub lua: Lua,
    pub snapshot: Rc<Cell<Snapshot>>,
    remaining: Rc<Cell<u32>>,
    fatal: Rc<Cell<bool>>,
    addon: Option<DiscoveredAddon>,
    host_registry: Rc<RefCell<HashMap<String, Value>>>,
}

struct BaseHost {
    lua: Lua,
    snapshot: Rc<Cell<Snapshot>>,
    util: Table,
    bit: Table,
    clean_os: Table,
    remaining: Rc<Cell<u32>>,
    fatal: Rc<Cell<bool>>,
}

fn create_base_host() -> Result<BaseHost> {
    let lua = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::PACKAGE | StdLib::OS,
        Default::default(),
    )?;
    lua.set_memory_limit(8 * 1024 * 1024)?;

    // Extract original pcall before modifying globals
    let orig_pcall: Function = lua.globals().get("pcall")?;

    // Deliberately constrained spike: strip dangerous functions and coroutine
    for key in [
        "dofile",
        "loadfile",
        "load",
        "loadstring",
        "xpcall",
        "collectgarbage",
        "coroutine",
    ] {
        lua.globals().set(key, Value::Nil)?;
    }
    let package: Table = lua.globals().get("package")?;
    package.set("loadlib", Value::Nil)?;
    package.set("loaders", lua.create_table()?)?;
    let package_loaded: Table = package.get("loaded")?;
    package_loaded.set("coroutine", Value::Nil)?;

    // Extract clean os whitelist (clock, date) and remove dangerous functions
    let os_orig: Table = lua.globals().get("os")?;
    let clock_fn: Function = os_orig.get("clock")?;
    let date_fn: Function = os_orig.get("date")?;
    let clean_os = lua.create_table()?;
    clean_os.set("clock", clock_fn)?;
    clean_os.set("date", date_fn)?;
    lua.globals().set("os", clean_os.clone())?;
    package_loaded.set("os", clean_os.clone())?;

    // Native bit module table
    let bit_table = super::bit::create_module(&lua)?;
    package_loaded.set("bit", bit_table.clone())?;

    let snapshot = Rc::new(Cell::new(Snapshot::default()));
    let util = lua.create_table()?;
    let state = snapshot.clone();
    util.set(
        "IsInGame",
        lua.create_function(move |_, ()| Ok(i32::from(state.get().in_game)))?,
    )?;
    let state = snapshot.clone();
    util.set(
        "IsStartMenuOpen",
        lua.create_function(move |_, ()| Ok(i32::from(state.get().start_menu)))?,
    )?;

    lua.set_named_registry_value("forge_callbacks", lua.create_table()?)?;
    let forge = lua.create_table()?;
    let types = lua.create_table()?;
    for (i, name) in ["Settings", "DisableScript", "Save", "Load", "OnEject"]
        .iter()
        .enumerate()
    {
        types.set(*name, i)?;
    }
    forge.set("CallbackType", types)?;
    forge.set(
        "RegisterCallback",
        lua.create_function(|lua, (kind, callback): (i32, Function)| {
            if !(0..=4).contains(&kind) {
                return Err(mlua::Error::runtime("unknown callback type"));
            }
            lua.named_registry_value::<Table>("forge_callbacks")?
                .set(kind + 1, callback)
        })?,
    )?;
    lua.globals().set("UiForge", forge)?;

    let remaining = Rc::new(Cell::new(0u32));
    let fatal = Rc::new(Cell::new(false));
    let budget = remaining.clone();
    let fatal_hook = fatal.clone();
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            let next = budget.get().checked_sub(1000).ok_or_else(|| {
                fatal_hook.set(true);
                mlua::Error::runtime("Forge instruction budget exceeded")
            })?;
            budget.set(next);
            Ok(VmState::Continue)
        },
    )?;

    // Install protected pcall wrapper that rethrows if the instruction budget was exceeded
    let fatal_pcall = fatal.clone();
    let pcall_fn = lua.create_function(move |_, args: mlua::MultiValue| {
        let call_res = orig_pcall.call::<mlua::MultiValue>(args);
        if fatal_pcall.get() {
            return Err(mlua::Error::runtime("Forge instruction budget exceeded"));
        }
        call_res
    })?;
    lua.globals().set("pcall", pcall_fn)?;

    Ok(BaseHost {
        lua,
        snapshot,
        util,
        bit: bit_table,
        clean_os,
        remaining,
        fatal,
    })
}

impl Host {
    pub fn for_addon(addon: &DiscoveredAddon) -> Result<Self> {
        let base = create_base_host()?;
        let host_registry = Rc::new(RefCell::new(HashMap::new()));
        super::package::install_package_loader(
            &base.lua,
            addon,
            base.util,
            base.bit,
            base.clean_os,
            host_registry.clone(),
        )?;

        let host = Self {
            lua: base.lua,
            snapshot: base.snapshot,
            remaining: base.remaining,
            fatal: base.fatal,
            addon: Some(addon.clone()),
            host_registry,
        };
        super::player::install_player_adapter(&host)?;
        super::visibility::install_standalone_ui_adapter(&host)?;
        super::group::install_group_adapter(&host)?;
        super::entity::install_entity_adapter(&host)?;
        super::entity::install_entity_list_adapter(&host)?;
        Ok(host)
    }

    pub fn new() -> Result<Self> {
        let base = create_base_host()?;
        let util_t = base.util.clone();
        let bit_t = base.bit.clone();
        let os_t = base.clean_os.clone();
        base.lua.globals().set(
            "require",
            base.lua.create_function(move |_, name: String| {
                if name == "frontiers_forge.util" {
                    Ok(Value::Table(util_t.clone()))
                } else if name == "bit" {
                    Ok(Value::Table(bit_t.clone()))
                } else if name == "os" {
                    Ok(Value::Table(os_t.clone()))
                } else if name == "coroutine" {
                    Err(mlua::Error::runtime("coroutine module is unavailable"))
                } else {
                    Err(mlua::Error::runtime(format!(
                        "unsupported spike module: {name}"
                    )))
                }
            })?,
        )?;

        Ok(Self {
            lua: base.lua,
            snapshot: base.snapshot,
            remaining: base.remaining,
            fatal: base.fatal,
            addon: None,
            host_registry: Rc::new(RefCell::new(HashMap::new())),
        })
    }

    pub fn compile_addon_entry(&self, addon: &DiscoveredAddon) -> Result<Function> {
        let canon = addon.entry.canonicalize().map_err(|e| {
            mlua::Error::runtime(format!(
                "failed to canonicalize entry '{}': {e}",
                addon.entry.display()
            ))
        })?;
        if !canon.starts_with(&addon.script_root) {
            return Err(mlua::Error::runtime(format!(
                "addon entry '{}' escapes active script root",
                canon.display()
            )));
        }
        let source = super::package::read_bounded_source(&canon, 1024 * 1024)
            .map_err(mlua::Error::runtime)?;
        self.lua
            .load(&source)
            .set_name(canon.to_string_lossy().to_string())
            .set_mode(ChunkMode::Text)
            .into_function()
    }

    pub fn run_addon(&self, addon: &DiscoveredAddon) -> Result<()> {
        let func = self.compile_addon_entry(addon)?;
        self.run(&func)
    }

    pub fn register_host_module(&self, name: impl Into<String>, value: Value) -> Result<()> {
        self.host_registry.borrow_mut().insert(name.into(), value);
        Ok(())
    }

    pub fn has_host_module(&self, name: &str) -> bool {
        self.host_registry.borrow().contains_key(name)
    }

    pub fn resolve_resource(
        &self,
        relative_path: &str,
    ) -> std::result::Result<PathBuf, super::package::ResourceError> {
        match &self.addon {
            Some(a) => super::package::resolve_resource(a, relative_path),
            None => Err(super::package::ResourceError::NotFound(
                "host was not initialized for an addon".to_string(),
            )),
        }
    }

    pub fn compile(&self, source: &str) -> Result<Function> {
        if source.len() > 1024 * 1024 {
            return Err(mlua::Error::runtime("spike source exceeds 1 MiB"));
        }
        self.lua
            .load(source)
            .set_name("Forge spike")
            .set_mode(ChunkMode::Text)
            .into_function()
    }

    pub fn run(&self, function: &Function) -> Result<()> {
        self.remaining.set(200_000);
        self.fatal.set(false);
        let res = function.call(());
        if self.fatal.get() {
            return Err(mlua::Error::runtime("Forge instruction budget exceeded"));
        }
        res
    }

    #[cfg(test)]
    pub(crate) fn execute(&self, source: &str) -> Result<()> {
        self.run(&self.compile(source)?)
    }

    pub fn callback(&self, kind: i32, value: Value) -> Result<Value> {
        if !(0..=4).contains(&kind) {
            return Err(mlua::Error::runtime("unknown callback type"));
        }
        self.remaining.set(200_000);
        self.fatal.set(false);
        let res = match self
            .lua
            .named_registry_value::<Table>("forge_callbacks")?
            .get::<Option<Function>>(kind + 1)?
        {
            Some(callback) => callback.call(value),
            None => Ok(Value::Nil),
        };
        if self.fatal.get() {
            return Err(mlua::Error::runtime("Forge instruction budget exceeded"));
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_host_keeps_globals_and_replaces_callbacks() {
        let host = Host::new().unwrap();
        host.execute("count = (count or 0) + 1; UiForge.RegisterCallback(UiForge.CallbackType.Save, function() return 'old' end); UiForge.RegisterCallback(UiForge.CallbackType.Save, function() return count end)").unwrap();
        host.execute("count = count + 1").unwrap();
        assert_eq!(host.callback(2, Value::Nil).unwrap().as_integer(), Some(2));
        assert!(host.callback(99, Value::Nil).is_err());
    }

    #[test]
    fn forge_host_stops_runaway_code_without_poisoning_next_call() {
        let host = Host::new().unwrap();
        let error = host.execute("while true do end").unwrap_err().to_string();
        assert!(error.contains("instruction budget"), "{error}");
        host.execute("answer = 42").unwrap();
        assert_eq!(host.lua.globals().get::<i32>("answer").unwrap(), 42);
    }

    #[test]
    fn forge_host_limits_memory_and_rejects_native_modules() {
        let host = Host::new().unwrap();
        assert!(
            host.execute("x = string.rep('x', 32 * 1024 * 1024)")
                .is_err()
        );
        host.execute("assert(package.loadlib == nil); assert(require('frontiers_forge.util').IsInGame() == 0)").unwrap();
        assert!(host.execute("require('ffi')").is_err());
    }
    #[test]
    fn forge_host_pcall_catches_ordinary_errors_and_preserves_multireturns_and_identity() {
        let host = Host::new().unwrap();
        // 1. Success with arbitrary multiple returns including nil
        host.execute(
            r#"
            local ok, a, b, c, d = pcall(function()
                return 1, nil, "three", { x = 4 }
            end)
            assert(ok == true)
            assert(a == 1)
            assert(b == nil)
            assert(c == "three")
            assert(type(d) == "table" and d.x == 4)
            "#,
        )
        .unwrap();

        // 2. Failure with error table identity preserved
        host.execute(
            r#"
            local err_obj = { code = 404, message = "not found" }
            local ok, err = pcall(function()
                error(err_obj)
            end)
            assert(ok == false)
            assert(err == err_obj, "error object identity must be preserved")
            assert(err.code == 404)
            "#,
        )
        .unwrap();

        // 3. Nested pcall
        host.execute(
            r#"
            local ok_out, res_out = pcall(function()
                local ok_in, err_in = pcall(function()
                    error("inner failure")
                end)
                assert(ok_in == false)
                assert(string.find(err_in, "inner failure"))
                return "outer success"
            end)
            assert(ok_out == true)
            assert(res_out == "outer success")
            "#,
        )
        .unwrap();
    }

    #[test]
    fn forge_host_pcall_cannot_swallow_budget_exhaustion_finite_control_and_recovers() {
        let host = Host::new().unwrap();
        // Finite negative control: 1,000,000 iterations exceeds 200,000 budget
        // Outer marker is placed AFTER pcall: if pcall swallowed error, outer_marker would become true.
        let source = r#"
            outer_marker = false
            inner_marker = false
            pcall(function()
                pcall(function()
                    for i = 1, 1000000 do
                    end
                    inner_marker = true
                end)
                outer_marker = true
            end)
        "#;
        let err = host.execute(source).unwrap_err().to_string();
        assert!(
            err.contains("instruction budget exceeded"),
            "expected budget error, got: {err}"
        );
        // Neither inner nor outer completion markers must ever be reached
        assert_eq!(
            host.lua
                .globals()
                .get::<Option<bool>>("inner_marker")
                .unwrap(),
            Some(false)
        );
        assert_eq!(
            host.lua
                .globals()
                .get::<Option<bool>>("outer_marker")
                .unwrap(),
            Some(false)
        );

        // Next independent host call must cleanly recover with replenished budget
        host.execute("recovered = 42").unwrap();
        assert_eq!(host.lua.globals().get::<i32>("recovered").unwrap(), 42);
    }

    #[test]
    fn forge_host_clean_os_clock_date_and_blocks_unsafe_functions() {
        let host = Host::new().unwrap();
        // os.clock returns non-negative number
        host.execute(
            r#"
            local t = os.clock()
            assert(type(t) == "number" and t >= 0)
            "#,
        )
        .unwrap();

        // os.date format and UTC table
        host.execute(
            r#"
            local year_str = os.date("%Y")
            assert(type(year_str) == "string" and string.len(year_str) == 4)
            local utc = os.date("!*t")
            assert(type(utc) == "table")
            assert(type(utc.year) == "number" and utc.year >= 2024)
            assert(type(utc.month) == "number" and utc.month >= 1 and utc.month <= 12)
            assert(type(utc.day) == "number" and utc.day >= 1 and utc.day <= 31)
            "#,
        )
        .unwrap();

        // Dangerous os functions must be nil in both _G.os and require("os")
        host.execute(
            r#"
            assert(os.execute == nil)
            assert(os.exit == nil)
            assert(os.remove == nil)
            assert(os.rename == nil)
            assert(os.getenv == nil)
            assert(os.tmpname == nil)

            local req_os = require("os")
            assert(req_os.execute == nil)
            assert(req_os.exit == nil)
            assert(req_os.remove == nil)
            assert(req_os.rename == nil)
            assert(req_os.getenv == nil)
            "#,
        )
        .unwrap();
    }

    #[test]
    fn forge_host_coroutine_and_dangerous_libraries_strictly_absent() {
        let host = Host::new().unwrap();
        // coroutine is nil in _G and cannot be required
        host.execute(
            r#"
            assert(coroutine == nil)
            assert(xpcall == nil)
            assert(loadfile == nil)
            assert(dofile == nil)
            assert(load == nil)
            assert(loadstring == nil)
            assert(collectgarbage == nil)
            assert(package.loadlib == nil)
            "#,
        )
        .unwrap();

        assert!(host.execute("require('coroutine')").is_err());
        assert!(host.execute("require('io')").is_err());
        assert!(host.execute("require('ffi')").is_err());
    }

    #[test]
    fn forge_host_callback_top_boundary_fatal_reset_and_recovery() {
        let host = Host::new().unwrap();
        // Register a callback that trips the instruction budget
        host.execute(
            r#"
            UiForge.RegisterCallback(UiForge.CallbackType.Save, function()
                for i = 1, 1000000 do end
                return "finished"
            end)
        "#,
        )
        .unwrap();

        let err = host.callback(2, Value::Nil).unwrap_err().to_string();
        assert!(
            err.contains("instruction budget exceeded"),
            "expected budget error, got: {err}"
        );

        // Next callback or run call resets fatal flag and budget at fresh top-level boundary
        host.execute(
            r#"
            UiForge.RegisterCallback(UiForge.CallbackType.Save, function()
                return "recovered"
            end)
        "#,
        )
        .unwrap();
        let res = host.callback(2, Value::Nil).unwrap();
        assert!(matches!(res, Value::String(ref s) if s.to_str().unwrap() == "recovered"));
    }
}
