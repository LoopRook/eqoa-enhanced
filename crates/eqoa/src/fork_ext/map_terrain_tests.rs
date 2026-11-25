//! Round A13's terrain layer on a real (software or hardware) GPU: the overview picture the game's map holds is drawn
//! into a mod's terrain picture, and a known world point lands on the pixel the transform names.
//! Without a GPU adapter the drawing test logs SKIP (counted in the report); the transform tests need none.
use super::*;
use crate::fork_ext::addons::map_terrain::{self, Pic, Transform, ViewReq, canvas_size, plan};
use render::wgpu;

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    pollster::block_on(adapter.request_device(&Default::default())).ok()
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture, size: (u32, u32)) -> Vec<u8> {
    let row = (size.0 * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * size.1) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size.1) } },
        wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = buf.slice(..).get_mapped_range().unwrap();
    let mut px = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for y in 0..size.1 as usize {
        px.extend_from_slice(&data[y * row as usize..y * row as usize + size.0 as usize * 4]);
    }
    px
}

#[test]
fn a13_the_transform_puts_world_points_on_their_pixels_and_back() {
    // Tunaria's overview: 13 x 17 cells of 2000 units, from column 1, row 0
    let t = Transform { extent: [2000.0, 0.0, 28000.0, 34000.0], size: (783, 1024), view: None };
    let (px, py) = t.project(2000.0, 0.0);
    assert!((px, py) == (0.0, 0.0), "the world's top left is the picture's top left");
    let (px, py) = t.project(28000.0, 34000.0);
    assert!((px - 783.0).abs() < 1e-3 && (py - 1024.0).abs() < 1e-3, "and its bottom right the far corner");
    let (px, py) = t.project(25300.0, 15200.0);
    assert!((px - 701.7).abs() < 0.1 && (py - 457.8).abs() < 0.1, "Freeport: {px}, {py}");
    let (x, z) = t.unproject(px, py);
    assert!((x - 25300.0).abs() < 0.5 && (z - 15200.0).abs() < 0.5, "unproject undoes project: {x}, {z}");
    assert!(t.contains(25300.0, 15200.0) && !t.contains(500.0, 15200.0) && !t.contains(28000.0, 100.0));
    // control: a transform that forgot the extent's origin would put Freeport at 25300 / 26000 of the width
    let naive: f32 = 25300.0 / 26000.0 * 783.0;
    assert!((naive - 701.7).abs() > 50.0, "the origin matters: the control is far from the real pixel");
}

#[test]
fn a13_the_picture_is_the_worlds_shape_and_never_bigger_than_the_overview() {
    assert_eq!(canvas_size(1024.0, (1664, 2176)), (783, 1024));
    assert_eq!(canvas_size(4096.0, (1664, 2176)), (1566, 2048), "never past the 2048 a mod may hold");
    assert_eq!(canvas_size(2048.0, (400, 300)), (400, 300), "never larger than the overview itself");
    assert_eq!(canvas_size(1.0, (1664, 2176)).1, 64, "and never below 64");
}

