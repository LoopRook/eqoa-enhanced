# Round 6.5 Report: Capabilities, Standalone Window Takeover, `open_loot`/`talk`, and Bag Viewer Auto-Loot

## Owner-test recovery, 2026-11-18 (base `20b3eebb`)

The earlier gate results below describe the build the owner tested, not acceptance
of this recovery. Live owner QA is pending. The interrupted builder's eight-file
uncommitted diff was preserved under ignored `run/`, reviewed, corrected, and
completed on `feat/eqoa-api-r6.5` as LoopRook. No push, merge, tag, disc data, or
server changes are part of this recovery.

### Changes and causes

1. **Hidden corpse reads:** the items snapshot already understands hidden sessions,
   but scope-own takeover depended on the global loot-source queue being untainted.
   Old unanswered requests could keep that queue ambiguous, so a mod's later open
   fell through to the native window. `window_takeover::PendingOpen` now observes
   the requests sent after arming, including their entity id. A conflicting request
   in that interval still declines takeover; ordinary player opens remain native.
   The separate source-name tracker stays conservative: ambiguous source names are
   not fabricated. The rendered Corpse tab and `eqoa.loot.get()` read the same items.
2. **Display versus taking:** the button is **Open nearest corpse**. Opening only
   displays the corpse; it leaves no take queued. Take all, Take all but junk, item
   menus, and the separate Auto-loot checkbox retain their existing taking paths.
   Existing round 5/6 take, refusal, Stop, and per-take server-close coverage stays.
   Button widths and padding fit the full labels at the default width; the
   rendered fixture lists three items and takes nothing.
3. **Merchant:** the previous API, spec, Lua discovery, and fixture metadata called
   bit `0x08` a merchant. The actual native interaction sends `0x004C` for bit
   `0x01`; bit `0x08` sends smith/repair `0x0061`. Discovery and takeover arming now
   use `0x01`, and the erroneous immediate "trainer completed" branch is removed.
   The existing `Play::interact_with_target` is called unchanged. The new test
   records exact outbound `0x004C + npc_id` bytes, skips a closer smith, and waits
   for matching `0x00B7` before success. Silence becomes "merchant did not answer"
   after the broker's existing 300 ticks (ten seconds), rather than leaving
   "Opening merchant..." indefinitely.
4. **Diagnostics:** always-on fork-side `takeover:` lines record dispatched mod
   open_loot/talk entity ids and opcodes, accepted/declined takeover decisions
   (including already-native windows), and hidden-session close reasons.
5. **Close once:** outgoing close packets and local `play: 0x0016` handler echoes
   are different. The old cleanup called `npcw_opcode(0x0016)` even after the server
   had already cleared the corpse, and again when releasing a holder with no
   session. This explains repeated log entries without proving repeated outgoing
   packets. Close obligations now carry whether a real session existed and why it
   ended; cleanup skips both the packet and native handler when already closed.
   An accepted distant corpse also stays readable: the old immediate 25-unit
   corpse-distance rejection is removed. Walking more than 25 units from the
   position where a hidden session opened still ends it, as does idle expiry.
6. **Bank:** unanswered opens and moves now end in the result line, mentioning
   the banker and possible distance. Silence does not establish a range refusal,
   so the wording is a question; actual refusal text continues unchanged through
   the existing result path. The server still decides range and permissions.

### Verification record

| Area | Implementation location |
|---|---|
| Request observation and hidden open decision | `crates/eqoa/src/fork_ext/addons/window_takeover.rs:362`, `:384` |
| Always-on decisions and closes | `crates/eqoa/src/fork_ext/play.rs:419`, `:545` |
| Hidden session lifetime | `crates/eqoa/src/fork_ext/play.rs:596` |
| Native merchant target and dispatch | `crates/eqoa/src/fork_ext/play.rs:1552` |
| Corpse display-only action | `bundled-mods/eqoa_bag_viewer/modules/bags.lua:512` |
| Merchant/banker timeout results | `bundled-mods/eqoa_bag_viewer/modules/bags.lua:1005`, `:1014` |

Always-on lines go through `window_takeover::log` (stderr, plus a test-only
buffer), so Rust tests assert the exact lines (request entity and opcode, accept,
decline with reason, close with reason). An earlier Python log checker inside the
crate was removed: it needed a built test executable and belonged in no test run.

Recovery note: the first builders were cut off with their implementation lost from
the tree by an interrupted negative control. It was restored from the saved stage
snapshots, reviewed hunk by hunk, and re-verified. The recovery commits are
`badf6702`, `1fb74cb6`, `5a720448`, `11bf2ccc`, `f5bc4bac`, `403ce60d` (one per fix
item, in that order: hidden corpse items, button label/display-only, merchant,
logs, one close per session + far corpse, bank/timeouts) plus the log-helper
commit that follows them.

### Executed negative controls (recovery)

