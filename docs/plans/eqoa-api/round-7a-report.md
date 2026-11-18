# Round 7 Part A audit fixes

Branch: `feat/eqoa-api-r7a`, base `9ea6b435`. Work remains in progress.

## Reproduction

First run: `python tools/run_gates.py --only round --filter round7a_ --timeout 60`:
1 passed, 11 failed, 0 ignored, 0 SKIP lines. The passing chat test confirms the
native hook has no emulator suspension mechanism. All eleven failures are assertions
or Lua calls exposing the audited missing/refusing behavior, not compilation errors.
The follow-up baseline includes the two GPU/icon tests and all twelve bundled mods.

## Boundaries

No upstream changes, seed-table changes, round-6.5-owned implementation changes,
bundled asset changes, or shared-budget increases. The release notes and author docs
are explicitly reserved for the lead/Part D. No push, merge, tag or rebase.

## Reference

Pinned read-only reference: `%USERPROFILE%/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/modules/frontiers_forge/`.
Camera: `camera.lua:6-15` returns an x/y/z table and defines no missing-camera case.
Chat: `chat.lua:567-569` defines suspended as wanted but detached outside the game;
the native adapter owns a consumer lease, never an emulator patch/cave.
Quest: `quest.lua:20-35` exposes `new` and `GetName` over raw pointers.
Cave: `cave.lua` exposes IsRegionZero, Alloc, Reserve, Get, Free, Release and Info.

## Validation still pending

All fixes, executed mutation controls, bundled before/after diagnostics, full gates,
release build, footprint and final counts.

Follow-up baseline: 3 passed, 12 failed, 0 ignored, 0 SKIP lines.
The production addon icon test and all twelve bundled mods passed; each bundled
mod produced zero manager diagnostics. The source-format descriptor failed as expected.

## Step 2: quiet placeholders

The base icon bindings now diagnose `unavailable` through the native sink and keep
Forge's nil texture failure shape. The one-off stderr warning is removed.
`hud.hidden_by` refuses without its visibility lease. The remaining install placeholders
were reviewed: player.get/hud.hide already refuse not_in_world; client.get describes a
true pre-world state; input.get's invented state is replaced by an unavailable refusal
(the production input installer replaces it later in the same install).

## Step 3: partial answers

`icons.rs` describes source_format unsupported: SafeLibrary/IconProvider retain only
width, height and decoded RGBA bytes, so the original disc encoding is unavailable.
Accessing the unsupported record field uses the existing guarded-record diagnostic.
`host.rs` diagnoses absent camera state while retaining the pinned table shape.
`chat.rs` records why false is real: the native consumer lease never detaches an
emulator code cave. Its test passed before changes; no behavior fix was applied.

## Step 4: guarded APIs

`package.rs` registers quest/cave diagnostic tables, including every pinned public
function and guarded unknown members. The existing reserved-module test now expects
quest require to succeed and new(0) to refuse; this is the one changed old assertion.
`host.rs` exposes GetWorldId from the native world state and registers util via the
package registry; `bit.rs` guards unknown keys and describes all eight functions.
`fonts.game` refuses: the existing ImGui TrueType atlas service cannot expose the
native bitmap font without an upstream change (excluded by this brief).
Per-member unsupported descriptors are closed in step 7's required hand list.

## Step 5: promised names

`file_io.rs` implements immediate-child filename-glob listing through the existing
mount resolver, private overlay precedence and tombstones. Directory and child
handles use the existing no-link/reparse guards. Relative UTF-8 paths, sorted
FileEntry records and the 1,024-entry refusal are explicit; enumeration never
reports a truncated list as complete. `map_views.rs` refuses images/load_image as
replaced by map.view and keeps the unbuilt waypoint pair not_yet with set_guide advice.

## Step 6: stale claims and install order

