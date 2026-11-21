# Round 7 H: last code corrections before the `eqoa/1` freeze

Branch `fix/eqoa-api-r7h` (from `docs/eqoa-api-r7-d2` at b30b5bd7). Source: `round-7d2-report.md` section 1,
items numbered as there. Every item got a test first (`round7h_*`, two files:
`crates/eqoa/src/fork_ext/addons/round7h_tests.rs` and `crates/eqoa/src/fork_ext/round7h_play_tests.rs`), run
once on the unfixed code. That run is each item's negative control: the test fails on the base, with the line
below. All new code is in fork-owned files; no upstream file is touched.

Reproduction run (base code, tests only, commit 4fc68dbe):
`python tools/run_gates.py --only round --filter round7h_` → **2 passed, 11 failed**; the two passes are item 1
and the alias check, which were expected to pass (they did not reproduce).

## 1. Per item

| # | Reproduced | Commit | Test | Failing line on the base (negative control) |
|---|---|---|---|---|
| 1 `Windows.active` | **No.** `assign_window_refs` (`addons/native_core.rs`) replaces the capture's null with the focused window's ref; `r6a_tests.rs` already asserts `g.active == mw.ref`. The D2 reader saw only `r6a_capture.rs`'s placeholder. Code left. | eac9baa5 (spec only) | `round7h_1_windows_active_is_the_focused_window` | passes on the base (expected) |
| 2 `follow(nil)` | Yes | eac9baa5 | `round7h_2_follow_nil_only_stops_following` | `round7h_play_tests.rs:115: follow(nil) with nothing followed started following the target` |
| 5 `cycle_target` hostile/friendly | Yes. Upstream has no such cycling: `Play::cycle_target(group: bool, ...)` (`crates/eqoa/src/play.rs`) is "group or not". Now refused `unsupported` with a reason. | 28eb0f5b | `round7h_5_cycle_target_refuses_filters_the_game_does_not_have` | `hostile: nil` (no error; the call was queued and cycled unfiltered) |
| 6 dispatched `ok` | Yes (three `dispatched` sites, plain, batch step, deferred send). Batch records already had `ok = nil` while running. | 4ff5b429 | `round7h_6_a_dispatched_result_has_no_ok_until_the_outcome_is_known` | `a dispatched result says ok = true` |
| 7 irreversible steps | Yes for `drop_quest`, `mastery_buy`, `mastery_refund` (now `not_yet`). `unmemorize_all` stays `supported`: the game never asks (below). | 5b3e7632 | `round7h_7_steps_the_game_confirms_are_not_yet_for_mods` | `drop_quest is supported` |
| 8 constant tables | Yes | 662240ce | `round7h_8_constant_tables_have_their_own_descriptor` | `eqoa.entities.status: left "unknown", right "supported"` |
| 9 draw-list descriptors | Yes | b6153133, b09c4cf1 | `round7h_9_draw_list_methods_are_described_from_their_registration` | `described: ` (none) |
| 10 `options.Button` fields | Yes | 6e144d24 | `round7h_10_option_button_record_has_declared_fields` | `eqoa.options.Button.id is undeclared` (and `item_id`, `label`) |
| 12 `resource.failed` names | Yes | 3428a9dc | `round7h_12_a_failed_load_names_a_capability_that_exists`; the GPU test `round7c_manager_events_come_from_their_production_callers` now also pins `eqoa.textures.rgba` | `texture=eqoa.texture.load names a capability that does not exist` |
| 13 `events.dropped` core-side | Yes | 81d6e826 | `round7h_13_core_side_loss_reports_no_made_up_sequence_numbers` | `made-up sequence numbers: 2..2` |
| low `input.bind` twice | Yes | d22a1423 | `round7h_low_input_bind_twice_runs_one_callback_per_press` | `first 1, second 1` |
| low no-argument list | Yes | 91ecaecf | `round7h_low_no_argument_functions_refuse_an_argument` | `chat.get_channel took an argument: not_in_world` (the argument was ignored) |
| low stale texts | n/a (text only) | 91ecaecf | none | `action_broker.rs:2` comment; `windows.open` `busy` message now "16 client window requests are already waiting" |
| low status follows registration | see section 3 | — | `round7h_low_legacy_alias_status_follows_its_binding` | passes on the base; see section 3 for its executed mutation |

