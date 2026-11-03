# Indoor map plan (dungeons, caves, buildings)

**Status:** Backlog (owner, 2026-11-03): plan both the minimap and the M map, build later.
**Reported:** 2026-11-03. Inside dungeons the minimap shows nothing useful. On PCSX2, FrontiersForge's
`mini_map` (the one players saw with Wereoxxs) did show them.

## What the player will see (draft; get the owner's OK before building)

1. Walk somewhere with a roof over you (a dungeon, cave or building) and the minimap switches from the
   outdoor picture to a top-down drawing of the rooms around you.
2. Ceilings and floors above you are cut away, so you see the floor you stand on: your level bright,
   lower levels shaded darker.
3. It follows you up and down stairs and redraws for the new floor.
4. Your arrow, NPC and group dots, waypoints and search marks stay on top, as they do outdoors.
5. Step back outside and the normal minimap returns.
6. Phase 2: the M map shows the same indoor view around you while you are indoors. Outdoors it is
   unchanged.

```
in play, every frame (budgeted)
  roof over the player? --no--> outdoor minimap (as now)
        |yes
        v
  collect rooms near the player (loaded zones only)
        v
  drop triangles above the cut height (player y + margin)
        v
  raster top-down, keep the highest y per pixel, shade by height relative to the player
        v
  minimap: indoor picture under the usual marks
  (phase 2: the M map opened indoors shows the same picture, zoomable)
```

## Why it is blank today (scout, 2026-11-03, spot-checked by the lead)

- The map's pictures come from each zone's **terrain patches and placed props**, drawn once from straight
  above (y -10,000..10,000, everything squashed flat) into 512-texel tiles on the 2,000-unit grid and
  cached as PNGs: `map/build.rs` (`tile_view` ~59-76, `placements` ~124-139, ~556-568), `map/cells.rs`,
  `map/picture.rs`, composited by `map/mini.rs` ~484-522.
- **Room geometry is never read** (`VIZoneRoom` sprites, record 0x3240), and there is no indoor/outdoor
  branch. A dungeon zone with no terrain parts is `Zone::Failed` (`build.rs` ~469-471), drawn as the grey
  "not drawn" fill (`mini.rs` ~500-503); otherwise you get a squashed top view of the roof.
- Separate pre-existing bug: a second zone whose middle falls in the same 2,000-unit cell is left out of
  the map entirely (`cells.rs`, "A second zone in a cell is left out of the map").

## What the client already has

- `vi::world::World` (`crates/vi/src/world.rs`): zones, then rooms. `Room.flags` bit 0 = **open to the
  sky** (closed rooms are indoors), `Room.bbox` with y, each room's resolved sprite, actors with
  transforms, and `loaded[]` per zone.
- `vi::collide::CollBuffer`: full vertices (`verts_f: Vec<Vect3>`, packed `verts_s`), so **per-vertex
  height exists** for slicing. It has no UVs, so the first version is untextured (flat height shading).
- eqoa holds the live world as `walker.scene` (`walker.rs:80`). `Play::map_frame` (`play.rs` ~7668)
  already has `walker` in scope where it calls `map.frame(&cx)` / `map.mini_frame(&cx, &mv)`
  (~7747-7751), but `map::Context` doesn't carry the scene. One context field (a hook in a shared file)
  makes it reachable.

## How Forge's mini_map does it (FrontiersForge 4026d5c, scripts/mini_map/modules)

- `world_geometry.lua`: walks the live zone/room arrays and collects rooms whose bbox is within a
  radius. Decodes each room's terrain sprite primitives plus its actors' buffers, transformed by
  position/rotation/scale. Keeps a 256-room decode cache. "Indoors" = a ceiling probe over 5 columns;
  any column open to the sky means outdoors.
- `map_render.lua`: a CPU triangle rasteriser, orthographic top-down, z-buffer by height (keeps the
  highest y per pixel). **Ceiling cut:** a triangle whose lowest vertex is above `cut_y` is dropped.
  Flat height-palette shading around the player's y (an optional textured mode). Work is budgeted at
  about 1 ms a step with learned throughput; framebuffers are pooled at 1024², then uploaded as a UI
  texture.
