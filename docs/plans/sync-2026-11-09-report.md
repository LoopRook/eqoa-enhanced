# Upstream sync 2026-11-09

Status: complete (sync finishing work and verification recorded; the full workspace gate retains two confirmed upstream AO failures). Merge `3307f50` (LoopRook), validation checkpoint `ee7ffa6`. Branch `sync/2026-11-09`; original fork HEAD `4ebe0f7`; pinned merge target `91f9e72dd6d111e20a37b73cea461444a18ad79c` (134 upstream commits). No push, tag or protected-branch merge.

## Hand resolutions

- `crates/eqoa/Cargo.toml`: Union dependencies, retaining ab_glyph and upstream additions.
- `crates/eqoa/src/bindpage.rs`: Keep upstream attack-G capture regression.
- `crates/eqoa/src/binds.rs`: Keep upstream Space/V jump and G attack, use O group waypoint, retain F2/F4 and Stats Box; preserve saved-layout migration.
- `crates/eqoa/src/groupmark.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/hud.rs`: Retain both HUD features and upstream minimap/tracker; remove duplicate enum members.
- `crates/eqoa/src/main.rs`: Retain Stats Box alongside upstream module registrations.
- `crates/eqoa/src/map/build.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/chrome.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/community.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/group_waypoint.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/markers.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/mini.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/mod.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/store.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/tests.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/map/tool.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/mapsuggest.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/options.rs`: Retain Lighting IDs 52-57 and Texture Folder; fork-owned rows preserve existing IDs; masters use 1054/1055.
- `crates/eqoa/src/play.rs`: Union upstream cape/race/inventory/event logic and fork API hooks, search/journal/HUD features; retain upstream master-switch tests.
- `crates/eqoa/src/settings.rs`: Union lighting and master settings with fork options, preserving every existing saved key; remove duplicate restored declarations/serialization.
- `crates/eqoa/src/touch.rs`: Retain upstream touch behavior and fork dpad suppression; remove duplicate holds initializer.
- `crates/eqoa/src/tracker.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/viewer.rs`: Retain upstream camera/AO/render-scale paths plus addon/report/search hooks; add offered GM Ctrl-click modifiers and focus reset.
- `crates/net/src/session.rs`: Retain upstream network behavior and quest negotiation plus fork mana bars; add offered GM travel protocol 0x7E34/35.
- `crates/render/src/lib.rs`: Upstream's verbatim (no fork edits; the circular map renderer lives in `render/src/map.rs` and `ui.rs`, which do differ), retaining upstream renderer exports and scaled capture callback.
- `crates/render/src/map.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/render/src/map.wgsl`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.

`README.md` and `CLAUDE.md` remain the fork versions; `UPDATE-NOTES.md` is upstream's. The script refreshes `docs/upstream/CLAUDE.md` at the pinned target. Only the owner's later shared-target build rule is updated in the fork CLAUDE file.

## Map source audit and ports

Verified `git log origin/master -- <map paths>` and actual patches for `65f9b13`, `6c5bb03`, `8003083`, `53c5cf5`, `476fdac`. Upstream's restore comes from the pre-deletion map at `6fc3ec6^`, also the fork's starting point. Three-way comparison against that common source preserved the fork versions and exposed actual upstream changes. `origin/master` is newer (`14636b2`); it is not silently merged. The explicitly requested later map refinements are ported separately on the pinned merge.

Taken:
- Map/Quest Helper masters: on by default, persisted as upstream's `map`/`quest_helper` keys; map disables compass/input/search, minimap, tile/cache work and releases GPU/world resources; quest negotiation remains silent if disabled before the first request and stops an existing request. The tracker can work with Map off. Existing subordinate choices survive master toggles. Masters have fork IDs 1054/1055 on a separate six-row-safe Map page.
- GM travel: offered 0x7E34 capability, one-shot Ctrl/Cmd click 0x7E35 world/x/z request, upstream legend and tests; focus loss clears Ctrl; server retains authorization. Play invokes travel through the fork-owned group adapter.
- Cache precedence: `EQOA_MAP_CACHE`, then `EQOA_CACHE/map`, then platform cache. Journal handling remains fork-owned.
- Quest rim: only own steps (`?`) remain on the circular rim; distant offers (`!`) disappear; steps draw over offers on both maps.
- Controls: O group waypoint, G attack; retain upstream Space/V jump and old-key/macro migration, plus fork F2/F4 search/stop.
- Additional applicable restore fixes: explicit window/target dimensions in map/minimap drawing and scaled capture callback; keep circular masks scaled to the render target. Map tile capture sets the new lighting fields, including `lights: None`. Upstream master-off, quest-negotiation, binding/macro, GM protocol and click tests retained/adapted.

