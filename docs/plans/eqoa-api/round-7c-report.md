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

Merged develop in 4399bcac. The post-merge focused run reported 65 passes,
one failure in the checked-in support-table comparison, and one explicit
no-adapter SKIP. The support reference will be regenerated after the fixes.
The post-merge footprint and tracked-path checks passed.

Red run A used the focused gate with the tests in 1ec6a293. It ran 11 tests
with 10 failures and no SKIP lines. The bundled-package compatibility fixture
passed. The first B attempt stopped at a compile error in the H2 decoder, fixed
in e9ac471e. That attempt produced no test evidence.

| Finding | First failure | Fix |
|---|---|---|
| H1 | Typed chat and the native editor both returned and completed a request under the chat grant for a slash line. Batch assertion: `batch slash line requires command`. | 06e60ef3 |
| H2 | `native enable must not authorize a Forge import` and `the legacy enable was already adopted`. | efb5d62a, e9ac471e |
| M3 | `own inventory: unsupported` and `hud.target` rejected as a declaration. | 863f3221 |
| M4 | The ungranted manifest displayed `Replaces game windows: Bank`. | 0f11d3c1 |
| M5 | ASCII chat at the 128-unit cutoff and a call with a non-nil channel both completed. | b1deb2b4 |

The remaining findings, negative controls, final verification and support
reference generation are pending.
