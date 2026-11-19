# Round 7 Part A: audit fixes

Branch `feat/eqoa-api-r7a`, base `9ea6b435`. Implementation and regression controls
are complete. Validation evidence and the mandatory post-commit gate command are
recorded below. No push, merge, tag or rebase.

Missing services now refuse visibly; excluded Forge modules load guarded tables;
file listing is implemented; map placeholders and capability claims are explicit.
Two audit claims did not reproduce: real addon texture installation already works,
and the native chat consumer lease has no emulator-cave suspension state.

## Scope and reference

All implementation is in the brief's allowed fork-owned addon files. The user also
explicitly approved the test-only assertion corrections listed below, including
one assertion in protected native_tests.rs and the stale hint assertion in ../items_tests.rs. No upstream changes, seed-table changes,
round-6.5 implementation changes, bundled asset edits or shared-budget increases.

Pinned reference, read outside this worktree (portable exact location):
`%USERPROFILE%/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/modules/frontiers_forge/`.

- `icon.lua:330-354,431-443`: failed texture lookup returns **one nil**; success is texture,width,height.
- `camera.lua:6-15`: x/y/z table; no missing-camera case is defined. Keep the shape and diagnose unavailable.
- `chat.lua:567-569`: suspension is a wanted emulator hook detached outside the game. Native leases never detach a code cave.
- `quest.lua:20-35`: new/GetName wrap raw pointers.
- `cave.lua:69,167,196,204,216,230,237`: IsRegionZero, Alloc, Reserve, Get, Free, Release, Info.
- `util.lua:400-403`: GetWorldId; native adapter uses the real world ID and nil outside a world.

## Changes by file

| File under crates/eqoa/src/fork_ext/addons | Change |
|---|---|
| icons.rs | Base texture functions diagnose unavailable and retain pinned nil failure arity; removed the base stderr warning. Original source format is unsupported because SafeLibrary/IconProvider retain only decoded RGBA. Production-addon texture/GPU regressions. |
| native.rs | hidden_by refuses unavailable without a visibility lease; input placeholder refuses until replaced; fonts.game refuses unsupported. Removed dead client.time arity name. Temporary unsupported hand list covers quest/cave and every member, plus source_format. |
| host.rs | Camera missing-state diagnostic, native GetWorldId, bit descriptor registration. Only the permitted camera/util/bit regions changed. |
| chat.rs | Reference comment explains why IsSendHookSuspended=false is truthful; no behavior change. |
| package.rs | Guarded quest/cave tables with per-call and unknown-member diagnostics; util joins host registry; updated old require expectation; production-install regression tests. |
| bit.rs | Eight supported descriptors and visible unknown-symbol lookup. |
| file_io.rs | Sorted immediate-child FileEntry listing, private/resource overlay resolver, tombstones, * and ? filename globs, relative paths, no-link/reparse guards, 1,024-entry cap with explicit quota refusal. |
| map_views.rs | images/load_image unsupported: replaced by eqoa.map.view; waypoint pair not_yet: not built; use eqoa.map.set_guide. |
| native_items.rs | Remove stale button_hint not_yet override and explicitly declare the populated field supported. |
| items_forge.rs | Correct implemented hint/action header and empty DEFERRED comment. |
| health.rs | Extend an existing full player table instead of replacing it; retain standalone health subset; approved assertion correction. |
| native_tests.rs | Only the user-approved hidden_by refusal assertion changed. |
| ../items_tests.rs | Only the user-approved stale hint comment and supported expectation changed. |
| round7a_collect_diagnostics.py | Fail-closed before/after collector for the existing 12-package Manager diagnostic test. |

Remaining install placeholders were reviewed: player.get/hud.hide already refuse
not_in_world; client.get describes a real pre-world state; input.get's invented
placeholder became unavailable (the production input installer replaces it in the
same install). No other invented placeholder answer remains in install.

## Reproduction table

All names below are new `round7a_` tests. Tests were written and executed against the
baseline before production fixes. A passing baseline finding received no behavior fix.

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
| MerchantState.button_hint stale not_yet | round7a_merchant_hint_supported | yes | 9f1b453f, b9ebaef2 |
| Health-last install drops full player API | round7a_player_install_order_retains_members | yes | 9f1b453f, b9ebaef2 |

