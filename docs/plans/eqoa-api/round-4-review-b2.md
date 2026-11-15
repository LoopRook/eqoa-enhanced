# Round 4 Part B, independent review b2 (bundled mods, input isolation, native removal)

Range `a7f7c5f..d9f177a` (13 commits), branch `feat/eqoa-api-r4`. Read-only: no cargo, nothing run.
Everything was read from committed blobs (`git show d9f177a:<path>`). Native reference:
`9e7257a:crates/eqoa/src/map/search.rs` and `journal.rs`; upstream reference `11aa5f0`. Line numbers
below are `d9f177a` unless prefixed `search.rs:` / `journal.rs:` (the deleted native files at
`9e7257a`). Statements about runtime behaviour are from reading code, not from running it; where
I estimate, I say so.

## Verdict: APPROVE WITH CHANGES

The API fixes (b1) are really fixed and the native removal is clean. The journal mod is a faithful
port. The search mod is mostly faithful in rules and texts, but two things must change before this
reaches a player: (1) the result list can never finish building while anything nearby moves, and
(2) a player updating from 0.5.2 very likely gets both mods switched off, so native search and
journal recording vanish with nothing replacing them. Several MEDIUM parity gaps follow (no
feedback for import/export, search button overlapping the map header in the committed capture,
the Group tab lists yourself, a click outside the box also acts on the map, F2 with the minimap
off does nothing).

## HIGH

### H1. Result list restarts on every entity/player event, so it can stay on "Searching..." forever (search.lua:60, :79-82, :120-135; main.lua:22-24)

`observe()` sets `M.dirty=true` unconditionally (search.lua:60). It runs on every frame in which
any of `entity.added/changed/removed`, `group.changed`, `map.changed`, `player.changed` fired
(main.lua:22-24 set `s.refresh`). `build_step` starts over when `M.dirty` (search.lua:121 ->
`build_start`), and `npc_step` does 1000 source rows per frame (search.lua:85). The Creatures
build needs catalogue 7,181 rows + journal rows + live rows = at least 8 uninterrupted frames, many
more with a real journal (23,000 rows = 31 frames; 80,000 = 90). `entity.changed` fires whenever an
entity's JSON differs, and `position` is part of it (api_updates.rs patch_entity), so one wandering
NPC every few frames, or any `player.changed` (hp/power regen, xp, movement), restarts the build.

Failure scenario: stand anywhere with wandering NPCs, press F2, type "wolf". The box shows
"Searching..." (main.lua:111) and Enter only sets `enter_pending` (search.lua:159), which is
consumed only when a build completes (search.lua:134). Nothing ever happens. The tests do not see
this: both search tests run with a frozen world (`settle()` steps 80 frames with no event;
map_mod_tests.rs:455-460, :494-496).

The native code solved exactly this on purpose: `search.rs:285` "Keep a stable result order while
nearby NPCs walk", invalidating only on names/counts/levels/zoning/player movement. Without it, even
when a build does finish, rows re-sort by distance under the cursor (search.lua:73-76), so Enter can
pick a different creature than the highlighted one.

Fix: do not set `dirty` from `observe()`; mark dirty only when the query, tab, world or the set of
live names/levels/counts changed (native rule), keep sorting by distance at build time only, and
do not abort an in-progress build for a position-only change. Test: moving live entity every frame
during a >1000-row build must still complete; negative control = current code.

### H2. A player updating from 0.5.2 gets Journal and Search switched off (manager_profiles.rs:94 and :341, manager.rs:888-899, runtime.rs:147-152)

`mod.json` `"enabled": true` only matters when no saved profile exists. Production calls
`enable_profiles` (runtime.rs:530, :566). For an existing `Default` profile, every addon absent
from `doc.enabled` becomes `false`: `entry.enabled_intent = data.enabled.get(&entry.key).copied().unwrap_or(false)`
(manager_profiles.rs:94, :341) and `Ok(Some(s.data.enabled.get(&key).copied().unwrap_or(false)))`
(manager.rs:888); `initially_enabled` is consulted only when the Default document is first created
(named_profiles.rs:351-353) or with no profile store. `bundled::install_bundled_mods_if_needed`
only extracts files; nothing adds the two new ids to an existing profile.

