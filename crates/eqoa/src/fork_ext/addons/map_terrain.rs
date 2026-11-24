//! Round A13: the world map's terrain as a picture a mod can draw (`eqoa.map.terrain`), so a mod-drawn map needs no
//! terrain drawing of its own. It is the picture the game's own map shows for the player's world when the map is
//! zoomed out to the whole world (the overview `map::cells` keeps on the GPU), scaled to the size the mod asks for,
//! with the transform between world coordinates and its pixels. Nothing here builds terrain: the game's map builds it
//! (behind the game, a few seconds after a world is entered); a mod's picture fills in as the build goes on.
use super::{
    host::Host,
    native::{bind, error},
    textures::{Handle, Textures},
};
use mlua::{Lua, Result, Value};
use render::wgpu;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Longest side a terrain picture may be asked for, in pixels (the texture budget of a mod that asks for large data).
pub(crate) const MAX_SIZE: f32 = 2048.0;
pub(crate) const MIN_SIZE: f32 = 64.0;
pub(crate) const DEFAULT_SIZE: f32 = 1024.0;
/// Most terrain pictures one mod may hold at once.
const MAX_PER_VM: usize = 2;

/// What the map's layer offers each frame (`Map::api_terrain`).
pub(crate) struct Source<'a> {
    pub texture: &'a wgpu::Texture,
    /// The world rectangle the picture covers: x_min, z_min, x_max, z_max.
    pub extent: [f32; 4],
    /// The picture's size in texels.
    pub texels: (u32, u32),
    pub world: i32,
    /// Which texture of the map's layer it is (it changes when the world does).
    pub texture_index: usize,
    /// Zones on the map, of zones to draw.
    pub progress: (usize, usize),
}

/// World coordinates to the picture's pixels and back: the picture's top left is (`x_min`, `z_min`), x grows to the
/// right and z grows downward (the map's own orientation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform {
    pub extent: [f32; 4],
    pub size: (u32, u32),
}

impl Transform {
    pub(crate) fn project(self, x: f32, z: f32) -> (f32, f32) {
        let (w, h) = (self.extent[2] - self.extent[0], self.extent[3] - self.extent[1]);
        ((x - self.extent[0]) / w * self.size.0 as f32, (z - self.extent[1]) / h * self.size.1 as f32)
    }
    pub(crate) fn unproject(self, px: f32, py: f32) -> (f32, f32) {
        let (w, h) = (self.extent[2] - self.extent[0], self.extent[3] - self.extent[1]);
        (self.extent[0] + px / self.size.0 as f32 * w, self.extent[1] + py / self.size.1 as f32 * h)
    }
    /// Whether the world point lies on the picture (inside the world's rectangle).
    pub(crate) fn contains(self, x: f32, z: f32) -> bool {
        x >= self.extent[0] && x < self.extent[2] && z >= self.extent[1] && z < self.extent[3]
    }
}

/// The picture's pixel size for a requested longest side: the world's aspect, never larger than the overview itself.
pub(crate) fn canvas_size(requested: f32, texels: (u32, u32)) -> (u32, u32) {
    let long = texels.0.max(texels.1).max(1) as f32;
    let side = requested.clamp(MIN_SIZE, MAX_SIZE).min(long);
    let k = side / long;
    (((texels.0 as f32 * k).round() as u32).max(1), ((texels.1 as f32 * k).round() as u32).max(1))
}

struct Canvas {
    id: u64,
    texture: wgpu::Texture,
    size: (u32, u32),
    world: i32,
    texture_index: usize,
    /// The progress the picture was drawn at.
    drawn_at: (usize, usize),
}

struct Lease {
    vm: u64,
    requested: f32,
    active: Cell<bool>,
    canvas: RefCell<Option<Canvas>>,
    handle: RefCell<Option<(u64, Handle)>>,
    error: RefCell<Option<String>>,
    /// The world rectangle and canvas size of the last draw: the transform's data.
    transform: Cell<Option<Transform>>,
    world: Cell<i32>,
    progress: Cell<(usize, usize)>,
}

