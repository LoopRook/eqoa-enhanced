//! Round A13: the world map's terrain as a picture a mod can draw (`eqoa.map.terrain`), so a mod-drawn map needs no
//! terrain drawing of its own. Two shapes:
//!
//! - the whole world (no `center`): the overview the game's map holds, scaled to the size asked for;
//! - a view (`center`, `scale`, `width`, `height`): what the game's own map shows for that view, the same way: the
//!   overview first, and over it the 512-texel detail tiles the game's map uses at that zoom (`scale` at or below
//!   `TILE_SCALE`), for the zones the game's map has built. The client asks the game's cache for the tiles of each
//!   held view (`Map::api_terrain_keep`, from `a13_play::tick`), so they are kept on the GPU as the game's map keeps
//!   its own.
//!
//! Nothing here builds terrain: the game's map builds it (behind play, a few seconds after a world is entered); a
//! mod's picture fills in as the build goes on, redrawn in place (the texture handle stays).
use super::{
    host::Host,
    native::{bind, error},
    textures::{Handle, Textures},
};
use mlua::{Lua, Result, Value};
use render::{BufferInit, wgpu};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Longest side a terrain picture may be asked for, in pixels (the texture budget every mod has).
pub(crate) const MAX_SIZE: f32 = 2048.0;
pub(crate) const MIN_SIZE: f32 = 64.0;
pub(crate) const DEFAULT_SIZE: f32 = 1024.0;
/// The closest a view zooms (the game's map: a world unit a pixel) and the furthest a mod may ask for.
pub(crate) const MIN_SCALE: f32 = 1.0;
pub(crate) const MAX_SCALE: f32 = 4096.0;
/// Most terrain pictures one mod may hold at once.
const MAX_PER_VM: usize = 2;

/// One detail tile on the GPU.
pub(crate) struct TileSrc<'a> {
    pub zone: usize,
    /// The world rectangle it covers: x0, z0, x1, z1.
    pub rect: [f32; 4],
    pub texture: &'a wgpu::Texture,
}

/// What the map's layer offers each frame (`Map::api_terrain`).
pub(crate) struct Source<'a> {
    pub texture: &'a wgpu::Texture,
    /// The world rectangle the overview covers: x_min, z_min, x_max, z_max.
    pub extent: [f32; 4],
    /// The overview's size in texels.
    pub texels: (u32, u32),
    pub world: i32,
    /// Which texture of the map's layer it is (it changes when the world does).
    pub texture_index: usize,
    /// Zones on the map, of zones to draw.
    pub progress: (usize, usize),
    /// The detail tiles on the GPU (none while the game's map is browsing another world).
    pub tiles: Vec<TileSrc<'a>>,
    /// The scale (world units a pixel) at or below which the game's map uses tiles.
    pub tile_scale: f32,
}

/// A view of the world: the world point at the picture's middle and the world units a pixel covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewReq {
    pub center: [f32; 2],
    pub scale: f32,
}

/// World coordinates to the picture's pixels and back. Whole world: the picture's top left is (`x_min`, `z_min`);
/// a view: its middle is `center`. In both, x grows to the right and z grows downward (the map's own orientation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transform {
    pub extent: [f32; 4],
    pub size: (u32, u32),
    pub view: Option<ViewReq>,
}