Failure scenario: any 0.5.2 player who ever started the addon runtime has a Default profile. After
updating, native F2/F4, the search button and creature recording are gone (native removal), and both
mods sit in Mods & Addons as Off until the player finds them. Sightings are not recorded in the
meantime. The migration (journal.json copy, key rebinds) does run, so the data is safe, but "nothing
lost" is false in practice. FORK-UPDATE-NOTES says "enabled-by-default", which is true only for new
installs.

Not run; please prove or disprove with a test that builds a Default profile written without these
two ids, starts `Runtime::new` + `enable_profiles`, and asserts both are enabled. If confirmed:
treat bundled mods first seen after the profile was written as `initially_enabled` (once, recorded
in the profile so a later manual Off sticks). Round 3's four mods may have the same hole; check.
Also none of the new manager tests use `enable_profiles` (map_mod_manager_tests.rs:128-260), which
is why this is invisible.

## MEDIUM

### M1. Import/Export give the player no feedback; footer lacks native texts (main.lua:15-18, :114-115; journal main.lua:116-122, :143-149)

Native showed "Imported N spawns", "Saved to EQOA-Creature-Journal.json", "Import: <error>", "Export: <error>"
for 6 s in the footer (search.rs:697-709, :934), plus `Journal: N creatures, M spawns`, and revealed
the exported file in the file manager (journal.rs:822). The mods: the journal only `print`s; it ignores
`resource.ready kind="export"` and `resource.failed kind="export"` entirely, and `import` failures are
silent. The search footer shows `Journal: <n> spawns` (creature count dropped) and `s.message`
(main.lua:114) which is set only when `mods.send` itself fails and is never cleared (the next success
sets it nil, but an error like "Journal unavailable" stays on screen indefinitely). Scenario: player
clicks Export; a file silently lands on the Desktop (or fails: Desktop missing, journal over 14 MiB,
disk full) and nothing on screen says which. Also a collision now writes `EQOA-Creature-Journal (2).json`
where native overwrote the one file; the player is never told the name. The native import also fell back
to `Desktop/EQOA-Creature-Journal.json` when the dialog gave nothing (journal.rs:846-865); the mod ends
with `cancelled`. Fix: journal publishes result text (event to search, or a toast/option row), search
shows it for 6 s; port the creature count.

### M2. Search button/panel overlap the full map's header; the committed capture shows it (main.lua:64-67; r4-search-map.png)

On the open map the mod hard-codes `x,y = 16,44` and the panel at `py=70`. In `r4-search-map.png` the
"Search F2" button sits on top of the line "Drawing the map: 0 of 221 areas". Native placed it from the
chrome footprint (`chrome::search_anchor`) and had a test (removed in this range, chrome.rs ~L531
`search_toolbar_clears_coordinates_progress_and_notices`) because pointer coordinates, progress and
notices ("Marker sent for approval") grow the header band. The mod has no equivalent, so those lines
are covered whenever they appear. Fix: expose the header band height through the map API
(`eqoa.map.get()`/layout) or place the button where no header text ever goes, and re-capture.

### M3. Group tab lists the player themself (search.lua:52-53)

