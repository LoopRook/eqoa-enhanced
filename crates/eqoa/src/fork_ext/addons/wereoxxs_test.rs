// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Full integration test suite for the unchanged wereoxxs_ui package.
//! Validates discovery, multi-module packaging, full 13-module native adapter surface,
//! callbacks (Settings, Save, Load, DisableScript, OnEject), multi-frame rendering, and GPU pipeline.

use super::{
    ability::AbilityBarSlotSnapshot,
    bridge::Bridge,
    chat::{ChatMessageSnapshot, MSG_TYPE_SAY},
    discovery,
    host::{Host, Snapshot},
    manager::{Command, Manager, Status},
};
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
use mlua::{Function, Table};
use render::wgpu;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

struct ScratchDir(PathBuf);

#[test]
fn forge_review_reference_requires_explicit_environment() {
    if std::env::var_os("EQOA_FORGE_REFERENCE").is_none() {
        assert!(
            std::panic::catch_unwind(find_wereoxxs_scripts_root).is_err(),
            "unset reference must fail rather than silently find a developer folder"
        );
    }
}

fn modern_package(id: &str) {
    let root = find_wereoxxs_scripts_root();
    modern_package_at(id, root);
}
fn modern_package_at(id: &str, root: PathBuf) {
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let addon = discovery::discover(&root)
        .addons
        .into_iter()
        .find(|a| a.id == id)
        .unwrap();
    let host = Host::for_addon(&addon).unwrap();
    let data = ScratchDir::new();
    super::file_io::install(&host, &addon, Some(data.0.clone())).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, &addon).unwrap();
    let fonts = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&fonts, &host.lua, &addon).unwrap();
    let snapshot = rich_in_game_snapshot();
    host.snapshot.set(snapshot);
    for _ in 0..4 {
        gpu.begin();
        let result = bridge.invoke(|| host.run_addon(&addon));
        gpu.render();
        let logs = host.drain_logs().unwrap();
        assert!(
            result.is_ok()
                && logs
                    .iter()
                    .all(|l| serde_json::from_str::<serde_json::Value>(l)
                        .is_ok_and(|d| d["code"] == "not_found")),
            "{id}: {result:?}, logs: {logs:?}"
        );
    }
    gpu.begin();
    let result = bridge.invoke(|| {
        host.execute("ImGui.Begin('settings')")?;
        host.callback(0, mlua::Value::Nil)?;
        if id == "avoids_modern_ui" {
            host.execute("for key,mod in pairs(ModPack) do if mod.Settings then ImGui.PushID(key);mod.Settings();ImGui.PopID() end end")?;
        }
        host.execute("ImGui.End()")
    });
    gpu.render();
    result.unwrap();
    assert!(
        host.drain_logs().unwrap().is_empty(),
        "{id}: swallowed settings error"
    );
    let saved = host.callback(2, mlua::Value::Nil).unwrap();
    host.callback(3, saved).unwrap();
    host.callback(1, mlua::Value::Nil).unwrap();
    host.callback(4, mlua::Value::Nil).unwrap();
    assert!(
        host.drain_logs().unwrap().is_empty(),
        "{id}: lifecycle error"
    );
    let diags = textures.borrow_mut().drain_diagnostics();
    assert!(
        diags.iter().all(|d| !d.contains("unsupported")),
        "{diags:?}"
    );
}

fn bundled_tree() -> ScratchDir {
    let root = ScratchDir::new();
    for file in crate::fork_ext::bundled::EMBEDDED_FILES {
        let path = root.0.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.data).unwrap();
    }
    root
}