Additional independent tests: round7a_bit_descriptors_registered,
round7a_forge_util_registered, round7a_refusing_forge_members_diagnose,
round7a_stale_arity_name_removed, round7a_item_header_matches_implemented_hint.
The last two check source-only stale claims, after starting a production addon host.

The audit's claim that install_fields already declares button_hint was also wrong:
deleting its not_yet override left it unknown. The first post-fix test exposed this;
its owning installer now declares it supported (b9ebaef2).

The file fixture covers private files, resource overlay precedence, tombstones,
nonrecursive directory records, both glob operators, sorted paths, sibling package
isolation, traversal/absolute/glob-path refusal, exactly 1,024 succeeding and 1,025
failing quota. Player install order compares the complete member set both ways.

## Executed negative controls

Command for every Rust row below (same shared target, worktree disc and scratch config):
`python tools/run_gates.py --only round --filter round7a_ --timeout 60`.

Control 1 restored baseline implementations while retaining the new tests: 2 passed,
12 failed, 0 ignored, 0 SKIP lines. The complete control additionally made chat
suspension true and made attach_icon_cache return before installing real functions:
**0 passed, 19 failed, 0 ignored, 0 SKIP lines**. Every mutation was restored.
These are observed failing locations in the mutated build, not current line numbers.

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
| round7a_stale_arity_name_removed | `package.rs:1290:9:`: assertion excluding the stale client.time arity name failed |
| round7a_util_and_bit_capabilities | `package.rs:1242:5:`: Lua nil-call / missing member |

The diagnostic collector's positive command:
`python crates/eqoa/src/fork_ext/addons/round7a_collect_diagnostics.py run/round7a-baseline.log <shared-target>/gates/feat-eqoa-api-r7a-addons.log`
reported twelve empty lists before and after. A copied after log was deliberately
marked FAILED; the same collector with `run/round7a-bad-diagnostics.log` as its after
argument exited 1 at line 24:
`ValueError: bundled diagnostic gate did not run all packages successfully`.
Absent, ignored and SKIP gates are also rejected; it cannot infer empty diagnostics
unless the existing Rust gate proved every package ran without any.

## Changed existing tests (explicitly approved)

- package.rs reserved-module test: quest require now succeeds and new(0) refuses per call, instead of require throwing.
- native_tests.rs native_pregame_getters_do_not_fabricate_world_or_hud_claims: only the empty hidden_by table assertion becomes nil,Error(unavailable). User approved this protected-file exception.
- items_tests.rs round5_capabilities_list_every_new_function_and_say_what_is_missing: only the stale button_hint comment and not_yet/round-6 expectation become supported. The user explicitly approved this second outside-allowlist test exception after the full suite exposed it.
- health.rs forge_health_adapter_unsupported_apis_absent: GetName/GetLevel/GetTotalStr must remain functions after health installs over the full production player module. Existing health/UI assertions remain.

No old test was deleted, newly ignored, or bypassed. A standalone health-only adapter
still has its historic subset. The file fixture asserts the existing unsupported filesystem contract on non-Windows
instead of skipping; the positive filesystem cases run on Windows. The large listing assertion uses Host::run, preserving
the real callback instruction-budget reset, limit and assertions. Raw chunks had
incorrectly accumulated setup/earlier assertions against that budget; it was not raised.

## Bundled diagnostics before/after

Collected through every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics:
it runs each package alone with required resources/journal, retains all diagnostics,
asserts all twelve ran, and fails on any diagnostic. The collector records that proof.

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

## Validation history and final gates

| Completed run | Passed | Failed | Ignored | SKIP lines |
|---|---:|---:|---:|---:|
| First reproduction | 1 | 11 | 0 | 0 |
| Expanded reproduction plus bundled gate | 3 | 12 | 0 | 0 |
| First post-fix focused plus bundled gate | 13 | 2 | 0 | 0 |
| Corrected focused plus bundled gate | 15 | 0 | 0 | 0 |
| Preliminary expanded focused gate | 18 | 1 | 0 | 0 |
| Preliminary addon suite | 511 | 3 | 25 | 12 |
| Isolated rebuilt 1,024/1,025-entry fixture | 1 | 0 | 0 | 0 |
| Optimized zero-addon allocation gate | 1 | 0 | 0 | 0 |
| Final expanded focused gate | 19 | 0 | 0 | 0 |

