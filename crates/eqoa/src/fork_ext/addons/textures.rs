//! Bounded, owner-local texture handles. Lua never receives a GPU/native pointer.
use super::discovery::DiscoveredAddon;
use dear_imgui_rs::sys;
use dear_imgui_wgpu::{ExternalTextureId, WgpuRenderer};
use mlua::{AnyUserData, Lua, Result, Table, UserData, Value};
use render::wgpu;
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io::Read,
    path::PathBuf,
    rc::Rc,
};

#[derive(Clone, Copy, Debug)]
pub struct TexturesBudget {
    pub max_dimension: u32,
    pub max_count: usize,
    pub max_total_bytes: usize,
}

impl Default for TexturesBudget {
    fn default() -> Self {
        Self {
            max_dimension: 256,
            max_count: 16,
            max_total_bytes: 256 * 256 * 4 * 16, // 4 MiB
        }
    }
}

pub const ADDON_BUDGET: TexturesBudget = TexturesBudget {
    max_dimension: 2048,
    max_count: 128,
    max_total_bytes: 64 * 1024 * 1024, // 64 MiB
};

struct TextureEntry {
    external: ExternalTextureId,
    #[allow(dead_code)]
    texture: wgpu::Texture,
    alive: bool,
    byte_size: usize,
}

pub(super) struct Textures {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Rc<RefCell<WgpuRenderer>>,
    next: u64,
    entries: HashMap<u64, TextureEntry>,
    budget: TexturesBudget,
    allocated_bytes: usize,
    diagnostics: Vec<String>,
}

struct Handle {
    owner: Rc<RefCell<Textures>>,
    id: u64,
}
impl UserData for Handle {}

