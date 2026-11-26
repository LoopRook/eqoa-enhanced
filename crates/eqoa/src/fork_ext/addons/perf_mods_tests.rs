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

/// Owner 2026-11-26: the line being typed wraps at the window's inner width (words whole, a long word broken inside), grows
/// upward by a row each, to four rows, then shows its last four so the caret at the end stays in view.
#[test]
fn owner1126_chat_the_line_being_typed_wraps_inside_the_window_and_grows_upward() {
    let (host, bridge, mut context) = chat_with_long_lines(3);
    frames(&host, &bridge, &mut context, 3);
    let vm = native_events::bus(&host.lua).borrow().vm;
    let open = |text: &str| {
        ws::open_client(vm, "eqoa_chat", "chat_entry", "key", json!({"channel": "say", "text": text}));
        native_core::flush_deferred();
    };
    let probe = |host: &Host| -> (i64, f64, f64, f64, String, f64) {
        host.lua
            .load("local s = chat_probe.state; return #s.entry_rows, s.entry_fit.widest, s.entry_fit.limit, s.entry_fit.full, table.concat(s.entry_rows, '|'), s.rect.y + s.rect.h")
            .eval()
            .unwrap()
    };
    // a short line: one row, the old height
    open("hello");
    frames(&host, &bridge, &mut context, 3);
    let (rows, _, _, _, _, bottom) = probe(&host);
    assert_eq!(rows, 1);
    // close it and type a long line of words: more than one row, none wider than the window, the bottom edge kept
    host.lua.load("chat_probe.state.text = 'the quick brown fox jumps over the lazy dog and then it keeps on running past the edge of the window to be sure'").exec().unwrap();
    frames(&host, &bridge, &mut context, 3);
    let (rows, widest, limit, full, shown, bottom2) = probe(&host);
    assert!(rows > 1 && rows <= 4, "wrapped to {rows} rows: {shown}");
    assert!(widest <= limit + 0.01, "no row wider than the window: {widest} > {limit}");
    assert!(full > limit, "control: on one row the line would have run off the right edge ({full} > {limit})");
    assert!(shown.ends_with('_'), "the caret ends the last row: {shown}");
    assert!((bottom2 - bottom).abs() < 1.5, "it grew upward: the bottom edge stayed ({bottom} -> {bottom2})");
    // each row break fell on a word boundary: the rows put back together are the line
    assert_eq!(shown.replace('|', " ").replace('_', ""), host.lua.load("return chat_probe.state.text").eval::<String>().unwrap(), "{shown}");
    // one long word breaks inside itself, still inside the window
    host.lua.load("chat_probe.state.text = string.rep('W', 60)").exec().unwrap();
    frames(&host, &bridge, &mut context, 3);
    let (rows, widest, limit, _, shown, _) = probe(&host);
    assert!(rows > 1 && widest <= limit + 0.01, "a long word: {rows} rows, {widest} <= {limit}: {shown}");
    // past four rows the last four show, with the caret
    host.lua.load("chat_probe.state.text = string.rep('W', 110)").exec().unwrap();
    frames(&host, &bridge, &mut context, 3);
    let (rows, widest, limit, _, shown, _) = probe(&host);
    assert!(rows <= 4 && widest <= limit + 0.01 && shown.ends_with('_'), "{rows} rows: {shown}");
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

// Owner 2026-11-25: long chat lines ran off the window's right edge. They wrap now. Negative control for all of these: the
// old drawing cut each message to one row, so a long line was one row (and `chat_probe.rows` did not exist).

fn drawn_rows(host: &Host) -> Vec<(String, f64)> {
    let texts: Vec<String> = host.lua.load("local t = {}; for _, r in ipairs(chat_probe.rows()) do t[#t + 1] = r.text end return t").eval().unwrap();
    let widths: Vec<f64> = host.lua.load("local t = {}; for _, r in ipairs(chat_probe.rows()) do t[#t + 1] = r.width end return t").eval().unwrap();
    texts.into_iter().zip(widths).collect()
}

fn inner_width(host: &Host) -> f64 {
    host.lua.load("return chat_probe.state.w - 12").eval().unwrap()
}

#[test]
fn owner1125b_chat_wraps_long_lines_at_the_window_width_on_word_boundaries() {
    let (host, bridge, mut context) = chat_with_long_lines(3);
    frames(&host, &bridge, &mut context, 3);
    let rows = drawn_rows(&host);
    let limit = inner_width(&host);
    // 3 messages, each far wider than the window: more rows than messages, none wider than the window
    assert!(rows.len() > 3, "a long line makes more than one row ({} rows)", rows.len());
    for (text, w) in &rows {
        assert!(*w <= limit + 0.01, "row {text:?} is {w:.1} wide in a window {limit} wide");
    }
    // the newest message's last word is on the last row; rows start and end on a word
    let last: String = host.lua.load("local l = chat_probe.lines(); return l[#l]").eval().unwrap();
    let words: Vec<&str> = last.split_whitespace().collect();
    let shown: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    let tail = shown.join(" ");
    assert!(tail.ends_with(words.last().unwrap()), "the end of the newest line is on the last row: {tail:?}");
    assert!(shown.iter().all(|r| !r.starts_with(' ') && !r.ends_with(' ')), "rows start and end on a word: {shown:?}");
    // a word wider than a row breaks inside the word
    native_core::observe_text(&format!("Dab: {}", "W".repeat(400)), [1.0; 4]);
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 3);
    let rows = drawn_rows(&host);
    assert!(rows.len() >= 4 && rows.iter().all(|r| r.1 <= limit + 0.01), "a 400 character word is broken to fit: {rows:?}");
    // every row keeps the message's colour
    native_core::observe_text(&format!("Dab: {}", "tinted words ".repeat(30)), [1.0, 0.0, 0.0, 1.0]);
    native_core::flush_deferred();
    frames(&host, &bridge, &mut context, 3);
    let reds: i64 = host
        .lua
        .load("local n = 0; for _, r in ipairs(chat_probe.rows()) do if r.color and r.color.r == 1 and r.color.g == 0 then n = n + 1 end end return n")
        .eval()
        .unwrap();
    assert!(reds >= 3, "the continuation rows are red as the message is ({reds} red rows)");
}

