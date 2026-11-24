// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Owner audit 2026-11-24 (the World Map review, C1): the client lets go of every claim when a world is left (a zone, a
//! teleport, a respawn, a disconnect), so a bundled mod that claimed once at load lost its claim for good. These run
//! the real lifecycle (`Core::reset`, then the new world's `world.entered` and `state.reset`) on the bundled mods and
//! look at what the client holds.
//!
//! The negative controls are the previous Lua of the mod (the tests read it from disk): with it, the same lifecycle
//! leaves the claim gone.
use std::{cell::RefCell, rc::Rc};
use super::{
    bridge::Bridge,
    discovery::{self, DiscoveredAddon},
    host::Host,
    native_core::{self, CORE, Core, View},
    health::HealthBarClaimsRegistry,
    native_events,
    visibility::{self, HudClaimsRegistry, VisibilityLease},
    window_sessions as ws,
    window_takeover::{self, TakeoverKind},
};

fn fresh() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    native_core::fixture_demand(true);
    super::action_broker::reset_all_for_test();
    ws::reset_for_test();
    super::native_a6::reset_for_test();
    window_takeover::reset_all_for_test();
    window_takeover::forget_switch_for_test();
    super::input::reset_input_state();
    crate::fork_ext::bindpage::reset_in_memory();
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        c.view.in_world = true;
        c.view.world_id = Some(1);
    });
}

fn found(id: &str) -> DiscoveredAddon {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods").canonicalize().unwrap();
    discovery::discover(&dir).addons.into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("{id} is bundled"))
}

struct Mod {
    host: Host,
    _bridge: Bridge,
}

fn start(id: &str) -> Mod {
    start_in(id, true)
}

/// A mod enabled in a world (`in_world`), or at the character screen.
fn start_in(id: &str, in_world: bool) -> Mod {
    CORE.with(|c| c.borrow_mut().view.in_world = in_world);
    let a = found(id);
    let host = Host::for_addon(&a).unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.run_addon(&a).unwrap();
    native_events::initial(&host).unwrap();
    native_events::dispatch(&host).unwrap();
    Mod { host, _bridge: bridge }
}

fn pump(mods: &[&Mod]) {
    native_core::flush_deferred();
    for m in mods {
        native_events::dispatch(&m.host).unwrap();
    }
}

/// The world is left as the client leaves it (`api_capture`: `Core::reset`), and the next one is entered (`publish`).
fn leave(reason: &str) {
    CORE.with(|c| c.borrow_mut().reset(reason));
}

fn enter(mods: &[&Mod]) {
    CORE.with(|c| c.borrow_mut().publish(View { in_world: true, phase: "world", world_id: Some(7), ..Default::default() }));
    pump(mods);
}

const REASONS: [&str; 3] = ["zone", "respawn", "disconnect"];

#[test]
fn start_menu_holds_the_start_key_again_in_every_new_world() {
    for reason in REASONS {
        fresh();
        let menu = start("eqoa_start_menu");
        assert!(window_takeover::is_taken(TakeoverKind::StartMenu), "{reason}: held at load");
        leave(reason);
        assert!(!window_takeover::is_taken(TakeoverKind::StartMenu), "{reason}: the client let go of it (the control that makes the next line mean something)");
        pump(&[&menu]);
        enter(&[&menu]);
        assert!(window_takeover::is_taken(TakeoverKind::StartMenu), "{reason}: held again in the new world; logs {:?}", menu.host.drain_logs());
        // and it is one claim, not one more each time
        leave(reason);
        pump(&[&menu]);
        enter(&[&menu]);
        assert!(window_takeover::is_taken(TakeoverKind::StartMenu), "{reason}: held again the second time");
    }
}

#[test]
fn chat_holds_the_chat_line_again_in_every_new_world() {
    for reason in REASONS {
        fresh();
        let chat = start("eqoa_chat");
        assert!(window_takeover::is_taken(TakeoverKind::ChatEntry), "{reason}: held at load");
        leave(reason);
        assert!(!window_takeover::is_taken(TakeoverKind::ChatEntry), "{reason}: the client let go of it");
        pump(&[&chat]);
        enter(&[&chat]);
        assert!(window_takeover::is_taken(TakeoverKind::ChatEntry), "{reason}: held again in the new world; logs {:?}", chat.host.drain_logs());
    }
}

