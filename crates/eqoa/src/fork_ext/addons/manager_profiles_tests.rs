// Included in manager::tests so this exercises the real VM/GPU lifecycle seam.
#[test]
fn mod_profiles_two_characters_restore_save_state_and_layout_on_fresh_vms() {
    use super::super::named_profiles::{Character, CharacterSnapshot, Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package(
        "a",
        true,
        r#"
        value=value or 0
        frames=(frames or 0)+1
        UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {value=value} end)
        UiForge.RegisterCallback(UiForge.CallbackType.Load,function(s) value=s.value end)
        ImGui.Begin('Orb');ImGui.End()
    "#,
    );
    let addon = super::super::discovery::discover(&root.0).addons.remove(0);
    let key = Storage::key(&addon).unwrap();
    let store = Store::new(Some(config.0.clone()));
    let lua = mlua::Lua::new();
    for (name, value, pos) in [("Healer", 11, "10,20"), ("Tank", 22, "110,120")] {
        let mut doc = Document::empty();
        doc.enabled.insert(key.clone(), true);
        doc.addons.insert(
            key.clone(),
            super::super::profile::encode(
                lua.load(format!("return {{value={value}}}"))
                    .eval()
                    .unwrap(),
            )
            .unwrap(),
        );
        doc.layout = format!("[Window][Orb]\nPos={pos}\nSize=80,80\nCollapsed=0\n\n");
        store.create(name, &doc).unwrap();
    }
    let a = CharacterSnapshot::new(true, "Ari", 3).unwrap();
    let b = CharacterSnapshot::new(true, "Bea", 1).unwrap();
    store
        .bind(
            &Character::new("server.example", a).unwrap(),
            "Healer",
            false,
        )
        .unwrap();
    store
        .bind(&Character::new("server.example", b).unwrap(), "Tank", false)
        .unwrap();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    let cleaned = std::rc::Rc::new(std::cell::Cell::new(0));
    for (character, profile, value, pos) in [
        (a, "Healer", 11, "10,20"),
        (b, "Tank", 22, "110,120"),
        (a, "Healer", 11, "10,20"),
    ] {
        manager.profile_boundary(&mut gpu.context, Some(character), "server.example");
        assert_eq!(manager.profile_name(), Some(profile));
        gpu.begin();
        manager.frame(Snapshot::default());
        gpu.render();
        manager.after_render();
        assert_eq!(
            manager.entries[0]
                .instance
                .as_ref()
                .unwrap()
                .host
                .lua
                .globals()
                .get::<i32>("value")
                .unwrap(),
            value
        );
        assert_eq!(
            manager.entries[0]
                .instance
                .as_ref()
                .unwrap()
                .host
                .lua
                .globals()
                .get::<i32>("frames")
                .unwrap(),
            1,
            "switch must initialize a fresh VM"
        );
        let mut ini = String::new();
        gpu.context.save_ini_settings(&mut ini);
        assert!(
            ini.contains(&format!("[Window][Orb]\nPos={pos}")),
            "layout was not applied before the first frame: {ini}"
        );
        let host = &manager.entries[0].instance.as_ref().unwrap().host;
        let count = cleaned.clone();
        host.lua
            .globals()
            .set(
                "record_cleanup",
                host.lua
                    .create_function(move |_, ()| {
                        count.set(count.get() + 1);
                        Ok(())
                    })
                    .unwrap(),
            )
            .unwrap();
        host.execute("UiForge.RegisterCallback(UiForge.CallbackType.DisableScript,record_cleanup);UiForge.RegisterCallback(UiForge.CallbackType.OnEject,record_cleanup)").unwrap();
    }
    assert_eq!(
        cleaned.get(),
        4,
        "each previous VM must DisableScript and OnEject once"
    );
}

