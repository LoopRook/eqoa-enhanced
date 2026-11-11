//! Sync regression: actual minimap composition follows upstream Move HUD.
use super::*;
use crate::hudlayout::{self, Beyond, Piece, Press};

#[test]
fn moved_minimap_moves_circle_border_hover_and_search() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("SKIP moved minimap: no GPU adapter");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    crate::settings::testing::reset(false);
    hudlayout::testing_reset();
    let lib = esf::Library::new();
    let vui = crate::pointer::testing::vui();
    for size in [(640, 480), (1280, 720), (768, 1024)] {
        hudlayout::reset();
        let mut map = crate::map::testing::up(size);
        map.open = false;
        let cx = Context {
            lib: &lib,
            device: &device,
            queue: &queue,
            format: render::TARGET_FORMAT,
            size,
            dt: 0.0,
            player: Some(([0.0; 3], 0.0)),
            world: 0,
            origin: [0.0; 2],
            group: Vec::new(),
            in_group: false,
            target: None,
            sticks: [0.0; 4],
            keyboard: true,
            words: None,
            vui: Some(&vui),
            quest_marks: None,
        };
        map.layers(&cx);
        let mv = MiniView {
            dots: vec![Dot {
                id: 1,
                at: [0.0; 2],
                color: [1.0; 4],
                name: "Rat".into(),
                level: Some(7),
                kind: "NPC",
            }],
            ..Default::default()
        };
        map.mini_frame(&cx, &mv);
        let original = map.mini.circle;
        let outer = map.mini.frame_px;
        map.mini_hover(Some([original[0], original[1]]));
        map.mini_frame(&cx, &mv);
        let before = map.layers.as_ref().unwrap().ui.quads.clone();
        let masked = map.mini.masked_until;
        let b = Beyond::of(size);
        let rect = [
            FRAME[0] as f32 + b.side,
            FRAME[1] as f32 - b.top,
            FRAME[2] as f32 + b.side,
            FRAME[3] as f32 - b.top,
        ];
        hudlayout::frame_begin();
        hudlayout::note(Piece::Minimap, rect, [0.0; 2]);
        hudlayout::frame_end();
        hudlayout::set_unlocked(true);
        let from = [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0];
        assert_eq!(hudlayout::press(from, b), Press::Piece(Piece::Minimap));
        hudlayout::drag_to([from[0] - 160.0, from[1] + 100.0], b);
        hudlayout::release();
        hudlayout::set_unlocked(false);
        let shift = hudlayout::shift(Piece::Minimap, b);
        assert_eq!(shift, [-160.0, 100.0]);
        let origin = window_from_frame(size, [0.0; 2]);
        let shifted = window_from_frame(size, shift);
        let delta = [shifted[0] - origin[0], shifted[1] - origin[1]];
        map.mini_hover(Some([original[0] + delta[0], original[1] + delta[1]]));
        map.mini_frame(&cx, &mv);
        let c = map.mini.circle;
        assert!((c[0] - original[0] - delta[0]).abs() < 0.001);
        assert!((c[1] - original[1] - delta[1]).abs() < 0.001);
        assert_eq!(c[2], original[2]);
        assert!(map.mini_contains([c[0], c[1]]));
        assert!(
            !map.mini_contains([original[0], original[1]]),
            "negative control: old center no longer hits"
        );
        assert!(
            !map.mini_contains([map.mini.frame_px[0], map.mini.frame_px[1]]),
            "corners stay masked"
        );
        let after = &map.layers.as_ref().unwrap().ui.quads;
        assert_eq!(
            before.len(),
            after.len(),
            "same dot card, border, north and search"
        );
        assert!(
            before.len() > masked + 2,
            "hover card and search were composed"
        );
        for (old, new) in before.iter().zip(after) {
            for (a, z) in old.pos.iter().zip(new.pos) {
                assert!(
                    (z[0] - a[0] - shift[0]).abs() < 1.1,
                    "quad follows x: {a:?} -> {z:?}"
                );
                assert!(
                    (z[1] - a[1] - shift[1]).abs() < 1.1,
                    "quad follows y: {a:?} -> {z:?}"
                );
            }
        }
        let old_button = frame_from_window(size, [outer[2], outer[3]]);
        assert!(
            !map.search
                .pointer([old_button[0] - 10.0, old_button[1] + 12.0], false),
            "old search location is inactive"
        );
        assert!(
            map.search.pointer(
                [
                    old_button[0] - 10.0 + shift[0],
                    old_button[1] + 12.0 + shift[1]
                ],
                false
            ),
            "search follows the moved rectangle"
        );
        assert!(map.search.open);
    }
    hudlayout::reset();
    hudlayout::testing_reset();
}