impl Lease {
    fn release(&self) {
        self.active.set(false);
        *self.canvas.borrow_mut() = None;
        if let Some((_, h)) = self.handle.borrow_mut().take() {
            h.owner.borrow_mut().mark_dead(h.id);
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some((_, h)) = self.handle.get_mut().take() {
            h.owner.borrow_mut().mark_dead(h.id);
        }
    }
}

#[derive(Default)]
struct State {
    leases: Vec<Weak<Lease>>,
    serial: u64,
    blit: Option<Blit>,
    draws: usize,
}
thread_local! {static STATE: RefCell<State> = RefCell::new(State::default());}

/// A mod holds a terrain picture.
pub(crate) fn wanted() -> bool {
    STATE.with(|s| s.borrow().leases.iter().any(|l| l.upgrade().is_some_and(|l| l.active.get())))
}

pub(crate) fn release_vm(vm: u64) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.leases.retain(|w| match w.upgrade() {
            Some(l) if l.vm == vm => {
                l.release();
                false
            }
            Some(_) => true,
            None => false,
        });
        if s.leases.is_empty() {
            s.blit = None;
        }
    });
}

#[cfg(test)]
pub(crate) fn held_count() -> usize {
    STATE.with(|s| s.borrow().leases.iter().filter(|l| l.upgrade().is_some_and(|l| l.active.get())).count())
}
#[cfg(test)]
pub(crate) fn draw_count() -> usize {
    STATE.with(|s| s.borrow().draws)
}
#[cfg(test)]
pub(crate) fn canvas_of_first() -> Option<(wgpu::Texture, (u32, u32))> {
    STATE.with(|s| {
        s.borrow().leases.iter().filter_map(Weak::upgrade).find_map(|l| l.canvas.borrow().as_ref().map(|c| (c.texture.clone(), c.size)))
    })
}

const SHADER: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var o: Out;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    o.uv = vec2<f32>((p[i].x + 1.0) * 0.5, (1.0 - p[i].y) * 0.5);
    return o;
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    let c = textureSample(src, samp, in.uv);
    // unbuilt ground is transparent in the overview; a mod's picture is opaque, dark where nothing is drawn yet
    return vec4<f32>(mix(vec3<f32>(0.04, 0.05, 0.07), c.rgb, c.a), 1.0);
}
"#;

struct Blit {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
}

impl Blit {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("addon terrain"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("addon terrain"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("addon terrain"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("addon terrain"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("addon terrain"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: render::TEXTURE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self { layout, sampler, pipeline }
    }
}

/// Each frame, with the map's own picture (`fork_ext/map.rs`, from the minimap's draw call, which runs every frame in
/// play): every held terrain picture is drawn again when it is new, when the world changed or when the map's build
/// has moved on since it was drawn.
pub(crate) fn draw(map: &crate::map::Map, device: &wgpu::Device, enc: &mut wgpu::CommandEncoder) {
    if !wanted() {
        return;
    }
    let src = map.api_terrain();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.leases.retain(|w| w.upgrade().is_some_and(|l| l.active.get()));
        let Some(src) = src else {
            // no world map yet (or a world with none): nothing to show, and the transform is not known
            for l in s.leases.iter().filter_map(Weak::upgrade) {
                l.transform.set(None);
                *l.canvas.borrow_mut() = None;
            }
            return;
        };
        if s.blit.is_none() {
            s.blit = Some(Blit::new(device));
        }
        let leases: Vec<Rc<Lease>> = s.leases.iter().filter_map(Weak::upgrade).collect();
        let mut drew = 0;
        for l in leases {
            let size = canvas_size(l.requested, src.texels);
            l.transform.set(Some(Transform { extent: src.extent, size }));
            l.world.set(src.world);
            l.progress.set(src.progress);
            let stale = l
                .canvas
                .borrow()
                .as_ref()
                .is_none_or(|c| c.size != size || c.world != src.world || c.texture_index != src.texture_index || c.drawn_at != src.progress);
            if !stale {
                continue;
            }
            s.serial += 1;
            let id = s.serial;
            let texture = match l.canvas.borrow_mut().take() {
                Some(c) if c.size == size => c.texture,
                _ => device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("addon terrain"),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: render::TEXTURE_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | if cfg!(test) { wgpu::TextureUsages::COPY_SRC } else { wgpu::TextureUsages::empty() },
                    view_formats: &[],
                }),
            };
            let blit = s.blit.as_ref().unwrap();
            let view = src.texture.create_view(&Default::default());
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("addon terrain"),
                layout: &blit.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&blit.sampler) },
                ],
            });
            let tv = texture.create_view(&Default::default());
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("addon terrain"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &tv,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&blit.pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.draw(0..3, 0..1);
            }
            drew += 1;
            *l.canvas.borrow_mut() =
                Some(Canvas { id, texture, size, world: src.world, texture_index: src.texture_index, drawn_at: src.progress });
        }
        s.draws += drew;
    });
}

