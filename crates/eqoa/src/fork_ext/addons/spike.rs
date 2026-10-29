//! Explicit opt-in only. No Context, input interception or draw pass by default.
use super::{
    bridge::Bridge,
    host::{Host, Snapshot},
    profile,
    textures::Textures,
};
use dear_imgui_rs::Context;
use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use mlua::{Function, Value};
use render::wgpu;
use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};
use winit::{
    event::{ElementState, WindowEvent},
    window::Window,
};

pub struct Spike {
    // Drop Lua-held resource references first, Context last.
    script: Function,
    host: Host,
    bridge: Bridge,
    textures: Rc<RefCell<Textures>>,
    renderer: Rc<RefCell<WgpuRenderer>>,
    platform: WinitPlatform,
    context: Context,
    source: PathBuf,
    profile: Option<PathBuf>,
    load_pending: bool,
    enabled: bool,
    error: Option<String>,
    cleaned: bool,
}

impl Spike {
    pub fn from_env(
        window: &Arc<Window>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Option<Self> {
        let path = std::env::var_os("EQOA_FORGE_SPIKE")?;
        match Self::new(window, device, queue, format, PathBuf::from(path)) {
            Ok(spike) => Some(spike),
            Err(error) => {
                eprintln!("Forge spike failed to load: {error}");
                None
            }
        }
    }
    fn new(
        window: &Arc<Window>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        source: PathBuf,
    ) -> Result<Self, String> {
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
        let host = Host::new().map_err(|e| e.to_string())?;
        let bridge = Bridge::install(&host).map_err(|e| e.to_string())?;
        let textures = Textures::new(device, queue, renderer.clone());
        Textures::install(&textures, &host.lua).map_err(|e| e.to_string())?;
        let source = source.canonicalize().map_err(|e| e.to_string())?;
        let script = host
            .compile(&read_source(&source)?)
            .map_err(|e| e.to_string())?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hash);
        let profile = crate::settings::config_dir().map(|p| {
            p.join("forge-spike")
                .join(format!("{:016x}.json", hash.finish()))
        });
        Ok(Self {
            renderer,
            platform,
            context,
            host,
            bridge,
            textures,
            script,
            source,
            profile,
            load_pending: true,
            enabled: true,
            error: None,
            cleaned: false,
        })
    }

