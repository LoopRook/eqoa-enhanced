use mlua::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

pub const MAX_EFFECTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectSnapshot {
    pub icon_ref: u32,
    pub name_len: u8,
    pub name: [u8; 32],
}

impl EffectSnapshot {
    pub fn name_str(&self) -> &str {
        let len = (self.name_len as usize).min(self.name.len());
        std::str::from_utf8(&self.name[..len]).unwrap_or("")
    }
}

impl Default for EffectSnapshot {
    fn default() -> Self {
        Self {
            icon_ref: 0,
            name_len: 0,
            name: [0u8; 32],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EffectsSnapshot {
    pub count: u8,
    pub effects: [EffectSnapshot; MAX_EFFECTS],
}

pub(crate) fn create_module(
    lua: &mlua::Lua,
    snapshot: &Rc<Cell<super::host::Snapshot>>,
) -> LuaResult<mlua::Table> {
    let t = lua.create_table()?;
    t.set("max_effects", MAX_EFFECTS as u32)?;

    let snap_cnt = Rc::clone(snapshot);
    t.set(
        "GetCount",
        lua.create_function(move |_, ()| {
            let snap = snap_cnt.get();
            if !snap.in_game {
                return Ok(0u32);
            }
            let count = (snap.effects.count as usize).min(MAX_EFFECTS);
            Ok(count as u32)
        })?,
    )?;

    let snap_icon = Rc::clone(snapshot);
    t.set(
        "GetIconRef",
        lua.create_function(move |_, index: i32| {
            let snap = snap_icon.get();
            if !snap.in_game || index < 0 {
                return Ok(None);
            }
            let idx = index as usize;
            let count = (snap.effects.count as usize).min(MAX_EFFECTS);
            if idx >= count {
                return Ok(None);
            }
            Ok(Some(snap.effects.effects[idx].icon_ref))
        })?,
    )?;

    let snap_name = Rc::clone(snapshot);
    t.set(
        "GetName",
        lua.create_function(move |_, index: i32| {
            let snap = snap_name.get();
            if !snap.in_game || index < 0 {
                return Ok(None);
            }
            let idx = index as usize;
            let count = (snap.effects.count as usize).min(MAX_EFFECTS);
            if idx >= count {
                return Ok(None);
            }
            Ok(Some(snap.effects.effects[idx].name_str().to_string()))
        })?,
    )?;

    let snap_eff = Rc::clone(snapshot);
    t.set(
        "GetEffect",
        lua.create_function(move |lua, index: i32| {
            let snap = snap_eff.get();
            if !snap.in_game || index < 0 {
                return Ok(None);
            }
            let idx = index as usize;
            let count = (snap.effects.count as usize).min(MAX_EFFECTS);
            if idx >= count {
                return Ok(None);
            }
            let eff = &snap.effects.effects[idx];
            let tbl = lua.create_table()?;
            tbl.set("index", index)?;
            tbl.set("icon_ref", eff.icon_ref)?;
            tbl.set("name", eff.name_str())?;
            Ok(Some(tbl))
        })?,
    )?;

    let snap_all = Rc::clone(snapshot);
    t.set(
        "All",
        lua.create_function(move |lua, ()| {
            let snap = snap_all.get();
            let count = if snap.in_game {
                (snap.effects.count as usize).min(MAX_EFFECTS)
            } else {
                0
            };
            let mut curr = 0;
            let iter = lua.create_function_mut(move |lua, ()| {
                if curr >= count {
                    return Ok((None, None));
                }
                let idx = curr;
                curr += 1;
                let eff = &snap.effects.effects[idx];
                let tbl = lua.create_table()?;
                tbl.set("index", idx as i32)?;
                tbl.set("icon_ref", eff.icon_ref)?;
                tbl.set("name", eff.name_str())?;
                Ok((Some(idx as i64), Some(tbl)))
            })?;
            Ok(iter)
        })?,
    )?;

    Ok(t)
}

pub fn install_effects_adapter(host: &super::host::Host) -> LuaResult<()> {
    let mod_t = create_module(&host.lua, &host.snapshot)?;
    host.register_host_module("frontiers_forge.effects", mlua::Value::Table(mod_t))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::DiscoveredAddon;
    use crate::fork_ext::addons::host::Host;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_test_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("eqoa_effects_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test effects addon").unwrap();
        DiscoveredAddon {
            id: "test_effects".to_string(),
            name: "Test Effects".to_string(),
            entry,
            script_root: root.to_path_buf(),
            package_root: root.to_path_buf(),
            module_dirs: vec![root.to_path_buf()],
            resource_dirs: vec![],
            api: Some("eqoa/1".to_string()),
            requested_capabilities: vec![],
            initially_enabled: true,
        }
    }

    fn test_host(name: &str) -> Host {
        let dir = temp_test_dir(name);
        let addon = test_addon(&dir);
        Host::for_addon(&addon).unwrap()
    }

    #[test]
    fn forge_effects_module_queries_and_all_iterator() {
        let host = test_host("queries");
        let lua = &host.lua;

        // Set snapshot with 2 active effects
        let mut snap = host.snapshot.get();
        snap.in_game = true;
        snap.effects.count = 2;

        let mut eff0 = EffectSnapshot::default();
        eff0.icon_ref = 0x1234;
        let name0 = b"Spirit of Wolf";
        eff0.name_len = name0.len() as u8;
        eff0.name[..name0.len()].copy_from_slice(name0);
        snap.effects.effects[0] = eff0;

        let mut eff1 = EffectSnapshot::default();
        eff1.icon_ref = 0x5678;
        let name1 = b"Chloroplast";
        eff1.name_len = name1.len() as u8;
        eff1.name[..name1.len()].copy_from_slice(name1);
        snap.effects.effects[1] = eff1;

        host.snapshot.set(snap);

        let script = r#"
            local Effects = require('frontiers_forge.effects')
            assert(Effects.max_effects == 8)
            assert(Effects.GetCount() == 2)
            assert(Effects.GetIconRef(0) == 0x1234)
            assert(Effects.GetName(0) == "Spirit of Wolf")
            assert(Effects.GetIconRef(1) == 0x5678)
            assert(Effects.GetName(1) == "Chloroplast")
            assert(Effects.GetIconRef(2) == nil)
            assert(Effects.GetName(2) == nil)

            local eff0 = Effects.GetEffect(0)
            assert(eff0.index == 0 and eff0.icon_ref == 0x1234 and eff0.name == "Spirit of Wolf")

            local count = 0
            for idx, eff in Effects.All() do
                count = count + 1
                if idx == 0 then
                    assert(eff.name == "Spirit of Wolf")
                elseif idx == 1 then
                    assert(eff.name == "Chloroplast")
                end
            end
            assert(count == 2)
        "#;
        lua.load(script).exec().unwrap();
    }
}
