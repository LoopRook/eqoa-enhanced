//! Native map adapter. Private cache access stays within Map's module.
use super::*;
use serde_json::{Value as Json, json};
impl Map {
    /// Whether this map has read the window's pins from the settings yet (a fresh map after a login has not,
    /// until its first frame in a world): before that, an absent pin says nothing.
    pub(crate) fn api_waypoints_loaded(&self) -> bool {
        self.waypoints_read
    }

    /// The window's pins read from the settings now, as the map's own first use does.
    pub(crate) fn api_load_waypoints(&mut self) {
        self.read_waypoints();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn api_validate_waypoint(
        &self,
        world: i32,
        current_world: i32,
        [x, z]: [f32; 2],
    ) -> Result<(), (&'static str, &'static str)> {
        if !x.is_finite() || !z.is_finite() {
            return Err(("invalid_argument", "waypoint coordinates must be finite"));
        }
        let cached = [&self.world, &self.home]
            .into_iter()
            .flatten()
            .find(|map| map.index == world);
        let Some(map) = cached else {
            if world == current_world
                || usize::try_from(world).is_ok_and(|index| index < self.names.names.len())
            {
                return Err(("not_ready", "the addressed world map is not loaded"));
            }
            return Err(("invalid_argument", "unknown waypoint world"));
        };
        let cells = &map.builder.cells;
        if cells.len() == 0 {
            return Err(("not_ready", "the addressed world map has no ground extent"));
        }
        let [xmin, zmin, xmax, zmax] = cells.extent();
        if ee::le(xmin, x) && ee::le(x, xmax) && ee::le(zmin, z) && ee::le(z, zmax) {
            Ok(())
        } else {
            Err(("invalid_argument", "waypoint is outside the world's ground extent"))
        }
    }

    /// Small allocation-free source key; dynamic records are rebuilt only on change.
    pub(crate) fn api_key(&self, world: i32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        world.hash(&mut h);
        self.open.hash(&mut h);
        crate::settings::map().hash(&mut h);
        crate::settings::map_markers().hash(&mut h);
        self.mini.up.hash(&mut h);
        for w in [&self.world, &self.home].into_iter().flatten() {
            w.index.hash(&mut h);
            w.name.hash(&mut h);
            let c = &w.builder.cells;
            (c.col0, c.row0, c.cols, c.rows).hash(&mut h);
        }
        for m in &self.quests.marks {
            (m.world, m.x.to_bits(), m.z.to_bits(), &m.npc, m.kind).hash(&mut h);
            m.quests.first().map(|q| q.levels).hash(&mut h);
        }
        for m in &self.community.marks {
            (m.world, m.at.map(f32::to_bits), &m.name, &m.note).hash(&mut h);
        }
        for (world, p) in &self.waypoints {
            (world, p.map(f32::to_bits)).hash(&mut h);
        }
        if let Some(m) = self.group_mark() {
            (m.world, m.at.map(f32::to_bits), &m.from).hash(&mut h);
        }
        crate::fork_ext::map_toolbar::anchor().map(|a| a.map(f32::to_bits)).hash(&mut h);
        #[cfg(not(target_arch = "wasm32"))]
        crate::fork_ext::addons::map_waypoint::hash_owner(&mut h);
        h.finish()
    }

    /// `world`'s waypoint put at `at` (none: taken away) through the map's own path, as a map click does.
    pub(crate) fn api_set_waypoint(&mut self, world: i32, at: Option<[f32; 2]>) {
        self.set_waypoint(world, at);
    }

    pub(crate) fn api_state(
        &self,
        world: i32,
        zone: &Json,
        words: Option<&vi::strtab::StringTable>,
    ) -> Json {
        let cached = self
            .world
            .as_ref()
            .filter(|w| w.index == world)
            .or_else(|| self.home.as_ref().filter(|w| w.index == world));
        let world_name = self
            .world_title(world, words)
            .or_else(|| cached.map(|w| title_case(w.name.split('.').next().unwrap_or(""))));
        let bounds = cached.map(|w| {
            let c = &w.builder.cells;
            json!({"x_min":ee::mul(c.col0 as f32,CELL),"z_min":ee::mul(c.row0 as f32,CELL),"x_max":ee::mul((c.col0+c.cols) as f32,CELL),"z_max":ee::mul((c.row0+c.rows) as f32,CELL)})
        });
        json!({"search_anchor":crate::fork_ext::map_toolbar::anchor().map(|a|json!({"x":a[0],"y":a[1],"width":a[2],"height":a[3]})),"enabled":crate::settings::map(),"open":self.open,"minimap":self.mini.up,"markers_on":crate::settings::map_markers(),"world_id":world,"world_name":world_name,"zone":zone,"bounds":bounds,"waypoint":self.waypoint(world).map(|p|{let (owner,label)=waypoint_owner(world,p);json!({"world_id":world,"x":p[0],"z":p[1],"owner":owner,"label":label})})})
    }
}

/// The mod that put `world`'s waypoint at `at`, and its label; none for the player's own pin.
fn waypoint_owner(world: i32, at: [f32; 2]) -> (Option<String>, Option<String>) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((owner, label)) = crate::fork_ext::addons::map_waypoint::owner(world, Some(at)) {
        return (Some(owner), label);
    }
    let _ = (world, at);
    (None, None)
}

