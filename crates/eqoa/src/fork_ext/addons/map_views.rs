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
            p[0] >= self.inner[0]
                && p[0] <= self.inner[2]
                && p[1] >= self.inner[1]
                && p[1] <= self.inner[3],
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
        let handle = bank
            .borrow_mut()
            .register_target(&bank, &canvas.texture)
            .map_err(mlua::Error::runtime)?;
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
                s.leases.retain(|w| w.strong_count() > 0);
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
            let mt = lua.create_table()?;
            mt.set(
                "__index",
                lua.create_function(move |lua, (_, k): (Value, String)| {
                    if k == "texture" {
                        info(lua, &l)
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
                map.draw_mini(device, enc, target, depth, window, size);
            }
            Route::None => {
                s.canvas = None;
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
                                | wgpu::TextureUsages::TEXTURE_BINDING,
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
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
                map.draw_mini(device, enc, &tv, &c.depth, window, c.size);
            }
        }
    });
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
}
