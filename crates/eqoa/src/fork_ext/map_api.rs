//! Native map adapter. Private cache access stays within Map's module.
use super::*;
use serde_json::{Value as Json, json};
impl Map {
    /// Small allocation-free source key; dynamic records are rebuilt only on change.
    pub(crate) fn api_key(&self, world: i32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        world.hash(&mut h);
        self.open.hash(&mut h);
        self.mini.up.hash(&mut h);
        for w in [&self.world, &self.home].into_iter().flatten() {
            w.index.hash(&mut h);
            w.name.hash(&mut h);
            let c = &w.builder.cells;
            (c.col0, c.row0, c.cols, c.rows).hash(&mut h);
        }
        for m in &self.quests.marks {
            (m.world, m.x.to_bits(), m.z.to_bits(), &m.npc).hash(&mut h);
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
        h.finish()
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
        json!({"open":self.open,"minimap":self.mini.up,"world_id":world,"world_name":world_name,"zone":zone,"bounds":bounds,"waypoint":self.waypoint(world).map(|p|json!({"world_id":world,"x":p[0],"z":p[1],"owner":null,"label":null}))})
    }
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
            out.push(json!({"world_id":m.world,"kind":"quest","name":m.npc,"x":m.x,"z":m.z,"level_min":levels.map(|x|x.0),"level_max":levels.map(|x|x.1),"label":null}));
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

#[cfg(test)]
impl Map {
    /// Disc-free native chrome/round-minimap scene used behind addon captures.
    /// No terrain art is fabricated; tiles remain absent without a disc.
    pub(crate) fn round4_fixture(device:&wgpu::Device,queue:&wgpu::Queue,size:(u32,u32),mini:bool)->Self{
        let mut map=crate::map::testing::up(size);
        map.open=!mini;
        let lib=esf::Library::new();let vui=crate::pointer::testing::vui();
        let cx=Context{lib:&lib,device,queue,format:wgpu::TextureFormat::Rgba8Unorm,size,dt:0.,player:Some(([25300.,0.,15200.],0.)),world:0,origin:[0.;2],group:Vec::new(),in_group:false,target:None,sticks:[0.;4],keyboard:true,words:None,vui:Some(&vui),quest_marks:None};
        map.layers(&cx);
        if mini {map.mini_frame(&cx,&mini::MiniView::default());} else {map.compose(&cx);}
        map
    }
}

#[cfg(test)]
#[path = "map_render_tests.rs"]
mod render_tests;
