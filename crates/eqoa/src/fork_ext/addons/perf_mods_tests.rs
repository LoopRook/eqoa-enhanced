// Performance round (2026-11-24), the bundled mods: what Chat and the Creature Journal ask the bridge and the API for
// each frame. Included by a6_tests.rs, so it shares that file's helpers (`fresh`, `addon`, `context`, `frames`).
//
// Each test counts calls instead of timing them (the same on any machine), and names its negative control: the change
// that makes the counted calls come back.

/// `CALCS` counts the ImGui text measures a mod asks for; wrapped before the mod runs.
const COUNT_CALCS: &str = "CALCS = 0; local ui = eqoa.imgui; local inner = ui.CalcTextSize; \
    ui.CalcTextSize = function(...) CALCS = CALCS + 1; return inner(...) end";

fn calcs(host: &Host) -> i64 {
    host.lua.load("return CALCS").eval().unwrap()
}

/// A Chat window with many long lines (each wider than the window, so each is cut to fit and drawn).
fn chat_with_long_lines(lines: usize) -> (Host, Bridge, Context) {
    fresh();
    let context = context();
    for i in 0..lines {
        native_core::observe_text(&format!("Dab: line {i} {}", "wandering words ".repeat(12)), [1.0; 4]);
    }
    native_core::flush_deferred();
    native_a6::set_chat_info(native_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say", "combat"])], reply_to: String::new() });
    let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.execute(COUNT_CALCS).unwrap();
    host.run_addon(&addon("eqoa_chat")).unwrap();
    (host, bridge, context)
}

#[test]
fn perf_chat_measures_a_line_once_and_draws_the_next_frames_from_memory() {
    let (host, bridge, mut context) = chat_with_long_lines(60);
    frames(&host, &bridge, &mut context, 3);
    assert!(host.drain_logs().unwrap().is_empty(), "the chat draws without a diagnostic");
    let first = frames(&host, &bridge, &mut context, 1);
    assert!(first > 0, "the window is drawn");

    // 40 more frames of the same chat: one reference measure a frame, nothing else (before: every frame measured each
    // row again, a character at a time for a line wider than the window)
    host.execute("CALCS = 0").unwrap();
    let mut same = true;
    for _ in 0..40 {
        same &= frames(&host, &bridge, &mut context, 1) == first;
    }
    let per_frame = calcs(&host) as f64 / 40.0;
    eprintln!("PERFC chat: {per_frame:.2} text measures a frame over 40 still frames");
    assert!(same, "every frame draws exactly what the first one did");
    assert!(per_frame <= 1.01, "a still chat asks the bridge to measure {per_frame:.2} texts a frame (one reference measure)");

    // negative control 1: a new line of chat is measured (once) and shown at once
    native_core::observe_text("Dab: a brand new line that must show", [1.0; 4]);
    native_core::flush_deferred();
    host.execute("CALCS = 0").unwrap();
    frames(&host, &bridge, &mut context, 1);
    let fresh_calls = calcs(&host);
    assert!(fresh_calls >= 2, "the new line was measured ({fresh_calls} measures)");
    let last: String = host.lua.load("local l = chat_probe.lines(); return l[#l]").eval().unwrap();
    assert!(last.contains("a brand new line"), "the tab shows the new line: {last}");
    host.execute("CALCS = 0").unwrap();
    frames(&host, &bridge, &mut context, 5);
    assert!(calcs(&host) <= 5, "and it is remembered after that ({} measures in 5 frames)", calcs(&host));

    // negative control 2: scrolling back shows older lines, which are measured when first shown
    host.execute("chat_probe.state.scroll = 20").unwrap();
    host.execute("CALCS = 0").unwrap();
    frames(&host, &bridge, &mut context, 1);
    assert!(calcs(&host) >= 4, "older lines shown by scrolling are measured ({})", calcs(&host));
    host.execute("chat_probe.state.scroll = 0").unwrap();
    frames(&host, &bridge, &mut context, 1);

    // negative control 3: a new size of the screen changes the scale: every text is measured and cut again
    super::hud_layout::set_window_size(1600, 900);
    host.execute("CALCS = 0").unwrap();
    frames(&host, &bridge, &mut context, 1);
    let resized = calcs(&host);
    assert!(resized >= 8, "a new scale measures the visible rows again ({resized} measures)");
    host.execute("CALCS = 0").unwrap();
    frames(&host, &bridge, &mut context, 3);
    assert!(calcs(&host) <= 3, "and then they are remembered again ({} in 3 frames)", calcs(&host));
}

#[test]
fn perf_chat_tab_lists_follow_new_lines_and_new_combat_hits() {
    let (host, bridge, mut context) = chat_with_long_lines(3);
    frames(&host, &bridge, &mut context, 3);
    let say = |host: &Host| -> i64 {
        host.execute("chat_probe.state.tab = 'say'").unwrap();
        host.lua.load("return chat_probe.visible()").eval().unwrap()
    };
    let before = say(&host);
    assert_eq!(before, 3, "the Say tab shows the three says");
    // a new Say line shows in the (kept) list of the Say tab the next frame
    native_core::observe_text("Dab: one more", [1.0; 4]);
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 2);
    assert_eq!(say(&host), 4, "a new line reaches the Say tab's list");
    // the Combat tab: a hit adds a line to the log it merges in
    host.execute("chat_probe.state.tab = 'combat'").unwrap();
    frames(&host, &bridge, &mut context, 2);
    let combat: i64 = host.lua.load("return chat_probe.visible()").eval().unwrap();
    CORE.with(|c| {
        c.borrow_mut().emit(
            "combat.hit",
            json!({"outgoing": true, "incoming": false, "kind": "melee", "amount": 7, "attacker_id": 1, "defender_id": 2}),
        )
    });
    frames(&host, &bridge, &mut context, 2);
    let after: i64 = host.lua.load("return chat_probe.visible()").eval().unwrap();
    assert_eq!(after, combat + 1, "a hit reaches the Combat tab's list ({combat} -> {after})");
    // the list is kept between frames: the same table comes back while nothing changed
    let kept: bool = host
        .lua
        .load("local tab = chat_probe.state.tab; local a = chat_probe.lines(); local b = chat_probe.lines(); return #a == #b")
        .eval()
        .unwrap();
    assert!(kept);
}

