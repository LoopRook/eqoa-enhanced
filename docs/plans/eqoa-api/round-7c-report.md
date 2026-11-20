# Round 7 Part C

Branch `feat/eqoa-api-r7c`, based on the November 18 upstream sync and then
updated from `develop` for 0.6.0.

## Capability and window contracts

- A refusal installs its Lua member and descriptor together. Native binds
  describe implemented methods and module status is derived when queried.
  Compatibility tables are inspected for members their adapters have yet
  to describe. Continuous controls name the upstream PR in their reasons.
- The round labels follow the mod API plan, including round 6 for takeover
  and the additional 6.5 actions and entity field. After the freeze, authors
  should use capability `revision` for feature detection.
- The audit classifies the round 6.5 additions in
  [the audit delta](round-7-audit.md#5-round-65-delta-part-c). Chat sender,
  channel code and direction, and the native hotbar's command text and icon
  fields have no projection in this client. Their descriptors give the reason.
- `replaces` is optional in `mod.json`. A full takeover also requires the
  existing takeover capability. A mod that uses `own` may omit `replaces`.
  The native manager displays the two approved disclosure lines from the
  manifest.
- **Use the game's windows** is a session-wide native switch. Turning it on
  releases every claim immediately and blocks new claims. Hidden sessions
  use the existing close path. Turning the switch off allows mods to claim
  windows again.

## Validation

The first CI attempt at `1539a24b` passed the checks job. Compilation of the
test binary failed to resolve a test-only field helper from the item fixture.
The next commit moved the helper into the shared addon test module. The
`develop` merge keeps the Part C UI disclosure as an unreleased change.

The first focused run passed all five registration and window-contract
tests. The record tests originally enumerated a Lua proxy with `pairs`,
which does not enumerate its backing table. The revised fixture checks
the underlying keys and reads their values through the public record.
The host event fixture uses a multi-step batch so it exercises progress
and completion.

The generated preflight reference from the Windows client has 1,742
descriptors, including 1,643 supported, nine partial, 50 planned and 40
unsupported. All planned rows name their plan. The checks name all 46
supported events and cover the 72 existing record schemas plus icon
surface info.

### Negative controls

One focused run applied these three temporary mutations. Each failed the
intended test.

| Mutation | Observed failure |
|---|---|
| Mark the refusing `eqoa.actions.move` member supported | Its capability status disagreed with the call's error code. |
| Remove `class_id` from the Play player projection | `eqoa.player.Player.class_id is missing`. |
| Suppress the combat hit event's production name | `Play did not emit: combat.hit`. |

Removing `class_id` also removed the default class argument for the
item check. That fixture now passes the class, race and level explicitly
to the real `can_use` binding. All three mutations were restored
byte-for-byte before this change.

The restored focused gate passed **11 tests, 0 failed, 0 ignored**.
It used the shared Cargo lock and reported the current worktree's
test executable.

## Fix round 1 (review of 39af911f)

The review (`eqoa-round-7c-review.md`, APPROVE WITH CHANGES, read-only) is answered finding by finding below.
Every finding got a test first, recorded failing on the unfixed code in one red run (commit `1174e85a`,
16 of 25 `round7c_` tests failing), then a fix commit. A negative-control run then broke each fix again and
recorded the failing line; the files were restored from Git (`git status` clean afterwards).

Commands: `python tools/run_gates.py --only round --filter round7c_` (shared target, the disc linked as
`run/eqoa.iso`). The negative controls used the same command with the mutations of
mutations listed per finding below applied (run A: all but the L1 save and L5; run B: those two), then `git checkout -- crates`.

### Findings

| Finding | Reproduced (red run line) | Fix commit(s) | Test | Negative control (mutation → failing line) |
|---|---|---|---|---|
| H1 takeover of a real window | yes: `inventory: invalid_argument` | `db40a282`, `4e71ba19` | `round7c_takeover_answers_unsupported_for_a_game_window_it_cannot_take_yet` | `is_window` arm disabled → `inventory: invalid_argument` |
| H1 `replaces` unchecked, label promises it | yes: label `Replaces game windows: Bank, Bags, Spellbook, Loot` | `db40a282` | `round7c_replaces_accepts_game_windows_and_reports_any_other_name` | `replaces` check disabled → `hud.target accepted` |
| H2 `WeaponSets.Use` ungated | yes: `Some(["eqoa.actions.chat"])`: left `"ok"`, right `"permission_denied"` | `3288fb06` | `round7c_native_weapon_set_alias_passes_the_weapon_set_gate` | gate disabled → same line |
| H2 native `SendChatText` ungated | yes: `[] /sit`: left `true`, right `false` | `3288fb06` | `round7c_native_chat_send_passes_the_chat_and_command_gates` | gate disabled → same line |
| H2 Forge imports unlabelled | yes: `arpg_ui`: left `[]` | `3288fb06` | `round7c_forge_imports_are_labelled_and_keep_their_acting_calls` | Forge branch disabled → `arpg_ui` |
| M1 `retry_after` never set | yes (after removing it from the optional list): `eqoa.Error.retry_after is missing` | `5ac24bf0`, `2a05bb66` | `round7c_play_records_match_declared_fields` | arm renamed → `eqoa.Error.retry_after is missing` |
| M2 event fields | yes: `eqoa.events.pointer.pressed.data.steps is missing`, `...pointer.wheel.data.button is missing` | `7c810593`, `bebf4a02` | `round7c_host_service_events_reach_lua_from_their_producers`, `round7c_world_events_come_from_play_changes`, `round7c_item_records_and_window_events_use_projected_native_state`, `round7c_manager_events_come_from_their_production_callers` | `steps` declared on `pointer.pressed` again → `eqoa.events.pointer.pressed.data.steps is missing` |
| M3 dialog label | yes: `["eqoa.actions.activate_row"]`: left `false`, right `true` | `4a6b1e65` | `round7c_dialog_label_follows_every_action_that_can_answer_a_dialog` | `activate_row` dropped from the list → same line |
| M4 events by direct emitter calls | not a code defect; the 8 events now come from their production callers and all 8 arrive | (test) `1174e85a` | `round7c_manager_events_come_from_their_production_callers` | `enabled` call removed from `Host::finish_frame` → `the manager did not emit: enabled` |
| M5 stale planned rows | yes: `49 stale planned rows`, e.g. `ImGui.BeginTooltip: planned for the mod API plan's round 4 map and ImGui additions`, `eqoa.actions.move: ... (mod API plan, round 6)` | `05b553ef` | `round7c_no_planned_row_names_a_closed_round_or_part` | JPEG row back to a round 4 reason → `eqoa.textures.formats.jpeg: JPEG decoding is planned in the mod API plan's round 4` |
| M5b waypoint (build) | yes: `not_yet: approved in the round 7 plan (Part A)` | `2a0a6dc5`, `efa56ce5` | `round7c_mod_moves_and_clears_the_players_own_waypoint` | `api_set_waypoint` call removed → `the game's own waypoint moved` |
| L1 switch not saved | yes: `the switch must hold after a restart` | `aef62d45`, `34b3be1a`, `364f8aa7` | `round7c_game_windows_switch_is_saved_and_stops_a_claimed_batch` | save removed → `the switch must hold after a restart`; batch stop removed → `batch results: 0` |
| L3 GPU part passes silently | by reading only (this PC has a GPU) | `1174e85a` (split) | `round7c_media_records_without_a_gpu_match_their_fields` (no GPU), `round7c_texture_record_matches_its_fields` (GPU, counted SKIP) | `render_scale` removed from the display record → `eqoa.client.DisplayMetrics.render_scale is missing` |
| L4 Forge header | yes: the section began `Each module lists its member count ...` | `d299a93b` | `round7c_forge_section_says_presence_is_not_verification` | sentence removed → same line |
| L5 status/code check only on `move` | the broader check finds no disagreeing member (88 refusing functions); the defect was the narrow check | `a0bed5e0`, `4d6755a7` | `round7c_every_refusing_member_reports_its_status_through_its_call` | `not_yet` refusals described `unsupported` → `eqoa.actions.move: code not_yet but status unsupported` (also `look`, `buttons`) |
| L2 spec wording | not this round's: Part D2 rewrites the spec | — | — | — |

The first L5 negative control did not fail the new check: it resolved `eqoa.*` with `rawget`, which misses
eqoa's modules, so it reached only the ImGui and Forge refusals. Fixed in `4d6755a7`; its negative control then failed as listed (run B).

### What changed, in a player's terms

- **Mods & Addons, Forge UIs.** Wereoxxs, ARPG UI and Modern UI (and any imported Forge package) now show
  `Actions: can act on its own (chat, command, weapon_set)`: they can send chat lines and slash commands and
  swap weapon sets. Their calls work exactly as before (tested: Wereoxxs's weapon-set and chat calls, no new
  diagnostic).
- **Mods & Addons, native mods.** A mod whose `replaces` names a window this build cannot replace yet shows
  it on a second line; one that names no game window is refused with a diagnostic. A mod that can confirm
  rows (`activate_row`) now shows `Can answer NPC dialogs`.
- **Use the game's windows** is remembered after a restart, and turning it on stops a running Sell junk or
  Take all at once (the Bag Viewer shows `stopped after N of M items: stopped`, as its Stop button does).
- **Map:** a mod may now move and clear the player's own waypoint (`eqoa.map.set_waypoint`/`clear_waypoint`);
  no bundled mod does yet. The pin is the game's own: saved, shown and reached as if the player placed it.
- A native `eqoa/1` mod that swaps weapon sets through `eqoa.WeaponSets.Use` or sends chat through Forge's
  `SendChatText` now needs the matching capability; no bundled native mod does either.

### Exact label strings

- `Replaces game windows: <titles>` (only `loot`, `merchant`, `bank`: `Loot`, `Merchant`, `Bank`)
- `Not replaceable in this version: <titles>` (the game's own window titles, e.g. `Inventory`, `Abilities`)
- `Actions: can act on its own (chat, command, weapon_set)` (every Forge import)
- `Can answer NPC dialogs` (a mod granted `eqoa.actions.dialog_answer` or `eqoa.actions.activate_row`)

### Decisions and readings (the brief left these open)

- **Window kinds.** The one list is the `Window.kind` names `eqoa.windows.get()` reports (spec §7's mode
  table plus `inventory`, `map`, `search`, `typing`), now in `window_kinds.rs`; the window capture, takeover,
  `replaces` and the labels all use it. The design's example names `hud.target` and `journal` are not window
  kinds (HUD pieces stay under `eqoa.hud`; the journal is `quest_log`), so `replaces` rejects them. The
  existing discovery test used `hud.target`; it now uses `inventory`. `release` and `taken` share the parse,
  so they answer `unsupported` for those windows too.
- **An unknown `replaces` name** rejects the manifest with a diagnostic, as the other invalid manifest
  values do.
- **Legacy gate.** The gate applies to a native `eqoa/1` package (and to a test host given capabilities); a
  Forge import (no `mod.json`) is never gated. A slash line needs `eqoa.actions.command`, any other line
  `eqoa.actions.chat`. Refused calls diagnose (shown in Mods & Addons); `WeaponSets.Use` returns `nil, Error`,
  `SendChatText` its Forge shape `false, "<code>: <message>"`.
- **Acting helpers, gate yes/no:** `eqoa.WeaponSets.Use` yes (`eqoa.actions.weapon_set`);
  `frontiers_forge.chat.SendChatText` yes (`chat`/`command`); `eqoa.actions.*` and `eqoa.windows.takeover`
  yes (unchanged); `eqoa.map.set_waypoint`/`clear_waypoint` no (spec §7 gives no capability; local pin only);
  `eqoa.map.open` and `eqoa.windows.open('summary')` no (open a local window, as the player's key does;
  nothing is sent); `eqoa.ChatHistory` no (read only); `frontiers_forge.camera` no (reads only; camera moves
  go through `eqoa.actions.camera`, gated); `frontiers_forge.input.SetKeySuppressed` no (suppresses a key,
  sends nothing). No other path reaches the action broker or the server.
- **M1.** No refusal knows a wait, so `retry_after` is `unsupported`. Going through the optional lists also
  found `BatchResult.detail` never filled (now `unsupported`), `BatchResult.current_step` always filled and
  `Mastery.stats`/`bonuses` always arrays (both taken off the lists). Every remaining entry has a one-line
  reason beside it.
- **M2.** Removed declarations (never filled): `pointer.pressed.data.steps`, `pointer.wheel.data.button`,
  `resource.ready.data.error`, `resource.failed.data.value`, `resource.failed.data.path`,
  `entity.added.data.before`, `entity.removed.data.after`. A field counts as filled when any delivery in the
  fixtures filled it; `EVENT_OPTIONAL` lists the rest with reasons.
- **M5.** The 22 ImGui members the bridge lacks are `unsupported` (mouse state: use `eqoa.input.pointer()` and
  the pointer events; the rest: not part of the bridge in `eqoa/1`); none of the bundled UIs calls them. JPEG
  is `unsupported` (PNG or RGBA). `move`/`look`/`buttons` stay `not_yet` after the upstream PR, round 8.
- **M5b.** `set_waypoint`/`clear_waypoint` are request/result actions (spec §7 map aliases, §8 results) with no
  capability, as the spec gives none. They use the map's own `set_waypoint` through the fork's existing
  `map_api` child module: no upstream line. `clear_waypoint` answers `busy` once the pin is no longer the
  mod's. A disabled mod's still-owned pin goes away; the player's pins stay.
- **L1.** The switch is a new `use_game_windows=` line in the fork's `enhanced.opt`; other lines are kept.

### Results

- **Red run** (`1174e85a`, tests only): 9 passed, 16 failed; every finding's test failed for its reason
  (lines in the table). Three red failures were harness faults, fixed before any code fix: Lua here has no
  `table.pack`; lazily built event data hides its keys until read (the check now reads the declared fields and
  the native data's keys); and the manager probe ran as a per-frame entry, which gets no `render` events.
- **Focused run on the final code** (`4d6755a7`; `d5f8a514` and `dab9b351` add docs and test-only
  changes): `python tools/run_gates.py --only round --filter round7c_`, disc linked (`run/eqoa.iso`,
  `EQOA_ISO`): **25 passed, 0 failed, 0 ignored, 0 SKIP lines**. 88 refusing functions checked through their
  call. With this GPU the manager test and the texture test ran; without one each prints a counted SKIP.
- **Negative controls:** run A 17 failed: its 14 target tests, plus three older tests the same breaks also
  catch (the round 7C discovery test, the label unit test, the `move` status test); the new L5 check did not
  fail there, see above. Run B 2 failed (L1 save and the corrected L5 check). Files restored with
  `git checkout -- crates`; `git status` clean after each.
- **Release build** (`run_gates.py --only release --release`): PASS. `SUPPORT-TABLE.md` and
  `eqoa-1-surface.json` regenerated from it (`d5f8a514`): 1,735 descriptors, 1,634 supported, 9 partial,
  89 unsupported, 3 not_yet (`move`, `look`, `buttons`). `round7b_support_table_matches_the_running_client`
  passes locally against it (5 passed with the four tests below).
- **wasm** (local and CI): PASS. **Footprint:** PASS; nothing outside `crates/eqoa/src/fork_ext/` and `docs/`
  changed since `39af911f` (no upstream line, no budget change). **No local paths:** PASS.
- **CI run 44673792210** (`d5f8a514`): checks PASS (wasm, footprint, paths, player-release 20/0, tools
  199/0 with 1 SKIP); tests FAIL: addons 588 passed, 2 failed, 17 SKIP; full 2703 passed, 4 failed, 291 SKIP.
  The zero-addon allocation gate (`performance_busy_world_zero_addons_frame_and_packet_allocation_bound`) and
  the bundled-mod tests passed. The four failures were existing tests holding the old contract:
  `round6c_batch_gesture_single_batch_refuses_second_serial_execution_and_capabilities` (BatchResult.detail),
  `round7a_map_refusals` (waypoint calls), `round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon`
  (JPEG), and `round6b_non_item_chat_command_windows_weapon_sets_and_spellbook`, whose `destroy` reason
  assertion Part C left stale (it already failed at `39af911f`). Fixed in `dab9b351`; the four pass locally.
- **CI run 44677711728** (`dab9b351`): **PASS.** checks PASS (wasm, footprint, paths, player-release 20/0, tools 199/0 with 1 SKIP); addons 590 passed, 0 failed, 25 ignored, 17 SKIP; full 3186 passed, 0 failed, 37 ignored, 378 SKIP (CI has no disc and no GPU). `ci_gates.py cleanup` run afterwards.
- The regeneration commit `d5f8a514` is followed by `dab9b351` (tests only, the surface is unchanged).

### Not verified

- Nothing was played: no live client, no server. The Forge labels, the switch after a real restart, and a
  mod moving the waypoint on the M map are tested only through the test fixtures.
- The decomp and eqoa-port oracle are not on this machine (oracle tests skip, as for every round here).
- The manager-level event test and the texture field test need a GPU; on CI they print SKIP (counted).
- L3 could not be shown failing on this PC, which has a GPU: reproduced by reading only.
- A mod that owned the waypoint and is disabled: its pin is cleared on the next step; covered by code reading
  and the ownership helpers, not by its own test.

## Fix round 2

From the cross-family review of 0e35c35e (`round-7c-fix1-review.md`, REJECT: 4 HIGH, 5 MEDIUM, 2 LOW). Two
builders: the first (a GPT model) merged develop, wrote the reproducing tests and fixed H1, M5, M3, M4, H2 and H3
before a usage limit stopped it; the second (Claude) kept its uncommitted H4 work after review, fixed M1, M2, L1,
L2 and the record-helper exception, ran every negative control below (including for the first builder's fixes),
regenerated the reference and ran the gates. Zero upstream lines (`git diff --stat 4399bcac..HEAD` outside
`fork_ext/`, `docs/` and the notes is empty).

Step 0: develop merged in 4399bcac. The post-merge focused run had 65 passes, one failure (the checked-in support
table, missing develop's `Dialog.prompt_kind`; fixed by the regeneration) and the one deliberate no-adapter SKIP.

### Per finding

"Red" is the reproducing test failing on the code before its fix (red run A at 1ec6a293 for the grant and window
findings; red run B at e9ac471e for the rest). "Negative control" is the second builder's own run: the fix broken
by the named mutation with every test present, `python tools/run_gates.py --only round --filter round7c_`, then
`git checkout -- crates` (status clean afterwards). Batch A held the first builder's fixes, batch B and C the
second builder's; each test failed only for its own mutation.

| Finding | Red (reproduced) | Commit | Test | Negative control: mutation → failing line |
|---|---|---|---|---|
| H1 chat vs command | yes: typed chat and the native editor completed a slash line under the chat grant; `batch slash line requires command` | 06e60ef3 | `round7c_typed_chat_checks_the_grant_for_the_final_line`, `round7c_batch_chat_checks_the_grant_for_the_final_line`, `round7c_native_chat_editor_checks_the_submitted_lines_grant`, `round7c_native_chat_send_passes_the_chat_and_command_gates` | `chat.rs line_capability`: `if false && line.starts_with('/')` → `r6d_tests.rs:88 nil req=RequestId(1) n=1 msg=nil` (typed, editor), `batch slash line requires command`, `round7c_fix_tests.rs:260 ["eqoa.actions.chat"] /sit left: true right: false` |
| H2 grants across rescan | yes: `native enable must not authorize a Forge import`, `the legacy enable was already adopted` | efb5d62a, e9ac471e | `round7c_enabled_native_package_cannot_gain_forge_grants`, `round7c_legacy_enable_adopts_once_then_refuses_new_grants_on_restart`, `round7c_bundled_packages_keep_their_enabled_disclosures` | `manager.rs`: `Some(old) if true \|\| old.allows(&new)` → `manager.rs:2529 native enable must not authorize a Forge import`, `manager.rs:2567 the legacy enable was already adopted` |
| H3 waypoint checks + grant | yes: `{world_id=99,...} left: "completed" right: "invalid_argument"`; `set requires eqoa.map.waypoint` | 71a394dd | `round7c_waypoint_checks_the_addressed_native_ground_at_dispatch`, `round7c_waypoint_writes_require_a_grant` | dispatch skips `api_validate_waypoint` → `api_capture_tests.rs:3526 {world_id=99,x=25300,z=15200} left: "completed" right: "invalid_argument"`; `waypoint_ready` returns `Ok(())` → `api_capture_tests.rs:3494 set requires eqoa.map.waypoint` |
| H4 waypoint ownership | yes: `the earlier world's owned pin was cleared` (left `Some([22000, 15000])`), `the second VM put this pin` (left `None`), `the replacement belongs to the player` (left `None`) | 63e625b7 | `round7c_disable_clears_every_world_the_mod_still_owns`, `round7c_player_owns_a_pin_replaced_at_the_same_position`, `round7c_other_mods_identical_pin_survives_disable_in_the_same_step` | per-step `reconcile` call removed → `api_capture_tests.rs:3579 the replacement belongs to the player`; `release_vm` marks only the last pin → `:3553 the earlier world's owned pin was cleared`; `placed` keeps a released record → `:3600 the second VM put this pin` |
| M1 claim used by talking | yes: `merchant: completed` | 5d59601b | `round7c_switch_stops_batch_that_reaches_a_claim_by_talking` (merchant and bank) | stop filter ignores `used_claims` → `items_tests.rs:4343 merchant: completed` |
| M2 late reply, staged work | yes: `late success count: 0`; `the native merchant did not consume the discarded sale` | 458956be, 3d4e5a39 | `round7c_switch_counts_an_already_sent_step_after_its_late_reply`, `round7c_switch_discards_a_staged_unsent_step`, `round7c_game_windows_switch_is_saved_and_stops_a_claimed_batch` | sent step not awaited → `items_tests.rs:4398 late success count: 0` and `:4310 stopped cancelled stopped 0`; staged step kept → `:4412 the native merchant did not consume the discarded sale` |
| M3 takeover precedence, HUD tokens | yes: `own inventory: unsupported`; `hud.target` rejected | 863f3221 | `round7c_takeover_checks_view_scope_before_build_support`, `round7c_window_design_manifest_loads_and_discloses_hud_target`, `round7c_manifest_declares_full_takeover_for_the_named_window_only` | view-scope check off → `round7c_fix_tests.rs:130 own inventory: unsupported`; discovery rejects HUD tokens → `round7c_fix_tests.rs:43 "hud.target" is not a game window kind or an eqoa.hud piece token`, `discovery.rs:986 discovered.diagnostics.is_empty()` |
| M4 label needs the grant | yes: `Replaces game windows: Bank` without the grant | 0f11d3c1 | `round7c_replacement_disclosure_requires_the_takeover_grant` | `can_takeover = true \|\| ...` → `round7c_fix_tests.rs:158 ["Replaces game windows: Bank"]` |
| M5 chat length, channel | yes: a 128-unit ASCII line and a non-nil channel both completed | b1deb2b4 | `round7c_typed_chat_rejects_the_native_utf16_cutoff`, `round7c_typed_chat_refuses_explicit_channels` | limit `>= 256` and channel check off → `r6d_tests.rs:88 nil req=RequestId(1) n=1 msg=nil` (both tests) |
| L1 refusal shapes, all 88 | yes (old checker): `expected three malformed returns to be caught; found 0` | ef2c2005 | `round7c_every_refusing_member_reports_its_status_through_its_call`, `round7c_refusal_checker_detects_wrong_codes_and_missing_values` | native refusals answer `unsupported` for a declared `unavailable` → `eqoa.hud.hidden_by: code unsupported but declared unavailable`; `ReadForeignCave` refusal not installed → `round7c_fix_tests.rs:501 refusing callables by name`; checker without the code and value-count checks (batch C) → `expected three malformed returns to be caught; found 0` |
| L2 switch through the store | test-only finding | c9d783dd | `round7c_game_windows_switch_round_trips_through_the_settings_store` | `save_switch` drops other lines → `items_tests.rs:4454 marker and unrelated line kept, switch saved left: "use_game_windows=on\nfirst_run_defaults_applied=on\n"` |
| Helper nil-record exception | yes: `an ordinary record must be a table even without declared fields` | eace9fa3 | `round7c_only_payload_free_events_can_have_a_nil_record` | `payload_free` condition removed → `native_tests.rs:1860 an ordinary record must be a table even without declared fields` |

How each fix works, briefly:

- **H4.** One ownership record per world, naming the mod. Every game step (before dispatch) drops, for good, the
  record of a pin that is absent or differs; another mod's `set_waypoint` replaces the record during dispatch;
  a disabled mod's records are marked released and cleared after dispatch, in every world, only where the pin
  is still exactly theirs. `clear_waypoint()` (no world argument) clears the most recently placed pin the mod
  still owns (spec row says so).
- **M1.** A `talk` step whose dispatch arms a merchant or bank open under the mod's claim records that claim on
  its batch (`used_claims`); the switch stops batches that hold or used a released claim. The record does not
  end sessions at batch end (unchanged behaviour there).
- **M2.** Stopping a batch whose step is on the wire keeps that step in flight; the batch's single
  `batch.result` waits for the reply (at most the 10 s server timeout, or a zone reset) and counts a confirmed
  step. `eqoa.actions.status(batch)` reads `stopped` at once and its `completed` is updated when the reply lands.
  A staged, unsent step is cancelled (its action result says "the batch stopped before this step was sent") and
  its staged transaction is cleared at the next step and again right before the merchant/loot and bank send
  hooks, so a stop that lands mid-frame is not sent by the native window either.
- **L1.** In test builds each refusal records its declared code and return shape; the check requires, per
  callable, nil plus an Error with exactly that code and the described message (`native`), or the pinned Forge
  shape value by value (`nil`; `nil, reason`; `nil, reason, nil`), and compares the sorted callables with a
  pinned list of all 88 names.
- **Helper.** `field_issues` takes `payload_free`; only `event_fields` passes it.

### Existing tests whose assertion changed

| Test | Before → after | Why |
|---|---|---|
| `round7c_game_windows_switch_is_saved_and_stops_a_claimed_batch` | one `stopped` result after three steps, no count → the test delivers the sent sale's reply, then one `stopped` result with `completed == 1` | M2: a stopped batch's result now waits for the step already sent. Without the reply it would arrive only at the 10 s timeout. |
| `round7c_every_refusing_member_reports_its_status_through_its_call` | `checked >= 30`, code compared only as "is `not_yet`" → every callable's exact declared shape and code, 88 by count and name | L1 |
| `round7c_mod_moves_and_clears_the_players_own_waypoint` | points (1200,3400), (50,60), player (10,20), no grant → (12000,3400), (5000,6000), player (10000,20000) on a fixture map with ground, `eqoa.map.waypoint` granted | H3: the old points are outside any map's ground and the call now needs the grant. Ownership and clear assertions unchanged. |
| `round7c_manifest_declares_full_takeover_for_the_named_window_only` (discovery) | replaces `["bank","bank","inventory"]` → `["bank","bank","hud.target"]`, dedup to `["bank","hud.target"]` | M3: restores round 1's HUD-token example, which loads again. |
| `round7c_manager_labels_come_from_manifest_declarations` (runtime) | `replacement_labels(list)` with 2 lines → `replacement_labels(list, can_takeover)` with 3 lines; adds `["hud.target"] → "Replaces game windows: Target frame"` | M3 restores the target-frame label expectation; M4 adds the grant argument and the HUD line. |
| `round7c_forge_imports_are_labelled_and_keep_their_acting_calls` | `Actions: can act on its own (chat, command, weapon_set)` → `(chat (ordinary lines), command (slash lines), weapon_set)` | H1 label wording. |
| `round7c_replaces_accepts_game_windows_and_reports_any_other_name` | `hud.target` listed as unknown → `hud.bogus`; its `views_only` fixture `replaces: ["inventory"]` alone → with `capabilities: ["eqoa.windows.takeover"]` | M3: `hud.target` is a valid declaration again. M4: without the grant the fixture would now get a diagnostic and no affirmative label. |
| `round6a_windows_modes_dialogs_hints_and_outside_world` (input box), `round6a_seam_dialogs_privchat_response_pages_and_confirm_yes_no` (response dialog) | optional fields `options selection style style_id` / `selection tag title chat password style style_id` → each plus `prompt_kind` | Found by the first full CI run (44704221098, the only 2 failures of 2748). develop's `Dialog.prompt_kind` (open_window fix) exists only on confirm dialogs, and this branch's record check requires every supported field not listed optional. The confirm check (`round6a_windows_modes...` confirm box) still requires it. 99408179. |

`round7c_window_design_manifest_loads_and_discloses_hud_target` is new this round (1ec6a293); M3/M4 tightened
its own first expectation (`any line contains "Target frame"`) to the exact two lines. No other existing
assertion changed. The shared record helper is stricter (an ordinary record may no longer be
nil); every existing record test still passes.

### Label strings (Mods & Addons)

- `Actions: can act on its own (chat (ordinary lines), command (slash lines), weapon_set, ...)`
- `Replaces game windows: <windows>` only with `eqoa.windows.takeover` granted; a manifest naming a window in
  `replaces` without it gets the diagnostic `Manifest 'replaces' lists game windows without the
  eqoa.windows.takeover capability` and no affirmative label.
- HUD tokens (`hud.target` and the rest of the `eqoa.hud` piece list): `Replaces game windows: Target frame`
  with the takeover grant (as before fix round 1); `Replaces HUD: Target frame` without it. For the lead: the
  brief's "labelled as they were before fix round 1" and M4's "the affirmative label needs the grant" meet here;
  the first builder chose this split. A HUD-only mod has no reason to hold the takeover grant, so it sees the
  `Replaces HUD:` line.
- `Not replaceable in this version: <views>`
- H2 manager line: `its permissions changed; enable it again to accept`
- `eqoa.map.waypoint` is listed with the other capabilities: "capability to set or clear a personal waypoint".

### What a player would notice

- A mod whose package type changed or that gained permissions since the player enabled it comes back
  **disabled** with "its permissions changed; enable it again to accept". Existing installs adopt their current
  permissions once, silently; the bundled mods stay enabled (`round7c_bundled_packages_keep_their_enabled_disclosures`).
- A mod with chat permission only can no longer run slash commands (`/sit` is refused `permission_denied` with a
  diagnostic), whether typed, in a batch, through the Forge chat helper or by submitting the game's chat editor.
- Mods need the new map waypoint permission to place or clear a pin; pins land only on a loaded map's ground.
  Disabling a mod removes only the pins it still owns, in every world; a pin the player cleared and placed
  again, even at the same spot, stays the player's.
- Stopping a mod's selling or banking run (its Stop, or **Use the game's windows**) reports a sale already sent
  once the server answers (Bag Viewer shows the true count), never sends a step that was only queued, and also
  stops runs that reached the merchant or bank by talking to them.
- Typed chat of 128 or more UTF-16 units is refused instead of silently vanishing.

### Gates

- Focused, with the disc (`EQOA_ISO` → `run/eqoa.iso`): `python tools/run_gates.py --only round --filter round7`
  → **88 passed, 0 failed, 1 SKIP** (`round7b_without_adapter_is_an_explicit_skip`, which forces the
  no-adapter path on purpose; same SKIP as the baseline). `--filter round7c_` alone: 47 passed, 0 SKIP.
- `--only wasm,release --release`: wasm check PASS, release build PASS.
- Footprint (`--base 35e2a43b`): PASS, no budget change. `check_no_local_paths.py`: PASS.
- Reference regenerated from that release build in 45de7f86 (adds `eqoa.map.waypoint` and develop's
  `eqoa.windows.Dialog.prompt_kind`); `round7b_support_table_matches_the_running_client` passes against it.
- Full set on GitHub (`ci_gates.py`): first run 44704221098 at 45de7f86 failed 2 of 2748 in `full` (the two
  `round6a_` dialog record checks above; `addons` 600 passed). Fixed in 99408179 (test-only, after the
  regeneration commit; the generated files are unchanged by it). Second run **44710027543 at 99408179: success**
  (addons 600 passed / 0 failed / 25 ignored / 17 SKIP; full 3227 passed / 0 failed / 37 ignored / 378 SKIP;
  wasm, footprint, paths, player-release 20/0, tools 201/0/1 SKIP). CI has no disc and no GPU: its SKIPs are
  those tests. `cleanup` run.
- Zero-addon gate: the new per-step calls (`reconcile`, `take_released`, `discard_expired_staged`) work on
  empty collections and allocate nothing; the zero-addon allocation test runs in the full set.

### Known limits and not verified

- **H4 residual (accepted by the lead):** a player who clears and re-pins the exact same point between two 30 Hz
  game steps (under 34 ms) is not seen, so a disable in the next step could clear that pin. Closing it takes
  one upstream line: at the top of `Map::set_waypoint` (`crates/eqoa/src/map/mod.rs:1191`), a call that drops
  the world's ownership record; the mod path (`api_set_waypoint`, then `map_waypoint::placed`) records again
  after it. Not taken (zero upstream lines this round).
- **L2 and the real file:** unit tests never touch disk. Upstream's `settings.rs` compiles an in-memory
  per-thread store under `cfg(test)` (`settings.rs:3016`) and leaves the file store (`:3071`, write to
  `.new` then rename) out of test builds. The new test goes through the same `read_config`/`write_config`
  calls and the client's start-up read (with the defaults migration) with the marker and an unrelated line,
  dropping every cache, but cannot open a file after a real process restart without an upstream test seam.
  The file store is upstream's, unchanged. A live check: toggle **Use the game's windows**, quit, restart.
- **M2 timing:** between the stop and the reply, `status(batch)` already reads `stopped` with the count so far;
  the one `batch.result` comes with the reply (or after 10 s as before, without the count).
- No live client or server session. The decomp/eqoa-port oracle is not on this machine; oracle tests skip.
- The bundled Wereoxxs, ARPG UI, Modern UI and `eqoa_*` mods are covered by the bundled-package test above and
  their own tests in the full CI set; not played.
