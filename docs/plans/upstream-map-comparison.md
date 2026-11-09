# Upstream's map and ours: the facts for the keep / take / combine decision

**Date:** 2026-11-09 · **Read-only comparison, nothing merged or changed**
**Refs:** `origin/master` 14636b2 (upstream), `develop` 4ebe0f7 (LoopRook/EQOA Enhanced), merge base 8435f2c8.
The brief said upstream's latest was 91f9e72; origin/master is 21 commits newer. Counts are given for both.

## The short answer

- **It is the same map.** Theirs is not a second, differently designed map. Our map began as a
  byte-for-byte restore of upstream's own map one day after upstream deleted it. Then we added to it.
- **We have all of theirs, plus about 2,100 lines of features they do not have** (search, creature
  journal, live guidance) and 1,600 lines of tests for them. They have five small front-end additions
  made after 9 November that we do not have yet (master off switch, GM map travel, Android cache path,
  quieter quest marks, O/G key layout).
- **Recommended: combine** — keep our files and port those five small upstream items. Keeping theirs
  alone would take working features away from players; keeping ours untouched would freeze us against
  upstream and make every later sync harder.

## 1. History

- **21–27 Oct 2026:** upstream built the map in public, starting with `3582731` on 21 Oct ("The map (M):
  the world drawn from the disc's terrain..."). `099aaf7` (26 Oct) added the quest tracker; later commits
  added the minimap, quest marks, camps/coaches, community pins and group waypoint.
- **27 Oct 2026 05:12 UTC:** upstream deleted all of it in `6fc3ec6` ("Vanilla without the map, the
  minimap, the quests' marks and tracker..."), to keep a vanilla client. Its release notes for that day
  say the additions "are gone".
- **28 Oct 2026 01:17 UTC:** our first map commit, `bd50b49` ("Restore QoL features: world map, minimap,
  quest guide, and stat box"). It restored **all 20 map-related files byte-for-byte identical** to the
  files upstream had just deleted. Our map is therefore **derived from upstream's earlier code**, not
  written independently.
- **28 Oct – 3 Nov:** about 18 fork commits extended it: `928868e` (native search, shared guidance,
  circular minimap), then the creature journal, entity hover, render-scale fixes, etc.
- **9 Nov 2026 05:58 UTC:** upstream put the same code back in `65f9b13` ("The map and the quest helper
  back (Extended > Map, on by default)") — its options comment says the user asked for "the map and quest
  helper we had in a former update". It was merged as `6c5bb03` at 09:10, then refined by `8003083`
  (master switch, compass gating, Android cache), `53c5cf5` (group waypoint O, attack G) and `476fdac`
  (fewer quest marks on the minimap rim, GM Ctrl+click travel).
- **Why the merge conflicts:** because upstream deleted and re-added the same paths, git sees two
  independent additions at the same places, not a normal edit. That is the whole cause of the 15
  add/add conflicts.

## 2. Features today, side by side

| Area | Ours (`develop`) | Theirs (`origin/master`) |
|---|---|---|
| World map (M) | Same disc-drawn world, full screen, our chrome | Same disc-drawn world, full screen, game-styled chrome |
| Minimap | Circular, GPU-masked, under the quick menu | Square 112 px, under the quick menu |
| Quest tracker | Same code: server tracker plus built-in guide | Same code |
| Quest marks / helper | "!" and "?" marks, cards, guide fallback | Same, plus 9 Nov refinement: only your own "?" stays on the rim |
| Group waypoint | Yes, key **G**, chat-based, orange on map/minimap/compass | Yes, key **O** (G became attack), same chat protocol |
| Community / shared markers | Yes, same eqoaclassic list, ETag cache | Yes, same list and cache |
| Map suggestions | In-map form for mouse/keyboard; controller still edit box | Game edit box; map closes while typing |
| Player position | Arrow, constant screen size | Same |
| Zoom / pan | Wheel + keys, drag, touch | Same |
| Search | **Ours only:** F2 NPC/Group/Places panel, F4 stop, live-vs-recorded guidance; name fragments, no fuzzy/level filter; full NPC catalogue native only | None |
| Icons | Camps, coaches, routes, towns, quests; neutral themed XP badges; Space Mono | Camps, coaches, routes, towns, quests; red XP badge; game font; coach horseshoe read from disc |
| Entity markers | Dots plus name/level/kind/distance hover cards; live matches highlighted | Dots only; tooltips for camps/quests |
| Zone data source | Disc terrain, per-zone 512 px tiles cached | Disc terrain, same tiles and cache |
| Options rows | UI > Minimap; Map & Quests > Quest Markers, Quest Tracker, Map Markers — **no master switch** | Map page > Map, Minimap, Map Markers, Quest Helper, Quest Tracker |
| Keybinds | M, N, +/-, Q waypoint, X markers, T towns, J quests, Space centre; **F2/F4 search/stop; G group** | Same core; **O group, G attack**, Ctrl+click GM travel; no search keys |
| Performance / caching | Same tile budget and cache; adds search result cache and journal flush | Same tile budget and cache; master off frees GPU tiles and cache memory |
| Android / web | Both build; web has OPFS cache; **web lacks the NPC catalogue and journal file I/O**; Android cache misses `EQOA_CACHE` | Both build; OPFS cache; **Android cache honours `EQOA_CACHE`** |

## 3. What a player would notice

**Only ours has:** F2 search and F4 stop; "Spawned nearby / Not detected" guidance that follows a live
creature first and recorded spawns second; a creature journal that learns NPCs as you play, with export
and import (FrontiersForge and Wereoxx PCSX2 files); circular minimap with entity hover cards; in-map
suggestion form; the dark/neutral/Space Mono look; binding-aware keyboard legend; fork render-scale fixes.

**Only theirs has:** a single **Map** master switch and a **Quest Helper** master switch that turn the
whole system off and release its GPU/cache memory; **GM Ctrl+click travel**; Android's proper cache path;
fewer quest "!" marks ringing the minimap; the O group / G attack layout.

**Same feature, different execution:** square red-accented minimap vs circular neutral one; suggestion
through the game's edit box vs an in-map form; separate Quest Markers/Tracker toggles vs a Quest Helper
master. The underlying map, camps, coaches, quests, community list, waypoints and group chat protocol
are the same code.

**Size:** theirs 20 files / 9,420 lines; ours 24 files / 13,669 lines (+45%). Our four new files are
3,669 lines (journal 1,567, search 1,716, suggest 238, theme 148), of which about 1,600 lines are tests.
The remaining ~580-line difference is edits to shared files: mostly UI layout, styling and the
search/journal hooks. So "styling" is a minority (roughly 500–700 lines); the rest is new function.

## 4. Data and licensing

- **Both** draw the world map from the player's own disc; **both** ship guide data: EQOA-Data's camps,
  coaches and towns (MIT, ~30 KB generated table) and the 733 KB quest guide.
- **Ours additionally ships** `crates/eqoa/resources/map-data/map-entities.json` (1.74 MB,
  FrontiersForge, MIT, pinned revision and SHA-256) and `SpaceMono-Regular.ttf` (99 KB, SIL OFL 1.1).
  Both notices are in the repo and the preview bundle. The catalogue is embedded natively; a sidecar
  or `EQOA_MAP_DATABASE` can override it; the web build does not embed it. It may differ from the
  connected server, and the docs say so.
- **Net effect:** about 1.8 MB larger binary/ZIP for ours; no new licence obligation beyond retaining
  the MIT/OFL notices. Theirs has no comparable NPC catalogue.

## 5. Merge options and honest estimates

**(a) Keep ours, reapply upstream's non-map work.** Resolve 27 conflicts (15 add/add map files and 12
integration files), then either port or knowingly drop upstream's five map commits. Cost roughly
**1–2 days** for the merge plus **0.5–1 day** porting. What we would lose if not ported: GM map travel
(map + `net/session` + legend + tests), the Map/Quest Helper master switches and their memory release,
Android cache path, quest-rim filtering, and O/G key parity. Risk: our map drifts from upstream, and
every future sync repeats the same conflicts.

**(b) Take theirs, drop ours.** Cost roughly **1–2 days**; technically low risk. Players lose search,
live guidance, the creature journal, circular minimap and hover cards, the in-map suggestion form, the
theme, F2/F4 and the fork's render-scale fixes. We would also have to strip search/journal hooks from
`play.rs`, `hud.rs`, `settings.rs`, `options.rs` and `binds.rs`. This is the largest product loss for the
least work.

**(c) Combine — recommended.** Use our files as the base and port upstream's five map commits
(`65f9b13`, `8003083`, `53c5cf5`, `476fdac`, plus the merge `6c5bb03`), adapting them to our search,
theme and circular minimap. Most of the work is: master switches in settings/options, GM travel through
`net/session` and the map, `EQOA_CACHE` in `store.rs`, the minimap rim rule, and one decision on G/O.
Cost roughly **2–3 focused days** including tests; risk is concentrated in `map/mod.rs`, `chrome.rs`,
`mini.rs`, `play.rs` and `settings.rs`, where both sides changed the same code. Both sides have tests,
which lowers the risk. Reason: ours already contains all of theirs plus the only substantial new
functionality; upstream's additions are small and self-contained; and because our map is their own
earlier code, there is no design incompatibility to reconcile.

If schedule is tight, do (c) in two steps: merge ours first (one day), then port GM travel, the master
switches and the cache path as a short follow-up.

---

## Technical appendix

### A. Commits and refs

| Ref | Commit | Date (UTC) | Note |
|---|---|---|---|
| Upstream first map | `3582731` | 2026-10-21 02:26 | Basic map from disc terrain |
| Upstream quest tracker | `099aaf7` | 2026-10-26 | Tracker under minimap |
| Upstream removal | `6fc3ec6` | 2026-10-27 05:12 | Deletes 20 map files |
| Fork restore (first fork map commit) | `bd50b49` | 2026-10-28 01:17 | Byte-identical restore |
| Fork search/guidance/theme | `928868e` | 2026-10-28 03:37 | Start of fork-only work |
| Upstream re-add | `65f9b13` | 2026-11-09 05:58 | Restores map + quest helper |
| Upstream merge | `6c5bb03` | 2026-11-09 09:10 | Merge map-quest-helper |
| Upstream cache/master switch | `8003083` | 2026-11-09 | `EQOA_CACHE`, compass gating |
| Upstream keys | `53c5cf5` | 2026-11-09 | O group, G attack |
| Upstream rim/GM travel | `476fdac` | 2026-11-09 | Last map change today |

Upstream commits since the merge base: **134 at 91f9e72**, **155 at 14636b2**. Only five touch map paths
(`65f9b13`, `6c5bb03`, `8003083`, `53c5cf5`, `476fdac`).

### B. The 15 add/add files and the rest

Add/add (stage 2 and 3 only, no merge base): `groupmark.rs`, `map/build.rs`, `map/chrome.rs`,
`map/community.rs`, `map/group_waypoint.rs`, `map/markers.rs`, `map/mini.rs`, `map/mod.rs`,
`map/store.rs`, `map/tests.rs`, `map/tool.rs`, `mapsuggest.rs`, `tracker.rs`, `render/map.rs`,
`render/map.wgsl`.

Identical on both sides (no conflict): `map/cells.rs`, `map/markers_data.rs`, `map/picture.rs`,
`map/quests.rs`, `map/view.rs`.

Ours only: `map/journal.rs` (1,567 lines), `map/search.rs` (1,716), `map/suggest.rs` (238),
`map/theme.rs` (148).

The other 12 conflicted files are the integration points: `Cargo.toml`, `bindpage.rs`, `binds.rs`,
`hud.rs`, `main.rs`, `options.rs`, `play.rs`, `settings.rs`, `touch.rs`, `viewer.rs`,
`net/session.rs`, `render/lib.rs`.

### C. What depends on the map files

- In upstream, `render/map.rs` (`MapLayer`, `TileShot`) is used **only** by `map/build.rs`, `map/mod.rs`,
  `map/mini.rs` and `map/picture.rs`. No other subsystem imports it.
- Map hooks live in `play.rs` (frame, context, compass), `hud.rs` (waypoint dots, minimap pick, tracker),
  `settings.rs` (waypoints, options), `options.rs` (rows 41–45), `binds.rs` (M/N/O/zoom/group),
  `touch.rs` (long-press group waypoint), `viewer.rs` (wheel/pointer/minimap key, shots), `main.rs`
  (`map shot`), `net/session.rs` (quest data and GM travel) and `web/main.js` (OPFS cache).
- `esf` terrain-alone support and `tools/gen_map_markers.py`, `tools/gen_quest_guide.py` are shared and
  byte-identical between the two refs; they do not choose a side.
- **Light Shadows check:** "a map's static casters" in `render/lightshadow` means the *light/shadow map*
  (a cube map of depth), not the world map. It shares no file and no type with `render/map.rs`; it is
  unrelated. Light Shadows does, however, add the `lights` field to `render::View`, so any merge must add
  `lights: None` to our `map/build.rs` (upstream already has it).

### D. Merge mechanics and compile hazards

`git merge-tree --write-tree` against `origin/master` + `develop` gives 27 conflicted paths. Known
incompatibilities to reconcile when combining:

1. `Map::draw` / `Mini::draw_mini`: upstream now takes `(window, size)` for Render Scale; ours derives
   size from the target and adds `draw_circle`/`draw_circle_range`. Our render-scale fix predates theirs;
   adopt theirs and layer the circle mask on it.
2. `render/map.rs` vertex layout: ours adds a fourth `circle` attribute and shader discard; upstream's
   version has three. Keep ours; it is additive.
3. Settings/options: theirs adds `map` and `quest_helper` master booleans and option IDs 41–45; ours uses
   fork IDs 1043–1046 on a "Map & Quests" page. Do not reuse IDs; port the masters as new fork IDs and
   keep our labels.
4. `binds.rs`: ours has `map_search` (F2) and `map_stop` (F4) rows; theirs has O for group waypoint and G
   for attack. Pick one layout deliberately; the safest is upstream's O/G plus our F2/F4, since G was free
   in ours and O was not used.
5. `net/session.rs`: GM travel needs upstream's `map_travel` field and reply handling; ours has diverged
   heavily (245 added / 597 removed lines), so port the travel hunk rather than the file.
6. `store.rs`: add the `EQOA_CACHE` branch above the platform cache directory; keep our journal cache
   untouched.
7. `mod.rs`: ours removed `settings::map()`, `Map::off()` and the quest-helper gate. Re-add them; they are
   small and do not collide with search/suggest state.
8. `mini.rs`: ours keeps both "!" and "?" on the rim; theirs keeps only own "?" on the rim. Port the rule
   into our `on_rim`/mark loop.

### E. Method and confidence

Read-only: `git log`, `git show`, `git diff --numstat`, `git merge-tree`, `git cat-file`, `git grep` with
`safe.directory=*`; no cargo, no checkout, no commit. Line counts are raw file lines at the named refs
(blank lines included), so they differ slightly from `--stat` output. The merge-tree is a dry run and
does not modify the index. This comparison did not compile either side, so the compile hazards above are
from reading the diffs, not from a build. Upstream moved from 91f9e72 to 14636b2 during the review; the
newer ref was used and the difference is noted. `origin/master` was not fetched.
