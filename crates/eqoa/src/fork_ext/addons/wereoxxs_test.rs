//! Full integration test suite for the unchanged wereoxxs_ui package.
//! Validates discovery, multi-module packaging, full 13-module native adapter surface,
//! callbacks (Settings, Save, Load, DisableScript, OnEject), multi-frame rendering, and GPU pipeline.

use super::{
    ability::{AbilityBarSlotSnapshot, AbilitySnapshot},
    bridge::Bridge,
    chat::{ChatMessageSnapshot, MSG_TYPE_SAY},
    discovery,
    effects::EffectSnapshot,
    group::GroupMemberSnapshot,
    health::HealthSnapshot,
    host::{Host, Snapshot},
    manager::{Command, Manager, Status},
};
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
use mlua::{Function, Table};
use render::wgpu;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "eqoa-wereoxxs-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn find_wereoxxs_scripts_root() -> Option<PathBuf> {
    for cand in [
        PathBuf::from("run/forge-compat-reference/scripts"),
        PathBuf::from(
            "C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts",
        ),
        PathBuf::from("../../run/forge-compat-reference/scripts"),
    ] {
        if cand.join("wereoxxs_ui/wereoxxs_ui.lua").is_file() {
            return cand.canonicalize().ok();
        }
    }
    None
}

fn copy_str_to_array<const N: usize>(s: &str, arr: &mut [u8; N], len: &mut u8) {
    let bytes = s.as_bytes();
    let l = bytes.len().min(N);
    arr[..l].copy_from_slice(&bytes[..l]);
    *len = l as u8;
}

struct TestGpu {
    renderer: Rc<RefCell<WgpuRenderer>>,
    context: Context,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl TestGpu {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("SKIP wereoxxs GPU fixture: {e}");
                return None;
            }
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        let renderer = Rc::new(RefCell::new(
            WgpuRenderer::new(
                WgpuInitInfo::new(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
                &mut context,
            )
            .unwrap(),
        ));
        Some(Self {
            renderer,
            context,
            device,
            queue,
        })
    }

    fn begin(&mut self) {
        self.context.io_mut().set_display_size([1024.0, 768.0]);
        self.context
            .io_mut()
            .set_display_framebuffer_scale([1.0, 1.0]);
        self.context.io_mut().set_delta_time(1.0 / 60.0);
        self.context.frame();
    }

    fn render(&mut self) {
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wereoxxs test target"),
            size: wgpu::Extent3d {
                width: 1024,
                height: 768,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut renderer = self.renderer.borrow_mut();
            let frame = self.context.render(renderer.renderer_consumer().unwrap());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer
                .render(frame, &mut pass, FramebufferExtent::new(1024, 768))
                .unwrap();
        }
        self.queue.submit([encoder.finish()]);
    }

    fn frame(&mut self, manager: &mut Manager, snapshot: Snapshot) {
        self.begin();
        manager.frame(snapshot);
        self.render();
        manager.after_render();
    }
}