Left/adapted deliberately:
- Upstream square minimap, red theme and game-font chrome replaced by the existing fork circular/theme implementation; fork render-scale/circle regressions retained.
- Upstream map rows 41-45 are represented by fork-owned rows instead: upstream 42 collides with existing Mana Bars 42. No existing fork IDs or saved keys are renumbered. Lighting IDs 56/57 remain upstream's.
- Upstream tests claiming Stats Box/Mana Bars/Target of Target no longer exist are inapplicable to Enhanced, which intentionally retains them. Corresponding fork persistence/input tests remain.
- Upstream edit-box-only suggestions remain as the controller flow; the fork in-map mouse/keyboard form stays.
- Later non-map commits after the pinned target are outside this merge.
- Upstream CLAUDE delta reviewed: preserve smoothed `lights::Moment`/`Chooser`, offered jumps/capes/custom races/inventory sizes, and support-petition bounds. Existing fork arithmetic/layout, release, privacy and addon instructions remain in force; no rewrite of the fork CLAUDE file was needed.

## Budgets and validation

Reviewed maxima vs old: HUD +316/-15 -> +316/-44; options +41/-41 -> +46/-142; Play +759/-53 -> +759/-89; settings +334/-23 -> +334/-38. Upstream restored map code is now baseline, so existing fork replacements count as deletions; options removes conflicting stock 41-45 rows/arms and moves the map/UI walk to owned tests, HUD retains circular hit tests/gauges/claims, Play retains search/journal/dot payload adaptations and every API seam, settings replaces vanilla-only assertions that Enhanced options disappeared. All reasons are recorded per file in `fork-footprint.json`; no renderer or net budget increase. Texture-pack test-only +1/-1 is a newly reviewed shared path; options hook count grows by two for the combined-registry test. New Map category hook is guarded.

Scripted rerun (`python tools/sync_upstream.py --continue --tests-only`): client 1797 passed / 0 failed / 29 ignored; net 63 passed, plus 9 integration tests; total 1869 passed / 0 failed / 29 ignored. There were 176 lines containing SKIP. The initial run was 1782 passed / 13 failed / 29 ignored (client stopped cargo before net); its failures exposed the merge issues below and are not counted as final passes. Explicit tools command `python -X utf8 -m unittest discover -s tools -p "test_*.py"`: 94 passed.

Corrections driven by that failing run:
- Keep upstream's single table-driven map/quest serialization, remove duplicate restored branches; existing round-trip/one-key tests caught the duplication.
- Scaled map captures pass logical `map.size` and actual target size separately; the existing 1x/2x/4x/2x/1x pixel tests failed before and pass after.
- Jump migration tests use free Y instead of fork-owned Stats Box B; production bindings remain unchanged.
- The new stock unique-row test sees the combined registry, and retail-boundary option tests explicitly select 24/100 under Enhanced defaults.
- Additional shared file `crates/eqoa/src/texpack.rs`: one-line Windows absolute-path fixture correction; no production change.
- The binding-aware Center hint follows the Space/jump row after attack moved to G; a rebound-key regression checks both advertised and dispatched actions. Master-off regression covers search-only state, released world data, canceled requests and preserved saved waypoints.

API hook audit: exactly one call each of `fork_hooks::begin_iteration`, `begin_game_event`, `end_game_event`, `observe_text` in Play. Begin/end still surround the native event match. Viewer retains `prepare_controls` in the gameplay and modal input branches; the owned adapter forwards `observe_report`. API capture/performance/input tests passed in the client suite.

Remaining-gate results and the upstream AO exception are recorded below.

The first full CI-target workspace build stopped before tests because C: had zero free bytes (`rustc-LLVM ERROR: no space on device`), not a code/test failure. Only this worktree's ignored generated build directories are relocated to `D:/eqoa-sync-2026-11-09-build-cache`; junctions preserve the original `target` paths and Cargo fingerprints. No source, game data, other checkout or user folder is removed. The owner then required one shared target (`C:/Users/hocka/Paseo/targets/shared`): the own retry was stopped, all remaining gates use that target, and the legacy worktree target and temporary D: cache are removed after gates. `C:/Users/hocka/Paseo/targets/eqoa-api-r2` is untouched.