Removed MerchantState.button_hint's stale not_yet override and the dead client.time
arity name. Updated the item adapter header to describe the implemented hint and
native action service. The health adapter extends an existing player table, so installing health last cannot
drop GetName and the rest of the full API. A standalone health-only installer keeps
its historic two-member subset; the full player installer can still follow it. Both orders are tested.

## Step 7: capability hand list

The temporary annotate_host hand list explicitly marks quest/cave and all nine
members unsupported with the raw-memory exclusion reason. The icon source-format
field is also honest on hosts lacking the texture service. Camera and icon texture
functions remain supported services whose missing state diagnoses unavailable;
marking them unsupported globally would contradict the production addon test.

## Reproduce table

All test names below have the `round7a_` prefix (shown in full). Baseline was the
production source at 9ea6b435, with regression tests committed at 30b6ff42.

| Finding | Test | Reproduced | Fix commit |
|---|---|---|---|
| Base GetTexture/GetUITexture answer without diagnostics | round7a_icon_placeholders_diagnose | yes | 6e3a09ac |
| Real addon GetTexture supposedly always nil | round7a_real_addon_texture_install_replaces_placeholders | no; real texture returned | no behavior fix |
| hud.hidden_by without lease looks like empty owners | round7a_hidden_by_refuses_without_lease | yes | 6e3a09ac |
| source_format claims supported but original encoding is absent | round7a_icon_source_format_describes_decoder_limit | yes | 0323f766, af3a5367 |
| Camera missing state answers zero silently | round7a_camera_missing_diagnoses | yes | 0323f766 |
| Send hook suspension constant allegedly fake | round7a_chat_native_hook_has_no_emulator_suspension | no; no native cave detachment state | comment in 0323f766 |
| quest/cave require throws instead of guarded refusals | round7a_quest_and_cave_guarded | yes | ace06df7, af3a5367 |
| util missing in capabilities/GetWorldId absent | round7a_util_and_bit_capabilities | yes | ace06df7 |
| bit descriptors missing | round7a_util_and_bit_capabilities | yes | ace06df7 |
| unknown bit member has no diagnostic | round7a_bit_unknown_diagnoses | yes | ace06df7 |
| fonts.game is a raw nil-call error | round7a_game_font_refuses | yes | ace06df7 |
| files.list missing | round7a_files_list_overlay_and_bounds | yes | 8489bd17 |
| map image/waypoint names give stale generic refusals | round7a_map_refusals | yes | 8489bd17 |
| MerchantState.button_hint stale not_yet | round7a_merchant_hint_supported | yes | 9f1b453f, follow-up field declaration |
| Health-last install drops full player API | round7a_player_install_order_retains_members | yes | 9f1b453f, follow-up preserving standalone subset |

The audit also incorrectly claimed install_fields already declares button_hint. It
did not: deleting the stale override left the descriptor unknown, exposed by the
first post-fix run. The field is now explicitly included by its owning installer.
No gesture policy, capability architecture, round labels or author docs were changed;
those audit rows belong to round 6.5/Parts C/D and are excluded by this brief.

## Interim validation

First post-fix focused + bundled run: 13 passed, 2 failed, 0 ignored, 0 SKIP lines.
Failures: Windows directory handles needed the directory-specific parent lock;
button_hint required its supported field declaration. Corrected run: 15 passed,
0 failed, 0 ignored, 0 SKIP lines. The bundled-session test passed before and after:
Wereoxxs UI, ARPG UI, Avoids Modern UI and all nine eqoa_* packages each produced
zero manager diagnostics (12 before / 12 after). No bundled files were edited.

Pinned texture failure arity is one nil (`icon.lua:330-354`, `431-443`), success is
texture,width,height. The base adapter now matches that arity exactly, with a
regression assertion. `util.lua:400-403` is the GetWorldId reference.

## Executed negative controls

Command for every row below:
`python tools/run_gates.py --only round --filter round7a_ --timeout 60`
(with the mandated shared target, scratch config and worktree disc).

