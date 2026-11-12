//! Bundled map mods through the real Lua host and Play/protocol capture.
use super::*;
use crate::fork_ext::addons::{
    discovery,
    host::Host,
    native_core::{CORE, Event},
    native_events,
    storage::Storage,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "eqoa-map-mod-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn journal() -> (Scratch, Host, PathBuf) {
    let scratch = Scratch::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods");
    let addon = discovery::discover(&root)
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_creature_journal")
        .expect("bundled creature journal must be discovered");
    assert!(addon.initially_enabled);
    let data = Storage::new(Some(scratch.0.join("config")))
        .data_dir(&addon)
        .unwrap()
        .unwrap();
    let host = Host::for_addon_with_files(&addon, data.clone()).unwrap();
    (scratch, host, data.join("files/data/journal.json"))
}
fn step(host: &Host, time: f64) {
    host.begin_iteration();
    native_events::dispatch(host).unwrap();
    let event = CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.seq += 1;
        c.frame += 1;
        Event {
            name: "render".into(),
            seq: c.seq,
            time,
            tick: c.tick,
            frame: c.frame,
            generation: c.generation,
            data: serde_json::json!({"dt":1.0/60.0}),
            bytes: 128,
            player_command: false,
            player_action: None,
        }
    });
    native_events::publish(&event);
    host.end_iteration();
}
fn creature(id: u32, name: &str, level: u8, pos: [f32; 3]) -> net::entity::Entity {
    net::entity::Entity {
        id,
        kind: net::entity::TYPE_CREATURE,
        world: 0,
        p18: 0,
        p19: 0,
        pos,
        actor_pos: pos,
        creature: Some(net::entity::Creature {
            name: name.into(),
            level,
            p255: 0,
            p256: 0,
            ..Default::default()
        }),
        ..Default::default()
    }
}
#[test]
fn round4b_journal_records_play_entities_with_native_filters_and_dedupe() {
    let mut p = Play::bundled_ui_fixture();
    p.session.self_id = Some(5000);
    p.pet_id = 91;
    let mut entities = vec![
        creature(10, "Bat", 15, [100., 5., 200.]),
        creature(11, "Bat", 15, [130., 80., 200.]),
        creature(12, "Bat", 15, [130.5, 5., 200.]),
        creature(13, "Bat", 16, [100., 5., 200.]),
        creature(14, "Bat", 15, [100., 5., 200.]),
    ];
    entities[4].world = 1;
    let mut e = creature(15, "Not creature", 15, [0.; 3]);
    e.kind = net::entity::TYPE_CREATURE.wrapping_add(1);
    entities.push(e);
    let mut e = creature(16, "Corpse", 15, [0.; 3]);
    e.p18 = 2;
    entities.push(e);
    let mut e = creature(17, "Other player", 15, [0.; 3]);
    e.creature.as_mut().unwrap().p256 = 2;
    entities.push(e);
    let mut e = creature(18, "Other pet", 15, [0.; 3]);
    e.creature.as_mut().unwrap().p256 = 3;
    entities.push(e);
    let mut e = creature(19, "Filtered", 15, [0.; 3]);
    e.creature.as_mut().unwrap().p255 = 7;
    entities.push(e);
    entities.extend([
        creature(20, "   ", 15, [0.; 3]),
        creature(5000, "Self", 15, [0.; 3]),
        creature(91, "Own pet", 15, [0.; 3]),
        creature(21, "Invalid position", 15, [f32::NAN; 3]),
        creature(22, "\u{2003}\u{85}", 15, [0.; 3]),
        creature(23, "\u{2003}Bat\u{a0}", 15, [100., 5., 200.]),
    ]);
    p.entities.slots = entities.into_iter().map(Some).collect();
    let (_scratch, host, path) = journal();
    p.forge_snapshot(None);
    let addon =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    host.run_addon(&addon).unwrap();
    for _ in 0..64 {
        step(&host, 0.5);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        !path.exists(),
        "journal must wait for quiet or disable before saving"
    );
    native_events::lifecycle(&host, "disabling").unwrap();
    native_events::clear(&host);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("journal flush missing: {e}; logs={:?}", host.drain_logs()));
    let saved: serde_json::Value = serde_json::from_str(&text).unwrap();
    let names = saved.as_object().unwrap();
    assert_eq!(names.len(), 1, "{saved}");
    let bats = saved["Bat"].as_object().unwrap();
    assert_eq!(bats.len(), 3, "{saved}");
    for id in ["10", "12", "13"] {
        assert!(bats.contains_key(id));
    }
    assert_eq!(
        saved["Bat"]["10"],
        serde_json::json!({"world":0,"level":15,"pos":[100,5,200]})
    );
    assert_eq!(saved["Bat"]["13"]["level"], 16);
    // Disabled VM is the negative control: a later valid packet cannot add data.
    let event = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let before = super::begin_game_event(&p, &event);
    p.entities
        .slots
        .push(Some(creature(99, "After disable", 20, [0.; 3])));
    super::end_game_event(&p, before);
    step(&host, 100.0);
    assert_eq!(fs::read_to_string(path).unwrap(), text);
}
#[test]
fn round4b_journal_reloads_large_file_imports_forge_and_exports_current_snapshot() {
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots = vec![Some(creature(
        90000,
        "New live creature",
        23,
        [10., 2., 30.],
    ))];
    let (scratch, host, path) = journal();
    let mut ids = serde_json::Map::new();
    for id in 10000..28000 {
        ids.insert(
            id.to_string(),
            serde_json::json!({"world":0,"level":19,"pos":[id as f64*40.0,12.5,1800.25]}),
        );
    }
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original =
        serde_json::to_vec(&serde_json::json!({"A creature with a long name": ids})).unwrap();
    assert!(original.len() > 1024 * 1024, "{}", original.len());
    fs::write(&path, &original).unwrap();
    let source = scratch.0.join("Forge.json");
    fs::write(&source, r#"{"A creature with a long name":{"10000":{"world":9,"level":99,"pos":[1,2,3]}},"Imported Bat":{"99999":{"world":1,"level":255,"pos":[12,13,14]},"99998":{"world":1,"level":256,"pos":[12,13,14]},"not-an-id":{"world":1,"level":10,"pos":[12,13,14]}},"Bad\u0085name":{"99997":{"level":1,"pos":[1,2,3]}}}"#).unwrap();
    let desktop = scratch.0.join("Desktop");
    host.test_player_files(Some(source), desktop.clone());
    p.forge_snapshot(None);
    let addon =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    host.run_addon(&addon).unwrap();
    for _ in 0..2200 {
        step(&host, 0.5);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        fs::read(&path).unwrap(),
        original,
        "load must not rewrite existing data before quiet"
    );
    let buttons = crate::fork_ext::addons::options::active_mod_items();
    let import = buttons
        .iter()
        .find(|(_, s)| s == "Import journal...")
        .unwrap()
        .0;
    let export = buttons
        .iter()
        .find(|(_, s)| s == "Export journal")
        .unwrap()
        .0;
    // Calling the same command without a physical activation cannot open a file.
    host.execute("assert(eqoa.mods.send('eqoa_creature_journal','import_journal'))")
        .unwrap();
    step(&host, 0.5);
    assert!(!path.parent().unwrap().join("imports").exists());
    assert!(crate::fork_ext::addons::options::activate_button(import));
    for _ in 0..200 {
        step(&host, 0.5);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(crate::fork_ext::addons::options::activate_button(export));
    let output = desktop.join("EQOA-Creature-Journal.json");
    for _ in 0..500 {
        step(&host, 0.5);
        if output.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let bytes =
        fs::read(output).unwrap_or_else(|e| panic!("export missing: {e}; {:?}", host.drain_logs()));
    assert!(bytes.len() > 1024 * 1024);
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        saved["A creature with a long name"]
            .as_object()
            .unwrap()
            .len(),
        18000
    );
    assert_eq!(
        saved["A creature with a long name"]["10000"]["level"], 19,
        "local record wins on import collision"
    );
    assert_eq!(saved["New live creature"]["90000"]["level"], 23);
    assert_eq!(saved["Imported Bat"].as_object().unwrap().len(), 1);
    assert_eq!(saved["Imported Bat"]["99999"]["level"], 255);
    assert!(saved.get("Bad\u{85}name").is_none());
    native_events::clear(&host);
}
#[test]
fn round4b_journal_saves_after_quiet_and_preserves_invalid_existing_json() {
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots = vec![Some(creature(700, "Quiet Bat", 3, [4., 5., 6.]))];
    p.forge_snapshot(None);
    let (_scratch, host, path) = journal();
    let addon =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    host.run_addon(&addon).unwrap();
    step(&host, 0.5);
    step(&host, 0.5);
    step(&host, 29.0);
    step(&host, 29.0);
    assert!(!path.exists());
    for _ in 0..100 {
        step(&host, 31.0);
        if path.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(path.exists(), "quiet save missing: {:?}", host.drain_logs());
    native_events::clear(&host);
    drop(host);
    fs::write(&path, b"{broken json").unwrap();
    let data = path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let host = Host::for_addon_with_files(&addon, data).unwrap();
    host.run_addon(&addon).unwrap();
    for _ in 0..100 {
        step(&host, 150.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    native_events::lifecycle(&host, "disabling").unwrap();
    native_events::clear(&host);
    assert_eq!(
        fs::read(&path).unwrap(),
        b"{broken json",
        "invalid original must survive load failure and disable"
    );
}
#[test]
fn round4b_journal_max_interval_flushes_during_continuous_discoveries() {
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots = vec![Some(creature(3000, "Continuous Bat", 8, [100., 2., 200.]))];
    p.forge_snapshot(None);
    let (_scratch, host, path) = journal();
    let addon =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    host.run_addon(&addon).unwrap();
    step(&host, 0.5);
    step(&host, 0.5);
    for i in 1..=6 {
        let event = net::session::Event::State {
            channel: i as u8,
            data: vec![],
        };
        let before = super::begin_game_event(&p, &event);
        p.entities.slots.push(Some(creature(
            3000 + i,
            "Continuous Bat",
            8,
            [100. + i as f32 * 40., 2., 200.],
        )));
        super::end_game_event(&p, before);
        step(&host, i as f64 * 20.);
        step(&host, i as f64 * 20.);
        if i < 6 {
            assert!(!path.exists(), "continuous changes must reset quiet timer");
        }
    }
    for _ in 0..100 {
        step(&host, 121.0);
        if path.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let saved: serde_json::Value = serde_json::from_slice(
        &fs::read(&path)
            .unwrap_or_else(|e| panic!("max interval missing: {e}; {:?}", host.drain_logs())),
    )
    .unwrap();
    assert_eq!(saved["Continuous Bat"].as_object().unwrap().len(), 7);
    native_events::clear(&host);
}
#[test]
fn round4b_text_editor_receives_keys_and_releases_held_gameplay() {
    use crate::fork_ext::addons::input as addon_input;
    use winit::keyboard::{Key, NamedKey};
    addon_input::reset_input_state();
    let mut p = Play::bundled_ui_fixture();
    p.forge_snapshot(None);
    let h = Host::new().unwrap();
    h.execute("texts='';arrows=0;eqoa.events.on('input.key',function(e) if e.data.down then if e.data.text then texts=texts..e.data.text end;if e.data.key=='up' then arrows=arrows+1 end end end);assert(eqoa.input.capture_text(true))").unwrap();
    p.aim_hold = true;
    p.info_up = true;
    super::begin_iteration(&mut p, false);
    assert!(
        !p.aim_hold && !p.info_up,
        "editor must release held camera and information state"
    );
    let mut report = crate::input::Report {
        buttons: u16::MAX,
        sticks: [1.; 4],
        triggers: [1.; 2],
    };
    crate::fork_ext::prepare_controls(&mut report, true, true);
    assert_eq!(report.buttons, 0);
    assert_eq!(report.sticks, [0.; 4]);
    for text in ["M", "P", "O", "G", "é"] {
        addon_input::route_mod_key(&Key::Character(text.into()), true, false, false, true, true);
    }
    addon_input::route_mod_key(
        &Key::Named(NamedKey::ArrowUp),
        true,
        true,
        false,
        true,
        true,
    );
    native_events::dispatch(&h).unwrap();
    h.execute("assert(texts=='MPOGé' and arrows==1)").unwrap();
    // Neither a native dialog nor a synthetic key is exposed as editor input.
    addon_input::route_mod_key(
        &Key::Character("secret".into()),
        true,
        false,
        false,
        false,
        true,
    );
    addon_input::route_mod_key(
        &Key::Character("synthetic".into()),
        true,
        false,
        true,
        true,
        true,
    );
    native_events::dispatch(&h).unwrap();
    h.execute("assert(texts=='MPOGé');eqoa.input.capture_text(false)")
        .unwrap();
    p.aim_hold = true;
    p.info_up = true;
    super::begin_iteration(&mut p, false);
    assert!(p.aim_hold && p.info_up);
    native_events::clear(&h);
    addon_input::reset_input_state();
}
fn search_host(scratch: &Scratch) -> (Host, discovery::DiscoveredAddon) {
    let addon =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_map_search")
            .expect("bundled map search");
    assert!(addon.initially_enabled);
    let data = Storage::new(Some(scratch.0.join("config")))
        .data_dir(&addon)
        .unwrap()
        .unwrap();
    (Host::for_addon_with_files(&addon, data).unwrap(), addon)
}
fn key(host: &Host, key: winit::keyboard::Key) {
    crate::fork_ext::addons::input::route_mod_key(&key, true, false, false, true, false);
    step(host, 0.5);
}
fn type_text(host: &Host, text: &str) {
    for c in text.chars() {
        key(host, winit::keyboard::Key::Character(c.to_string().into()));
    }
}
fn settle(host: &Host) {
    for _ in 0..80 {
        step(host, 0.5);
    }
}
#[test]
fn round4b_search_selects_live_with_dots_follows_and_falls_back_to_known() {
    use crate::fork_ext::addons::{guides, input};
    use winit::keyboard::{Key, NamedKey};
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    let mut bat = creature(6000, "MPOG Bat", 15, [200., 2., 300.]);
    bat.p19 = 255;
    let mut bat2 = creature(6001, "MPOG Bat", 16, [500., 2., 800.]);
    bat2.p19 = 255;
    p.entities.slots = vec![Some(bat), Some(bat2)];
    p.player.pos = [190., 2., 290.];
    p.forge_snapshot(None);
    let (scratch, journal, path) = journal();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path,r#"{"MPOG Bat":{"100":{"world":0,"level":15,"pos":[210,2,310]},"101":{"world":0,"level":15,"pos":[610,2,710]},"102":{"world":1,"level":16,"pos":[810,2,910]}}}"#).unwrap();
    // A running declared owner is required for the shared file.
    let ja =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    journal.run_addon(&ja).unwrap();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    for _ in 0..1500 {
        step(&journal, 0.5);
        step(&h, 0.5);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h.execute("local s=require('search');assert(#s.catalog>1000 and #s.journal==3)")
        .unwrap();
    key(&h, Key::Named(NamedKey::F2));
    type_text(&h, "MPOG Bat");
    settle(&h);
    h.execute("local s=require('search');assert(s.open and s.query=='MPOG Bat');assert(#s.rows==1 and s.rows[1].count==2 and s.rows[1].detail=='Spawned nearby: 2; levels 15-16')").unwrap();
    assert!(!p.map.open, "typing M/P/O/G must not open the native map");
    key(&h, Key::Named(NamedKey::Enter));
    let vm = native_events::bus(&h.lua).borrow().vm;
    assert_eq!(guides::target_for(vm), Some([200., 300.]));
    assert_eq!(guides::dots_for(vm).len(), 2);
    let packet = net::session::Event::State {
        channel: 0,
        data: vec![],
    };
    let before = super::begin_game_event(&p, &packet);
    p.entities.slots[0].as_mut().unwrap().pos = [250., 2., 350.];
    super::end_game_event(&p, before);
    step(&h, 0.5);
    assert_eq!(
        guides::target_for(vm),
        Some([250., 350.]),
        "live guide follows protocol movement"
    );
    for channel in 0..2 {
        let packet = net::session::Event::State {
            channel,
            data: vec![],
        };
        let before = super::begin_game_event(&p, &packet);
        p.entities.slots[channel as usize] = None;
        super::end_game_event(&p, before);
        step(&h, 0.5);
        settle(&h);
    }
    assert_eq!(
        guides::target_for(vm),
        Some([210., 310.]),
        "lost live entity falls back to nearest recorded spawn"
    );
    key(&h, Key::Named(NamedKey::F4));
    assert!(guides::target_for(vm).is_none());
    key(&h, Key::Named(NamedKey::F2));
    for _ in 0..8 {
        key(&h, Key::Named(NamedKey::Backspace));
    }
    type_text(&h, "level 15 odus");
    settle(&h);
    h.execute("local s=require('search');assert(#s.rows==1 and s.rows[1].name=='MPOG Bat')")
        .unwrap();
    key(&h, Key::Named(NamedKey::Enter));
    assert_eq!(guides::target_for(vm), Some([210., 310.]));
    key(&h, Key::Named(NamedKey::F4));
    key(&h, Key::Named(NamedKey::F2));
    for _ in 0..13 {
        key(&h, Key::Named(NamedKey::Backspace));
    }
    type_text(&h, "no such creature 123");
    settle(&h);
    key(&h, Key::Named(NamedKey::Enter));
    assert!(
        guides::target_for(vm).is_none(),
        "unmatched Enter cannot create a guide"
    );
    native_events::clear(&h);
    native_events::clear(&journal);
    input::reset_input_state();
}
#[test]
fn round4b_search_group_departure_places_and_unicode_editor_limit() {
    use crate::fork_ext::addons::{guides, input};
    use winit::keyboard::{Key, NamedKey};
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.group.in_group = true;
    p.group.members = vec![crate::group::Member {
        id: 77,
        name: "Group Friend".into(),
        active: true,
        x: 0,
        pos: [50., 2., 75.],
        ..Default::default()
    }];
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    key(&h, Key::Named(NamedKey::F2));
    key(&h, Key::Named(NamedKey::Tab));
    type_text(&h, "Group Friend");
    settle(&h);
    h.execute("local s=require('search');assert(s.tab==2 and #s.rows==1 and s.rows[1].detail=='Group position')").unwrap();
    key(&h, Key::Named(NamedKey::Enter));
    let vm = native_events::bus(&h.lua).borrow().vm;
    assert_eq!(guides::target_for(vm), Some([50., 75.]));
    let packet = net::session::Event::GroupDisbanded;
    let before = super::begin_game_event(&p, &packet);
    p.group.clear();
    super::end_game_event(&p, before);
    settle(&h);
    assert!(guides::target_for(vm).is_none());
    h.execute("assert(require('search').guide==nil)").unwrap();
    key(&h, Key::Named(NamedKey::F2));
    for _ in 0..12 {
        key(&h, Key::Named(NamedKey::Backspace));
    }
    key(&h, Key::Named(NamedKey::Tab));
    type_text(&h, "Saltwater");
    settle(&h);
    h.execute("local s=require('search');assert(s.tab==3 and #s.rows>0 and s.rows[1].name:find('(XP camp)',1,true))").unwrap();
    key(&h, Key::Named(NamedKey::Enter));
    assert!(guides::target_for(vm).is_some());
    key(&h, Key::Named(NamedKey::F4));
    assert!(guides::target_for(vm).is_none());
    key(&h, Key::Named(NamedKey::F2));
    for _ in 0..9 {
        key(&h, Key::Named(NamedKey::Backspace));
    }
    type_text(&h, &"é".repeat(81));
    h.execute("local s=require('search');assert(#s.query==160)")
        .unwrap();
    key(&h, Key::Named(NamedKey::Backspace));
    h.execute("assert(#require('search').query==158)").unwrap();
    key(&h, Key::Named(NamedKey::Escape));
    h.execute("assert(not require('search').open)").unwrap();
    assert!(!input::is_text_capture_active());
    p.map.open = true;
    p.forge_snapshot(None);
    h.execute("assert(eqoa.map.get().open)").unwrap();
    p.map.open = false;
    p.forge_snapshot(None);
    h.execute("assert(not eqoa.map.get().open)").unwrap();
    native_events::clear(&h);
    input::reset_input_state();
}

#[test]
fn round4b_journal_protects_existing_json_array_root() {
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots = vec![Some(creature(700, "Array Bat", 3, [4., 5., 6.]))];
    p.forge_snapshot(None);
    let (_scratch, host, path) = journal();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"[1,2,3]").unwrap();
    let addon =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    host.run_addon(&addon).unwrap();
    for _ in 0..100 {
        step(&host, 0.5);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    native_events::lifecycle(&host, "disabling").unwrap();
    native_events::clear(&host);
    assert_eq!(
        fs::read(path).unwrap(),
        b"[1,2,3]",
        "non-object root must be protected just like malformed JSON"
    );
}
#[test]
fn round4b_search_lists_inactive_group_member_as_unavailable() {
    use crate::fork_ext::addons::{guides, input};
    use winit::keyboard::{Key, NamedKey};
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.group.in_group = true;
    p.group.members = vec![crate::group::Member {
        id: 77,
        name: "Absent Friend".into(),
        active: false,
        ..Default::default()
    }];
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    key(&h, Key::Named(NamedKey::F2));
    key(&h, Key::Named(NamedKey::Tab));
    type_text(&h, "Absent Friend");
    settle(&h);
    h.execute("local s=require('search');assert(#s.rows==1 and s.rows[1].detail=='Position unavailable' and s.rows[1].at==nil)").unwrap();
    key(&h, Key::Named(NamedKey::Enter));
    assert_eq!(
        guides::guide_count(),
        0,
        "unknown group position must not become origin"
    );
    native_events::clear(&h);
    input::reset_input_state();
}

#[test]
fn round4b_search_f2_opens_map_with_minimap_off_and_obeys_map_master() {
    use crate::fork_ext::addons::{actions, input};
    use winit::keyboard::{Key, NamedKey};
    input::reset_input_state();
    actions::clear_actions();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    crate::settings::set_minimap(false).unwrap();
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    key(&h, Key::Named(NamedKey::F2));
    super::begin_iteration(&mut p, true);
    p.forge_snapshot(None);
    settle(&h);
    assert!(
        p.map.open,
        "F2 must reveal the M map when there is no minimap"
    );
    h.execute("assert(require('search').open)").unwrap();
    key(&h, Key::Named(NamedKey::Escape));
    p.map.close();
    let mut options = crate::settings::client();
    options.map = false;
    crate::settings::set_client(options);
    p.forge_snapshot(None);
    key(&h, Key::Named(NamedKey::F2));
    super::begin_iteration(&mut p, true);
    settle(&h);
    assert!(!p.map.open);
    h.execute("assert(not require('search').open);local ok,err=eqoa.map.open();assert(ok==nil and err.code=='unavailable')").unwrap();
    native_events::clear(&h);
    input::reset_input_state();
    actions::clear_actions();
    crate::settings::testing::reset(false);
}

#[test]
fn round4b_journal_caps_total_and_import_and_export_truncated_view() {
    use crate::fork_ext::addons::options;
    for (initial, n, limit) in [(true, 80001, 80000), (false, 50001, 50000)] {
        let mut p = Play::bundled_ui_fixture();
        p.entities.slots.clear();
        p.forge_snapshot(None);
        let (scratch, host, path) = journal();
        let ids = (100000..100000 + n)
            .map(|id| {
                (
                    id.to_string(),
                    serde_json::json!({"world":0,"level":2,"pos":[1,2,3]}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let bytes = serde_json::to_vec(&serde_json::json!({"Cap Creature":ids})).unwrap();
        let source = scratch.0.join("Forge.json");
        fs::write(&source, &bytes).unwrap();
        if initial {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &bytes).unwrap();
        }
        let desktop = scratch.0.join("Desktop");
        host.test_player_files(Some(source), desktop.clone());
        let a = discovery::discover(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"),
        )
        .addons
        .into_iter()
        .find(|a| a.id == "eqoa_creature_journal")
        .unwrap();
        host.run_addon(&a).unwrap();
        if !initial {
            let id = options::active_mod_items()
                .iter()
                .find(|(_, label)| label == "Import journal...")
                .unwrap()
                .0;
            assert!(options::activate_button(id));
        }
        for _ in 0..5000 {
            step(&host, 0.5);
            std::thread::sleep(std::time::Duration::from_millis(1));
            if !initial && path.exists() {
                break;
            }
        }
        let id = options::active_mod_items()
            .iter()
            .find(|(_, label)| label == "Export journal")
            .unwrap()
            .0;
        assert!(options::activate_button(id));
        let exported = desktop.join("EQOA-Creature-Journal.json");
        for _ in 0..500 {
            step(&host, 0.5);
            if exported.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let bytes = fs::read(&exported).unwrap_or_else(|e| {
            panic!(
                "capped export missing initial={initial}: {e}; {:?}",
                host.drain_logs()
            )
        });
        let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            saved["Cap Creature"].as_object().unwrap().len(),
            limit,
            "cap must reject exactly the extra valid spawn, initial={initial}"
        );
        native_events::clear(&host);
    }
}

#[test]
fn round4b_fix_moving_world_finishes_search_and_enter() {
    use crate::fork_ext::addons::{guides, input};
    use winit::keyboard::{Key, NamedKey};
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    let mut bat = creature(6000, "Moving test bat", 15, [200., 2., 300.]);
    bat.p19 = 255;
    p.entities.slots = vec![Some(bat)];
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    settle(&h);
    // Inject a catalogue large enough to require several bounded build slices.
    h.execute("local s=require('search'); local v={};for i=1,8000 do v['Fixture '..i]={world={0},pos={{i,0,i}}} end;s.load(v,'catalog')").unwrap();
    settle(&h);
    key(&h, Key::Named(NamedKey::F2));
    type_text(&h, "Moving test bat");
    key(&h, Key::Named(NamedKey::Enter));
    for i in 0..40 {
        let packet = net::session::Event::State {
            channel: 0,
            data: vec![],
        };
        let before = super::begin_game_event(&p, &packet);
        p.entities.slots[0].as_mut().unwrap().pos[0] = 250. + i as f32;
        super::end_game_event(&p, before);
        step(&h, 0.5);
    }
    h.execute("local s=require('search');assert(not s.open, 'moving entities must not restart a query build');assert(s.guide and s.guide.name=='Moving test bat')").unwrap();
    let vm = native_events::bus(&h.lua).borrow().vm;
    assert_eq!(guides::target_for(vm), Some([289., 300.]));
    native_events::clear(&h);
    input::reset_input_state();
}


#[test]
fn round4b_fix_journal_reports_early_export_refusal() {
    let mut p = Play::bundled_ui_fixture();
    p.forge_snapshot(None);
    let (scratch, h, path) = journal();
    let (search, search_addon) = search_host(&scratch);
    search.run_addon(&search_addon).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "[]").unwrap();
    let a =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    h.run_addon(&a).unwrap();
    for _ in 0..100 {
        step(&h, 1.);
        step(&search, 1.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let export = crate::fork_ext::addons::options::active_mod_items()
        .into_iter()
        .find(|(_, s)| s == "Export journal")
        .unwrap()
        .0;
    assert!(crate::fork_ext::addons::options::activate_button(export));
    for _ in 0..3 {
        step(&h, 1.);
        step(&search, 1.);
    }
    search.execute("local s=require('search');assert(s.message and s.message:find('Export:',1,true) and s.message:find('protect',1,true),tostring(s.message))").unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "[]");
    native_events::clear(&h);
    native_events::clear(&search);
}

#[test]
fn round4b_fix_journal_records_actor_position_and_protocol_fallback() {
    let mut p = Play::bundled_ui_fixture();
    let mut a = creature(6000, "Actor bat", 15, [10., 0., 20.]);
    a.actor_pos = [110., 5., 120.];
    let mut b = creature(6001, "Fallback bat", 15, [210., 0., 220.]);
    b.actor_pos = [0.; 3];
    p.entities.slots = vec![Some(a), Some(b)];
    p.forge_snapshot(None);
    let (_scratch, h, path) = journal();
    let a =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    h.run_addon(&a).unwrap();
    for _ in 0..100 {
        step(&h, 1.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    native_events::lifecycle(&h, "disabling").unwrap();
    native_events::clear(&h);
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        saved["Actor bat"]["6000"]["pos"],
        serde_json::json!([110, 5, 120])
    );
    assert_eq!(
        saved["Fallback bat"]["6001"]["pos"],
        serde_json::json!([210, 0, 220])
    );
}

#[test]
fn round4b_fix_save_deadline_restarts_after_snapshot() {
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots.clear();
    p.forge_snapshot(None);
    let (_scratch, h, path) = journal();
    let a =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    h.run_addon(&a).unwrap();
    for _ in 0..80 {
        step(&h, 1.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let packet = net::session::Event::State {
        channel: 0,
        data: vec![],
    };
    let before = super::begin_game_event(&p, &packet);
    p.entities.slots.push(Some(creature(
        6000,
        "Before snapshot",
        15,
        [100., 0., 100.],
    )));
    super::end_game_event(&p, before);
    step(&h, 2.);
    step(&h, 121.); // maximum dirty interval snapshots revision 1
    let packet = net::session::Event::State {
        channel: 1,
        data: vec![],
    };
    let before = super::begin_game_event(&p, &packet);
    p.entities
        .slots
        .push(Some(creature(6001, "After snapshot", 15, [200., 0., 200.])));
    super::end_game_event(&p, before);
    for _ in 0..150 {
        step(&h, 122.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(saved.get("Before snapshot").is_some());
    assert!(
        saved.get("After snapshot").is_none(),
        "new sightings must start a new 120-second window, not immediately resave"
    );
    for _ in 0..150 {
        step(&h, 152.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(
        saved.get("After snapshot").is_some(),
        "quiet deadline still flushes later changes"
    );
    native_events::clear(&h);
}

#[test]
fn round4b_fix_known_guide_upgrades_and_waypoint_tracks_changes() {
    use crate::fork_ext::addons::{guides, input, native_core};
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots.clear();
    p.map.open = true;
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    settle(&h);
    h.execute("local s=require('search');s.load({['Upgrade bat']={world={0},pos={{100,0,200}}}},'catalog')").unwrap();
    settle(&h);
    h.execute("local s=require('search');s.toggle();s.query='Upgrade bat';s.dirty=true")
        .unwrap();
    settle(&h);
    key(
        &h,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter),
    );
    let vm = native_events::bus(&h.lua).borrow().vm;
    assert_eq!(guides::target_for(vm), Some([100., 200.]));
    let packet = net::session::Event::State {
        channel: 0,
        data: vec![],
    };
    let before = super::begin_game_event(&p, &packet);
    let mut e = creature(6010, "Upgrade bat", 15, [700., 0., 800.]);
    e.p19 = 255;
    p.entities.slots = vec![Some(e)];
    super::end_game_event(&p, before);
    p.forge_snapshot(None);
    step(&h, 1.);
    step(&h, 1.);
    assert_eq!(
        guides::target_for(vm),
        Some([700., 800.]),
        "known guide must upgrade when its creature appears"
    );
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.map["waypoint"] = serde_json::json!({"world_id":0,"x":300,"z":400});
        c.emit("map.changed", serde_json::json!({}));
    });
    step(&h, 1.);
    step(&h, 1.);
    h.execute("local s=require('search');s.choose({name='Personal waypoint',kind='waypoint',at={x=300,z=400}})").unwrap();
    assert_eq!(guides::target_for(vm), Some([300., 400.]));
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.map["waypoint"] = serde_json::json!({"world_id":0,"x":900,"z":950});
        c.emit("map.changed", serde_json::json!({}));
    });
    step(&h, 1.);
    step(&h, 1.);
    assert_eq!(guides::target_for(vm), Some([900., 950.]));
    native_core::CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.map["waypoint"] = serde_json::Value::Null;
        c.emit("map.changed", serde_json::json!({}));
    });
    step(&h, 1.);
    step(&h, 1.);
    assert_eq!(
        guides::target_for(vm),
        None,
        "deleted waypoint must stop guidance"
    );
    h.execute("require('search').choose({name='Upgrade bat',kind='npc',key='upgrade bat',at={x=700,z=800}})").unwrap();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.world_id = Some(1);
        c.view.map["world_id"] = serde_json::json!(1);
        c.emit("map.changed", serde_json::json!({}));
    });
    step(&h, 1.);
    step(&h, 1.);
    h.execute("assert(require('search').status()=='Unavailable here: Upgrade bat')")
        .unwrap();
    CORE.with(|c| c.borrow_mut().view.in_world = false);
    h.execute("local s=require('search');s.choose({name='Rejected guide',kind='place',at={x=10,z=20}});assert(s.guide==nil,'failed set_guide must leave no guide')").unwrap();
    assert_eq!(guides::target_for(vm), None);
    native_events::clear(&h);
    input::reset_input_state();
}

#[test]
fn round4b_fix_closed_search_defers_observation_and_shared_reload() {
    use crate::fork_ext::addons::input;
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    settle(&h);
    h.execute("local s=require('search');assert(not s.open);s.live={};s.refresh=true;s.observe();assert(s.refresh and #s.live==0,'closed observe must do nothing')").unwrap();
    native_events::clear(&h);
    input::reset_input_state();
}

#[test]
fn round4b_fix_busy_world_max_journal_cost_and_incremental_queries() {
    use crate::fork_ext::addons::input;
    use std::time::{Duration, Instant};
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.entities.slots = (0..200)
        .map(|i| {
            let mut e = creature(
                700000 + i,
                &format!("Moving fixture {i}"),
                15,
                [i as f32 * 40., 0., 100.],
            );
            e.p19 = 255;
            Some(e)
        })
        .collect();
    p.forge_snapshot(None);
    let (scratch, j, path) = journal();
    let mut value = serde_json::Map::new();
    for name in 0..10000 {
        let mut ids = serde_json::Map::new();
        for n in 0..8 {
            let id = name * 8 + n;
            ids.insert(
                id.to_string(),
                serde_json::json!({"world":0,"level":15,"pos":[id*40,0,100]}),
            );
        }
        value.insert(
            format!("Perf creature {name:05}"),
            serde_json::Value::Object(ids),
        );
    }
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, &bytes).unwrap();
    let ja =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    j.run_addon(&ja).unwrap();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut loaded = false;
    while Instant::now() < deadline {
        step(&j, 1.);
        step(&h, 1.);
        loaded=h.lua.load("local s=require('search');return #s.journal==80000 and #s.catalog>1000 and #s.jobs==0").eval::<bool>().unwrap();
        if loaded {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(loaded, "80,000-spawn workload did not load");
    h.execute("assert(#eqoa.entities.list()==200);local s=require('search');assert(s.by_name.journal['perf creature 00000'] and #s.by_name.journal['perf creature 00000']==8)").unwrap();
    let mut closed = Vec::new();
    let mut open = Vec::new();
    let mut minimum = u32::MAX;
    for phase in 0..2 {
        if phase == 1 {
            key(
                &h,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2),
            );
            for _ in 0..160 {
                step(&h, 1.);
                minimum = minimum.min(h.instructions_remaining());
            }
            h.execute("local s=require('search');assert(s.open and not s.build and #s.npc_cache>=10000);cached_rows=s.npc_cache").unwrap();
        }
        for frame in 0..120 {
            for e in p.entities.slots.iter_mut().flatten() {
                e.pos[0] += 1.;
                e.actor_pos = e.pos;
            }
            p.forge_snapshot(None);
            if phase == 1 && frame % 8 == 0 {
                let c = "Perf creature 0".chars().nth(frame / 8).unwrap();
                crate::fork_ext::addons::input::route_mod_key(
                    &winit::keyboard::Key::Character(c.to_string().into()),
                    true,
                    false,
                    false,
                    true,
                    false,
                );
            }
            let start = Instant::now();
            step(&j, 1.);
            step(&h, 1.);
            let ms = start.elapsed().as_secs_f64() * 1000.;
            minimum = minimum.min(h.instructions_remaining());
            if phase == 0 {
                closed.push(ms)
            } else {
                open.push(ms)
            }
        }
        if phase == 0 {
            h.execute("local s=require('search');assert(not s.open and #s.live==0 and s.refresh)")
                .unwrap();
        } else {
            for _ in 0..60 {
                step(&h, 1.);
            }
            h.execute("local s=require('search');assert(s.npc_cache==cached_rows and not s.build and #s.rows==10000 and #s.live==200);assert(s.query=='Perf creature 0')").unwrap();
        }
    }
    for (name, mut values) in [("closed", closed), ("open typing", open)] {
        values.sort_by(f64::total_cmp);
        eprintln!(
            "R4 B2 {} {}: 200 moving entities, 80000 spawns / 10000 names / {} bytes, 120 frames mean={:.3}ms p95={:.3}ms max={:.3}ms; minimum instructions remaining={minimum}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            name,
            bytes.len(),
            values.iter().sum::<f64>() / 120.,
            values[114],
            values[119]
        );
    }
    assert!(
        minimum > 800000,
        "busy callbacks must retain at least 20% of the fatal instruction budget: {minimum}"
    );
    native_events::clear(&h);
    native_events::clear(&j);
    input::reset_input_state();
}

#[test]
fn round4b_fix_shared_reload_is_lazy_and_retries_late_journal_owner() {
    use crate::fork_ext::addons::input;
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.map.open = true;
    p.forge_snapshot(None);
    let (scratch, j, path) = journal();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    for _ in 0..10 {
        step(&h, 1.);
    }
    h.execute("assert(#require('search').journal==0)").unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{"Lazy bat":{"1":{"world":0,"level":15,"pos":[100,0,200]}}}"#,
    )
    .unwrap();
    let ja =
        discovery::discover(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods"))
            .addons
            .into_iter()
            .find(|a| a.id == "eqoa_creature_journal")
            .unwrap();
    j.run_addon(&ja).unwrap();
    key(
        &h,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        step(&j, 3.);
        step(&h, 3.);
        if h.lua
            .load("return #require('search').journal==1")
            .eval::<bool>()
            .unwrap()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h.execute("assert(#require('search').journal==1,'opening must retry a late shared owner')")
        .unwrap();
    key(
        &h,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape),
    );
    h.execute("shared_seen=0;eqoa.events.on('shared.changed',function(e) if e.data.mod=='eqoa_creature_journal' then shared_seen=shared_seen+1 end end)").unwrap();
    j.execute("assert(eqoa.files.save_json_async('journal.json',{['Lazy bat']={['1']={world=0,level=15,pos={100,0,200}},['2']={world=0,level=15,pos={500,0,600}}}}))").unwrap();
    for _ in 0..250 {
        step(&j, 4.);
        step(&h, 4.);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h.execute("local s=require('search');assert(shared_seen>0,'shared save workload did not run');assert(not s.open and #s.journal==1,'closed shared change must not reload the journal')").unwrap();
    key(
        &h,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        step(&j, 5.);
        step(&h, 5.);
        if h.lua
            .load("return #require('search').journal==2")
            .eval::<bool>()
            .unwrap()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h.execute(
        "assert(#require('search').journal==2,'next open must load the new shared snapshot')",
    )
    .unwrap();
    native_events::clear(&h);
    native_events::clear(&j);
    input::reset_input_state();
}

#[test]
fn round4b_fix_wheel_is_local_and_steps_whole_rows() {
    use crate::fork_ext::addons::input;
    input::reset_input_state();
    crate::settings::testing::reset(false);
    let mut p = Play::bundled_ui_fixture();
    p.map.open = true;
    p.forge_snapshot(None);
    let scratch = Scratch::new();
    let (h, a) = search_host(&scratch);
    h.run_addon(&a).unwrap();
    settle(&h);
    // Drain the previously queued render; these deltas occur between draw frames.
    native_events::dispatch(&h).unwrap();
    h.execute("local s=require('search');s.open=true;s.dirty=false;s.build=nil;s.selected=8;s.offset=3;s.list_rect={x=10,y=100,w=300,h=200};s.rows={};for i=1,20 do s.rows[i]={name='Row '..i,detail='',kind='place',at={x=i,z=i}} end").unwrap();
    CORE.with(|c| {
        c.borrow_mut().emit(
            "pointer.wheel",
            serde_json::json!({"x":900,"y":700,"steps":0.3,"inside_window":true}),
        )
    });
    h.begin_iteration();
    native_events::dispatch(&h).unwrap();
    h.end_iteration();
    h.execute(
        "assert(require('search').selected==8,'wheel outside the list must leave selection alone')",
    )
    .unwrap();
    for n in [0.3, -0.3, -1.7, -0.2, -0.8] {
        h.execute("require('search').list_rect={x=10,y=100,w=300,h=200}")
            .unwrap();
        CORE.with(|c| {
            c.borrow_mut().emit(
                "pointer.wheel",
                serde_json::json!({"x":20,"y":110,"steps":n,"inside_window":true}),
            )
        });
        h.begin_iteration();
        native_events::dispatch(&h).unwrap();
        h.end_iteration();
        h.execute("local s=require('search');assert(s.selected%1==0 and s.offset%1==0,'row indices must be integers')").unwrap();
    }
    h.execute("assert(require('search').selected==11)").unwrap();
    native_events::clear(&h);
    input::reset_input_state();
}
