# Upstream sync 2026-11-09

Status: merge resolved; validation in progress. Branch `sync/2026-11-09`; original fork HEAD `4ebe0f7`; pinned merge target `91f9e72dd6d111e20a37b73cea461444a18ad79c` (134 upstream commits). No push, tag or protected-branch merge.

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
- `crates/eqoa/src/options.rs`: Retain Lighting IDs 52?57 and Texture Folder; fork-owned rows preserve existing IDs; masters use 1054/1055.
- `crates/eqoa/src/play.rs`: Union upstream cape/race/inventory/event logic and fork API hooks, search/journal/HUD features; retain upstream master-switch tests.
- `crates/eqoa/src/settings.rs`: Union lighting and master settings with fork options, preserving every existing saved key; remove duplicate restored declarations/serialization.
- `crates/eqoa/src/touch.rs`: Retain upstream touch behavior and fork dpad suppression; remove duplicate holds initializer.
- `crates/eqoa/src/tracker.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/eqoa/src/viewer.rs`: Retain upstream camera/AO/render-scale paths plus addon/report/search hooks; add offered GM Ctrl-click modifiers and focus reset.
- `crates/net/src/session.rs`: Retain upstream network behavior and quest negotiation plus fork mana bars; add offered GM travel protocol 0x7E34/35.
- `crates/render/src/lib.rs`: Retain upstream renderer exports and scaled capture callback; circular map renderer remains fork-owned.
- `crates/render/src/map.rs`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.
- `crates/render/src/map.wgsl`: Keep fork implementation; reconcile upstream restored baseline and applicable later fixes without dropping search, journal, theme or circular clipping.

`README.md` and `CLAUDE.md` remain the fork versions; `UPDATE-NOTES.md` is upstream's. The script refreshes `docs/upstream/CLAUDE.md` at the pinned target.

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
- Upstream map rows 41?45 are represented by fork-owned rows instead: upstream 42 collides with existing Mana Bars 42. No existing fork IDs or saved keys are renumbered. Lighting IDs 56/57 remain upstream's.
- Upstream tests claiming Stats Box/Mana Bars/Target of Target no longer exist are inapplicable to Enhanced, which intentionally retains them. Corresponding fork persistence/input tests remain.
- Upstream edit-box-only suggestions remain as the controller flow; the fork in-map mouse/keyboard form stays.
- Later non-map commits after the pinned target are outside this merge.

## Budgets and validation

Reviewed maxima vs old: HUD +316/-15 ? +316/-44; options +41/-41 ? +42/-140; Play +759/-53 ? +759/-89; settings +334/-23 ? +334/-38. Upstream restored map code is now baseline, so existing fork replacements count as deletions; options removes conflicting stock 41?45 rows/arms and moves the map/UI walk to owned tests, HUD retains circular hit tests/gauges/claims, Play retains search/journal/dot payload adaptations and every API seam, settings replaces vanilla-only assertions that Enhanced options disappeared. All reasons are recorded per file in `fork-footprint.json`; no renderer or net budget increase. New Map category hook is guarded.

Pending scripted client/net tests, tools unittest, addon/bundled UI suite, workspace CI test, wasm check and release build. Commands use one shared cargo lock, one thread, scratch `target/sync-config`, isolated target and a 3600-second process timeout. Test totals, ignored and explicit SKIP counts will be recorded after completion.

## Unverified

Live owner play, a real GM server travel, Android device behavior, browser runtime and disc/oracle prerequisites not present in this environment. No claim of live acceptance.
