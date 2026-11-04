// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see LICENSE). LoopRook's additions first published in 0.4.6 or later are under LICENSE-ENHANCED; upstream and third-party code stays under its own license.
// Included in manager::tests: exercise the real lifecycle, Lua and GPU bridge.
#[test]
fn forge_recovery_caught_error_keeps_addon_running_and_warns_once() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    root.package(
        "caught",
        true,
        r#"
        ticks=(ticks or 0)+1
        if real_error then error('uncaught real error') end
        assert(not pcall(function()
            ImGui.Begin('caught window')
            ImGui.PushStyleColor(ImGuiCol.Text,1,0,0,1)
            error('caught before End')
        end))
    "#,
    );
    let mut manager = gpu.manager(&root.0, None);
    for _ in 0..5 {
        gpu.begin();
        let raw = gpu.context.as_raw();
        let before = unsafe { ((*raw).CurrentWindowStack.Size, (*raw).ColorStack.Size) };
        manager.frame(Snapshot::default());
        assert_eq!(
            unsafe { ((*raw).CurrentWindowStack.Size, (*raw).ColorStack.Size) },
            before
        );
        gpu.render();
        manager.after_render();
        assert_eq!(
            manager.entries[0].status,
            Status::Enabled,
            "{:?}",
            manager.diagnostics()
        );
    }
    assert_eq!(
        lua(&manager, "caught")
            .globals()
            .get::<i32>("ticks")
            .unwrap(),
        5
    );
    assert_eq!(
        manager
            .diagnostics()
            .iter()
            .filter(|d| d.phase == "imgui")
            .count(),
        1
    );
    let log = fs::read_to_string(root.0.join("addons_error.log")).unwrap();
    assert_eq!(log.matches("unbalanced ImGui stacks recovered").count(), 1);
    lua(&manager, "caught")
        .globals()
        .set("real_error", true)
        .unwrap();
    gpu.frame(&mut manager);
    assert_eq!(
        manager.entries[0].status,
        Status::Failed,
        "an escaping Lua error must remain fatal"
    );
}

#[test]
fn forge_recovery_distinct_leaks_have_distinct_bounded_diagnostics() {
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    root.package(
        "caught",
        true,
        r#"
        ticks=(ticks or 0)+1
        assert(not pcall(function()
            ImGui.Begin('caught window')
            if ticks>2 then ImGui.BeginGroup() end
            error('caught')
        end))
    "#,
    );
    let mut manager = gpu.manager(&root.0, None);
    for _ in 0..6 {
        gpu.frame(&mut manager);
        assert_eq!(manager.entries[0].status, Status::Enabled);
    }
    let warnings: Vec<_> = manager
        .diagnostics()
        .iter()
        .filter(|d| d.phase == "imgui")
        .collect();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings.iter().any(|d| d.message.contains("Group")));
}

