// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A8: crafting for a mod without a running game: the capability gates, the argument checks, the two window
//! kinds, the Mods & Addons line, the record the read returns, and the bundled Crafting mod (its key, its windows,
//! what it asks the game for when clicked, and that no text leaves its window). The `Play`-driven parts (replies
//! read for the mod's window, the tray, one combine, the keep-this-recipe question) are in
//! `fork_ext/a8_play_tests.rs`.
//!
//! Each test names its negative control: what it would see if the change were not there.
use super::{
    action_broker,
    bridge::Bridge,
    discovery::{DiscoveredAddon, EntryMode},
    host::Host,
    native_a8,
    native_core::{self, CORE, Core},
    native_events, window_sessions as ws, window_takeover,
};
use dear_imgui_rs::Context;
use serde_json::json;
use std::path::PathBuf;

const ID: &str = "eqoa_crafting";

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    action_broker::reset_all_for_test();
    ws::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    native_a8::clear();
    crate::fork_ext::bindpage::reset_in_memory();
    CORE.with(|c| c.borrow_mut().view.in_world = true);
}

fn host(caps: &[&str]) -> Host {
    Host::with_test_capabilities(caps).unwrap()
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").join(ID).canonicalize().unwrap()
}

fn manifest() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("mod.json")).unwrap()).unwrap()
}

