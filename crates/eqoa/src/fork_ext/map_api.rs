//! Native map adapter. Private cache access stays within Map's module.
use super::*;
use serde_json::{Value as Json, json};
impl Map {
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
        json!({"world_id":world,"world_name":world_name,"zone":zone,"bounds":bounds,"waypoint":self.waypoint(world).map(|p|json!({"world_id":world,"x":p[0],"z":p[1],"owner":null,"label":null}))})
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
            out.push(json!({"kind":kind,"name":name,"x":at[0],"z":at[1],"level_min":lo,"level_max":hi,"label":null}));
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
