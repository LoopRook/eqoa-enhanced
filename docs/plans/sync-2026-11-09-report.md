# Upstream sync 2026-11-09

Status: merge committed as `3307f50` (LoopRook); validation follow-ups in progress. Branch `sync/2026-11-09`; original fork HEAD `4ebe0f7`; pinned merge target `91f9e72dd6d111e20a37b73cea461444a18ad79c` (134 upstream commits). No push, tag or protected-branch merge.

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
- `crates/render/src/lib.rs`: Retain upstream renderer exports and scaled capture callback; circular map renderer remains fork-owned.
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

Pending addon/bundled UI explicit suite, workspace CI test, wasm check and release build.

The first full CI-target workspace build stopped before tests because C: had zero free bytes (`rustc-LLVM ERROR: no space on device`), not a code/test failure. Only this worktree's ignored generated build directories are relocated to `D:/eqoa-sync-2026-11-09-build-cache`; junctions preserve the original `target` paths and Cargo fingerprints. No source, game data, other checkout or user folder is removed. The owner then required one shared target (`C:/Users/hocka/Paseo/targets/shared`): the own retry was stopped, all remaining gates use that target, and the legacy worktree target and temporary D: cache are removed after gates. `C:/Users/hocka/Paseo/targets/eqoa-api-r2` is untouched.

`tools/sync_upstream.py` now honors an explicitly configured `CARGO_TARGET_DIR` instead of overriding it. A regression verifies shared-target preservation, scratch-config isolation and a single test thread, with an unset-target control. It failed on the old implementation (configured target replaced by local target) and passes after the one-line correction. The owner rule is recorded in the fork CLAUDE and sync procedure. Commands use the shared cargo lock, one thread, scratch `target/sync-config`, and a 3600-second process timeout. After the owner's disk-space instruction, every remaining cargo invocation uses `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`. Test totals, ignored and explicit SKIP counts will be recorded after completion.

The footprint command is `python -X utf8 tools/check_fork_footprint.py --base 91f9e72`: PASS (353 owned / 31 shared, named hooks intact). Local `origin/master` has moved to `14636b2`; the default checker compares that later ref and is not a check of this pinned merge. CI already fetches and points its upstream ref to `reviewed_base` before checking.

The first complete shared-target workspace run reached renderer tests and found three failures (`every_module_validates`, point-line and sun-line receiver assembly). Root cause: Windows `core.autocrlf` checked `lit.wgsl` out as CRLF, while upstream's exact replacement constants end in LF. This also prevented the production shadow receiver assembly. The fork-owned `.gitattributes` now enforces LF for `crates/render/src/*.wgsl`; renderer Rust/shader implementations remain unchanged in Git. A real Git checkout fixture with `core.autocrlf=true` failed before the attribute and passes after it. Vendor byte-preservation rules remain separate. This adds one tools regression (96 total).

## Unverified

Live owner play, a real GM server travel, Android device behavior, browser runtime and disc/oracle prerequisites not present in this environment. No claim of live acceptance.