`tools/sync_upstream.py` now honors an explicitly configured `CARGO_TARGET_DIR` instead of overriding it. A regression verifies shared-target preservation, scratch-config isolation and a single test thread, with an unset-target control. It failed on the old implementation (configured target replaced by local target) and passes after the one-line correction. The owner rule is recorded in the fork CLAUDE and sync procedure. Commands use the shared cargo lock, one thread, scratch `target/sync-config`, and a 3600-second process timeout. After the owner's disk-space instruction, every remaining cargo invocation uses `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`. Test totals, ignored and explicit SKIP counts will be recorded after completion.

The footprint command is `python -X utf8 tools/check_fork_footprint.py --base 91f9e72`: PASS (353 owned / 31 shared, named hooks intact). Local `origin/master` has moved to `14636b2`; the default checker compares that later ref and is not a check of this pinned merge. CI already fetches and points its upstream ref to `reviewed_base` before checking.

The first complete shared-target workspace run reached renderer tests and found three failures (`every_module_validates`, point-line and sun-line receiver assembly). Root cause: Windows `core.autocrlf` checked `lit.wgsl` out as CRLF, while upstream's exact replacement constants end in LF. This also prevented the production shadow receiver assembly. The fork-owned `.gitattributes` now enforces LF for `crates/render/src/*.wgsl`; renderer Rust/shader implementations remain unchanged in Git. A real Git checkout fixture with `core.autocrlf=true` failed before the attribute and passes after it. Vendor byte-preservation rules remain separate. This adds one tools regression (96 total).

## Unverified

Live owner play, a real GM server travel, Android device behavior, browser runtime and disc/oracle prerequisites not present in this environment. No claim of live acceptance.

## Final verification (2026-11-09)

Checkpoint: `ee7ffa6` (`sync: follow-up fixes from validation`), after reviewing all 17 follow-up files and `git diff --check`. No conflict resolution was redone and no follow-up hunk was discarded. Rust/shader source is unchanged after that checkpoint.