The first control restored baseline implementations while retaining current tests:
2 passed, 12 failed, 0 ignored, 0 SKIP lines. The complete second control additionally
made chat.IsSendHookSuspended return true and made attach_icon_cache return before
installing its real functions. It also ran independent bit/util metadata checks and
source-hygiene tests. Result: **0 passed, 19 failed, 0 ignored, 0 SKIP lines**.
All deliberate mutations were restored immediately after the run.

| Test (same command above) | Observed failing line in mutated build |
|---|---|
| round7a_icon_source_format_describes_decoder_limit | `icons.rs:1755:9:`: assertion `left == right` failed |
| round7a_real_addon_texture_install_replaces_placeholders | `icons.rs:1726:9:`: assertion `left != right` failed |
| round7a_chat_native_hook_has_no_emulator_suspension | `package.rs:1473:174:`: Lua assertion / missing guarded API |
| round7a_files_list_overlay_and_bounds | `package.rs:1454:14:`: Lua list is nil |
| round7a_player_install_order_retains_members | `package.rs:1401:13:`: health_first=false |
| round7a_bit_descriptors_registered | `package.rs:1276:5:`: Lua assertion / missing guarded API |
| round7a_bit_unknown_diagnoses | `package.rs:1357:9:`: [] |
| round7a_camera_missing_diagnoses | `package.rs:1342:9:`: [] |
| round7a_forge_util_registered | `package.rs:1281:5:`: Lua assertion / missing guarded API |
| round7a_game_font_refuses | `package.rs:1253:5:`: Lua nil-call / missing member |
| round7a_hidden_by_refuses_without_lease | `package.rs:1224:5:`: Lua assertion / missing guarded API |
| round7a_icon_placeholders_diagnose | `package.rs:1323:219:`: Lua assertion / missing guarded API |
| round7a_item_header_matches_implemented_hint | `package.rs:1296:9:`: assertion failed: !include_str!("items_forge.rs").contains("says it is not built (`DEFERRED`)") |
| round7a_map_refusals | `package.rs:1259:5:`: Lua assertion / missing guarded API |
| round7a_merchant_hint_supported | `package.rs:1269:5:`: Lua assertion / missing guarded API |
| round7a_quest_and_cave_guarded | `package.rs:1230:5:`: require throws unsupported FrontiersForge API |
| round7a_refusing_forge_members_diagnose | `package.rs:1309:20:`: require throws unsupported FrontiersForge API |
| round7a_stale_arity_name_removed | `package.rs:1290:9:`: assertion failed: !include_str!("native.rs").contains("| \"eqoa.client.time\"") |
| round7a_util_and_bit_capabilities | `package.rs:1242:5:`: Lua nil-call / missing member |

The source-hygiene tests cover the dead client.time arity name and stale item-header
claims, which have no distinct runtime behavior to exercise. Both start from a
production addon host before checking those source-only claims.

## Source-only audit rows and test boundaries

`round7a_stale_arity_name_removed` reproduces the dead guard name and
`round7a_item_header_matches_implemented_hint` reproduces the stale header claims;
both failed under the executed complete negative-control command above. Independent
`round7a_bit_descriptors_registered` and `round7a_forge_util_registered` avoid one
missing member masking another capability assertion. Every newly refusing quest/cave
member and unknown member is checked against the native diagnostic sink by
`round7a_refusing_forge_members_diagnose`.

The file fixture covers private files; shared-resource overlay precedence; tombstones;
nonrecursive directory records; filename * and ? globbing; sorting; another package's
files remaining unreachable; relative-only paths; forbidden glob paths; exactly 1,024
entries succeeding; and 1,025 failing quota. The 1,024-entry assertion runs through
Host::run so the callback gets the production instruction-budget reset; the limit
and assertions are unchanged. The earlier raw-chunk fixture exhausted its cumulative
budget after setup and was corrected rather than raising the budget.

