//! Native map output routing: the minimap capture and the one overlay seam.
use render::wgpu;

/// Native minimap output routing. The underlying map preparation/draw remains upstream's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_minimap(map: &crate::map::Map, device: &wgpu::Device, enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, depth: &wgpu::TextureView, window: (u32,u32), size:(u32,u32), _format: wgpu::TextureFormat) {
    #[cfg(not(target_arch="wasm32"))]
    crate::fork_ext::addons::map_views::draw(map,device,enc,target,depth,window,size,_format);
    crate::fork_ext::addons::map_terrain::draw(map,device,enc);
    #[cfg(target_arch="wasm32")]
    map.draw_mini(device,enc,target,depth,window,size);
}

/// One neutral map overlay seam, called with each native map's own transform.
pub(crate) fn overlay(ui:&mut render::ui::Ui,white:usize,world:i32,project:impl Fn([f32;2])->[f32;2],bounds:[f32;4]) {
    #[cfg(not(target_arch="wasm32"))]
    crate::fork_ext::addons::guides::draw(ui,white,world,project,bounds);
    #[cfg(target_arch="wasm32")]
    let _=(ui,white,world,project,bounds);
}

/// The minimap's frame art, taken back out of the layer's quads while a mod's view holds the
/// minimap: the art reaches farther in than the border `INSET` gives the picture (it covers the
/// picture's edge), so a circle cut from the capture would show it. Stock draws keep it.
pub(crate) fn capture_without_frame(quads: &mut Vec<render::ui::Quad>, from: usize) {
    #[cfg(not(target_arch = "wasm32"))]
    if crate::fork_ext::addons::map_views::capturing() {
        quads.truncate(from);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (quads, from);
}