#[test]
fn social_holds_the_requests_and_invites_again_in_every_new_world() {
    for reason in REASONS {
        fresh();
        let social = start("eqoa_social");
        let held = || window_takeover::is_taken(TakeoverKind::FriendRequest) && window_takeover::is_taken(TakeoverKind::GuildInvite);
        assert!(held(), "{reason}: held when it starts in a world");
        leave(reason);
        assert!(!held(), "{reason}: the client let go of them");
        pump(&[&social]);
        enter(&[&social]);
        assert!(held(), "{reason}: held again in the new world; logs {:?}", social.host.drain_logs());
    }
}

/// The Chat mod hides the game's chat while it is on. It hid it once, at load, and (a) failed when the mod was enabled at
/// the character screen (no world to hide in), (b) never hid it again after the game let go. Now it hides on each
/// `world.entered` and holds exactly one claim (no orphan an old token leaves behind).
#[test]
fn chat_hides_the_games_chat_in_every_world_with_one_claim() {
    for reason in REASONS {
        fresh();
        // enabled before any world: the character screen
        // as the manager makes a mod's host: with a lease on the game's HUD (the host `Host::for_addon` makes has none)
        CORE.with(|c| c.borrow_mut().view.in_world = false);
        let a = found("eqoa_chat");
        let host = Host::for_addon(&a).unwrap();
        host.snapshot.set(crate::play::Play::bundled_ui_fixture().forge_snapshot(None));
        let claims = Rc::new(RefCell::new(HudClaimsRegistry::new()));
        let lease = VisibilityLease::with_hud_claims(Rc::new(RefCell::new(HealthBarClaimsRegistry::new())), claims.clone()).unwrap();
        lease.update(host.snapshot.get().health, true);
        visibility::install_managed_health_adapter(&host, &lease).unwrap();
        let bridge = Bridge::install(&host).unwrap();
        CORE.with(|c| c.borrow_mut().view.in_world = false); // (the game's fixture above put a world up)
        host.run_addon(&a).unwrap();
        native_events::initial(&host).unwrap();
        native_events::dispatch(&host).unwrap();
        let chat = Mod { host, _bridge: bridge };
        // the manager brings the lease up to date at the start of each frame (`Manager::frame`): claims of an earlier world go
        let health = chat.host.snapshot.get().health;
        // (the next frame's update is what shows a claim in the registry)
        let hiding = || {
            lease.update(health, true);
            i64::from(claims.borrow().is_hidden("chat_window"))
        };
        // the frame's order: the game's events happen, the lease is brought up to date, then the mods hear (a mod's claim made
        // while hearing a new world's `world.entered` is of that world's number and stays)
        let step = |f: &dyn Fn()| {
            f();
            lease.update(health, true);
            pump(&[&chat]);
        };
        assert_eq!(hiding(), 0, "{reason}: nothing to hide before a world");
        step(&|| {
            CORE.with(|c| c.borrow_mut().publish(View { in_world: true, phase: "world", world_id: Some(7), ..Default::default() }));
        });
        assert_eq!(hiding(), 1, "{reason}: the game's chat is hidden once the world is entered; logs {:?}", chat.host.drain_logs());
        step(&|| leave(reason));
        assert_eq!(hiding(), 0, "{reason}: the client let go of the hide with the world (the control)");
        step(&|| {
            CORE.with(|c| c.borrow_mut().publish(View { in_world: true, phase: "world", world_id: Some(8), ..Default::default() }));
        });
        assert_eq!(hiding(), 1, "{reason}: and in the next world");
        // disabled: the one claim goes, with no orphan left behind by an earlier world's token
        native_events::lifecycle(&chat.host, "disabling").unwrap();
        assert_eq!(hiding(), 0, "{reason}: the game's chat is the game's again when the mod is off");
    }
}
