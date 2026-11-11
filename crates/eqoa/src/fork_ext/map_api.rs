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
        let bounds = self.world.as_ref().filter(|w| w.index == world).map(|w| {
            let c = &w.builder.cells;
            json!({"x_min":ee::mul(c.col0 as f32,CELL),"z_min":ee::mul(c.row0 as f32,CELL),"x_max":ee::mul((c.col0+c.cols) as f32,CELL),"z_max":ee::mul((c.row0+c.rows) as f32,CELL)})
        });
        json!({"world_id":world,"world_name":self.world_title(world,words),"zone":zone,"bounds":bounds,"waypoint":self.waypoint(world).map(|p|json!({"world_id":world,"x":p[0],"z":p[1],"owner":null,"label":null}))})
    }
}
