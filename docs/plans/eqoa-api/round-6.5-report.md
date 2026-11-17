# Round 6.5 Report: Capabilities, Standalone Window Takeover, `open_loot`/`talk`, and Bag Viewer Auto-Loot

Branch: `feat/eqoa-api-r6.5` from `develop` (`9046c28b`), reviewed footprint base `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Commits are LoopRook's; nothing was pushed, merged, or tagged.

## Commits

| Commit | Summary |
|---|---|
| `f6bbf3c6` | `feat(addons): drop one-gesture rule and enforce action capabilities` |
| `9e46ea78` | `feat(addons): add window takeover, open_loot, talk, and npc_flags` |
| `29d07f6a` | `feat(bag_viewer): standalone loot/merchant/bank takeover and auto-loot` |
| `e737a3de` | `docs(api): round 6.5 spec, getting started, seams, update notes, and report` |
| `b1ebec96` | `fix(addons): skip takeover TLS probes when idle and reset hidden sessions in tests (L4, L5)` |
| `0d625967` | `fix(addons): match npc_id when resolving talk server open (L2)` |
| `2465752a` | `fix(addons): clear corpse visibility and master-loot state on hidden loot release (L3)` |
| `a9b0c566` | `fix(api,addons): scope window takeover to mod-started sessions and release on stale (M1-M3)` |
| `06ea4c10` | `docs(api): fix talk/npc_flags spec and document takeover scoping and limits (L1, L6)` |
| `b3c11196` | `feat(api): add own/all takeover scope to eqoa.windows.takeover (M1 amendment)` |
| `01a48721` | `fix(addons): require explicit capability list for actions and takeover (not_declared)` |

## What Was Delivered

### 1. Drop One-Gesture Rule + Capability Enforcement & UI Labeling
- Removed per-call player-gesture gating (`needs_player_action`) across `action_broker.rs` and `native_state.rs`; gameplay actions (`eqoa.actions.*`) and `eqoa.windows.takeover` require explicit manifest capability declarations (`eqoa.actions.*`, `eqoa.actions.<name>`, `eqoa.actions.batch`, `eqoa.windows.takeover`).
- Mods without a `capabilities` list in `mod.json` (`DiscoveredAddon::declared_capabilities() == None`) or bare `Host::new()` sessions receive a visible `not_declared` refusal + diagnostic (while reads keep their current behavior); mods with a `capabilities` list lacking the capability receive `permission_denied`.
- Added `Host::requested_capabilities(&self)` (`host.rs`) and `Session.capabilities` / `capability_grants` / `require_capability` (`native.rs`).
- Derived `autonomous_action_label(caps)` and Mods & Addons manager detail lines (`runtime.rs`) from the same `declared_capabilities()` source (`Actions: can act on its own (all actions)` or `Actions: can act on its own (<kinds>)`).

### 2. Window Takeover (`scope = "own" | "all"`), `open_loot`, `talk`, and `Entity.npc_flags`
- **`crates/eqoa/src/fork_ext/addons/window_takeover.rs` & `native_r6a.rs`**:
  - Added `eqoa.windows.takeover(kind, opts?)` (`opts.scope` = `"own"` (default) or `"all"`), `eqoa.windows.release(kind?)`, and `eqoa.windows.taken(kind)` for `"loot" | "merchant" | "bank"`.
  - Requires declared capability `eqoa.windows.takeover` (`permission_denied` otherwise).
  - With `scope = "own"` (default), only sessions opened by the holding mod's own `open_loot`/`talk` (or batch corpse reopen) are hidden; player-started opens still show their normal native windows (`mode == 1 / 10 / 11`).
  - With `scope = "all"`, every session of that kind (including player-started opens) is hidden (`mode == 0`) for mods drawing a full replacement window (documented that such mods must draw their own UI and give the player a close control via `release`).
  - Hidden `scope = "own"` sessions store the request's `target_id` (M2: preserving banker ID across retargeting between `talk` and `0x124D`) and auto-end when the mod's action/batch completes, on walk-away (>25 units), or after a 15 s idle timeout (M3). Hidden `scope = "all"` sessions stay open across single transactions until released, walked away (>25 units), or zoned.
  - Zero-addon / idle fast path (`HAS_ACTIVITY` `Cell<bool>`, L4) performs zero TLS borrows on `window_taken`, `bank_taken_tick`, `sync_hidden_sessions`, and `*_session_open`; `reset_all_for_test` resets all hidden sessions and activity flags (L5).
  - Second holder returns `nil, Error{code="busy"}` and records a diagnostic naming the current holding addon.
  - Auto-releases on disable, reload, VM error (`release_vm`), and zoning (`release_all`), closing hidden native sessions cleanly (`close_hidden_session`, sending `0x0016` and clearing `loot.visible`, `loot.ml`, and master-loot state on loot release, L3).
- **`crates/eqoa/src/fork_ext/play.rs` & `items_capture.rs`**:
  - Added `window_taken`, `try_accept_open`, `sync_hidden_sessions`, `NoopRaster`, and `bank_taken_tick` so hidden loot, merchant, and bank sessions keep native `mode == 0`, draw nothing, and steal no controls while routing `npcw` and `bankplay` actions (`0x1254`/`0x1255`).
  - Added native `0x0085` string-table error word lookup so server refusals expose readable reasons.
  - Exposed creature `npc_flags` (`npc_flags: uint?`, `nil` without a creature record) via `api_capture.rs` and `api_updates.rs`.
- **`crates/eqoa/src/fork_ext/addons/action_broker.rs`**:
  - Added `eqoa.actions.open_loot(target?)` (corpses only; nearest corpse via `ee` math when omitted; sends `0x0015`; correlates `0x0018` / `0x0085`).
  - Added `eqoa.actions.talk(target?, opts?)` (`kind = "merchant" | "bank" | "any"`; refuses corpses and NPCs missing the requested `npc_flags`; nearest matching NPC via `ee` math when omitted; sends the native target-interaction opcode `0x004C`/`0x124D`/`0x0061`/`0x0004`; correlates `0x00B7` matching `npc_id` [L2], `0x124D`, or `0x0085`).
- **Known Limits (`EQOA-API-SPEC.md`)**:
  - (L3) On packet arrival, the upstream open sound cue plays before the fork takeover hook suppresses the window.
  - (L6) Generic window actions (`select_row`, `activate_row`, `dialog_answer`, `menu_command`) gate on `play.mode` and are refused on hidden/replaced windows (`mode == 0`); mods use direct actions (`loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`) instead.

### 3. Upstream Seam Accounting (`round-6.5-seams.md`)
- Total upstream lines added: **4** inserted one-line hook calls across 2 files (`0` upstream lines deleted or rewritten; pre-approved ceiling 6), documented in `docs/plans/eqoa-api/round-6.5-seams.md` and `docs/fork-footprint.json`:
  - `crates/eqoa/src/npcw.rs` (2 lines): `0x0018` and `0x00B7` takeover mode suppression.
  - `crates/eqoa/src/bankplay.rs` (2 lines): `0x124D` takeover mode suppression and `run_bank` `bank_taken_tick` entry hook.

### 4. Bag Viewer (`bundled-mods/eqoa_bag_viewer`) & Auto-Loot
- Updated `mod.json` (declared `eqoa.windows.takeover`, `eqoa.entities.*`, `eqoa.player.get`, `eqoa.target.get`, `eqoa.options.*`), `modules/bags.lua`, `main.lua`, and `bundled-mods/SOURCE.txt`.
- **Bag Viewer (`bundled-mods/eqoa_bag_viewer`)** uses `scope = "own"` only: instead of holding global takeovers while idle/open, it acquires takeover around its own actions/batches (**Loot**, **Take all**, **Take all but junk** including Sandstorm per-take reopen, **Sell junk**, **Open bank**, **Deposit**, **Withdraw**) and releases on batch/action completion, refusal, zoning, or panel close, leaving manual player clicks on corpses/merchants/bankers in their normal native windows.
- Added an **Auto-loot** option (default OFF) that automatically opens and loots non-junk items from killed targets becoming nearby corpses under `scope = "own"` without opening the native loot window.
- **Known Auto-Loot Limitation**: EQOA's native protocol does not transmit kill credit or corpse ownership flags to the client, so Auto-loot tracks the player's targeted living NPC and enables auto-loot only when that tracked entity dies/turns into a nearby corpse while Auto-loot is ON. Corpses left by other players or already-dead NPCs are skipped by Auto-loot (manual **Loot** still works on any nearby corpse).

### 5. Documentation
- Updated `docs/modding/EQOA-API-SPEC.md` (`Entity.npc_flags`, `eqoa.windows.takeover/release/taken`, `eqoa.actions.open_loot`, `eqoa.actions.talk`, capability enforcement replacing the one-gesture rule).
- Updated `docs/modding/GETTING-STARTED.md` (capability requirements in place of one-gesture wording).
- Updated `FORK-UPDATE-NOTES.md` (`## Unreleased` player-visible notes for autonomous action labels, standalone Bag Viewer takeover, and Auto-loot).