`group.get().members` includes the local player (`self_leader` is derived from `members.first()`,
api_updates.rs group_value; the native-tests fixture has a "Self" member). Native excluded it
(`Some(m.id) != me`, deleted play.rs block in e13b39c). `observe()` only filters `g.active`.
Scenario: in a group, Group tab shows your own name first (distance 0); Enter guides you to yourself.
Also native listed inactive/other-world members as greyed rows ("Position unavailable"/"In another
world"); the `g.active` filter hides them. Fix: skip `g.id == player.id`; keep inactive members as
non-selectable rows.

### M4. A click outside the box closes it and also acts on the map or the world; wheel scrolls both (main.lua:44-54, :96)

Native consumed the click (`search.pointer` returned true; play.rs hunk in e13b39c) and the wheel.
The mod claims only the panel and button rectangles (`claim_pointer`, pointer_api.rs: claims are
rectangles; unclaimed clicks pass through). The `pointer.pressed` handler closes the box but the same
click goes on: on the M map it places or removes a personal waypoint (map click), in the world it
may target/attack. Right-click close likewise. The `pointer.wheel` handler moves the selection for a
wheel anywhere on screen while the same wheel also zooms the minimap/camera. Fix: while open, claim
the whole window rectangle (panel hits still win) and handle wheel only over the panel.

### M5. F2 with no minimap does nothing; native opened the map (main.lua:57-59)

Native F2 ran `p.map.show()` when neither the minimap nor the map was up (deleted viewer.rs hunk in
e13b39c). The mod's `draw()` closes the box in that case (`if not mini and not map.open then ... s.close()`),
so with Minimap off, or hidden by a UI mod's hide lease, F2 toggles and snaps shut; Search is reachable
only after pressing M. Same for the "Map" master option off. Fix: when neither is up, open the full map
(needs an API to show the map) or say "Open the map to search" instead of silently closing.

### M6. Guide semantics weaker than native: no upgrade to live, static waypoint, world-blind status (search.lua:136-181)

Native `destination()` is re-evaluated every frame (search.rs:377-401): a known-spot guide switches to a
live creature when one appears, the nearest recorded spot tracks the player, a waypoint guide follows
the pin and clears when the pin is removed (`set_waypoint`, search.rs:316), and other-world guides show
"Unavailable here". The mod chooses once: `choose()` passes `x,z` for known spots (search.lua:145-146) or
`entity_id` for a live one, and `status()` recomputes "Live N" from `M.live` (search.lua:174-176) while
the drawn line (host guide) still points at the known spot. Scenarios: (a) guided to a Known spawn, the
creature spawns in view: status says "Live 40" but the line points at the old spawn; (b) waypoint guide
stays on the old pin after the player moves it; (c) after zoning, "Known 12345: Name" (static `r.at`,
no world check at search.lua:173-180) instead of "Unavailable here"; (d) `guide.lost` with reason
`world_left` -> `reacquire` -> `set_guide` fails (`not_in_world`) but `M.guide` stays, so the X button and a
status line remain with no guide drawn (main.lua:39-41, search.lua:145,150). Also the "Track waypoint"
button (search.rs:805) has no counterpart; only the Places-tab "Personal waypoint" row. Fix: on
`entity.added` of a matching name while guiding a known spot (and `player.changed` movement beyond a
threshold), re-run `choose`; handle waypoint changes (`map.changed`); clear `M.guide` when `set_guide` fails.

### M7. Per-frame and per-event cost scales with catalogue/journal, and two paths sit near the fatal instruction budget

The Lua budget is 4,000,000 instructions per iteration across all callbacks and exceeding it is fatal
(host.rs INSTRUCTION_BUDGET, `fatal` + pcall rethrow). Measured frame cost in the committed test is for a
frozen world with a 23,000-spawn journal; release numbers are still "pending" in the report.

1. `observe()` runs every frame in which any entity/player event fired, whether or not the box is open or
   a guide exists (main.lua:22-24, search.lua:43-65): `eqoa.entities.list()` (array, never cached by
   `record_cached`, so it rebuilds up to 128 guarded entity tables each call), `group.get()`, and
   `eqoa.map.markers(w)` four times (each clones and converts the static marker list; one cache slot
   shared by four worlds, so it thrashes). Static places can be built once; live/group only while
   open or guiding. (Estimate: sub-millisecond per frame, but it is on the frame thread every busy
   frame and the measured 3 ms mean does not include it.)
2. `choose()` scans the whole catalogue and the whole journal twice (search.lua:144 and :148): at 7.2k +
   80k rows that is about 175k loop iterations, roughly 2 to 2.5M VM instructions (estimate), against
   a 4M per-iteration budget shared with the frame's other callbacks; `guide.lost` repeats it
   (main.lua:126). A budget overrun is fatal for the mod. Index spawns by lowercase name at load
   (a table name -> list) so choose is O(spawns of that name).
3. The unfiltered Creatures sort (search.lua:69-77): about n log n comparator calls for every distinct
   name (about 7k+ names; estimate 1.5 to 2.6M instructions) all inside the single frame that finishes the
   build. The report already records one budget overrun here, fixed by pre-computing distances; margin
   unknown. Sort incrementally or measure `instructions_remaining` in a test with 80k spawns / 10k names.
4. Per keystroke the whole build restarts and scans 1000 rows per frame (search.lua:85): journal rows
   do `searchable..` concat + `string.format` + `lower` for each row (search.lua:92), so 80k spawns is 80+
   frames (about 1.3 s at 60 fps, more at low fps) and ~8 MB of garbage strings per keystroke before
   any result shows; native filtered in one synchronous pass over names with a cached result.
   Index per name (aggregate once at load) and filter names, not spawns.
5. Every journal save fires `shared.changed` and the search mod re-reads and re-materialises the entire
   journal even with the box closed (main.lua:25-27, search.lua:12-39): at the 80k cap that is a 14 MB
   JSON parse on the worker, 160+ materialisation slices and about 80k new tables while the old copy is
   still live (rough peak 70 to 90 MB of a 128 MiB heap, estimate not measured). Reload lazily on next open
   and only when the file changed.
6. The journal's `save_json_async` copies the whole 80k-spawn table synchronously on the frame thread at
   every save (30 s quiet / 120 s max). Part A measured about 4 ms per 14 MiB snapshot in isolation; no
   80k-spawn hitch is measured in this range.

### M8. `pointer.wheel` steps are fractional, `selected`/`offset` can become non-integers (main.lua:50-54)

`steps` is the raw wheel delta (`LineDelta` y or `PixelDelta/40`; pointer_api.rs emit). Hi-res wheels
and precision touchpads send fractions. `s.selected=max(1,min(#rows, selected - steps))` then holds e.g.
2.7, `s.offset=max(1, selected-5)` becomes e.g. 3.3, and `for i=s.offset,...` indexes `s.rows[3.3]` = nil, so
`r.name` (main.lua:106-108) throws inside the render handler on a precision touchpad after scrolling more
than 5 rows; Enter on a fractional selection does nothing (search.lua:159). Fix: round/step by sign
(`steps>0 and -1 or 1`), as native did (`key("up"/"down")` per wheel event).

## LOW

### Parity details (native had, mods lack or differ)
- Live position: native used `actor_pos` (dead-reckoned) for live creatures and the journal
  (journal.rs:379-380, search.rs Live::of); the mods use API `position` = `e.pos`, the last server position
  (api_capture.rs). Acknowledged earlier (a2 L8) but not fixed; journal coordinates and live follow lag by
  up to one server tick of movement.
- Journal stores names trimmed (journal main.lua:50); native stored `c.name` untrimmed.
- Guide visuals: native drew live dots (5 px, green) and known dots (3 px, amber), colours by kind
  (waypoint/group/known), solid line to live and dashed to known, and clamped to the round minimap rim
  (search.rs:1014-1073). The host guide draws red 3 px squares for known dots, one dashed style, no live dots
  (guides.rs draw).
- Journal search text: native matched words across all spawns of a creature (level/world/coordinates of any
  spawn, search.rs:~470-500); the mod requires every word inside one spawn's string (search.lua:91-93), and
  only that spawn contributes to the row (nearest/levels).
- `EQOA_MAP_DATABASE` and `map-entities.json` beside the exe (search.rs:181-240) no longer work; README/docs
  (README.md:53, docs/DEVELOPMENT.md:28, docs/PLAYER-README.txt:3, MAP-PLAYTEST.md) still tell players to
  keep `map-entities.json` beside the exe. The release ZIP still carries it for no use.
- Button label hard-coded "Search  F2" (main.lua:89) after the player rebinds; native showed the live bind.
  The minimap hover tooltip "Search / F2" is gone. The status line has no background and is drawn always
  at `x-300` (main.lua:93), not only on minimap hover.
- Autorun/follow are cancelled while the box is open (play.rs `autorun=false; following=false`, fork_ext/play.rs
  hunk of 2a5c8af); native let autorun and following continue (`move_ok ... || self.autorun || self.following`).
- Controller navigation of the box (D-pad up/down, confirm, cancel, tab; deleted play.rs key hunk) is gone:
  `prepare_controls` zeroes the pad report while text capture is on (fork_ext/mod.rs).
- Text input uses `Key::Character`/Space (input.rs route_mod_key), not winit's composed `event.text`;
  dead-key and IME text (US-International accents, CJK) cannot be typed, native could.
- Group guide is cleared when the member merely zones to another world (`guide.lost` -> `clear`, main.lua:39-41);
  native kept "Unavailable here" until the member left the group.
- Browser and non-mod platforms lose search entirely (native search worked in the web build for group/places/
  live). Probably intended (mods are native-only); say so in the notes.
- Native 1 Hz scan throttle is replaced by events; `events.dropped`/`state.reset` rescans (journal main.lua:153)
  are fine.

### Other
- journal main.lua:138-141: after a save completes while newer changes exist, `first_dirty` is not reset, so
  `now - first_dirty >= 120` is still true and the next save starts at once (and each save triggers a full
  reload in the search mod). Native reset `first_dirty_time` at snapshot time (journal.rs:428). Set
  `first_dirty = saving.started` or clear it at snapshot, keep it only for changes after it.
- journal main.lua:119: `export_journal` returns silently when `save(true)` returns nil (protected, merging,
  over 14 MiB); same for failures with no text (see M1).
- search main.lua:9-12: the shared journal is read once at start and then only on `shared.changed`; if the
  journal mod is enabled later or its VM was not yet registered, `resource.failed` is swallowed and the
  journal stays empty until the first save. Retry on failure.
- search.lua:133: `offset` is not clamped when rows shrink, so the visible window can start past `selected`.
- Dead code left in `map/theme.rs` (`magnifier`, `control`, `text`, `fill`, `frame`, `LIVE`, `GROUP`, `INPUT`,
  `RAISED`, `SELECTED`) now has no caller; Part C is stated to remove it, but expect dead-code warnings in
  the meantime. `crates/eqoa/src/fork_ext/report.rs::reveal_in_file_manager` may be unused now (not checked).
- A duplicate `mods/resources/map-entities.json` (1.74 MB) remains beside the new
  `bundled-mods/eqoa_map_search/resources/` copy; the pin test hashes the first and compares bytes to the
  second (async_json.rs:377-383, file_io.rs:1291). Fine for the gates; cleanup candidate. The footprint file
  still lists `crates/eqoa/resources/map-data/**` and `map-ui/**` as owned paths.
- Controls rebinding: key migration runs only when a mod registers a bind (bindpage.rs load_saved_key). If the
  player opens Controls and changes anything before a mod VM starts, the native binds writer drops the unknown
  `key.map_search=` line first. Round 3 has the same property; low risk.

## Answers to the five checks

### 1. Parity (mods vs native)

Matches (checked line by line): journal filters (creature, `class_code<2`, not corpse, `search_flags==0`,
named, not self/pet; journal main.lua:45-55), cross-world 30-unit same-level dedupe by grid (`near`, boundary
inclusive, :33-44; test covers dx=30 and 30.5), caps 80,000 / 50,000 per import (:26, :103), name <=64 bytes,
control-char and `\194[\128-\159]` rejection, id <=20 digits, level 0..255 integer, world wrap to i32, pos finite
(:62-72), local record wins on import, ids known skip, 30 s quiet / 120 s max timing (:162, with the L-note
above), flush on `disabling`, export name `EQOA-Creature-Journal.json`, 16 MiB load / 14 MiB save via the
async API, Forge catalogue `world[i]`/`pos[i]` with x=pos[1], z=pos[3], `w>=0`, finite < 1e7 (search.lua:4-11,
:26-30), tabs, 80-char limit counted in characters, Tab/Up/Down/Enter/Escape/Backspace (:152-169), status texts
"Live/Known/Waypoint/Group N: name" and "Unavailable here: name" (:171-181), detail texts, nearest live then
nearest recorded with catalogue winning ties (:136-151), guide dots (caps 80,000), F4 clear, group member leaving
clears (:61-64 and `guide.lost`), button 20x20 at the minimap's bottom-right (`m.x+m.width-20, m.y+m.height+2`
equals `mini_search_rect`), panel over-map left and over-minimap to the left of the minimap.
Missing or different: see H1, M1-M8 and the LOW list above ("every behaviour native had that the mods lack"):
stable result list, import/export feedback and file-manager reveal, import Desktop fallback, creature count,
Group excludes self, click/wheel consumption, F2 opening the map, guide re-evaluation, Track waypoint button,
header-aware placement, live dots and guide styling, `actor_pos`, `EQOA_MAP_DATABASE`/exe-side catalogue,
pad navigation, composed text, autorun while open, bound-key label.

