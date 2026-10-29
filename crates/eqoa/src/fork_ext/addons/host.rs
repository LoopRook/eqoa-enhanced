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
}

pub(crate) struct Host {
    pub lua: Lua,
    pub snapshot: Rc<Cell<Snapshot>>,
    remaining: Rc<Cell<u32>>,
    addon: Option<DiscoveredAddon>,
    host_registry: Rc<RefCell<HashMap<String, Value>>>,
}

fn create_base_host() -> Result<(Lua, Rc<Cell<Snapshot>>, Table, Rc<Cell<u32>>)> {
    let lua = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::PACKAGE,
        Default::default(),
    )?;
    lua.set_memory_limit(8 * 1024 * 1024)?;
    // Deliberately constrained spike: protected calls/coroutines could catch a
    // budget error and continue forever. Broader Lua compatibility is a later gate.
    for key in [
        "dofile",
        "loadfile",
        "load",
        "loadstring",
        "pcall",
        "xpcall",
        "collectgarbage",
    ] {
        lua.globals().set(key, Value::Nil)?;
    }
    let package: Table = lua.globals().get("package")?;
    package.set("loadlib", Value::Nil)?;
    package.set("loaders", lua.create_table()?)?;

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
    let budget = remaining.clone();
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            let next = budget
                .get()
                .checked_sub(1000)
                .ok_or_else(|| mlua::Error::runtime("Forge instruction budget exceeded"))?;
            budget.set(next);
            Ok(VmState::Continue)
        },
    )?;

    Ok((lua, snapshot, util, remaining))
}

impl Host {
    pub fn for_addon(addon: &DiscoveredAddon) -> Result<Self> {
        let (lua, snapshot, util, remaining) = create_base_host()?;
        let host_registry = Rc::new(RefCell::new(HashMap::new()));
        super::package::install_package_loader(&lua, addon, util, host_registry.clone())?;

        Ok(Self {
            lua,
            snapshot,
            remaining,
            addon: Some(addon.clone()),
            host_registry,
        })
    }

    pub fn new() -> Result<Self> {
        let (lua, snapshot, util, remaining) = create_base_host()?;
        lua.globals().set(
            "require",
            lua.create_function(move |_, name: String| {
                if name == "frontiers_forge.util" {
                    Ok(util.clone())
                } else {
                    Err(mlua::Error::runtime(format!(
                        "unsupported spike module: {name}"
                    )))
                }
            })?,
        )?;

        Ok(Self {
            lua,
            snapshot,
            remaining,
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
        function.call(())
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
        match self
            .lua
            .named_registry_value::<Table>("forge_callbacks")?
            .get::<Option<Function>>(kind + 1)?
        {
            Some(callback) => callback.call(value),
            None => Ok(Value::Nil),
        }
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
}