## Executed Negative Controls

Each fix was temporarily broken in source, tested against the focused regression test to observe failure, and cleanly restored:

1. **Capability gate (`eqoa.windows.takeover`)**:
   - **Fault**: Temporarily bypassed `require_capability(lua, "eqoa.windows.takeover", ...)` in `crates/eqoa/src/fork_ext/addons/native_r6a.rs`.
   - **Observed failure**: `cargo test --locked -p eqoa --bin eqoa round65 -- --test-threads=1` failed `play::fork_hooks::api_capture::items_tests::round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` at `crates/eqoa/src/fork_ext/items_tests.rs:2844:10` (`assertion failed` verifying undeclared capability denial).
2. **Native window takeover hook (`window_taken`)**:
   - **Fault**: Temporarily forced `window_taken` in `crates/eqoa/src/fork_ext/play.rs` to return `false`.
   - **Observed failure**: `cargo test --locked -p eqoa --bin eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:2879:5` (`assertion 'left == right' failed: taken-over loot must keep mode == 0; left: 1, right: 0`).
3. **Bag Viewer server refusal handling across dispatch phase (`bags.lua`)**:
   - **Fault**: Prior to filtering `d.phase ~= "dispatch"` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua` `M.on_action_result`, dispatching `open_loot` cleared `M.pending_open` before `0x0085` arrived.
   - **Observed failure**: `cargo test --locked -p eqoa --bin eqoa round65_bag_viewer -- --test-threads=1` failed `round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal` at `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs:2333:5` (`left: Some(true), right: Some(false)` instead of displaying server refusal text).
4. **L4 zero-addon zero-probe fast path (`window_takeover.rs`)**:
   - **Fault**: Forced `window_takeover::has_activity()` to return `true`.
   - **Observed failure**: `cargo test -p eqoa round65_zero_addon_zero_takeover_probes` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3172:5` (`left: 64, right: 0`).