### 2. Removal

Clean. I took every line deleted by `e13b39c` in `map/mod.rs`, `chrome.rs`, `mini.rs`, `play.rs`, `viewer.rs`,
`binds.rs`, `tool.rs`, `tests.rs`, `bindpage.rs` and checked it against `11aa5f0`: the only deleted lines that also
exist in upstream are bare braces/`return;`/`#[test]`; no upstream code was lost. `rg` finds no `map::search`,
`map::journal`, `map_search`, `map_stop`, `search.open` or `journal::` left in the six files (remaining "search"
hits are upstream's own player search/who/group search). Small leftovers:
- play.rs `if (self.map.open) && (self.mode != 0 || self.is_dead())` keeps stray parentheses; upstream is
  `if self.map.open && ...` (11aa5f0). Restore it (one line less divergence, one less conflict).
- binds.rs/bindpage.rs still carry the fork's test substitutions made to dodge F2/`b` (`f10`, `y`, `map` for
  `hide_ui`, plus added `"F2"`/`"F4"` helper arms), about +39/-37 and +9/-5 against upstream. With
  `map_search` (F2) and the `b` row both gone, upstream's test lines can be restored verbatim and the
  budgets tightened further, as the brief asked ("restore upstream's text").
- `theme.rs` dead items and the `mods/resources` duplicate, as listed.
- `viewer.rs` budget now +66/-11, `play.rs` +148/-55 etc.: tightened as claimed; the budgets look measured.
The native `Map::off`/`close` changes next to the removal are earlier round-minimap edits, not this commit.