fn texture_info(lua: &Lua, lease: &Lease) -> Result<Value> {
    *lease.error.borrow_mut() = None;
    let canvas = lease.canvas.borrow();
    let Some(canvas) = canvas.as_ref().filter(|_| lease.active.get()) else {
        return Ok(Value::Nil);
    };
    let Some(bank) = lua.app_data_ref::<Rc<RefCell<Textures>>>().map(|b| b.clone()) else {
        return Ok(Value::Nil);
    };
    let mut cached = lease.handle.borrow_mut();
    if cached.as_ref().is_none_or(|(id, _)| *id != canvas.id) {
        if let Some((_, h)) = cached.take() {
            h.owner.borrow_mut().mark_dead(h.id);
        }
        match bank.borrow_mut().register_target(&bank, &canvas.texture) {
            Ok(h) => *cached = Some((canvas.id, h)),
            Err(e) => {
                super::native::diagnose(lua, "eqoa.map.terrain.texture", "quota", &e);
                *lease.error.borrow_mut() = Some(e);
                return Ok(Value::Nil);
            }
        }
    }
    let h = cached.as_ref().unwrap().1.clone();
    let t = lua.create_table()?;
    t.set("texture", lua.create_userdata(h)?)?;
    t.set("width", canvas.size.0)?;
    t.set("height", canvas.size.1)?;
    t.set("uv", lua.create_sequence_from([0.0f32, 0.0, 1.0, 1.0])?)?;
    Ok(Value::Table(t))
}