- It reads PS2 guest memory, so it can't run here (`docs/FORGE-COMPATIBILITY-MATRIX.md` section 5). The
  port is native: the same algorithm over `walker.scene`.

## Design

- **Fork-owned code:** a new `crates/eqoa/src/map/indoor.rs` (the indoor picture), with mini/view
  integration in the map module's own files. The only shared-file change is a hook passing
  `&walker.scene` (and the player's y) into `map::Context` at `play.rs` ~7695-7751, within budget.
- **Indoor test:** the player's room (or the rooms whose bbox contains the player) has `flags & 1 == 0`.
  Fall back to Forge's ceiling probe if room flags turn out unreliable at doorways. Add hysteresis
  (e.g. 0.5 s) so the map doesn't flicker in doorways.
- **Geometry:** rooms of loaded zones within the shown radius: the room sprite's collision triangles,
  plus static actors' buffers transformed (as `build.rs` `placements` does for props). Cache the
  decoded triangles per room.
- **Raster:** CPU, Forge-style (simplest and proven). Depth = y, drop triangles whose lowest vertex is
  above `cut = player_y + margin` (margin about a head's height). Shade: the player's level bright; below
  it darker by height difference; walls (steep triangles) as outlines. Output an RGBA buffer, uploaded as
  one texture, drawn in the minimap's circle with the existing marks on top.
- **Budget:** incremental like `Builder::step`, at most about 1-2 ms a frame. Redraw when the player
  moves more than about 1/8 of the radius, changes floor (|Δy| > margin), or turns, if heading-up ever
  applies (the minimap is north-up today). Show the last picture while a new one builds.
- **Game logic stays untouched:** this is presentation only. Use `ee::*` wherever a comparison feeds
  anything game-side (none expected).
- **Phase 2, the M map:** when opened indoors, render the same picture at the map's zoom around the
  player, so pan and zoom are limited to the loaded rooms. Outdoors it is unchanged.

## Steps (one feature branch, `feat/indoor-map`, from develop)

1. Context hook and an indoor detector with tests (open-sky room vs closed room fixtures, plus the
   doorway hysteresis).
2. Room triangle collection, with the world transform from vi structs, tested on a synthetic two-floor
   fixture.
3. Rasteriser plus the ceiling cut plus shading, tested pixel by pixel on the fixture. Negative control:
   without the cut, the upper floor hides the lower.
4. Minimap integration behind a setting (Map & Quests: "Indoor minimap", default on), marks on top.
   Capture tests at 1280x720.
5. Budget/perf: a frame-time test on the largest dungeon available in the disc data (disc tests skip on
   machines without the disc; count the skips).
6. Phase 2: the M map indoors.
7. A release build, then the owner play-tests on known dungeons (e.g. Blackburrow, the Druk's orc tunnel,
   the Salt Mine, the Gerntar Mines) with a checklist of floors, stairs, doorways and the outside
   return.

## Risks and open questions

- **CPU cost:** big rooms are tens of thousands of triangles. Budget per frame and cache per room.
- **What counts as indoors:** room flags vs the ceiling probe, doorways, open-roofed courtyards inside
  dungeons.
- **Multi-floor:** the cut height must follow the player's y, or upper floors cover the lower.
- **Untextured first:** whether prim/pick buffers keep UVs (`vi/src/pick.rs`) is unchecked; a textured
  mode would be a follow-up.
- **The second-zone-per-cell bug** is separate (outdoor map). Fix it on its own branch.
- Not verified yet: which dungeon zones are `Failed` vs terrain-only in practice (inferred from code,
  not run).

## Suggested team

Scout done (GLM Go). Build: Codex Sol high (a stateful, perf-sensitive feature). Review: a different
family (MiniMax or GLM). The owner approves the 5-line description above before the build starts.