5. **L5 test reset clears hidden sessions (`window_takeover.rs`)**:
   - **Fault**: Skipped clearing `hidden` in `window_takeover::reset_all_for_test()`.
   - **Observed failure**: `cargo test -p eqoa round65_reset_all_for_test_clears_hidden_sessions` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3202:5` (`assertion failed: !window_takeover::has_activity()`).
6. **L2 talk `0x00B7` `npc_id` correlation (`action_broker.rs`)**:
   - **Fault**: Bypassed `opened_npc_id` equality check in `resolve_server_talk_open`.
   - **Observed failure**: `cargo test -p eqoa round65_open_loot_talk_npc_flags_and_strict_target_guards` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3136:168` (`wrong npc_id must not confirm talk`).
7. **L3 hidden loot release cleanup (`play.rs`)**:
   - **Fault**: Bypassed `play.npcw_opcode(0x0016, &[])` cleanup in `close_hidden_session`.
   - **Observed failure**: `cargo test -p eqoa round65_loot_release_clears_corpse_visibility_and_master_loot` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3237:5` (`corpse must be cleared`).
8. **M1 `scope = "own"` does not hide player-started opens (`window_takeover.rs`)**:
   - **Fault**: Accepted opens without checking `pending_opens` under `scope = "own"`.
   - **Observed failure**: `cargo test -p eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:2895:5` (`player-started loot must open native window even under takeover; left: 0, right: 1`).
9. **M1 amendment `scope = "all"` hides player-started opens (`window_takeover.rs`)**:
   - **Fault**: Disabled the `holder_scope == TakeoverScope::All` branch in `try_accept_open`.
   - **Observed failure**: `cargo test -p eqoa round65_window_takeover_scopes_own_and_all` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3503:5` (`scope=all must hide player-opened loot window; left: 1, right: 0`).