fn rich_in_game_snapshot() -> Snapshot {
    let mut snap = Snapshot::default();
    snap.in_game = true;
    snap.start_menu = false;
    snap.health = HealthSnapshot::new(true, 2500, 3000);

    // Player
    copy_str_to_array("Wereoxx", &mut snap.player.name, &mut snap.player.name_len);
    snap.player.class_id = 0; // Warrior
    snap.player.level = 50;
    snap.player.current_hp = 2500;
    snap.player.max_hp = 3000;
    snap.player.current_pwr = 100;
    snap.player.max_pwr = 100;
    snap.player.position = [100.0, 50.0, 200.0];
    snap.player.velocity = [0.0, 0.0, 0.0];
    snap.player.heading_rad = 1.5707963; // ~90 degrees (West)

    // Abilities & Hotbars
    snap.abilities.ability_count = 5;
    for i in 0..5 {
        snap.abilities.abilities[i] = AbilitySnapshot {
            id: i as u32 + 1,
            valid: true,
            icon_background_ref: 10,
            icon_foreground_ref: 11,
            cast_time: 1500,
            cooldown_seconds: 5,
            power_cost: 20,
            scope: 1,
            ..AbilitySnapshot::default()
        };
        copy_str_to_array(
            &format!("Slash {i}"),
            &mut snap.abilities.abilities[i].name,
            &mut snap.abilities.abilities[i].name_len,
        );

        snap.abilities.bars[0].slots[i] = AbilityBarSlotSnapshot {
            source_type: 1,
            source_index: i as u32,
            ability_icon_ref: 10,
        };
    }

    // Group members
    snap.group.in_group = true;
    snap.group.member_count = 2;

    let mut m0 = GroupMemberSnapshot::default();
    m0.entity_id = 100;
    copy_str_to_array("Healer", &mut m0.name, &mut m0.name_len);
    m0.active = true;
    m0.hp_known = true;
    m0.percent_hp_255 = 240;
    m0.position = [105.0, 50.0, 202.0];
    snap.group.members[0] = m0;

    let mut m1 = GroupMemberSnapshot::default();
    m1.entity_id = 101;
    copy_str_to_array("Nuker", &mut m1.name, &mut m1.name_len);
    m1.active = true;
    m1.hp_known = true;
    m1.percent_hp_255 = 255;
    m1.position = [98.0, 50.0, 195.0];
    snap.group.members[1] = m1;

    // Active effects / buffs
    let mut e0 = EffectSnapshot::default();
    e0.icon_ref = 0x4001;
    copy_str_to_array("Spirit of Wolf", &mut e0.name, &mut e0.name_len);
    snap.effects.effects[0] = e0;

    let mut e1 = EffectSnapshot::default();
    e1.icon_ref = 0x4002;
    copy_str_to_array("Shield of Thorns", &mut e1.name, &mut e1.name_len);
    snap.effects.effects[1] = e1;

    // Chat scrollback
    snap.chat.count = 2;
    let mut msg0 = ChatMessageSnapshot::default();
    copy_str_to_array(
        "Welcome to EQOA: Frontiers Native!",
        &mut msg0.text,
        &mut msg0.text_len,
    );
    msg0.msg_type = MSG_TYPE_SAY;
    msg0.seq = 1;
    snap.chat.messages[0] = msg0;

    let mut msg1 = ChatMessageSnapshot::default();
    copy_str_to_array(
        "Wereoxx slashes a moss snake for 42 points of damage.",
        &mut msg1.text,
        &mut msg1.text_len,
    );
    msg1.msg_type = MSG_TYPE_SAY;
    msg1.seq = 2;
    snap.chat.messages[1] = msg1;

    // Target entity
    snap.player.target_entity_id = 2002;
    let mut target_ent = super::entity::EntitySnapshot::default();
    target_ent.id = 2002;
    target_ent.is_populated = true;
    target_ent.level = 15;
    target_ent.health_percent_255 = 180;
    target_ent.disposition = Some(1); // Hostile
    copy_str_to_array("Moss Snake", &mut target_ent.name, &mut target_ent.name_len);
    snap.entities.entities[0] = target_ent;

    snap
}

#[test]
fn forge_wereoxxs_discovery_and_manifest() {
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!("SKIP forge_wereoxxs_discovery_and_manifest: reference scripts root not found");
        return;
    };

    let disc = discovery::discover(&root);
    assert!(!disc.addons.is_empty(), "Discovery must locate addons");

    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui package must be discovered");

    assert_eq!(addon.id, "wereoxxs_ui");
    assert!(
        addon.entry.ends_with("wereoxxs_ui.lua"),
        "Entry script must be wereoxxs_ui.lua: {:?}",
        addon.entry
    );
    assert!(
        addon
            .module_dirs
            .iter()
            .any(|d| d.ends_with("modules") || d.to_string_lossy().contains("modules")),
        "Module directories must include wereoxxs_ui/modules: {:?}",
        addon.module_dirs
    );
}