### 3. Migration

Journal data: `map_migration` copies `<config>/journal.json` to the mod's private `data/journal.json` once
(marker `journal-migration-v1`), never touches the original, hard-link publish, tested including no-resurrect.
Fine, provided the mod runs: see H2. Keys: `key.map_search`/`key.map_stop` -> `mod.eqoa_map_search.open/clear`,
existing mod lines win including an explicit empty, collisions keep both; tested (bindpage.rs:342-360). The bind
ids `open` and `clear` match the mod (main.lua:19-20). Not done: any enable-state migration (H2), the display
label (LOW), and nothing is shown to the player that the journal moved. The old native saves between the copy
and removal cannot occur now (same release).

### 4. b1 fixes

- H1 (export name/extension): fixed. Allow-list `json|txt|csv|png`, no leading dot, no whitespace padding, 128
  bytes, no separators, reserved names via `m.key`; `round4b_review_export_refuses_executable_and_reserved_names`
  (file_io.rs:786) feeds `.bat .lnk .exe desktop.ini .hidden.json .json CON.txt x/y x\y file.json.`, plus a
  positive `Journal.JSON`. The report states a RED run ("unsafe .bat export"); I cannot verify it.
- M1 (command flooding): fixed. Separate command queue, 4 pending per sender, 4 KiB args; test floods and asserts
  a 2 MiB `entity.added` and a `json_saved` completion survive and a second sender still gets through
  (file_io.rs:767-785).