10. **M2 bank target preservation across retarget (`window_takeover.rs`)**:
    - **Fault**: Stored `7777` instead of `pending.target_id` when accepting `Bank` hidden session.
    - **Observed failure**: `cargo test -p eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:2990:5` (`M2: hidden banker_id must come from talk request (9003), not retargeted play.target (7777); left: Some(7777), right: Some(9003)`).
11. **M3 walk-away release (`play.rs`)**:
    - **Fault**: Bypassed position distance checks in `sync_hidden_sessions`.
    - **Observed failure**: `cargo test -p eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3029:5` (`M3: walk-away must end hidden loot session`).
12. **Explicit `capabilities` list requirement (`native.rs`)**:
    - **Fault**: Restored `let Some(caps) = s.capabilities.as_ref() else { return Ok(()); };` in `require_capability`.
    - **Observed failure**: `cargo test -p eqoa round65_undeclared_capabilities_refuses_actions_and_takeover_with_not_declared` failed at `crates/eqoa/src/fork_ext/addons/action_broker.rs:4775:14` (`assist without capabilities list must refuse with not_declared`).

## Gate & Verification Results (`EQOA_ISO=run/eqoa.iso`)

`python tools/run_gates.py --release` (no `--filter`, no `--skip-step`) + focused verification:

| Step | Result | Details |
|---|---|---|
| `round65` | PASS | 12 passed, 0 failed, 0 ignored (`cargo test -p eqoa round65 -- --test-threads=1`) |
| `addons` | PASS (609.60s) | 499 passed, 0 failed, 25 ignored (9 distinct skips: Windows symlink privilege not held x4, `EQOA_FORGE_SPIKE` missing x2, `EQOA_BUNDLED_UI_GOLDENS` missing x3) |
| `full` | 2385 passed, 1 failed, 30 ignored (2239.74s) | Sole failure: unrelated pre-existing `fork_ext::report_core::tests::enhanced_report_transport_does_not_follow_redirects` (`WouldBlock` timeout under parallel test load; passed in 0.03s when run via `cargo test -p eqoa enhanced_report_transport_does_not_follow_redirects`). 30 distinct skips (`run/client.bin`, `EQOA_2002_ISO`, external `sandstorm-patch-20261015/S2` / `go-eqoa-server` patches, `1.6-savestates`, Windows symlink privilege, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`) |
| `wasm` | PASS (10.31s) | `cargo check --locked -p eqoa --target wasm32-unknown-unknown` |
| `footprint` | PASS (3.31s) | `python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` |
| `paths` | PASS (12.50s) | `python tools/check_no_local_paths.py` |
| `player-release` | PASS (66.60s) | `python tools/test_player_release.py` |
| `release` | PASS (283.56s) | Release binary built cleanly (`eqoa.exe`) |

Plus `python tools/update_bundled_manifest.py --check`: `Bundled mod manifests are up to date.`