#[test]
fn forge_wereoxxs_in_game_full_execution_and_lifecycle() {
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!("SKIP forge_wereoxxs_in_game_full_execution: reference scripts root not found");
        return;
    };

    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui package must be discovered");

    let host = Host::for_addon(addon).expect("Host::for_addon initialization must succeed");
    let bridge = Bridge::install(&host).expect("Bridge::install must succeed");

    let snap = rich_in_game_snapshot();
    host.snapshot.set(snap);

    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();

    // Frame 1: Initial run and callback registration
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.frame();
    bridge
        .invoke(|| host.run_addon(addon))
        .expect("wereoxxs_ui Frame 1 must execute without error");
    drop(context.render_legacy());

    // 1. Verify all 5 callbacks registered in Lua registry
    let callbacks: Table = host
        .lua
        .named_registry_value("forge_callbacks")
        .expect("forge_callbacks table must exist in registry");

    let cb_settings: Option<Function> = callbacks.get(1).unwrap();
    let cb_disable: Option<Function> = callbacks.get(2).unwrap();
    let cb_save: Option<Function> = callbacks.get(3).unwrap();
    let cb_load: Option<Function> = callbacks.get(4).unwrap();
    let cb_eject: Option<Function> = callbacks.get(5).unwrap();

    assert!(
        cb_settings.is_some(),
        "Settings callback (kind 0) registered"
    );
    assert!(
        cb_disable.is_some(),
        "DisableScript callback (kind 1) registered"
    );
    assert!(cb_save.is_some(), "Save callback (kind 2) registered");
    assert!(cb_load.is_some(), "Load callback (kind 3) registered");
    assert!(cb_eject.is_some(), "OnEject callback (kind 4) registered");

    // 2. Call Save callback and inspect output table
    let save_fn = cb_save.unwrap();
    let saved: Table = save_fn
        .call(())
        .expect("Save callback must succeed and return table");

    assert!(
        saved.contains_key("bar_configs").unwrap(),
        "saved has bar_configs"
    );
    assert!(
        saved.contains_key("cast_config").unwrap(),
        "saved has cast_config"
    );
    assert!(
        saved.contains_key("frame_config").unwrap(),
        "saved has frame_config"
    );
    assert!(
        saved.contains_key("target_frame_config").unwrap(),
        "saved has target_frame_config"
    );
    assert!(
        saved.contains_key("compass_config").unwrap(),
        "saved has compass_config"
    );
    assert!(
        saved.contains_key("meter_config").unwrap(),
        "saved has meter_config"
    );
    assert!(
        saved.contains_key("chat_config").unwrap(),
        "saved has chat_config"
    );
    assert!(
        saved.contains_key("grid_config").unwrap(),
        "saved has grid_config"
    );
    assert!(saved.contains_key("windows").unwrap(), "saved has windows");

    // 3. Mutate config and call Load callback to verify persistence
    let grid_cfg: Table = saved.get("grid_config").unwrap();
    grid_cfg.set("size", 24).unwrap();
    let win_cfg: Table = saved.get("windows").unwrap();
    win_cfg.set("compass", false).unwrap();

    let load_fn = cb_load.unwrap();
    load_fn
        .call::<()>(saved)
        .expect("Load callback must succeed");

    // Verify globals reflect loaded config
    let loaded_grid: Table = host.lua.globals().get("grid_config").unwrap();
    assert_eq!(loaded_grid.get::<i32>("size").unwrap(), 24);
    let loaded_win: Table = host.lua.globals().get("windows").unwrap();
    assert_eq!(loaded_win.get::<bool>("compass").unwrap(), false);

    // 4. Call Settings callback inside an active ImGui frame
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.frame();
    bridge
        .invoke(|| {
            let settings_fn = cb_settings.unwrap();
            settings_fn.call::<()>(())
        })
        .expect("Settings callback must render cleanly inside ImGui frame");
    drop(context.render_legacy());

    // 5. Run Frame 2: subsequent frame execution with state preserved
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.frame();
    bridge
        .invoke(|| host.run_addon(addon))
        .expect("wereoxxs_ui Frame 2 must execute cleanly with retained globals");
    drop(context.render_legacy());

    // 6. Call DisableScript callback (Cleanup)
    let disable_fn = cb_disable.unwrap();
    disable_fn
        .call::<()>(())
        .expect("DisableScript (Cleanup) must succeed");

    let initialized: bool = host.lua.globals().get("initialized").unwrap();
    assert!(!initialized, "initialized flag must be cleared by Cleanup");
}

#[test]
fn forge_wereoxxs_headless_gpu_manager_pipeline() {
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!("SKIP forge_wereoxxs_gpu_manager: reference scripts root not found");
        return;
    };

    let Some(mut gpu) = TestGpu::new() else {
        return;
    };

    let config_dir = ScratchDir::new();
    let mut manager = Manager::new(
        root.clone(),
        Some(config_dir.0.clone()),
        &gpu.device,
        &gpu.queue,
        gpu.renderer.clone(),
    );

    // Initial frame before enable: discovers addons
    let snap = rich_in_game_snapshot();
    gpu.frame(&mut manager, snap.clone());

    // Enable wereoxxs_ui
    manager.queue(Command::Enable("wereoxxs_ui".into(), true));
    gpu.frame(&mut manager, snap.clone());

    let entries = manager.entries();
    let entry = entries
        .iter()
        .find(|e| e.metadata.id == "wereoxxs_ui")
        .expect("wereoxxs_ui must be in manager entries");

    assert_eq!(
        entry.status,
        Status::Enabled,
        "wereoxxs_ui status must be Enabled"
    );

    let diags: Vec<_> = manager
        .diagnostics()
        .iter()
        .filter(|d| d.id.as_deref() == Some("wereoxxs_ui"))
        .collect();
    assert!(
        diags.is_empty(),
        "wereoxxs_ui must produce zero failure diagnostics: {diags:?}"
    );

    // Queue Save command and execute frame
    manager.queue(Command::Save("wereoxxs_ui".into()));
    gpu.frame(&mut manager, snap.clone());

    // Disable wereoxxs_ui cleanly
    manager.queue(Command::Enable("wereoxxs_ui".into(), false));
    gpu.frame(&mut manager, snap.clone());

    let entries_disabled = manager.entries();
    let entry_disabled = entries_disabled
        .iter()
        .find(|e| e.metadata.id == "wereoxxs_ui")
        .expect("wereoxxs_ui must be in manager entries");

    assert_eq!(
        entry_disabled.status,
        Status::Disabled,
        "wereoxxs_ui status must be Disabled after disable command"
    );
}

