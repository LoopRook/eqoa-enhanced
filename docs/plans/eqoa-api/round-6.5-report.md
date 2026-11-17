# Round 6.5 Report: Capabilities, Standalone Window Takeover, `open_loot`/`talk`, and Bag Viewer Auto-Loot

Branch: `feat/eqoa-api-r6.5` from `develop` (`9046c28b`), reviewed footprint base `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Commits are LoopRook's; nothing was pushed, merged, or tagged.

## Commits

| Commit | Summary |
|---|---|
| `f6bbf3c6` | `feat(addons): drop one-gesture rule and enforce action capabilities` |
| `9e46ea78` | `feat(addons): add window takeover, open_loot, talk, and npc_flags` |
| `29d07f6a` | `feat(bag_viewer): standalone loot/merchant/bank takeover and auto-loot` |

## What Was Delivered

### 1. Drop One-Gesture Rule + Capability Enforcement & UI Labeling
- Removed per-call player-gesture gating (`needs_player_action`) across `action_broker.rs` and `native_state.rs`; gameplay actions and batches are now authorized by manifest capabilities (`eqoa.actions.*`, `eqoa.actions.<name>`, `eqoa.actions.batch`).
- Added `Host::requested_capabilities(&self)` (`host.rs`) and `Session.capabilities` / `has_capability` / `require_capability` (`native.rs`).
- Added `autonomous_action_label(caps: &[String])` and Mods & Addons manager detail lines (`runtime.rs`) so any mod declaring `eqoa.actions.*` or `eqoa.actions.<name>` displays `Actions: can act on its own (all actions)` or `Actions: can act on its own (<kinds>)`.

### 2. Window Takeover, `open_loot`, `talk`, and `Entity.npc_flags`
- **`crates/eqoa/src/fork_ext/addons/window_takeover.rs` & `native_r6a.rs`**:
  - Added `eqoa.windows.takeover(kind)`, `eqoa.windows.release(kind?)`, and `eqoa.windows.taken(kind)` for `"loot" | "merchant" | "bank"`.
  - Requires declared capability `eqoa.windows.takeover` (`permission_denied` otherwise).
  - Second holder returns `nil, Error{code="busy"}` and records a diagnostic naming the current holding addon.
  - Auto-releases on disable, reload, VM error (`release_vm`), and zoning (`release_all`), closing any hidden native session cleanly (`sync_hidden_sessions`).
- **`crates/eqoa/src/fork_ext/play.rs` & `items_capture.rs`**:
  - Added `window_taken`, `HiddenSessions`, `NoopRaster`, and `bank_taken_tick` so taken-over loot, merchant, and bank sessions keep native `mode == 0`, draw nothing, and steal no controls while routing `npcw` and `bankplay` actions (`0x1254`/`0x1255`).
  - Added native `0x0085` string-table error word lookup so server refusals expose readable reasons.
  - Exposed creature `npc_flags` (`npc_flags: uint?`, `nil` without a creature record) via `api_capture.rs` and `api_updates.rs`.
- **`crates/eqoa/src/fork_ext/addons/action_broker.rs`**:
  - Added `eqoa.actions.open_loot(target?)` (corpses only; nearest corpse via `ee` math when omitted; sends `0x0015`; correlates `0x0018` / `0x0085`).
  - Added `eqoa.actions.talk(target?, opts?)` (`kind = "merchant" | "bank" | "any"`; refuses corpses and NPCs missing the requested `npc_flags`; nearest matching NPC via `ee` math when omitted; sends `0x001E`; correlates `0x00B7` / `0x124D` / `0x0085`).

### 3. Upstream Seam Accounting (`round-6.5-seams.md`)
- Total upstream lines added: **4** inserted one-line hook calls across 2 files (`0` upstream lines deleted or rewritten; pre-approved ceiling 6), documented in `docs/plans/eqoa-api/round-6.5-seams.md` and `docs/fork-footprint.json`:
  - `crates/eqoa/src/npcw.rs` (2 lines): `0x0018` and `0x00B7` takeover mode suppression.
  - `crates/eqoa/src/bankplay.rs` (2 lines): `0x124D` takeover mode suppression and `run_bank` `bank_taken_tick` entry hook.

### 4. Bag Viewer (`bundled-mods/eqoa_bag_viewer`) & Auto-Loot
- Updated `mod.json` (declared `eqoa.windows.takeover`, `eqoa.entities.*`, `eqoa.player.get`, `eqoa.target.get`, `eqoa.options.*`), `modules/bags.lua`, `main.lua`, and `bundled-mods/SOURCE.txt`.
- Opening Bag Viewer takes over `"loot"`, `"merchant"`, and `"bank"` and releases on close (`"loot"` remains held when closed while **Auto-loot** is enabled), keeping native windows suppressed (`mode == 0`) while supporting **Loot**, **Take all**, **Take all but junk** (including Sandstorm per-take reopen), **Sell junk**, **Open bank**, **Deposit**, **Withdraw**, and visible server refusal messages (`0x0085`).
- Added an **Auto-loot** option (default OFF) that automatically opens and loots non-junk items from killed targets becoming nearby corpses without opening the native loot window.
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

## Gate & Verification Results (`EQOA_ISO=run/eqoa.iso`)

`python tools/run_gates.py --filter round65` (completed cleanly, **ALL GATES PASSED**):

| Step | Result | Details |
|---|---|---|
| `round` | PASS (194.97s) | 7 passed, 0 failed, 0 ignored (`round65` tests across `action_broker`, `runtime`, `items_tests`, `bag_viewer_tests`) |
| `addons` | PASS (440.27s) | 498 passed, 0 failed, 25 ignored (9 skips: Windows symlink privilege not held x4, `EQOA_FORGE_SPIKE` missing x2, `EQOA_BUNDLED_UI_GOLDENS` missing x3) |
| `full` | PASS (1949.53s) | 2381 passed, 0 failed, 30 ignored (30 distinct skips: `run/client.bin` missing, `EQOA_2002_ISO` missing, external `sandstorm-patch-20261015/S2` / `go-eqoa-server` patches missing, `1.6-savestates` missing, Windows symlink privilege not held, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`) |
| `wasm` | PASS (36.07s) | `cargo check --locked -p eqoa --target wasm32-unknown-unknown` |
| `footprint` | PASS (2.67s) | `python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` (`510 fork-owned, 42 shared`) |
| `paths` | PASS (6.49s) | `python tools/check_no_local_paths.py` |
| `player-release` | PASS (42.75s) | `python tools/test_player_release.py` |

Plus `python tools/update_bundled_manifest.py --check`: `Bundled mod manifests are up to date.`
Plus release build verification via `python tools/run_gates.py --release --skip-step addons,full` (verified after doc updates).