What changed, item by item:

- **2.** `play.rs` `ActionKind::Follow`: with no entity and nothing followed the action completes and does
  nothing (it no longer calls the game's toggle). With a follow running, `follow(nil)` still stops it.
- **5.** `action_broker.rs` `cycle_target`: `"hostile"` and `"friendly"` give `nil, Error{unsupported}`
  ("the game has no 'hostile' target cycling; use 'group', 'corpse' or no filter"). `nil`, `"any"`, `"nearby"`,
  `"group"`, `"corpse"` unchanged.
- **6.** `ok` is nil while queued or dispatched and for `unknown` (and for a request cancelled after it was
  sent, as before); true for `completed`; false for `refused` and for a cancel before sending. Bag Viewer reads
  `state` before `ok` on every dispatched path (`modules/bags.lua` `on_action_result`: `state == "dispatched"` is
  tested first; the pending-open branch needs `phase ~= "dispatch"` for `ok == true`), so it needed no change and
  no checksum refresh. No other bundled reader of `ok` exists (`grep` of `bundled-mods/**` and `mods/**`).
- **7.** `window_actions.rs`: the three names are `refuse_native(..., "not_yet", Some(6), reason)` (the same
  mechanism as `move`/`look`/`buttons`, so the status comes from that registration). Reason: "the game asks
  the player to confirm this step; a mod cannot take it until the mod API plan's confirm contract (window
  replacement) ties a mod-drawn confirmation to the player's own answer" (the round 7C rule wants a `not_yet`
  reason to point to the plan and name no closed round). Their parsing and dispatch code is kept for that
  contract: a test-only switch (`BIND_CONFIRM_GATED_FOR_TEST`, compiled only in tests) binds them as built so the
  round 6 dispatch tests keep covering it. Nothing is sent by the refused calls (the test checks the session).
- **8.** `native.rs`: `describe` for the two tables, beside their keys.
- **9.** `bridge_drawlist.rs` `install`: makes one never-drawn `DrawList` and describes every function in its
  userdata `__index` as `ImDrawList.<Name>` (round 2). A method added to or removed from `methods` changes the
  registry with it; the test checks that each described name is a method of a live draw list and that the 14
  built ones and nothing else are described. Only where the native API is installed.
- **10.** `native.rs` `install_fields`: `eqoa.options.Button = id label item_id` (round 4).
- **12.** `native_events.rs`: `resource()` names `eqoa.textures.load`, `eqoa.fonts.load`, `eqoa.sound.load`
  by kind; new `resource_from()` for a texture made from memory names `eqoa.textures.rgba` (the eqoa/1 face of
  `UiForge.CreateTextureFromMemory`; a Forge mod calling that directly also gets this name). `path`: still
  declared (it is filled, for `import` and `export`); the spec and the field test's comment now say it is only
  there (and `value` only for `json`). Reading of the brief: the round 7C event-field rule is per event, not
  per `kind`, so "declared only where it is filled" was taken as "say exactly which kinds fill it".
- **13.** `native_core.rs` `source_loss`: the core-side `events.dropped` carries `count` and `streams` only;
  `first_seq`/`last_seq` are nil there (the lost deltas never had event numbers). A mod's own queue overflow
  still gives the real numbers.
- **low, `input.bind`.** A per-VM registry table maps a bind id to its `input.key` subscription; a second
  `bind` of the id unsubscribes the first. A later bind without a callback leaves the key with none.
- **low, no-argument list.** `eqoa.chat.channel`/`eqoa.combat.counters` → `eqoa.chat.get_channel`/
  `eqoa.combat.get_counters`: those two now refuse an argument with `invalid_argument` (no bundled mod passes
  one). The spec lists the whole set.

## 2. Irreversible steps: what the game does, and the action audit

The game's own UI (upstream code, ported from the decomp):

- **Drop a quest:** asks. `questlog.rs`: "X asks (word 574) and a yes sends 0x007D"; `QuestUp::Drop` comes only
  from `BoxAnswer::Yes`.