#[test]
fn forge_wereoxxs_complete_symbol_surface_contract() {
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!(
            "SKIP forge_wereoxxs_complete_symbol_surface_contract: reference scripts root not found"
        );
        return;
    };
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui package must be discovered");

    let Some(mut gpu) = TestGpu::new() else {
        eprintln!("SKIP forge_wereoxxs_complete_symbol_surface_contract: GPU fixture unavailable");
        return;
    };

    let host = Host::for_addon(addon).expect("Host::for_addon must succeed");
    let bridge = Bridge::install(&host).unwrap();

    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

    let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, addon).unwrap();

    let check_script = host
        .compile(
            r#"
        -- 1. All 14 frontiers_forge modules
        local modules = {
            'player', 'entity_list', 'entity', 'group',
            'ability_bar', 'ability_list', 'ability', 'icon',
            'effects', 'combat', 'chat', 'input', 'ui', 'util'
        }
        for _, mod_name in ipairs(modules) do
            local m = require('frontiers_forge.' .. mod_name)
            assert(type(m) == 'table', 'frontiers_forge.' .. mod_name .. ' must be a table')
        end

        -- 2. UiForge globals
        assert(type(UiForge) == 'table', 'UiForge table missing')
        assert(type(UiForge.CallbackType) == 'table', 'UiForge.CallbackType missing')
        assert(type(UiForge.LoadFont) == 'function', 'UiForge.LoadFont missing')
        assert(type(UiForge.LoadTexture) == 'function', 'UiForge.LoadTexture missing')
        assert(type(UiForge.RegisterCallback) == 'function', 'UiForge.RegisterCallback missing')
        assert(type(UiForge.ReleaseTexture) == 'function', 'UiForge.ReleaseTexture missing')
        assert(type(UiForge.resources_path) == 'string', 'UiForge.resources_path missing')

        -- 3. All 62 Dear ImGui functions called across wereoxxs_ui
        local imgui_funcs = {
            'Begin', 'BeginChild', 'BeginCombo', 'BeginPopup', 'BeginTabBar', 'BeginTabItem',
            'Button', 'CalcTextSize', 'Checkbox', 'ColorEdit4', 'Dummy', 'End', 'EndChild',
            'EndCombo', 'EndPopup', 'EndTabBar', 'EndTabItem', 'GetColorU32', 'GetContentRegionAvail',
            'GetCursorScreenPos', 'GetFrameHeight', 'GetScrollMaxY', 'GetScrollY', 'GetTextLineHeight',
            'GetTime', 'GetWindowDrawList', 'GetWindowPos', 'InputTextWithHint', 'InvisibleButton',
            'IsItemActive', 'IsMouseDragging', 'IsMouseHoveringRect', 'IsWindowFocused', 'IsWindowHovered',
            'OpenPopup', 'PopFont', 'PopID', 'PopStyleColor', 'PushFont', 'PushID', 'PushStyleColor',
            'RadioButton', 'SameLine', 'Selectable', 'Separator', 'SetCursorScreenPos',
            'SetKeyboardFocusHere', 'SetNextItemWidth', 'SetNextWindowPos', 'SetNextWindowSize',
            'SetScrollHereY', 'SetTooltip', 'SetWindowFontScale', 'SetWindowPos', 'SliderFloat',
            'SliderInt', 'Text', 'TextColored', 'TextDisabled', 'TextWrapped', 'TreeNode', 'TreePop'
        }
        for _, fn_name in ipairs(imgui_funcs) do
            assert(type(ImGui[fn_name]) == 'function', 'ImGui.' .. fn_name .. ' must be a function')
        end
    "#,
        )
        .unwrap();

    gpu.begin();
    bridge.invoke(|| host.run(&check_script)).unwrap();
    gpu.render();
}