#[test]
fn owner1125b_chat_rewraps_on_resize_and_scrolls_by_rows_to_the_oldest_row() {
    let (host, bridge, mut context) = chat_with_long_lines(8);
    frames(&host, &bridge, &mut context, 3);
    let rows_of_newest = |host: &Host| -> i64 { host.lua.load("local l = chat_probe.state.lines; return #l[#l].wrows").eval().unwrap() };
    let at_330 = rows_of_newest(&host);
    host.execute("chat_probe.state.w = 640").unwrap();
    frames(&host, &bridge, &mut context, 3);
    let at_640 = rows_of_newest(&host);
    host.execute("chat_probe.state.w = 220").unwrap();
    frames(&host, &bridge, &mut context, 3);
    let at_220 = rows_of_newest(&host);
    assert!(at_640 < at_330 && at_330 < at_220, "a wider window wraps into fewer rows: {at_640} < {at_330} < {at_220}");
    assert!(drawn_rows(&host).iter().all(|r| r.1 <= inner_width(&host) + 0.01), "and nothing is wider than the window");
    // the newest row is at the bottom with no scrolling; the wheel's rows go back from there
    host.execute("chat_probe.state.scroll = 0").unwrap();
    frames(&host, &bridge, &mut context, 1);
    let bottom = drawn_rows(&host);
    host.execute("chat_probe.state.scroll = 3").unwrap();
    frames(&host, &bridge, &mut context, 1);
    let back = drawn_rows(&host);
    assert_eq!(back.last().unwrap().0, bottom[bottom.len() - 4].0, "three rows back, the bottom row is the fourth from the bottom");
    // too far back is held at the oldest row: the top of the window is the first row of the oldest message
    host.execute("chat_probe.state.scroll = 100000").unwrap();
    frames(&host, &bridge, &mut context, 1);
    let oldest = drawn_rows(&host);
    assert!(oldest[0].0.starts_with("Dab: line 0"), "the oldest message's first row is at the top: {:?}", oldest[0]);
    assert!(host.lua.load("return chat_probe.state.scroll < 100000").eval::<bool>().unwrap(), "the scroll was held at the oldest row");
}

#[test]
fn owner1125b_chat_wraps_only_the_messages_the_visible_rows_come_from() {
    let (host, bridge, mut context) = chat_with_long_lines(60);
    frames(&host, &bridge, &mut context, 3);
    let wrapped: i64 = host
        .lua
        .load("local n = 0; for _, l in ipairs(chat_probe.state.lines) do if l.wrows then n = n + 1 end end return n")
        .eval()
        .unwrap();
    assert!(wrapped >= 1 && wrapped <= 10, "{wrapped} of 60 messages were measured to wrap: only what is shown, not the history");
}

#[test]
fn owner1125b_chat_link_split_across_rows_clicks_on_each_row() {
    fresh();
    let mut context = context();
    // an item link's name (at most 64 characters) that does not fit a row of the narrowest window
    let name = "Sword of the Great Wandering Hero of the Frozen North";
    let text = format!("Dab: {name}");
    native_a6::stash_links(&[(5, name.chars().count() as u16, 4242)]);
    native_core::observe_text(&text, [1.0; 4]);
    native_core::flush_deferred();
    native_a6::set_chat_info(native_a6::ChatInfo { tabs: vec![(0, "Main".into(), vec!["say", "combat"])], reply_to: String::new() });
    let host = Host::for_addon(&addon("eqoa_chat")).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon("eqoa_chat")).unwrap();
    host.execute("chat_probe.state.w = 200").unwrap();
    frames(&host, &bridge, &mut context, 3);
    let n_hits: i64 = host.lua.load("local n = 0; for _, h in ipairs(chat_probe.state.hits) do if h.id == 'link' then n = n + 1 end end return n").eval().unwrap();
    let rows = drawn_rows(&host).len() as i64;
    assert!(rows >= 2 && n_hits == rows, "the link spans {rows} rows and has a click area on each ({n_hits})");
    // a click on the second row's part opens the link
    action_broker::reset_all_for_test();
    let (x, y): (f64, f64) = host
        .lua
        .load("local n = 0; for _, h in ipairs(chat_probe.state.hits) do if h.id == 'link' then n = n + 1; if n == 2 then return h.x + h.w / 2, h.y + h.h / 2 end end end return -1, -1")
        .eval()
        .unwrap();
    assert!(x >= 0.0);
    super::pointer_api::position([x as f32, y as f32], true, (1024, 768));
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
    native_events::dispatch(&host).unwrap();
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["open_link"], "the second row's part of the link opens it: {:?}", host.drain_logs());
}