Each fix was reverted in the working tree and the `round65` tests run; all 28 pass
with the fixes. With a fix reverted, these fail (first failing assertion):

| Fix reverted | Failing test |
|---|---|
| request-tracked claim (stale loot bookkeeping) | `round65_fix2_open_corpse_shows_items_hidden_even_with_stale_loot_requests`: "the game's corpse window must not open" (mode 1) |
| Open button takes (`loot_all`) | `round65_fix2_open_button_only_displays_never_takes`: result "Take all: queued..." not "Corpse ready" |
| merchant flag 0x01 to 0x08 | `round65_fix2_merchant_uses_native_flags_and_exact_request`: smith 0x0061 chosen, no 0x004C |
| 0x0016 sent even if the server closed | `round65_fix2_one_close_per_hidden_session`: 2 closes, expected 1 |
| close with no session | same test, part (c) |
| 25-unit corpse-distance check back | `round65_fix2_far_corpse_session_is_not_closed_at_once` |
| unanswered result text (merchant, bank open, bank move) | `round65_fix2_unanswered_merchant_talk_fails_visibly`, `round65_fix2_unanswered_bank_requests_fail_visibly` |
| each log line removed (open_loot, talk, accept, decline, close, release-without-session) | the open, merchant, M2 and one-close tests |

No upstream line was added or changed: **4 of the 6 permitted seam lines remain
in use** (see `round-6.5-seams.md`). All implementation stays in fork-owned files.
Bundled file checksums are regenerated with the mod changes.

The native player-initiated windows, live merchant response/range behavior, bank
range/refusal wording, and ownership permissions still need the owner's server
test. Protocol-shaped offline replays cannot establish live server acceptance.
The decomp/oracle is unavailable on this machine; no oracle result is claimed.

---

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
| `edf30939` | `fix(addons): takeover claims need a known matching target, owed closes, target restore, no hiding of open windows (M1 Rust, M2, M3, L4)` |
| `70b58da4` | `fix(bag_viewer): auto-loot only after you hit it and out of combat; Loot tab renews its session (M1, L5, L6)` |
| `6d73a1a8` | `fix(bag_viewer): declare exactly the actions the mod uses; spec and notes for the review fixes (L7)` |

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

## Review Fixes (Sonnet review: APPROVE WITH CHANGES, no HIGH)

Each fix has a test that failed before it. Rust fixes were written test-first and the failure below is that first red run; the Lua fixes were checked by running the new tests against the previous Lua (`EQOA_R5B_TEST_SOURCE` pointing at a copy of the old package). Items that share code are in one commit (M1 Rust with M2, M3, L4; M1 Lua with L5, L6). L4 needed no new upstream line: the total stays 4 of 6.

