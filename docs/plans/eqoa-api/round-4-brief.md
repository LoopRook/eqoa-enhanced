# EQOA round 4: map API and map mods (brief, Opus lead, 2026-11-11)

Repository `C:/Users/hocka/Github Projects/EQOA Native Rust`. Branch `feat/eqoa-api-r4` from `develop`
(after the 2026-11-11 upstream sync merges). Plan: `docs/plans/EQOA-MOD-API-PLAN.md` "Round 4" and
"Rules for every worker brief" (paste-in rules; obey all). Spec: `docs/modding/EQOA-API-SPEC.md`
(§5 HUD, §7 "Map data and waypoints", §9 sandbox, §10 hooks). Round 3's report
`docs/plans/eqoa-api/round-3-report.md` shows how the HUD layer, mod keys and mod option rows work.

## Goal and owner decisions (2026-11-11)

Upstream's map files return to upstream's text, or carry one neutral seam. The fork's map features
become bundled Lua mods, on by default, with nothing lost for players:

1. **eqoa_creature_journal**: records each creature seen (name, id, world, level, x/z) per server;
   journal Import (FrontiersForge/PCSX2) and Export (Desktop) stay as buttons in the search box's
   footer; the player's existing `journal.json` carries over untouched; feeds search.
2. **eqoa_map_search**: F2 (or the small button under the minimap) opens the search box over the M map
   or minimap; tabs Creatures / Group / Places; arrows + Enter set a guide (dashed line to the nearest
   live or known spawn, on the M map and the minimap, plus dots for every known spawn); the guide
   follows a moving creature/member and clears when they leave; F4 clears it. F2/F4 become the mod's
   keys (same defaults, rebindable on Controls). Upstream's list of places (P on the map) is untouched.
3. **eqoa_round_minimap**: hides upstream's square minimap and shows the round one in the same place
   and size, with its ring; shows all upstream's minimap shows (terrain, upstream's indoor view, camps,
   coaches, quest marks, waypoints, group) cut to the circle, rim marks for things beyond the edge;
   hovering a dot shows a card (name, level, Player/Pet/NPC, distance); N toggles, wheel zooms; the
   search button and guide show on it. Mod off = upstream's square minimap.
4. **No map skin mod.** The M map takes upstream's look (upstream's colours, game font, upstream's
   fixed key legend). The fork's dark/red + Space Mono look lives on in what the mods draw. A
   "skinnable map" seam goes to the backlog (to offer upstream in round 8).

Facts from the scout (develop cbcf86b vs origin/master; re-check after the sync):
- Fork map code: new files `map/search.rs` (1,716 lines), `map/journal.rs` (1,567), `map/theme.rs`
  (148), `fork_ext/map.rs` (circular rim); fork blocks in `map/mod.rs` (~18 hunks: search, theme font,
  guidance, toolbar), `map/chrome.rs` (binding-aware legend, search anchor), `map/mini.rs` (circle,
  `Dot`/hover card, masked draws, search inside minimap), `map/markers.rs` (badge restyle),
  `map/tool.rs` (mapshot fixtures), `map/tests.rs` (render-scale GPU test), `map/community.rs`
  (test temp dir); `render/src/map.rs` + `map.wgsl` (circle vertex attribute, discard mask);
  `render/src/ui.rs` (`add_font_at_height`, `draw_range`, `draw_circle_range`); `play.rs` (search
  holds, search/live/group feed, journal record per frame, key/click routing, busy, move/look locks,
  `Dot::of`, tests); `viewer.rs` (F2/F4, text/key, wheel/pointer/touch routing); `binds.rs`
  (`map_search` F2, `map_stop` F4 rows).
- Catalogue: `crates/eqoa/resources/map-data/map-entities.json` (1.66 MiB, Forge entity_database,
  SHA-256 pinned in search.rs, embedded with `include_str!`, `EQOA_MAP_DATABASE` / beside-exe overrides).
  Font `resources/map-ui/SpaceMono-Regular.ttf` (OFL).
- Journal: `<config>/journal.json`, format `{"<name>":{"<id>":{"world":w,"level":l,"pos":[x,y,z]}}}`,
  caps 16 MiB load / 14 MiB save / 80,000 spawns / 50,000 per import, dedupe 30 units at same level,
  save after 30 s quiet or every 120 s; filters in journal.rs:345-386 (creature type, p256<2, no
  corpse, p255==0, named).
- `eqoa.map` is declared "not_yet, round 4" (`fork_ext/addons/native.rs` ~:100) with get, images,
  load_image, set_waypoint, clear_waypoint. `eqoa.hud.hide` has no `minimap` element. No pointer API.

## Parts, each its own fresh session

- **Part A (Sol high): the API**, below. Ends with tests + report `docs/plans/eqoa-api/round-4-report.md`
  (Part A section). Reviewed (Claude Sonnet) before Part B.
- **Part B (Sol medium, fresh): journal + search mods**, and removal of native search/journal from
  upstream's files and `binds.rs`; migration of journal file and key binds.
