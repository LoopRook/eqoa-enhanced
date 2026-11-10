//! GPU readback/PNG comparison, following gpu_test.rs and renderer picture tests.
//! Goldens must be reviewed separately; never bless the current output in a test.
use super::*;

fn pixels(gpu: &TestGpu) -> Vec<u8> {
    let target = gpu.last_target.as_ref().unwrap();
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bundled UI readback"),
        size: 1024 * 768 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024 * 4),
                rows_per_image: Some(768),
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    buffer.slice(..).get_mapped_range().unwrap().to_vec()
}

fn compare_picture(id: &str) {
    let Some(mut gpu) = TestGpu::new() else {
        return;
    };
    let tree = bundled_tree();
    let addon = discovery::discover(&tree.0)
        .addons
        .into_iter()
        .find(|a| a.id == id)
        .unwrap();
    let host = Host::for_addon(&addon).unwrap();
    let data = ScratchDir::new();
    super::super::file_io::install(&host, &addon, Some(data.0.clone())).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let textures =
        super::super::textures::Textures::new(&gpu.device, &gpu.queue, gpu.renderer.clone());
    super::super::textures::Textures::install_for_addon(&textures, &host.lua, &addon).unwrap();
    let fonts = Rc::new(RefCell::new(super::super::fonts::FontBank::new(
        0,
        super::super::fonts::current_context_epoch(),
    )));
    super::super::fonts::FontBank::install_for_addon(&fonts, &host.lua, &addon).unwrap();
    let play = crate::play::Play::bundled_ui_fixture();
    let mut vertices = 0;
    for frame in 0..4 {
        fonts.borrow_mut().flush_pending(&mut gpu.context);
        host.snapshot.set(play.forge_snapshot(None));
        gpu.begin();
        bridge.invoke(|| host.run_addon(&addon)).unwrap();
        vertices = gpu.render();
        if frame == 0 && id == "wereoxxs_ui" {
            // The bundled package intentionally defaults every replacement panel off.
            // Select the same visible profile as its existing menu/pause GPU fixture.
            host.execute("for _,key in ipairs({'bars','casting','player','target','group','compass','meter','chat'}) do windows[key]=true end").unwrap();
        }
    }
    assert!(vertices > 0, "{id} must draw its HUD after warmup");
    let actual = pixels(&gpu);
    // Always exercise the negative control, including when no golden is installed:
    // dropping the entire addon layer must change the captured picture.
    gpu.begin();
    gpu.render();
    let blank = pixels(&gpu);
    assert!(
        actual != blank,
        "{id}: missing HUD must fail picture comparison"
    );

    let out = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("r25-{id}.png"));
    let mut encoder = png::Encoder::new(std::fs::File::create(&out).unwrap(), 1024, 768);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&actual)
        .unwrap();
    let Some(root) = std::env::var_os("EQOA_BUNDLED_UI_GOLDENS") else {
        eprintln!(
            "SKIP {id} picture comparison: no reviewed EQOA_BUNDLED_UI_GOLDENS; actual {}",
            out.display()
        );
        return;
    };
    let path = PathBuf::from(root).join(format!("{id}.png"));
    let bytes = std::fs::read(&path).expect("configured golden must exist");
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut expected = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut expected).unwrap();
    assert_eq!(
        (info.width, info.height, info.color_type, info.bit_depth),
        (1024, 768, png::ColorType::Rgba, png::BitDepth::Eight)
    );
    expected.truncate(info.buffer_size());
    assert!(
        actual == expected,
        "{id}: reviewed picture differs; actual {}",
        out.display()
    );
    assert!(
        blank != expected,
        "{id}: golden must reject a missing addon layer"
    );
}

#[test]
fn bundled_wereoxxs_picture_comparison() {
    compare_picture("wereoxxs_ui");
}
#[test]
fn bundled_arpg_picture_comparison() {
    compare_picture("arpg_ui");
}
#[test]
fn bundled_modern_picture_comparison() {
    compare_picture("avoids_modern_ui");
}