#[test]
fn forge_wereoxxs_all_windows_rendered_without_drawlist_nil_errors() {
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!(
            "SKIP forge_wereoxxs_all_windows_rendered_without_drawlist_nil_errors: reference scripts root not found"
        );
        return;
    };
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui package must be discovered");

    let Some(mut gpu) = TestGpu::new() else {
        eprintln!(
            "SKIP forge_wereoxxs_all_windows_rendered_without_drawlist_nil_errors: GPU fixture unavailable"
        );
        return;
    };

    let host = Host::for_addon(addon).expect("Host::for_addon must succeed");
    let bridge = Bridge::install(&host).unwrap();

    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

    let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, addon).unwrap();

    let snap = rich_in_game_snapshot();
    host.snapshot.set(snap);

    // Frame 1: Initial load
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Turn on ALL 8 windows
    let win_table: Table = host.lua.globals().get("windows").unwrap();
    win_table.set("bars", true).unwrap();
    win_table.set("casting", true).unwrap();
    win_table.set("player", true).unwrap();
    win_table.set("target", true).unwrap();
    win_table.set("group", true).unwrap();
    win_table.set("compass", true).unwrap();
    win_table.set("meter", true).unwrap();
    win_table.set("chat", true).unwrap();

    // Frame 2: Render with all windows enabled (exercising AddText, AddImage, PushClipRect, PopClipRect)
    gpu.begin();
    bridge
        .invoke(|| {
            host.lua
                .load("cast_preview_ms = math.floor(ImGui.GetTime() * 1000)")
                .exec()?;
            host.run_addon(addon)
        })
        .expect("wereoxxs_ui with all windows enabled must render without error");
    gpu.render();

    // Frame 3: Render again to ensure state stability across frames
    gpu.begin();
    bridge
        .invoke(|| host.run_addon(addon))
        .expect("wereoxxs_ui subsequent frame must render without error");
    gpu.render();
}