    pub fn event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        if let Err(error) = self
            .platform
            .handle_window_event(&mut self.context, window, event)
        {
            self.fail(error.to_string());
        }
        if !self.enabled || self.error.is_some() {
            return false;
        }
        match event {
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.context.io().want_capture_keyboard()
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            }
            | WindowEvent::MouseWheel { .. } => self.context.io().want_capture_mouse(),
            _ => false,
        }
    }

    pub fn draw(
        &mut self,
        window: &Window,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        in_game: bool,
        start_menu: bool,
    ) {
        self.host.snapshot.set(Snapshot {
            in_game,
            start_menu,
            ..Snapshot::default()
        });
        if let Err(error) = self.draw_inner(window, encoder, target) {
            self.fail(error);
        }
    }

    fn draw_inner(
        &mut self,
        window: &Window,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) -> Result<(), String> {
        self.platform
            .prepare_frame(&mut self.context, window)
            .map_err(|e| e.to_string())?;
        let extent = FramebufferExtent::from_texture(target.texture());
        self.context
            .io_mut()
            .set_display_size([extent.width() as f32, extent.height() as f32]);
        self.context
            .io_mut()
            .set_display_framebuffer_scale([1.0, 1.0]);
        let ui = self.context.frame();
        let mut action = 0;
        if self.enabled && self.error.is_none() {
            if let Err(error) = self.bridge.invoke(|| self.host.run(&self.script)) {
                self.error = Some(error.to_string());
                self.enabled = false;
            } else if self.load_pending {
                self.load_pending = false;
                // Load after the unchanged entry script has registered callbacks.
                if let Some(path) = &self.profile {
                    match load_profile(&self.host, path) {
                        Ok(()) => {}
                        Err(error) => {
                            self.error = Some(error);
                            self.enabled = false;
                        }
                    }
                }
            }
        }
        ui.window("Forge feasibility spike").build(|| {
            ui.text(
                self.source
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
            );
            if let Some(error) = &self.error {
                ui.text_wrapped(error);
            }
            let was_enabled = self.enabled;
            ui.checkbox("Enabled", &mut self.enabled);
            if was_enabled && !self.enabled {
                action = 4;
            }
            if !was_enabled && self.enabled {
                action = 3;
            }
            if ui.button("Reload source") {
                action = 3;
            }
            if ui.button("Save profile") {
                action = 1;
            }
            if ui.button("Load profile") {
                action = 2;
            }
            if self.profile.is_none() {
                ui.text("Profile storage unavailable; select EQOA_CONFIG.");
            }
            if self.enabled && self.error.is_none() {
                if let Err(error) = self
                    .bridge
                    .invoke(|| self.host.callback(0, Value::Nil).map(|_| ()))
                {
                    self.error = Some(error.to_string());
                    self.enabled = false;
                }
            }
        });
        self.platform
            .prepare_render(ui, window)
            .map_err(|e| e.to_string())?;
        {
            let mut renderer = self.renderer.borrow_mut();
            let frame = self
                .context
                .render(renderer.renderer_consumer().map_err(|e| e.to_string())?);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Forge spike overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
        }
        self.textures.borrow_mut().collect(false);
        let result = match action {
            1 => self.save(),
            2 => self
                .profile
                .as_ref()
                .ok_or_else(|| "profile directory unavailable".into())
                .and_then(|p| load_profile(&self.host, p)),
            3 => self.reload(),
            4 => {
                let result = self.save();
                self.cleanup(1);
                result
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
        if !self.enabled {
            self.cleanup(1);
        }
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        if self.load_pending || self.error.is_some() {
            return Err("profile not saved: script is uninitialized or failed".into());
        }
        let path = self
            .profile
            .as_ref()
            .ok_or("profile directory unavailable")?;
        let value = self
            .host
            .callback(2, Value::Nil)
            .and_then(profile::encode)
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("profile exceeds 1 MiB".into());
        }
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    }
    fn cleanup(&mut self, kind: i32) {
        if self.cleaned {
            return;
        }
        if let Err(error) = self.host.callback(kind, Value::Nil) {
            eprintln!("Forge cleanup callback: {error}");
        }
        self.textures.borrow_mut().collect(true);
        self.cleaned = true;
    }
    fn reload(&mut self) -> Result<(), String> {
        // Validate the replacement before tearing down the current owner.
        let host = Host::new().map_err(|e| e.to_string())?;
        let script = host
            .compile(&read_source(&self.source)?)
            .map_err(|e| e.to_string())?;
        let bridge = Bridge::install(&host).map_err(|e| e.to_string())?;
        Textures::install(&self.textures, &host.lua).map_err(|e| e.to_string())?;
        self.cleanup(1);
        self.host = host;
        self.script = script;
        self.bridge = bridge;
        self.error = None;
        self.enabled = true;
        self.load_pending = true;
        self.cleaned = false;
        Ok(())
    }
    fn fail(&mut self, error: String) {
        eprintln!("Forge spike: {error}");
        self.error = Some(error);
        self.enabled = false;
        self.cleanup(1);
    }
}

impl Drop for Spike {
    fn drop(&mut self) {
        if self.error.is_none()
            && let Err(error) = self.save()
        {
            eprintln!("Forge profile save: {error}");
        }
        self.cleanup(4);
        if let Err(error) = self.platform.detach_window(&mut self.context) {
            eprintln!("Forge detach: {error}");
        }
    }
}

fn read_source(path: &std::path::Path) -> Result<String, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("source exceeds 1 MiB".into());
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}
fn load_profile(host: &Host, path: &std::path::Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("profile exceeds 1 MiB".into());
    }
    let json = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    host.callback(
        3,
        profile::decode(&host.lua, &json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
