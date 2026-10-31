//! Decodes game icon textures into ImGui textures for Lua addons.
//! Authoritative contracts:
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/icon.lua`
//! - Consumer: `scripts/wereoxxs_ui/wereoxxs_ui.lua`, `modules/ability_bars.lua`,
//!   `modules/casting_bar.lua`, `modules/party_frames.lua`, `modules/paint.lua`.

use super::textures::{Handle, Textures};
use mlua::{Lua, Result as LuaResult, Table, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub const MAX_CACHED_ICONS: usize = 64;
pub const MAX_CACHED_UI_TEXTURES: usize = 64;
pub const UI_TEX_COUNT: u32 = 0xFC; // 252 entries

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait IconProvider {
    fn decode_icon(&self, id: u32) -> Option<DecodedIcon>;
    fn decode_ui_texture(&self, _tex_id: u32) -> Option<DecodedIcon> {
        None
    }
    #[allow(dead_code)]
    fn era(&self) -> esf::Era {
        esf::Era::Frontiers
    }
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
    opts: TrimOptions,
) -> Option<(Vec<u8>, u32, u32)> {
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return None;
    }

    let pad_color = if opts.trim_color {
        detect_padding_color(rgba, w, h)
    } else {
        None
    };

    let is_padded = |x: u32, y: u32| -> bool {
        let o = ((y * w + x) * 4) as usize;
        let a = rgba[o + 3];
        if opts.trim_transparent && a == 0 {
            return true;
        }
        if let Some(target) = pad_color {
            if rgba[o..o + 4] == target {
                return true;
            }
        }
        false
    };

    let mut min_x = 0u32;
    let mut max_x = w;
    let mut min_y = 0u32;
    let mut max_y = h;

    while min_y < max_y {
        let mut all_padded = true;
        for x in min_x..max_x {
            if !is_padded(x, min_y) {
                all_padded = false;
                break;
            }
        }
        if all_padded {
            min_y += 1;
        } else {
            break;
        }
    }

    while max_y > min_y {
        let mut all_padded = true;
        for x in min_x..max_x {
            if !is_padded(x, max_y - 1) {
                all_padded = false;
                break;
            }
        }
        if all_padded {
            max_y -= 1;
        } else {
            break;
        }
    }

    while min_x < max_x {
        let mut all_padded = true;
        for y in min_y..max_y {
            if !is_padded(min_x, y) {
                all_padded = false;
                break;
            }
        }
        if all_padded {
            min_x += 1;
        } else {
            break;
        }
    }

    while max_x > min_x {
        let mut all_padded = true;
        for y in min_y..max_y {
            if !is_padded(max_x - 1, y) {
                all_padded = false;
                break;
            }
        }
        if all_padded {
            max_x -= 1;
        } else {
            break;
        }
    }

    let new_w = max_x.saturating_sub(min_x);
    let new_h = max_y.saturating_sub(min_y);
    if new_w == 0 || new_h == 0 {
        return Some((rgba[0..((w * h * 4) as usize)].to_vec(), w, h));
    }

    let mut out = Vec::with_capacity((new_w * new_h * 4) as usize);
    for y in min_y..max_y {
        let row_start = ((y * w + min_x) * 4) as usize;
        let row_end = row_start + (new_w * 4) as usize;
        out.extend_from_slice(&rgba[row_start..row_end]);
    }

    Some((out, new_w, new_h))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconKey {
    pub hash: u32,
    pub trim: TrimOptions,
}

#[derive(Clone)]
struct CachedTexture {
    handle_id: u64,
    width: u32,
    height: u32,
    stale: bool,
    token: Arc<()>,
}

impl CachedTexture {
    fn is_live(&self) -> bool {
        Arc::strong_count(&self.token) > 1
    }
}

pub struct IconCache {
    textures: Rc<RefCell<Textures>>,
    icons: HashMap<IconKey, CachedTexture>,
    ui_textures: HashMap<u32, CachedTexture>,
    stale_textures: Vec<CachedTexture>,
    lru_order: VecDeque<IconKey>,
    ui_lru_order: VecDeque<u32>,
    claims: HashSet<String>,
    budget_diag_logged: bool,
    ui_tex_budget_diag_logged: bool,
    ui_tex_diag_logged: bool,
    last_generation: u64,
}

impl IconCache {
    pub fn new(textures: Rc<RefCell<Textures>>) -> Self {
        Self {
            textures,
            icons: HashMap::new(),
            ui_textures: HashMap::new(),
            stale_textures: Vec::new(),
            lru_order: VecDeque::new(),
            ui_lru_order: VecDeque::new(),
            claims: HashSet::new(),
            budget_diag_logged: false,
            ui_tex_budget_diag_logged: false,
            ui_tex_diag_logged: false,
            last_generation: 0,
        }
    }

    #[allow(dead_code)]
    pub fn stale_count(&self) -> usize {
        self.stale_textures.len()
    }

    #[allow(dead_code)]
    pub fn ui_cached_count(&self) -> usize {
        self.ui_textures.len()
    }

    pub fn acquire(&mut self, owner: &str) -> bool {
        self.claims.insert(owner.to_string());
        true
    }

    pub fn release(&mut self, owner: &str) {
        self.claims.remove(owner);
        if self.claims.is_empty() {
            let mut textures = self.textures.borrow_mut();
            self.icons.retain(|_, entry| {
                if !entry.is_live() {
                    textures.mark_dead(entry.handle_id);
                    false
                } else {
                    true
                }
            });
            self.ui_textures.retain(|_, entry| {
                if !entry.is_live() {
                    textures.mark_dead(entry.handle_id);
                    false
                } else {
                    true
                }
            });
            self.lru_order.retain(|k| self.icons.contains_key(k));
            self.ui_lru_order.retain(|id| self.ui_textures.contains_key(id));
            self.stale_textures.retain(|entry| {
                if !entry.is_live() {
                    textures.mark_dead(entry.handle_id);
                    false
                } else {
                    true
                }
            });
            textures.collect(false);
        }
    }

    pub fn owner_count(&self) -> usize {
        self.claims.len()
    }

    pub fn has_claim(&self, owner: &str) -> bool {
        self.claims.contains(owner)
    }

    pub fn cached_count(&self) -> usize {
        self.icons.len() + self.ui_textures.len()
    }

    pub fn drop_cache(&mut self) {
        let mut textures = self.textures.borrow_mut();
        self.icons.retain(|_, entry| {
            if !entry.is_live() {
                textures.mark_dead(entry.handle_id);
                false
            } else {
                true
            }
        });
        self.ui_textures.retain(|_, entry| {
            if !entry.is_live() {
                textures.mark_dead(entry.handle_id);
                false
            } else {
                true
            }
        });
        self.lru_order.retain(|k| self.icons.contains_key(k));
        self.ui_lru_order.retain(|id| self.ui_textures.contains_key(id));
        self.stale_textures.retain(|entry| {
            if !entry.is_live() {
                textures.mark_dead(entry.handle_id);
                false
            } else {
                true
            }
        });
        textures.collect(false);
    }

    fn check_generation(&mut self, current_gen: u64) {
        if current_gen != self.last_generation {
            self.last_generation = current_gen;
            // F2: Mark existing entries stale without invalidating live handles.
            for entry in self.icons.values_mut() {
                entry.stale = true;
            }
            for entry in self.ui_textures.values_mut() {
                entry.stale = true;
            }
        }
    }

    fn cleanup_stale(&mut self) {
        let mut textures = self.textures.borrow_mut();
        self.stale_textures.retain(|entry| {
            if !entry.is_live() {
                textures.mark_dead(entry.handle_id);
                false
            } else {
                true
            }
        });
    }

    pub fn get_or_load(
        &mut self,
        hash: u32,
        trim: TrimOptions,
        provider: &dyn IconProvider,
    ) -> Option<(u64, u32, u32, Arc<()>)> {
        self.check_generation(provider.generation());
        self.cleanup_stale();

        let key = IconKey { hash, trim };
        if let Some(entry) = self.icons.get_mut(&key) {
            if !entry.stale {
                self.lru_order.retain(|k| k != &key);
                self.lru_order.push_back(key);
                return Some((entry.handle_id, entry.width, entry.height, entry.token.clone()));
            } else {
                let old = self.icons.remove(&key).unwrap();
                self.lru_order.retain(|k| k != &key);
                self.stale_textures.push(old);
            }
        }

        // Bounded icon cache: max 64 textures
        let current_count = self.icons.len();
        if current_count >= MAX_CACHED_ICONS {
            // Find oldest icon not held by live Lua handle
            let mut evicted_key = None;
            for candidate in &self.lru_order {
                if let Some(entry) = self.icons.get(candidate) {
                    if !entry.is_live() {
                        evicted_key = Some(*candidate);
                        break;
                    }
                }
            }
            if let Some(evict) = evicted_key {
                self.lru_order.retain(|k| k != &evict);
                if let Some(removed) = self.icons.remove(&evict) {
                    let mut textures = self.textures.borrow_mut();
                    textures.mark_dead(removed.handle_id);
                    textures.collect(false);
                }
            } else {
                // All 64 are actively held by live handles! Refuse new allocation with diagnostic.
                if !self.budget_diag_logged {
                    self.budget_diag_logged = true;
                    eprintln!(
                        "frontiers_forge.icon: icon texture budget exceeded ({}/{} live handles held)",
                        current_count, MAX_CACHED_ICONS
                    );
                }
                return None;
            }
        }

        let icon = match provider.decode_icon(hash) {
            Some(i) => i,
            None => return None,
        };
        let (cropped, tw, th) = match trim_rgba(&icon.rgba, icon.width, icon.height, trim) {
            Some(t) => t,
            None => {
                log_icon_error(hash, "trimmed empty");
                return None;
            }
        };
        if tw == 0 || th == 0 {
            log_icon_error(hash, "trimmed empty");
            return None;
        }

        let handle = self
            .textures
            .borrow_mut()
            .upload_rgba8(&self.textures, "icon", tw, th, &cropped)
            .ok()?;
        let handle_id = handle.id;
        let token = Arc::new(());

        self.icons.insert(
            key,
            CachedTexture {
                handle_id,
                width: tw,
                height: th,
                stale: false,
                token: token.clone(),
            },
        );
        self.lru_order.push_back(key);

        Some((handle_id, tw, th, token))
    }

    pub fn get_or_load_ui_texture(
        &mut self,
        tex_id: u32,
        provider: &dyn IconProvider,
    ) -> Option<(u64, u32, u32, Arc<()>)> {
        if tex_id >= UI_TEX_COUNT {
            return None;
        }

        self.check_generation(provider.generation());
        self.cleanup_stale();

        if let Some(entry) = self.ui_textures.get_mut(&tex_id) {
            if !entry.stale {
                self.ui_lru_order.retain(|&id| id != tex_id);
                self.ui_lru_order.push_back(tex_id);
                return Some((entry.handle_id, entry.width, entry.height, entry.token.clone()));
            } else {
                let old = self.ui_textures.remove(&tex_id).unwrap();
                self.stale_textures.push(old);
            }
        }

        let icon = match provider.decode_ui_texture(tex_id) {
            Some(i) => i,
            None => {
                if !self.ui_tex_diag_logged {
                    self.ui_tex_diag_logged = true;
                    eprintln!("frontiers_forge.icon: GetUITexture: HUD atlas or UI surface is unavailable");
                }
                return None;
            }
        };

        if self.ui_textures.len() >= MAX_CACHED_UI_TEXTURES {
            let mut evicted_id = None;
            for &candidate in &self.ui_lru_order {
                if let Some(entry) = self.ui_textures.get(&candidate) {
                    if !entry.is_live() {
                        evicted_id = Some(candidate);
                        break;
                    }
                }
            }
            if let Some(evict) = evicted_id {
                self.ui_lru_order.retain(|&id| id != evict);
                if let Some(removed) = self.ui_textures.remove(&evict) {
                    let mut textures = self.textures.borrow_mut();
                    textures.mark_dead(removed.handle_id);
                    textures.collect(false);
                }
            } else {
                if !self.ui_tex_budget_diag_logged {
                    self.ui_tex_budget_diag_logged = true;
                    eprintln!("frontiers_forge.icon: UI texture budget exceeded (64/64 live handles held)");
                }
                return None;
            }
        }

        let handle = self
            .textures
            .borrow_mut()
            .upload_rgba8(&self.textures, "ui_icon", icon.width, icon.height, &icon.rgba)
            .ok()?;
        let handle_id = handle.id;
        let token = Arc::new(());

        self.ui_lru_order.push_back(tex_id);
        self.ui_textures.insert(
            tex_id,
            CachedTexture {
                handle_id,
                width: icon.width,
                height: icon.height,
                stale: false,
                token: token.clone(),
            },
        );

        Some((handle_id, icon.width, icon.height, token))
    }
}

// -----------------------------------------------------------------------------
// Safe library & atlas representation (100% safe Rust, zero raw pointers)
// -----------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::AtomicU64;

#[cfg(not(target_arch = "wasm32"))]
static ATLAS_CLONE_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(not(target_arch = "wasm32"))]
static SAFE_LIBRARY_CLONE_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(not(target_arch = "wasm32"))]
static LOOKUP_ATTEMPT_COUNT: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn atlas_clone_count() -> u64 {
    ATLAS_CLONE_COUNT.load(Ordering::Relaxed)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn safe_library_clone_count() -> u64 {
    SAFE_LIBRARY_CLONE_COUNT.load(Ordering::Relaxed)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn lookup_attempt_count() -> u64 {
    LOOKUP_ATTEMPT_COUNT.load(Ordering::Relaxed)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn reset_test_counters() {
    ATLAS_CLONE_COUNT.store(0, Ordering::Relaxed);
    SAFE_LIBRARY_CLONE_COUNT.store(0, Ordering::Relaxed);
    LOOKUP_ATTEMPT_COUNT.store(0, Ordering::Relaxed);
}

#[derive(Default)]
pub struct SafeAtlas {
    pub entries: Vec<vi::ui::UiTexEntry>,
    pub textures: vi::strtab::StringTable,
}

impl Clone for SafeAtlas {
    fn clone(&self) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        ATLAS_CLONE_COUNT.fetch_add(1, Ordering::Relaxed);
        Self {
            entries: self.entries.clone(),
            textures: self.textures.clone(),
        }
    }
}

#[derive(Default)]
pub struct SafeLibrary {
    pub surfaces: HashMap<i32, Arc<DecodedIcon>>,
    pub view: esf::Era,
    pub total_bytes: usize,
}

impl Clone for SafeLibrary {
    fn clone(&self) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        SAFE_LIBRARY_CLONE_COUNT.fetch_add(1, Ordering::Relaxed);
        Self {
            surfaces: self.surfaces.clone(),
            view: self.view,
            total_bytes: self.total_bytes,
        }
    }
}

impl SafeLibrary {
    pub fn decode_surface(&self, dict_id: i32) -> Option<DecodedIcon> {
        self.surfaces.get(&dict_id).map(|d| (**d).clone())
    }

    pub fn has_surface(&self, dict_id: i32) -> bool {
        self.surfaces.contains_key(&dict_id)
    }
}

pub struct LiveLibraryProvider {
    lib: Arc<SafeLibrary>,
    atlas: Option<Arc<SafeAtlas>>,
    generation: u64,
}

impl IconProvider for LiveLibraryProvider {
    fn decode_icon(&self, id: u32) -> Option<DecodedIcon> {
        if let Some(d) = self.lib.decode_surface(id as i32) {
            Some(d)
        } else {
            queue_pending_lookup(id);
            None
        }
    }

    fn decode_ui_texture(&self, tex_id: u32) -> Option<DecodedIcon> {
        let atlas = self.atlas.as_ref()?;
        if tex_id as usize >= atlas.entries.len() {
            return None;
        }
        let entry = atlas.entries[tex_id as usize];
        let name = atlas.textures.get(entry.row, 0);
        if name.is_empty() || name == "_null_" {
            return None;
        }
        let dict_id = vi::ui::icon_id(name);
        let full = match self.lib.decode_surface(dict_id as i32) {
            Some(f) => f,
            None => {
                queue_pending_lookup(dict_id);
                return None;
            }
        };
        let src_x = entry.uv0[0];
        let src_y = entry.uv0[1];
        let w = entry.uv1[0];
        let h = entry.uv1[1];
        if src_x < 0 || src_y < 0 || w <= 0 || h <= 0 {
            return None;
        }
        let (src_x, src_y, w, h) = (src_x as u32, src_y as u32, w as u32, h as u32);
        if src_x + w > full.width || src_y + h > full.height {
            return None;
        }
        let mut cropped = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            let src_off = ((src_y + y) * full.width + src_x) as usize * 4;
            let dst_off = (y * w) as usize * 4;
            cropped[dst_off..dst_off + (w as usize * 4)]
                .copy_from_slice(&full.rgba[src_off..src_off + (w as usize * 4)]);
        }
        Some(DecodedIcon {
            width: w,
            height: h,
            rgba: cropped,
        })
    }

    fn era(&self) -> esf::Era {
        self.lib.view
    }

    fn generation(&self) -> u64 {
        self.generation
    }
}

thread_local! {
    static CUSTOM_PROVIDER: RefCell<Option<Box<dyn IconProvider>>> = const { RefCell::new(None) };
    static SAFE_LIBRARY: RefCell<Option<Arc<SafeLibrary>>> = const { RefCell::new(None) };
    static LIVE_ATLAS: RefCell<Option<Arc<SafeAtlas>>> = const { RefCell::new(None) };
    static ART_GENERATION: Cell<u64> = const { Cell::new(0) };
    static SNAPSHOT_COUNT: Cell<u64> = const { Cell::new(0) };
    static ACTIVE_CACHES: RefCell<Vec<std::rc::Weak<RefCell<IconCache>>>> = const { RefCell::new(Vec::new()) };
    static PENDING_LOOKUPS: RefCell<Option<HashSet<u32>>> = const { RefCell::new(None) };
    static FAILED_LOOKUPS: RefCell<Option<HashSet<u32>>> = const { RefCell::new(None) };
    static PREFETCH_FINGERPRINT: Cell<u64> = const { Cell::new(0) };
}

pub fn queue_pending_lookup(id: u32) {
    PENDING_LOOKUPS.with(|p| {
        let mut borrow = p.borrow_mut();
        borrow.get_or_insert_with(HashSet::new).insert(id);
    });
}

pub fn drain_pending_lookups() -> Vec<u32> {
    PENDING_LOOKUPS.with(|p| {
        p.borrow_mut().take().map_or(Vec::new(), |s| s.into_iter().collect())
    })
}

pub fn get_cached_or_collect_prefetch_ids(
    abilities: &std::collections::BTreeMap<i64, net::dump::Ability>,
    hud_menus: &crate::hudmenu::HudMenus,
    effects: &net::dump::Effects,
    inventory: &[net::dump::NetItem],
) -> Vec<u32> {
    let mut h = 0xcbf29ce484222325u64;
    for a in abilities.values() {
        h = (h ^ (a.icon_front as u64)).wrapping_mul(0x100000001b3);
        h = (h ^ (a.icon_back as u64)).wrapping_mul(0x100000001b3);
    }
    for m in &hud_menus.defs {
        for e in &m.entries {
            h = (h ^ (e.icon2 as u64)).wrapping_mul(0x100000001b3);
            h = (h ^ (e.x08 as u64)).wrapping_mul(0x100000001b3);
        }
    }
    for i in 0..(effects.count as usize).min(8) {
        h = (h ^ (effects.icon[i] as u64)).wrapping_mul(0x100000001b3);
    }
    for it in inventory {
        h = (h ^ (it.item.icon as u64)).wrapping_mul(0x100000001b3);
        h = (h ^ (it.item.icon2 as u64)).wrapping_mul(0x100000001b3);
    }

    let prev_h = PREFETCH_FINGERPRINT.with(|pf| pf.get());
    if h == prev_h && prev_h != 0 {
        return Vec::new();
    }
    PREFETCH_FINGERPRINT.with(|pf| pf.set(h));
    collect_prefetch_ids(abilities, hud_menus, effects, inventory)
}
pub fn collect_prefetch_ids(
    abilities: &std::collections::BTreeMap<i64, net::dump::Ability>,
    hud_menus: &crate::hudmenu::HudMenus,
    effects: &net::dump::Effects,
    inventory: &[net::dump::NetItem],
) -> Vec<u32> {
    let mut ids = Vec::new();
    for ability in abilities.values() {
        if ability.icon_front != 0 && ability.icon_front != -1 {
            ids.push(ability.icon_front as u32);
        }
        if ability.icon_back != 0 && ability.icon_back != -1 {
            ids.push(ability.icon_back as u32);
        }
    }
    for menu in &hud_menus.defs {
        for entry in &menu.entries {
            if entry.icon2 != -1 && entry.icon2 != 0 {
                ids.push(entry.icon2 as u32);
            }
            if entry.x08 != -1 && entry.x08 != 0 {
                ids.push(entry.x08 as u32);
            }
        }
    }
    for i in 0..(effects.count as usize).min(8) {
        let ic = effects.icon[i];
        if ic != 0 && ic != u32::MAX {
            ids.push(ic);
        }
    }
    for item in inventory {
        if item.item.icon != 0 && item.item.icon != -1 {
            ids.push(item.item.icon as u32);
        }
        if item.item.icon2 != 0 && item.item.icon2 != -1 {
            ids.push(item.item.icon2 as u32);
        }
    }
    ids
}

pub fn log_icon_error(hash: u32, reason: &str) {
    use std::sync::Mutex;
    static LOGGED: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
    let mut guard = match LOGGED.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let set = guard.get_or_insert_with(HashSet::new);
    if !set.insert(hash) {
        return;
    }
    drop(guard);

    let log_dir = crate::fork_ext::mods::mods_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("addons_error.log");
    let line = format!("[icon] 0x{:08X} ({}) {}
", hash, hash, reason);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn cleanup_active_stale() {
    ACTIVE_CACHES.with(|caches| {
        let mut list = caches.borrow_mut();
        list.retain(|weak| {
            if let Some(cache) = weak.upgrade() {
                cache.borrow_mut().cleanup_stale();
                true
            } else {
                false
            }
        });
    });
}

pub fn set_custom_icon_provider(provider: Option<Box<dyn IconProvider>>) {
    CUSTOM_PROVIDER.with(|p| *p.borrow_mut() = provider);
}

#[allow(dead_code)]
pub fn snapshot_count() -> u64 {
    SNAPSHOT_COUNT.with(|c| c.get())
}

#[allow(dead_code)]
pub fn snapshot_size_bytes() -> usize {
    SAFE_LIBRARY.with(|sl| sl.borrow().as_ref().map_or(0, |l| l.total_bytes))
}

#[allow(dead_code)]
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn clone_snapshot_arc() -> Option<Arc<SafeLibrary>> {
    SAFE_LIBRARY.with(|sl| sl.borrow().clone())
}

pub fn snapshot_has_icon(id: u32) -> bool {
    SAFE_LIBRARY.with(|sl| {
        sl.borrow()
            .as_ref()
            .map_or(false, |lib| lib.has_surface(id as i32))
    })
}

pub fn update_live_data(lib: &esf::Library, atlas: Option<Arc<SafeAtlas>>, prefetch_ids: &[u32]) {
    if let Some(a) = atlas {
        LIVE_ATLAS.with(|la| *la.borrow_mut() = Some(a));
    }

    let needs_rebuild = SAFE_LIBRARY.with(|sl| match &*sl.borrow() {
        Some(existing) => existing.view != lib.view(),
        None => true,
    });

    if needs_rebuild {
        FAILED_LOOKUPS.with(|fl| fl.borrow_mut().as_mut().map(|s| s.clear()));
        PREFETCH_FINGERPRINT.with(|pf| pf.set(0));

        let atlas_ref = LIVE_ATLAS.with(|la| la.borrow().clone());
        let mut surfaces = HashMap::new();
        let mut total_bytes = 0usize;

        // 1. Preload UI.CSF surfaces
        let is_2002 = lib.view() == esf::Era::Y2002;
        for (i, (name, _)) in lib.files.iter().enumerate() {
            let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
            if base.eq_ignore_ascii_case("UI.CSF") {
                let file_idx = if is_2002 {
                    lib.stand_in_of(i).unwrap_or(i)
                } else {
                    i
                };
                if file_idx < lib.files.len() {
                    let c = &lib.files[file_idx].1;
                    for s in &c.surfaces {
                        if let Some(img) = s.decode(&c.data) {
                            total_bytes += img.rgba.len();
                            surfaces.insert(
                                s.dict_id,
                                Arc::new(DecodedIcon {
                                    width: img.width,
                                    height: img.height,
                                    rgba: img.rgba,
                                }),
                            );
                        }
                    }
                }
            }
        }

        // 2. Preload HUD atlas entries
        if let Some(atlas) = atlas_ref.as_ref() {
            for entry in &atlas.entries {
                let name = atlas.textures.get(entry.row, 0);
                if !name.is_empty() && name != "_null_" {
                    let dict_id = vi::ui::icon_id(name) as i32;
                    if !surfaces.contains_key(&dict_id) {
                        if let Some((c, s)) = lib.surface(dict_id) {
                            if let Some(img) = s.decode(&c.data) {
                                total_bytes += img.rgba.len();
                                surfaces.insert(
                                    dict_id,
                                    Arc::new(DecodedIcon {
                                        width: img.width,
                                        height: img.height,
                                        rgba: img.rgba,
                                    }),
                                );
                            }
                        }
                    }
                }
            }
        }

        let safe_lib = Arc::new(SafeLibrary {
            surfaces,
            view: lib.view(),
            total_bytes,
        });
        SAFE_LIBRARY.with(|sl| *sl.borrow_mut() = Some(safe_lib));
        SNAPSHOT_COUNT.with(|c| c.set(c.get().wrapping_add(1)));
    }

    // 3. Process prefetch_ids and drained pending lookups
    let mut to_lookup = drain_pending_lookups();
    to_lookup.extend_from_slice(prefetch_ids);

    if !to_lookup.is_empty() {
        SAFE_LIBRARY.with(|sl| {
            let mut sl_borrow = sl.borrow_mut();
            if let Some(safe_lib) = sl_borrow.as_mut() {
                FAILED_LOOKUPS.with(|fl| {
                    let mut fl_borrow = fl.borrow_mut();
                    let failed = fl_borrow.get_or_insert_with(HashSet::new);
                    // Fast filter: only examine IDs not already cached and not previously failed
                    let needed: Vec<u32> = to_lookup
                        .into_iter()
                        .filter(|&id| {
                            !safe_lib.surfaces.contains_key(&(id as i32)) && !failed.contains(&id)
                        })
                        .collect();

                    if needed.is_empty() {
                        return;
                    }

                    // Look up surfaces first WITHOUT calling make_mut
                    let mut newly_decoded = Vec::new();
                    for id in needed {
                        #[cfg(not(target_arch = "wasm32"))]
                        LOOKUP_ATTEMPT_COUNT.fetch_add(1, Ordering::Relaxed);
                        let dict_id = id as i32;
                        if let Some((c, s)) = lib.surface(dict_id) {
                            if let Some(img) = s.decode(&c.data) {
                                newly_decoded.push((
                                    dict_id,
                                    Arc::new(DecodedIcon {
                                        width: img.width,
                                        height: img.height,
                                        rgba: img.rgba,
                                    }),
                                ));
                            } else {
                                failed.insert(id);
                                log_icon_error(id, "surface decode failed");
                            }
                        } else {
                            failed.insert(id);
                            log_icon_error(id, "not found in library or snapshot");
                        }
                    }

                    // ONLY call make_mut if at least one id is actually new and resolvable!
                    if !newly_decoded.is_empty() {
                        let lib_mut = Arc::make_mut(safe_lib);
                        for (dict_id, icon) in newly_decoded {
                            lib_mut.total_bytes += icon.rgba.len();
                            lib_mut.surfaces.insert(dict_id, icon);
                        }
                    }
                });
            }
        });
    }
}

pub fn notify_art_changed() {
    ART_GENERATION.with(|g| g.set(g.get().wrapping_add(1)));
}

pub fn with_active_provider<R, F: FnOnce(&dyn IconProvider) -> R>(f: F) -> R {
    CUSTOM_PROVIDER.with(|custom| {
        if let Some(ref p) = *custom.borrow() {
            return f(p.as_ref());
        }
        let safe_lib = SAFE_LIBRARY.with(|sl| sl.borrow().clone());
        if let Some(lib) = safe_lib {
            let atlas = LIVE_ATLAS.with(|la| la.borrow().clone());
            let generation = ART_GENERATION.with(|g| g.get());
            let live = LiveLibraryProvider {
                lib,
                atlas,
                generation,
            };
            return f(&live);
        }
        struct EmptyProvider;
        impl IconProvider for EmptyProvider {
            fn decode_icon(&self, _id: u32) -> Option<DecodedIcon> {
                None
            }
        }
        f(&EmptyProvider)
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
                eprintln!("frontiers_forge.icon: GetUITexture is not implemented in base adapter");
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
    ACTIVE_CACHES.with(|caches| caches.borrow_mut().push(Rc::downgrade(&cache)));

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

    // GetSurfaceInfo
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
                    info.set("format", 2)?;
                    info.set("palette_count", 0)?;
                    info.set("palette_format", 0)?;
                    Ok(Value::Table(info))
                } else {
                    Ok(Value::Nil)
                }
            })
        })?,
    )?;

    // GetTexture
    {
        let cache_ref = cache.clone();
        let textures_ref = textures.clone();
        icon_mod.set(
            "GetTexture",
            lua.create_function(move |lua, (hash, opts): (Value, Option<Table>)| {
                let hash_u32 = match hash {
                    Value::Integer(i) if i > 0 => i as u32,
                    Value::Number(n) if n > 0.0 => n as u32,
                    _ => return Ok((Value::Nil, Value::Nil, Value::Nil)),
                };
                let trim = TrimOptions::from_lua_table(opts.as_ref());
                let result = with_active_provider(|provider| {
                    cache_ref.borrow_mut().get_or_load(hash_u32, trim, provider)
                });

                if let Some((handle_id, tw, th, token)) = result {
                    let handle = Handle {
                        owner: textures_ref.clone(),
                        id: handle_id,
                        token: Some(token),
                    };
                    let ud = lua.create_userdata(handle)?;
                    Ok((
                        Value::UserData(ud),
                        Value::Integer(tw as i64),
                        Value::Integer(th as i64),
                    ))
                } else {
                    Ok((Value::Nil, Value::Nil, Value::Nil))
                }
            })?,
        )?;
    }

    // GetUITexture
    {
        let cache_ref = cache.clone();
        let textures_ref = textures;
        icon_mod.set(
            "GetUITexture",
            lua.create_function(move |lua, tex_id: Value| {
                let tex_id_u32 = match tex_id {
                    Value::Integer(i) if i >= 0 && i < UI_TEX_COUNT as i64 => i as u32,
                    Value::Number(n) if n >= 0.0 && n < UI_TEX_COUNT as f64 => n as u32,
                    _ => return Ok((Value::Nil, Value::Nil, Value::Nil)),
                };

                let result = with_active_provider(|provider| {
                    cache_ref
                        .borrow_mut()
                        .get_or_load_ui_texture(tex_id_u32, provider)
                });

                if let Some((handle_id, tw, th, token)) = result {
                    let handle = Handle {
                        owner: textures_ref.clone(),
                        id: handle_id,
                        token: Some(token),
                    };
                    let ud = lua.create_userdata(handle)?;
                    Ok((
                        Value::UserData(ud),
                        Value::Integer(tw as i64),
                        Value::Integer(th as i64),
                    ))
                } else {
                    Ok((Value::Nil, Value::Nil, Value::Nil))
                }
            })?,
        )?;
    }

    Ok(cache)
}
