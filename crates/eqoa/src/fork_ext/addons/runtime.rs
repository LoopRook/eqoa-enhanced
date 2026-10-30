//! Lazy native app adapter for addon management and physical-pixel rendering.
//!
//! Planned adapter: Dormant -> either standalone Spike OR managed Active, never
//! both. Active drops Manager before shared renderer/platform/Context. It lazily
//! attaches to the existing Gfx window/device; missing/empty root without an open
//! request creates no Context, platform, backend, pass or VM. Discovery is cached,
//! not performed every frame; explicit Rescan remains a manager command.
//!
//! Actual event routing forwards platform events first, then delegates physical
//! press ownership to capture.rs. Lifecycle events are never consumed. Mode/error
//! transitions preserve ownership until release; only focus loss clears it.
//!
//! The shared managed render helper below is intended for production AND headless
//! adapter tests: physical target metrics, one Context frame, control commands,
//! intentional settings body, backend pass and unconditional after_render boundary.

use super::{
    capture::{Button, Capture, Owner},
    discovery::Discovery,
    host::Snapshot,
    manager::{Command, Manager},
    spike::Spike,
    storage::Storage,
};
use dear_imgui_rs::{Context, Ui};
use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use render::wgpu;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use winit::{
    event::{ElementState, WindowEvent},
    window::Window,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Paths {
    mods: PathBuf,
    config: Option<PathBuf>,
    spike: Option<PathBuf>,
}
impl Paths {
    pub fn select(
        default_mods: PathBuf,
        fixture_mods: Option<PathBuf>,
        config: Option<PathBuf>,
        spike: Option<PathBuf>,
    ) -> Self {
        Self {
            mods: fixture_mods.unwrap_or(default_mods),
            config,
            spike,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Launch {
    Dormant,
    Managed,
    Spike(PathBuf),
}

pub(crate) struct Runtime {
    paths: Paths,
    discovery: Discovery,
    initial_enabled: bool,
    graphics: Option<Graphics>,
    capture: Capture,
    open: bool,
    settings: Option<String>,
    error: Option<String>,
    want_mouse: bool,
    stopped: bool,
}
enum Graphics {
    Managed(Box<Active>),
    Spike(Box<Spike>),
}
struct Active {
    // Manager and Lua-held resource references must drop before Context.
    manager: Manager,
    renderer: Rc<RefCell<WgpuRenderer>>,
    platform: WinitPlatform,
    context: Context,
}
impl Runtime {
    pub fn from_env() -> Self {
        Self::new(Paths::select(
            crate::fork_ext::mods::mods_dir(),
            std::env::var_os("EQOA_FORGE_MODS").map(PathBuf::from),
            crate::settings::config_dir(),
            std::env::var_os("EQOA_FORGE_SPIKE").map(PathBuf::from),
        ))
    }
    pub(super) fn new(paths: Paths) -> Self {
        super::visibility::publish(false);
        let discovery = super::discovery::discover(&paths.mods);
        let storage = Storage::new(paths.config.clone());
        let initial_enabled = discovery.addons.iter().any(|addon| {
            Storage::key(addon)
                .and_then(|key| storage.enabled(&key))
                .ok()
                .flatten()
                .unwrap_or(addon.initially_enabled)
        });
        Self {
            paths,
            discovery,
            initial_enabled,
            graphics: None,
            capture: Capture::default(),
            open: false,
            settings: None,
            error: None,
            want_mouse: false,
            stopped: false,
        }
    }
    pub(super) fn launch(&self, open_request: bool) -> Launch {
        if let Some(path) = &self.paths.spike {
            Launch::Spike(path.clone())
        } else if open_request || self.initial_enabled {
            Launch::Managed
        } else {
            Launch::Dormant
        }
    }
    pub fn discovery(&self) -> &Discovery {
        &self.discovery
    }
    pub fn request_open(&mut self) {
        self.open = true;
        self.error = None;
    }
    pub fn event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let (mut keyboard, mut mouse) = (false, false);
        match self.graphics.as_mut() {
            Some(Graphics::Managed(active)) => {
                if let Err(e) =
                    active
                        .platform
                        .handle_window_event(&mut active.context, window, event)
                {
                    self.error = Some(e.to_string());
                    crate::say(&format!("Addons: {e}"));
                }
                if self.error.is_none() {
                    keyboard = active.context.io().want_capture_keyboard();
                    mouse = active.context.io().want_capture_mouse();
                }
            }
            Some(Graphics::Spike(spike)) => {
                // Preserve the alternative fixture's platform forwarding. Its
                // press/wheel result is only a capture request, never ownership.
                let wants = spike.event(window, event);
                keyboard = wants;
                mouse = wants;
            }
            None => {}
        }
        self.want_mouse = mouse;
        if self.error.is_some() {
            self.pause_health();
        }
        let consumed = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                let button = Button::Key(event.physical_key);
                match event.state {
                    ElementState::Pressed => {
                        self.capture.press(button, keyboard && !is_synthetic) == Owner::Addon
                    }
                    ElementState::Released => self.capture.release(button) == Owner::Addon,
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    self.capture.press(Button::Mouse(*button), mouse) == Owner::Addon
                }
                ElementState::Released => {
                    self.capture.release(Button::Mouse(*button)) == Owner::Addon
                }
            },
            WindowEvent::CursorMoved { .. } => self.capture.motion(mouse),
            WindowEvent::MouseWheel { .. } => self.capture.wheel(mouse),
            WindowEvent::Ime(_) => self.capture.text(keyboard),
            WindowEvent::Focused(false) => {
                self.capture.focus_lost();
                self.want_mouse = false;
                false
            }
            WindowEvent::CursorLeft { .. } => {
                self.capture.cursor_left();
                false
            }
            _ => false,
        };
        if consumed {
            window.request_redraw();
        }
        consumed
    }
    pub fn captures_motion(&self) -> bool {
        self.capture.motion(self.want_mouse)
    }
    fn pause_health(&self) {
        if let Some(Graphics::Managed(active)) = &self.graphics {
            active.manager.suspend_health();
            active.manager.suspend_sound();
        }
        super::visibility::publish(false);
    }
    pub fn draw(
        &mut self,
        window: &std::sync::Arc<Window>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        encoder: &mut wgpu::CommandEncoder,
        final_target: &wgpu::TextureView,
        snapshot: Snapshot,
    ) {
        if self.stopped {
            super::visibility::publish(false);
            return;
        }
        if super::take_open_request() {
            self.request_open();
        }
        if self.graphics.is_none() && self.error.is_none() {
            match self.launch(self.open) {
                Launch::Dormant => {
                    super::visibility::publish(false);
                    return;
                }
                Launch::Spike(_) => {
                    self.graphics = Spike::from_env(window, device, queue, format)
                        .map(|s| Graphics::Spike(Box::new(s)));
                    if self.graphics.is_none() {
                        self.error = Some(
                            "Could not load the selected addon fixture. Check the client log."
                                .into(),
                        );
                    }
                }
                Launch::Managed => match Active::new(&self.paths, window, device, queue, format) {
                    Ok(active) => self.graphics = Some(Graphics::Managed(Box::new(active))),
                    Err(e) => {
                        crate::say(&format!("Addons: {e}"));
                        self.error = Some(e);
                    }
                },
            }
        }
        match self.graphics.as_mut() {
            Some(Graphics::Spike(spike)) => spike.draw(
                window,
                encoder,
                final_target,
                snapshot.in_game,
                snapshot.start_menu,
            ),
            Some(Graphics::Managed(active)) if self.error.is_none() => {
                let result = active
                    .platform
                    .prepare_frame(&mut active.context, window)
                    .map_err(|e| e.to_string())
                    .and_then(|_| {
                        let mut plan = FramePlan::collect(Vec::new(), self.settings.clone());
                        plan.controls = Some(self.open);
                        plan.root = Some(self.paths.mods.clone());
                        render_managed(
                            &mut active.context,
                            &active.renderer,
                            &mut active.manager,
                            encoder,
                            final_target,
                            snapshot,
                            plan,
                            Some((&mut active.platform, window)),
                        )
                    });
                match result {
                    Ok(state) => {
                        self.open = state.open;
                        self.settings = state.settings;
                        self.want_mouse = active.context.io().want_capture_mouse();
                    }
                    Err(e) => {
                        active.manager.after_render();
                        active.manager.suspend_health();
                        active.manager.suspend_sound();
                        super::visibility::publish(false);
                        self.want_mouse = false;
                        crate::say(&format!("Addons paused: {e}"));
                        self.error = Some(e);
                    }
                }
            }
            _ => {}
        }
        if self.error.is_none() && !self.stopped {
            if let Some(Graphics::Managed(a)) = &self.graphics {
                let claims = a.manager.hud_claims();
                super::visibility::publish_claims(&claims.borrow());
            } else {
                super::visibility::publish(false);
            }
        } else {
            super::visibility::publish(false);
        }
    }
    pub fn shutdown(&mut self) {
        self.pause_health();
        if self.stopped {
            return;
        }
        self.stopped = true;
        if let Some(Graphics::Managed(active)) = self.graphics.as_mut() {
            active.manager.shutdown();
            if let Err(e) = active.platform.detach_window(&mut active.context) {
                crate::say(&format!("Addons: {e}"));
            }
        }
        self.graphics = None;
        self.capture.focus_lost();
        self.want_mouse = false;
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl Active {
    fn new(
        paths: &Paths,
        window: &std::sync::Arc<Window>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        super::fonts::bump_context_epoch();
        let mut context = Context::create();
        context
            .set_ini_filename::<PathBuf>(None)
            .map_err(|e| e.to_string())?;
        let mut platform = WinitPlatform::new(&mut context).map_err(|e| e.to_string())?;
        platform
            .attach_window(window.clone(), HiDpiMode::Locked(1.0), &mut context)
            .map_err(|e| e.to_string())?;
        let renderer = Rc::new(RefCell::new(
            WgpuRenderer::new(
                WgpuInitInfo::new(device.clone(), queue.clone(), format),
                &mut context,
            )
            .map_err(|e| e.to_string())?,
        ));
        let manager = Manager::new(
            paths.mods.clone(),
            paths.config.clone(),
            device,
            queue,
            renderer.clone(),
        );
        Ok(Self {
            manager,
            renderer,
            platform,
            context,
        })
    }
}

pub(super) enum Action {
    Enable(String, bool),
    Reload(String),
    Rescan,
    Save(String),
    Load(String),
    Settings(String),
    CloseSettings,
}
pub(super) struct FramePlan {
    commands: Vec<Command>,
    settings: Option<String>,
    controls: Option<bool>,
    root: Option<PathBuf>,
    #[cfg(test)]
    fault: Option<RenderFault>,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderFault {
    PrepareRender,
    RendererConsumer,
    AfterRender,
}
impl FramePlan {
    pub fn collect(actions: Vec<Action>, previous_settings: Option<String>) -> Self {
        let mut plan = Self {
            commands: Vec::new(),
            settings: previous_settings,
            controls: None,
            root: None,
            #[cfg(test)]
            fault: None,
        };
        for action in actions {
            match action {
                Action::Enable(id, on) => plan.commands.push(Command::Enable(id, on)),
                Action::Reload(id) => plan.commands.push(Command::Reload(id)),
                Action::Rescan => plan.commands.push(Command::Rescan),
                Action::Save(id) => plan.commands.push(Command::Save(id)),
                Action::Load(id) => plan.commands.push(Command::Load(id)),
                Action::Settings(id) => plan.settings = Some(id),
                Action::CloseSettings => plan.settings = None,
            }
        }
        plan
    }
}

fn dispatch_frame(
    ui: &Ui,
    manager: &mut Manager,
    snapshot: Snapshot,
    plan: FramePlan,
) -> Option<String> {
    for command in plan.commands {
        manager.queue(command);
    }
    let mut called = false;
    let mut selected = plan
        .settings
        .filter(|id| manager.entries().iter().any(|e| e.metadata.id == *id));
    if let Some(id) = selected.as_ref() {
        let name = manager
            .entries()
            .into_iter()
            .find(|e| e.metadata.id == *id)
            .unwrap()
            .metadata
            .name;
        let mut open = true;
        ui.window(format!("Addon settings: {name}"))
            .opened(&mut open)
            .build(|| {
                manager.queue(Command::Settings(id.clone()));
                manager.frame(snapshot);
                called = true;
            });
        if !open {
            selected = None;
        }
    }
    if !called {
        manager.frame(snapshot);
    }
    selected
}

struct FrameState {
    open: bool,
    settings: Option<String>,
}
fn controls(ui: &Ui, manager: &Manager, open: &mut bool, root: Option<&PathBuf>) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.window("Addons").opened(open).build(|| {
        if let Some(readiness)=manager.health_readiness() {ui.text_wrapped(readiness);}
        if let Some(root)=root {ui.text_wrapped(root.to_string_lossy());}
        if ui.button("Rescan folder") {actions.push(Action::Rescan);}
        let entries=manager.entries();
        if entries.is_empty() {ui.text_wrapped("No addons found. Open Mods Folder in Options to add Lua files or addon folders, then rescan.");}
        for entry in entries {
            ui.separator();ui.text(format!("{} ({})",entry.metadata.name,entry.metadata.id));
            ui.text(match entry.status {super::manager::Status::Disabled=>"Disabled",super::manager::Status::Enabled=>"Enabled",super::manager::Status::Failed=>"Failed — reload to retry"});
            let id=&entry.metadata.id;
            if ui.button(format!("{}##enable-{id}",if entry.enabled_intent {"Disable"} else {"Enable"})) {actions.push(Action::Enable(id.clone(),!entry.enabled_intent));}
            ui.same_line();if ui.button(format!("Reload##{id}")) {actions.push(Action::Reload(id.clone()));}
            ui.same_line();if ui.button(format!("Settings##{id}")) {actions.push(Action::Settings(id.clone()));}
            if ui.button(format!("Save profile##{id}")) {actions.push(Action::Save(id.clone()));}
            ui.same_line();if ui.button(format!("Load profile##{id}")) {actions.push(Action::Load(id.clone()));}
            if !entry.initialized {ui.text("Enable this addon to initialize its settings.");}
        }
        for diagnostic in manager.diagnostics() {
            ui.separator();ui.text_wrapped(format!("{}{}",diagnostic.id.as_ref().map(|id|format!("{id}: ")).unwrap_or_default(),diagnostic.message));
        }
    });
    actions
}

fn render_managed(
    context: &mut Context,
    renderer: &Rc<RefCell<WgpuRenderer>>,
    manager: &mut Manager,
    encoder: &mut wgpu::CommandEncoder,
    final_target: &wgpu::TextureView,
    snapshot: Snapshot,
    mut plan: FramePlan,
    platform: Option<(&mut WinitPlatform, &Window)>,
) -> Result<FrameState, String> {
    #[cfg(test)]
    let fault = plan.fault;
    let result = (|| {
        let extent = FramebufferExtent::from_texture(final_target.texture());
        context
            .io_mut()
            .set_display_size([extent.width() as f32, extent.height() as f32]);
        context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
        manager.flush_pending_fonts(context);
        let ui = context.try_frame().map_err(|e| e.to_string())?;
        let mut open = plan.controls.unwrap_or(false);
        if open {
            let actions = controls(ui, manager, &mut open, plan.root.as_ref());
            let extra = FramePlan::collect(actions, plan.settings.take());
            plan.commands.extend(extra.commands);
            plan.settings = extra.settings;
        }
        let settings = dispatch_frame(ui, manager, snapshot, plan);
        #[cfg(test)]
        if fault == Some(RenderFault::PrepareRender) {
            return Err("injected prepare_render failure".into());
        }
        if let Some((platform, window)) = platform {
            platform
                .prepare_render(ui, window)
                .map_err(|e| e.to_string())?;
        }
        let mut renderer = renderer.borrow_mut();
        #[cfg(test)]
        if fault == Some(RenderFault::RendererConsumer) {
            return Err("injected renderer_consumer failure".into());
        }
        let frame = context
            .try_render(renderer.renderer_consumer().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Addon overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: final_target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer
            .render(frame, &mut pass, extent)
            .map_err(|e| e.to_string())?;
        #[cfg(test)]
        if fault == Some(RenderFault::AfterRender) {
            return Err("injected post-render failure".into());
        }
        Ok(FrameState { open, settings })
    })();
    // A platform/consumer error can leave NewFrame open without reaching
    // Render. Close it only after UI/pass/PendingFrame borrows have dropped.
    // This is a no-op for an already rendered (or not begun) native frame.
    context.end_frame();
    manager.after_render();
    if result.is_err() {
        manager.suspend_health();
        manager.suspend_sound();
    }
    super::visibility::publish(result.is_ok() && manager.health_hidden());
    result
}

#[cfg(test)]
mod tests {
    use super::super::{discovery::discover, storage::Storage};
    use super::*;
    use dear_imgui_rs::sys;
    use dear_imgui_wgpu::WgpuInitInfo;
    use mlua::Lua;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "eqoa-forge-runtime-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn native(&self, source: &str) {
            fs::create_dir(self.0.join("a")).unwrap();
            fs::write(self.0.join("a/main.lua"), source).unwrap();
            fs::write(self.0.join("a/mod.json"),r#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"a","name":"a","entry":"main.lua","enabled":true}"#).unwrap();
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("eqoa-forge-runtime-")
            );
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn forge_runtime_lazy_missing_empty_disabled_and_diagnostic_roots_need_an_open_request() {
        let root = Scratch::new();
        for path in [root.0.join("missing"), root.0.clone()] {
            let previous_context = unsafe { sys::igGetCurrentContext() };
            let runtime = Runtime::new(Paths::select(path, None, None, None));
            assert_eq!(
                unsafe { sys::igGetCurrentContext() },
                previous_context,
                "lazy inspection must not create a Context"
            );
            assert!(runtime.graphics.is_none());
            assert_eq!(runtime.launch(false), Launch::Dormant);
            assert_eq!(
                runtime.launch(true),
                Launch::Managed,
                "empty/diagnostic manager must still open"
            );
        }
        fs::write(
            root.0.join("legacy.lua"),
            "error('must not execute during discovery')",
        )
        .unwrap();
        let runtime = Runtime::new(Paths::select(root.0.clone(), None, None, None));
        assert_eq!(runtime.discovery().addons.len(), 1);
        assert_eq!(runtime.launch(false), Launch::Dormant);
        fs::write(
            root.0.join("added.lua"),
            "error('no automatic rediscovery')",
        )
        .unwrap();
        assert_eq!(
            runtime.discovery().addons.len(),
            1,
            "frame planning must use cached discovery"
        );
    }

    #[test]
    fn forge_runtime_fixture_selection_config_isolation_persisted_intent_and_spike_exclusion() {
        let defaults = Scratch::new();
        let fixture = Scratch::new();
        let config = Scratch::new();
        fixture.native("error('selection must never execute source')");
        let paths = Paths::select(
            defaults.0.clone(),
            Some(fixture.0.clone()),
            Some(config.0.clone()),
            None,
        );
        assert_eq!(paths.mods, fixture.0);
        assert_eq!(paths.config, Some(config.0.clone()));
        let runtime = Runtime::new(paths.clone());
        assert_eq!(runtime.launch(false), Launch::Managed);
        let addon = discover(&fixture.0).addons.remove(0);
        let key = Storage::key(&addon).unwrap();
        Storage::new(Some(config.0.clone()))
            .set_enabled(&key, false)
            .unwrap();
        let disabled = Runtime::new(paths);
        assert_eq!(disabled.launch(false), Launch::Dormant);
        let fresh_config = Scratch::new();
        let fresh = Runtime::new(Paths::select(
            defaults.0.clone(),
            Some(fixture.0.clone()),
            Some(fresh_config.0.clone()),
            None,
        ));
        assert_eq!(
            fresh.launch(false),
            Launch::Managed,
            "separate scratch config must not inherit disabled state"
        );
        let spike = fixture.0.join("fixture.lua");
        let alternative = Runtime::new(Paths::select(
            defaults.0.clone(),
            Some(fixture.0.clone()),
            None,
            Some(spike.clone()),
        ));
        assert_eq!(
            alternative.launch(true),
            Launch::Spike(spike),
            "manager open request cannot create a second Context alongside spike"
        );
    }

    #[test]
    fn forge_runtime_ui_actions_queue_controls_and_settings_selection_without_execution() {
        let plan = FramePlan::collect(
            vec![
                Action::Enable("a".into(), true),
                Action::Save("a".into()),
                Action::Load("a".into()),
                Action::Reload("b".into()),
                Action::Rescan,
                Action::Settings("b".into()),
            ],
            Some("a".into()),
        );
        assert_eq!(plan.settings.as_deref(), Some("b"));
        assert_eq!(plan.commands.len(), 5);
        assert!(matches!(&plan.commands[0],Command::Enable(id,true) if id=="a"));
        assert!(matches!(&plan.commands[1],Command::Save(id) if id=="a"));
        assert!(matches!(&plan.commands[2],Command::Load(id) if id=="a"));
        assert!(matches!(&plan.commands[3],Command::Reload(id) if id=="b"));
        assert!(matches!(&plan.commands[4], Command::Rescan));
        assert!(
            FramePlan::collect(vec![Action::CloseSettings], plan.settings)
                .settings
                .is_none()
        );
    }

    #[test]
    fn forge_runtime_health_publication_resets_on_construction_pause_stop_and_drop() {
        use super::super::visibility::{publish, published_health_hidden};
        let root = Scratch::new();
        let config = Scratch::new();
        publish(true);
        let mut runtime = Runtime::new(Paths::select(
            root.0.clone(),
            None,
            Some(config.0.clone()),
            None,
        ));
        assert!(
            !published_health_hidden(),
            "new graphics lifetime cannot inherit claims"
        );
        publish(true);
        runtime.pause_health();
        assert!(!published_health_hidden());
        publish(true);
        runtime.shutdown();
        assert!(!published_health_hidden());
        publish(true);
        drop(runtime);
        assert!(
            !published_health_hidden(),
            "idempotent shutdown also revokes stale publication"
        );
    }

    #[test]
    fn forge_runtime_reopens_same_context_after_prepare_render_failure() {
        assert_failure_reopen(RenderFault::PrepareRender);
    }

    #[test]
    fn forge_runtime_reopens_same_context_after_renderer_consumer_failure() {
        assert_failure_reopen(RenderFault::RendererConsumer);
    }

    #[test]
    fn forge_runtime_post_render_failure_cleanup_is_idempotent() {
        assert_failure_reopen(RenderFault::AfterRender);
    }

    fn assert_failure_reopen(fault: RenderFault) {
        let root = Scratch::new();
        let config = Scratch::new();
        root.native(r#"
            frames=(frames or 0)+1
            require('frontiers_forge.ui').DisableHealthBar('runtime recovery')
            if frames>=2 then
                ImGui.GetBackgroundDrawList():AddLine(ImVec2.new(128,0),ImVec2.new(128,192),ImGui.GetColorU32(0,1,0,1),4)
            end
            UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {[1]=frames} end)
        "#);
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("SKIP forge_runtime recovery GPU prerequisite: {e}");
                return;
            }
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut context = Context::create();
        context.set_ini_filename::<PathBuf>(None).unwrap();
        let original_context = unsafe { sys::igGetCurrentContext() };
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
        let mut manager = Manager::new(
            root.0.clone(),
            Some(config.0.clone()),
            &device,
            &queue,
            renderer.clone(),
        );
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("recovery readback"),
            size: wgpu::Extent3d {
                width: 256,
                height: 192,
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
        context.io_mut().set_delta_time(1.0 / 60.0);
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut plan = FramePlan::collect(Vec::new(), None);
        plan.fault = Some(fault);
        let snapshot = Snapshot {
            in_game: true,
            health: super::super::health::HealthSnapshot::new(true, 1, 100),
            ..Default::default()
        };
        let error = render_managed(
            &mut context,
            &renderer,
            &mut manager,
            &mut encoder,
            &view,
            snapshot,
            plan,
            None,
        )
        .err()
        .expect("injection must fail actual common helper");
        assert!(error.starts_with("injected"), "unexpected failure: {error}");
        assert!(!manager.health_hidden());
        assert!(
            !super::super::visibility::published_health_hidden(),
            "failed overlay cannot leave health hidden"
        );
        queue.submit([encoder.finish()]);
        // This also closes a leaked frame before panicking in the RED run,
        // avoiding an unsafe native NewFrame assertion in the test process.
        assert!(
            !context.end_frame(),
            "{fault:?} left a native ImGui frame open"
        );
        assert!(!context.end_frame(), "cleanup must remain idempotent");
        assert_eq!(unsafe { sys::igGetCurrentContext() }, original_context);

        // Reopen on the SAME Context/manager/backend. The injected failure ran
        // entry once; this frame must run it again, not trip awaiting_render.
        let mut encoder = device.create_command_encoder(&Default::default());
        render_managed(
            &mut context,
            &renderer,
            &mut manager,
            &mut encoder,
            &view,
            snapshot,
            FramePlan::collect(vec![Action::Save("a".into())], None),
            None,
        )
        .unwrap();
        assert!(manager.health_hidden());
        assert!(super::super::visibility::published_health_hidden());
        assert!(!context.end_frame());
        assert_eq!(unsafe { sys::igGetCurrentContext() }, original_context);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 192 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256 * 4),
                    rows_per_image: Some(192),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        recv.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range().unwrap();
        let pixel = &pixels[(8 * 256 + 128) * 4..][..4];
        assert!(
            pixel[0] < 30 && pixel[1] > 220 && pixel[2] < 30,
            "same-context reopened frame must really render green: {pixel:?}"
        );
        let addon = discover(&root.0).addons.remove(0);
        let lua = Lua::new();
        let saved = Storage::new(Some(config.0.clone()))
            .load_profile(&Storage::key(&addon).unwrap(), &lua)
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.as_table().unwrap().get::<i32>(1).unwrap(),
            1,
            "queued Save must execute on recovered boundary after first entry"
        );
        assert!(
            !manager
                .diagnostics()
                .iter()
                .any(|d| d.message.contains("after_render is required")),
            "failure path must retire the previous manager frame before reopening"
        );
        manager.shutdown();
        assert!(!super::super::visibility::published_health_hidden());
    }

    #[test]
    fn forge_runtime_real_settings_panel_and_physical_final_target_at_scene_x2_x4() {
        let root = Scratch::new();
        let config = Scratch::new();
        root.native(r#"
            local w,h=ImGui.GetDisplaySize()
            assert(w==256 and h==192,'overlay dimensions must be final physical client area')
            ImGui.GetBackgroundDrawList():AddLine(ImVec2.new(w/2,0),ImVec2.new(w/2,h),ImGui.GetColorU32(0,1,0,1),4)
            if not registered then
                registered=true
                UiForge.RegisterCallback(UiForge.CallbackType.Settings,function() settings=(settings or 0)+1;ImGui.Text('settings body') end)
                UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {[1]=settings or 0} end)
            end
        "#);
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("SKIP forge_runtime GPU prerequisite: {e}");
                return;
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
        let mut manager = Manager::new(
            root.0.clone(),
            Some(config.0.clone()),
            &device,
            &queue,
            renderer.clone(),
        );
        let descriptor = |width, height| wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let target = device.create_texture(&descriptor(256, 192));
        let view = target.create_view(&Default::default());
        for (index, scale) in [2, 4, 4].into_iter().enumerate() {
            // Simulate the scene's supersampled attachment; the common overlay
            // helper must consume only the final resolved attachment below.
            let _scene = device.create_texture(&descriptor(256 * scale, 192 * scale));
            context.io_mut().set_delta_time(1.0 / 60.0);
            let actions = match index {
                1 => vec![Action::Settings("a".into())],
                2 => vec![Action::Save("a".into()), Action::CloseSettings],
                _ => Vec::new(),
            };
            let mut encoder = device.create_command_encoder(&Default::default());
            render_managed(
                &mut context,
                &renderer,
                &mut manager,
                &mut encoder,
                &view,
                Snapshot::default(),
                FramePlan::collect(actions, None),
                None,
            )
            .unwrap();
            queue.submit([encoder.finish()]);
        }
        let window = unsafe { sys::igFindWindowByName(c"Addon settings: a".as_ptr()) };
        assert!(
            !window.is_null(),
            "selected callback must have a deliberate host settings window"
        );
        let addon = discover(&root.0).addons.remove(0);
        let lua = Lua::new();
        let saved = Storage::new(Some(config.0.clone()))
            .load_profile(&Storage::key(&addon).unwrap(), &lua)
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.as_table().unwrap().get::<i32>(1).unwrap(),
            1,
            "settings callback must execute exactly once in selected body"
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 192 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256 * 4),
                    rows_per_image: Some(192),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        recv.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range().unwrap();
        let green = |x: usize| {
            let i = (8 * 256 + x) * 4;
            pixels[i] < 30 && pixels[i + 1] > 220 && pixels[i + 2] < 30
        };
        assert!(
            green(128),
            "overlay center stays at physical x128 after scene x2/x4"
        );
        manager.shutdown();
    }
}