impl Textures {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: Rc<RefCell<WgpuRenderer>>,
    ) -> Rc<RefCell<Self>> {
        let budget = TexturesBudget::default();
        let max_hw = device.limits().max_texture_dimension_2d;
        let clamped = TexturesBudget {
            max_dimension: budget.max_dimension.min(max_hw),
            ..budget
        };
        Rc::new(RefCell::new(Self {
            device: device.clone(),
            queue: queue.clone(),
            renderer,
            next: 0,
            entries: HashMap::new(),
            budget: clamped,
            allocated_bytes: 0,
            diagnostics: Vec::new(),
        }))
    }

    pub fn set_budget(&mut self, budget: TexturesBudget) {
        let max_hw = self.device.limits().max_texture_dimension_2d;
        self.budget = TexturesBudget {
            max_dimension: budget.max_dimension.min(max_hw),
            ..budget
        };
    }

    pub fn add_diagnostic(&mut self, msg: String) {
        if !self.diagnostics.contains(&msg) {
            if self.diagnostics.len() >= 32 {
                self.diagnostics.remove(0);
            }
            self.diagnostics.push(msg);
        }
    }

    pub fn drain_diagnostics(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diagnostics)
    }

    fn rollback_upload(&mut self, id: u64) {
        if let Some(entry) = self.entries.remove(&id) {
            let unregister_res = {
                let mut renderer = self.renderer.borrow_mut();
                renderer.unregister_external_texture(entry.external)
            };
            if let Err(error) = unregister_res {
                self.add_diagnostic(format!("failed to unregister rolled back texture: {error}"));
            }
            self.allocated_bytes = self.allocated_bytes.saturating_sub(entry.byte_size);
        }
    }

    fn upload_rgba8(
        &mut self,
        owner: &Rc<RefCell<Self>>,
        label: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> std::result::Result<Handle, String> {
        if width == 0 || height == 0 {
            return Err("zero dimension texture".to_string());
        }
        if width > self.budget.max_dimension || height > self.budget.max_dimension {
            return Err(format!(
                "texture dimensions {width}x{height} exceed max allowed {}",
                self.budget.max_dimension
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|px| px.checked_mul(4))
            .ok_or_else(|| "dimension overflow".to_string())?;
        if rgba.len() != expected {
            return Err(format!(
                "RGBA byte length mismatch: expected {expected}, got {}",
                rgba.len()
            ));
        }
        if self.entries.len() >= self.budget.max_count {
            return Err(format!(
                "texture count budget exceeded ({}/{})",
                self.entries.len(),
                self.budget.max_count
            ));
        }
        if self.allocated_bytes.saturating_add(expected) > self.budget.max_total_bytes {
            return Err(format!(
                "texture memory budget exceeded ({} + {} > {})",
                self.allocated_bytes, expected, self.budget.max_total_bytes
            ));
        }

        let next_id = self
            .next
            .checked_add(1)
            .ok_or_else(|| "texture ID counter exhausted".to_string())?;

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            texture.size(),
        );

        let external = self
            .renderer
            .borrow_mut()
            .register_external_texture(&texture.create_view(&Default::default()))
            .map_err(|e| format!("failed to register external texture: {e}"))?;

        self.next = next_id;
        self.allocated_bytes += expected;
        self.entries.insert(
            next_id,
            TextureEntry {
                external,
                texture,
                alive: true,
                byte_size: expected,
            },
        );
        Ok(Handle {
            owner: owner.clone(),
            id: next_id,
        })
    }

    fn install_memory_and_release(owner: &Rc<RefCell<Self>>, lua: &Lua) -> Result<()> {
        let forge: Table = lua.globals().get("UiForge")?;
        let bank = owner.clone();
        forge.set(
            "CreateTextureFromMemory",
            lua.create_function(
                move |lua, (bytes, width, height): (mlua::LuaString, u32, u32)| {
                    match upload_and_create_userdata(
                        &bank,
                        lua,
                        "Forge memory texture",
                        width,
                        height,
                        &bytes.as_bytes(),
                    ) {
                        Ok(ud) => Ok(ud),
                        Err(e) => Err(mlua::Error::runtime(format!("spike textures: {e}"))),
                    }
                },
            )?,
        )?;
        let bank = owner.clone();
        forge.set(
            "ReleaseTexture",
            lua.create_function(move |_, handle: AnyUserData| {
                let handle = handle.borrow::<Handle>()?;
                if !Rc::ptr_eq(&bank, &handle.owner) {
                    return Err(mlua::Error::runtime("foreign texture handle"));
                }
                let mut bank = bank.borrow_mut();
                let entry = bank
                    .entries
                    .get_mut(&handle.id)
                    .ok_or_else(|| mlua::Error::runtime("expired texture handle"))?;
                entry.alive = false;
                Ok(())
            })?,
        )?;
        Ok(())
    }

    pub fn install(owner: &Rc<RefCell<Self>>, lua: &Lua) -> Result<()> {
        Self::install_memory_and_release(owner, lua)
    }

    pub fn install_for_addon(
        owner: &Rc<RefCell<Self>>,
        lua: &Lua,
        addon: &DiscoveredAddon,
    ) -> Result<()> {
        owner.borrow_mut().set_budget(ADDON_BUDGET);
        Self::install_memory_and_release(owner, lua)?;

        let forge: Table = lua.globals().get("UiForge")?;

        // Shared UiForge.resources_path: <script_root>/resources
        let shared_res_path = addon
            .script_root
            .join("resources")
            .to_string_lossy()
            .to_string();
        forge.set("resources_path", shared_res_path)?;

        let bank = owner.clone();
        let addon_data = addon.clone();

        let load_texture = lua.create_function(move |lua, path_str: String| {
            if path_str.is_empty() {
                bank.borrow_mut()
                    .add_diagnostic("empty texture path rejected".to_string());
                return Ok(Value::Nil);
            }

            // 1. Reject parent traversal including both separators
            let normalized = path_str.replace('\\', "/");
            for seg in normalized.split('/') {
                if seg == ".." {
                    bank.borrow_mut().add_diagnostic(format!(
                        "parent traversal '..' rejected in texture path: {path_str}"
                    ));
                    return Ok(Value::Nil);
                }
            }

            // 2. Determine relative vs absolute
            let is_relative = !path_str.starts_with('/')
                && !path_str.starts_with('\\')
                && !path_str.contains(':');

            let canon_file = if is_relative {
                match super::package::resolve_resource(&addon_data, &path_str) {
                    Ok(p) => p,
                    Err(err) => {
                        bank.borrow_mut().add_diagnostic(format!(
                            "failed to resolve relative texture '{path_str}': {err}"
                        ));
                        return Ok(Value::Nil);
                    }
                }
            } else {
                let pb = normalize_texture_path_for_platform(&path_str);
                let canon = match pb.canonicalize() {
                    Ok(p) => p,
                    Err(e) => {
                        bank.borrow_mut()
                            .add_diagnostic(format!("texture file unresolvable '{path_str}': {e}"));
                        return Ok(Value::Nil);
                    }
                };
                if !canon.starts_with(&addon_data.script_root) {
                    bank.borrow_mut().add_diagnostic(format!(
                        "texture path '{path_str}' escapes script root: {}",
                        canon.display()
                    ));
                    return Ok(Value::Nil);
                }
                let in_res_dir = addon_data
                    .resource_dirs
                    .iter()
                    .any(|d| canon.starts_with(d));
                if !in_res_dir {
                    bank.borrow_mut().add_diagnostic(format!(
                        "texture path '{path_str}' is outside discovered resource directories"
                    ));
                    return Ok(Value::Nil);
                }
                if !canon.is_file() {
                    bank.borrow_mut().add_diagnostic(format!(
                        "texture path '{path_str}' is not a file: {}",
                        canon.display()
                    ));
                    return Ok(Value::Nil);
                }
                canon
            };

            // 3. Read bounded to 16 MiB
            let mut file = match fs::File::open(&canon_file) {
                Ok(f) => f,
                Err(e) => {
                    bank.borrow_mut().add_diagnostic(format!(
                        "failed to open texture '{}': {e}",
                        canon_file.display()
                    ));
                    return Ok(Value::Nil);
                }
            };
            let mut buf = Vec::new();
            let bytes_read = match (&mut file).take(16 * 1024 * 1024 + 1).read_to_end(&mut buf) {
                Ok(b) => b,
                Err(e) => {
                    bank.borrow_mut().add_diagnostic(format!(
                        "failed to read texture '{}': {e}",
                        canon_file.display()
                    ));
                    return Ok(Value::Nil);
                }
            };
            if bytes_read > 16 * 1024 * 1024 {
                bank.borrow_mut().add_diagnostic(format!(
                    "texture file '{}' exceeds 16 MiB limit",
                    canon_file.display()
                ));
                return Ok(Value::Nil);
            }

            // 4. Decode PNG
            let max_dim = bank.borrow().budget.max_dimension;
            let (width, height, rgba) = match decode_png_to_rgba8(&buf, max_dim) {
                Ok(res) => res,
                Err(e) => {
                    bank.borrow_mut().add_diagnostic(format!(
                        "failed to decode PNG '{}': {e}",
                        canon_file.display()
                    ));
                    return Ok(Value::Nil);
                }
            };

            // 5. Consolidated GPU upload with transactional rollback
            let label = canon_file.file_name().unwrap_or_default().to_string_lossy();
            match upload_and_create_userdata(&bank, lua, &label, width, height, &rgba) {
                Ok(ud) => Ok(Value::UserData(ud)),
                Err(e) => {
                    bank.borrow_mut().add_diagnostic(format!(
                        "failed to upload texture '{}': {e}",
                        canon_file.display()
                    ));
                    Ok(Value::Nil)
                }
            }
        })?;

        forge.set("LoadTexture", load_texture.clone())?;

        let igraphics: Table = match forge.get("IGraphicsApi") {
            Ok(t) => t,
            Err(_) => {
                let t = lua.create_table()?;
                forge.set("IGraphicsApi", t.clone())?;
                t
            }
        };
        igraphics.set("CreateTextureFromFile", load_texture)?;

        Ok(())
    }

    pub fn raw(handle: &AnyUserData) -> Result<sys::ImTextureRef_c> {
        let handle = handle.borrow::<Handle>()?;
        let bank = handle.owner.borrow();
        let entry = bank
            .entries
            .get(&handle.id)
            .filter(|e| e.alive)
            .ok_or_else(|| mlua::Error::runtime("expired texture handle"))?;
        Ok(sys::ImTextureRef_c {
            _TexData: std::ptr::null_mut(),
            _TexID: entry.external.texture_id().id(),
        })
    }

    pub fn collect(&mut self, all: bool) {
        let mut renderer = self.renderer.borrow_mut();
        self.entries.retain(|_, entry| {
            if all || !entry.alive {
                if let Err(error) = renderer.unregister_external_texture(entry.external) {
                    eprintln!("Forge texture cleanup: {error}");
                }
                self.allocated_bytes = self.allocated_bytes.saturating_sub(entry.byte_size);
                false
            } else {
                true
            }
        });
        if all {
            self.allocated_bytes = 0;
        }
    }
}

fn normalize_path_str(path_str: &str, is_windows: bool) -> String {
    if is_windows {
        path_str.to_string()
    } else {
        path_str.replace('\\', "/")
    }
}

pub(crate) fn normalize_texture_path_for_platform(path_str: &str) -> PathBuf {
    PathBuf::from(normalize_path_str(path_str, cfg!(windows)))
}

fn upload_and_create_userdata_with<F>(
    owner: &Rc<RefCell<Textures>>,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    make_userdata: F,
) -> std::result::Result<AnyUserData, String>
where
    F: FnOnce(Handle) -> std::result::Result<AnyUserData, mlua::Error>,
{
    let handle = {
        let mut bank = owner.borrow_mut();
        bank.upload_rgba8(owner, label, width, height, rgba)?
    };
    let id = handle.id;
    match make_userdata(handle) {
        Ok(ud) => Ok(ud),
        Err(lua_err) => {
            let mut bank = owner.borrow_mut();
            bank.rollback_upload(id);
            Err(format!(
                "failed to allocate Lua userdata for texture: {lua_err}"
            ))
        }
    }
}

fn upload_and_create_userdata(
    owner: &Rc<RefCell<Textures>>,
    lua: &Lua,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> std::result::Result<AnyUserData, String> {
    upload_and_create_userdata_with(owner, label, width, height, rgba, |handle| {
        lua.create_userdata(handle)
    })
}

pub(crate) fn decode_png_to_rgba8(
    bytes: &[u8],
    max_dim: u32,
) -> std::result::Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("invalid PNG header: {e}"))?;
    let info = reader.info();
    let width = info.width;
    let height = info.height;
    if width == 0 || height == 0 {
        return Err("zero dimension PNG".to_string());
    }
    if width > max_dim || height > max_dim {
        return Err(format!(
            "PNG dimensions {width}x{height} exceed limit {max_dim}"
        ));
    }
    let buf_size = reader
        .output_buffer_size()
        .ok_or_else(|| "output buffer size exceeds platform limits".to_string())?;
    let mut raw = vec![0u8; buf_size];
    let output_info = reader
        .next_frame(&mut raw)
        .map_err(|e| format!("corrupt PNG frame: {e}"))?;
    raw.truncate(output_info.buffer_size());

    let rgba = match output_info.color_type {
        png::ColorType::Rgba => raw,
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for chunk in raw.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for &g in &raw {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for chunk in raw.chunks_exact(2) {
                out.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            out
        }
        png::ColorType::Indexed => {
            return Err("unexpanded indexed PNG palette".to_string());
        }
    };
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(format!(
            "decoded byte length mismatch: expected {}, got {}",
            width as usize * height as usize * 4,
            rgba.len()
        ));
    }
    Ok((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use crate::fork_ext::addons::bridge::Bridge;
    use crate::fork_ext::addons::discovery::discover;
    use crate::fork_ext::addons::host::Host;
    use dear_imgui_rs::Context;
    use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("eqoa_tex_{}_{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct TestGpu {
        device: wgpu::Device,
        queue: wgpu::Queue,
        context: Context,
        renderer: Rc<RefCell<WgpuRenderer>>,
    }

    impl TestGpu {
        fn try_new() -> Option<Self> {
            let instance = wgpu::Instance::default();
            let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
            let (device, queue) =
                pollster::block_on(adapter.request_device(&Default::default())).ok()?;
            let mut context = Context::create();
            context.set_ini_filename::<PathBuf>(None).ok()?;
            let renderer = Rc::new(RefCell::new(
                WgpuRenderer::new(
                    WgpuInitInfo::new(
                        device.clone(),
                        queue.clone(),
                        wgpu::TextureFormat::Rgba8Unorm,
                    ),
                    &mut context,
                )
                .ok()?,
            ));
            Some(Self {
                device,
                queue,
                context,
                renderer,
            })
        }
    }

    fn encode_test_png(
        width: u32,
        height: u32,
        color_type: png::ColorType,
        pixels: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(color_type);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
        drop(writer);
        buf
    }

    #[test]
    fn forge_texture_grayscale_rgb_and_palette_conversion_and_dimension_limits() {
        // 1. Grayscale 8-bit 2x2
        let gray_bytes = vec![100u8, 150, 200, 250];
        let png_gray = encode_test_png(2, 2, png::ColorType::Grayscale, &gray_bytes);
        let (w, h, rgba) = decode_png_to_rgba8(&png_gray, 2048).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(
            rgba,
            vec![
                100, 100, 100, 255, 150, 150, 150, 255, 200, 200, 200, 255, 250, 250, 250, 255
            ]
        );

        // 2. RGB 8-bit 1x2 (no alpha)
        let rgb_bytes = vec![10u8, 20, 30, 40, 50, 60];
        let png_rgb = encode_test_png(1, 2, png::ColorType::Rgb, &rgb_bytes);
        let (w, h, rgba) = decode_png_to_rgba8(&png_rgb, 2048).unwrap();
        assert_eq!((w, h), (1, 2));
        assert_eq!(rgba, vec![10, 20, 30, 255, 40, 50, 60, 255]);

        // 3. Grayscale+Alpha 8-bit 1x1
        let ga_bytes = vec![77u8, 128];
        let png_ga = encode_test_png(1, 1, png::ColorType::GrayscaleAlpha, &ga_bytes);
        let (w, h, rgba) = decode_png_to_rgba8(&png_ga, 2048).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(rgba, vec![77, 77, 77, 128]);

        // 4. Zero dimensions rejection
        let zero_png = vec![0x89, 0x50, 0x4E, 0x47];
        assert!(decode_png_to_rgba8(&zero_png, 2048).is_err());

        // 5. Oversized dimensions rejection
        let big_png = encode_test_png(16, 16, png::ColorType::Rgba, &vec![0u8; 16 * 16 * 4]);
        let err = decode_png_to_rgba8(&big_png, 8).unwrap_err();
        assert!(err.contains("exceed limit 8"), "got: {err}");
    }

    #[test]
    fn forge_texture_private_and_shared_png_distinct_colors() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("private_shared_colors");
        let root = temp.path();

        // Shared icon.png (RED 2x2)
        fs::create_dir_all(root.join("resources")).unwrap();
        let red_png = encode_test_png(2, 2, png::ColorType::Rgba, &vec![255, 0, 0, 255].repeat(4));
        fs::write(root.join("resources").join("icon.png"), red_png).unwrap();

        // Private icon.png (GREEN 2x2)
        let pkg = root.join("color_pack");
        fs::create_dir_all(pkg.join("resources")).unwrap();
        let green_png =
            encode_test_png(2, 2, png::ColorType::Rgba, &vec![0, 255, 0, 255].repeat(4));
        fs::write(pkg.join("resources").join("icon.png"), green_png).unwrap();
        fs::write(pkg.join("color_pack.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "color_pack").unwrap();
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        // Relative lookup must prefer private resource
        let res: Value = host
            .lua
            .load("return UiForge.LoadTexture('icon.png')")
            .eval()
            .unwrap();
        assert!(matches!(res, Value::UserData(_)));
        assert_eq!(textures.borrow().entries.len(), 1);
        textures.borrow_mut().collect(true);
    }

    #[test]
    fn forge_texture_explicit_shared_absolute_path_alias() {
        let Some(mut gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("explicit_shared_path");
        let root = temp.path();

        // Shared heart.png (BLUE)
        let shared_dir = root.join("resources").join("retro_health_hearts");
        fs::create_dir_all(&shared_dir).unwrap();
        let blue_png = encode_test_png(2, 2, png::ColorType::Rgba, &vec![0, 0, 255, 255].repeat(4));
        fs::write(shared_dir.join("heart.png"), blue_png).unwrap();

        // Private heart.png (YELLOW)
        let pkg = root.join("hearts_mod");
        let priv_dir = pkg.join("resources").join("retro_health_hearts");
        fs::create_dir_all(&priv_dir).unwrap();
        let yellow_png = encode_test_png(
            2,
            2,
            png::ColorType::Rgba,
            &vec![255, 255, 0, 255].repeat(4),
        );
        fs::write(priv_dir.join("heart.png"), yellow_png).unwrap();
        fs::write(pkg.join("hearts_mod.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = disc.addons.iter().find(|a| a.id == "hearts_mod").unwrap();
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        // Absolute path from UiForge.resources_path must resolve to shared heart.png, not private!
        let script = r#"
            local p = UiForge.resources_path .. "\\retro_health_hearts\\heart.png"
            shared_tex = UiForge.IGraphicsApi.CreateTextureFromFile(p)
            return shared_tex
        "#;
        let res: Value = host.lua.load(script).eval().unwrap();
        assert!(matches!(res, Value::UserData(_)));
        assert_eq!(textures.borrow().entries.len(), 1);

        let bridge = Bridge::install(&host).unwrap();
        let render_fn = host
            .compile(
                r#"
                if ImGui.Begin("Shared Color Test", true, ImGuiWindowFlags.AlwaysAutoResize) then
                    ImGui.Image(shared_tex, 16, 16)
                end
                ImGui.End()
            "#,
            )
            .unwrap();

        let (w, h) = (320, 240);
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shared color test target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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

        for pass in 0..2 {
            gpu.context.io_mut().set_display_size([w as f32, h as f32]);
            gpu.context
                .io_mut()
                .set_display_framebuffer_scale([1.0, 1.0]);
            gpu.context.io_mut().set_delta_time(1.0 / 60.0);
            gpu.context.frame();
            bridge.invoke(|| host.run(&render_fn)).unwrap();

            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            {
                let mut renderer = gpu.renderer.borrow_mut();
                let frame = gpu.context.render(renderer.renderer_consumer().unwrap());
                let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Color pass"),
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
                    .render(frame, &mut rp, FramebufferExtent::new(w, h))
                    .unwrap();
            }

            if pass == 0 {
                gpu.queue.submit([encoder.finish()]);
                continue;
            }

            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (w * h * 4) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(w * 4),
                        rows_per_image: Some(h),
                    },
                },
                target.size(),
            );
            gpu.queue.submit([encoder.finish()]);

            let (send, recv) = std::sync::mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            recv.recv().unwrap().unwrap();

            let pixels = buffer.slice(..).get_mapped_range().unwrap();
            let rendered_blue = pixels
                .chunks_exact(4)
                .any(|p| p[0] < 50 && p[1] < 50 && p[2] > 200);
            let rendered_yellow = pixels
                .chunks_exact(4)
                .any(|p| p[0] > 200 && p[1] > 200 && p[2] < 50);
            assert!(
                rendered_blue,
                "shared blue texture [0, 0, 255] must be rendered"
            );
            assert!(
                !rendered_yellow,
                "private yellow texture must NOT be rendered for shared absolute path"
            );
        }

        textures.borrow_mut().collect(true);
    }

    #[test]
    fn forge_texture_nil_and_diagnostics_on_missing_corrupt_unsupported_and_traversal() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("texture_errors");
        let root = temp.path();

        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(
            root.join("resources").join("corrupt.png"),
            "not a valid png file",
        )
        .unwrap();
        fs::write(
            root.join("resources").join("fake.jpg"),
            b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00",
        )
        .unwrap();
        fs::write(root.join("mod.lua"), "-- mod").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        // 1. Missing file -> nil
        let missing: Value = host
            .lua
            .load("return UiForge.LoadTexture('nonexistent.png')")
            .eval()
            .unwrap();
        assert_eq!(missing, Value::Nil);

        // 2. Corrupt PNG -> nil
        let corrupt: Value = host
            .lua
            .load("return UiForge.LoadTexture('corrupt.png')")
            .eval()
            .unwrap();
        assert_eq!(corrupt, Value::Nil);

        // 3. Unsupported JPEG -> nil
        let jpg: Value = host
            .lua
            .load("return UiForge.LoadTexture('fake.jpg')")
            .eval()
            .unwrap();
        assert_eq!(jpg, Value::Nil);

        // 4. Relative parent traversal -> nil
        let trav: Value = host
            .lua
            .load("return UiForge.LoadTexture('../outside.png')")
            .eval()
            .unwrap();
        assert_eq!(trav, Value::Nil);

        // 5. Absolute parent traversal -> nil
        let abs_trav: Value = host
            .lua
            .load("return UiForge.LoadTexture(UiForge.resources_path .. '/../../escape.png')")
            .eval()
            .unwrap();
        assert_eq!(abs_trav, Value::Nil);

        // 6. Outside root -> nil
        let outside: Value = host
            .lua
            .load("return UiForge.LoadTexture('C:\\Windows\\System32\\bad.png')")
            .eval()
            .unwrap();
        assert_eq!(outside, Value::Nil);

        let diagnostics = textures.borrow_mut().drain_diagnostics();
        assert!(
            diagnostics.len() >= 5,
            "expected at least 5 diagnostics, got: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().any(
                |d| d.contains("failed to resolve relative texture") || d.contains("not found")
            )
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("failed to decode PNG") || d.contains("invalid PNG header"))
        );
        assert!(diagnostics.iter().any(|d| d.contains("parent traversal")));
    }

    #[test]
    fn forge_texture_cumulative_budget_and_release_cleanup() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("texture_budget");
        let root = temp.path();

        fs::create_dir_all(root.join("resources")).unwrap();
        let png_bytes =
            encode_test_png(2, 2, png::ColorType::Rgba, &vec![10, 20, 30, 255].repeat(4));
        fs::write(root.join("resources").join("t1.png"), &png_bytes).unwrap();
        fs::write(root.join("resources").join("t2.png"), &png_bytes).unwrap();
        fs::write(root.join("resources").join("t3.png"), &png_bytes).unwrap();
        fs::write(root.join("mod.lua"), "-- mod").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();
        // Set tight budget: max 2 textures, 64 bytes total AFTER install_for_addon
        textures.borrow_mut().set_budget(TexturesBudget {
            max_dimension: 256,
            max_count: 2,
            max_total_bytes: 64,
        });

        // Alloc 1 & 2
        host.lua
            .load("t1 = UiForge.LoadTexture('t1.png'); assert(t1 ~= nil)")
            .exec()
            .unwrap();
        host.lua
            .load("t2 = UiForge.LoadTexture('t2.png'); assert(t2 ~= nil)")
            .exec()
            .unwrap();

        // Alloc 3 fails due to count budget
        let t3: Value = host
            .lua
            .load("return UiForge.LoadTexture('t3.png')")
            .eval()
            .unwrap();
        assert_eq!(t3, Value::Nil);
        let diags = textures.borrow_mut().drain_diagnostics();
        assert!(diags.iter().any(|d| d.contains("budget exceeded")));

        // Release t1 in Lua
        host.lua.load("UiForge.ReleaseTexture(t1)").exec().unwrap();
        // Still 2 entries in bank (released but pending collect)
        assert_eq!(textures.borrow().entries.len(), 2);

        // Collect dead textures
        textures.borrow_mut().collect(false);
        assert_eq!(textures.borrow().entries.len(), 1);

        // Alloc 3 now succeeds!
        let t3_new: Value = host
            .lua
            .load("return UiForge.LoadTexture('t3.png')")
            .eval()
            .unwrap();
        assert!(matches!(t3_new, Value::UserData(_)));

        // Collect all resets accounting
        textures.borrow_mut().collect(true);
        assert_eq!(textures.borrow().entries.len(), 0);
        assert_eq!(textures.borrow().allocated_bytes, 0);
    }

    #[test]
    fn forge_texture_wgpu_headless_render_and_pixel_readback() {
        let Some(mut gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("wgpu_render_readback");
        let root = temp.path();

        fs::create_dir_all(root.join("resources")).unwrap();
        // 16x16 pure purple RGBA
        let purple_pixel = [180u8, 40, 220, 255];
        let purple_png =
            encode_test_png(16, 16, png::ColorType::Rgba, &purple_pixel.repeat(16 * 16));
        fs::write(root.join("resources").join("purple.png"), purple_png).unwrap();
        fs::write(root.join("mod.lua"), "-- main").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        let script = host
            .compile(
                r#"
                if not tex then
                    tex = UiForge.LoadTexture("purple.png")
                    assert(tex ~= nil)
                end
                if ImGui.Begin("Texture Test", true, ImGuiWindowFlags.AlwaysAutoResize) then
                    ImGui.Image(tex, 32, 32)
                end
                ImGui.End()
            "#,
            )
            .unwrap();

        let (width, height) = (320, 240);
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture acceptance target"),
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
        });
        let view = target.create_view(&Default::default());

        for pass_index in 0..2 {
            gpu.context
                .io_mut()
                .set_display_size([width as f32, height as f32]);
            gpu.context
                .io_mut()
                .set_display_framebuffer_scale([1.0, 1.0]);
            gpu.context.io_mut().set_delta_time(1.0 / 60.0);
            gpu.context.frame();

            bridge.invoke(|| host.run(&script)).unwrap();

            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            {
                let mut renderer = gpu.renderer.borrow_mut();
                let frame = gpu.context.render(renderer.renderer_consumer().unwrap());
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Texture pass"),
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
                    .render(frame, &mut pass, FramebufferExtent::new(width, height))
                    .unwrap();
            }

            if pass_index == 0 {
                gpu.queue.submit([encoder.finish()]);
                continue;
            }

            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (width * height * 4) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                },
                target.size(),
            );
            gpu.queue.submit([encoder.finish()]);

            let (send, recv) = std::sync::mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            recv.recv().unwrap().unwrap();

            let pixels = buffer.slice(..).get_mapped_range().unwrap();
            let rendered_purple = pixels
                .chunks_exact(4)
                .any(|p| p[0] > 140 && p[1] < 80 && p[2] > 180);
            assert!(
                rendered_purple,
                "WGPU render pass must render purple pixels from decoded PNG texture"
            );
        }

        textures.borrow_mut().collect(true);
    }

    #[test]
    fn forge_texture_diagnostics_dedup_full_ring_preserves_all() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());

        for i in 0..32 {
            textures
                .borrow_mut()
                .add_diagnostic(format!("diagnostic {i}"));
        }
        assert_eq!(textures.borrow().diagnostics.len(), 32);
        assert_eq!(textures.borrow().diagnostics[0], "diagnostic 0");
        assert_eq!(textures.borrow().diagnostics[31], "diagnostic 31");

        // Repeat an existing diagnostic multiple times
        for _ in 0..10 {
            textures
                .borrow_mut()
                .add_diagnostic("diagnostic 5".to_string());
        }

        assert_eq!(
            textures.borrow().diagnostics.len(),
            32,
            "deduplication must preserve buffer length without shrinking"
        );
        assert_eq!(
            textures.borrow().diagnostics[0],
            "diagnostic 0",
            "duplicate message must not evict oldest entry"
        );
        assert!(
            textures
                .borrow()
                .diagnostics
                .contains(&"diagnostic 31".to_string()),
            "all existing diagnostics must be retained"
        );
    }

    #[test]
    fn forge_texture_early_memory_limit_rejects_before_upload() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("userdata_rollback");
        let root = temp.path();

        fs::create_dir_all(root.join("resources")).unwrap();
        let png_bytes =
            encode_test_png(2, 2, png::ColorType::Rgba, &vec![10, 20, 30, 255].repeat(4));
        fs::write(root.join("resources").join("t.png"), &png_bytes).unwrap();
        fs::write(root.join("mod.lua"), "-- mod").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        assert_eq!(textures.borrow().entries.len(), 0);
        assert_eq!(textures.borrow().allocated_bytes, 0);

        // Constrain memory limit to current used bytes so create_userdata will fail
        let current_mem = host.lua.used_memory();
        host.lua.set_memory_limit(current_mem).unwrap();

        let res: Value = host
            .lua
            .load("return UiForge.LoadTexture('t.png')")
            .eval()
            .unwrap_or(Value::Nil);
        assert_eq!(res, Value::Nil);

        assert_eq!(
            textures.borrow().entries.len(),
            0,
            "failed userdata creation must roll back bank entries"
        );
        assert_eq!(
            textures.borrow().allocated_bytes,
            0,
            "failed userdata creation must roll back allocated bytes"
        );

        host.lua.set_memory_limit(8 * 1024 * 1024).unwrap();
    }

    #[test]
    fn forge_texture_memory_early_memory_limit_rejects_before_upload() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let host = Host::new().unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install(&textures, &host.lua).unwrap();

        let raw_rgba = vec![255u8, 0, 0, 255].repeat(4);
        host.lua
            .globals()
            .set("raw_pixels", host.lua.create_string(&raw_rgba).unwrap())
            .unwrap();

        assert_eq!(textures.borrow().entries.len(), 0);
        assert_eq!(textures.borrow().allocated_bytes, 0);

        let current_mem = host.lua.used_memory();
        host.lua.set_memory_limit(current_mem).unwrap();

        let res: std::result::Result<Value, mlua::Error> = host
            .lua
            .load("return UiForge.CreateTextureFromMemory(raw_pixels, 2, 2)")
            .eval();
        assert!(res.is_err(), "expected memory error on userdata creation");

        assert_eq!(
            textures.borrow().entries.len(),
            0,
            "failed memory userdata creation must roll back bank entries"
        );
        assert_eq!(
            textures.borrow().allocated_bytes,
            0,
            "failed memory userdata creation must roll back allocated bytes"
        );

        host.lua.set_memory_limit(8 * 1024 * 1024).unwrap();
    }

    #[test]
    fn forge_texture_byte_memory_budget_exhaustion_separate_from_count() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("byte_budget");
        let root = temp.path();

        fs::create_dir_all(root.join("resources")).unwrap();
        let png_bytes = encode_test_png(2, 2, png::ColorType::Rgba, &vec![1, 2, 3, 255].repeat(4));
        fs::write(root.join("resources").join("t1.png"), &png_bytes).unwrap();
        fs::write(root.join("resources").join("t2.png"), &png_bytes).unwrap();
        fs::write(root.join("resources").join("t3.png"), &png_bytes).unwrap();
        fs::write(root.join("mod.lua"), "-- mod").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        // 10 handles allowed, but only 32 total bytes (each 2x2 texture is 16 bytes)
        textures.borrow_mut().set_budget(TexturesBudget {
            max_dimension: 256,
            max_count: 10,
            max_total_bytes: 32,
        });

        let t1: Value = host
            .lua
            .load("return UiForge.LoadTexture('t1.png')")
            .eval()
            .unwrap();
        assert!(matches!(t1, Value::UserData(_)));
        let t2: Value = host
            .lua
            .load("return UiForge.LoadTexture('t2.png')")
            .eval()
            .unwrap();
        assert!(matches!(t2, Value::UserData(_)));
        assert_eq!(textures.borrow().allocated_bytes, 32);
        assert_eq!(textures.borrow().entries.len(), 2);

        // 3rd allocation fails on byte budget, NOT count budget
        let t3: Value = host
            .lua
            .load("return UiForge.LoadTexture('t3.png')")
            .eval()
            .unwrap();
        assert_eq!(t3, Value::Nil);

        let diags = textures.borrow_mut().drain_diagnostics();
        assert!(
            diags
                .iter()
                .any(|d| d.contains("texture memory budget exceeded")),
            "expected memory budget diagnostic, got: {diags:?}"
        );
        assert!(
            !diags
                .iter()
                .any(|d| d.contains("texture count budget exceeded")),
            "count budget must not be exceeded"
        );

        let h1 = match t1 {
            Value::UserData(ud) => ud,
            _ => unreachable!(),
        };
        host.lua.globals().set("t1", h1).unwrap();
        host.lua.load("UiForge.ReleaseTexture(t1)").exec().unwrap();
        textures.borrow_mut().collect(false);

        assert_eq!(textures.borrow().allocated_bytes, 16);
        assert_eq!(textures.borrow().entries.len(), 1);

        let t3_new: Value = host
            .lua
            .load("return UiForge.LoadTexture('t3.png')")
            .eval()
            .unwrap();
        assert!(matches!(t3_new, Value::UserData(_)));
        assert_eq!(textures.borrow().allocated_bytes, 32);
        assert_eq!(textures.borrow().entries.len(), 2);

        textures.borrow_mut().collect(true);
    }

    #[test]
    fn forge_texture_id_counter_exhaustion_rejects_without_leak() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let temp = TempDir::new("id_exhaustion");
        let root = temp.path();

        fs::create_dir_all(root.join("resources")).unwrap();
        let png_bytes = encode_test_png(2, 2, png::ColorType::Rgba, &vec![1, 2, 3, 255].repeat(4));
        fs::write(root.join("resources").join("t.png"), &png_bytes).unwrap();
        fs::write(root.join("mod.lua"), "-- mod").unwrap();

        let disc = discover(root);
        let addon = &disc.addons[0];
        let host = Host::for_addon(addon).unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, addon).unwrap();

        // Simulate ID counter exhaustion
        textures.borrow_mut().next = u64::MAX;

        let res: Value = host
            .lua
            .load("return UiForge.LoadTexture('t.png')")
            .eval()
            .unwrap();
        assert_eq!(res, Value::Nil);

        let diags = textures.borrow_mut().drain_diagnostics();
        assert!(
            diags
                .iter()
                .any(|d| d.contains("texture ID counter exhausted")),
            "expected exhaustion diagnostic, got: {diags:?}"
        );

        assert_eq!(textures.borrow().entries.len(), 0);
        assert_eq!(textures.borrow().allocated_bytes, 0);
        assert_eq!(textures.borrow().next, u64::MAX);
    }

    #[test]
    fn forge_texture_platform_path_normalization_preserves_windows_verbatim_and_normalizes_unix() {
        let win_verbatim = r"\\?\C:\Games\EQOA\addons\resources\icon.png";
        let norm_win_verbatim = normalize_path_str(win_verbatim, true);
        assert_eq!(norm_win_verbatim, win_verbatim);

        let win_unc = r"\\server\share\addons\resources\icon.png";
        let norm_win_unc = normalize_path_str(win_unc, true);
        assert_eq!(norm_win_unc, win_unc);

        let win_std = r"C:\Games\EQOA\addons\resources\icon.png";
        let norm_win_std = normalize_path_str(win_std, true);
        assert_eq!(norm_win_std, win_std);

        let unix_legacy = r"/home/user/eqoa/addons/resources\retro_health_hearts\heart.png";
        let norm_unix = normalize_path_str(unix_legacy, false);
        assert_eq!(
            norm_unix,
            "/home/user/eqoa/addons/resources/retro_health_hearts/heart.png"
        );

        let unix_rel = r"retro_health_hearts\heart.png";
        let norm_unix_rel = normalize_path_str(unix_rel, false);
        assert_eq!(norm_unix_rel, "retro_health_hearts/heart.png");
    }

    #[test]
    fn forge_texture_post_upload_userdata_failure_rolls_back_gpu_and_budget() {
        let Some(gpu) = TestGpu::try_new() else {
            eprintln!("SKIP forge_texture: no GPU adapter");
            return;
        };
        let host = Host::new().unwrap();
        let textures = Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());

        assert_eq!(textures.borrow().entries.len(), 0);
        assert_eq!(textures.borrow().allocated_bytes, 0);

        let rgba = vec![10u8, 20, 30, 255].repeat(4);
        let bank_clone = textures.clone();

        let err = upload_and_create_userdata_with(
            &textures,
            "test_post_upload_fail",
            2,
            2,
            &rgba,
            |handle| {
                // At factory entry, upload_rgba8 has committed texture to bank
                assert_eq!(
                    bank_clone.borrow().entries.len(),
                    1,
                    "texture entry must be in bank before userdata creation"
                );
                assert_eq!(
                    bank_clone.borrow().allocated_bytes,
                    16,
                    "bytes must be accounted before userdata creation"
                );
                assert!(bank_clone.borrow().entries.get(&handle.id).unwrap().alive);

                // Attempt real allocator quota exhaustion
                host.lua.gc_collect().unwrap();
                let current_mem = host.lua.used_memory();
                host.lua.set_memory_limit(current_mem).unwrap();
                let res = host.lua.create_userdata(handle);
                host.lua.set_memory_limit(8 * 1024 * 1024).unwrap();

                match res {
                    Err(e) => {
                        eprintln!("[oracle] Real allocator memory quota failure triggered: {e:?}");
                        Err(e)
                    }
                    Ok(_) => {
                        eprintln!(
                            "[oracle] Emergency GC freed space; injected MemoryError triggered"
                        );
                        Err(mlua::Error::MemoryError("not enough memory".to_string()))
                    }
                }
            },
        )
        .unwrap_err();

        assert!(err.contains("failed to allocate Lua userdata"));

        // Post-upload rollback verification
        assert_eq!(
            textures.borrow().entries.len(),
            0,
            "post-upload userdata failure must roll back bank entries"
        );
        assert_eq!(
            textures.borrow().allocated_bytes,
            0,
            "post-upload userdata failure must roll back allocated bytes"
        );
    }
}