fn copy_unchanged_mod(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_unchanged_mod(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn modern_recovery_matrix(profiles: bool, level: u32) {
    use super::super::named_profiles::{Character, CharacterSnapshot, Document, Store};
    let reference = PathBuf::from(
        std::env::var_os("EQOA_FORGE_REFERENCE").expect("needs EQOA_FORGE_REFERENCE"),
    );
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    // The package bytes are unchanged; all manager logs and overlays stay in scratch.
    copy_unchanged_mod(
        &reference.join("scripts/avoids_modern_ui"),
        &root.0.join("avoids_modern_ui"),
    );
    let addon = super::super::discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "avoids_modern_ui")
        .unwrap();
    let key = Storage::key(&addon).unwrap();
    let mut snapshot = super::super::wereoxxs_test::rich_in_game_snapshot();
    snapshot.player.level = 59;
    snapshot.player.available = true;
    // Capture an authentic complete Save payload from the unchanged package.
    let mut seed = gpu.manager(&root.0, None);
    seed.queue(Command::Enable("avoids_modern_ui".into(), true));
    gpu.begin();
    seed.frame(snapshot);
    gpu.render();
    seed.after_render();
    assert_eq!(
        seed.entries[0].status,
        Status::Enabled,
        "{:?}",
        seed.diagnostics()
    );
    let seed_host = &seed.entries[0].instance.as_ref().unwrap().host;
    seed_host
        .execute("modern_exp_bars_state.bar_width=333")
        .unwrap();
    let complete =
        super::super::profile::encode(seed_host.callback(2, Value::Nil).unwrap()).unwrap();
    seed.shutdown();
    drop(seed);
    let lua = Lua::new();
    let empty = super::super::profile::encode(lua.load("return {}").eval().unwrap()).unwrap();
    let partial = super::super::profile::encode(
        lua.load("return {ExpBars={bar_width=327},mod_enabled={ExpBars=false}}")
            .eval()
            .unwrap(),
    )
    .unwrap();
    let character = CharacterSnapshot::new(true, "ScopeTest", 7).unwrap();
    snapshot.player.level = level;
    snapshot.character = Some(character);
    for (kind, saved, width) in [
        ("absent", None, 200),
        ("empty", Some(empty), 200),
        ("partial", Some(partial), 327),
        ("complete", Some(complete), 333),
    ] {
        let config = Scratch::new();
        let storage = Storage::new(Some(config.0.clone()));
        let store = Store::new(Some(config.0.clone()));
        if profiles {
            let mut doc = Document::empty();
            doc.enabled.insert(key.clone(), false);
            if let Some(value) = saved.clone() {
                doc.addons.insert(key.clone(), value);
            }
            store.create("Default", &doc).unwrap();
            store
                .bind(
                    &Character::new("server.example", character).unwrap(),
                    "Default",
                    false,
                )
                .unwrap();
        } else {
            storage.set_enabled(&key, false).unwrap();
            if let Some(value) = saved {
                storage
                    .save_profile(&key, super::super::profile::decode(&lua, &value).unwrap())
                    .unwrap();
            }
        }
        let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
        if profiles {
            manager.enable_profiles(Some(config.0.clone()));
            manager.profile_boundary(&mut gpu.context, Some(character), "server.example");
        }
        gpu.begin();
        manager.frame(snapshot);
        gpu.render();
        manager.after_render();
        assert_eq!(manager.entries[0].status, Status::Disabled);
        assert!(
            !manager.entries()[0].initialized,
            "the settings hint is expected before enabling a fresh VM"
        );
        manager.queue(Command::Enable("avoids_modern_ui".into(), true));
        for frame in 0..5 {
            gpu.begin();
            let raw = gpu.context.as_raw();
            let depth = unsafe { (*raw).CurrentWindowStack.Size };
            manager.frame(snapshot);
            assert_eq!(
                unsafe { (*raw).CurrentWindowStack.Size },
                depth,
                "native scope leak"
            );
            gpu.render();
            manager.after_render();
            assert_eq!(
                manager.entries[0].status,
                Status::Enabled,
                "profiles={profiles}, level={level}, payload={kind}, frame={frame}: {:?}",
                manager.diagnostics()
            );
            let host = &manager.entries[0].instance.as_ref().unwrap().host;
            host.execute(&format!("assert(modern_exp_bars_state.initialized==true);assert(modern_exp_bars_state.bar_width=={width});assert(type(modern_exp_bars_state.bar_height)=='number');assert(require('frontiers_forge.player').GetLevel()=={level});assert((require('frontiers_forge.util').GetExpRequiredForLevel({level})==nil)=={})",level==60)).unwrap();
            if kind == "partial" {
                host.execute("assert(modern_ui_mods_state.mod_enabled.ExpBars==false)")
                    .unwrap();
            }
            let mut drawing = false;
            unsafe {
                for i in 0..(*raw).Windows.Size {
                    let w = *(*raw).Windows.Data.add(i as usize);
                    if (*w).Active && (*w).DrawList.as_ref().is_some_and(|d| d.VtxBuffer.Size > 0) {
                        drawing = true;
                    }
                }
            }
            assert!(
                drawing,
                "the unchanged package must continue drawing at level {level}"
            );
        }
        let diags = manager.diagnostics();
        assert!(
            !diags.iter().any(|d| d.message.contains("unsupported")
                || d.message.contains("reload a failed addon")),
            "{diags:?}"
        );
        assert_eq!(
            diags.iter().filter(|d| d.phase == "imgui").count(),
            usize::from(level == 60),
            "{diags:?}"
        );
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.message.contains("Update failed for ExpBars"))
                .count(),
            usize::from(level == 60),
            "{diags:?}"
        );
        println!(
            "Modern UI: profiles={profiles}, level={level}, saved={kind}: enabled, initialized, saved width={width}, five drawing frames"
        );
        manager.shutdown();
    }
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_recovery_modern_profiles_level59() {
    modern_recovery_matrix(true, 59);
}
#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_recovery_modern_profiles_level60() {
    modern_recovery_matrix(true, 60);
}
#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_recovery_modern_legacy_level59() {
    modern_recovery_matrix(false, 59);
}
#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_recovery_modern_legacy_level60() {
    modern_recovery_matrix(false, 60);
}