fn addon() -> DiscoveredAddon {
    DiscoveredAddon {
        id: ID.into(),
        name: ID.into(),
        entry: root().join("main.lua"),
        script_root: root(),
        package_root: root(),
        module_dirs: vec![root()],
        resource_dirs: vec![root()],
        api: Some("eqoa/1".into()),
        entry_mode: EntryMode::Once,
        capabilities_declared: true,
        requested_capabilities: manifest()["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect(),
        extensions: Default::default(),
        initially_enabled: false,
        mods_root: None,
    }
}

fn context() -> Context {
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([1024.0, 768.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

fn frames(host: &Host, bridge: &Bridge, context: &mut Context, n: usize) -> usize {
    let mut vertices = 0;
    for _ in 0..n {
        context.frame();
        bridge
            .invoke(|| {
                native_events::dispatch(host)?;
                native_events::render(host)
            })
            .unwrap();
        vertices = context.render_legacy().draw_data().total_vtx_count();
    }
    vertices
}

fn press(host: &Host, key: &str, combo: &str) {
    CORE.with(|c| {
        c.borrow_mut().emit("input.key", json!({"key": key, "combo": combo, "down": true, "repeat": false, "synthetic": false, "text": ""}))
    });
    native_events::dispatch(host).unwrap();
}

fn click(host: &Host, x: f64, y: f64) {
    CORE.with(|c| c.borrow_mut().emit("pointer.pressed", json!({"x": x, "y": y, "inside_window": true, "button": "left"})));
    native_events::dispatch(host).unwrap();
}

/// A state as `fork_ext/a8_play.rs` publishes it, with the longest names the game allows (64 characters).
fn state(question: bool) -> serde_json::Value {
    let long = "A".repeat(63);
    let words = "The quick brown fox jumps over the lazy dog. ".repeat(6);
    json!({
        "revision": 0,
        "pending": "none",
        "listed_skill": 3,
        "skills": (0..6).map(|i| json!({"id": i, "name": "Weaponsmithing", "level": 60, "rank": 12})).collect::<Vec<_>>(),
        "recipes": (0..12).map(|i| json!({
            "id": 100 + i, "name": long, "level": 60, "skill": 3, "component": 1, "component_name": long,
            "difficulty": 3, "difficulty_name": "Ultra Rare", "order": i,
        })).collect::<Vec<_>>(),
        "detail": {
            "recipe": 100, "name": long, "description": words, "rarity": 3, "count": 4, "level": 60,
            "ingredients": [
                {"name": long, "need": 2, "have": 99999},
                {"name": long, "need": 1, "have": 0},
                {"name": long, "need": 1, "have": 1},
            ],
            "tools": [{"name": long, "have": false}, {"name": long, "have": true}],
        },
        "tray": (0..4).map(|i| json!({"slot": i + 1, "id": 900 + i, "name": long})).collect::<Vec<_>>(),
        "items": (0..20).map(|i| json!({"slot": i + 1, "id": 900 + i, "name": long, "quantity": 99999, "in_tray": 4})).collect::<Vec<_>>(),
        "question": question.then(|| json!({"text": words, "recipe": 4242})),
        "notice": json!({"text": words, "seq": 1}),
    })
}

const READ: &str = "eqoa.craft.get";

#[test]
fn a8_every_name_needs_its_own_declaration_and_a_mod_without_it_gets_nothing() {
    fresh();
    native_a8::publish(state(false));
    let bare = host(&["eqoa.events.on"]);
    // the read
    bare.execute("local c, e = eqoa.craft.get(); assert(c == nil and (e.code == 'not_declared' or e.code == 'permission_denied'), e and e.code)").unwrap();
    // each action
    for name in native_a8::NAMES {
        bare.execute(&format!(
            "local r, e = eqoa.actions.{name}({{slot = 1, id = 1}}); assert(r == nil and e ~= nil and (e.code == 'not_declared' or e.code == 'permission_denied'), '{name}: ' .. tostring(e and e.code))"
        ))
        .unwrap();
    }
    // the control: declared, the same calls are accepted (or fail only on their arguments)
    let granted = host(&[READ, "eqoa.actions.craft_combine"]);
    granted.execute("local c = assert(eqoa.craft.get()); assert(c.pending == 'none')").unwrap();
    granted.execute("local r, e = eqoa.actions.craft_combine(); assert(r ~= nil and e == nil)").unwrap();
    // one action's grant is not another's
    granted.execute("local r, e = eqoa.actions.craft_keep(true); assert(r == nil and e.code == 'permission_denied', e and e.code)").unwrap();
    // the wildcard grants every action but not the read
    let wild = host(&["eqoa.actions.*"]);
    wild.execute("assert(eqoa.actions.craft_tray_clear() ~= nil)").unwrap();
    wild.execute("local c, e = eqoa.craft.get(); assert(c == nil and e.code == 'permission_denied')").unwrap();
}

#[test]
fn a8_arguments_are_checked_before_anything_is_queued() {
    fresh();
    let h = host(&["eqoa.actions.*"]);
    for (call, code) in [
        ("eqoa.actions.craft_list(6)", "invalid_argument"),
        ("eqoa.actions.craft_list(-1)", "invalid_argument"),
        ("eqoa.actions.craft_list('3')", "invalid_argument"),
        ("eqoa.actions.craft_list(1.5)", "invalid_argument"),
        ("eqoa.actions.craft_inspect(nil)", "invalid_argument"),
        ("eqoa.actions.craft_inspect(2^40)", "invalid_argument"),
        ("eqoa.actions.craft_tray_add(5)", "invalid_argument"),
        ("eqoa.actions.craft_tray_add({slot = 0, id = 3})", "invalid_argument"),
        ("eqoa.actions.craft_tray_add({slot = 1})", "invalid_argument"),
        ("eqoa.actions.craft_tray_remove({id = 1})", "invalid_argument"),
        ("eqoa.actions.craft_fill(7)", "invalid_argument"),
        ("eqoa.actions.craft_fill({5})", "invalid_argument"),
        ("eqoa.actions.craft_fill({{slot=1,id=1},{slot=1,id=1},{slot=1,id=1},{slot=1,id=1},{slot=1,id=1}})", "out_of_range"),
        ("eqoa.actions.craft_keep('yes')", "invalid_argument"),
        ("eqoa.actions.craft_keep()", "invalid_argument"),
        ("eqoa.actions.craft_combine(1)", "invalid_argument"),
        ("eqoa.actions.craft_tray_clear(1)", "invalid_argument"),
    ] {
        h.execute(&format!("local r, e = {call}; assert(r == nil and e ~= nil and e.code == '{code}', [=[{call}: ]=] .. tostring(e and e.code))")).unwrap();
    }
    assert!(action_broker::queued_kinds_for_test().is_empty(), "nothing was queued by a bad call");
    // the controls: well-formed calls queue, each under its own name
    h.execute(
        r#"
        assert(eqoa.actions.craft_list(5)); assert(eqoa.actions.craft_inspect(12))
        assert(eqoa.actions.craft_tray_add({slot = 2, id = 3})); assert(eqoa.actions.craft_tray_remove({slot = 2, id = 3}))
        assert(eqoa.actions.craft_fill({})); assert(eqoa.actions.craft_keep(false))
        "#,
    )
    .unwrap();
    assert_eq!(
        action_broker::queued_kinds_for_test(),
        vec!["craft_list", "craft_inspect", "craft_tray_add", "craft_tray_remove", "craft_fill", "craft_keep"]
    );
}

#[test]
fn a8_a_mod_may_open_the_two_crafting_windows_and_a_trainer_is_still_not_one() {
    fresh();
    let h = host(&["eqoa.windows.session", "eqoa.events.on"]);
    h.execute(
        r#"
        local a = assert(eqoa.windows.open_session('recipe_book', {opened_from = 'key'}))
        local b = assert(eqoa.windows.open_session('experiment'))
        assert(a.kind == 'recipe_book' and a.title == 'Recipe Book' and b.kind == 'experiment' and b.title == 'Experiment')
        assert(eqoa.windows.session('recipe_book').ref == a.ref)
        local t, e = eqoa.windows.open_session('trainer')
        assert(t == nil and e.code == 'unsupported', e and e.code)
        "#,
    )
    .unwrap();
    assert!(ws::any_open(&["recipe_book", "experiment"]));
    // closing both is what ends the client's reading of the server's crafting replies
    h.execute("assert(eqoa.windows.close_session(eqoa.windows.session('recipe_book').ref)); assert(eqoa.windows.close_session(eqoa.windows.session('experiment').ref))").unwrap();
    assert!(!ws::any_open(&["recipe_book", "experiment"]), "control: with both closed, none is open");
    // the player's switch closes a mod's crafting window like any other
    h.execute("assert(eqoa.windows.open_session('experiment'))").unwrap();
    assert!(ws::any_open(&["recipe_book", "experiment"]));
    window_takeover::set_use_game_windows(true);
    assert!(!ws::any_open(&["recipe_book", "experiment"]), "Use the game's windows ends it");
    window_takeover::set_use_game_windows(false);
}

#[test]
fn a8_mods_and_addons_says_what_crafting_can_do_and_nothing_for_a_mod_that_cannot() {
    let found = super::discovery::discover(root().parent().unwrap());
    let a = found.addons.into_iter().find(|a| a.id == ID).expect("the Crafting mod is bundled");
    let lines = super::runtime::disclosure_lines(&a);
    assert!(lines.contains(&"Provides: Crafting window (Shift+R)".to_string()), "{lines:?}");
    assert!(
        lines.contains(&"Crafting: can fill the crafting tray, combine what is in it, answer the keep-this-recipe question".to_string()),
        "{lines:?}"
    );
    // the actions are not also listed as acting on their own: they have their own, plainer line
    assert!(!lines.iter().any(|l| l.starts_with("Actions: can act on its own") && l.contains("craft")), "{lines:?}");
    // controls: reading and the harmless steps name nothing; a mod with no crafting names no crafting line
    let caps = |c: &[&str]| c.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(native_a8::crafting_label(&caps(&["eqoa.craft.get", "eqoa.actions.craft_list", "eqoa.actions.craft_inspect", "eqoa.actions.craft_tray_clear"])), None);
    assert_eq!(native_a8::crafting_label(&caps(&["eqoa.actions.craft_combine"])).as_deref(), Some("Crafting: can combine what is in it"));
    assert_eq!(native_a8::crafting_label(&caps(&["eqoa.actions.chat"])), None);
    assert_eq!(super::runtime::autonomous_action_label(&caps(&["eqoa.actions.craft_combine", "eqoa.actions.craft_fill"])), None);
    assert_eq!(
        super::runtime::autonomous_action_label(&caps(&["eqoa.actions.craft_combine", "eqoa.actions.chat"])).as_deref(),
        Some("Actions: can act on its own (chat (ordinary lines))")
    );
}

#[test]
fn a8_the_read_returns_the_declared_fields_and_changes_revision_only_when_the_state_does() {
    fresh();
    let h = host(&[READ]);
    h.execute("assert(eqoa.craft.get() == nil, 'no window, nothing to read')").unwrap();
    native_a8::publish(state(true));
    h.execute(
        r#"
        local c = assert(eqoa.craft.get())
        for _, f in ipairs({'revision','pending','listed_skill','skills','recipes','detail','tray','items','question','notice'}) do
            assert(c[f] ~= nil, f)
        end
        local r, d = c.recipes[1], c.detail
        for _, f in ipairs({'id','name','level','skill','component','component_name','difficulty','difficulty_name','order'}) do assert(r[f] ~= nil, f) end
        for _, f in ipairs({'recipe','name','description','rarity','count','level','ingredients','tools'}) do assert(d[f] ~= nil, f) end
        assert(d.ingredients[1].need == 2 and d.ingredients[1].have == 99999 and d.tools[1].have == false and d.tools[2].have == true)
        assert(c.tray[1].slot == 1 and c.items[1].in_tray == 4 and c.question.recipe == 4242 and c.notice.seq == 1)
        assert(c.skills[1].name == 'Weaponsmithing' and c.skills[1].level == 60)
        first = c.revision
        "#,
    )
    .unwrap();
    // the same state again: the same revision (the control for the bump below)
    native_a8::publish(state(true));
    h.execute("assert(eqoa.craft.get().revision == first)").unwrap();
    native_a8::publish(state(false));
    h.execute("local c = eqoa.craft.get(); assert(c.revision > first and c.question == nil)").unwrap();
    native_a8::clear();
    h.execute("assert(eqoa.craft.get() == nil)").unwrap();
}

// ---- the bundled mod ----

/// Every `eqoa.<module>.<name>` the script calls that needs a declaration is declared.
#[test]
fn a8_the_mod_ships_off_says_what_it_provides_and_declares_exactly_what_it_calls() {
    let m = manifest();
    assert_eq!(m["enabled"], false, "ships off");
    assert!(m["provides"].as_array().is_some_and(|p| p.len() == 1));
    let src = std::fs::read_to_string(root().join("main.lua")).unwrap();
    assert!(!src.contains('\r'), "bundled mod files keep LF line endings");
    assert!(!std::fs::read_to_string(root().join("README.md")).unwrap().contains('\r'));
    let declared: Vec<String> = m["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    let used = |prefix: &str| -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = src.as_str();
        while let Some(i) = rest.find(prefix) {
            rest = &rest[i + prefix.len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !found.contains(&name) {
                found.push(name);
            }
        }
        found
    };
    for name in used("eqoa.actions.") {
        assert!(declared.contains(&format!("eqoa.actions.{name}")), "calls eqoa.actions.{name} and must declare it");
    }
    for cap in declared.iter().filter(|c| c.starts_with("eqoa.actions.")) {
        let name = cap.trim_start_matches("eqoa.actions.");
        assert!(used("eqoa.actions.").iter().any(|u| u == name), "declares {cap} and never calls it");
        assert!(native_a8::NAMES.contains(&name), "{name} is one of the A8 names");
    }
    assert_eq!(used("eqoa.craft."), vec!["get"]);
    assert!(declared.contains(&"eqoa.craft.get".to_string()));
    for name in used("eqoa.windows.") {
        assert!(matches!(name.as_str(), "open_session" | "close_session"), "uses eqoa.windows.{name}");
    }
    assert!(declared.contains(&"eqoa.windows.session".to_string()));
    // no call that would act without a click: nothing but click and key handlers reaches an action. (A text check:
    // every `eqoa.actions.` call sits inside a `run("..", function() ... end)` the click handlers run.)
    for line in src.lines().filter(|l| l.contains("eqoa.actions.")) {
        assert!(line.contains("run(") || line.contains("return eqoa.actions."), "an action outside run(): {line}");
    }
    assert!(crate::fork_ext::bundled::PACKAGES.contains(&ID));
    for file in ["mod.json", "main.lua", "README.md"] {
        assert!(crate::fork_ext::bundled::EMBEDDED_FILES.iter().any(|f| f.path == format!("{ID}/{file}")), "{file} ships");
    }
}

#[test]
fn a8_shift_r_is_free_on_the_controls_page_and_a_plain_r_is_the_games() {
    use crate::fork_ext::bindpage;
    bindpage::reset_in_memory();
    bindpage::register(ID, "open", "shift+r", "Crafting");
    assert_eq!(bindpage::get_key(ID, "open").as_deref(), Some("shift+r"), "free");
    // the control: plain R is /reply's, so a mod default on it gets an empty row (this is why the key is Shift+R)
    bindpage::register("eqoa_x", "r", "r", "Plain R");
    assert_eq!(bindpage::get_key("eqoa_x", "r").as_deref(), Some(""));
    // no other bundled mod holds Shift+R
    let bundled = root().parent().unwrap().to_path_buf();
    for entry in std::fs::read_dir(bundled).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ID || !entry.path().join("main.lua").exists() {
            continue;
        }
        let text = std::fs::read_to_string(entry.path().join("main.lua")).unwrap_or_default();
        assert!(!text.contains("default_key = \"shift+r\""), "{name} also defaults to Shift+R");
    }
}

fn session_open() -> bool {
    ws::any_open(&["recipe_book", "experiment"])
}

#[test]
fn a8_the_mod_opens_on_shift_r_draws_and_closes_every_way_it_should() {
    fresh();
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    assert_eq!(crate::fork_ext::bindpage::get_key(ID, "open").as_deref(), Some("shift+r"));
    // control: shut, it draws nothing and holds no session
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0);
    assert!(!session_open());
    // the key opens it: a recipe_book session, a drawn window (before the client has read anything: "Opening...")
    press(&host, "r", "shift+r");
    assert!(session_open(), "opened; logs {:?}", host.drain_logs());
    let vm = native_events::bus(&host.lua).borrow().vm;
    assert!(ws::get(vm, "recipe_book").is_some() && ws::get(vm, "experiment").is_none());
    assert!(frames(&host, &bridge, &mut context, 3) > 0);
    assert!(host.drain_logs().unwrap().is_empty());
    // the key again closes it
    press(&host, "r", "shift+r");
    assert!(!session_open());
    assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "and draws nothing");
    // Escape closes it too
    press(&host, "r", "shift+r");
    press(&host, "escape", "escape");
    assert!(!session_open());
    // the client ends the session (zoning, death): the mod hears and closes
    for end in [
        Box::new(|| ws::set_env(ws::Env { away: true, ..ws::Env::default() })) as Box<dyn Fn()>,
        Box::new(|| ws::set_env(ws::Env { dead: true, ..ws::Env::default() })),
    ] {
        ws::set_env(ws::Env::default());
        press(&host, "r", "shift+r");
        assert!(session_open());
        end();
        frames(&host, &bridge, &mut context, 2);
        assert!(!session_open());
        assert_eq!(frames(&host, &bridge, &mut context, 2), 0, "stopped drawing when its session ended");
    }
    ws::set_env(ws::Env::default());
    // the player's switch: the key does nothing and says why
    window_takeover::set_use_game_windows(true);
    host.drain_logs().unwrap();
    press(&host, "r", "shift+r");
    let logs = format!("{:?}", host.drain_logs().unwrap());
    window_takeover::set_use_game_windows(false);
    assert!(!session_open() && logs.contains("did not open") && !logs.contains("invalid_argument"), "{logs}");
    // disabling closes everything
    press(&host, "r", "shift+r");
    assert!(session_open());
    native_events::clear(&host);
    assert!(!session_open());
}

/// A window of the mod on screen, its geometry from main.lua (HUD pixels).
struct Screen {
    k: f64,
    x0: f64,
    y0: f64,
}

const W: f64 = 360.0;
const H: f64 = 350.0;

fn screen(host: &Host) -> Screen {
    // as main.lua places the window: the frame's two scales differ on a screen that is not 640x448
    let (k, sx, sy, ox, oy): (f64, f64, f64, f64, f64) = host
        .lua
        .load("local f = eqoa.hud.frame(); return math.min(f.scale_x, f.scale_y), f.scale_x, f.scale_y, f.offset_x, f.offset_y")
        .eval()
        .unwrap();
    Screen { k, x0: (1024.0 - W * k).min(ox + (640.0 - W) / 2.0 * sx).max(0.0), y0: (768.0 - H * k).min(oy + (448.0 - H) / 2.0 * sy).max(0.0) }
}

#[test]
fn a8_clicks_ask_the_game_for_the_list_the_detail_the_tray_and_one_combine() {
    fresh();
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    press(&host, "r", "shift+r");
    native_a8::publish(state(false));
    frames(&host, &bridge, &mut context, 3);
    let s = screen(&host);
    let mut tap = |x: f64, y: f64| {
        click(&host, s.x0 + x * s.k, s.y0 + y * s.k);
        frames(&host, &bridge, &mut context, 2);
    };
    let kinds = || action_broker::queued_kinds_for_test();
    let sw = (W - 16.0) / 6.0;
    // a skill asks for its list
    tap(8.0 + 2.0 * sw + 5.0, 50.0);
    assert_eq!(kinds(), vec!["craft_list"]);
    // a recipe row asks for its detail (and is the one the window then shows)
    tap(60.0, 94.0);
    assert_eq!(kinds(), vec!["craft_list", "craft_inspect"]);
    // Fill tray, Combine, Empty tray are each one action; the tray in this state is full, so they are all enabled
    tap(30.0, 304.0);
    assert_eq!(kinds().last(), Some(&"craft_fill"));
    tap(100.0, 304.0);
    assert_eq!(kinds().last(), Some(&"craft_combine"));
    let n = kinds().len();
    tap(190.0, 304.0);
    assert_eq!(kinds().len(), n + 1);
    assert_eq!(kinds().last(), Some(&"craft_tray_clear"));
    // the Experiment tab lists the bags' components; a click on one adds it, a tray slot click removes it
    tap(8.0 + 84.0 + 20.0, 28.0);
    assert!(ws::get(native_events::bus(&host.lua).borrow().vm, "experiment").is_some(), "the new tab's window is open");
    assert!(ws::get(native_events::bus(&host.lua).borrow().vm, "recipe_book").is_none(), "and the old one closed");
    assert!(session_open(), "never a moment with neither");
    let n = kinds().len();
    tap(60.0, 46.0);
    assert_eq!(&kinds()[n..], ["craft_tray_add"]);
    tap(30.0, 276.0);
    assert_eq!(&kinds()[n..], ["craft_tray_add", "craft_tray_remove"]);
    assert!(host.drain_logs().unwrap().is_empty());
}

#[test]
fn a8_while_the_server_has_not_answered_the_buttons_that_would_send_are_not_there() {
    fresh();
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    press(&host, "r", "shift+r");
    let mut waiting = state(false);
    waiting["pending"] = json!("combine");
    native_a8::publish(waiting);
    frames(&host, &bridge, &mut context, 3);
    let s = screen(&host);
    for (x, y) in [(100.0, 304.0), (30.0, 304.0), (8.0 + 5.0, 50.0), (60.0, 94.0), (30.0, 276.0)] {
        click(&host, s.x0 + x * s.k, s.y0 + y * s.k);
        frames(&host, &bridge, &mut context, 1);
    }
    assert!(action_broker::queued_kinds_for_test().is_empty(), "nothing is asked while a request is out: {:?}", action_broker::queued_kinds_for_test());
    // the control: answered, the same clicks ask
    native_a8::publish(state(false));
    frames(&host, &bridge, &mut context, 3);
    click(&host, s.x0 + 100.0 * s.k, s.y0 + 304.0 * s.k);
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["craft_combine"]);
}

#[test]
fn a8_the_question_is_answered_by_its_two_buttons_and_nothing_behind_it_takes_a_click() {
    fresh();
    let mut context = context();
    let host = Host::for_addon(&addon()).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&addon()).unwrap();
    press(&host, "r", "shift+r");
    native_a8::publish(state(true));
    frames(&host, &bridge, &mut context, 3);
    let s = screen(&host);
    let mut tap = |x: f64, y: f64| {
        click(&host, s.x0 + x * s.k, s.y0 + y * s.k);
        frames(&host, &bridge, &mut context, 1);
    };
    // the buttons behind the question (Combine, a recipe, a tab, Fill) do nothing while it is up
    for (x, y) in [(100.0, 304.0), (30.0, 304.0), (60.0, 94.0), (8.0 + 84.0 + 20.0, 28.0), (8.0 + 5.0, 50.0)] {
        tap(x, y);
    }
    assert!(action_broker::queued_kinds_for_test().is_empty(), "{:?}", action_broker::queued_kinds_for_test());
    // Keep recipe and Leave it each answer once
    tap(40.0, 190.0);
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["craft_keep"]);
    tap(130.0, 190.0);
    assert_eq!(action_broker::queued_kinds_for_test(), vec!["craft_keep", "craft_keep"]);
    // no key answers it: Enter and Y do not reach it
    press(&host, "enter", "enter");
    press(&host, "y", "y");
    assert_eq!(action_broker::queued_kinds_for_test().len(), 2);
}

