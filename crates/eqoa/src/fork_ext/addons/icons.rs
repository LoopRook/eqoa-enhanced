//! Decodes game icon textures into ImGui textures for Lua addons.
//! Authoritative contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/icon.lua`
//! - Consumer: `scripts/wereoxxs_ui/wereoxxs_ui.lua`, `modules/ability_bars.lua`,
//!   `modules/casting_bar.lua`, `modules/party_frames.lua`, `modules/paint.lua`.

use super::textures::{Handle, Textures};
use mlua::{Lua, Result as LuaResult, Table, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

pub const MAX_CACHED_ICONS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait IconProvider {
    fn decode_icon(&self, id: u32) -> Option<DecodedIcon>;
    #[allow(dead_code)]
    fn era(&self) -> esf::Era {
        esf::Era::Frontiers
    }
    #[allow(dead_code)]
    fn generation(&self) -> u64 {
        0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrimOptions {
    pub trim_transparent: bool,
    pub trim_color: bool,
}

impl TrimOptions {
    pub fn from_lua_table(table: Option<&Table>) -> Self {
        match table {
            Some(t) => {
                let trim_transparent = t.get::<bool>("trim_transparent").unwrap_or(false)
                    || t.get::<bool>("trim").unwrap_or(false);
                let trim_color = t.get::<bool>("trim_color").unwrap_or(false);
                Self {
                    trim_transparent,
                    trim_color,
                }
            }
            None => Self {
                trim_transparent: false,
                trim_color: false,
            },
        }
    }
}

pub fn detect_padding_color(rgba: &[u8], w: u32, h: u32) -> Option<[u8; 4]> {
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let mut counts: HashMap<[u8; 4], usize> = HashMap::new();
    let mut ring_total = 0usize;

    let mut tally = |x: u32, y: u32| {
        let o = ((y * w + x) * 4) as usize;
        if o + 4 <= rgba.len() {
            let a = rgba[o + 3];
            if a != 0 {
                let color = [rgba[o], rgba[o + 1], rgba[o + 2], a];
                *counts.entry(color).or_insert(0) += 1;
                ring_total += 1;
            }
        }
    };

    for x in 0..w {
        tally(x, 0);
        if h > 1 {
            tally(x, h - 1);
        }
    }
    for y in 1..h.saturating_sub(1) {
        tally(0, y);
        if w > 1 {
            tally(w - 1, y);
        }
    }

    if ring_total == 0 {
        return None;
    }

    let mut best_color = None;
    let mut best_count = 0;
    for (color, count) in counts {
        if count > best_count {
            best_count = count;
            best_color = Some(color);
        }
    }

    if best_count * 2 >= ring_total {
        best_color
    } else {
        None
    }
}

pub fn trim_rgba(
    rgba: &[u8],
    w: u32,
    h: u32,
    trim: TrimOptions,
) -> Option<(Vec<u8>, u32, u32)> {
    if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
        return None;
    }
    if !trim.trim_transparent && !trim.trim_color {
        return Some((rgba.to_vec(), w, h));
    }

    let pad_color = if trim.trim_color {
        detect_padding_color(rgba, w, h)
    } else {
        None
    };

    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0i64 - 1;
    let mut max_y = 0i64 - 1;

    for y in 0..h {
        let row = (y * w * 4) as usize;
        for x in 0..w {
            let o = row + (x * 4) as usize;
            let a = rgba[o + 3];
            let mut is_padding = false;
            if trim.trim_transparent && a == 0 {
                is_padding = true;
            } else if let Some(pad) = pad_color {
                if rgba[o..o + 4] == pad {
                    is_padding = true;
                }
            }
            if !is_padding {
                if x < min_x {
                    min_x = x;
                }
                if (x as i64) > max_x {
                    max_x = x as i64;
                }
                if y < min_y {
                    min_y = y;
                }
                if (y as i64) > max_y {
                    max_y = y as i64;
                }
            }
        }
    }

    if max_x < 0 {
        // Every pixel was padding so there is nothing to draw.
        return None;
    }

    if min_x == 0 && min_y == 0 && max_x == (w as i64 - 1) && max_y == (h as i64 - 1) {
        return Some((rgba.to_vec(), w, h));
    }

    let tw = (max_x - min_x as i64 + 1) as u32;
    let th = (max_y - min_y as i64 + 1) as u32;
    let mut cropped = vec![0u8; (tw * th * 4) as usize];
    for y in 0..th {
        let src_row = (((min_y + y) * w + min_x) * 4) as usize;
        let dst_row = (y * tw * 4) as usize;
        let len = (tw * 4) as usize;
        cropped[dst_row..dst_row + len].copy_from_slice(&rgba[src_row..src_row + len]);
    }

    Some((cropped, tw, th))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconKey {
    pub hash: u32,
    pub trim: TrimOptions,
}

#[derive(Clone, Debug)]
pub struct IconEntry {
    pub handle_id: u64,
    pub width: u32,
    pub height: u32,
    pub last_used: u64,
}

pub struct IconCache {
    textures: Rc<RefCell<Textures>>,
    cache: HashMap<IconKey, IconEntry>,
    claims: HashSet<String>,
    use_counter: u64,
    last_generation: u64,
}

impl IconCache {
    pub fn new(textures: Rc<RefCell<Textures>>) -> Self {
        Self {
            textures,
            cache: HashMap::new(),
            claims: HashSet::new(),
            use_counter: 0,
            last_generation: 0,
        }
    }

    pub fn acquire(&mut self, owner: &str) -> bool {
        self.claims.insert(owner.to_string());
        true
    }

    pub fn release(&mut self, owner: &str) {
        self.claims.remove(owner);
        if self.claims.is_empty() {
            self.drop_cache();
        }
    }

    pub fn owner_count(&self) -> usize {
        self.claims.len()
    }

    pub fn has_claim(&self, owner: &str) -> bool {
        self.claims.contains(owner)
    }

    pub fn cached_count(&self) -> usize {
        self.cache.len()
    }

    pub fn drop_cache(&mut self) {
        let mut textures = self.textures.borrow_mut();
        for entry in self.cache.values() {
            textures.mark_dead(entry.handle_id);
        }
        textures.collect(false);
        self.cache.clear();
    }

    pub fn get_or_load(
        &mut self,
        hash: u32,
        trim: TrimOptions,
        provider: &dyn IconProvider,
    ) -> Option<(u64, u32, u32)> {
        let current_gen = provider.generation();
        if current_gen != self.last_generation {
            self.drop_cache();
            self.last_generation = current_gen;
        }

        let key = IconKey { hash, trim };
        if let Some(entry) = self.cache.get_mut(&key) {
            self.use_counter = self.use_counter.wrapping_add(1);
            entry.last_used = self.use_counter;
            return Some((entry.handle_id, entry.width, entry.height));
        }

        let icon = provider.decode_icon(hash)?;
        let (cropped, tw, th) = trim_rgba(&icon.rgba, icon.width, icon.height, trim)?;

        // Enforce bounded cache (LRU eviction)
        if self.cache.len() >= MAX_CACHED_ICONS {
            if let Some((&lru_key, _)) = self
                .cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
            {
                if let Some(removed) = self.cache.remove(&lru_key) {
                    let mut textures = self.textures.borrow_mut();
                    textures.mark_dead(removed.handle_id);
                    textures.collect(false);
                }
            }
        }

        let handle = self
            .textures
            .borrow_mut()
            .upload_rgba8(&self.textures, "icon", tw, th, &cropped)
            .ok()?;
        let handle_id = handle.id;

        self.use_counter = self.use_counter.wrapping_add(1);
        self.cache.insert(
            key,
            IconEntry {
                handle_id,
                width: tw,
                height: th,
                last_used: self.use_counter,
            },
        );

        Some((handle_id, tw, th))
    }
}

thread_local! {
    static CUSTOM_PROVIDER: RefCell<Option<Box<dyn IconProvider>>> = const { RefCell::new(None) };
    static LIVE_LIBRARY: Cell<Option<*const esf::Library>> = const { Cell::new(None) };
    static ART_GENERATION: Cell<u64> = const { Cell::new(0) };
}

pub fn set_custom_icon_provider(provider: Option<Box<dyn IconProvider>>) {
    CUSTOM_PROVIDER.with(|p| *p.borrow_mut() = provider);
}

pub fn set_live_library(lib: *const esf::Library) {
    LIVE_LIBRARY.with(|l| l.set(Some(lib)));
}

pub fn notify_art_changed() {
    ART_GENERATION.with(|g| g.set(g.get().wrapping_add(1)));
}

pub fn with_active_provider<R, F: FnOnce(&dyn IconProvider) -> R>(f: F) -> R {
    CUSTOM_PROVIDER.with(|custom| {
        if let Some(ref p) = *custom.borrow() {
            return f(p.as_ref());
        }
        LIVE_LIBRARY.with(|lib_ptr| {
            if let Some(ptr) = lib_ptr.get() {
                let lib = unsafe { &*ptr };
                let art_gen = ART_GENERATION.with(|g| g.get());
                struct LiveProvider<'a>(&'a esf::Library, u64);
                impl<'a> IconProvider for LiveProvider<'a> {
                    fn decode_icon(&self, id: u32) -> Option<DecodedIcon> {
                        let (c, s) = self.0.surface(id as i32)?;
                        let img = s.decode(&c.data)?;
                        Some(DecodedIcon {
                            width: img.width,
                            height: img.height,
                            rgba: img.rgba,
                        })
                    }
                    fn era(&self) -> esf::Era {
                        self.0.view()
                    }
                    fn generation(&self) -> u64 {
                        self.1
                    }
                }
                f(&LiveProvider(lib, art_gen))
            } else {
                struct NullProvider;
                impl IconProvider for NullProvider {
                    fn decode_icon(&self, _: u32) -> Option<DecodedIcon> {
                        None
                    }
                }
                f(&NullProvider)
            }
        })
    })
}

static UI_TEX_DIAG_LOGGED: AtomicBool = AtomicBool::new(false);

pub fn install_base_icon_adapter(lua: &Lua, host: &super::host::Host) -> LuaResult<()> {
    let icon_mod = match lua.named_registry_value::<Table>("__forge_icon_mod") {
        Ok(t) => t,
        Err(_) => {
            let t = lua.create_table()?;
            lua.set_named_registry_value("__forge_icon_mod", t.clone())?;
            t
        }
    };
    let icon_claims = Rc::new(RefCell::new(HashSet::<String>::new()));

    {
        let claims = icon_claims.clone();
        icon_mod.set(
            "Acquire",
            lua.create_function(move |_, owner: String| {
                claims.borrow_mut().insert(owner);
                Ok(true)
            })?,
        )?;
    }

    {
        let claims = icon_claims.clone();
        icon_mod.set(
            "Release",
            lua.create_function(move |_, owner: String| {
                claims.borrow_mut().remove(&owner);
                Ok(())
            })?,
        )?;
    }

    {
        let claims = icon_claims.clone();
        icon_mod.set(
            "GetOwnerCount",
            lua.create_function(move |_, ()| Ok(claims.borrow().len()))?,
        )?;
    }

    {
        let claims = icon_claims.clone();
        icon_mod.set(
            "HasClaim",
            lua.create_function(move |_, owner: String| Ok(claims.borrow().contains(&owner)))?,
        )?;
    }

    icon_mod.set(
        "GetTexture",
        lua.create_function(|_, (_hash, _opts): (Value, Option<Table>)| {
            Ok((Value::Nil, Value::Nil, Value::Nil))
        })?,
    )?;

    icon_mod.set(
        "GetUITexture",
        lua.create_function(|_, _tex_id: Value| {
            if !UI_TEX_DIAG_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!("frontiers_forge.icon: GetUITexture is not implemented");
            }
            Ok((Value::Nil, Value::Nil, Value::Nil))
        })?,
    )?;

    icon_mod.set(
        "GetSurfaceInfo",
        lua.create_function(|lua, hash: Value| {
            let hash_u32 = match hash {
                Value::Integer(i) if i > 0 => i as u32,
                Value::Number(n) if n > 0.0 => n as u32,
                _ => return Ok(Value::Nil),
            };
            with_active_provider(|provider| {
                if let Some(icon) = provider.decode_icon(hash_u32) {
                    let info = lua.create_table()?;
                    info.set("width", icon.width)?;
                    info.set("height", icon.height)?;
                    info.set("format", 2)?; // RGBA32
                    info.set("palette_count", 0)?;
                    info.set("palette_format", 0)?;
                    Ok(Value::Table(info))
                } else {
                    Ok(Value::Nil)
                }
            })
        })?,
    )?;

    host.register_host_module("frontiers_forge.icon", Value::Table(icon_mod.clone()))?;
    if let Ok(pkg) = lua.globals().get::<Table>("package") {
        if let Ok(loaded) = pkg.get::<Table>("loaded") {
            let _ = loaded.set("frontiers_forge.icon", icon_mod);
        }
    }
    Ok(())
}

pub fn attach_icon_cache(
    lua: &Lua,
    textures: Rc<RefCell<Textures>>,
) -> LuaResult<Rc<RefCell<IconCache>>> {
    let cache = Rc::new(RefCell::new(IconCache::new(textures.clone())));

    let icon_mod: Table = match lua.named_registry_value::<Table>("__forge_icon_mod") {
        Ok(t) => t,
        Err(_) => {
            let t = lua.create_table()?;
            lua.set_named_registry_value("__forge_icon_mod", t.clone())?;
            t
        }
    };
    if let Ok(pkg) = lua.globals().get::<Table>("package") {
        if let Ok(loaded) = pkg.get::<Table>("loaded") {
            let _ = loaded.set("frontiers_forge.icon", icon_mod.clone());
        }
    }

    // Acquire
    {
        let cache_ref = cache.clone();
        icon_mod.set(
            "Acquire",
            lua.create_function(move |_, owner: String| {
                Ok(cache_ref.borrow_mut().acquire(&owner))
            })?,
        )?;
    }

    // Release
    {
        let cache_ref = cache.clone();
        icon_mod.set(
            "Release",
            lua.create_function(move |_, owner: String| {
                cache_ref.borrow_mut().release(&owner);
                Ok(())
            })?,
        )?;
    }

    // GetOwnerCount
    {
        let cache_ref = cache.clone();
        icon_mod.set(
            "GetOwnerCount",
            lua.create_function(move |_, ()| Ok(cache_ref.borrow().owner_count()))?,
        )?;
    }

    // HasClaim
    {
        let cache_ref = cache.clone();
        icon_mod.set(
            "HasClaim",
            lua.create_function(move |_, owner: String| {
                Ok(cache_ref.borrow().has_claim(&owner))
            })?,
        )?;
    }

    // GetTexture
    {
        let cache_ref = cache.clone();
        let textures_ref = textures.clone();
        icon_mod.set(
            "GetTexture",
            lua.create_function(move |lua, (hash_val, opts): (Value, Option<Table>)| {
                let hash = match hash_val {
                    Value::Integer(i) if i > 0 && i != -1 => i as u32,
                    Value::Number(n) if n > 0.0 && n < 4294967295.0 => n as u32,
                    _ => return Ok((Value::Nil, Value::Nil, Value::Nil)),
                };
                if hash == 0 || hash == 0xFFFFFFFF {
                    return Ok((Value::Nil, Value::Nil, Value::Nil));
                }

                let trim = TrimOptions::from_lua_table(opts.as_ref());

                let result = with_active_provider(|provider| {
                    cache_ref.borrow_mut().get_or_load(hash, trim, provider)
                });

                if let Some((handle_id, tw, th)) = result {
                    let handle = Handle {
                        owner: textures_ref.clone(),
                        id: handle_id,
                    };
                    let ud = lua.create_userdata(handle)?;
                    Ok((Value::UserData(ud), Value::Integer(tw as i64), Value::Integer(th as i64)))
                } else {
                    Ok((Value::Nil, Value::Nil, Value::Nil))
                }
            })?,
        )?;
    }

    // GetUITexture
    icon_mod.set(
        "GetUITexture",
        lua.create_function(|_, _tex_id: Value| {
            if !UI_TEX_DIAG_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!("frontiers_forge.icon: GetUITexture is not implemented");
            }
            Ok((Value::Nil, Value::Nil, Value::Nil))
        })?,
    )?;

    Ok(cache)
}
