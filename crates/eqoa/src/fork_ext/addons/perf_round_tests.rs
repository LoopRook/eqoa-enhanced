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
    /// of a quarter of them each frame, as the server's updates do (`entity.changed`).
    fn walk_creatures(&mut self, tick: usize) {
        let me = self.me;
        for i in 3..self.play.entities.slots.len() {
            let Some(e) = self.play.entities.slots[i].as_mut() else { continue };
            let a = i as f32 * 2.399_963 + tick as f32 * 0.01;
            let d = 20.0 + (i % 40) as f32 * 5.0;
            let at = [me[0] + a.cos() * d, me[1], me[2] + a.sin() * d];
            e.pos = at;
            e.actor_pos = at;
            if (tick + i) % 4 == 0 {
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