- **Part C (Sol medium, fresh): round minimap mod**, removal of the circle/hover/theme code; upstream's
  map files, `render/map.rs`, `map.wgsl` back to upstream's text (or one seam). Frame-time measurement
  with the three mods on.
Each part gets its own brief from the lead; this file is the shared design.

## Part A: the API (design decisions are fixed; implementation details are yours)

Write the spec additions first into `docs/modding/EQOA-API-SPEC.md` §7 "Map data and waypoints"
(replace the sentence ruling out guide search/theme changes with these decisions; keep the existing
rows). Then build. Each new value has a test starting from Play/protocol-shaped data, with a negative
control.

1. `eqoa.map.get()` and `map.changed` as already specified.
2. `eqoa.map.markers(world_id)` → array of `{kind="camp"|"coach"|"town"|"quest"|"community"|
   "waypoint"|"group_waypoint", name, x, z, level_min?, level_max?, label?}` from the same native
   sources upstream's map draws (markers_data, quests, community cache, waypoints). Read-only.
3. `eqoa.map.view{center?, zoom, size, shape="square"|"circle", rim=bool}` → a view record with a
   `texture` (TextureInfo usable by `eqoa.hud.image`) plus `project(x,z)` → px,py,inside and
   `unproject(px,py)` → x,z. The client renders **upstream's own minimap drawing** (terrain, indoor
   view, marks, waypoints, group) into the texture; no terrain or marks drawn in Lua. `shape="circle"`
   masks in fork-owned render code (a fork-owned pipeline/shader file under `crates/render/src/`, not
   upstream's `map.rs`/`map.wgsl`) and with `rim=true` clamps marks beyond the edge onto the rim.
   One render per view per frame at most, only while a mod holds the view; zero cost with no view.
4. `eqoa.map.set_guide{world_id, x?, z?, entity_id?, dots?={{x,z},...}, label?}` /
   `eqoa.map.clear_guide()`: a guide the mod owns (separate from the player's waypoint), drawn by the
   client as the dashed line + dots on the M map, on upstream's square minimap and on views. With
   `entity_id` the line follows the live entity; `guide.lost` event when it leaves. Cleared on
   disable. Drawing lives in fork-owned code reached through ONE map overlay seam (called from the M
   map's and the minimap's draw with their transform) — this is the round's one map seam.
5. `eqoa.hud.hide("minimap")` (and `show`): new element; hides upstream's minimap and its clicks.
   Upstream's Move HUD (11aa5f0, synced 2026-11-11b) lets the player drag HUD pieces, kept in
   `hud.opt`: `eqoa.hud.layout()` must give the minimap's current rectangle (moved or not) so the
   round minimap mod sits where the player put upstream's minimap, and follows a later move.
6. `eqoa.input.pointer()` → `{x,y,inside_window:bool}` in HUD coordinates, plus events
   `pointer.pressed`/`pointer.wheel` with coordinates, and `eqoa.input.claim_pointer(rect)` so clicks
   and wheel inside a mod's rectangle (minimap, search button) do not reach the game.
7. Mods over the open M map: mod HUD/ImGui drawing and mod keys/text input work while the M map is
   open (the search box sits over the map). Check today's behaviour and route through the existing
   addon input path; report any new seam needed.
8. Packaged read-only data above 1 MiB: the catalogue moves into the search mod's folder (or the shared
   `mods/resources` tree); `eqoa.files.load_json(path)` decodes JSON natively off the frame thread
   and returns the table through `resource.ready` (kind "json"), size cap 16 MiB, read-only mounts
   only. Keep the SHA-256 pin test on the moved file.
9. `eqoa.files.save_json_async(path, value)` (private area only): native encode and write off the
   frame thread, latest value wins; for the journal (up to 14 MiB). Only build this if a measured Lua
   save of a 14 MiB journal costs more than 2 ms on the frame; record the measurement either way.
10. Entity events with what the journal needs: confirm the existing entity deltas carry name, id,
   world, level, position, creature/corpse flags; add only what is missing.

Not in Part A: removing native search/journal/minimap code, writing the mods.

## Gates (every part)

- Tests: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` with a
  scratch `EQOA_CONFIG`; client suite through `python tools/sync_upstream.py`-style single-thread run
  is not needed per part, but run `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1` once at
  the end of each part; `cargo check --locked -p eqoa --target wasm32-unknown-unknown`; release build
  after the final commit. Zero-addon allocation gate passes. Count SKIP lines.
- Bundled Wereoxxs, ARPG, Modern UI and round 3's four mods still load and pass.
- `python tools/check_fork_footprint.py --base <reviewed_base>` PASS. Report each budget change.
  Part A may add the overlay seam and the `minimap` hide check; Parts B/C must bring map files down.
- A benchmark asserts its workload ran. No test that logs SKIP and returns Ok counts as proof.
- Known GPU-only failures on this PC, green on CI: two AO tests, upstream particles16.
- Shared `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared`, mkdir lock
  `C:/Users/hocka/Paseo/cargo-build.lock` with a timeout, one build at a time, cargo in the
  foreground. Long output to a file; read only summaries.
- Commit on your branch only, as LoopRook. No push, merge or tag. `git -c safe.directory=*` in managed
  worktrees.