| Item | What changed | Test | Failure without the fix |
|---|---|---|---|
| M1 retarget | `open_loot`/`talk` record the player's previous target (`window_takeover::note_retarget`). When the mod is done (no pending open, no hidden session, nothing queued, in flight or running: `action_broker::vm_is_idle`) and no native window is up, the previous target returns through the game's own target request, unless the player chose another target or it is gone or a corpse. The spec documents the target change and the restore. | `round65_fix_m1_previous_target_restored_when_the_mods_open_is_done` | With the restore call disabled: `M1: the player's previous target comes back` left 9001, right 9500 (`items_tests.rs:3829`). |
| M1 combat | Auto-loot (Lua) does not run while in combat and goes on when the fight ends. The signal is the game's own combat flag (`player.fighting`, the one that asks for battle music), auto-attack on, or a live NPC whose `target_id` is the player (the best signal the client has: the wire has no "being attacked" flag beyond these). `player.changed` re-runs the held-back loot. | `round65_fix_bag_viewer_auto_loot_waits_while_in_combat` | Old Lua: `M1: no auto-loot while a live hostile has you targeted` left 1, right 0 (`bag_viewer_tests.rs:2583`). |
| M2 | Under scope own a pending open claims an open only when its target is known to match: loot and merchant by the observed id, bank by the player's current target. An unmatched open stays native and the pending claim survives for its own reply. A pending claim whose request is refused, timed out or gone is dropped at the next iteration (`action_broker::open_requests_waiting`). `scope = "all"` is unchanged. | `round65_fix_m2_pending_open_never_claims_a_player_started_open`, `round65_fix_m2_refused_open_clears_pending_claim` | `overlapping loot requests: the reply must not be claimed by the pending open` left 0, right 1 (`items_tests.rs:3655`); `a refused talk must not leave a pending claim on the player's own open` left 0, right 10 (`items_tests.rs:3712`). |
| M3 | `sync_sessions_with` records an owed close per kind (`close_owed`); it is taken and sent only when `mode == 0`, otherwise it stays (and keeps the takeover state active) until a native window no longer blocks it. Corpse, `LOOT_BUSY` and master-loot state are cleared when it is sent. | `round65_fix_m3_hidden_session_end_while_mode_nonzero_is_deferred_not_dropped` | `the owed close must keep the takeover state alive` (`items_tests.rs:3737`); before the fix the close was consumed and never sent. |
| L4 | `window_taken` declines when the window's mode was already current before the packet (`MODE_BEFORE_EVENT`, set in `begin_game_event`, cleared in `end_game_event`). | `round65_fix_l4_window_already_open_is_not_hidden_on_arrival` (loot, merchant, bank) | `L4: an already-open native corpse window must not be hidden by the reply` left 0, right 1 (`items_tests.rs:3770`). |
| L5 | `eqoa.windows.takeover(kind)` called again while holding renews a live hidden session's idle timer; the Loot tab calls it every frame it is shown. Walk-away, zoning, release and panel close still end the session. | `round65_fix_bag_viewer_loot_tab_keeps_hidden_session_alive` | Old Lua: `L5: the Loot tab must renew the session while it is shown` (`bag_viewer_tests.rs:2638`). |
| L6 | Auto-loot's "my kill" signal: you hit the creature (an outgoing damage `combat.hit` that is not your pet's, or your auto-attack on it while targeted). Not a player's or pet's corpse, not one nobody on your side touched, not another player's kill. Still uncertain (the wire carries no kill credit or loot rights): a creature you hit that another player then killed counts, and the server's refusal (shown in the result line) decides; manual **Loot** works on any corpse. | `round65_fix_bag_viewer_auto_loot_only_corpses_of_creatures_you_hit` | Old Lua: `a corpse of a creature you did not hit is not auto-looted` left 1, right 0 (`bag_viewer_tests.rs:2521`). |
| L7 | `mod.json` declares `eqoa.actions.open_loot, talk, loot, loot_all, sell, bank_move, use_item, equip, quick_slot, batch` instead of `eqoa.actions.*`, so the Mods & Addons label reads "can act on its own (open_loot, talk, loot, loot_all, sell, bank_move, use_item, equip, quick_slot, batch)". `buy` and `bank_money` are not called by the mod, so they are not declared (`cancel` and `stop_batch` ask for no capability). The bundled manifest was regenerated. | `round65_fix_bag_viewer_manifest_declares_exactly_the_actions_it_uses`; the label assertion in `round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal` | With the old `mod.json`: `no blanket eqoa.actions.*` (`bag_viewer_tests.rs:2671`). |

The three tests the review asked for: pending open beside a player-started open (`round65_fix_m2_pending_open_never_claims_a_player_started_open`), hidden-session end while `mode != 0` (`round65_fix_m3_...`), window already open on arrival (`round65_fix_l4_...`).

Changes to older tests, forced by the new rules and not weakening them: the bank step of `round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` no longer retargets before `0x124D` (an unmatched bank open is now native; that case is covered by the M2 test), `round65_loot_release_clears_corpse_visibility_and_master_loot` records its loot request so the reply is known to match, `activate_hidden_for_test` passes the player's target for banks, `round65_bag_viewer_auto_loot_option_takes_all_but_junk_on_killed_target` makes the kill a hit of yours, and the label assertion names the declared actions.

Needs the live server: the restore after a mod's open (the server's order of the retarget and the loot request), the owed close under a player's native window, the combat flag in a real fight, and Auto-loot after a real kill.

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

After the review fixes, `python tools/run_gates.py --release` (no `--filter`, no `--skip-step`): **ALL GATES PASSED**. The `enhanced_report_transport_does_not_follow_redirects` flake did not occur in this run.

| Step | Result | Details |
|---|---|---|
| `addons` | PASS (500.09s) | 503 passed, 0 failed, 25 ignored (9 distinct skips: Windows symlink privilege not held x4, `EQOA_FORGE_SPIKE` missing x2, `EQOA_BUNDLED_UI_GOLDENS` missing x3) |
| `full` | PASS (2026.22s) | 2395 passed, 0 failed, 30 ignored (30 distinct skips: `run/client.bin`, `EQOA_2002_ISO`, external Sandstorm/go-eqoa-server patch folders, `1.6-savestates`, Windows symlink privilege, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`) |
| `wasm` | PASS (21.05s) | `cargo check --locked -p eqoa --target wasm32-unknown-unknown` |
| `footprint` | PASS (2.30s) | `python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` (no budget change: 4 of 6 upstream lines) |
| `paths` | PASS (5.30s) | `python tools/check_no_local_paths.py` |
| `player-release` | PASS (42.03s) | `python tools/test_player_release.py` |
| `release` | PASS (178.34s) | Release binary built cleanly |

Also: `python tools/update_bundled_manifest.py --check` passes after the manifest regeneration. The oracle tests skip on this machine (the decomp and eqoa-port oracle are not here); the 2002-disc, live-S2 and pinned-Forge tests skip as listed above.