/// Static catalogue uses exactly the map's pinned marker arrays, built once per world.
pub(crate) fn static_markers(world: i32) -> std::rc::Rc<Vec<Json>> {
    thread_local! { static CACHE: std::cell::RefCell<BTreeMap<i32,std::rc::Rc<Vec<Json>>>> = const { std::cell::RefCell::new(BTreeMap::new()) }; }
    CACHE.with(|c| c.borrow_mut().entry(world).or_insert_with(|| {
        let mut out = Vec::new();
        for m in markers::of_world(world) {
            let at = m.at();
            let (kind,name,lo,hi) = match m {
                markers::Marker::Camp(i) => { let c = &markers_data::CAMPS[i]; ("camp",c.name,Some(c.levels.0),Some(c.levels.1)) },
                markers::Marker::Coach(i) => ("coach",markers_data::COACHES[i].coachman,None,None),
            };
            out.push(json!({"kind":kind,"name":name,"x":at[0],"z":at[1],"level_min":lo,"level_max":hi,"label":match m { markers::Marker::Camp(_) => format!("{name} (XP camp)"), markers::Marker::Coach(i) => format!("{} - {name} (coach)",markers_data::COACHES[i].town) }}));
        }
        for (_,t) in markers::towns_of(world) { out.push(json!({"kind":"town","name":t.name,"x":t.at[0],"z":t.at[1],"label":null})); }
        std::rc::Rc::new(out)
    }).clone())
}
impl Map {
    pub(crate) fn api_markers(&self) -> Vec<Json> {
        let mut out = Vec::new();
        for m in &self.quests.marks {
            let levels = m.quests.first().map(|q| q.levels);
            out.push(json!({"world_id":m.world,"kind":"quest","step":m.kind != 1,"name":m.npc,"x":m.x,"z":m.z,"level_min":levels.map(|x|x.0),"level_max":levels.map(|x|x.1),"label":null}));
        }
        for m in &self.community.marks {
            out.push(json!({"world_id":m.world,"kind":"community","name":m.name,"x":m.at[0],"z":m.at[1],"label":m.note}));
        }
        for (&world, &p) in &self.waypoints {
            out.push(json!({"world_id":world,"kind":"waypoint","name":"Waypoint","x":p[0],"z":p[1],"label":null}));
        }
        if let Some(m) = self.group_mark() {
            out.push(json!({"world_id":m.world,"kind":"group_waypoint","name":m.from,"x":m.at[0],"z":m.at[1],"label":null}));
        }
        out
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    #[test]
    fn map_state_keeps_player_world_calibration_while_browsing_places() {
        let mut map = testing::up((640, 448));
        let before = map.api_state(0, &Json::Null, None);
        // Native Places transfers the player's world into home while showing another.
        map.home = map.world.take();
        let after = map.api_state(0, &Json::Null, None);
        assert_eq!(before["bounds"], after["bounds"]);
        assert_eq!(before["world_name"], after["world_name"]);
        assert!(
            map.api_state(1, &Json::Null, None)["bounds"].is_null(),
            "other world is not fabricated"
        );
    }
}

impl Map {
    pub(crate) fn api_minimap_transform(
        &self,
        center: [f32; 2],
        window: (u32, u32),
    ) -> Option<crate::fork_ext::addons::map_views::Projection> {
        if !self.mini.up || self.open {
            return None;
        }
        let moved = crate::hudlayout::shift(
            crate::hudlayout::Piece::Minimap,
            crate::hudlayout::Beyond::of(window),
        );
        let outer = mini::window_rect_moved(window, mini::FRAME, moved);
        let inner = mini::window_rect_moved(window, mini::inner(), moved);
        let k = (inner[2] - inner[0]) / (mini::inner()[2] - mini::inner()[0]) as f32;
        let scale = if self.indoor.inside() {
            self.indoor.scale()
        } else {
            self.mini.scale
        } / k;
        Some(crate::fork_ext::addons::map_views::Projection {
            center,
            scale,
            outer,
            inner,
            window,
        })
    }
}


/// The disc's own HUD art (`$EQOA_ISO`): UI.CSF's surfaces and SUPPORT's atlas, as `attach_ui`
/// builds the real HUD from them. None (a SKIP the caller reports) without the disc.
#[cfg(test)]
pub(crate) struct RealUi {
    pub lib: esf::Library,
    pub vui: crate::ui::Vui,
}
#[cfg(test)]
thread_local! {static REAL_UI:std::cell::RefCell<Option<std::rc::Rc<RealUi>>>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
pub(crate) fn real_ui() -> Option<std::rc::Rc<RealUi>> {
    REAL_UI.with(|r| r.borrow().clone())
}
#[cfg(test)]
pub(crate) fn use_real_ui(on: bool) -> bool {
    if !on {
        REAL_UI.with(|r| *r.borrow_mut() = None);
        return true;
    }
    if real_ui().is_some() {
        return true;
    }
    let Some(path) = std::env::var_os("EQOA_ISO") else {
        eprintln!("SKIP real HUD art: EQOA_ISO is not set");
        return false;
    };
    let built = (|| {
        let mut disc = iso::Iso::open(std::path::Path::new(&path)).ok()?;
        let mut lib = esf::Library::new();
        lib.add("UI.CSF", esf::Container::open(disc.read("UI.CSF").ok()?).ok()?);
        let textures = vi::strtab::StringTable::parse(&disc.read("TEXTURES.TXT").ok()?)?;
        let support = crate::modules::load_now(crate::modules::Module::Support, std::path::Path::new(&path))?;
        let atlas = vi::ui::read_atlas(&support)?;
        Some(RealUi { lib, vui: crate::ui::Vui::new(Default::default(), textures, atlas) })
    })();
    match built {
        Some(r) => {
            REAL_UI.with(|c| *c.borrow_mut() = Some(std::rc::Rc::new(r)));
            true
        }
        None => {
            eprintln!("SKIP real HUD art: the disc's UI.CSF, TEXTURES.TXT or SUPPORT did not load");
            false
        }
    }
}

#[cfg(test)]
impl Map {
    pub(crate) fn round7c_add_calibrated_world(&mut self, index: i32) {
        let mut world = crate::map::testing::up(self.size).world.expect("calibrated fixture");
        world.index = index;
        self.home = Some(world);
    }

    /// Disc-free native chrome/round-minimap scene used behind addon captures.
    /// No terrain art is fabricated; tiles remain absent without a disc.
    pub(crate) fn round4_fixture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
        mini: bool,
    ) -> Self {
        let mut map = crate::map::testing::up(size);
        map.open = !mini;
        let real = real_ui();
        let blank = esf::Library::new();
        let vui_blank = crate::pointer::testing::vui();
        let (lib, vui) = real.as_ref().map_or((&blank, &vui_blank), |r| (&r.lib, &r.vui));
        let cx = Context {
            lib,
            device,
            queue,
            format: wgpu::TextureFormat::Rgba8Unorm,
            size,
            dt: 0.,
            player: Some(([25300., 0., 15200.], 0.)),
            world: 0,
            origin: [0.; 2],
            group: Vec::new(),
            in_group: false,
            target: None,
            sticks: [0.; 4],
            keyboard: true,
            words: None,
            vui: Some(vui),
            quest_marks: None,
        };
        map.layers(&cx);
        if mini {
            map.mini_frame(&cx,&mini::MiniView::default());
            // Terrain stand-in: the minimap's whole frame green, as the picture would be.
            let r=mini::window_rect(size,mini::FRAME);
            let l=map.layers.as_mut().unwrap();
            let white=l.white;
            l.map.quads.push(render::map::MapQuad::rect(white,r,[0.0,0.0,1.0,1.0],[0.1,0.6,0.2,1.0]));
        } else {map.compose(&cx);}
        map
    }

    pub(crate) fn round4_waypoint(&mut self,world:i32,at:Option<[f32;2]>){self.set_waypoint(world,at);}

    /// The pins read from the settings, as the map's first frame in a world does.
    pub(crate) fn round7c_read_waypoints(&mut self){self.api_load_waypoints();}

    /// A minimap frame as `Play::map_frame` makes it each frame (a moved or toggled minimap
    /// is laid out again), centred on `at` with `view`'s dots and marks, over a terrain
    /// stand-in of `color` (a picture without a disc).
    pub(crate) fn round4_mini_frame(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,size:(u32,u32),at:[f32;3],view:&mini::MiniView,color:[f32;4]){
        let real=real_ui();let blank=esf::Library::new();let vui_blank=crate::pointer::testing::vui();
        let (lib,vui)=real.as_ref().map_or((&blank,&vui_blank),|r|(&r.lib,&r.vui));
        let cx=Context{lib,device,queue,format:wgpu::TextureFormat::Rgba8Unorm,size,dt:0.,player:Some((at,0.)),world:0,origin:[0.;2],group:Vec::new(),in_group:false,target:None,sticks:[0.;4],keyboard:true,words:None,vui:Some(vui),quest_marks:None};
        self.mini_frame(&cx,view);
        let moved=crate::hudlayout::shift(crate::hudlayout::Piece::Minimap,crate::hudlayout::Beyond::of(size));
        let r=mini::window_rect_moved(size,mini::FRAME,moved);
        let l=self.layers.as_mut().unwrap();
        let white=l.white;
        l.map.quads.push(render::map::MapQuad::rect(white,r,[0.0,0.0,1.0,1.0],color));
    }

    pub(crate) fn round4_group_waypoint(&mut self,world:i32,at:[f32;2]){
        self.set_group_waypoint(group_waypoint::GroupMark{world,at,from:"Mate".into()});
    }
}

#[cfg(test)]
#[path = "map_render_tests.rs"]
mod render_tests;

impl Map {
    /// The player's own world's overview (not the world a Places browse shows), once the map's layer and the world
    /// are there, with the detail tiles the map holds on the GPU (the player's world only).
    pub(crate) fn api_terrain(&self) -> Option<crate::fork_ext::addons::map_terrain::Source<'_>> {
        use crate::fork_ext::addons::map_terrain::{Source, TileSrc};
        let w = self.home.as_ref().or(self.world.as_ref())?;
        let index = w.overview?;
        let layer = &self.layers.as_ref()?.map;
        let texture = layer.texture_of(index)?;
        let c = &w.builder.cells;
        let texels = c.overview_size();
        if texels.0 == 0 || texels.1 == 0 {
            return None;
        }
        let mut tiles = Vec::new();
        if self.home.is_none() {
            for (&zone, &(t, _)) in &w.tiles {
                if let (Some(cell), Some(texture)) = (c.cell_of.get(zone).copied().flatten(), layer.texture_of(t)) {
                    tiles.push(TileSrc { zone, rect: c.rect(cell), texture });
                }
            }
        }
        Some(Source { texture, extent: c.extent(), texels, world: w.index, texture_index: index, progress: w.builder.progress(), tiles, tile_scale: TILE_SCALE })
    }

    /// Keeps the detail tiles the mod's held views show (`(centre, scale, picture size)` each): asked of the game's
    /// cache, marked as wanted for the minimap's keeping (`mini.wanted`, which `take_pictures` honours) and not let go
    /// as least lately used. Called once a step, before the map's frame, from `a13_play::tick`.
    pub(crate) fn api_terrain_keep(&mut self, views: &[([f32; 2], f32, (u32, u32))]) {
        if self.home.is_some() {
            return;
        }
        let Some(w) = self.world.as_mut() else { return };
        for &(center, scale, size) in views {
            let mut zones = wanted_tiles(&w.builder.cells, view::View { center, scale }, size);
            zones.truncate(12);
            for zi in zones {
                match w.tiles.get_mut(&zi) {
                    Some(t) => t.1 = self.frame,
                    None => {
                        w.builder.want_tile(zi);
                    }
                }
                if !self.mini.wanted.contains(&zi) {
                    self.mini.wanted.push(zi);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "map_terrain_tests.rs"]
mod map_terrain_tests;