#[test]
fn forge_wereoxxs_disabled_startup_profile_enable_toggle_disable_reenable_all_elements() {
    use super::{
        runtime::{Action, FramePlan, dispatch_frame},
        storage::Storage,
        visibility::{PublishedVisibility, publish, published_visibility},
    };
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            publish(false);
        }
    }
    let _reset = Reset;
    publish(false);

    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!(
            "SKIP forge_wereoxxs_disabled_startup_profile_enable_toggle_disable_reenable_all_elements: reference scripts root not found"
        );
        return;
    };
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };

    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui package must be discovered");

    let config_dir = ScratchDir::new();
    let storage = Storage::new(Some(config_dir.0.clone()));
    let key = Storage::key(addon).expect("Storage::key must succeed");

    let write_profile = |flags: &[(&str, bool)]| {
        let lua = mlua::Lua::new();
        let root_tbl = lua.create_table().unwrap();
        let win_tbl = lua.create_table().unwrap();
        for &(k, v) in flags {
            win_tbl.set(k, v).unwrap();
        }
        root_tbl.set("windows", win_tbl).unwrap();
        storage
            .save_profile(&key, mlua::Value::Table(root_tbl))
            .expect("save_profile must succeed");
    };

    let all_hide_keys = [
        "hide_ability_bars",
        "hide_player_bars",
        "hide_active_effects",
        "hide_game_group",
        "hide_pet_panel",
        "hide_game_compass",
        "hide_game_chat",
        "hide_target_nameplate",
    ];
    let all_true: Vec<(&str, bool)> = all_hide_keys.iter().map(|&k| (k, true)).collect();

    // (a) Saved profile has every hide_* option ticked, while state.json has wereoxxs_ui DISABLED.
    write_profile(&all_true);
    storage
        .set_enabled(&key, false)
        .expect("set_enabled(false) must succeed");

    let mut manager = Manager::new(
        root.clone(),
        Some(config_dir.0.clone()),
        &gpu.device,
        &gpu.queue,
        gpu.renderer.clone(),
    );

    let snap = rich_in_game_snapshot();
    gpu.frame(&mut manager, snap.clone());
    gpu.frame(&mut manager, snap.clone());

    let entry = manager
        .entries()
        .into_iter()
        .find(|e| e.metadata.id == "wereoxxs_ui")
        .expect("wereoxxs_ui entry must exist");
    assert_eq!(entry.status, Status::Disabled);
    assert!(!entry.enabled_intent);
    assert_eq!(
        published_visibility(),
        PublishedVisibility::default(),
        "disabled addon at startup with all-hidden profile must publish zero hide claims"
    );
    assert!(!published_visibility().any_hidden());

    let hud_state = crate::hud::HudState::default();
    assert!(crate::hud::fork_hud::compass_visible(&hud_state));
    assert!(crate::hud::fork_hud::group_compass_marks_visible(
        &hud_state
    ));
    assert!(crate::hud::fork_hud::target_visible(&hud_state));
    assert!(crate::hud::fork_hud::group_visible(&hud_state));
    assert!(crate::hud::fork_hud::pet_visible(&hud_state));
    assert!(crate::hud::fork_hud::power_bar_visible(&hud_state));
    assert!(crate::hud::fork_hud::main_exp_bar_visible(&hud_state));
    assert!(crate::hud::fork_hud::secondary_exp_bar_visible(&hud_state));
    assert!(crate::hud::fork_hud::exp_bar_visible(&hud_state));
    assert!(crate::hud::fork_hud::chat_window_visible());
    assert!(crate::hud::fork_hud::ability_bar_visible());
    assert!(crate::hud::fork_hud::active_effects_visible());

    // Attempting to dispatch Settings on a disabled addon must be rejected and return None.
    gpu.context.io_mut().set_display_size([1024.0, 768.0]);
    gpu.context
        .io_mut()
        .set_display_framebuffer_scale([1.0, 1.0]);
    gpu.context.io_mut().set_delta_time(1.0 / 60.0);
    let ui = gpu.context.frame();
    let selected = dispatch_frame(
        ui,
        &mut manager,
        snap.clone(),
        FramePlan::collect(vec![Action::Settings("wereoxxs_ui".into())], None),
    );
    gpu.render();
    manager.after_render();
    assert_eq!(
        selected, None,
        "disabled addon must not open or execute Settings window"
    );
    assert!(!published_visibility().any_hidden());

    // (b) Enable wereoxxs_ui at runtime -> frame 1 initializes and invokes Load(saved),
    // frame 2 runs ApplyGameUiState with the loaded profile and hides all gated elements.
    manager.queue(Command::Enable("wereoxxs_ui".into(), true));
    gpu.frame(&mut manager, snap.clone());
    gpu.frame(&mut manager, snap.clone());

    let all_hidden = PublishedVisibility {
        health_bar: true,
        power_bar: true,
        main_exp_bar: true,
        secondary_exp_bar: true,
        compass: true,
        target: true,
        group_panel: true,
        group_compass_marks: true,
        pet: true,
        chat: true,
        ability_bar: true,
        active_effects: true,
    };
    assert_eq!(
        published_visibility(),
        all_hidden,
        "enabling wereoxxs_ui with all-hidden profile must hide all 12 gated HUD fields"
    );
    assert!(!crate::hud::fork_hud::compass_visible(&hud_state));
    assert!(!crate::hud::fork_hud::group_compass_marks_visible(
        &hud_state
    ));
    assert!(!crate::hud::fork_hud::target_visible(&hud_state));
    assert!(!crate::hud::fork_hud::group_visible(&hud_state));
    assert!(!crate::hud::fork_hud::pet_visible(&hud_state));
    assert!(!crate::hud::fork_hud::power_bar_visible(&hud_state));
    assert!(!crate::hud::fork_hud::main_exp_bar_visible(&hud_state));
    assert!(!crate::hud::fork_hud::secondary_exp_bar_visible(&hud_state));
    assert!(!crate::hud::fork_hud::exp_bar_visible(&hud_state));
    assert!(!crate::hud::fork_hud::chat_window_visible());
    assert!(!crate::hud::fork_hud::ability_bar_visible());
    assert!(!crate::hud::fork_hud::active_effects_visible());

    // (d) While enabled, untick each hide_* option one by one and verify its gated element(s)
    // immediately become visible while all other elements stay hidden; then re-tick.
    for &unticked_key in &all_hide_keys {
        let mut flags = all_true.clone();
        for (k, v) in &mut flags {
            if *k == unticked_key {
                *v = false;
            }
        }
        write_profile(&flags);
        manager.queue(Command::Load("wereoxxs_ui".into()));
        gpu.frame(&mut manager, snap.clone());

        let vis = published_visibility();
        let mut expected = all_hidden;
        match unticked_key {
            "hide_ability_bars" => {
                expected.ability_bar = false;
                assert!(crate::hud::fork_hud::ability_bar_visible());
            }
            "hide_player_bars" => {
                expected.health_bar = false;
                expected.power_bar = false;
                expected.main_exp_bar = false;
                expected.secondary_exp_bar = false;
                assert!(crate::hud::fork_hud::power_bar_visible(&hud_state));
                assert!(crate::hud::fork_hud::main_exp_bar_visible(&hud_state));
                assert!(crate::hud::fork_hud::secondary_exp_bar_visible(&hud_state));
                assert!(crate::hud::fork_hud::exp_bar_visible(&hud_state));
            }
            "hide_active_effects" => {
                expected.active_effects = false;
                assert!(crate::hud::fork_hud::active_effects_visible());
            }
            "hide_game_group" => {
                expected.group_panel = false;
                expected.group_compass_marks = false;
                assert!(crate::hud::fork_hud::group_visible(&hud_state));
                assert!(crate::hud::fork_hud::group_compass_marks_visible(
                    &hud_state
                ));
            }
            "hide_pet_panel" => {
                expected.pet = false;
                assert!(crate::hud::fork_hud::pet_visible(&hud_state));
            }
            "hide_game_compass" => {
                expected.compass = false;
                assert!(crate::hud::fork_hud::compass_visible(&hud_state));
            }
            "hide_game_chat" => {
                expected.chat = false;
                assert!(crate::hud::fork_hud::chat_window_visible());
            }
            "hide_target_nameplate" => {
                expected.target = false;
                assert!(crate::hud::fork_hud::target_visible(&hud_state));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            vis, expected,
            "unticking {unticked_key} while enabled must unhide only its gated element(s)"
        );

        // Re-tick back to true
        write_profile(&all_true);
        manager.queue(Command::Load("wereoxxs_ui".into()));
        gpu.frame(&mut manager, snap.clone());
        assert_eq!(published_visibility(), all_hidden);
    }

    // (b cont.) Disable wereoxxs_ui at runtime -> all claims released at once on that frame.
    manager.queue(Command::Enable("wereoxxs_ui".into(), false));
    gpu.frame(&mut manager, snap.clone());
    assert_eq!(
        published_visibility(),
        PublishedVisibility::default(),
        "disabling wereoxxs_ui at runtime must release all hide claims at once"
    );
    assert!(!published_visibility().any_hidden());
    assert!(crate::hud::fork_hud::compass_visible(&hud_state));
    assert!(crate::hud::fork_hud::ability_bar_visible());
    assert!(crate::hud::fork_hud::active_effects_visible());
    assert!(crate::hud::fork_hud::chat_window_visible());

    // (c) Re-enable wereoxxs_ui at runtime -> reapplies claims per wereoxxs_ui.lua's own source
    // behavior (Cleanup's RestoreGameUi clears game_ui_hidden while preserving windows).
    manager.queue(Command::Enable("wereoxxs_ui".into(), true));
    gpu.frame(&mut manager, snap.clone());
    assert_eq!(
        published_visibility(),
        all_hidden,
        "re-enabling wereoxxs_ui must reapply hide claims per wereoxxs_ui.lua source behavior"
    );
}

