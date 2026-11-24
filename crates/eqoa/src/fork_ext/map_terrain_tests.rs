//! Round A13's terrain layer on a real (software or hardware) GPU: the overview picture the game's map holds is drawn
//! into a mod's terrain picture, and a known world point lands on the pixel the transform names.
//! Without a GPU adapter the drawing test logs SKIP (counted in the report); the transform tests need none.
use super::*;
use crate::fork_ext::addons::map_terrain::{self, Transform, canvas_size};
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
    let t = Transform { extent: [2000.0, 0.0, 28000.0, 34000.0], size: (783, 1024) };
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
    host.execute("t, err = eqoa.map.terrain({size = 1024}); assert(t ~= nil, err and err.message); local px, py = t.project(1, 1); assert(px == nil, 'not ready before the first draw')").unwrap();
    let mut enc = device.create_command_encoder(&Default::default());
    map_terrain::draw(&map, &device, &mut enc);
    queue.submit([enc.finish()]);
    assert_eq!(map_terrain::draw_count(), before + 1, "the held picture was drawn once");
    // drawn once, not every frame (the control: a second frame with nothing new does not redraw)
    let mut enc = device.create_command_encoder(&Default::default());
    map_terrain::draw(&map, &device, &mut enc);
    queue.submit([enc.finish()]);
    assert_eq!(map_terrain::draw_count(), before + 1, "nothing changed: not drawn again");

    let (canvas, csize) = map_terrain::canvas_of_first().expect("the picture exists");
    let pixels = read(&device, &queue, &canvas, csize);
    let at = |x: f32, y: f32| {
        let (x, y) = (x as u32, y as u32);
        let i = ((y * csize.0 + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };
    let t = Transform { extent, size: csize };
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
    map_terrain::release_vm(7001);
}
