// Performance round: what the mods cost in a real-shaped scene (included in manager tests).
impl Rm {
    /// The journal mod's saved file, as its data directory holds it: `{name: {id: {world, level, pos}}}`.
    fn write_journal(&self, journal: &serde_json::Value) {
        let found = super::super::discovery::discover(&self._root.0).addons.into_iter().find(|a| a.id == "eqoa_creature_journal").expect("the journal mod is installed");
        let data = Storage::new(Some(self._config.0.clone())).data_dir(&found).unwrap().unwrap();
        let path = data.join("files/data/journal.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(journal).unwrap()).unwrap();
    }
    /// The 200-odd creatures walk: every one moves a little each frame, and the session tells the API
    /// of an eighth of them each frame (`R4P_CHURN`), as the server's updates do (`entity.changed`).
    fn walk_creatures(&mut self, tick: usize) {
        // the player runs: the position moves every frame, so the snapshot cannot be reused
        self.walker.cam.walk.phys.position.x.0 += 0.15;
        let me = self.me;
        // a creature's update reaches the API every `churn`th frame: 8 is about four a second at 30 frames a second
        let churn: usize = std::env::var("R4P_CHURN").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        for i in 3..self.play.entities.slots.len() {
            let Some(e) = self.play.entities.slots[i].as_mut() else { continue };
            let a = i as f32 * 2.399_963 + tick as f32 * 0.01;
            let d = 20.0 + (i % 40) as f32 * 5.0;
            let at = [me[0] + a.cos() * d, me[1], me[2] + a.sin() * d];
            e.pos = at;
            e.actor_pos = at;
            if (tick + i) % churn == 0 {
                self.play.entity_slot_changed(i);
            }
        }
    }
    /// Runs `n` frames with the creatures walking, and gives the mean and p95 of a whole frame
    /// (CPU to submit, then the GPU's completion) and the mean CPU time to submit.
    fn timed_frames(&mut self, n: usize) -> (f64, f64, f64) {
        let mut whole = Vec::with_capacity(n);
        let mut cpu = 0.0;
        for tick in 0..n {
            self.walk_creatures(tick);
            let t = std::time::Instant::now();
            let tex = self.frame();
            cpu += t.elapsed().as_secs_f64() * 1e3;
            self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            whole.push(t.elapsed().as_secs_f64() * 1e3);
            drop(tex);
        }
        whole.sort_by(f64::total_cmp);
        (whole.iter().sum::<f64>() / n as f64, whole[(n * 95 / 100).min(n - 1)], cpu / n as f64)
    }
}

const WEREOXXS: &str = "wereoxxs_ui";
const WEREOXXS_PANELS: &[&str] = &[
    "DrawAbilityBarWindows(now_ms)",
    "DrawCastingBar(now_ms)",
    "DrawPlayerFrame(now_ms)",
    "DrawTargetFrame(now_ms)",
    "DrawGroupFrames(now_ms)",
    "DrawCompass()",
    "DrawMeter(now_ms)",
    "DrawChat()",
];

/// Instruments a scratch entry function. Keep these probes outside the main timing pass.
fn wereoxxs_profile(rm: &mut Rm, samples: usize) {
    wereoxxs_install_profile(rm);
    for tick in 0..samples {
        wereoxxs_busy_frame(rm, tick + samples);
    }
    wereoxxs_print_profile(rm, samples);
}

fn wereoxxs_install_profile(rm: &mut Rm) {
    let path = rm._root.0.join("wereoxxs_ui/wereoxxs_ui.lua");
    let source = fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let (definitions, frame) = source.rsplit_once("\nApplyDpadRotation()\n").unwrap();
    let mut frame = frame.to_owned();
    for call in WEREOXXS_PANELS {
        let label = call.split_once('(').unwrap().0;
        let statement = format!("then {call} end");
        assert!(frame.contains(&statement));
        frame = frame.replace(
            &statement,
            &format!("then local start = os.clock(); {call}; profile_add('{label}', start) end"),
        );
    }
    for call in [
        "Combat.Update()",
        "AbilityList.Update()",
        "Chat.Update()",
        "ApplyGameUiState(paused)",
        "solo_meter:Update(now_ms, combat_totals)",
        "PumpEnterKey(now_ms, paused)",
    ] {
        let label = call.split_once('(').unwrap().0;
        assert!(frame.contains(call));
        frame = frame.replace(
            call,
            &format!("do local start = os.clock(); {call}; profile_add('{label}', start) end"),
        );
    }
    let l = lua(&rm.manager, WEREOXXS);
    let profiled = l
        .load(format!(
            "{definitions}\n\
             local function profile_add(label, start)\n\
                 profile_ms[label] = (profile_ms[label] or 0) + (os.clock() - start) * 1000\n\
             end\n\
             ApplyDpadRotation()\n{frame}"
        ))
        .into_function()
        .unwrap();
    l.globals()
        .set("profile_ms", l.create_table().unwrap())
        .unwrap();
    l.load(
        r#"
        local load_font = UiForge.LoadFont
        UiForge.LoadFont = function(...)
            local start = os.clock()
            local font = load_font(...)
            profile_ms["UiForge.LoadFont"] = (profile_ms["UiForge.LoadFont"] or 0)
                + (os.clock() - start) * 1000
            return font
        end
    "#,
    )
    .exec()
    .unwrap();
    rm.manager.entries[0].instance.as_mut().unwrap().function = profiled;
}

fn wereoxxs_print_profile(rm: &Rm, samples: usize) {
    let l = lua(&rm.manager, WEREOXXS);
    let rows: Table = l.globals().get("profile_ms").unwrap();
    for row in rows.pairs::<String, f64>() {
        let (name, ms) = row.unwrap();
        eprintln!(
            "wereoxxs profile {name}: {:.3} ms/frame",
            ms / samples as f64
        );
    }
}

#[test]
fn measure_wereoxxs_recorded_session() {
    use super::super::addon_perf;
    use crate::fork_ext::session_replay;

    let Some(paths) = std::env::var_os("EQOA_WEREOXXS_RECORDINGS") else {
        eprintln!("SKIP Wereoxxs recording replay: set EQOA_WEREOXXS_RECORDINGS");
        return;
    };
    let parts: Vec<_> = std::env::split_paths(&paths)
        .map(|path| fs::read(path).expect("read recording part"))
        .collect();
    let rows: Vec<_> = parts
        .iter()
        .flat_map(|part| session_replay::rows(part).expect("decode recording part"))
        .collect();
    assert!(rows.windows(2).all(|pair| pair[0].micros <= pair[1].micros));
    let intervals = [(41, 43), (183, 185), (525, 527)];
    let first_us = rows.first().expect("recording has rows").micros;
    let last_us = rows.last().unwrap().micros;
    assert!(
        first_us <= intervals[0].0 * 1_000_000 && last_us >= intervals[2].1 * 1_000_000,
        "the recording must cover all three sample intervals"
    );

    let profile: Option<serde_json::Value> = std::env::var_os("EQOA_WEREOXXS_SAVED_PROFILE")
        .map(|path| serde_json::from_slice(&fs::read(path).unwrap()).unwrap());
    let size = std::env::var("EQOA_WEREOXXS_SIZE").map_or(RM_SIZE, |size| {
        let (width, height) = size.split_once('x').expect("use WIDTHxHEIGHT");
        (width.parse().unwrap(), height.parse().unwrap())
    });
    let Some(mut rm) = Rm::sized(|p| matches!(p, WEREOXXS | "resources"), 0, size, 1) else {
        eprintln!("SKIP Wereoxxs recording replay: no GPU adapter");
        return;
    };
    let reference = std::env::var_os("EQOA_WEREOXXS_REFERENCE_ENTRY");
    if let Some(path) = reference {
        fs::copy(path, rm._root.0.join("wereoxxs_ui/wereoxxs_ui.lua")).unwrap();
    }
    addon_perf::force_for_test(true);
    rm.play = crate::play::Play::replay_ui_fixture();
    rm.play.map.mini.up = false;
    let lib = esf::Library::new();
    let mut renderer = render::Renderer::new(&rm.gpu.device, &rm.gpu.queue, render::TARGET_FORMAT);
    if let Some(p) = &profile {
        rm.gpu
            .context
            .load_ini_settings(p["layout"].as_str().unwrap());
    }
    rm.manager.queue(Command::Enable(WEREOXXS.into(), true));
    rm.frames(3);
    assert_eq!(
        rm.manager.entries[0].status,
        Status::Enabled,
        "{:?}",
        rm.manager.diagnostics()
    );
    let l = lua(&rm.manager, WEREOXXS);
    l.load("os.date = function() return '12:34' end")
        .exec()
        .unwrap();
    if let Some(p) = &profile {
        let entry = &rm.manager.entries[0];
        let saved = super::super::profile::decode(&l, &p["addons"][&entry.key]).unwrap();
        assert!(
            matches!(saved, Value::Table(_)),
            "the profile has no Wereoxxs settings"
        );
        Manager::invoke(entry.instance.as_ref().unwrap(), 3, saved, false).unwrap();
    } else {
        l.load(
            "for _, key in ipairs({'bars','casting','player','target','group','compass','meter','chat'}) do
                 windows[key] = true
             end",
        ).exec().unwrap();
    }
    let profiling = std::env::var_os("EQOA_WEREOXXS_PROFILE").is_some();
    if profiling {
        wereoxxs_install_profile(&mut rm);
    }

    let mut rows = rows.iter().peekable();
    let mut frame_us = first_us;
    let mut frames = 0;
    let mut interval = 0;
    while let Some(&(start, end)) = intervals.get(interval) {
        if frame_us >= end * 1_000_000 {
            assert!(frames >= 30);
            let messages: usize = l.load("return #chat_state.messages").eval().unwrap();
            let host = &rm.manager.entries[0].instance.as_ref().unwrap().host;
            let snapshot = host.snapshot.get();
            assert!(snapshot.in_game && snapshot.abilities.ability_count > 0);
            eprintln!(
                "wereoxxs replay {start}-{end}s: {frames} frames, {messages} chat rows, {} abilities, {} effects, {:.3} ms/frame",
                snapshot.abilities.ability_count,
                snapshot.effects.count,
                addon_perf::peek_addon_ms(WEREOXXS),
            );
            for line in addon_perf::take_report() {
                eprintln!("wereoxxs replay {line}");
            }
            if profiling {
                wereoxxs_print_profile(&rm, frames);
            }
            if std::env::var_os("EQOA_WEREOXXS_CAPTURE").is_some() {
                use sha2::{Digest, Sha256};
                let pixels = rm_pixels(&rm.gpu, &rm.target.as_ref().unwrap().0);
                assert!(pixels.chunks_exact(4).any(|rgba| rgba != &pixels[..4]));
                eprintln!(
                    "wereoxxs replay {start}-{end}s GPU RGBA SHA-256 {:x}",
                    Sha256::digest(pixels)
                );
            }
            interval += 1;
            frames = 0;
            continue;
        }
        while rows.peek().is_some_and(|row| row.micros <= frame_us) {
            let row = rows.next().unwrap();
            if row.sent && row.ty == 0x40 && row.bytes.len() == 41 {
                rm.play
                    .replay_ui_target(u32::from_le_bytes(row.bytes[37..41].try_into().unwrap()));
            } else {
                rm.play.enqueue_replay_ui_row(row);
            }
        }
        rm.play.frame(
            &mut rm.walker,
            &lib,
            crate::play::Gpu {
                renderer: &mut renderer,
                device: &rm.gpu.device,
                queue: &rm.gpu.queue,
            },
            (frame_us / 1_000) as i64,
            true,
            None,
        );
        rm.play.prepare_replay_ui_frame();
        let measuring = frame_us >= start * 1_000_000;
        if measuring {
            if frames == 0 {
                addon_perf::take_report();
                if profiling {
                    l.globals()
                        .set("profile_ms", l.create_table().unwrap())
                        .unwrap();
                }
            }
            frames += 1;
        }
        rm.frame();
        frame_us += if measuring { 33_333 } else { 250_000 };
    }
    assert_eq!(
        rm.manager.entries[0].status,
        Status::Enabled,
        "{:?}",
        rm.manager.diagnostics()
    );
    rm.manager.shutdown();
    addon_perf::force_for_test(false);
}

fn wereoxxs_busy_frame(rm: &mut Rm, tick: usize) {
    rm.walk_creatures(tick);
    if tick.is_multiple_of(4) {
        let line = format!(
            "Party traffic {tick}: moving towards the camp while the healer recovers power."
        );
        super::super::native_core::observe_text(&line, [0.5, 0.75, 1.0, 1.0]);
    }
    rm.frame();
}

#[test]
fn wereoxxs_custom_font_load_budget() {
    let Some(mut rm) = Rm::with(|p| matches!(p, WEREOXXS | "resources"), 0) else {
        return;
    };
    if let Some(path) = std::env::var_os("EQOA_WEREOXXS_REFERENCE_ENTRY") {
        fs::copy(path, rm._root.0.join("wereoxxs_ui/wereoxxs_ui.lua")).unwrap();
    }
    rm.manager.queue(Command::Enable(WEREOXXS.into(), true));
    rm.frames(3);
    assert_eq!(
        rm.manager.entries[0].status,
        Status::Enabled,
        "{:?}",
        rm.manager.diagnostics()
    );
    let l = lua(&rm.manager, WEREOXXS);
    l.load(
        r#"
        local load = UiForge.LoadFont
        font_loads = 0
        UiForge.LoadFont = function(...)
            font_loads = font_loads + 1
            return load(...)
        end
        windows.chat = true
        font_configs.chat.font = "Merriweather Sans"
        font_configs.chat.size = 44
    "#,
    )
    .exec()
    .unwrap();
    rm.frames(8);
    let loads = l.globals().get::<u32>("font_loads").unwrap();
    assert_eq!(
        loads, 1,
        "eight frames at one font size use one font request"
    );

    l.load("font_configs.chat.size = 28").exec().unwrap();
    rm.frames(4);
    assert_eq!(l.globals().get::<u32>("font_loads").unwrap(), 2);
    l.load("font_configs.chat.font = 'Cinzel'").exec().unwrap();
    rm.frames(4);
    assert_eq!(l.globals().get::<u32>("font_loads").unwrap(), 3);
    assert_eq!(
        rm.manager.entries[0].status,
        Status::Enabled,
        "{:?}",
        rm.manager.diagnostics()
    );
    rm.manager.shutdown();
}

#[test]
fn wereoxxs_busy_zone_frame_cost() {
    big_stack(wereoxxs_busy_zone_frame_cost_body)
}
fn wereoxxs_busy_zone_frame_cost_body() {
    let reference = std::env::var_os("EQOA_WEREOXXS_REFERENCE_CHAT").map(PathBuf::from);
    let before = reference
        .as_deref()
        .and_then(|path| wereoxxs_run_busy_zone(Some(path), true));
    let after = wereoxxs_run_busy_zone(None, before.is_some());
    if let Some(before) = before {
        let after = after.expect("the bundled UI must render when the original rendered");
        assert!(
            before == after,
            "the busy-zone picture differs from the original"
        );
        eprintln!("wereoxxs busy-zone picture matches the original byte for byte");
    }
}

fn wereoxxs_run_busy_zone(reference_chat: Option<&Path>, capture: bool) -> Option<Vec<u8>> {
    use super::super::{
        addon_perf,
        chat::{MSG_TYPE_GUILD, MSG_TYPE_PARTY, MSG_TYPE_SAY, MSG_TYPE_SHOUT, MSG_TYPE_TELL},
        host::Host,
        native_core,
    };

    addon_perf::force_for_test(true);
    let Some(mut rm) = Rm::with(|p| matches!(p, WEREOXXS | "resources"), 200) else {
        addon_perf::force_for_test(false);
        return None;
    };
    let label = if let Some(path) = reference_chat {
        fs::copy(path, rm._root.0.join("wereoxxs_ui/modules/chat_window.lua")).unwrap();
        "original"
    } else {
        "bundled"
    };
    rm.play.fill_bundled_ui_combat_panels();
    rm.manager.queue(Command::Enable(WEREOXXS.into(), true));
    rm.frames(3);
    let l = lua(&rm.manager, WEREOXXS);
    l.load("os.date = function() return '12:34' end")
        .exec()
        .unwrap();
    let kinds = [
        MSG_TYPE_SAY,
        MSG_TYPE_PARTY,
        MSG_TYPE_GUILD,
        MSG_TYPE_SHOUT,
        MSG_TYPE_TELL,
    ];
    for index in 0..2000 {
        let kind = kinds[index % kinds.len()];
        let prefix = if kind == MSG_TYPE_TELL {
            "Scout tells you, "
        } else {
            ""
        };
        let text = format!(
            "{prefix}Chat line {index}: moving towards the camp while the healer recovers power."
        );
        native_core::observe_text(&text, [f32::from_bits(kind), 0.75, 1.0, 1.0]);
        if (index + 1).is_multiple_of(250) {
            rm.frame();
        }
    }
    l.load(r#"
        assert(#chat_state.messages == 2000)
        for _, key in ipairs({'bars','casting','player','target','group','compass','meter','chat'}) do
            windows[key] = true
        end
    "#).exec().unwrap();
    rm.frames(8);

    const SAMPLES: usize = 40;
    let mut instructions = 0;
    let mut peak_instructions = 0;
    addon_perf::take_report();
    for tick in 0..SAMPLES {
        wereoxxs_busy_frame(&mut rm, tick);
        let instance = rm.manager.entries[0].instance.as_ref().unwrap();
        let used = Host::INSTRUCTION_BUDGET - instance.host.instructions_remaining();
        instructions += used;
        peak_instructions = peak_instructions.max(used);
    }
    assert_eq!(
        rm.manager.entries[0].status,
        Status::Enabled,
        "{:?}",
        rm.manager.diagnostics()
    );
    let snapshot = rm.manager.entries[0]
        .instance
        .as_ref()
        .unwrap()
        .host
        .snapshot
        .get();
    assert_eq!(snapshot.abilities.ability_count, 14);
    for (bar, count) in snapshot.abilities.bars.iter().zip([5, 5, 4]) {
        assert_eq!(bar.slot_count, count);
    }
    assert_eq!(snapshot.group.member_count, 5);
    assert_eq!(snapshot.effects.count, 8);
    assert!(l.load("return chat_state.messages[#chat_state.messages].text:find('Party traffic', 1, true) ~= nil").eval::<bool>().unwrap(), "live chat reached the warmed scrollback");
    eprintln!(
        "wereoxxs busy zone ({label}): {:.3} ms/frame; {} Lua instructions/frame, peak {peak_instructions}",
        addon_perf::peek_addon_ms(WEREOXXS),
        instructions as usize / SAMPLES,
    );
    for line in addon_perf::take_report() {
        eprintln!("wereoxxs busy zone ({label}): {line}");
    }
    // An instruction budget keeps the regression independent of machine load.
    const PEAK_INSTRUCTION_BUDGET: u32 = 110_000;
    if reference_chat.is_some() {
        assert!(
            peak_instructions > PEAK_INSTRUCTION_BUDGET,
            "the original chat must exceed the budget"
        );
        eprintln!("wereoxxs original exceeded the {PEAK_INSTRUCTION_BUDGET} instruction budget");
    } else {
        assert!(
            peak_instructions <= PEAK_INSTRUCTION_BUDGET,
            "wereoxxs used {peak_instructions} instructions in one frame (budget {PEAK_INSTRUCTION_BUDGET})"
        );
    }
    if std::env::var_os("EQOA_WEREOXXS_PROFILE").is_some() {
        wereoxxs_profile(&mut rm, SAMPLES);
        assert_eq!(
            rm.manager.entries[0].status,
            Status::Enabled,
            "{:?}",
            rm.manager.diagnostics()
        );
    }
    addon_perf::force_for_test(false);
    let pixels = capture.then(|| {
        let mut normal = rm_pixels(&rm.gpu, &rm.target.as_ref().unwrap().0);
        l.load(
            r#"
            chat_config.show_timestamps = false
            chat_config.color_tell = {0.4, 1, 0.6, 1}
            local begin = ImGui.BeginTabItem
            ImGui.BeginTabItem = function(name)
                if name == 'Scout' then
                    local _, draw = begin(name, true, ImGuiTabItemFlags.SetSelected)
                    return draw
                end
                return begin(name)
            end
        "#,
        )
        .exec()
        .unwrap();
        let target = rm.frames(4);
        let filtered = rm_pixels(&rm.gpu, &target);
        assert!(
            filtered != normal,
            "the selected tell tab must change the picture"
        );
        normal.extend(filtered);
        normal
    });
    rm.manager.shutdown();
    pixels
}

/// A journal in which "Test Rat" (the creature in the busy world) has `spawns` known spawns
/// scattered round the player, in the player's world.
fn perf_journal(world: i64, me: [f32; 3], spawns: usize) -> serde_json::Value {
    let rows: serde_json::Map<String, serde_json::Value> = (0..spawns)
        .map(|i| {
            let a = i as f64 * 2.399_963;
            let d = 15.0 + (i % 60) as f64 * 5.0;
            let at = serde_json::json!({"world": world, "level": 7, "pos": [me[0] as f64 + a.cos() * d, me[1], me[2] as f64 + a.sin() * d]});
            ((10000 + i).to_string(), at)
        })
        .collect();
    serde_json::json!({ "Test Rat": rows })
}

/// Searches "Test Rat" in the map search mod and guides to it, as the player does.
fn perf_set_guide(rm: &mut Rm) {
    let l = lua(&rm.manager, "eqoa_map_search");
    l.load("local s=require('search');s.toggle();s.query='test rat';s.dirty=true").exec().unwrap();
    // the journal's file is read by a worker: give it wall-clock time, as the large-journal test does
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        rm.walk_creatures(0);
        rm.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let ready: bool = l.load("local s=require('search');return not s.dirty and not s.build and #s.rows>0 and #s.journal>=300").eval().unwrap();
        if ready {
            break;
        }
    }
    let row: String = l.load("local s=require('search');return s.rows[1] and s.rows[1].name or ''").eval().unwrap();
    assert_eq!(row, "Test Rat", "the search found the creature");
    l.load("local s=require('search');s.choose(s.rows[1])").exec().unwrap();
    rm.frames(3);
}

/// Waits (wall clock) until the map search has read its catalogue and the journal.
fn perf_wait_for_loads(rm: &mut Rm) {
    rm.frames(3);
    let l = lua(&rm.manager, "eqoa_map_search");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        rm.frame();
        std::thread::sleep(std::time::Duration::from_millis(2));
        if l.load("local s=require('search');return #s.jobs==0 and #s.catalog>1000 and #s.journal>=300").eval::<bool>().unwrap() {
            return;
        }
    }
    panic!("the catalogue and journal did not load");
}

/// A guide with 300+ known spawns costs little: the map search's own time plus the guide's native
/// update and drawing, with the guide set against the same scene without it, is under half a
/// millisecond a frame, and the guide's dots really are drawn on the minimap.
#[test]
fn round4_guide_costs_under_half_a_millisecond() {
    big_stack(round4_guide_costs_under_half_a_millisecond_body)
}
fn round4_guide_costs_under_half_a_millisecond_body() {
    super::super::addon_perf::force_for_test(false);
    let packages = |p: &str| matches!(p, "eqoa_creature_journal" | "eqoa_map_search" | "eqoa_round_minimap");
    let samples: usize = std::env::var("R4C_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
    let mut costs = Vec::new();
    for guided in [false, true] {
        let Some(mut rm) = Rm::with(packages, 200) else { return };
        let world = rm.play.player.world as i64;
        rm.write_journal(&perf_journal(world, rm.me, 420));
        perf_wait_for_loads(&mut rm);
        assert!(rm.manager.entries.iter().all(|e| e.status == Status::Enabled), "{:?}", rm.manager.diagnostics());
        if guided {
            perf_set_guide(&mut rm);
            assert_eq!(super::super::guides::guide_count(), 1, "the guide is set");
            let dots = super::super::guides::dots_for_any();
            assert!(dots >= 300, "a guide with 300+ known spawns: {dots}");
        } else {
            assert_eq!(super::super::guides::guide_count(), 0);
        }
        rm.frames(20);
        super::super::addon_perf::force_for_test(true);
        let before = super::super::guides::quads_drawn();
        let (mean, p95, cpu) = rm.timed_frames(samples);
        let drawn = super::super::guides::quads_drawn() - before;
        if guided {
            assert!(drawn > 0, "the guide's dots and line reach the minimap's quads");
        } else {
            assert_eq!(drawn, 0, "no guide, nothing drawn");
        }
        let mod_ms = super::super::addon_perf::peek_addon_ms("eqoa_map_search");
        let native_ms = super::super::addon_perf::peek_section_ms("guide.draw") + super::super::addon_perf::peek_section_ms("guide.update");
        eprintln!("R4P guide={guided}: frames={samples} mean={mean:.3} ms (CPU {cpu:.3}) p95={p95:.3}; map search {mod_ms:.3} ms + guide native {native_ms:.3} ms; guide quads/frame {:.1}", drawn as f64 / samples as f64);
        for line in super::super::addon_perf::take_report() {
            eprintln!("R4P   {line}");
        }
        super::super::addon_perf::force_for_test(false);
        costs.push(mod_ms + native_ms);
        rm.manager.shutdown();
    }
    let cost = costs[1] - costs[0];
    eprintln!("R4P guide cost: {cost:.3} ms a frame");
    if !cfg!(debug_assertions) {
        assert!(cost < 0.5, "the guide costs {cost:.3} ms a frame");
    }
}

/// How long one entity record takes to build as a guarded Lua record (what `entities.list` and every
/// `entity.*` event pay per entity).
#[test]
fn round4_record_build_micro() {
    big_stack(round4_record_build_micro_body)
}
fn round4_record_build_micro_body() {
    let Some(mut rm) = Rm::with(|p| p == "eqoa_round_minimap", 20) else { return };
    rm.frames(8);
    let json = super::super::native_core::CORE.with(|c| c.borrow().view.entities[6].clone());
    let l = lua(&rm.manager, "eqoa_round_minimap");
    let n = 4000;
    let t = std::time::Instant::now();
    for _ in 0..n {
        let v = super::super::native::record(&l, "eqoa.entities.Entity", &json).unwrap();
        drop(v);
    }
    eprintln!("R4P record build: {:.2} us per entity record ({} fields)", t.elapsed().as_secs_f64() * 1e6 / n as f64, json.as_object().unwrap().len());
    // where it goes: the same leaves in a bare table, then the three-table guard around nothing
    let leaves: Vec<(&String, &serde_json::Value)> = json.as_object().unwrap().iter().filter(|(_, v)| !v.is_object()).collect();
    let t = std::time::Instant::now();
    for _ in 0..n {
        let b = l.create_table_with_capacity(0, leaves.len()).unwrap();
        for (k, v) in &leaves {
            match v {
                serde_json::Value::Bool(x) => b.raw_set(k.as_str(), *x).unwrap(),
                serde_json::Value::Number(x) => b.raw_set(k.as_str(), x.as_f64().unwrap()).unwrap(),
                serde_json::Value::String(x) => b.raw_set(k.as_str(), x.as_str()).unwrap(),
                _ => {}
            }
        }
        drop(b);
    }
    eprintln!("R4P   bare table with {} leaves: {:.2} us", leaves.len(), t.elapsed().as_secs_f64() * 1e6 / n as f64);
    let t = std::time::Instant::now();
    for _ in 0..n {
        let mut items: Vec<(&str, mlua::Value)> = Vec::with_capacity(leaves.len());
        for (k, v) in &leaves {
            match v {
                serde_json::Value::Bool(x) => items.push((k.as_str(), mlua::Value::Boolean(*x))),
                serde_json::Value::Number(x) => items.push((k.as_str(), mlua::Value::Number(x.as_f64().unwrap()))),
                serde_json::Value::String(x) => items.push((k.as_str(), mlua::Value::String(l.create_string(x).unwrap()))),
                _ => {}
            }
        }
        let b = l.create_table_from(items).unwrap();
        drop(b);
    }
    eprintln!("R4P   create_table_from with {} leaves: {:.2} us", leaves.len(), t.elapsed().as_secs_f64() * 1e6 / n as f64);
    let t = std::time::Instant::now();
    for _ in 0..n {
        let b = l.create_table().unwrap();
        let v = super::super::native::guard(&l, b, "eqoa.entities.Entity", None).unwrap();
        drop(v);
    }
    eprintln!("R4P   one guard round an empty table: {:.2} us", t.elapsed().as_secs_f64() * 1e6 / n as f64);
    let t = std::time::Instant::now();
    for _ in 0..n {
        let b = l.create_table().unwrap();
        drop(b);
    }
    eprintln!("R4P   one create_table: {:.2} us", t.elapsed().as_secs_f64() * 1e6 / n as f64);
    rm.manager.shutdown();
}

/// The seven mods' costs in a scene shaped like the real game: 200 creatures walking with the session's
/// updates reaching the API, the stock minimap's art, the native HUD layer drawn through a `Ui` to the
/// GPU, the ImGui pass, to the GPU's completion. Printed per mod (`addons:`, `core:`, `addon-api:`).
#[test]
fn round4_seven_mods_in_a_real_shaped_scene() {
    big_stack(round4_seven_mods_in_a_real_shaped_scene_body)
}
fn round4_seven_mods_in_a_real_shaped_scene_body() {
    use super::super::{addon_perf, hud_layer};
    let real = crate::map::extension::use_real_ui(true);
    eprintln!("R4P art: {}", if real { "the disc's atlas and strings (surfaces and font are stand-ins)" } else { "stand-ins (no disc)" });
    const SEVEN: [&str; 7] = ["eqoa_stats_box", "eqoa_target_of_target", "eqoa_mana_bars", "eqoa_hud_numbers", "eqoa_creature_journal", "eqoa_map_search", "eqoa_round_minimap"];
    let samples: usize = std::env::var("R4C_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let mut rows: Vec<(String, Vec<&str>)> = vec![("no mods".into(), vec![])];
    for m in SEVEN {
        rows.push((m.into(), vec![m]));
    }
    rows.push(("all seven".into(), SEVEN.to_vec()));
    let mut means = Vec::new();
    for (label, mods) in rows {
        addon_perf::force_for_test(false);
        let Some(rm) = Rm::with(|p| mods.contains(&p) || (p == "eqoa_creature_journal" && mods.contains(&"eqoa_map_search")), 200) else {
            crate::map::extension::use_real_ui(false);
            return;
        };
        let mut rm = rm.with_hud();
        let world = rm.play.player.world as i64;
        if mods.contains(&"eqoa_creature_journal") || mods.contains(&"eqoa_map_search") {
            rm.write_journal(&perf_journal(world, rm.me, 420));
        }
        rm.frames(6);
        if mods.contains(&"eqoa_stats_box") {
            lua(&rm.manager, "eqoa_stats_box").load("eqoa.options.set('stats_box', true)").exec().unwrap();
        }
        if mods.contains(&"eqoa_target_of_target") {
            lua(&rm.manager, "eqoa_target_of_target").load("eqoa.options.set('target_of_target', true)").exec().unwrap();
        }
        if mods.contains(&"eqoa_map_search") {
            perf_wait_for_loads(&mut rm);
        }
        rm.frames(30);
        assert!(rm.manager.entries.iter().all(|e| e.status == Status::Enabled), "{label}: {:?}", rm.manager.diagnostics());
        assert!(rm.state_len_dots() >= 200, "200 dots for the native minimap");
        addon_perf::force_for_test(true);
        let before: Vec<usize> = mods.iter().map(|m| hud_layer::drawn_commands(m)).collect();
        let (mean, p95, cpu) = rm.timed_frames(samples);
        let drawn: Vec<(String, usize)> = mods.iter().zip(&before).map(|(m, b)| (m.to_string(), hud_layer::drawn_commands(m) - b)).collect();
        let hud: String = drawn.iter().filter(|(_, n)| *n > 0).map(|(m, n)| format!("{m} {:.1}", *n as f64 / samples as f64)).collect::<Vec<_>>().join(", ");
        eprintln!("R4P [{label}] frames={samples} mean={mean:.3} ms (CPU {cpu:.3}) p95={p95:.3}; HUD commands/frame: {hud}");
        for line in addon_perf::take_report() {
            eprintln!("R4P     {line}");
        }
        addon_perf::force_for_test(false);
        means.push((label, mean));
        rm.manager.shutdown();
    }
    let base = means[0].1;
    for (label, mean) in &means {
        eprintln!("R4P over no mods: {label}: {:+.3} ms", mean - base);
    }
    crate::map::extension::use_real_ui(false);
}

/// The bundled Forge UIs in the same scene, their time split into the Forge layer (the `ImGui` bridge, the
/// draw lists, the `frontiers_forge.*` adapters) and their own Lua. Printed; the workload is asserted
/// (each UI ran and drew).
#[test]
fn round4_forge_uis_split_between_our_layer_and_their_lua() {
    big_stack(round4_forge_uis_split_between_our_layer_and_their_lua_body)
}
fn round4_forge_uis_split_between_our_layer_and_their_lua_body() {
    use super::super::addon_perf;
    let real = crate::map::extension::use_real_ui(true);
    eprintln!("R4P art: {}", if real { "the disc's atlas and strings" } else { "stand-ins (no disc)" });
    let samples: usize = std::env::var("R4C_SAMPLES").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let mut base = None;
    for package in ["", "wereoxxs_ui", "arpg_ui", "avoids_modern_ui"] {
        // on before the addon is made: the ImGui and adapter wrappers are put in at install time
        addon_perf::force_for_test(true);
        let Some(rm) = Rm::with(|p| !package.is_empty() && (p == package || p == "resources"), 200) else {
            crate::map::extension::use_real_ui(false);
            return;
        };
        let mut rm = rm.with_hud();
        rm.frames(4);
        if !package.is_empty() {
            rm.manager.queue(Command::Enable(package.to_string(), true));
        }
        rm.frames(10);
        if package == "wereoxxs_ui" {
            // his windows start off until the player turns them on in his settings: all on, as the owner runs it
            lua(&rm.manager, package)
                .load("for _, k in ipairs({'bars','casting','player','target','group','compass','meter','chat'}) do windows[k] = true end")
                .exec()
                .unwrap();
        }
        rm.frames(30);
        if !package.is_empty() {
            assert!(rm.manager.entries.iter().all(|e| e.status == Status::Enabled), "{package}: {:?}", rm.manager.diagnostics());
            assert_eq!(rm.manager.entries.len(), 1, "{package} loaded");
        }
        let _ = addon_perf::take_report();
        let (mean, p95, cpu) = rm.timed_frames(samples);
        let label = if package.is_empty() { "no mods" } else { package };
        eprintln!("R4P [{label}] frames={samples} mean={mean:.3} ms (CPU {cpu:.3}) p95={p95:.3}");
        for line in addon_perf::take_report() {
            eprintln!("R4P     {line}");
        }
        addon_perf::force_for_test(false);
        match base {
            None => base = Some(mean),
            Some(b) => eprintln!("R4P   over no mods: {:+.3} ms", mean - b),
        }
        rm.manager.shutdown();
    }
    crate::map::extension::use_real_ui(false);
}

/// What one Forge adapter call pays to read the snapshot: the cell hands out a copy of the whole struct.
#[test]
fn round4_snapshot_read_cost_micro() {
    big_stack(round4_snapshot_read_cost_micro_body)
}
fn round4_snapshot_read_cost_micro_body() {
    let cell = std::rc::Rc::new(std::cell::Cell::new(Snapshot::default()));
    let n = 20_000;
    let t = std::time::Instant::now();
    let mut acc = 0u64;
    for _ in 0..n {
        let snap = cell.get();
        acc += snap.abilities.ability_count as u64;
    }
    eprintln!("R4P Snapshot is {} bytes; Cell::get + one field: {:.2} us ({acc})", std::mem::size_of::<Snapshot>(), t.elapsed().as_secs_f64() * 1e6 / n as f64);
}