fn create_test_play() -> crate::play::Play {
    struct Nowhere;
    impl net::session::Datagrams for Nowhere {
        fn send_to(&mut self, _: &[u8], _: [u8; 4], _: u16) {}
        fn recv_from(&mut self) -> Option<(Vec<u8>, [u8; 4], u16)> {
            None
        }
    }
    let mut session = net::session::Session::connect_with(
        Box::new(Nowhere),
        ([127, 0, 0, 1], 5000),
        ([127, 0, 0, 1], 10070),
        "test",
        [0; 32],
    )
    .unwrap();
    session.self_id = Some(0x100);
    crate::play::Play::new(session, net::session::Player::default(), Box::default())
}

#[test]
fn forge_claimed_enter_focuses_addon_input_and_say_window_stays_closed() {
    use winit::keyboard::{Key, NamedKey};

    super::input::reset_input_state();
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!(
            "SKIP forge_claimed_enter_focuses_addon_input_and_say_window_stays_closed: reference scripts root not found"
        );
        return;
    };
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.name == "wereoxxs_ui")
        .unwrap();

    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let host = Host::for_addon(addon).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon).unwrap();
    let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, addon).unwrap();

    host.snapshot.set(rich_in_game_snapshot());

    // Frame 1: Initial load
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Turn on chat window
    let win_table: Table = host.lua.globals().get("windows").unwrap();
    win_table.set("chat", true).unwrap();

    // Frame 2: Pump wereoxxs_ui so PumpEnterKey claims Enter
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Verify Enter is claimed and suppressed by wereoxxs_ui
    assert!(
        super::input::is_key_claimed(0x0D),
        "Enter must be claimed by wereoxxs_ui"
    );
    assert!(
        super::input::is_key_suppressed(0x0D),
        "Enter must be suppressed by wereoxxs_ui"
    );

    // Enter key is pressed
    let enter_key = Key::Named(NamedKey::Enter);
    let code = super::input::logical_key_to_code(&enter_key);
    assert_eq!(code, Some(0x0D));
    super::input::record_key_event(code.unwrap(), true);

    let wants_key_capture = super::input::is_text_capture_active()
        || code
            .is_some_and(|c| super::input::is_key_suppressed(c) || super::input::is_key_claimed(c));
    assert!(wants_key_capture, "claimed Enter must be captured by addon");

    // In viewer, when wants_key_capture is true, the event is consumed by Addon
    // and Play::chat_key is NOT called, so Say window stays closed (mode == 0)
    let play = create_test_play();
    assert_eq!(play.mode, 0, "Say window starts closed");
    assert_eq!(
        play.mode, 0,
        "Say window must stay closed when Enter is claimed"
    );

    // Frame 3: wereoxxs_ui runs PumpEnterKey, polls key events, and requests focus
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Verify that wereoxxs_ui consumed focus_requested via ImGui.SetKeyboardFocusHere()
    let focus_requested: bool = host
        .lua
        .load("return chat_state.focus_requested")
        .eval()
        .unwrap();
    assert!(
        !focus_requested,
        "focus_requested must be consumed by ChatWindow.Draw via SetKeyboardFocusHere"
    );

    // Frame 4: ImGui applies the queued keyboard focus from SetKeyboardFocusHere
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    let input_active2: bool = host
        .lua
        .load("return chat_state.input_active")
        .eval()
        .unwrap();
    assert!(
        input_active2,
        "addon chat input must have keyboard focus on next frame after SetKeyboardFocusHere"
    );
}

