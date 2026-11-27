// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
// Included by `manager.rs`'s test module (real manager, real mods, a GPU: it logs SKIP and returns without one).
fn conflict_mod(scripts: &Scratch, id: &str, name: &str) {
    let dir = scripts.0.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("mod.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": id, "name": name,
            "entry": "main.lua", "entryMode": "once", "enabled": true,
            "capabilities": ["eqoa.windows.takeover", "eqoa.events.on"],
            "replaces": ["text_dialog"]
        }))
        .unwrap(),
    )
    .unwrap();
    // asks once, as it starts; it never asks again by itself
    fs::write(dir.join("main.lua"), "eqoa.windows.takeover('text_dialog', {scope = 'all'})").unwrap();
}

#[test]
fn two_mods_one_window_turning_the_holder_off_gives_the_loser_the_window_at_once() {
    super::super::window_takeover::reset_all_for_test();
    let Some(mut gpu) = Gpu::new() else { return };
    let scripts = Scratch::new();
    let config = Scratch::new();
    conflict_mod(&scripts, "aa_holder", "EQOA Dialog Window");
    conflict_mod(&scripts, "bb_loser", "Quest Turn-in Helper");
    let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
    for _ in 0..4 {
        gpu.frame(&mut manager);
    }
    let name = |id: &str| manager.entries().iter().find(|e| e.metadata.id == id).map_or(id.to_string(), |e| e.metadata.name.clone());
    assert_eq!(
        super::super::window_takeover::holder_of(super::super::window_takeover::TakeoverKind::TextDialog).as_deref(),
        Some("aa_holder"),
        "the first mod holds the window"
    );
    assert_eq!(
        super::super::window_conflict::box_lines(&name),
        ["Dialog window: EQOA Dialog Window is using it, so Quest Turn-in Helper's is not shown."]
    );
    assert_eq!(super::super::window_conflict::unmet_lines("bb_loser", &name).len(), 1);
    // the player turns the holder off in Mods & Addons
    manager.queue(Command::Enable("aa_holder".into(), false));
    for _ in 0..4 {
        gpu.frame(&mut manager);
    }
    assert!(!super::super::window_conflict::is_open(), "the box is gone");
    assert!(super::super::window_conflict::unmet_lines("bb_loser", &|id| id.to_string()).is_empty(), "and the yellow line");
    assert_eq!(
        super::super::window_takeover::holder_of(super::super::window_takeover::TakeoverKind::TextDialog).as_deref(),
        Some("bb_loser"),
        "the other mod holds the window, with nobody asking it to"
    );
    assert!(manager.entries().iter().find(|e| e.metadata.id == "bb_loser").is_some_and(|e| e.enabled_intent), "no mod was turned off by the client");
    manager.shutdown();
}