#[test]
fn mod_profiles_manager_save_as_bind_class_switch_and_delete_actions() {
    use super::super::named_profiles::{Character, CharacterSnapshot, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    let a = CharacterSnapshot::new(true, "Ari", 3).unwrap();
    let b = CharacterSnapshot::new(true, "Bea", 3).unwrap();
    manager.profile_boundary(&mut gpu.context, Some(a), "server.example");
    manager.queue_profile(ProfileAction::SaveAs("Healer".into()));
    manager.queue_profile(ProfileAction::BindCharacter);
    manager.queue_profile(ProfileAction::BindClass);
    manager.profile_boundary(&mut gpu.context, Some(a), "server.example");
    assert_eq!(manager.profile_name(), Some("Healer"));
    let store = Store::new(Some(config.0.clone()));
    assert_eq!(
        store
            .resolve(&Character::new("server.example", a).unwrap())
            .unwrap()
            .0,
        "Healer"
    );
    manager.profile_boundary(&mut gpu.context, Some(b), "server.example");
    assert_eq!(
        manager.profile_name(),
        Some("Healer"),
        "class default must select the profile on world entry"
    );
    manager.queue_profile(ProfileAction::SaveAs("Temp".into()));
    manager.profile_boundary(&mut gpu.context, Some(b), "server.example");
    assert_eq!(manager.profile_name(), Some("Temp"));
    manager.queue_profile(ProfileAction::Load("Healer".into()));
    manager.queue_profile(ProfileAction::Delete("Temp".into()));
    manager.profile_boundary(&mut gpu.context, Some(b), "server.example");
    assert!(store.read("Temp").is_err());
    manager.queue_profile(ProfileAction::SaveAs("../bad".into()));
    manager.profile_boundary(&mut gpu.context, Some(b), "server.example");
    assert_eq!(manager.profile_name(), Some("Healer"));
    assert!(
        manager
            .diagnostics()
            .iter()
            .any(|d| d.phase == "profiles" && d.message.contains("profile name"))
    );
    gpu.context.io_mut().set_display_size([800., 600.]);
    gpu.context.io_mut().set_delta_time(1. / 60.);
    let ui = gpu.context.frame();
    ui.window("Addons").build(|| manager.profile_controls(ui));
    gpu.render();
    manager.after_render();
}

#[test]
fn mod_profiles_two_clients_save_only_their_own_active_bundle() {
    use super::super::named_profiles::{Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package(
        "a",
        true,
        r#"value=value or 0
        UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {value=value} end)
        UiForge.RegisterCallback(UiForge.CallbackType.Load,function(s) value=s.value end)"#,
    );
    let addon = super::super::discovery::discover(&root.0).addons.remove(0);
    let key = Storage::key(&addon).unwrap();
    let store = Store::new(Some(config.0.clone()));
    for (name, value) in [("Healer", 11), ("Tank", 22)] {
        let mut doc = Document::empty();
        doc.enabled.insert(key.clone(), true);
        let lua = mlua::Lua::new();
        doc.addons.insert(
            key.clone(),
            super::super::profile::encode(
                lua.load(format!("return {{value={value}}}"))
                    .eval()
                    .unwrap(),
            )
            .unwrap(),
        );
        store.create(name, &doc).unwrap();
    }
    let mut a = gpu.manager(&root.0, Some(config.0.clone()));
    let mut b = gpu.manager(&root.0, Some(config.0.clone()));
    for (manager, name) in [(&mut a, "Healer"), (&mut b, "Tank")] {
        manager.enable_profiles(Some(config.0.clone()));
        manager.queue_profile(ProfileAction::Load(name.into()));
        manager.profile_boundary(&mut gpu.context, None, "server.example");
        gpu.begin();
        manager.frame(Snapshot::default());
        gpu.render();
        manager.after_render();
    }
    a.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .execute("value=77")
        .unwrap();
    a.queue_profile(ProfileAction::Save);
    a.profile_boundary(&mut gpu.context, None, "server.example");
    b.queue_profile(ProfileAction::Save);
    b.profile_boundary(&mut gpu.context, None, "server.example");
    assert_eq!(a.profile_name(), Some("Healer"));
    assert_eq!(b.profile_name(), Some("Tank"));
    let lua = mlua::Lua::new();
    for (name, value) in [("Healer", 77), ("Tank", 22)] {
        let doc = store.read(name).unwrap();
        let saved = super::super::profile::decode(&lua, &doc.addons[&key]).unwrap();
        assert_eq!(
            saved.as_table().unwrap().get::<i32>("value").unwrap(),
            value
        );
    }
    assert!(!config.0.join("addons/imgui.ini").exists());
    assert!(
        !Storage::new(Some(config.0.clone()))
            .profile_path(&key)
            .unwrap()
            .exists(),
        "named clients must not write legacy per-addon Save files"
    );
    a.shutdown();
    a.flush_profile(&mut gpu.context, true);
    b.shutdown();
    b.flush_profile(&mut gpu.context, true);
}

#[test]
fn mod_profiles_failed_load_keeps_active_and_can_escape_damaged_default() {
    use super::super::named_profiles::{Character, CharacterSnapshot, Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    let store = Store::new(Some(config.0.clone()));
    store.create("Default", &Document::empty()).unwrap();
    store.create("Healer", &Document::empty()).unwrap();
    store.create("Tank", &Document::empty()).unwrap();
    let character = CharacterSnapshot::new(true, "Ari", 3).unwrap();
    store
        .bind(
            &Character::new("server.example", character).unwrap(),
            "Healer",
            false,
        )
        .unwrap();
    fs::write(store.path("Default").unwrap(), b"broken-default").unwrap();
    fs::write(store.path("Tank").unwrap(), b"broken-tank").unwrap();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    // Regression: settings operations must work outside a bound ImGui frame,
    // including an empty fallback layout after a damaged Default.
    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
    }
    manager.profile_boundary(&mut gpu.context, Some(character), "server.example");
    assert!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }.is_null());
    assert_eq!(manager.profile_name(), Some("Healer"));
    manager.queue_profile(ProfileAction::Load("Tank".into()));
    manager.profile_boundary(&mut gpu.context, Some(character), "server.example");
    assert_eq!(manager.profile_name(), Some("Healer"));
    assert!(
        manager
            .diagnostics()
            .iter()
            .any(|d| d.phase == "profiles" && d.message.contains("Profile action failed"))
    );
    assert_eq!(
        fs::read(store.path("Default").unwrap()).unwrap(),
        b"broken-default"
    );
    assert_eq!(
        fs::read(store.path("Tank").unwrap()).unwrap(),
        b"broken-tank"
    );
}