All Cargo gates used `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, the shared `C:/Users/hocka/Paseo/cargo-build.lock` mkdir lock with retry/timeout, scratch `EQOA_CONFIG`, one test thread and a 3600-second process timeout. C: free space was checked before every invocation (minimum observed about 70 GiB, above the required 10 GiB). Windows gates used Rust 1.99.1, target `x86_64-pc-windows-msvc`, and `RUSTFLAGS=-C target-feature=+crt-static`, matching enhanced-dev CI. The Forge reference was `4026d5cebcede24943ece8dc34833e584f46cd90`, UiForge `6f2d125cc686cbc882f8098a939cc03cc23a59a1`; both explicit reference and spike environment variables were supplied.

| Gate | Result | Ignored / explicit SKIP messages |
| --- | --- | --- |
| Addon suite (`--bin eqoa fork_ext::addons::`) | 332 passed, 0 failed | 23 ignored opt-ins; 4 symlink privilege SKIPs |
| Forge opt-ins (`--bin eqoa fork_ext:: -- --ignored`) | 24 passed, 0 failed | 0 ignored, 0 SKIPs |
| Bundled installation/update suite (`--bin eqoa fork_ext::bundled`) | 23 passed, 0 failed | 1 ignored, separately passed in the opt-in run; 0 SKIPs |
| CI-equivalent full Windows workspace, with `--no-fail-fast` to collect every target | 2,152 passed, **2 failed** across 101 summaries | 31 ignored; 266 SKIP lines |
| wasm (`cargo +1.99.1 check --locked -p eqoa --target wasm32-unknown-unknown`) | PASS | Initial prerequisite failure: wasm standard library absent; installed for 1.99.1, then reran successfully |
| Explicit Python tools discovery (`python -X utf8 -m unittest discover -s tools -p "test_*.py"`) | 96 passed | 0 SKIPs |
| Fresh sync script rerun (`python -X utf8 tools/sync_upstream.py --continue --tests-only`) | 1,869 Rust tests passed, 0 failed; 96 Python tests passed | 29 ignored; 174 SKIP messages |
| Pinned footprint (`python -X utf8 tools/check_fork_footprint.py --base 91f9e72`) | PASS: 353 fork-owned / 31 shared; all named hooks intact | All budgets pass; no further increase |

The sync-script result above was rerun after the usage-limit interruption; the unattended run was not used as final evidence. Aggregate Cargo pass totals include tests that return after logging SKIP; they do not establish disc/oracle coverage. Workspace SKIPs cover absent disc images, decomp/oracle executables, live captures/save states/decrypted assets, and Windows symlink privilege. Its 31 ignored tests comprise 24 Forge opt-ins (all passed separately), 2 FX benchmarks, 3 cape/light/helmet surveys, and AO/ground-fog cost measurements.

### Confirmed upstream workspace failure

The full Windows workspace gate is **not green**: `render --test ao` fails `the_foot_of_a_wall_darkened_and_open_ground_not` (plain grey `[127; 3]` versus asserted `[128; 3]`) and `the_same_at_freeport_as_at_the_origin` (factor difference 54 versus maximum 6) on this machine's NVIDIA GeForce RTX 3070 Ti / Vulkan. The other 2,152 workspace tests report passes. Repeating the unchanged AO tests with `WGPU_BACKEND=dx12` reproduces both failures (factor difference 58).

Negative control: extracted untouched pinned upstream `91f9e72` with `git archive` into an ignored scratch tree, then ran `cargo +1.99.1 test --locked --manifest-path target/upstream-baseline/Cargo.toml -p render --test ao -- --nocapture --test-threads=1` with the same shared target/lock/static CRT. It reproduces the identical two Vulkan failures (1 passed / 2 failed / 1 ignored, zero SKIPs). `git diff 91f9e72 -- crates/render` confirms all AO tests, AO code and scene shaders are unchanged; fork differences are map/UI paths. This is a verified upstream failure on this host, retained visibly rather than weakening assertions or changing unrelated renderer behavior. A green CI-equivalent workspace gate remains outstanding.

### API and bundled UI preservation

Rechecked exactly one call each of `fork_hooks::begin_iteration`, `begin_game_event`, `end_game_event`, and `observe_text` in `play.rs`; begin/end bracket the native event match. Both `prepare_controls` calls remain in `viewer.rs` before console polling. API capture, controller/input and performance regressions pass in the client suite. Wereoxxs lifecycle, render pipeline, symbol surface, input capture and worst-case chat tests pass; unchanged ARPG and Avoids packages pass the opt-ins, with bundled native resource/lifecycle coverage also passing.

The pinned `reviewed_base` was already `91f9e72dd6d111e20a37b73cea461444a18ad79c` in the merge/checkpoint. Retained deliberately after rechecking every budget; no move to the later `origin/master`, and no additional budget or hook-count changes in this finishing session. Earlier reviewed budget reasons remain above and in `docs/fork-footprint.json` / `docs/FORK-OWNERSHIP.md`.

### Release build and generated-file cleanup

`cargo +1.99.1 build --release --locked -p eqoa --target x86_64-pc-windows-msvc` passed with static CRT in 4m 38s. The same command is the final post-commit verification; its evidence is archived as `run/sync-2026-11-09-validation/finish-release-final.log`. The executable is `C:/Users/hocka/Paseo/targets/shared/x86_64-pc-windows-msvc/release/eqoa.exe`; this is a local build, not a published release.

Archived the sync/validation logs and API audit under ignored `run/sync-2026-11-09-validation/` before cleanup. Verified the absolute cleanup targets and that `target/` contains no tracked files. Detached the four `target/{debug,release,wasm32-unknown-unknown,x86_64-pc-windows-msvc}` junctions themselves without traversing them, then removed this worktree's generated `target/` folder and `D:/eqoa-sync-2026-11-09-build-cache`. Both paths are absent. The shared cache remains the only build target. `C:/Users/hocka/Paseo/targets/eqoa-api-r2` still exists and was untouched. The final build uses scratch configuration/logs in the ignored archive, so it does not recreate a worktree target.

No push, tag, merge into develop/enhanced, live play acceptance, or release publication was performed. The known upstream AO failures, missing fixture/privilege coverage, live owner/GM-server checks, Android device behavior and browser runtime remain outstanding; successful compilation and addon tests do not establish those results.

## Review fixes

Independent review `docs/plans/sync-2026-11-09-review.md` (now tracked) found three HIGH and four MEDIUM issues; all are fixed here, plus two trivial LOW documentation corrections. Shared upstream files (`options.rs`, `viewer.rs`, `bindpage.rs`) got minimal changes; no upstream code was deleted to simplify a hunk.

- **H1 (options save round-trip):** restored the block the merge dropped at the end of `the_extended_pages`, re-walked against the fork's order (menu 0 Game, 1 Interface, 3 Extended, 5 Save Changes; categories Video 0, World 1, Camera 5). It drives ✕ on Auto Attack, Video Brightness, World Fog and Camera Turn Speed, then Save Changes and Yes, and asserts `client.opt` = `lod=on\nbrightness=4\ncamera_distance=11\ncamera_turn=125\nfog=4\n` and `game.opt`'s `auto_attack = 1`. The asserted values are the fork's defaults plus one step each. Negative control: commenting out `crate::settings::save(o)` made the test fail at `testing::kept("client.opt").unwrap()`; restored bytes pass.
- **H2 (duplicate map capture):** deleted the merge's second, unreachable map key-capture block (`viewer.rs`); the first stays and keeps `p.keyboard = true`.
- **H3 (F3 while the map is up):** moved the F3 bug-report handler above the map capture, matching upstream's F9 → F3 → map order. `keyboard()` needs an `ActiveEventLoop`, so a live dispatch test is not available; `sync_f3_is_handled_before_the_map_takes_every_key` in `fork_ext/report_sync_tests.rs` is a source-order wiring guard (and asserts one map block), complementing the existing F3 dialog test.
- **M1 (bindpage `(null)` labels):** the two tests use the `hide_ui` row again, so assertions read `Hide UI: F10. H does nothing now.`, `Hide UI: |(. (null) has no button now.` and `(null): |9. Hide UI has no button now.`; F10 (not upstream's F2) is used for the keyboard case because F2 is now Map search's default.
- **M2 (G attack / O group waypoint):** verified against `Binds::from_text`; saved custom layouts migrate (including G→jump and macros). A fork player on unsaved defaults keeps the owner's chosen new defaults, and `FORK-UPDATE-NOTES.md` already says "Group waypoints now use O; G attacks". Not a code defect; no change.
- **M3 (left/right running Mods & Addons actions):** `fork_options::change` no longer falls through to `activate`, so LEFT/RIGHT only toggle values and the action rows act on ✕ only. Regression `left_and_right_do_not_run_the_action_rows` checks the Addon Manager request is not raised by a step.
- **M4 (`CursorLeft` duplicate calls):** removed the duplicated `map.let_go()` / `mini_hover(None)` pair in `viewer.rs`; `chat_let_go()` stays.
- **L1/L2 (docs):** corrected the `render/src/lib.rs` wording (it is upstream's verbatim; the circular map lives in `render/src/map.rs`/`ui.rs`) and noted the restored upstream quest-negotiation silence in `FORK-UPDATE-NOTES.md`. L3 was verified against upstream and left; L4's scratch-config path nit is unchanged (logs/configuration, not a Cargo cache).

Verification after the fixes, with `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, the shared mkdir lock, scratch `EQOA_CONFIG` and one test thread:

| Gate | Result |
| --- | --- |
| Focused client tests (`options::`, `bindpage::`, `viewer::`, `sync_`) | 36 + 6 passed, 0 failed |
| `python tools/sync_upstream.py --continue --tests-only` | 1,871 passed / 0 failed / 29 ignored; 176 SKIP messages; Python tools 96 passed |
| Explicit Python tools discovery | 96 passed |
| `cargo check --locked -p eqoa --target wasm32-unknown-unknown` | PASS |
| `python -X utf8 tools/check_fork_footprint.py --base 91f9e72` | PASS: 353 fork-owned / 31 shared; no budget raised |
| `cargo build --release --locked -p eqoa` | PASS (final post-commit build, static CRT, `x86_64-pc-windows-msvc`) |

The two `render --test ao` failures remain the same on untouched upstream `91f9e72` on this GPU (NVIDIA RTX 3070 Ti / Vulkan), as recorded above; they are unrelated to these files and were not rerun as evidence here.