#[test]
fn native_shared_bundled_fonts_and_gear_image_use_real_assets_and_issued_aliases() {
    let root = bundled_tree();
    let addon = discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "wereoxxs_ui")
        .unwrap();
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let host = Host::for_addon(&addon).unwrap();
    let _bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, &addon).unwrap();
    let fonts = Rc::new(RefCell::new(super::fonts::FontBank::new(
        1,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&fonts, &host.lua, &addon).unwrap();
    host.execute(r#"fonts={};aliases={};for i,path in ipairs({'fonts/Cinzel/static/Cinzel-Regular.ttf','fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf','fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf'}) do fonts[i]=assert(UiForge.LoadFont('resources/'..path,16));assert(tostring(fonts[i]):find('pending'));aliases[i]=assert(UiForge.LoadFont(UiForge.resources_path..'\\'..path:gsub('/','\\'),16));assert(tostring(aliases[i]):find('pending')) end;local t,w,h=UiForge.IGraphicsApi.CreateTextureFromFile(UiForge.resources_path..'\\gear-icon.png');assert(t~=nil and w>0 and h>0);local info=assert(eqoa.textures.load('resources/gear-icon.png'));assert(info.width==w and info.height==h)"#).unwrap();
    let loaded: Table = host.lua.globals().get("fonts").unwrap();
    let aliases: Table = host.lua.globals().get("aliases").unwrap();
    for i in 1..=3 {
        let a = loaded.get::<mlua::AnyUserData>(i).unwrap();
        let b = aliases.get::<mlua::AnyUserData>(i).unwrap();
        assert!(Rc::ptr_eq(
            &a.borrow::<super::fonts::LuaImFont>().unwrap().state,
            &b.borrow::<super::fonts::LuaImFont>().unwrap().state
        ));
    }
    fonts.borrow_mut().flush_pending(&mut gpu.context);
    host.execute("for _,font in ipairs(fonts) do assert(tostring(font):find('ready'),'shared font must not fall back') end").unwrap();
}

#[test]
fn native_modern_real_migration_is_private_persistent_and_isolated() {
    let root = bundled_tree();
    let config = ScratchDir::new();
    let addons = discovery::discover(&root.0).addons;
    let modern = addons.iter().find(|a| a.id == "avoids_modern_ui").unwrap();
    let neighbor = addons.iter().find(|a| a.id == "arpg_ui").unwrap();
    let paths = [
        "modern_ui_mods_settings.cfg",
        "ability_bar_settings.cfg",
        "chat_log_settings.cfg",
        "effects_settings.cfg",
        "exp_bars/exp_bars_settings.cfg",
        "group_frames_settings.cfg",
        "retro_health_hearts/health_bar_settings.cfg",
        "mana_pool/mana_bar_settings.cfg",
        "pet_frame_settings.cfg",
        "quest_log_settings.cfg",
        "quest_log_notes.cfg",
    ];
    for path in paths {
        let file = root.0.join("resources").join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "size=31\n").unwrap();
    }
    let host = Host::for_addon(modern).unwrap();
    super::file_io::install(&host, modern, Some(config.0.join("modern"))).unwrap();
    host.execute(r#"persist=require('avoids_persist');for _,path in ipairs({'modern_ui_mods_settings.cfg','ability_bar_settings.cfg','chat_log_settings.cfg','effects_settings.cfg','exp_bars/exp_bars_settings.cfg','group_frames_settings.cfg','retro_health_hearts/health_bar_settings.cfg','mana_pool/mana_bar_settings.cfg','pet_frame_settings.cfg','quest_log_settings.cfg','quest_log_notes.cfg'}) do local state={size=0};assert(persist.MigrateLegacyFile(UiForge.resources_path..'\\'..path:gsub('/','\\'),state,{{key='size',type='number'}}));assert(state.size==31);assert(io.open(UiForge.resources_path..'/'..path)==nil) end;assert(eqoa.files.write('package_resources/quest_log_notes.cfg','private notes'));assert(eqoa.files.read('package_resources/quest_log_notes.cfg')=='private notes')"#).unwrap();
    drop(host);
    let reloaded = Host::for_addon(modern).unwrap();
    super::file_io::install(&reloaded, modern, Some(config.0.join("modern"))).unwrap();
    reloaded.execute("assert(eqoa.files.read('resources/ability_bar_settings.cfg')==nil);assert(eqoa.files.read('package_resources/quest_log_notes.cfg')=='private notes')").unwrap();
    let other = Host::for_addon(neighbor).unwrap();
    super::file_io::install(&other, neighbor, Some(config.0.join("other"))).unwrap();
    other.execute("assert(eqoa.files.read('resources/ability_bar_settings.cfg')=='size=31\\n');assert(eqoa.files.write('resources/same.cfg','neighbor'))").unwrap();
    reloaded.execute("assert(eqoa.files.write('resources/same.cfg','modern'));assert(eqoa.files.read('resources/same.cfg')=='modern');assert(os.rename(UiForge.resources_path..'/same.cfg',UiForge.resources_path..'/renamed.cfg'));assert(eqoa.files.read('resources/same.cfg')==nil)").unwrap();
    other
        .execute("assert(eqoa.files.read('resources/same.cfg')=='neighbor')")
        .unwrap();
    for path in paths {
        assert_eq!(
            std::fs::read_to_string(root.0.join("resources").join(path)).unwrap(),
            "size=31\n"
        );
    }
}
#[test]
fn native_bundled_arpg_and_modern_scripts_load_save_load_and_disable_unchanged() {
    let root = bundled_tree();
    for id in ["arpg_ui", "avoids_modern_ui", "wereoxxs_ui"] {
        modern_package_at(id, root.0.canonicalize().unwrap());
    }
}
#[test]
fn native_bundled_hearts_once_mode_draws_and_releases_its_health_claim() {
    use super::native_core::{CORE, Core, View};
    CORE.with(|c| {
        let mut core = Core::default();
        core.publish(View {
            in_world: true,
            phase: "world",
            world_id: Some(0),
            player: serde_json::json!({"id":256,"hp":{"current":50,"maximum":100,"base":100}}),
            ..Default::default()
        });
        *c.borrow_mut() = core;
    });
    let root = bundled_tree();
    let config = ScratchDir::new();
    let addon = discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_example_hearts")
        .unwrap();
    let store = super::storage::Storage::new(Some(config.0.clone()));
    let key = super::storage::Storage::key(&addon).unwrap();
    let profile_host = Host::new().unwrap();
    store
        .save_profile(
            &key,
            profile_host
                .lua
                .load("return {x=180,y=100,size=18}")
                .eval()
                .unwrap(),
        )
        .unwrap();
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let mut manager = Manager::new(
        root.0.clone(),
        Some(config.0.clone()),
        &gpu.device,
        &gpu.queue,
        gpu.renderer.clone(),
    );
    manager.queue(Command::Enable("eqoa_example_hearts".into(), true));
    let snapshot = rich_in_game_snapshot();
    for _ in 0..3 {
        gpu.begin();
        manager.frame(snapshot);
        let vertices = gpu.render();
        manager.after_render();
        assert!(vertices > 0);
    }
    assert!(manager.health_hidden());
    assert!(
        manager.diagnostics().is_empty(),
        "{:?}",
        manager.diagnostics()
    );
    manager.queue(Command::Save("eqoa_example_hearts".into()));
    manager.queue(Command::Load("eqoa_example_hearts".into()));
    manager.queue(Command::Enable("eqoa_example_hearts".into(), false));
    gpu.begin();
    manager.frame(snapshot);
    gpu.render();
    manager.after_render();
    assert!(!manager.health_hidden());
    manager.queue(Command::Enable("eqoa_example_hearts".into(), true));
    gpu.begin();
    manager.frame(snapshot);
    assert!(gpu.render() > 0);
    manager.after_render();
    manager.queue(Command::Reload("eqoa_example_hearts".into()));
    gpu.begin();
    manager.frame(snapshot);
    gpu.render();
    manager.after_render();
    gpu.begin();
    manager.frame(snapshot);
    gpu.render();
    manager.after_render();
    assert!(
        manager
            .entries()
            .iter()
            .find(|e| e.metadata.id == "eqoa_example_hearts")
            .is_some_and(|e| e.status == Status::Enabled)
    );
    let saved = store
        .load_profile(&key, &profile_host.lua)
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.as_table().unwrap().get::<i32>("x").unwrap(),
        180,
        "disable/re-enable/reload must preserve the example's saved layout"
    );
    manager.shutdown();
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_modern_arpg_unchanged_package() {
    modern_package("arpg_ui");
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_modern_avoids_unchanged_package() {
    modern_package("avoids_modern_ui");
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_modern_quest_log_and_util_surface() {
    let root = find_wereoxxs_scripts_root();
    let addon = discovery::discover(&root)
        .addons
        .into_iter()
        .find(|a| a.id == "avoids_modern_ui")
        .unwrap();
    let host = Host::for_addon(&addon).unwrap();
    host.execute("local q=require('frontiers_forge.quest_log');assert(q.max_quests==8 and q.GetCount()==0);assert(q.GetQuestByIndex(-1)==nil and q.GetQuestByIndex(0)==nil);assert(q.Quests()()==nil)").unwrap();
    let mut snap = rich_in_game_snapshot();
    snap.journal = super::quest_log::JournalSnapshot::from_titles(
        true,
        &[
            "Bat Eyes".encode_utf16().collect(),
            "Caf\u{e9} \u{1f409}".encode_utf16().collect(),
            Vec::new(),
        ],
    );
    host.snapshot.set(snap);
    host.execute("local q=require('frontiers_forge.quest_log');assert(q.GetCount()==3);assert(q.GetQuestByIndex(0):GetName()=='Bat Eyes');assert(q.GetQuestByIndex(1):GetName()=='Caf\u{e9} \u{1f409}');assert(q.GetQuestByIndex(2):GetName()=='');assert(q.GetQuestByIndex(-1)==nil and q.GetQuestByIndex(3)==nil);local n=0;for i,quest in q.Quests() do assert(i==n);assert(quest:GetName()==q.GetQuestByIndex(i):GetName());n=n+1 end;assert(n==3)").unwrap();
    snap.journal.count = 9;
    host.snapshot.set(snap);
    host.execute("assert(require('frontiers_forge.quest_log').GetCount()==0)")
        .unwrap();
    snap.in_game = false;
    snap.journal.count = 3;
    host.snapshot.set(snap.managed());
    host.execute("assert(require('frontiers_forge.quest_log').GetCount()==0)")
        .unwrap();
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_modern_scripts_path_and_file_io_surface() {
    let root = find_wereoxxs_scripts_root();
    let addon = discovery::discover(&root)
        .addons
        .into_iter()
        .find(|a| a.id == "avoids_modern_ui")
        .unwrap();
    let host = Host::for_addon(&addon).unwrap();
    host.execute("assert(type(UiForge.scripts_path)=='string');assert(type(io.open)=='function');assert(type(os.remove)=='function');assert(select('#',require('frontiers_forge.util').LogError('test error'))==0)").unwrap();
}

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

// Fixture tests are explicitly ignored in plain CI runs. Opting in with
// --include-ignored requires a valid pinned checkout; no developer fallback.
fn find_wereoxxs_scripts_root() -> PathBuf {
    let reference = std::env::var_os("EQOA_FORGE_REFERENCE")
        .expect("needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)");
    let scripts = PathBuf::from(reference).join("scripts");
    assert!(
        scripts.join("wereoxxs_ui/wereoxxs_ui.lua").is_file(),
        "EQOA_FORGE_REFERENCE must contain the pinned scripts tree: {}",
        scripts.display()
    );
    scripts
        .canonicalize()
        .expect("canonical pinned reference scripts")
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
    last_target: Option<wgpu::Texture>,
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
            last_target: None,
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

    fn render(&mut self) -> usize {
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
        let total_vtx = {
            let mut renderer = self.renderer.borrow_mut();
            let frame = self.context.render(renderer.renderer_consumer().unwrap());
            let reconciled = renderer.reconcile_frame(frame).unwrap();
            let vtx = reconciled.draw_data().total_vtx_count();
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
                .render_reconciled(reconciled, &mut pass, FramebufferExtent::new(1024, 768))
                .unwrap();
            vtx
        };
        self.queue.submit([encoder.finish()]);
        self.last_target = Some(target);
        total_vtx
    }

    fn frame(&mut self, manager: &mut Manager, snapshot: Snapshot) -> usize {
        self.begin();
        manager.frame(snapshot);
        let vtx = self.render();
        manager.after_render();
        vtx
    }
}

pub(super) fn rich_in_game_snapshot() -> Snapshot {
    crate::play::Play::bundled_ui_fixture().forge_snapshot(None)
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_discovery_and_manifest() {
    let root = find_wereoxxs_scripts_root();

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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_in_game_full_execution_and_lifecycle() {
    let root = find_wereoxxs_scripts_root();

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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_headless_gpu_manager_pipeline() {
    let root = find_wereoxxs_scripts_root();

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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_complete_symbol_surface_contract() {
    let root = find_wereoxxs_scripts_root();
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

        -- 3. All 64 Dear ImGui functions called across wereoxxs_ui
        local imgui_funcs = {
            'Begin', 'BeginChild', 'BeginCombo', 'BeginPopup', 'BeginTabBar', 'BeginTabItem',
            'Button', 'CalcTextSize', 'Checkbox', 'ColorEdit4', 'Dummy', 'End', 'EndChild',
            'EndCombo', 'EndPopup', 'EndTabBar', 'EndTabItem', 'GetColorU32', 'GetContentRegionAvail',
            'GetCursorScreenPos', 'GetFrameHeight', 'GetScrollMaxY', 'GetScrollY', 'GetTextLineHeight',
            'GetTextLineHeightWithSpacing', 'GetTime', 'GetWindowDrawList', 'GetWindowPos',
            'InputTextWithHint', 'InvisibleButton', 'IsItemActive', 'IsMouseDragging',
            'IsMouseHoveringRect', 'IsWindowFocused', 'IsWindowHovered', 'OpenPopup', 'PopFont',
            'PopID', 'PopStyleColor', 'PushFont', 'PushID', 'PushStyleColor', 'RadioButton',
            'SameLine', 'Selectable', 'Separator', 'SetCursorScreenPos', 'SetKeyboardFocusHere',
            'SetNextItemWidth', 'SetNextWindowPos', 'SetNextWindowSize', 'SetScrollHereY',
            'SetScrollY', 'SetTooltip', 'SetWindowFontScale', 'SetWindowPos', 'SliderFloat',
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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_all_windows_rendered_without_drawlist_nil_errors() {
    let root = find_wereoxxs_scripts_root();
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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
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

    let root = find_wereoxxs_scripts_root();
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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_claimed_enter_focuses_addon_input_and_say_window_stays_closed() {
    replay_live_enter(false);
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn live_regression_enter_after_focus_cleanup_reaches_wereoxxs_not_game_chat() {
    replay_live_enter(true);
}

fn replay_live_enter(focus_cleanup: bool) {
    use super::capture::{Button, Capture, Owner};
    use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
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

    if focus_cleanup {
        super::input::focus(false);
        super::input::focus(true);
        // Resume the unchanged Lua VM: its local enter_claimed is still true.
        gpu.begin();
        bridge.invoke(|| host.run_addon(addon)).unwrap();
        gpu.render();
    }
    let code = super::input::logical_key_to_code(&Key::Named(NamedKey::Enter));
    let mut capture = Capture::default();
    let button = Button::Key(PhysicalKey::Code(KeyCode::Enter));
    let mut play = create_test_play();
    for down in [true, false] {
        super::input::record_key_event(code.unwrap(), down);
        if super::input::forwards_key_to_imgui(false, code, down, true) {
            gpu.context
                .io_mut()
                .add_key_event(dear_imgui_rs::Key::Enter, down);
        }
        let owner = if down {
            capture.press(
                button,
                super::input::wants_key_capture(false, code, false, true),
            )
        } else {
            capture.release(button)
        };
        if down && owner == Owner::Game {
            play.chat_key("", false);
        }
        assert_eq!(play.mode, 0, "physical Enter must not open the game's chat");
        assert_eq!(owner, Owner::Addon, "Enter down and up belong to Wereoxxs");
    }

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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_unclaimed_enter_opens_say_window_as_before() {
    use winit::keyboard::{Key, NamedKey};

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
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

    let wants_key_capture = super::input::wants_key_capture(false, code, false, true);
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
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_addon_focused_typing_does_not_leak_keys_to_game() {
    use winit::keyboard::{Key, NamedKey, SmolStr};

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
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

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_claimed_unsuppressed_key_delivers_to_addon_and_reaches_game() {
    use winit::keyboard::{Key, SmolStr};

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.name == "wereoxxs_ui")
        .unwrap();
    let host = Host::for_addon(addon).unwrap();
    let _bridge = Bridge::install(&host).unwrap();

    let setup = r#"
        local Input = require('frontiers_forge.input')
        local ok, err = Input.AcquireKey("tracker", string.byte("w"))
        assert(ok == true and err == nil)
        -- Explicitly DO NOT suppress 'w': addon is watching the key, not taking it from the game
        assert(Input.IsKeySuppressed(string.byte("w")) == false)
    "#;
    host.lua.load(setup).exec().unwrap();

    // Verify key is claimed but not suppressed
    assert!(super::input::is_key_claimed(b'w'));
    assert!(!super::input::is_key_suppressed(b'w'));

    // W is pressed
    let w_key = Key::Character(SmolStr::new("w"));
    let code = super::input::logical_key_to_code(&w_key);
    assert_eq!(code, Some(b'w'));
    super::input::record_key_event(code.unwrap(), true);

    // wants_key_capture must be FALSE because the key is not suppressed and text capture is inactive
    let wants_key_capture = super::input::wants_key_capture(false, code, false, true);
    assert!(
        !wants_key_capture,
        "claimed but unsuppressed key must NOT be captured/swallowed by addon"
    );

    // PollKeyEvents still delivers W to the claiming addon
    let poll = r#"
        local Input = require('frontiers_forge.input')
        local events = Input.PollKeyEvents("tracker")
        assert(#events == 1, "owner must receive polled events for claimed key")
        assert(events[1].key == string.byte("w") and events[1].is_down == true)
    "#;
    host.lua.load(poll).exec().unwrap();

    // Because wants_key_capture was false, the key reaches the game (e.g. movement / Play)
    let mut play = create_test_play();
    assert_eq!(play.hud_menus.sel, 0);
    play.hud_key(crate::hudmenu::Key::Right);
    assert_ne!(
        play.hud_menus.sel, 0,
        "game receives unsuppressed hotkey/input normally"
    );
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_dialog_receives_enter_even_when_addon_suppresses_enter() {
    use winit::keyboard::{Key, NamedKey};

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
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

    // Turn on chat window so Enter is claimed and suppressed
    let win_table: Table = host.lua.globals().get("windows").unwrap();
    win_table.set("chat", true).unwrap();

    // Frame 2: Pump wereoxxs_ui so PumpEnterKey claims & suppresses Enter
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    assert!(
        super::input::is_key_claimed(0x0D),
        "Enter must be claimed by wereoxxs_ui"
    );
    assert!(
        super::input::is_key_suppressed(0x0D),
        "Enter must be suppressed by wereoxxs_ui"
    );

    // Enter key event
    let enter_key = Key::Named(NamedKey::Enter);
    let code = super::input::logical_key_to_code(&enter_key);
    assert_eq!(code, Some(0x0D));

    // Case 1: Game dialog is open (mode 9: VIWndPrivChat, mode 14: mbox, or mode 7: response)
    let mut play = create_test_play();
    play.mode = 9; // Dialog is active!
    assert!(
        !crate::fork_ext::is_plain_play(Some(&mut play)),
        "mode 9 must NOT be plain play"
    );

    let wants_key_capture_dialog = super::input::wants_key_capture(
        false,
        code,
        false,
        crate::fork_ext::is_plain_play(Some(&mut play)),
    );
    assert!(
        !wants_key_capture_dialog,
        "Enter must NOT be captured by addon while game dialog is active (fails before fix)"
    );

    // Because wants_key_capture was false, Enter reaches the game dialog
    let handled = play.dialog_key(crate::dialog::Key::Enter);
    assert!(handled, "dialog_key must receive and handle Enter");
    assert_eq!(
        play.mode, 0,
        "dialog confirmation must close dialog and return to mode 0"
    );

    // Case 2: Now in plain play (mode 0, no UI/dialogs)
    assert!(
        crate::fork_ext::is_plain_play(Some(&mut play)),
        "mode 0 must be plain play"
    );
    let wants_key_capture_plain = super::input::wants_key_capture(
        false,
        code,
        false,
        crate::fork_ext::is_plain_play(Some(&mut play)),
    );
    assert!(
        wants_key_capture_plain,
        "Enter MUST be captured by addon during plain play"
    );
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_pauses_and_hides_in_all_menu_modes() {
    let root = find_wereoxxs_scripts_root();
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };

    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui must be discovered");

    let host = Host::for_addon(addon).expect("Host::for_addon must succeed");
    let bridge = Bridge::install(&host).unwrap();

    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

    let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, addon).unwrap();

    let mut snap = rich_in_game_snapshot();
    host.snapshot.set(snap);

    // Frame 1: Initial load
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    gpu.render();

    // Turn on windows
    let win_table: Table = host.lua.globals().get("windows").unwrap();
    win_table.set("bars", true).unwrap();
    win_table.set("casting", true).unwrap();
    win_table.set("player", true).unwrap();
    win_table.set("target", true).unwrap();
    win_table.set("group", true).unwrap();
    win_table.set("compass", true).unwrap();
    win_table.set("meter", true).unwrap();
    win_table.set("chat", true).unwrap();

    // Warm up frame so ImGui windows are sized and placed
    gpu.begin();
    bridge
        .invoke(|| {
            host.lua
                .load("cast_preview_ms = math.floor(ImGui.GetTime() * 1000)")
                .exec()?;
            host.run_addon(addon)
        })
        .unwrap();
    gpu.render();

    // Frame A: In normal gameplay (mode 0), Wereoxxs draws its full HUD overlay windows
    snap.start_menu = false;
    snap.ui_mode = 0;
    snap.ui_busy = false;
    host.snapshot.set(snap);
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let vtx_playing = gpu.render();
    assert!(
        vtx_playing > 0,
        "Wereoxxs must draw windows in normal gameplay, got {vtx_playing} vertices"
    );

    // Frame B: In spellbook mode (mode 5), Wereoxxs must pause and stop drawing windows
    snap.start_menu = true;
    snap.ui_mode = 5;
    snap.ui_busy = true;
    host.snapshot.set(snap);
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let vtx_spellbook = gpu.render();
    assert_eq!(
        vtx_spellbook, 0,
        "Wereoxxs must draw 0 window vertices while spellbook is open, got {vtx_spellbook}"
    );

    // Frame C: In auctions mode (mode 39), Wereoxxs must pause and stop drawing windows
    snap.start_menu = true;
    snap.ui_mode = 39;
    snap.ui_busy = true;
    host.snapshot.set(snap);
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let vtx_auctions = gpu.render();
    assert_eq!(
        vtx_auctions, 0,
        "Wereoxxs must draw 0 window vertices while auctions is open, got {vtx_auctions}"
    );

    // Frame D: In trainer mode (mode 12), Wereoxxs must pause
    snap.start_menu = true;
    snap.ui_mode = 12;
    snap.ui_busy = true;
    host.snapshot.set(snap);
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let vtx_trainer = gpu.render();
    assert_eq!(
        vtx_trainer, 0,
        "Wereoxxs must draw 0 window vertices while trainer is open, got {vtx_trainer}"
    );

    // Frame E: In options mode (mode 34), Wereoxxs must pause
    snap.start_menu = true;
    snap.ui_mode = 34;
    snap.ui_busy = true;
    host.snapshot.set(snap);
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let vtx_options = gpu.render();
    assert_eq!(
        vtx_options, 0,
        "Wereoxxs must draw 0 window vertices while options is open, got {vtx_options}"
    );

    // Frame F: Return to normal gameplay (mode 0), Wereoxxs resumes drawing windows!
    snap.start_menu = false;
    snap.ui_mode = 0;
    snap.ui_busy = false;
    host.snapshot.set(snap);
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let vtx_resumed = gpu.render();
    assert!(
        vtx_resumed > 0,
        "Wereoxxs must resume drawing windows after menu closes, got {vtx_resumed} vertices"
    );
}

#[derive(Default)]
struct TestBlitRecorder {
    blits: Vec<(u32, [i32; 2])>,
    strings: Vec<String>,
}

impl vi::ui::Raster for TestBlitRecorder {
    fn blit(
        &mut self,
        id: u32,
        _flags: u32,
        at: [i32; 2],
        _a: [i32; 2],
        _b: [i32; 2],
        _mode: i32,
        _alpha: f32,
    ) {
        self.blits.push((id, at));
    }
    fn stretch_blit(
        &mut self,
        id: u32,
        _flags: u32,
        at: [i32; 2],
        _size: [i32; 2],
        _a: [i32; 2],
        _b: [i32; 2],
        _mode: i32,
        _alpha: f32,
    ) {
        self.blits.push((id, at));
    }
    fn fill_fan(&mut self, _color: [f32; 4], _corners: [[i32; 2]; 4]) {}
    fn blit_full(&mut self, id: u32, _flags: u32, at: [i32; 2], _mode: i32, _alpha: f32) {
        self.blits.push((id, at));
    }
    fn draw_string(&mut self, _at: [i32; 2], _color: [f32; 4], s: &[u16]) {
        self.strings.push(String::from_utf16_lossy(s));
    }
    fn draw_centered_string(&mut self, _at: [i32; 2], _color: [f32; 4], s: &[u16], _mode: u32) {
        self.strings.push(String::from_utf16_lossy(s));
    }
    fn string_width(&self, _s: &[u16]) -> i32 {
        0
    }
    fn point_size(&self) -> i32 {
        12
    }
    fn viewport(&self) -> [i32; 4] {
        [0, 0, 639, 447]
    }
    fn set_viewport(&mut self, _v: [i32; 4]) {}
}

fn test_vui() -> vi::ui::Vui {
    let atlas = (0..252)
        .map(|_| vi::ui::UiTexEntry {
            uv1: [16, 16],
            ..Default::default()
        })
        .collect();
    vi::ui::Vui::new(Default::default(), Default::default(), atlas)
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_escape_and_history_keys_with_chat_claimed_do_not_draw_or_open_game_chat() {
    use super::storage::Storage;

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui must be discovered");

    let Some(mut gpu) = TestGpu::new() else {
        return;
    };

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

    // Saved profile has hide_game_chat enabled
    write_profile(&[("hide_game_chat", true)]);

    let mut manager = Manager::new(
        root.clone(),
        Some(config_dir.0.clone()),
        &gpu.device,
        &gpu.queue,
        gpu.renderer.clone(),
    );

    let mut snap = rich_in_game_snapshot();
    gpu.frame(&mut manager, snap.clone());

    // Enable wereoxxs_ui
    manager.queue(Command::Enable("wereoxxs_ui".into(), true));
    gpu.frame(&mut manager, snap.clone());
    gpu.frame(&mut manager, snap.clone());

    // Verify chat window is claimed (suppressed) in plain play
    assert!(
        !crate::hud::fork_hud::chat_window_visible(),
        "chat_window must be claimed/hidden in plain play"
    );

    let vui = test_vui();
    let mut wnd = crate::chat::WndChat::new();
    wnd.text.add_text(&[65, 66], [1.0; 4], 0, |_| 10);

    // R2 or C history with chat claimed in plain play:
    // Even if history mode / scroll is activated, the game chat does not draw.
    wnd.mode = 33;
    wnd.set_scroll(1);
    let mut recorder_history = TestBlitRecorder::default();
    crate::hud::fork_hud::draw_chat_window(&mut wnd, &vui, &mut recorder_history, 1000, 0.016);
    assert_eq!(
        recorder_history.blits.len(),
        0,
        "game chat history must NOT draw while chat is claimed in plain play"
    );
    assert_eq!(
        wnd.mode, 0,
        "history mode must be reset to 0 while chat is claimed"
    );

    // Escape with chat claimed opens the start menu (mode 13).
    // Wereoxxs's show_while_paused deliberately calls UI.EnableChatWindow,
    // so the Enable call is honoured and the game chat may draw in the start menu (parity).
    snap.start_menu = true;
    snap.ui_mode = 13;
    gpu.frame(&mut manager, snap.clone());

    assert!(
        crate::hud::fork_hud::chat_window_visible(),
        "Wereoxxs EnableChatWindow during pause/start menu must be honoured"
    );
    let mut recorder_menu = TestBlitRecorder::default();
    crate::hud::fork_hud::draw_chat_window(&mut wnd, &vui, &mut recorder_menu, 1000, 0.016);
    assert!(
        !recorder_menu.blits.is_empty(),
        "game chat may draw in the start menu when Wereoxxs unhides it for pause"
    );

    // Negative Control: Unclaimed chat (hide_game_chat = false)
    write_profile(&[("hide_game_chat", false)]);
    manager.queue(Command::Load("wereoxxs_ui".into()));
    snap.start_menu = false;
    snap.ui_mode = 0;
    gpu.frame(&mut manager, snap.clone());
    gpu.frame(&mut manager, snap.clone());

    assert!(
        crate::hud::fork_hud::chat_window_visible(),
        "chat_window must be visible when unclaimed"
    );

    // In plain play when unclaimed, history mode and scroll draw normally:
    wnd.mode = 33;
    wnd.set_scroll(1);
    let mut recorder_unclaimed = TestBlitRecorder::default();
    crate::hud::fork_hud::draw_chat_window(&mut wnd, &vui, &mut recorder_unclaimed, 1000, 0.016);
    assert!(
        !recorder_unclaimed.blits.is_empty(),
        "game chat history MUST draw when unclaimed"
    );
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_wereoxxs_worst_case_scrollback_10000_messages_and_budget_measurements() {
    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
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

    // Configure Wereoxxs to worst-case maximum settings:
    // - All windows enabled
    // - Chat scrollback set to 10,000 messages
    // - Populate 10,000 messages across all channel types (Say, Group, Guild, Shout, Tell, Combat)
    let setup_script = r#"
        local ChatWindow = require("chat_window")
        windows = {
            bars = true,
            casting = true,
            player = true,
            target = true,
            group = true,
            compass = true,
            meter = true,
            chat = true,
        }
        chat_config.max_messages = 10000
        local types = {"Say", "Group", "Guild", "Shout", "Tell", "Combat"}
        for i = 1, 10000 do
            local t = types[(i % #types) + 1]
            local text = "Message " .. i .. " scrollback measurement entry with realistic text line length."
            if t == "Tell" then
                text = "Player" .. (i % 5) .. ": " .. text
            end
            ChatWindow.Add(chat_state, chat_config, text, t)
        end
    "#;
    host.execute(setup_script).unwrap();

    // 1. Show that at the old ImGui call budget (4096), this worst-case frame FAILS:
    bridge.set_call_limit(4096);
    gpu.begin();
    let err_calls = bridge.invoke(|| host.run_addon(addon)).unwrap_err();
    gpu.render();
    assert!(
        err_calls
            .to_string()
            .contains("ImGui call budget exceeded: 4097 calls (limit: 4096)"),
        "expected call budget exceeded error at 4096, got: {err_calls}"
    );

    // 2. Show that at the old instruction budget (200,000), this worst-case frame FAILS:
    bridge.set_call_limit(super::bridge::CALL_BUDGET);
    let entry_fn = host.compile_addon_entry(addon).unwrap();
    gpu.begin();
    let err_instr = bridge
        .invoke(|| host.run_with_budget(&entry_fn, 200_000))
        .unwrap_err();
    gpu.render();
    assert!(
        err_instr
            .to_string()
            .contains("Forge instruction budget exceeded"),
        "expected instruction budget exceeded error at 200,000, got: {err_instr}"
    );

    // 3. At the new limits (CALL_BUDGET = 131,072, INSTRUCTION_BUDGET = 4,000,000), it PASSES:
    gpu.begin();
    bridge.invoke(|| host.run_addon(addon)).unwrap();
    let measured_calls = bridge.calls();
    let measured_instructions =
        Host::INSTRUCTION_BUDGET.saturating_sub(host.instructions_remaining());
    let measured_geometry = bridge.geometry();
    println!(
        "MEASURED WORST-CASE WEREOXXS: calls = {measured_calls}, instructions = {measured_instructions}, geometry = {measured_geometry}"
    );
    assert!(
        measured_calls > 4096,
        "measured calls must exceed old 4096 cap: got {measured_calls}"
    );
    assert!(
        measured_instructions > 200_000,
        "measured instructions must exceed old 200,000 cap: got {measured_instructions}"
    );
    assert!(
        measured_calls * 4 <= super::bridge::CALL_BUDGET,
        "new call budget (131,072) must have at least 4x margin over measured calls ({measured_calls})"
    );
    assert!(
        measured_instructions as u64 * 4 <= Host::INSTRUCTION_BUDGET as u64,
        "new instruction budget (4,000,000) must have at least 4x margin over measured instructions ({measured_instructions})"
    );
    gpu.render();
}

#[test]
#[ignore = "needs EQOA_FORGE_REFERENCE (pinned FrontiersForge 4026d5c / UiForge 6f2d125)"]
fn forge_addon_runtime_error_releases_all_claims_and_unhides_game_hud() {
    use super::storage::Storage;
    use super::visibility::published_visibility;

    super::input::reset_input_state();
    let root = find_wereoxxs_scripts_root();
    let disc = discovery::discover(&root);
    let addon = disc
        .addons
        .iter()
        .find(|a| a.id == "wereoxxs_ui")
        .expect("wereoxxs_ui must be discovered");

    let Some(mut gpu) = TestGpu::new() else {
        return;
    };

    let config_dir = ScratchDir::new();
    let storage = Storage::new(Some(config_dir.0.clone()));
    let key = Storage::key(addon).expect("Storage::key must succeed");

    // Profile with all hide options enabled
    let lua = mlua::Lua::new();
    let root_tbl = lua.create_table().unwrap();
    let win_tbl = lua.create_table().unwrap();
    win_tbl.set("bars", true).unwrap();
    win_tbl.set("chat", true).unwrap();
    win_tbl.set("player", true).unwrap();
    win_tbl.set("hide_ability_bars", true).unwrap();
    win_tbl.set("hide_player_bars", true).unwrap();
    win_tbl.set("hide_game_chat", true).unwrap();
    win_tbl.set("hide_game_compass", true).unwrap();
    win_tbl.set("hide_target_nameplate", true).unwrap();
    win_tbl.set("hide_game_group", true).unwrap();
    win_tbl.set("hide_pet_panel", true).unwrap();
    win_tbl.set("hide_active_effects", true).unwrap();
    root_tbl.set("windows", win_tbl).unwrap();
    storage
        .save_profile(&key, mlua::Value::Table(root_tbl))
        .unwrap();

    let mut manager = Manager::new(
        root.clone(),
        Some(config_dir.0.clone()),
        &gpu.device,
        &gpu.queue,
        gpu.renderer.clone(),
    );

    let snap = rich_in_game_snapshot();

    // Enable wereoxxs_ui and run 2 frames to establish claims
    manager.queue(Command::Enable("wereoxxs_ui".into(), true));
    gpu.frame(&mut manager, snap.clone());
    gpu.frame(&mut manager, snap.clone());

    // Verify all hide claims are held while addon is healthy
    assert!(
        published_visibility().any_hidden(),
        "healthy addon with all hide options must hold claims"
    );
    assert!(!crate::hud::fork_hud::chat_window_visible());
    assert!(!crate::hud::fork_hud::ability_bar_visible());

    let entry = manager
        .entries()
        .into_iter()
        .find(|e| e.metadata.id == "wereoxxs_ui")
        .unwrap();
    assert_eq!(entry.status, Status::Enabled);

    // Induce fatal runtime error on the next frame (e.g. call budget exceeded)
    manager.set_call_limit_for_all(5);

    // Run frame where the error occurs
    gpu.frame(&mut manager, snap.clone());

    // Verify addon entered Status::Failed
    let failed_entry = manager
        .entries()
        .into_iter()
        .find(|e| e.metadata.id == "wereoxxs_ui")
        .unwrap();
    assert_eq!(failed_entry.status, Status::Failed);

    // Verify Addon Manager captured the diagnostic error
    assert!(
        manager
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("ImGui call budget exceeded")),
        "Addon Manager must record the diagnostic error"
    );

    // Verify ALL hide claims are immediately released, bringing game HUD back
    assert!(
        !published_visibility().any_hidden(),
        "all hide claims must be released when addon hits a runtime error"
    );
    assert!(
        crate::hud::fork_hud::chat_window_visible(),
        "game chat window must be visible again after addon fault"
    );
    assert!(
        crate::hud::fork_hud::ability_bar_visible(),
        "game ability bars must be visible again after addon fault"
    );
}

#[test]
fn live_regression_modern_missing_legacy_settings_probe_is_quiet() {
    let root = bundled_tree();
    let addon = discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "avoids_modern_ui")
        .unwrap();
    let config = ScratchDir::new();
    let host = Host::for_addon(&addon).unwrap();
    super::file_io::install(&host, &addon, Some(config.0.clone())).unwrap();
    host.execute(
        r#"
        local persist = require('avoids_persist')
        local state = {size=34}
        assert(persist.MigrateLegacyFile(UiForge.resources_path..'/ability_bar_settings.cfg',
            state, {{key='size',type='number'}}) == false)
        assert(state.size == 34)
    "#,
    )
    .unwrap();
    assert!(
        super::native::finish(&host.lua).is_empty(),
        "the real Modern UI missing-legacy migration probe is normal"
    );
}

#[test]
fn live_regression_wereoxxs_slot_positions_default_and_stable_profile() {
    struct Provider;
    impl super::icons::IconProvider for Provider {
        fn decode_icon(&self, _id: u32) -> Option<super::icons::DecodedIcon> {
            let mut rgba = vec![0; 32 * 32 * 4];
            for y in 2..30 {
                for x in 2..30 {
                    rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
                        .copy_from_slice(&[64, 192, 96, 255]);
                }
            }
            for y in 6..26 {
                for x in 6..26 {
                    rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
                        .copy_from_slice(&[224, 64, 32, 255]);
                }
            }
            Some(super::icons::DecodedIcon {
                width: 32,
                height: 32,
                rgba,
            })
        }
        fn decode_ui_texture(&self, id: u32) -> Option<super::icons::DecodedIcon> {
            self.decode_icon(id)
        }
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            super::icons::set_custom_icon_provider(None);
        }
    }
    let _reset = Reset;
    let root = bundled_tree();
    let addon = discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "wereoxxs_ui")
        .unwrap();
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let host = Host::for_addon(&addon).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, &addon).unwrap();
    let _cache = super::icons::attach_icon_cache(&host.lua, textures).unwrap();
    super::icons::set_custom_icon_provider(Some(Box::new(Provider)));
    let mut snapshot = rich_in_game_snapshot();
    snapshot.player.available = true;
    for bar in 0..3 {
        snapshot.abilities.bars[bar].slot_count = if bar == 2 { 4 } else { 5 };
        for slot in 0..5 {
            snapshot.abilities.bars[bar].slots[slot] = AbilityBarSlotSnapshot {
                source_type: if bar == 2 {
                    super::ability::ITEM_SOURCE_TAG
                } else {
                    super::ability::ABILITY_SOURCE_TAG
                },
                source_index: if bar == 2 {
                    slot as u32
                } else {
                    slot as u32 + 1
                },
                ability_icon_ref: 11,
            };
        }
    }
    for slot in 0..5 {
        snapshot.abilities.abilities[slot].id = 400 + slot as u32;
        snapshot.abilities.abilities[slot].display_index = slot as u32;
        snapshot.abilities.abilities[slot].cooldown_lockout_ms = 480_000;
        snapshot.abilities.items[slot].populated = true;
        snapshot.abilities.items[slot].icon_ref = 11;
        let item = &mut snapshot.abilities.items[slot];
        copy_str_to_array("Potion", &mut item.name, &mut item.name_len);
    }
    host.snapshot.set(snapshot);
    host.execute(
        r#"
        probes = {}
        windows = {bars=true}
        local bars = require('ability_bars')
        local paint = require('paint')
        local border = paint.Border
        local draw = bars.Draw
        local get_list = ImGui.GetWindowDrawList
        local current
        paint.Border = function(d,x,y,w,h,...)
            if current then current.rects[#current.rects+1] = {x=x,y=y,w=w,h=h} end
            return border(d,x,y,w,h,...)
        end
        ImGui.GetWindowDrawList = function()
            local raw = get_list()
            if not current then return raw end
            return setmetatable({}, {__index=function(_,key)
                return function(_, ...)
                    if key=='AddImage' then
                        local tex,p0,p1 = ...
                        current.images[#current.images+1] = {x=p0.x,y=p0.y,w=p1.x-p0.x,h=p1.y-p0.y}
                    end
                    return raw[key](raw,...)
                end
            end})
        end
        bars.Draw = function(slots,config,selected)
            current = {rects={},images={},count=#slots,step=config.slot_size+config.spacing}
            local result = current
            local x,y,name = draw(slots,config,selected)
            current = nil
            probes[#probes+1] = result
            return x,y,name
        end
        function distinct_bar_positions()
            if #probes~=3 then return false end
            for _,bar in ipairs(probes) do
                if #bar.rects~=bar.count or bar.step<=0 then return false end
                if bar.count==5 and #bar.images<bar.count then return false end
                for i,a in ipairs(bar.rects) do
                    if a.w<=0 or a.h<=0 then return false end
                    for j=1,i-1 do
                        local b=bar.rects[j]
                        if math.abs(a.x-b.x)<3 and math.abs(a.y-b.y)<3 then return false end
                    end
                end
            end
            return true
        end
    "#,
    )
    .unwrap();
    for _ in 0..3 {
        host.execute("probes={}").unwrap();
        gpu.begin();
        bridge.invoke(|| host.run_addon(&addon)).unwrap();
        gpu.render();
    }
    let layout: String = host.lua.load(r#"
        local out = {}
        for _,bar in ipairs(probes) do
            local rects = {}
            for _,r in ipairs(bar.rects) do rects[#rects+1] = string.format('%.1f,%.1f',r.x,r.y) end
            out[#out+1] = string.format('count=%d images=%d step=%.1f [%s]',bar.count,#bar.images,bar.step,table.concat(rects,';'))
        end
        return table.concat(out, ' | ')
    "#).eval().unwrap();
    eprintln!("MEASURED WEREOXXS SLOT GEOMETRY: {layout}");
    host.execute("assert(distinct_bar_positions(), 'default slot positions must be distinct')")
        .unwrap();
    let config = ScratchDir::new();
    let storage = super::storage::Storage::new(Some(config.0.clone()));
    let key = super::storage::Storage::key(&addon).unwrap();
    // The unchanged pre-round-2 Save/Load callback table, stored with the same
    // tagged profile codec as Stable. Exercise a vertical left bar and all tabs.
    host.execute(
        "bar_configs[1].layout='vertical';bar_configs[1].slot_size=48;bar_configs[1].spacing=6",
    )
    .unwrap();
    let saved = host.callback(2, mlua::Value::Nil).unwrap();
    storage.save_profile(&key, saved).unwrap();
    host.execute("bar_configs[1].slot_size=1;bar_configs[1].spacing=0")
        .unwrap();
    host.callback(3, storage.load_profile(&key, &host.lua).unwrap().unwrap())
        .unwrap();
    host.execute("assert(bar_configs[1].slot_size==48 and bar_configs[1].spacing==6 and bar_configs[1].layout=='vertical');probes={}").unwrap();
    gpu.begin();
    bridge.invoke(|| host.run_addon(&addon)).unwrap();
    gpu.render();
    host.execute(
        "assert(distinct_bar_positions(), 'Stable-format profile must preserve distinct slots')",
    )
    .unwrap();
    // A deliberately collapsed profile is a negative control for the geometry
    // detector, not a claim that Stable or the current adapter creates it.
    host.execute("bar_configs[1].spacing=-bar_configs[1].slot_size;probes={}")
        .unwrap();
    gpu.begin();
    bridge.invoke(|| host.run_addon(&addon)).unwrap();
    gpu.render();
    host.execute(
        "assert(not distinct_bar_positions(), 'collapsed-layout control must be detected')",
    )
    .unwrap();
}

pub(crate) fn modern_slot_positions(snapshot: Snapshot) -> Option<Vec<(f32, f32)>> {
    struct Provider;
    impl super::icons::IconProvider for Provider {
        fn decode_icon(&self, id: u32) -> Option<super::icons::DecodedIcon> {
            if id == 0 || id == u32::MAX {
                return None;
            }
            let mut rgba = vec![0; 32 * 32 * 4];
            for y in 2..30 {
                for x in 2..30 {
                    rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4].copy_from_slice(
                        if (x + y) % 2 == 0 {
                            &[64, 192, 96, 255]
                        } else {
                            &[224, 64, 32, 255]
                        },
                    );
                }
            }
            Some(super::icons::DecodedIcon {
                width: 32,
                height: 32,
                rgba,
            })
        }
        fn decode_ui_texture(&self, id: u32) -> Option<super::icons::DecodedIcon> {
            self.decode_icon(id)
        }
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            super::icons::set_custom_icon_provider(None);
        }
    }
    let _reset = Reset;
    let root = bundled_tree();
    let addon = discovery::discover(&root.0)
        .addons
        .into_iter()
        .find(|a| a.id == "avoids_modern_ui")
        .unwrap();
    let mut gpu = TestGpu::new()?;
    let host = Host::for_addon(&addon).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures = super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::textures::Textures::install_for_addon(&textures, &host.lua, &addon).unwrap();
    let _cache = super::icons::attach_icon_cache(&host.lua, textures).unwrap();
    super::icons::set_custom_icon_provider(Some(Box::new(Provider)));
    host.snapshot.set(snapshot);
    host.execute(
        r#"
        modern_bar = require('avoids_ability_bar')
        modern_ability_bar_state.show_navigation_highlight=false
        modern_ability_bar_state.show_cooldown_overlay=false
        layout_rects={}
        local get_list=ImGui.GetWindowDrawList
        ImGui.GetWindowDrawList=function()
            local raw=get_list()
            return setmetatable({}, {__index=function(_,key)
                return function(_, ...)
                    if key=='AddRectFilled' then
                        local a,b=...
                        if math.abs(b.x-a.x-40)<0.1 and math.abs(b.y-a.y-40)<0.1 then
                            layout_rects[#layout_rects+1]={x=a.x,y=a.y}
                        end
                    end
                    return raw[key](raw,...)
                end
            end})
        end
    "#,
    )
    .unwrap();
    for _ in 0..3 {
        host.execute("layout_rects={}").unwrap();
        gpu.begin();
        bridge
            .invoke(|| host.execute("modern_bar.Update()"))
            .unwrap();
        gpu.render();
    }
    let rows: Table = host.lua.globals().get("layout_rects").unwrap();
    Some(
        rows.sequence_values::<Table>()
            .map(|r| {
                let r = r.unwrap();
                (r.get("x").unwrap(), r.get("y").unwrap())
            })
            .collect(),
    )
}

#[path = "bundled_picture_tests.rs"]
mod bundled_picture_tests;