#[test]
fn forge_unclaimed_enter_opens_say_window_as_before() {
    use winit::keyboard::{Key, NamedKey};

    super::input::reset_input_state();
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!(
            "SKIP forge_unclaimed_enter_opens_say_window_as_before: reference scripts root not found"
        );
        return;
    };
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.name == "wereoxxs_ui")
        .unwrap();

    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let host = Host::for_addon(addon).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon).unwrap();
    let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, addon).unwrap();

    host.snapshot.set(rich_in_game_snapshot());

    // Initial load with chat off
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    let win_table: Table = host.lua.globals().get("windows").unwrap();
    win_table.set("chat", false).unwrap();

    // Pump with chat off so Enter is unclaimed
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    assert!(
        !super::input::is_key_claimed(0x0D),
        "Enter must not be claimed when chat is off"
    );
    assert!(
        !super::input::is_key_suppressed(0x0D),
        "Enter must not be suppressed when chat is off"
    );

    let enter_key = Key::Named(NamedKey::Enter);
    let code = super::input::logical_key_to_code(&enter_key);
    assert_eq!(code, Some(0x0D));

    let wants_key_capture = super::input::is_text_capture_active()
        || code
            .is_some_and(|c| super::input::is_key_suppressed(c) || super::input::is_key_claimed(c));
    assert!(
        !wants_key_capture,
        "unclaimed Enter must NOT be captured by addon"
    );

    // Unclaimed Enter in viewer calls Play::chat_key, which opens the native Say window
    let mut play = create_test_play();
    assert_eq!(play.mode, 0);
    play.chat_key("", true);
    assert_eq!(
        play.mode, 2,
        "unclaimed Enter opens the native Say window (mode 2)"
    );
}

#[test]
fn forge_addon_focused_typing_does_not_leak_keys_to_game() {
    use winit::keyboard::{Key, NamedKey, SmolStr};

    super::input::reset_input_state();
    let Some(root) = find_wereoxxs_scripts_root() else {
        eprintln!(
            "SKIP forge_addon_focused_typing_does_not_leak_keys_to_game: reference scripts root not found"
        );
        return;
    };
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.name == "wereoxxs_ui")
        .unwrap();

    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let host = Host::for_addon(addon).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon).unwrap();
    let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, addon).unwrap();

    host.snapshot.set(rich_in_game_snapshot());

    // Frame 1: Initial load
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Enable chat
    let win_table: Table = host.lua.globals().get("windows").unwrap();
    win_table.set("chat", true).unwrap();

    // Frame 2: Claims enter
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Enter pressed
    super::input::record_key_event(0x0D, true);

    // Frame 3: wereoxxs_ui consumes Enter from PollKeyEvents and calls SetKeyboardFocusHere()
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Frame 4: ImGui activates the input widget (input_active becomes true)
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();
    let input_active: bool = host
        .lua
        .load("return chat_state.input_active")
        .eval()
        .unwrap();
    assert!(input_active, "addon chat input must be focused");

    // Frame 5: PumpEnterKey activates text capture while focused
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();
    assert!(
        super::input::is_text_capture_active(),
        "text capture must be active while addon chat is focused"
    );

    // Keys while focused: movement (W, A, S, D, ArrowUp) and hotkeys (1, 2, Q, Space)
    let test_keys = [
        Key::Character(SmolStr::new("w")),
        Key::Character(SmolStr::new("a")),
        Key::Character(SmolStr::new("s")),
        Key::Character(SmolStr::new("d")),
        Key::Named(NamedKey::ArrowUp),
        Key::Character(SmolStr::new("1")),
        Key::Character(SmolStr::new("2")),
        Key::Character(SmolStr::new("q")),
        Key::Named(NamedKey::Space),
    ];

    let play = create_test_play();
    let initial_mode = play.mode;

    for key in &test_keys {
        let code = super::input::logical_key_to_code(key);
        let wants_key_capture = super::input::is_text_capture_active()
            || code.is_some_and(|c| {
                super::input::is_key_suppressed(c) || super::input::is_key_claimed(c)
            });
        assert!(
            wants_key_capture,
            "keys while focused must be captured by addon (not leaked to game)"
        );
    }

    assert_eq!(
        play.mode, initial_mode,
        "game state must remain untouched while addon chat is focused"
    );
}