- M2 (Desktop/picked file under reparse policy): fixed for export (`quota_root: None` skips the link checks) and
  import (`read_selected` plain open, only the private copy sandboxed); Windows junction test exists. Real `rfd`
  and `desktop_dir()` still run in no test (cfg(test) replaces both), and the Desktop folder is still created if
  missing (deliberate).
- M3 (imports growth): fixed by `prune_imports` keeping three and `import_path` outside save quota; test saves a
  14 MiB journal after four 6 MiB imports.
- M4 (activation breadth): one shared activation across VMs, command authority non-forwardable and expiring;
  tests at file_io.rs:665 and :696. Rate limiting of exports was not added (design note in b1; allow-list
  bounds the damage).
Not covered by tests: the bundled mods' actual Import/Export wiring through the search box (the tests use the
journal's own option rows); search -> journal command click; pointer-click authority (only key).

### 5. Performance

See M7 (items 1-6) and H1. Summary: nothing per frame in the journal mod scales with journal size except the
synchronous save snapshot (every 30/120 s) and the rare full scan after `state.reset`/`events.dropped`;
`record()` does O(1) grid lookups per entity event and calls `eqoa.player.get()` (cached object) per event. In the
search mod, per-frame cost is `observe()` on every event frame (not gated on open/guide), the build restart bug,
and reloads of the whole journal after each journal save; per keystroke is a full rescan at 1000 rows/frame;
`choose()` and the unfiltered sort are the two spots close to the 4M-instruction fatal budget. The measured
"3.065 ms mean / 6.7 ms max" (debug, 23,000 spawns, 1.4 MB) is a frozen-world measurement with no moving entities
and no 80k journal, so it does not cover these.

## What I could not check
Release-mode timings, the real `rfd`/Desktop path, a live window with OneDrive Desktop, any non-Windows file
backend, whether `record_cached` thrash costs what I estimated, and H2 at runtime. The Part B report section is
still marked "in progress" (final gates, footprint table, dry-run conflicts are not in this range).
