//! Integration checks for the reviewed upstream touch implementation.
use crate::input::bit;
use crate::pointer::testing::{NullRaster, vui};
use crate::touch::{Phase, TouchPad};
use web_time::{Duration, Instant};

#[test]
fn upstream_touch_dpad_reaches_quick_chat_and_releases_without_a_click() {
    let size = (1280, 720);
    let now = Instant::now();
    let layout = crate::touch::layout(size, &crate::touch::Opts::default());
    let mut pad = TouchPad::new(true);
    pad.window = true;
    for (direction, key) in [
        (bit::UP, crate::pointer::key::UP),
        (bit::DOWN, crate::pointer::key::DOWN),
        (bit::LEFT, crate::pointer::key::LEFT),
        (bit::RIGHT, crate::pointer::key::RIGHT),
    ] {
        let mut chat = crate::quickchat::QuickChat::default();
        chat.load(&[net::dump::TextSet {
            id: 0,
            entries: (0..4)
                .map(|id| net::dump::TextEntry {
                    id,
                    text: "Hello".into(),
                    label: "Hello".into(),
                })
                .collect(),
        }]);
        assert!(chat.open());
        let v = vui();
        let (mut cues, mut outs) = (Vec::new(), Vec::new());
        let mut frame = |chat: &mut crate::quickchat::QuickChat, outs: &mut Vec<_>| {
            chat.run(
                &mut NullRaster,
                &mut crate::quickchat::Ctx {
                    vui: &v,
                    now: 0,
                    cues: &mut cues,
                    outs,
                },
            );
        };
        frame(&mut chat, &mut outs);
        assert!(outs.is_empty(), "no direction pressed: nothing said");
        let at = layout
            .buttons
            .iter()
            .find(|b| b.bit == direction)
            .unwrap()
            .at;
        pad.touch(1, Phase::Started, at, now, size);
        assert_eq!(pad.report().buttons, direction);
        let mut console = crate::input::Console::new();
        let mut edges = console.poll(pad.report(), true, false, false);
        // libpad passes through its connection states before it reads a pad.
        for _ in 0..8 {
            if !edges.is_empty() {
                break;
            }
            edges = console.poll(pad.report(), true, false, false);
        }
        assert!(
            edges.contains(&(key, true)),
            "touch through libpad: {edges:?}"
        );
        for (code, down) in edges {
            chat.key(code, down);
        }
        frame(&mut chat, &mut outs);
        pad.touch(1, Phase::Ended, at, now, size);
        assert_eq!(pad.report().buttons, 0);
        let edges = console.poll(pad.report(), true, false, false);
        assert!(edges.contains(&(key, false)));
        for (code, down) in edges {
            chat.key(code, down);
        }
        frame(&mut chat, &mut outs);
        assert!(
            outs.iter().any(
                |o| matches!(o, crate::quickchat::Out::Say(s) if *s == crate::ui::wide("Hello"))
            )
        );
        assert!(!chat.visible());
        assert!(
            pad.take_clicks().is_empty(),
            "D-pad release never becomes a map tap"
        );
    }
}

#[test]
fn upstream_touch_hold_sets_one_map_group_waypoint_and_lift_is_no_tap() {
    crate::settings::testing::reset(false);
    let size = (1280, 720);
    let at = [900.0, 300.0];
    let now = Instant::now();
    let held = now + Duration::from_millis(601);
    let mut map = crate::map::testing::up(size);
    let mut pad = TouchPad::new(true);
    pad.window = true;
    pad.holds = true;
    pad.touch(1, Phase::Started, at, now, size);
    assert!(pad.take_holds(now + Duration::from_millis(599)).is_empty());
    for point in pad.take_holds(held) {
        assert!(map.long_press(point));
    }
    assert!(map.take_group_ask().is_some());
    assert!(pad.take_holds(held).is_empty(), "one hold, one request");
    pad.touch(1, Phase::Ended, at, held, size);
    assert!(pad.take_clicks().is_empty());
    assert!(map.take_group_ask().is_none());
    // With long presses disabled, the identical finger is an ordinary tap.
    pad.holds = false;
    pad.touch(2, Phase::Started, at, now, size);
    assert!(pad.take_holds(held).is_empty());
    pad.touch(2, Phase::Ended, at, held, size);
    assert_eq!(pad.take_clicks().len(), 1);
    crate::settings::testing::reset(false);
}
