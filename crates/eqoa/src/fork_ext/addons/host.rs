// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
pub const INSTRUCTION_BUDGET: u32 = 4_000_000;

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
    pub character: Option<super::named_profiles::CharacterSnapshot>,
    pub in_game: bool,
    pub start_menu: bool,
    pub ui_mode: u32,
    pub ui_busy: bool,
    pub health: super::health::HealthSnapshot,
    pub player: super::player::PlayerSnapshot,
    pub group: super::group::GroupSnapshot,
    pub entities: super::entity::EntitiesSnapshot,
    pub abilities: super::ability::AbilitiesSnapshot,
    pub chat: super::chat::ChatSnapshot,
    pub combat: super::combat::CombatSnapshot,
    pub input: super::input::InputSnapshot,
    pub effects: super::effects::EffectsSnapshot,
    pub journal: super::quest_log::JournalSnapshot,
    pub weapon_sets: super::weapon_sets::WeaponSetsSnapshot,
    pub chat_history: crate::fork_ext::chat_history::ChatHistorySnapshot,
}
impl Snapshot {
    pub fn managed(mut self) -> Self {
        if !self.in_game {
            self.health = super::health::HealthSnapshot::unavailable();
            self.player = super::player::PlayerSnapshot::default();
            self.group = super::group::GroupSnapshot::default();
            self.entities = super::entity::EntitiesSnapshot::default();
            self.abilities = super::ability::AbilitiesSnapshot::default();
            self.chat = super::chat::ChatSnapshot::default();
            self.combat = super::combat::CombatSnapshot::default();
            self.input = super::input::InputSnapshot::default();
            self.effects = super::effects::EffectsSnapshot::default();
            self.journal = super::quest_log::JournalSnapshot::default();
            self.weapon_sets = super::weapon_sets::WeaponSetsSnapshot::default();
            self.chat_history = crate::fork_ext::chat_history::snapshot(false);
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
    iteration_active: Cell<bool>,
    addon: Option<DiscoveredAddon>,
    host_registry: Rc<RefCell<HashMap<String, Value>>>,
}

impl Drop for Host {
    fn drop(&mut self) {
        super::native_events::clear(self);
        for error in super::file_io::close_all(&self.lua) {
            eprintln!("{error}");
        }
    }
}

struct BaseHost {
    lua: Lua,
    snapshot: Rc<Cell<Snapshot>>,
    util: Table,
    window: Table,
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
    // Deliberate divergence: Util.IsStartMenuOpen and Window.IsStartMenuOpen return
    // true for ANY game window (shops, bank, trade, loot, mail, spellbook, etc.),
    // not only the pause menu's pages, because shipped packages use it to hide over game windows.
    util.set(
        "IsStartMenuOpen",
        lua.create_function(move |_, ()| Ok(i32::from(state.get().start_menu)))?,
    )?;
    let state = snapshot.clone();
    util.set(
        "IsUiBusy",
        lua.create_function(move |_, ()| Ok(state.get().ui_busy))?,
    )?;
    let state = snapshot.clone();
    util.set(
        "GetUiMode",
        lua.create_function(move |_, ()| Ok(state.get().ui_mode))?,
    )?;

    let ui_mode = lua.create_table()?;
    ui_mode.set("normal", 0x00)?;
    ui_mode.set("loot", 0x01)?;
    ui_mode.set("input", 0x02)?;
    ui_mode.set("text_dialog", 0x09)?;
    ui_mode.set("merchant", 0x0A)?;
    ui_mode.set("trainer", 0x0C)?;
    ui_mode.set("start_menu", 0x0D)?;
    ui_mode.set("confirm", 0x0E)?;
    ui_mode.set("context_menu", 0x1E)?;
    ui_mode.set("r2_chat_window", 0x21)?;
    util.set("UiMode", ui_mode.clone())?;

    let window_mod = lua.create_table()?;
    let state = snapshot.clone();
    window_mod.set(
        "IsStartMenuOpen",
        lua.create_function(move |_, ()| Ok(state.get().start_menu))?,
    )?;
    let state = snapshot.clone();
    window_mod.set(
        "GetMode",
        lua.create_function(move |_, ()| Ok(state.get().ui_mode))?,
    )?;
    let state = snapshot.clone();
    window_mod.set(
        "IsBusy",
        lua.create_function(move |_, ()| Ok(state.get().ui_busy))?,
    )?;
    window_mod.set("Mode", ui_mode)?;
    package_loaded.set("frontiers_forge.window", window_mod.clone())?;
    let state = snapshot.clone();
    util.set(
        "GetCompassDegrees",
        lua.create_function(move |_, ()| {
            let deg = (state.get().player.heading_rad.to_degrees() as f64).rem_euclid(360.0);
            Ok(deg)
        })?,
    )?;
    util.set(
        "GetBearingToPoint",
        lua.create_function(|_, (from, to): (Table, Table)| {
            let from_x: f64 = from.get("x").unwrap_or(0.0);
            let from_z: f64 = from.get("z").unwrap_or(0.0);
            let to_x: f64 = to.get("x").unwrap_or(0.0);
            let to_z: f64 = to.get("z").unwrap_or(0.0);
            let dx = to_x - from_x;
            let dz = to_z - from_z;
            let deg = (-dx).atan2(-dz).to_degrees().rem_euclid(360.0);
            Ok(deg)
        })?,
    )?;
    util.set(
        "GetExpRequiredForLevel",
        lua.create_function(|_, level: i32| {
            if (1..=59).contains(&level) {
                // Pinned Forge util.lua:59-73 uses the original 28/25
                // table through 59; the native HUD changes its formula at 51.
                let next = (level + 1) as u32;
                Ok(Some(next.pow(3) * (level as u32 * 28 + 25)))
            } else {
                Ok(None)
            }
        })?,
    )?;
    util.set(
        "IsBattleMusicPlaying",
        lua.create_function(|_, ()| {
            Ok(super::native_core::CORE.with(|c| {
                c.borrow().view.player["fighting"]
                    .as_bool()
                    .unwrap_or(false)
            }))
        })?,
    )?;
    util.set(
        "IsBitZero",
        lua.create_function(|_, (value, mask): (u32, u32)| Ok(value & mask == 0))?,
    )?;
    for name in ["utf8_to_utf16", "utf16_to_utf8"] {
        util.set(name,lua.create_function(move |lua,_:Value|{let reason="unsupported: pointer-based UTF conversion is unavailable; native API strings are UTF-8";super::native::diagnose(lua,&format!("frontiers_forge.util.{name}"),"unsupported",reason);Ok((Value::Nil,reason))})?)?;
    }

    lua.set_named_registry_value("forge_callbacks", lua.create_table()?)?;
    let forge = lua.create_table()?;
    lua.set_named_registry_value("forge_log", lua.create_table()?)?;
    forge.set(
        "Log",
        lua.create_function(|lua, (level, message): (i32, String)| {
            let messages: Table = lua.named_registry_value("forge_log")?;
            if messages.raw_len() < 64 {
                let bounded: String = message.chars().take(4096).collect();
                messages.raw_push(format!("Lua log (level {level}): {bounded}"))?;
            }
            Ok(())
        })?,
    )?;
    util.set(
        "LogError",
        lua.create_function(|lua, message: String| {
            let forge: Table = lua.globals().get("UiForge")?;
            if let Some(log) = forge.get::<Option<Function>>("Log")? {
                log.call::<()>((2, message))?;
            }
            Ok(())
        })?,
    )?;
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
                mlua::Error::runtime(format!(
                    "Forge instruction budget exceeded: {} instructions (limit: {})",
                    INSTRUCTION_BUDGET, INSTRUCTION_BUDGET
                ))
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
            return Err(mlua::Error::runtime(format!(
                "Forge instruction budget exceeded: {} instructions (limit: {})",
                INSTRUCTION_BUDGET, INSTRUCTION_BUDGET
            )));
        }
        call_res
    })?;
    lua.globals().set("pcall", pcall_fn)?;

    Ok(BaseHost {
        lua,
        snapshot,
        util,
        window: window_mod,
        bit: bit_table,
        clean_os,
        remaining,
        fatal,
    })
}