- **Buy a Class Mastery skill:** asks. `classmastery.rs` `Focus::Inspect`: `message.setup(1663, 1664 or
  1807+1664, PROMPT_YESNO)`; `send02` only after "0 is yes".
- **Refund a Class Mastery skill:** asks. Same place, words 1665/1666, then `send03`.
- **Unmemorize All:** does not ask. `spellbook.rs` popup message `0x0809_0000` pushes `Out::Send00FE` at once.

| Action | Does the game ask? | Irreversible | Status after this round |
|---|---|---|---|
| `drop_quest` | yes (Yes/No, word 574) | yes | **`not_yet`** |
| `mastery_buy` | yes (Yes/No, 1663/1664) | spends points; refundable only through its own confirmed refund | **`not_yet`** |
| `mastery_refund` | yes (Yes/No, 1665/1666) | yes | **`not_yet`** |
| `unmemorize_all` | no (menu choice, sent at once) | no (abilities can be memorized again) | supported |
| `sell` | **yes** for a worn, quick-slotted, no-trade or non-misc item (`merchant.rs` `sell`, words 930/931/932/1842); a misc item sells at once | yes | supported (worn items refused `not_ready`); **put under "Under review" in the spec: lead's decision, see section 6** |
| `buy` | no (a quantity box for stacks, no Yes/No) | no (can be sold back, at a loss) | supported |
| `destroy` | yes (item menu, 916/917/918/1841) | yes | `unsupported` (unchanged) |
| `loot`, `loot_all`, `open_loot` | no | no | supported |
| `bank_move`, `bank_money` | no | no | supported |
| `equip`, `use_item`, `swap_items`, `quick_slot`, `weapon_set`, `prepare_weapon_set` | no | no (`use_item` consumes a charge or potion as the player's use does) | supported |
| `assign_ability`, `unmemorize_ability`, `arrange_ability` | no | no | supported |
| `mastery_share`, `mastery_browse` | no | no | supported |
| `target`, `cycle_target`, `assist`, `target_target`, `interact`, `attack`, `cast`, `use_hotbar`, `autorun`, `follow`, `stop`, `camera` | no | no | supported |
| `talk` | no | no | supported |
| `chat`, `command` | the game's own command rules apply (a `/quit` raises the game's box) | per command | supported |
| `open_window`, `close_window`, `select_row`, `activate_row`, `menu_command`, `input_text` | they press the game's keys; any box the game raises is still raised | no by themselves | supported |
| `dialog_answer` | it is the answer to the game's box | as the step answered | supported; see section 6 |
| `toggle_stats_box` | no | no | supported (Stats Box only) |
| `move`, `look`, `buttons` | n/a | n/a | `not_yet` (unchanged) |
| `batch` steps | as each step | as each step | the 23 step kinds unchanged; none of the three `not_yet` names was a step kind |

Forge: no Forge compatibility call reaches `DropQuest`, `MasteryBuy`, `MasteryRefund` or `UnmemorizeAll`
(the only creators of those kinds are `window_actions.rs` and `action_broker.rs`'s own bindings and batch parser;
`quest_log.rs` is read-only). No bundled Forge UI (Wereoxxs, ARPG UI, Modern UI) calls any `eqoa.actions.*`
(`grep` of `bundled-mods/**`); so none loses a working call.

## 3. Names whose status does not follow a registration

Every `describe(..., "supported", ...)` site was read. Each is one of:

- a binding's own registration (`bind`, `bind_compat`, `register_descriptor`, `refuse_member`/`refuse_native`,
  the ImGui table walk in `native_media.rs`, the Forge/UiForge walks in `annotate_host`, the draw list from
  this round): follows registration;
- a constant table and its keys, described where the table is built (`native_items.rs` `constants`, the two
  tables of item 8): follows;
- the aliases `eqoa.WeaponSets`/`eqoa.ChatHistory`: described by `annotate_host` only if `root.raw_get(name)`
  is non-nil, i.e. only if bound;