impl Transform {
    pub(crate) fn project(self, x: f32, z: f32) -> (f32, f32) {
        if let Some(v) = self.view {
            return ((x - v.center[0]) / v.scale + self.size.0 as f32 / 2.0, (z - v.center[1]) / v.scale + self.size.1 as f32 / 2.0);
        }
        let (w, h) = (self.extent[2] - self.extent[0], self.extent[3] - self.extent[1]);
        ((x - self.extent[0]) / w * self.size.0 as f32, (z - self.extent[1]) / h * self.size.1 as f32)
    }
    pub(crate) fn unproject(self, px: f32, py: f32) -> (f32, f32) {
        if let Some(v) = self.view {
            return ((px - self.size.0 as f32 / 2.0) * v.scale + v.center[0], (py - self.size.1 as f32 / 2.0) * v.scale + v.center[1]);
        }
        let (w, h) = (self.extent[2] - self.extent[0], self.extent[3] - self.extent[1]);
        (self.extent[0] + px / self.size.0 as f32 * w, self.extent[1] + py / self.size.1 as f32 * h)
    }
    /// Whether the world point lies within the world's bounds (`bounds`).
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

/// What one picture is made of, bottom to top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pic {
    Overview,
    Tile(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlanQuad {
    pub pic: Pic,
    /// Where it lands in the picture, in pixels: x0, y0, x1, y1.
    pub dst: [f32; 4],
}

/// The quads a picture is drawn from: the whole world is the overview alone; a view is the overview and, at the
/// game's tile zoom, the tiles of the zones that meet the view (each as the game's own map places them).
pub(crate) fn plan(
    tr: Transform,
    tile_scale: f32,
    tiles: &[(usize, [f32; 4])],
) -> Vec<PlanQuad> {
    let at = |r: [f32; 4]| {
        let (a, b) = (tr.project(r[0], r[1]), tr.project(r[2], r[3]));
        [a.0, a.1, b.0, b.1]
    };
    let mut out = vec![PlanQuad { pic: Pic::Overview, dst: at(tr.extent) }];
    let Some(v) = tr.view else { return out };
    if v.scale > tile_scale {
        return out;
    }
    let (w, h) = (tr.size.0 as f32, tr.size.1 as f32);
    for &(zone, rect) in tiles {
        let d = at(rect);
        if d[2] < 0.0 || d[0] > w || d[3] < 0.0 || d[1] > h {
            continue;
        }
        out.push(PlanQuad { pic: Pic::Tile(zone), dst: d });
    }
    out
}

/// What a picture was drawn from: it is drawn again only when this changes.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    view: Option<ViewReq>,
    progress: (usize, usize),
    world: i32,
    texture_index: usize,
    tiles: Vec<usize>,
}

struct Canvas {
    id: u64,
    texture: wgpu::Texture,
    size: (u32, u32),
    key: Key,
}

struct Lease {
    vm: u64,
    /// Whole world: the longest side asked for. A view: unused.
    requested: f32,
    /// A view's picture size (a view only).
    view_size: (u32, u32),
    view: Cell<Option<ViewReq>>,
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

/// The views held now (centre, scale, size), for the client to keep their tiles (`Map::api_terrain_keep`).
pub(crate) fn view_requests() -> Vec<([f32; 2], f32, (u32, u32))> {
    STATE.with(|s| {
        s.borrow()
            .leases
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|l| l.active.get())
            .filter_map(|l| l.view.get().map(|v| (v.center, v.scale, l.view_size)))
            .collect()
    })
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
pub(crate) fn canvas_of_first() -> Option<(wgpu::Texture, (u32, u32), u64)> {
    STATE.with(|s| {
        s.borrow()
            .leases
            .iter()
            .filter_map(Weak::upgrade)
            .find_map(|l| l.canvas.borrow().as_ref().map(|c| (c.texture.clone(), c.size, c.id)))
    })
}

const SHADER: &str = r#"
struct Q { rect: vec4<f32> };
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> q: Q;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    var c = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    let u = c[i];
    var o: Out;
    o.pos = vec4<f32>(mix(q.rect.x, q.rect.z, u.x), mix(q.rect.y, q.rect.w, u.y), 0.0, 1.0);
    o.uv = u;
    return o;
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return textureSample(src, samp, in.uv);
}
"#;

struct Blit {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
}

impl Blit {
    fn new(device: &wgpu::Device) -> Self {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("addon terrain"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(1, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
                entry(
                    2,
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                ),
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
        // colour blended by the picture's own alpha (unbuilt ground is transparent); the target's alpha stays 1
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
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
                    blend: Some(blend),
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
/// play): every held terrain picture is drawn again when it is new, when its view or the world changed, when the game's
/// map has built more ground or loaded more tiles. A picture is drawn again in place: its texture and handle stay.
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
        let tile_rects: Vec<(usize, [f32; 4])> = src.tiles.iter().map(|t| (t.zone, t.rect)).collect();
        let mut drew = 0;
        for l in leases {
            let view = l.view.get();
            let size = if view.is_some() { l.view_size } else { canvas_size(l.requested, src.texels) };
            let tr = Transform { extent: src.extent, size, view };
            l.transform.set(Some(tr));
            l.world.set(src.world);
            l.progress.set(src.progress);
            let quads = plan(tr, src.tile_scale, &tile_rects);
            let key = Key {
                view,
                progress: src.progress,
                world: src.world,
                texture_index: src.texture_index,
                tiles: quads.iter().filter_map(|q| if let Pic::Tile(z) = q.pic { Some(z) } else { None }).collect(),
            };
            let stale = l.canvas.borrow().as_ref().is_none_or(|c| c.size != size || c.key != key);
            if !stale {
                continue;
            }
            // redrawn in place when the texture fits: the id (and so the mod's handle) stays
            let old = l.canvas.borrow_mut().take();
            let (texture, id) = match old {
                Some(c) if c.size == size => (c.texture, c.id),
                _ => {
                    s.serial += 1;
                    (
                        device.create_texture(&wgpu::TextureDescriptor {
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
                        s.serial,
                    )
                }
            };
            let blit = s.blit.as_ref().unwrap();
            let tv = texture.create_view(&Default::default());
            // one uniform and one bind group per quad, made before the pass that uses them
            let mut groups = Vec::with_capacity(quads.len());
            for q in &quads {
                let tex = match q.pic {
                    Pic::Overview => src.texture,
                    Pic::Tile(z) => match src.tiles.iter().find(|t| t.zone == z) {
                        Some(t) => t.texture,
                        None => continue,
                    },
                };
                let (w, h) = (size.0 as f32, size.1 as f32);
                let ndc = [q.dst[0] / w * 2.0 - 1.0, 1.0 - q.dst[1] / h * 2.0, q.dst[2] / w * 2.0 - 1.0, 1.0 - q.dst[3] / h * 2.0];
                let mut bytes = [0u8; 16];
                for (k, v) in ndc.iter().enumerate() {
                    bytes[k * 4..k * 4 + 4].copy_from_slice(&v.to_ne_bytes());
                }
                let buffer = device.buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("addon terrain quad"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let view = tex.create_view(&Default::default());
                groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("addon terrain"),
                    layout: &blit.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&blit.sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: buffer.as_entire_binding() },
                    ],
                }));
            }
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("addon terrain"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &tv,
                        depth_slice: None,
                        resolve_target: None,
                        // unbuilt ground: dark, opaque
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.04, g: 0.05, b: 0.07, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&blit.pipeline);
                for g in &groups {
                    pass.set_bind_group(0, g, &[]);
                    pass.draw(0..6, 0..1);
                }
            }
            drew += 1;
            *l.canvas.borrow_mut() = Some(Canvas { id, texture, size, key });
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

/// A finite number out of a Lua value.
fn number(v: &Value) -> Option<f32> {
    let n = match v {
        Value::Integer(n) => *n as f32,
        Value::Number(n) => *n as f32,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

/// `{x, z}` out of a table (`x`/`z` fields), or an `invalid_argument` reason.
fn centre(v: &Value) -> std::result::Result<[f32; 2], String> {
    let Value::Table(t) = v else { return Err("center must be {x = number, z = number}".into()) };
    let get = |k: &str| t.get::<Value>(k).ok().as_ref().and_then(number);
    match (get("x"), get("z")) {
        (Some(x), Some(z)) => Ok([x, z]),
        _ => Err("center must be {x = number, z = number}".into()),
    }
}

fn scale_of(v: &Value) -> std::result::Result<f32, (&'static str, String)> {
    let Some(n) = number(v) else { return Err(("invalid_argument", "scale must be a number".into())) };
    if !(MIN_SCALE..=MAX_SCALE).contains(&n) {
        return Err(("out_of_range", format!("scale must be {MIN_SCALE}..{MAX_SCALE} world units a pixel")));
    }
    Ok(n)
}

fn dimension(v: &Value, what: &str) -> std::result::Result<u32, (&'static str, String)> {
    let Some(n) = number(v) else { return Err(("invalid_argument", format!("{what} must be a number"))) };
    if !(MIN_SIZE..=MAX_SIZE).contains(&n) {
        return Err(("out_of_range", format!("{what} must be 64..2048")));
    }
    Ok(n.round() as u32)
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
            for pair in arg.clone().pairs::<Value, Value>() {
                let (k, _) = pair?;
                let known = matches!(&k, Value::String(s) if matches!(s.to_str().as_deref(), Ok("size" | "center" | "scale" | "width" | "height")));
                if !known {
                    return error(lua, SYMBOL, "invalid_argument", "only size, center, scale, width and height are supported");
                }
            }
            let field = |k: &str| arg.get::<Value>(k).unwrap_or(Value::Nil);
            let size = match field("size") {
                Value::Nil => None,
                v => match number(&v) {
                    Some(n) if (MIN_SIZE..=MAX_SIZE).contains(&n) => Some(n),
                    Some(_) => return error(lua, SYMBOL, "out_of_range", "size must be 64..2048"),
                    None => return error(lua, SYMBOL, "invalid_argument", "size must be a number"),
                },
            };
            let is_view = !matches!(field("center"), Value::Nil) || !matches!(field("scale"), Value::Nil)
                || !matches!(field("width"), Value::Nil) || !matches!(field("height"), Value::Nil);
            let (view, view_size) = if is_view {
                if matches!(field("center"), Value::Nil) {
                    return error(lua, SYMBOL, "invalid_argument", "a view needs center = {x, z}");
                }
                let c = match centre(&field("center")) {
                    Ok(c) => c,
                    Err(m) => return error(lua, SYMBOL, "invalid_argument", &m),
                };
                let dim = |k: &str| -> std::result::Result<u32, (&'static str, String)> {
                    match field(k) {
                        Value::Nil => Ok(size.unwrap_or(DEFAULT_SIZE).round() as u32),
                        v => dimension(&v, k),
                    }
                };
                let (w, h) = match (dim("width"), dim("height")) {
                    (Ok(w), Ok(h)) => (w, h),
                    (Err((c, m)), _) | (_, Err((c, m))) => return error(lua, SYMBOL, c, &m),
                };
                let scale = match field("scale") {
                    // the game's own opening scale for this picture: 2500 units across its shorter side
                    Value::Nil => (2500.0 / w.min(h) as f32).max(MIN_SCALE),
                    v => match scale_of(&v) {
                        Ok(s) => s,
                        Err((c, m)) => return error(lua, SYMBOL, c, &m),
                    },
                };
                (Some(ViewReq { center: c, scale }), (w, h))
            } else {
                (None, (0, 0))
            };
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
                requested: size.unwrap_or(DEFAULT_SIZE),
                view_size,
                view: Cell::new(view),
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
                        None => error(lua, SYMBOL, "unavailable", "the game's map is not loaded yet"),
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
                        None => error(lua, SYMBOL, "unavailable", "the game's map is not loaded yet"),
                    }
                })?,
            )?;
            let p = l.clone();
            t.set(
                "set_view",
                lua.create_function(move |lua, arg: Value| {
                    const SYMBOL: &str = "eqoa.map.terrain.set_view";
                    if !p.active.get() {
                        return stale(lua, SYMBOL);
                    }
                    let Some(now) = p.view.get() else {
                        return error(lua, SYMBOL, "invalid_argument", "this picture is the whole world: ask for a view with center to move one");
                    };
                    let Value::Table(t) = arg else {
                        return error(lua, SYMBOL, "invalid_argument", "set_view takes {center = {x, z}?, scale = number?}");
                    };
                    let mut next = now;
                    match t.get::<Value>("center").unwrap_or(Value::Nil) {
                        Value::Nil => {}
                        v => match centre(&v) {
                            Ok(c) => next.center = c,
                            Err(m) => return error(lua, SYMBOL, "invalid_argument", &m),
                        },
                    }
                    match t.get::<Value>("scale").unwrap_or(Value::Nil) {
                        Value::Nil => {}
                        v => match scale_of(&v) {
                            Ok(s) => next.scale = s,
                            Err((c, m)) => return error(lua, SYMBOL, c, &m),
                        },
                    }
                    p.view.set(Some(next));
                    // the transform follows at once (the next frame's draw also has the world's bounds)
                    if let Some(tr) = p.transform.get() {
                        p.transform.set(Some(Transform { view: Some(next), ..tr }));
                    }
                    lua.pack_multi(true)
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