fn stale(lua: &Lua, symbol: &str) -> Result<mlua::MultiValue> {
    error(lua, symbol, "stale_handle", "terrain picture closed")
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    bind(
        lua,
        "map",
        "terrain",
        lua.create_function(|lua, arg: Option<mlua::Table>| {
            const SYMBOL: &str = "eqoa.map.terrain";
            if !super::native_core::CORE.with(|c| c.borrow().view.in_world) {
                return error(lua, SYMBOL, "not_in_world", "enter a loaded world first");
            }
            let arg = arg.unwrap_or(lua.create_table()?);
            for pair in arg.clone().pairs::<String, Value>() {
                let (k, _) = pair?;
                if k != "size" {
                    return error(lua, SYMBOL, "invalid_argument", "only size is supported");
                }
            }
            let size = match arg.get::<Value>("size")? {
                Value::Nil => DEFAULT_SIZE,
                Value::Integer(n) => n as f32,
                Value::Number(n) => n as f32,
                _ => return error(lua, SYMBOL, "invalid_argument", "size must be a number"),
            };
            if !size.is_finite() || !(MIN_SIZE..=MAX_SIZE).contains(&size) {
                return error(lua, SYMBOL, "out_of_range", "size must be 64..2048");
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            let count = STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.leases.retain(|w| w.upgrade().is_some_and(|l| l.active.get()));
                s.leases.iter().filter_map(Weak::upgrade).filter(|l| l.vm == vm).count()
            });
            if count >= MAX_PER_VM {
                return error(lua, SYMBOL, "quota", "at most two terrain pictures per mod");
            }
            let l = Rc::new(Lease {
                vm,
                requested: size,
                active: Cell::new(true),
                canvas: RefCell::new(None),
                handle: RefCell::new(None),
                error: RefCell::new(None),
                transform: Cell::new(None),
                world: Cell::new(-1),
                progress: Cell::new((0, 0)),
            });
            STATE.with(|s| s.borrow_mut().leases.push(Rc::downgrade(&l)));
            let t = lua.create_table()?;
            let p = l.clone();
            t.set(
                "project",
                lua.create_function(move |lua, (x, z): (f32, f32)| {
                    const SYMBOL: &str = "eqoa.map.terrain.project";
                    if !x.is_finite() || !z.is_finite() {
                        return error(lua, SYMBOL, "invalid_argument", "finite coordinates required");
                    }
                    if !p.active.get() {
                        return stale(lua, SYMBOL);
                    }
                    match p.transform.get() {
                        Some(tr) => {
                            let (px, py) = tr.project(x, z);
                            lua.pack_multi((px, py, tr.contains(x, z)))
                        }
                        None => error(lua, SYMBOL, "not_ready", "the game's map is not loaded yet"),
                    }
                })?,
            )?;
            let p = l.clone();
            t.set(
                "unproject",
                lua.create_function(move |lua, (x, y): (f32, f32)| {
                    const SYMBOL: &str = "eqoa.map.terrain.unproject";
                    if !x.is_finite() || !y.is_finite() {
                        return error(lua, SYMBOL, "invalid_argument", "finite coordinates required");
                    }
                    if !p.active.get() {
                        return stale(lua, SYMBOL);
                    }
                    match p.transform.get() {
                        Some(tr) => lua.pack_multi(tr.unproject(x, y)),
                        None => error(lua, SYMBOL, "not_ready", "the game's map is not loaded yet"),
                    }
                })?,
            )?;
            let close = l.clone();
            t.set("close", lua.create_function(move |_, _: Value| { close.release(); Ok(true) })?)?;
            let get = l.clone();
            t.set(
                "get_texture",
                lua.create_function(move |lua, ()| {
                    let value = texture_info(lua, &get)?;
                    if let Some(e) = get.error.borrow().as_ref() {
                        error(lua, "eqoa.map.terrain.texture", "quota", e)
                    } else {
                        lua.pack_multi((value, Value::Nil))
                    }
                })?,
            )?;
            let mt = lua.create_table()?;
            mt.set(
                "__index",
                lua.create_function(move |lua, (_, k): (Value, String)| match k.as_str() {
                    "texture" => texture_info(lua, &l),
                    "error" => match l.error.borrow().as_ref() {
                        Some(e) => super::native::record(
                            lua,
                            "eqoa.Error",
                            &serde_json::json!({"code":"quota","message":e,"capability":"eqoa.map.terrain.texture"}),
                        ),
                        None => Ok(Value::Nil),
                    },
                    "world_id" => Ok(if l.transform.get().is_some() { Value::Integer(l.world.get() as i64) } else { Value::Nil }),
                    "width" | "height" => Ok(match l.transform.get() {
                        Some(tr) => Value::Integer(if k == "width" { tr.size.0 } else { tr.size.1 } as i64),
                        None => Value::Nil,
                    }),
                    "bounds" => match l.transform.get() {
                        Some(tr) => {
                            let b = lua.create_table()?;
                            b.set("x_min", tr.extent[0])?;
                            b.set("z_min", tr.extent[1])?;
                            b.set("x_max", tr.extent[2])?;
                            b.set("z_max", tr.extent[3])?;
                            Ok(Value::Table(b))
                        }
                        None => Ok(Value::Nil),
                    },
                    "progress" => match l.transform.get() {
                        Some(_) => {
                            let (done, total) = l.progress.get();
                            let b = lua.create_table()?;
                            b.set("done", done)?;
                            b.set("total", total)?;
                            Ok(Value::Table(b))
                        }
                        None => Ok(Value::Nil),
                    },
                    _ => Ok(Value::Nil),
                })?,
            )?;
            t.set_metatable(Some(mt))?;
            lua.pack_multi((t, Value::Nil))
        })?,
    )
}