- tables that exist regardless of a binding, legitimately: record types and fields (`install_fields`,
  `install_event_fields`, the `ActionResult`/`BatchResult`/`FileEntry`/`SurfaceInfo`/`IconOptions` lists) are
  shapes, held to the records by the round 7C field tests; the event names (`native_events.rs` `EVENTS`) are
  held to their producers by `round7c_supported_event_inventory_has_fixture_coverage`; `eqoa.api` and
  `eqoa.revision` are set two lines above their describe; `eqoa.textures.formats.png/rgba` and
  `eqoa.map.waypoint` are facts and a grant name, not functions; `eqoa.files.null` is set one line above.

The freeze guard's `eqoa.ChatHistory` question, executed. `round7h_low_legacy_alias_status_follows_its_binding`
passes on the code; with `super::chat_history::install(&host)?` commented out in `Host::new` (`host.rs`) the same
command (`python tools/run_gates.py --only round --filter round7h_low_legacy_alias`) fails with
`eqoa.ChatHistory: bound false, described unknown`; `host.rs` was restored with `git checkout` and its sha1
checked equal. So the alias's status does follow its binding. The likely reason the freeze guard saw it stay
`supported`: the alias is installed in two places, `Host::new` and `Host::for_addon` (`host.rs`, the one the
manager and `eqoa mod-api` use), so removing one leaves the other. Not re-run with the freeze guard's own
mutation (it is on another branch).

## 4. Existing assertions changed

| Where | Before | After | Why |
|---|---|---|---|
| `r6d_tests.rs` `round6d_capabilities_are_supported_for_what_is_wired` | `drop_quest`, `mastery_buy`, `mastery_refund` `supported` | `not_yet` (the other eight still `supported`) | item 7 |
| `api_capture_tests.rs` `round6b_non_item_chat_command_windows_weapon_sets_and_spellbook` | `drop_quest` status `supported` | `not_yet` | item 7 |
| `r6d_tests.rs` four tests (`round6d_drop_quest_...`, `round6d_mastery_actions_...`, `round6d_review_d_mastery_buy_refund_...`, `round6d_review_d_edit_box_replaced_and_quest_stamp_changed_are_stale`) | `rig()` | `rig_confirm_gated()` (the test-only switch); assertions unchanged | item 7: keep the built dispatch covered |
| `manager.rs` `round7c_manager_events_come_from_their_production_callers` | — | adds `error.capability == 'eqoa.textures.rgba'` | item 12 |
| `docs_tests.rs` | comment only | — | item 8 |
| `round7c_fix_tests.rs` `round7c_every_refusing_member_reports_its_status_through_its_call` | 88 pinned refusing callables | 91: adds `eqoa.actions.drop_quest`, `mastery_buy`, `mastery_refund` | item 7 |
| `api_capture_tests.rs` `round6b_assist_emits_dispatch_and_server_phases_via_play` | dispatched result `ok == true` | `ok == nil` | item 6 |

## 5. Results

All with the disc linked (`run/eqoa.iso`, `EQOA_ISO`), through `tools/run_gates.py` and the shared lock.

- Reproduction (tests only, base code): `--only round --filter round7h_`: 2 passed, 11 failed, each for its
  reason (section 1).