/// The window's own bounds: its first filled rectangle is the background; every vertex the mod draws (text included)
/// must lie within it, at the default size, with the longest names the game allows.
fn layer_vertices() -> Vec<[f32; 2]> {
    let name = std::ffi::CString::new("eqoa_crafting_layer").unwrap();
    let w = unsafe { dear_imgui_rs::sys::igFindWindowByName(name.as_ptr()) };
    assert!(!w.is_null(), "the mod's layer window is there");
    unsafe {
        let list = (*w).DrawList;
        let buf = &(*list).VtxBuffer;
        (0..buf.Size as usize).map(|i| {
            let v = &*buf.Data.add(i);
            [v.pos.x, v.pos.y]
        }).collect()
    }
}

#[test]
fn a8_no_text_leaves_the_window_at_the_default_size_with_the_longest_names() {
    for (tab_click, question) in [(None, false), (Some((8.0 + 84.0 + 20.0, 28.0)), false), (None, true)] {
        fresh();
        let mut context = context();
        let host = Host::for_addon(&addon()).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        host.run_addon(&addon()).unwrap();
        press(&host, "r", "shift+r");
        native_a8::publish(state(question));
        frames(&host, &bridge, &mut context, 3);
        let s = screen(&host);
        if let Some((x, y)) = tab_click {
            click(&host, s.x0 + x * s.k, s.y0 + y * s.k);
        }
        frames(&host, &bridge, &mut context, 3);
        let v = layer_vertices();
        assert!(v.len() > 100, "the window drew its text ({} vertices)", v.len());
        // the window's border is a one-pixel line centred on its edge: half a pixel each side, so a pixel of room
        let (left, top) = (s.x0 as f32 - 1.0, s.y0 as f32 - 1.0);
        let (right, bottom) = ((s.x0 + W * s.k) as f32 + 1.0, (s.y0 + H * s.k) as f32 + 1.0);
        let outside: Vec<_> = v.iter().filter(|p| p[0] < left || p[0] > right || p[1] < top || p[1] > bottom).collect();
        assert!(outside.is_empty(), "tab {tab_click:?} question {question}: {} vertices outside {left}..{right} x {top}..{bottom}: {:?}", outside.len(), &outside[..outside.len().min(4)]);
        // and the window fits the game's 640x448 frame
        assert!(W <= 640.0 && H <= 448.0);
    }
}