/// The Creature Journal hears every entity update. An update that changed only fields it does not read (hit points,
/// who the creature targets) is dropped before the work of `record`; one that changed a field it reads is worked.
/// The mod's own file runs against a stand-in `eqoa` that counts `eqoa.player.get`, which `record` asks for once a
/// creature passes its first checks (the journal is ready: its file is "not found").
#[test]
fn perf_journal_drops_updates_of_fields_it_does_not_read() {
    let lua = mlua::Lua::new();
    lua.load(
        r#"
        ASKED, HANDLERS = 0, {}
        eqoa = {
            events = { on = function(name, fn) HANDLERS[name] = fn; return name end },
            options = { button = function() end },
            files = { load_json = function() return nil, { message = "file not found" } end },
            mods = { send = function() end },
            player = { get = function() ASKED = ASKED + 1; return { id = 1, pet_id = 2 } end },
        }
        "#,
    )
    .exec()
    .unwrap();
    let src = std::fs::read_to_string(root("eqoa_creature_journal").join("main.lua")).unwrap();
    lua.load(&src).exec().unwrap();
    lua.load(
        r#"
        function creature(id)
            return { id = id, name = "a rat " .. id, level = 3, is_creature = true, corpse = false, class_code = 0,
                search_flags = 0, world_id = 1, position = { x = id * 100.0, y = 0.0, z = 0.0 } }
        end
        function update(field, from)
            ASKED = 0
            for id = from, from + 39 do
                HANDLERS["entity.changed"]({ frame = id, data = { after = creature(id), changed = { field } } })
            end
            return ASKED
        end
        "#,
    )
    .exec()
    .unwrap();
    let worked: i64 = lua.load("return update('position', 2000)").eval().unwrap();
    let dropped: i64 = lua.load("return update('hp', 3000)").eval().unwrap();
    let no_list: i64 = lua.load("ASKED = 0; HANDLERS['entity.changed']({ frame = 9, data = { after = creature(4000) } }); return ASKED").eval().unwrap();
    eprintln!("PERFJ player.get asks for 40 updates: position {worked}, hp {dropped}; an update with no list: {no_list}");
    assert_eq!(worked, 40, "control: 40 updates of a field it reads are each worked");
    assert_eq!(dropped, 0, "40 hit-point updates were not worked");
    assert_eq!(no_list, 1, "an update that does not say what changed is still worked");
}