impl Host {
    pub(crate) fn addon_id(&self) -> String {
        self.addon
            .as_ref()
            .map_or("test", |a| a.id.as_str())
            .to_string()
    }
    pub(crate) fn once_mode(&self) -> bool {
        self.addon
            .as_ref()
            .and_then(|a| std::fs::read(a.package_root.join("mod.json")).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .is_some_and(|v| v["entryMode"] == "once")
    }
    pub fn drain_logs(&self) -> Result<Vec<String>> {
        let messages: Table = self.lua.named_registry_value("forge_log")?;
        let mut output = messages
            .sequence_values::<String>()
            .collect::<Result<Vec<_>>>()?;
        self.lua
            .set_named_registry_value("forge_log", self.lua.create_table()?)?;
        output.extend(super::file_io::drain_diagnostics(&self.lua));
        output.extend(super::native::drain(&self.lua));
        Ok(output)
    }
    pub const INSTRUCTION_BUDGET: u32 = INSTRUCTION_BUDGET;

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
            iteration_active: Cell::new(false),
            addon: Some(addon.clone()),
            host_registry,
        };
        host.register_host_module("frontiers_forge.window", Value::Table(base.window))?;
        super::player::install_player_adapter(&host)?;
        super::visibility::install_standalone_ui_adapter(&host)?;
        super::group::install_group_adapter(&host)?;
        super::entity::install_entity_adapter(&host)?;
        super::entity::install_entity_list_adapter(&host)?;
        super::ability::install_ability_adapter(&host)?;
        super::ability::install_ability_list_adapter(&host)?;
        super::ability::install_ability_bar_adapter(&host)?;
        super::ability::install_icon_adapter(&host)?;
        super::chat::install_chat_adapter(&host)?;
        super::combat::install_combat_adapter(&host)?;
        super::input::install_input_adapter(&host)?;
        super::effects::install_effects_adapter(&host)?;
        super::quest_log::install(&host)?;
        super::weapon_sets::install(&host)?;
        super::chat_history::install(&host)?;
        let forge: Table = host.lua.globals().get("UiForge")?;
        forge.set(
            "scripts_path",
            addon.script_root.to_string_lossy().replace("\\\\?\\", ""),
        )?;
        forge.set(
            "resources_path",
            addon
                .script_root
                .join("resources")
                .to_string_lossy()
                .replace("\\\\?\\", ""),
        )?;
        super::file_io::install(&host, addon, None)?;
        super::native::install(&host)?;
        Ok(host)
    }

    pub fn new() -> Result<Self> {
        let base = create_base_host()?;
        let util_t = base.util.clone();
        let window_t = base.window.clone();
        let bit_t = base.bit.clone();
        let os_t = base.clean_os.clone();
        base.lua.globals().set(
            "require",
            base.lua.create_function(move |lua, name: String| {
                if name == "frontiers_forge.util" {
                    Ok(Value::Table(util_t.clone()))
                } else if name == "frontiers_forge.window" {
                    Ok(Value::Table(window_t.clone()))
                } else if name == "bit" {
                    Ok(Value::Table(bit_t.clone()))
                } else if name == "os" {
                    Ok(Value::Table(os_t.clone()))
                } else if name == "frontiers_forge.icon" {
                    if let Ok(icon_t) = lua.named_registry_value::<mlua::Table>("__forge_icon_mod")
                    {
                        Ok(Value::Table(icon_t))
                    } else {
                        Err(mlua::Error::runtime("frontiers_forge.icon not attached"))
                    }
                } else if name == "coroutine" {
                    Err(mlua::Error::runtime("coroutine module is unavailable"))
                } else {
                    Err(mlua::Error::runtime(format!(
                        "unsupported spike module: {name}"
                    )))
                }
            })?,
        )?;

        let host = Self {
            lua: base.lua,
            snapshot: base.snapshot,
            remaining: base.remaining,
            fatal: base.fatal,
            iteration_active: Cell::new(false),
            addon: None,
            host_registry: Rc::new(RefCell::new(HashMap::new())),
        };
        super::weapon_sets::install(&host)?;
        super::chat_history::install(&host)?;
        super::native::install(&host)?;
        Ok(host)
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
    pub(crate) fn registered_modules(&self) -> HashMap<String, Value> {
        self.host_registry.borrow().clone()
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

    pub fn instructions_remaining(&self) -> u32 {
        self.remaining.get()
    }

    pub fn run(&self, function: &Function) -> Result<()> {
        self.run_with_budget(function, Self::INSTRUCTION_BUDGET)
    }
    pub(crate) fn begin_iteration(&self) {
        if !self.iteration_active.replace(true) {
            self.remaining.set(Self::INSTRUCTION_BUDGET);
            self.fatal.set(false);
        }
    }
    pub(crate) fn end_iteration(&self) {
        self.iteration_active.set(false);
    }
    pub(crate) fn frame_entry(&self, function: &Function) -> Result<()> {
        self.begin_iteration();
        let bus = super::native_events::bus(&self.lua);
        let initialized = bus.borrow().initialized;
        let once = bus.borrow().once_mode;
        if initialized {
            super::native_events::dispatch(self)?;
        }
        if !once || !initialized {
            let _phase = super::native::phase(&self.lua, "entry");
            let _drawing = super::bridge::drawing(!once);
            function.call::<()>(())?;
        }
        bus.borrow_mut().initialized = true;
        Ok(())
    }
    pub(crate) fn finish_frame(&self, first: bool) -> Result<()> {
        if first {
            super::native_events::lifecycle(self, "enabled")?;
            super::native_events::initial(self)?;
        }
        super::native_events::render(self)?;
        if self.fatal.get() {
            return Err(mlua::Error::runtime(
                "eqoa iteration instruction budget exceeded",
            ));
        }
        Ok(())
    }

    pub fn run_with_budget(&self, function: &Function, budget: u32) -> Result<()> {
        self.remaining.set(budget);
        self.fatal.set(false);
        let res = function.call(());
        if self.fatal.get() {
            return Err(mlua::Error::runtime(format!(
                "Forge instruction budget exceeded: {} instructions (limit: {})",
                budget, budget
            )));
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
        if !self.iteration_active.get() {
            self.remaining.set(Self::INSTRUCTION_BUDGET);
            self.fatal.set(false);
        }
        let _phase = super::native::phase(&self.lua, &format!("callback.{kind}"));
        if let Some(result) = super::native_settings::callback(&self.lua, kind, &value) {
            return result;
        }
        if kind == 0 {
            super::native_events::settings_render(self)?;
        }
        let res = match self
            .lua
            .named_registry_value::<Table>("forge_callbacks")?
            .get::<Option<Function>>(kind + 1)?
        {
            Some(callback) => callback.call(value),
            None => Ok(Value::Nil),
        };
        if self.fatal.get() {
            return Err(mlua::Error::runtime(format!(
                "Forge instruction budget exceeded: {} instructions (limit: {})",
                Self::INSTRUCTION_BUDGET,
                Self::INSTRUCTION_BUDGET
            )));
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
                    for i = 1, 10000000 do
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
                for i = 1, 10000000 do end
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
    #[test]
    fn forge_host_util_and_window_menu_semantics() {
        let host = Host::new().unwrap();
        // 1. Initial / Normal gameplay: start menu closed, ui_mode 0, not busy
        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: false,
            ui_mode: 0,
            ui_busy: false,
            ..Default::default()
        });

        host.execute(
            r#"
            local Util = require("frontiers_forge.util")
            local Window = require("frontiers_forge.window")

            assert(Util.IsStartMenuOpen() == 0)
            assert(Window.IsStartMenuOpen() == false)
            assert(Util.GetUiMode() == 0)
            assert(Window.GetMode() == 0)
            assert(Util.IsUiBusy() == false)
            assert(Window.IsBusy() == false)

            -- Identified UiMode constants
            assert(Util.UiMode.normal == 0x00)
            assert(Util.UiMode.loot == 0x01)
            assert(Util.UiMode.input == 0x02)
            assert(Util.UiMode.text_dialog == 0x09)
            assert(Util.UiMode.merchant == 0x0A)
            assert(Util.UiMode.trainer == 0x0C)
            assert(Util.UiMode.start_menu == 0x0D)
            assert(Util.UiMode.confirm == 0x0E)
            assert(Util.UiMode.context_menu == 0x1E)
            assert(Util.UiMode.r2_chat_window == 0x21)

            assert(Window.Mode.normal == 0x00)
            assert(Window.Mode.start_menu == 0x0D)
            assert(Window.Mode.trainer == 0x0C)
        "#,
        )
        .unwrap();

        // 2. Spellbook mode (5): start menu open, ui_mode 5, busy
        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: true,
            ui_mode: 5,
            ui_busy: true,
            ..Default::default()
        });

        host.execute(
            r#"
            local Util = require("frontiers_forge.util")
            local Window = require("frontiers_forge.window")

            assert(Util.IsStartMenuOpen() == 1)
            assert(Window.IsStartMenuOpen() == true)
            assert(Util.GetUiMode() == 5)
            assert(Window.GetMode() == 5)
            assert(Util.IsUiBusy() == true)
            assert(Window.IsBusy() == true)
        "#,
        )
        .unwrap();

        // 3. Auction search mode (39): start menu open, ui_mode 39, busy
        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: true,
            ui_mode: 39,
            ui_busy: true,
            ..Default::default()
        });

        host.execute(
            r#"
            local Util = require("frontiers_forge.util")
            assert(Util.IsStartMenuOpen() == 1)
            assert(Util.GetUiMode() == 39)
            assert(Util.IsUiBusy() == true)
        "#,
        )
        .unwrap();

        // 4. Chat editbox mode (2): start menu closed, ui_mode 2, busy
        host.snapshot.set(Snapshot {
            in_game: true,
            start_menu: false,
            ui_mode: 2,
            ui_busy: true,
            ..Default::default()
        });

        host.execute(
            r#"
            local Util = require("frontiers_forge.util")
            assert(Util.IsStartMenuOpen() == 0)
            assert(Util.GetUiMode() == 2)
            assert(Util.IsUiBusy() == true)
        "#,
        )
        .unwrap();
    }
}