#[test]
fn a13_terrain_draws_the_overview_and_a_known_point_lands_on_its_pixel() {
    let Some((device, queue)) = gpu() else {
        eprintln!("SKIP a13 terrain GPU test: no adapter");
        return;
    };
    map_terrain::release_vm(7001);
    let size = (640, 448);
    let lib = esf::Library::new();
    let cx = Context { lib: &lib, device: &device, queue: &queue, format: render::TARGET_FORMAT, size, dt: 0.0, player: None, world: 0, origin: [0.0; 2], group: Vec::new(), in_group: false, target: None, sticks: [0.0; 4], keyboard: true, words: None, vui: None, quest_marks: None };
    let mut map = crate::map::testing::up(size);
    map.layers(&cx);
    // no overview texture yet: nothing to draw from (the negative control: the source says so, not a blank picture)
    assert!(map.api_terrain().is_none(), "a world whose overview is not on the GPU offers no terrain");
    // the overview: every cell green, Freeport's cell red, as the map's own build would write them
    let (cells, freeport) = {
        let w = map.world.as_ref().unwrap();
        (w.builder.cells.len(), w.builder.cells.at(25300.0, 15200.0).expect("Freeport's cell"))
    };
    let (ow, oh) = map.world.as_ref().unwrap().builder.cells.overview_size();
    let tex = map.layers.as_mut().unwrap().map.texture(&device, ow, oh, OVERVIEW_LEVELS);
    for cell in 0..cells {
        let at = map.world.as_ref().unwrap().builder.cells.overview_at(cell);
        let colour = if cell == freeport { [255, 0, 0, 255] } else { [0, 255, 0, 255] };
        let px: Vec<u8> = (0..128 * 128).flat_map(|_| colour).collect();
        map.layers.as_mut().unwrap().map.write(&queue, tex, at, 128, 128, &px);
    }
    map.world.as_mut().unwrap().overview = Some(tex);
    let src = map.api_terrain().expect("the overview is on the GPU now");
    assert_eq!((src.world, src.texels), (0, (ow, oh)));
    let extent = src.extent;
    drop(src);

    // a mod's lease: drawn only while one is held (the control: nothing is drawn before)
    let before = map_terrain::draw_count();
    let mut enc = device.create_command_encoder(&Default::default());
    map_terrain::draw(&map, &device, &mut enc);
    queue.submit([enc.finish()]);
    assert_eq!(map_terrain::draw_count(), before, "no mod holds a terrain picture: nothing is drawn");

    let host = crate::fork_ext::addons::host::Host::with_test_capabilities(&[]).unwrap();
    crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow_mut().view.in_world = true);
    host.execute("t, err = eqoa.map.terrain({size = 1024}); assert(t ~= nil, err and err.message); local px, py = t.project(1, 1); assert(px == nil and py.code == 'unavailable', 'not loaded before the first draw: ' .. tostring(py and py.code))").unwrap();
    let mut enc = device.create_command_encoder(&Default::default());
    map_terrain::draw(&map, &device, &mut enc);
    queue.submit([enc.finish()]);
    assert_eq!(map_terrain::draw_count(), before + 1, "the held picture was drawn once");
    // drawn once, not every frame (the control: a second frame with nothing new does not redraw)
    let mut enc = device.create_command_encoder(&Default::default());
    map_terrain::draw(&map, &device, &mut enc);
    queue.submit([enc.finish()]);
    assert_eq!(map_terrain::draw_count(), before + 1, "nothing changed: not drawn again");

    let (canvas, csize, id0) = map_terrain::canvas_of_first().expect("the picture exists");
    let pixels = read(&device, &queue, &canvas, csize);
    let at = |x: f32, y: f32| {
        let (x, y) = (x as u32, y as u32);
        let i = ((y * csize.0 + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let t = Transform { extent, size: csize, view: None };
    let (fx, fy) = t.project(25300.0, 15200.0);
    let red = at(fx, fy);
    assert!(red[0] > 200 && red[1] < 60 && red[3] == 255, "Freeport's pixel ({fx}, {fy}) is its red cell: {red:?}");
    let (gx, gy) = t.project(5000.0, 1000.0);
    let green = at(gx, gy);
    assert!(green[1] > 200 && green[0] < 60, "another cell is green: {green:?}");
    // a point two cells over from Freeport is not red: the picture is not one colour
    let (nx, ny) = t.project(25300.0 - 4000.0, 15200.0);
    assert!(at(nx, ny)[0] < 60, "two cells away is green");
    // Lua reads the same transform
    host.execute(&format!(
        "local px, py, on = t.project(25300, 15200); assert(math.abs(px - {fx}) < 0.01 and math.abs(py - {fy}) < 0.01 and on == true, tostring(px)); \
         local x, z = t.unproject(px, py); assert(math.abs(x - 25300) < 0.5 and math.abs(z - 15200) < 0.5); \
         local _, _, off = t.project(100, 100); assert(off == false, 'outside the world'); \
         assert(t.bounds.x_min == {} and t.bounds.z_max == {} and t.world_id == 0 and t.width == {} and t.height == {}); \
         assert(t.progress.total >= 0); \
         assert(t.close() == true); local a, e = t.project(1, 1); assert(a == nil and e.code == 'stale_handle')",
        extent[0], extent[3], csize.0, csize.1
    ))
    .unwrap();
    assert_eq!(map_terrain::held_count(), 0, "closed: released");
    let _ = id0;
    map_terrain::release_vm(7001);
}

#[test]
fn a13_a_view_at_the_games_opening_zoom_uses_the_games_detail_tiles() {
    // Tunaria: 13 x 17 cells from column 1; the game opens on 2500 units across the window's shorter side
    let extent = [2000.0, 0.0, 28000.0, 34000.0];
    let window = (1920u32, 1080u32);
    let opening = crate::map::view::opening_scale(window);
    let around = [25300.0, 15200.0];
    // every cell has a tile on the GPU
    let tiles: Vec<(usize, [f32; 4])> = (0..17)
        .flat_map(|r| (1..14).map(move |c| (r * 13 + (c - 1), [c as f32 * 2000.0, r as f32 * 2000.0, (c + 1) as f32 * 2000.0, (r + 1) as f32 * 2000.0])))
        .collect();
    let tr = Transform { extent, size: window, view: Some(ViewReq { center: around, scale: opening }) };
    let quads = plan(tr, TILE_SCALE, &tiles);
    let used: Vec<_> = quads.iter().filter(|q| matches!(q.pic, Pic::Tile(_))).collect();
    assert!(!used.is_empty() && used.len() <= 9, "tiles for the zones the view meets: {}", used.len());
    // a tile covers 2000 units in 512 texels, drawn `2000 / scale` pixels wide: the game's own geometry
    let q = used[0];
    let units_per_texel = (q.dst[2] - q.dst[0]) / 512.0 * opening;
    assert!((units_per_texel - 2000.0 / 512.0).abs() < 0.01, "3.9 units a texel, as the game's map: {units_per_texel}");
    // the control: the whole-world picture at the largest size has no tiles and 17 units a texel at best
    let whole = Transform { extent, size: (1566, 2048), view: None };
    let overview_only = plan(whole, TILE_SCALE, &tiles);
    assert!(overview_only.iter().all(|q| q.pic == Pic::Overview));
    let overview_units_per_texel = 26000.0 / 1664.0 * (1664.0 / 1566.0);
    assert!(overview_units_per_texel > 2.0 * units_per_texel + 1.0, "the overview path is far blurrier: {overview_units_per_texel}");
    // zoomed out past the game's tile zoom the game uses the overview alone, so does the picture
    let far = Transform { extent, size: window, view: Some(ViewReq { center: around, scale: TILE_SCALE * 2.0 }) };
    assert!(plan(far, TILE_SCALE, &tiles).iter().all(|q| q.pic == Pic::Overview));
    // a view's transform: its middle is the centre
    let (px, py) = tr.project(around[0], around[1]);
    assert!((px - 960.0).abs() < 1e-3 && (py - 540.0).abs() < 1e-3);
    let (x, z) = tr.unproject(px + 100.0, py);
    assert!((x - (around[0] + 100.0 * opening)).abs() < 0.01 && (z - around[1]).abs() < 0.01);
}

const TILE_SCALE: f32 = 2000.0 / 128.0 / 2.0;

fn draw_once(map: &Map, device: &wgpu::Device, queue: &wgpu::Queue) -> std::time::Duration {
    let t = std::time::Instant::now();
    let mut enc = device.create_command_encoder(&Default::default());
    map_terrain::draw(map, device, &mut enc);
    queue.submit([enc.finish()]);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    t.elapsed()
}

#[test]
fn a13_a_view_draws_the_tiles_keeps_its_handle_while_redrawn_and_a_view_change_is_cheap() {
    let Some((device, queue)) = gpu() else {
        eprintln!("SKIP a13 terrain view GPU test: no adapter");
        return;
    };
    let size = (640, 448);
    let lib = esf::Library::new();
    let cx = Context { lib: &lib, device: &device, queue: &queue, format: render::TARGET_FORMAT, size, dt: 0.0, player: None, world: 0, origin: [0.0; 2], group: Vec::new(), in_group: false, target: None, sticks: [0.0; 4], keyboard: true, words: None, vui: None, quest_marks: None };
    let mut map = crate::map::testing::up(size);
    map.layers(&cx);
    let (cells, freeport) = {
        let w = map.world.as_ref().unwrap();
        (w.builder.cells.len(), w.builder.cells.at(25300.0, 15200.0).unwrap())
    };
    let (ow, oh) = map.world.as_ref().unwrap().builder.cells.overview_size();
    let tex = map.layers.as_mut().unwrap().map.texture(&device, ow, oh, OVERVIEW_LEVELS);
    for cell in 0..cells {
        let at = map.world.as_ref().unwrap().builder.cells.overview_at(cell);
        let px: Vec<u8> = (0..128 * 128).flat_map(|_| [0, 255, 0, 255]).collect();
        map.layers.as_mut().unwrap().map.write(&queue, tex, at, 128, 128, &px);
    }
    map.world.as_mut().unwrap().overview = Some(tex);
    let host = crate::fork_ext::addons::host::Host::with_test_capabilities(&[]).unwrap();
    crate::fork_ext::addons::native_core::CORE.with(|c| c.borrow_mut().view.in_world = true);
    host.execute("t = assert(eqoa.map.terrain({center = {x = 25300, z = 15200}, scale = 2.34, width = 640, height = 448}))").unwrap();
    assert_eq!(map_terrain::view_requests().len(), 1, "the client is told which view's tiles to keep");
    let first = draw_once(&map, &device, &queue);
    let (canvas, csize, id0) = map_terrain::canvas_of_first().unwrap();
    assert_eq!(csize, (640, 448));
    let read_at = |x: u32, y: u32| {
        let px = read(&device, &queue, &canvas, csize);
        let i = ((y * csize.0 + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    let centre = read_at(320, 224);
    assert!(centre[1] > 200 && centre[0] < 60, "no tile yet: the overview shows (green): {centre:?}");
    // Freeport's tile arrives (blue): the game's detail replaces the overview where it covers
    let zone = map.world.as_ref().unwrap().builder.cells.zone_at[freeport].unwrap();
    let t = map.layers.as_mut().unwrap().map.texture(&device, 512, 512, render::map::full_chain(512, 512));
    let blue: Vec<u8> = (0..512 * 512).flat_map(|_| [0, 0, 255, 255]).collect();
    map.layers.as_mut().unwrap().map.write(&queue, t, [0, 0], 512, 512, &blue);
    map.world.as_mut().unwrap().tiles.insert(zone, (t, 0));
    let with_tile = draw_once(&map, &device, &queue);
    let (canvas, _, id1) = map_terrain::canvas_of_first().unwrap();
    assert_eq!(id0, id1, "redrawn in place: the texture and so the mod's handle stay");
    let px = read(&device, &queue, &canvas, csize);
    let i = ((224 * csize.0 + 320) * 4) as usize;
    assert!(px[i + 2] > 200 && px[i + 1] < 60, "the tile is drawn over the overview: {:?}", &px[i..i + 4]);
    // control: a view past the game's tile zoom keeps the overview only
    host.execute("assert(t.set_view({scale = 30}) == true)").unwrap();
    draw_once(&map, &device, &queue);
    let (canvas, _, id2) = map_terrain::canvas_of_first().unwrap();
    assert_eq!(id0, id2);
    let px = read(&device, &queue, &canvas, csize);
    assert!(px[i + 1] > 200 && px[i + 2] < 60, "zoomed out: the overview again: {:?}", &px[i..i + 4]);
    // moving the view redraws (and the transform follows at once)
    host.execute("assert(t.set_view({center = {x = 25300 + 1000, z = 15200}, scale = 2.34}) == true); local px = t.project(26300, 15200); assert(math.abs(px - 320) < 0.01, px)").unwrap();
    let moved = draw_once(&map, &device, &queue);
    let (_, _, id3) = map_terrain::canvas_of_first().unwrap();
    assert_eq!(id0, id3, "a view change keeps the handle too");
    eprintln!("a13 terrain view cost (draw + submit + wait, 640x448): first {first:?}, tile arrives {with_tile:?}, view change {moved:?}");
    host.execute("assert(t.close() == true)").unwrap();
    map_terrain::release_vm(0);
}