The initial focused failures were Windows directory locking and the missing supported
button_hint field declaration; both corrected. Preliminary suite failures were the
two obsolete assertions above and the cumulative-budget fixture. Its full-bin run
was superseded after approval because its binary predated the assertion corrections;
it is not counted as a completed full suite. Only that runner's verified process tree
was stopped; its lock was removed after its owner PID was confirmed dead.

A sandbox-only direct fixture attempt refused writes (environment failure, not a
product result). The rebuilt executable with normal gate access passed the complete
listing fixture in 18.22s. No new round7a test printed SKIP.

Optimized command (shared lock/cache, scratch config, worktree disc, static CRT):
`cargo test --release --locked -p eqoa --bin eqoa play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound -- --exact --test-threads=1 --nocapture`.
Allocations: world frame 0, menu frame 0, entity packet 0, chat 0; full native views 0.
Its deliberately allocated 4 KiB control was observed. Measured world/menu frame times
5.044/4.925 us; packet 0.088 us; chat 0.011 us (this run, not a performance promise).

Footprint check passed against unchanged reviewed base
`a76d8d27a1c3f51fca545d7eabaca9e5dc336091`; no budget raised. Tracked-path check passed.
Only the mandated shared Cargo target was used; no target folder was created here.

Completed comprehensive gate summary, before the last approved stale-test correction:

| Suite | Passed | Failed | Ignored | SKIP lines |
|---|---:|---:|---:|---:|
| round | 19 | 0 | 0 | 0 |
| addons | 513 | 1 | 25 | 9 |
| full | 2394 | 1 | 30 | 50 |
| Focused corrected hint plus unchanged timing benchmark | 2 | 0 | 0 | 0 |

WASM, footprint, paths, and player-release passed. The full suite's sole failure
was the stale button_hint not_yet expectation; the user approved its correction,
and the focused check above passed after that correction. Existing prerequisite
SKIP reasons include unavailable symlink privilege, pinned Forge fixtures,
reviewed UI goldens, client.bin, live S2/save-state fixtures, and the separate 2002
disc. No new round7a test skips and no ignored-test list changes were introduced.

The final commit is validated with
`python tools/run_gates.py --filter round7a_ --release --timeout 60`, using the
mandated shared target, shared lock, worktree disc and scratch configuration.
Post-commit results are written by that runner to
`<shared-target>/gates/feat-eqoa-api-r7a-summary.md` and its per-step logs. This
tracked report records completed runs only; the final response gives the actual
post-commit result rather than predicting counts here.

One rebuilt addon run completed with 513 passed, 1 failed, 25 ignored and 9 SKIP
lines. Its only failure was the pre-existing
round_3_all_four_mods_per_frame_performance_benchmark: 5.164 ms/frame against the
5 ms debug limit (round_3_picture_tests.rs:976). New regressions and the approved
old assertions passed. No benchmark threshold, skip, or tested implementation was
changed. The subsequent isolated rerun, together with the corrected hint test,
passed: 2 passed, 0 failed, 0 ignored, 0 SKIP. The final post-commit run repeats the
whole addon suite, including this benchmark.


## Deliberate limits and handoff

fonts.game cannot expose the game's bitmap font through the existing ImGui TrueType
service without an upstream change, so it refuses. Original icon encoding is not
retained by the decoder service and remains an unsupported field. Map image names
are replaced by map.view; personal waypoint names remain unbuilt with honest not_yet
refusals as instructed. These are completed refusal contracts, not silent placeholders.

Action policy, seed/round labels, capability-system redesign, spec/author docs,
release notes and bundled content belong to round 6.5/Parts C/D and were not edited.
No release was published and no owner playtest/visual-golden acceptance is claimed.
The shared experience log was left to the lead under this brief's worktree-only rule.