A standalone health-only adapter retains its original subset, while the new install-order
test compares the complete player member set in both orders. Changed old assertions
are listed in the approved test-only scope exception below; no tests were deleted or ignored. Formatting was applied only to changed fork-owned regions.

No implementation edits were made to run data, bundled assets, seed tables, protected round-6.5 files,
shared upstream code, spec/author docs, release notes or footprint budgets.

## Bundled diagnostics comparison

Collected with the existing `every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics`
test before changes and after the implementation. It runs each package alone (with
its required shared resources/journal), checks all twelve ran, and retains every
manager diagnostic for the final assertion.

| Package | Before | After |
|---|---:|---:|
| wereoxxs_ui | 0 | 0 |
| arpg_ui | 0 | 0 |
| avoids_modern_ui | 0 | 0 |
| eqoa_bag_viewer | 0 | 0 |
| eqoa_creature_journal | 0 | 0 |
| eqoa_example_hearts | 0 | 0 |
| eqoa_hud_numbers | 0 | 0 |
| eqoa_mana_bars | 0 | 0 |
| eqoa_map_search | 0 | 0 |
| eqoa_round_minimap | 0 | 0 |
| eqoa_stats_box | 0 | 0 |
| eqoa_target_of_target | 0 | 0 |

## Approved test-only scope exception

The user explicitly approved these two test-only updates after the first full-addon
run exposed old expectations incompatible with Part A:

- `native_tests.rs`, native_pregame_getters_do_not_fabricate_world_or_hud_claims:
  replace only the empty hidden_by table assertion with nil,Error(unavailable).
  No round-6.5 implementation or other lines in this protected file were changed.
- `health.rs`, forge_health_adapter_unsupported_apis_absent: require GetName,
  GetLevel and GetTotalStr to remain functions after health installs over the
  production full player module. Existing health/UI assertions remain intact.

These are contract corrections, not skipped tests or disabled assertions. Along with
the package require assertion, they are the complete list of changed old tests.
The initial full-addon run was 511 passed, 3 failed, 25 ignored: the two old assertions
above and the now-corrected cumulative-budget file fixture. Final gates must rerun them.

## Additional focused verification

The rebuilt test executable was run directly with
`round7a_files_list_overlay_and_bounds --test-threads=1 --nocapture`, a scratch
EQOA_CONFIG and the same filesystem access as the gates: **1 passed, 0 failed,
0 ignored, 0 SKIP lines** (18.22s). A sandbox-only attempt refused its fixture
writes; it is recorded as an environment failure, not a product result.

`python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091`
passed against the unchanged reviewed policy. Shared budgets were not raised.
`python -X utf8 tools/check_no_local_paths.py` also passed. The reference path above
uses %USERPROFILE% so the required external reference is reproducible without a
tracked personal-profile path. No disc path was recorded.

## Diagnostic collector

`round7a_collect_diagnostics.py` consumes the existing Rust Manager gate logs. It
requires exactly one passed gate, rejects failed/ignored/SKIP results, and records
that all twelve per-package diagnostic lists are empty only after that assertion.

Positive command:
`python crates/eqoa/src/fork_ext/addons/round7a_collect_diagnostics.py run/round7a-baseline.log <shared-target>/gates/feat-eqoa-api-r7a-addons.log`
reported twelve empty lists before and twelve after.
Negative control: a copied after log marked that gate FAILED, then
`python crates/eqoa/src/fork_ext/addons/round7a_collect_diagnostics.py run/round7a-baseline.log run/round7a-bad-diagnostics.log`
exited 1 at collector line 24: `ValueError: bundled diagnostic gate did not run all packages successfully`.

The preliminary full-bin run was stopped after the approved test-only changes were
committed, because its binary predates those assertions. It is not counted as a
completed full suite. Only its own verified runner process tree was stopped; its
single-token lock was removed after its owner PID was confirmed dead. Final gates
will run the complete set on the committed code; no tests are excluded to mask failures.
