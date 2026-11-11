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
