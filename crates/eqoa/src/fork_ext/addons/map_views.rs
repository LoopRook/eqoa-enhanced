//! One native minimap output, shared by held Lua views. No terrain generation here.
use super::{
    host::Host,
    native::{bind, error},
    native_core::CORE,
    textures::{Handle, Textures},
};
use mlua::{Lua, Result, Table, Value};
use render::wgpu;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Projection {
    pub center: [f32; 2],
    pub scale: f32,
    pub outer: [f32; 4],
    pub inner: [f32; 4],
    pub window: (u32, u32),
}
impl Projection {
    pub(crate) fn project(self, size: f32, x: f32, z: f32) -> (f32, f32, bool) {
        let p = [
            (self.outer[0] + self.outer[2]) * 0.5 + (x - self.center[0]) / self.scale,
            (self.outer[1] + self.outer[3]) * 0.5 + (z - self.center[1]) / self.scale,
        ];
        (
            (p[0] - self.outer[0]) * size / (self.outer[2] - self.outer[0]),
            (p[1] - self.outer[1]) * size / (self.outer[3] - self.outer[1]),
            (p[0] - (self.inner[0] + self.inner[2]) * 0.5)
                .hypot(p[1] - (self.inner[1] + self.inner[3]) * 0.5)
                <= (self.inner[2] - self.inner[0]).min(self.inner[3] - self.inner[1]) * 0.5,
        )
    }
    fn unproject(self, size: f32, x: f32, y: f32) -> (f32, f32) {
        (
            (x / size - 0.5) * (self.outer[2] - self.outer[0]) * self.scale + self.center[0],
            (y / size - 0.5) * (self.outer[3] - self.outer[1]) * self.scale + self.center[1],
        )
    }
}
struct Canvas {
    id: u64,
    texture: wgpu::Texture,
    depth: wgpu::TextureView,
    size: (u32, u32),
    window: (u32, u32),
    format: wgpu::TextureFormat,
}
struct Lease {
    vm: u64,
    size: f32,
    active: Cell<bool>,
    texture: RefCell<Option<(u64, Handle)>>,
    error: RefCell<Option<String>>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some((_, h)) = self.texture.get_mut().take() {
            h.owner.borrow_mut().mark_dead(h.id);
        }
    }
}
#[derive(Default)]
struct State {
    leases: Vec<Weak<Lease>>,
    canvas: Option<Rc<Canvas>>,
    serial: u64,
    drawn: Option<u64>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
#[cfg(test)]
pub(crate) fn held_count() -> usize {
    STATE.with(|s| {
        s.borrow()
            .leases
            .iter()
            .filter(|l| l.upgrade().is_some_and(|l| l.active.get()))
            .count()
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Route {
    Stock,
    Capture,
    None,
}
pub(crate) fn route(hidden: bool, held: bool, ready: bool) -> Route {
    if !hidden {
        Route::Stock
    } else if held && ready {
        Route::Capture
    } else {
        Route::None
    }
}
/// Whether a held view has the minimap now (the stock one hidden): the layer is drawn for the
/// capture, not the window.
pub(crate) fn capturing() -> bool {
    super::visibility::effective_visibility().minimap && STATE.with(|s| s.borrow().leases.iter().any(|l| l.upgrade().is_some_and(|l| l.active.get())))
}
pub(crate) fn release_vm(vm: u64) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.leases.retain(|w| {
            if let Some(l) = w.upgrade() {
                if l.vm == vm {
                    l.active.set(false);
                    if let Some((_, h)) = l.texture.borrow_mut().take() {
                        h.owner.borrow_mut().mark_dead(h.id);
                    }
                    false
                } else {
                    true
                }
            } else {
                false
            }
        });
        if s.leases.is_empty() {
            s.canvas = None;
        }
    });
}
fn projection() -> Option<Projection> {
    CORE.with(|c| c.borrow().view.minimap)
}
fn info(lua: &Lua, lease: &Lease) -> Result<Value> {
    *lease.error.borrow_mut() = None;
    if !lease.active.get() || !super::visibility::effective_visibility().minimap {
        return Ok(Value::Nil);
    }
    let Some(p) = projection() else {
        return Ok(Value::Nil);
    };
    let Some(canvas) = STATE.with(|s| s.borrow().canvas.clone()) else {
        return Ok(Value::Nil);
    };
    if canvas.window != p.window {
        return Ok(Value::Nil);
    }
    let Some(bank) = lua
        .app_data_ref::<Rc<RefCell<Textures>>>()
        .map(|b| b.clone())
    else {
        return Ok(Value::Nil);
    };
    let mut cached = lease.texture.borrow_mut();
    if cached.as_ref().is_none_or(|(id, _)| *id != canvas.id) {
        if let Some((_, h)) = cached.take() {
            h.owner.borrow_mut().mark_dead(h.id);
        }
        let handle = match bank.borrow_mut().register_target(&bank, &canvas.texture) {
            Ok(h) => h,
            Err(e) => {
                super::native::diagnose(lua, "eqoa.map.view.texture", "quota", &e);
                *lease.error.borrow_mut() = Some(e);
                return Ok(Value::Nil);
            }
        };
        *cached = Some((canvas.id, handle));
    }
    let h = cached.as_ref().unwrap().1.clone();
    let t = lua.create_table()?;
    t.set("texture", lua.create_userdata(h)?)?;
    t.set("width", lease.size)?;
    t.set("height", lease.size)?;
    t.set(
        "uv",
        lua.create_sequence_from([
            p.outer[0] / p.window.0 as f32,
            p.outer[1] / p.window.1 as f32,
            p.outer[2] / p.window.0 as f32,
            p.outer[3] / p.window.1 as f32,
        ])?,
    )?;
    Ok(Value::Table(t))
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (name, status, reason) in [
        ("images", "unsupported", "replaced by eqoa.map.view"),
        ("load_image", "unsupported", "replaced by eqoa.map.view"),
        (
            "set_waypoint",
            "not_yet",
            "not built; use eqoa.map.set_guide",
        ),
        (
            "clear_waypoint",
            "not_yet",
            "not built; use eqoa.map.set_guide",
        ),
    ] {
        let symbol = format!("eqoa.map.{name}");
        bind(
            lua,
            "map",
            name,
            lua.create_function(move |lua, _: mlua::MultiValue| {
                error(lua, &symbol, status, reason)
            })?,
        )?;
        super::native::describe(
            lua,
            &format!("eqoa.map.{name}"),
            status,
            if status == "not_yet" { Some(4) } else { None },
            reason,
        );
    }

    bind(
        lua,
        "map",
        "view",
        lua.create_function(|lua, arg: Option<Table>| {
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.map.view",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let arg = arg.unwrap_or(lua.create_table()?);
            for pair in arg.clone().pairs::<String, Value>() {
                let (k, _) = pair?;
                if k != "size" && k != "layers" {
                    return error(
                        lua,
                        "eqoa.map.view",
                        "invalid_argument",
                        "only size and layers=all are supported",
                    );
                }
            }
            if arg
                .get::<Option<String>>("layers")?
                .is_some_and(|x| x != "all")
            {
                return error(
                    lua,
                    "eqoa.map.view",
                    "unsupported",
                    "native view contains all layers; circular images can crop corner marks",
                );
            }
            let size = arg.get::<Option<f32>>("size")?.unwrap_or(112.0);
            if !size.is_finite() || !(16.0..=1024.0).contains(&size) {
                return error(
                    lua,
                    "eqoa.map.view",
                    "out_of_range",
                    "size must be 16..1024",
                );
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            let count = STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.leases.retain(|w| w.upgrade().is_some_and(|l| l.active.get()));
                s.leases
                    .iter()
                    .filter_map(Weak::upgrade)
                    .filter(|l| l.vm == vm)
                    .count()
            });
            if count >= 4 {
                return error(
                    lua,
                    "eqoa.map.view",
                    "quota",
                    "at most four held views per mod",
                );
            }
            let l = Rc::new(Lease {
                vm,
                size,
                active: Cell::new(true),
                texture: RefCell::new(None),
                error: RefCell::new(None),
            });
            STATE.with(|s| s.borrow_mut().leases.push(Rc::downgrade(&l)));
            let t = lua.create_table()?;
            let p = l.clone();
            t.set(
                "project",
                lua.create_function(move |lua, (x, z): (f32, f32)| {
                    if !x.is_finite() || !z.is_finite() {
                        return error(
                            lua,
                            "eqoa.map.view.project",
                            "invalid_argument",
                            "finite coordinates required",
                        );
                    }
                    if !p.active.get() {
                        return error(
                            lua,
                            "eqoa.map.view.project",
                            "stale_handle",
                            "view disabled",
                        );
                    }
                    match projection() {
                        Some(v) => lua.pack_multi(v.project(p.size, x, z)),
                        None => error(
                            lua,
                            "eqoa.map.view.project",
                            "unavailable",
                            "native minimap has not been prepared",
                        ),
                    }
                })?,
            )?;
            let p = l.clone();
            t.set(
                "unproject",
                lua.create_function(move |lua, (x, y): (f32, f32)| {
                    if !x.is_finite() || !y.is_finite() {
                        return error(
                            lua,
                            "eqoa.map.view.unproject",
                            "invalid_argument",
                            "finite coordinates required",
                        );
                    }
                    if !p.active.get() {
                        return error(
                            lua,
                            "eqoa.map.view.unproject",
                            "stale_handle",
                            "view disabled",
                        );
                    }
                    match projection() {
                        Some(v) => lua.pack_multi(v.unproject(p.size, x, y)),
                        None => error(
                            lua,
                            "eqoa.map.view.unproject",
                            "unavailable",
                            "native minimap has not been prepared",
                        ),
                    }
                })?,
            )?;
            let close = l.clone();
            t.set("close", lua.create_function(move |_, _: Value| { close.active.set(false); if let Some((_, h)) = close.texture.borrow_mut().take() { h.owner.borrow_mut().mark_dead(h.id); } Ok(true) })?)?;
            let get = l.clone();
            t.set("get_texture", lua.create_function(move |lua, ()| {
                let value = info(lua, &get)?;
                if let Some(e) = get.error.borrow().as_ref() {
                    error(lua, "eqoa.map.view.texture", "quota", e)
                } else { lua.pack_multi((value, Value::Nil)) }
            })?)?;
            let mt = lua.create_table()?;
            mt.set(
                "__index",
                lua.create_function(move |lua, (_, k): (Value, String)| {
                    if k == "texture" {
                        info(lua, &l)
                    } else if k == "error" {
                        match l.error.borrow().as_ref() {
                            Some(e) => super::native::record(lua, "eqoa.Error", &serde_json::json!({"code":"quota","message":e,"capability":"eqoa.map.view.texture"})),
                            None => Ok(Value::Nil),
                        }
                    } else {
                        Ok(Value::Nil)
                    }
                })?,
            )?;
            t.set_metatable(Some(mt))?;
            lua.pack_multi((t, Value::Nil))
        })?,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    map: &crate::map::Map,
    device: &wgpu::Device,
    enc: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    window: (u32, u32),
    size: (u32, u32),
    format: wgpu::TextureFormat,
) {
    let hidden = super::visibility::effective_visibility().minimap;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.leases
            .retain(|w| w.upgrade().is_some_and(|l| l.active.get()));
        match route(hidden, !s.leases.is_empty(), map.mini.up && !map.open) {
            Route::Stock => {
                s.canvas = None;
                s.drawn = None;
                native_draw(map, device, enc, target, depth, window, size);
            }
            Route::None => {
                s.canvas = None;
                s.drawn = None;
            }
            Route::Capture => {
                let frame = CORE.with(|c| c.borrow().frame);
                if s.drawn == Some(frame) {
                    return;
                }
                s.drawn = Some(frame);
                let k = (2048.0 / window.0.max(window.1).max(1) as f32).min(1.0);
                let dims = (
                    (window.0 as f32 * k).round().max(1.0) as u32,
                    (window.1 as f32 * k).round().max(1.0) as u32,
                );
                if s.canvas
                    .as_ref()
                    .is_none_or(|c| c.window != window || c.format != format)
                {
                    s.serial += 1;
                    s.canvas = Some(Rc::new(Canvas {
                        id: s.serial,
                        texture: device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("addon native minimap"),
                            size: wgpu::Extent3d {
                                width: dims.0,
                                height: dims.1,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING
                                | if cfg!(test) { wgpu::TextureUsages::COPY_SRC } else { wgpu::TextureUsages::empty() },
                            view_formats: &[],
                        }),
                        depth: render::depth_view(device, dims.0, dims.1),
                        size: dims,
                        window,
                        format,
                    }));
                }
                let c = s.canvas.as_ref().unwrap();
                let tv = c.texture.create_view(&Default::default());
                {
                    let _clear = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear addon minimap"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &tv,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                // The native pipelines write colour only, so an opaque clear keeps the
                                // texture's alpha 1 for the HUD image to blend; unmapped ground is black.
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &c.depth,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                native_draw(map, device, enc, &tv, &c.depth, window, c.size);
            }
        }
    });
}
#[cfg(test)]
thread_local! {static DRAW_CALLS:Cell<usize>=const{Cell::new(0)};}
#[cfg(test)]
pub(crate) fn canvas() -> Option<(wgpu::Texture, (u32, u32))> {
    STATE.with(|s| s.borrow().canvas.as_ref().map(|c| (c.texture.clone(), c.size)))
}
#[cfg(test)]
pub(crate) fn draw_calls() -> usize {
    DRAW_CALLS.with(Cell::get)
}
fn native_draw(
    map: &crate::map::Map,
    device: &wgpu::Device,
    enc: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    window: (u32, u32),
    size: (u32, u32),
) {
    #[cfg(test)]
    DRAW_CALLS.with(|c| c.set(c.get() + 1));
    map.draw_mini(device, enc, target, depth, window, size);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round4_native_minimap_route_is_exclusive_and_no_view_has_no_capture() {
        assert_eq!(route(false, true, true), Route::Stock);
        assert_eq!(route(true, true, true), Route::Capture);
        assert_eq!(route(true, false, true), Route::None);
        assert_eq!(route(true, true, false), Route::None);
    }

    #[test]
    fn round4_live_capture_quota_lifecycle_and_device_cleanup() {
        live_capture_fixture(true);
    }
    #[test]
    fn round4_live_texture_quota_returns_nil_error() {
        live_capture_fixture(false);
    }
    fn live_capture_fixture(check_draw_count: bool) {
        use super::super::{health, textures, visibility};
        use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
            .expect("GPU capture fixture needs adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut context = dear_imgui_rs::Context::create();
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
        let bank = Textures::new(&device, &queue, renderer);
        super::super::hud_layout::set_window_size(640, 448);
        let mut p = crate::play::Play::bundled_ui_fixture();
        p.map.mini.up = true;
        p.map.open = false;
        let walker = crate::play::Play::bundled_ui_walker();
        let h = Host::new().unwrap();
        h.snapshot.set(p.forge_snapshot(Some(&walker)));
        Textures::install(&bank, &h.lua).unwrap();
        h.lua.set_app_data(bank.clone());
        let claims = Rc::new(RefCell::new(visibility::HudClaimsRegistry::new()));
        let lease = visibility::VisibilityLease::with_hud_claims(
            Rc::new(RefCell::new(health::HealthBarClaimsRegistry::new())),
            claims.clone(),
        )
        .unwrap();
        lease.update(h.snapshot.get().health, true);
        visibility::install_managed_health_adapter(&h, &lease).unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 640,
                height: 448,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let tv = target.create_view(&Default::default());
        let depth = render::depth_view(&device, 640, 448);
        let call = |p: &crate::play::Play| {
            let mut enc = device.create_command_encoder(&Default::default());
            draw(
                &p.map,
                &device,
                &mut enc,
                &tv,
                &depth,
                (640, 448),
                (640, 448),
                wgpu::TextureFormat::Rgba8Unorm,
            );
            queue.submit([enc.finish()]);
        };
        DRAW_CALLS.with(|c| c.set(0));
        call(&p);
        assert_eq!(DRAW_CALLS.with(Cell::get), 1);
        assert!(STATE.with(|s| s.borrow().canvas.is_none()));
        h.execute("v=assert(eqoa.map.view{});hide=assert(eqoa.hud.hide('minimap'))")
            .unwrap();
        visibility::publish_claims(&claims.borrow());
        call(&p);
        call(&p);
        if check_draw_count {
            assert_eq!(
                DRAW_CALLS.with(Cell::get),
                2,
                "one stock draw followed by exactly one capture in a frame"
            );
        }
        assert!(STATE.with(|s| s.borrow().canvas.is_some()));
        assert_eq!(
            projection().expect("live minimap projection").window,
            (640, 448)
        );
        assert!(h.lua.app_data_ref::<Rc<RefCell<Textures>>>().is_some());
        bank.borrow_mut().set_budget(textures::TexturesBudget {
            max_count: 0,
            ..textures::ADDON_BUDGET
        });
        h.execute("assert(v.texture==nil);local t,e=v.get_texture();assert(t==nil and e.code=='quota');assert(v.error.code=='quota')").unwrap();
        bank.borrow_mut().set_budget(textures::ADDON_BUDGET);
        h.execute("assert(v.texture~=nil)").unwrap();
        h.execute("for i=1,6 do local w=assert(eqoa.map.view{});assert(w:close()) end")
            .unwrap();
        super::super::hud_layout::set_window_size(1920, 1080);
        p.forge_snapshot(Some(&walker));
        h.execute("assert(v.texture==nil)").unwrap();
        super::super::native_events::clear(&h);
        h.execute("assert(v.texture==nil);local x,e=v.project(0,0);assert(x==nil and e.code=='stale_handle')").unwrap();
        assert!(STATE.with(|s| s.borrow().canvas.is_none() && s.borrow().leases.is_empty()));
        visibility::publish(false);
        super::super::hud_layout::set_window_size(640, 448);
    }
}