- After the fixes, `--only round,addons --filter play::fork_hooks round7h_`: round 225 passed, 2 failed;
  addons 684 passed, 4 failed, 25 ignored, 11 SKIP lines (symlink privilege, Forge pinned references, Wereoxxs
  recordings, picture goldens, the mod-api adapter). The six failures: my own quoting bug in the item 5 test;
  the support table (regenerated at the end); and three existing assertions items 6 and 7 changed (section 4),
  plus the `not_yet` reason rule. Fixed in d2e50016; rerun of those six and all `round7h_`/`round6d_`:
  **28 passed, 0 failed**. In that run: zero-addon gate `allocs/frame=0.0`, `allocs/menu_frame=0.0`,
  `allocs/event=0.0`; the bundled UI tests (Wereoxxs, ARPG, Modern UI, hearts, Bag Viewer's manager tests) all
  passed; the GPU test `round7c_manager_events_come_from_their_production_callers` ran here (not skipped) and
  passed with the new `eqoa.textures.rgba` assertion.
- After regeneration (96d89f85): `--only round,wasm,footprint,paths --filter docs_ round7b_support_table
  round7h_`: 19 passed, 0 failed, 0 SKIP; wasm, footprint, paths PASS.
- Final focused run, `--only round,wasm,footprint,paths --filter round7`: **192 passed, 0 failed, 0 ignored,
  1 SKIP line** (`round7b_without_adapter_is_an_explicit_skip`: the mod-api reference test's own
  no-adapter path); wasm, footprint, paths PASS.
- Footprint: PASS, no budget change; no file outside `crates/eqoa/src/fork_ext/` and `docs/` changed on this
  branch (no upstream line).
- **GitHub CI run 44900190950 at 96d89f85: success.** addons 688 passed / 0 failed / 25 ignored / 17 SKIP;
  full 3336 passed / 0 failed / 37 ignored / 378 SKIP; wasm, footprint, paths success; player-release
  20 passed; tools 201 passed / 1 SKIP. CI has no disc and no GPU; its SKIPs are those. `ci_gates.py cleanup`
  run.
- Release build: `--only release --release` PASS; `SUPPORT-TABLE.md` and `eqoa-1-surface.json` regenerated
  from it in the separate commit 96d89f85. The diff is exactly: `eqoa.abilities.scope`, `eqoa.entities.status`,
  `eqoa.options.Button` and its three fields, 14 `ImDrawList.*`, three actions `supported` → `not_yet`, and the
  date. `docs/modding/eqoa-1-frozen.json` not created or touched.

What a player or a bundled mod notices: nothing. No bundled mod calls `follow`, `cycle_target`, `drop_quest`,
the mastery actions, `get_channel`/`get_counters`, or binds one id twice; Bag Viewer reads `state` before `ok`;
no bundled Forge UI calls any `eqoa.actions.*`. Callers checked by `grep` of `bundled-mods/**` and `mods/**`:
Bag Viewer (`open_loot`, `talk`, `batch`, `loot`, `loot_all`, `cancel`, `stop_batch`, `bank_move`, `use_item`,
`equip`, `quick_slot`, `sell`; reads `ok` in `bags.lua` `on_action_result`/`on_batch_result` and `main.lua`),
Stats Box (`toggle_stats_box`, one `input.bind`), Target of Target (`assist`), Map Search (two `input.bind`
ids), Wereoxxs/ARPG/Modern UI (Forge only). A mod author notices the changes listed in section 1.

## 6. For the lead (not done here)

- **`sell` skips the game's confirmation.** The owner's rule (an irreversible step keeps its confirmation) and
  the game's merchant (`merchant.rs` `sell`: asks before selling a quick-slotted, no-trade or non-misc item)
  say `eqoa.actions.sell` should not sell those without the player's answer. As built it does (a worn item is
  refused). It was not changed: the brief's binding decision covers four actions, and gating `sell` would
  change Bag Viewer's Sell 1 / Sell stack and its junk sale (player-marked junk). The spec lists it under
  "Under review, not part of the promise", so it is not frozen as "sells without asking".
- **`dialog_answer` (and `activate_row` on a dialog) answer the game's own Yes/No for the player.** They need
  their own grant, shown as "Can answer NPC dialogs". Under the owner's rule the confirm contract should tie a
  mod-drawn confirm to the player's own answer; today the grant is the only tie. Not verified: whether chained
  window actions on the item page can open an item's popup, reach Destroy and answer its box (that would reach
  a step `destroy` refuses). Worth a test before the freeze.

## 7. Not verified

- Nothing was played; the changes run in tests built from `Play` state, not in a live client.
- The decomp and eqoa-port oracle are not on this machine. "Does the game ask" was read from the upstream port
  (`questlog.rs`, `classmastery.rs`, `spellbook.rs`, `merchant.rs`, `itemmenu.rs`), which cites the decomp; not
  checked against the decomp itself.
- Whether window actions can be chained on the item page to reach Destroy and its box (section 6).
- The freeze guard's own `ChatHistory` mutation was not repeated; the explanation in section 3 is from reading
  `host.rs`.
- The report itself was committed after the regeneration commit (docs only, no code), so the regeneration is
  the last code-relevant commit, not the last commit.
