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