#[test]
fn mod_profiles_failed_migration_cannot_create_empty_default_on_shutdown() {
    use super::super::named_profiles::Store;
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package("a", true, "");
    fs::create_dir_all(config.0.join("addons")).unwrap();
    let old = config.0.join("addons/state.json");
    fs::write(&old, b"unreadable-legacy-state").unwrap();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    manager.shutdown();
    manager.flush_profile(&mut gpu.context, true);
    let store = Store::new(Some(config.0.clone()));
    assert!(
        !store.path("Default").unwrap().exists(),
        "failed migration must remain retryable, never replaced by an empty Default"
    );
    assert_eq!(fs::read(&old).unwrap(), b"unreadable-legacy-state");
    fs::write(&old, br#"{"schemaVersion":1,"enabled":{}}"#).unwrap();
    let addons = super::super::discovery::discover(&root.0).addons;
    assert!(
        store
            .migrate(&Storage::new(Some(config.0.clone())), &addons)
            .is_ok(),
        "repairing legacy input must allow migration on next launch"
    );
}

#[test]
fn mod_profiles_missing_window_layout_does_not_inherit_previous_geometry() {
    use super::super::named_profiles::{Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package(
        "a",
        true,
        "ImGui.SetNextWindowPos(40,50,4);ImGui.Begin('Orb');x,y=ImGui.GetWindowPos();ImGui.End()",
    );
    let addon = super::super::discovery::discover(&root.0).addons.remove(0);
    let key = Storage::key(&addon).unwrap();
    let store = Store::new(Some(config.0.clone()));
    let mut doc = Document::empty();
    doc.enabled.insert(key, true);
    doc.layout = "[Window][Orb]\nPos=110,120\nSize=80,80\nCollapsed=0\n\n".into();
    store.create("Positioned", &doc).unwrap();
    doc.layout.clear();
    store.create("Fresh", &doc).unwrap();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    for (name, x, y) in [("Positioned", 110., 120.), ("Fresh", 40., 50.)] {
        manager.queue_profile(ProfileAction::Load(name.into()));
        manager.profile_boundary(&mut gpu.context, None, "server.example");
        gpu.begin();
        manager.frame(Snapshot::default());
        gpu.render();
        manager.after_render();
        let h = &manager.entries[0].instance.as_ref().unwrap().host;
        assert_eq!(
            h.lua.globals().get::<f32>("x").unwrap(),
            x,
            "{name} position must follow its own layout/FirstUseEver condition"
        );
        assert_eq!(h.lua.globals().get::<f32>("y").unwrap(), y);
    }
}

#[test]
fn mod_profiles_disabled_windows_do_not_contaminate_saved_layout() {
    use super::super::named_profiles::{Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package("a",true,"ImGui.SetNextWindowPos(40,50,4);ImGui.SetNextWindowSize(70,60,4);ImGui.Begin('Orb');x,y=ImGui.GetWindowPos();ImGui.End()");
    let addon = super::super::discovery::discover(&root.0).addons.remove(0);
    let key = Storage::key(&addon).unwrap();
    let store = Store::new(Some(config.0.clone()));
    let mut doc = Document::empty();
    doc.enabled.insert(key.clone(), true);
    doc.layout = "[Window][Orb]\nPos=110,120\nSize=180,160\nCollapsed=0\n\n".into();
    store.create("Positioned", &doc).unwrap();
    doc.enabled.insert(key, false);
    doc.layout.clear();
    store.create("Dormant", &doc).unwrap();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    manager.queue_profile(ProfileAction::Load("Positioned".into()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    manager.queue_profile(ProfileAction::Load("Dormant".into()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    manager.flush_profile(&mut gpu.context, true);
    assert!(
        !store
            .read("Dormant")
            .unwrap()
            .layout
            .contains("[Window][Orb]"),
        "a disabled window from the old profile must never be saved into the new profile"
    );
    manager.queue_profile(ProfileAction::Load("Dormant".into()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    manager.queue(Command::Enable("a".into(), true));
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    let h = &manager.entries[0].instance.as_ref().unwrap().host;
    assert_eq!(h.lua.globals().get::<f32>("x").unwrap(), 40.);
    assert_eq!(h.lua.globals().get::<f32>("y").unwrap(), 50.);
    assert_eq!(manager.entries[0].status, Status::Enabled);
    let mut ini = String::new();
    gpu.context.save_ini_settings(&mut ini);
    assert!(
        ini.contains("[Window][Orb]\nPos=40,50\nSize=70,60"),
        "new-profile defaults must set its own size: {ini}"
    );
}

#[test]
fn mod_profiles_failed_save_keeps_previous_bundle_on_later_autosave() {
    use super::super::named_profiles::Store;
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package(
        "a",
        true,
        "UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {value=77} end)",
    );
    root.package("b",true,"UiForge.RegisterCallback(UiForge.CallbackType.Save,function() error('synthetic Save failure') end)");
    root.package("c", false, "");
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    manager.queue_profile(ProfileAction::Save);
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    assert!(
        manager
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("Save callback failed"))
    );
    // A later enabled-state autosave must not publish part of the failed Save.
    manager.queue(Command::Enable("c".into(), true));
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    manager.flush_profile(&mut gpu.context, false);
    assert!(
        Store::new(Some(config.0.clone()))
            .read("Default")
            .unwrap()
            .addons
            .is_empty()
    );
}

#[test]
fn mod_profiles_shutdown_save_failure_preserves_the_entire_previous_file() {
    use super::super::named_profiles::Store;
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    root.package("a", true, "UiForge.RegisterCallback(UiForge.CallbackType.Save,function() saves=(saves or 0)+1;return {value=77} end)");
    root.package("b", true, "UiForge.RegisterCallback(UiForge.CallbackType.Save,function() saves=(saves or 0)+1;error('synthetic shutdown Save failure') end)");
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    manager.flush_profile(&mut gpu.context, true);
    let store = Store::new(Some(config.0.clone()));
    let path = store.path("Default").unwrap();
    let previous = fs::read(&path).unwrap();
    manager.shutdown();
    manager.flush_profile(&mut gpu.context, true);
    assert_eq!(
        fs::read(&path).unwrap(),
        previous,
        "one failed shutdown Save must prevent publication of all new state and layout"
    );
    assert!(
        manager
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("Save") && d.message.contains("not saved"))
    );
    for entry in &manager.entries {
        assert_eq!(
            entry
                .instance
                .as_ref()
                .unwrap()
                .host
                .lua
                .globals()
                .get::<i32>("saves")
                .unwrap(),
            1
        );
    }
}

#[test]
fn mod_profiles_active_load_and_default_world_entry_preserve_unsaved_session() {
    use super::super::named_profiles::CharacterSnapshot;
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    fs::create_dir_all(config.0.join("addons")).unwrap();
    fs::write(
        config.0.join("addons/imgui.ini"),
        "[Window][Orb]\nPos=11,22\nSize=80,80\nCollapsed=0\n\n",
    )
    .unwrap();
    root.package("a", true, "value=value or 0; frames=(frames or 0)+1; UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {value=value} end); ImGui.Begin('Orb');ImGui.End()");
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    manager.profile_boundary(&mut gpu.context, None, "server.example");
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .execute("value=77")
        .unwrap();
    gpu.context
        .load_ini_settings("[Window][Orb]\nPos=110,120\nSize=180,180\nCollapsed=0\n\n");
    let mut changed_layout = String::new();
    gpu.context.save_ini_settings(&mut changed_layout);
    for identity in [None, CharacterSnapshot::new(true, "Ari", 3)] {
        manager.queue_profile(ProfileAction::Load("Default".into()));
        manager.profile_boundary(&mut gpu.context, identity, "server.example");
        let host = &manager.entries[0]
            .instance
            .as_ref()
            .expect("active profile must keep its VM")
            .host;
        assert_eq!(host.lua.globals().get::<i32>("value").unwrap(), 77);
        assert_eq!(host.lua.globals().get::<i32>("frames").unwrap(), 1);
        let mut layout = String::new();
        gpu.context.save_ini_settings(&mut layout);
        assert_eq!(
            layout, changed_layout,
            "active load/world entry must preserve unsaved layout"
        );
    }
    manager.queue_profile(ProfileAction::Revert);
    manager.profile_boundary(
        &mut gpu.context,
        CharacterSnapshot::new(true, "Ari", 3),
        "server.example",
    );
    gpu.begin();
    manager.frame(Snapshot::default());
    gpu.render();
    manager.after_render();
    let host = &manager.entries[0].instance.as_ref().unwrap().host;
    assert_eq!(host.lua.globals().get::<i32>("value").unwrap(), 0);
    assert_eq!(host.lua.globals().get::<i32>("frames").unwrap(), 1);
    let mut reverted_layout = String::new();
    gpu.context.save_ini_settings(&mut reverted_layout);
    assert!(
        reverted_layout.contains("[Window][Orb]\nPos=11,22"),
        "explicit revert must restore saved layout: {reverted_layout}"
    );
}

#[test]
fn mod_profiles_failed_world_entry_retries_after_backoff_with_one_diagnostic() {
    use super::super::named_profiles::{Character, CharacterSnapshot, Document, Store};
    let Some(mut gpu) = Gpu::new() else {
        return;
    };
    let root = Scratch::new();
    let config = Scratch::new();
    let store = Store::new(Some(config.0.clone()));
    store.create("Healer", &Document::empty()).unwrap();
    let identity = CharacterSnapshot::new(true, "Ari", 3).unwrap();
    store
        .bind(
            &Character::new("server.example", identity).unwrap(),
            "Healer",
            false,
        )
        .unwrap();
    let path = store.path("Healer").unwrap();
    let healthy = fs::read(&path).unwrap();
    fs::write(&path, b"broken").unwrap();
    let mut manager = gpu.manager(&root.0, Some(config.0.clone()));
    manager.enable_profiles(Some(config.0.clone()));
    manager.profile_boundary(&mut gpu.context, Some(identity), "server.example");
    assert_eq!(manager.profile_name(), Some("Default"));
    std::thread::sleep(std::time::Duration::from_millis(2100));
    manager.profile_boundary(&mut gpu.context, Some(identity), "server.example");
    assert_eq!(manager.profile_name(), Some("Default"));
    fs::write(&path, healthy).unwrap();
    for _ in 0..10 {
        manager.profile_boundary(&mut gpu.context, Some(identity), "server.example");
    }
    assert_eq!(
        manager.profile_name(),
        Some("Default"),
        "automatic failures must back off instead of retrying every frame"
    );
    std::thread::sleep(std::time::Duration::from_millis(2100));
    manager.profile_boundary(&mut gpu.context, Some(identity), "server.example");
    assert_eq!(
        manager.profile_name(),
        Some("Healer"),
        "a failed identity must remain retryable after its file is repaired"
    );
    assert_eq!(
        manager
            .diagnostics()
            .iter()
            .filter(|d| d.phase == "profiles" && d.message.contains("Could not switch profile"))
            .count(),
        1
    );
}

#[test]
fn test_profiles_ui_class_name_display_and_button_label() {
    use super::named::{class_display, class_default_button_label};
    use crate::fork_ext::addons::named_profiles::{Character, CharacterSnapshot};

    // In-range classes display proper name
    assert_eq!(class_display(0), "Warrior");
    assert_eq!(class_display(11), "Necromancer");
    assert_eq!(class_display(14), "Alchemist");

    // Out-of-range classes show number
    assert_eq!(class_display(15), "class 15");
    assert_eq!(class_display(99), "class 99");

    // Button label uses class name if in range
    let necromancer = Character::new(
        "server.example",
        CharacterSnapshot::new(true, "zecron", 11).unwrap(),
    ).unwrap();
    assert_eq!(class_default_button_label(Some(&necromancer)), "Use as Necromancer default");

    // Button label falls back to "Use as class default" if out of range or None
    let unknown_class = Character {
        server: "server.example".into(),
        name: "unknown".into(),
        class: 99,
    };
    assert_eq!(class_default_button_label(Some(&unknown_class)), "Use as class default");
    assert_eq!(class_default_button_label(None), "Use as class default");
}
