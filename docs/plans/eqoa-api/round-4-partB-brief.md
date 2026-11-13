# EQOA round 4 Part B: journal and search mods (fresh Sol, medium)

Worktree `<worktree:eqoa-api-r4>`, branch `feat/eqoa-api-r4`.
`git -c safe.directory=*` on git. Read: `docs/plans/eqoa-api/round-4-brief.md` (owner-approved
specs of mods 1 and 2, the Gates), `docs/plans/eqoa-api/round-4-report.md` (Part A and its fix round:
the API you build on), round 3's mods in `bundled-mods/` (eqoa_stats_box etc.: how a bundled mod,
its keys `eqoa.input.bind`, option rows, HUD layer and tests are done), and the native code being
replaced: `crates/eqoa/src/map/search.rs`, `map/journal.rs`, `map/theme.rs` and their hooks.

## Goal

The two mods below do what the native search and journal do now, for the player, and the native
search/journal code leaves upstream's files. Behaviour parity with today's native code is the
acceptance test: port its rules, limits and texts, do not redesign.

## Small API additions first (spec them in EQOA-API-SPEC.md, test each, negative controls)

1. **Shared files between mods.** mod.json `"shares": ["data/journal.json"]` lets a mod publish files
   from its private area read-only; another mod reads them with `eqoa.files.load_json("@<mod_id>/
   <path>")` (same caps and worker as load_json). Event `shared.changed{mod, path}` after the owner
   mod's save lands. Reading a non-shared file, or a disabled mod's file, fails with a clear error.
2. **Player-driven import/export (native only).** `eqoa.files.import{title, extensions}`: only
   accepted inside a pointer/key event handler (a player click or key); opens the native file dialog
   off the frame thread (as journal.rs does with rfd), copies the chosen file into the mod's private
   `imports/` and reports its path through `resource.ready` (cancel = `resource.failed` code
   "cancelled"). `eqoa.files.export(private_path, file_name)`: same click rule; copies a private file
   to the Desktop under `file_name` (no overwrite prompt needed: add " (2)" etc.). Both are visible
   in the Addon Manager as permissions the mod declares (`"permissions": ["import","export"]`).
3. One-time migration (fork-owned native code, first start of this build): copy `<config>/journal.json`
   into the journal mod's private `data/journal.json` if that does not exist yet. The original file
   stays where it is, untouched.

## Mod 1: bundled-mods/eqoa_creature_journal (on by default)

Port journal.rs: record from entity events (Part A item 10 fields) with the same filters
(journal.rs creature filters: not self/pet, creature type, p256<2, no corpse, p255==0, named), the
same format `{"<name>":{"<id>":{"world","level","pos":[x,y,z]}}}`, dedupe (30 units at the same
level), caps (80,000 spawns, 50,000 per import, 16 MiB load, 14 MiB save), save after 30 s quiet or
every 120 s through `save_json_async`, flush on disable. Shares `data/journal.json`. Import accepts
FrontiersForge/PCSX2 journals exactly as journal.rs does; Export writes EQOA-Creature-Journal.json to
the Desktop. Import/Export are triggered from the search box footer (mod 2) through API addition 4
below, and also from two option rows "Import journal..." / "Export journal" in the journal's own
Mods & Addons options (so they work with search off).

4. **Mod commands.** mod.json `"commands": ["import_journal","export_journal"]` declares commands
   other mods may send; `eqoa.mods.send(mod_id, command, args?)` delivers event `mod.command{from,
   command, args}` to that mod at the next event boundary. A command sent from inside a player
   click/key handler is delivered marked player-initiated, so the receiver may call
   `eqoa.files.import/export` (the click rule follows the command, once). Undeclared commands,
   disabled or missing targets fail with a clear error; bounded queue per VM.

## Mod 2: bundled-mods/eqoa_map_search (on by default)

Port search.rs: F2 (mod key, default F2, on Controls) opens the box over the M map or the minimap;
the small 20x20 button under the minimap (HUD layer + `claim_pointer`, placed from
`eqoa.hud.layout()` minimap rect, follows Move HUD); tabs Creatures / Group / Places; text input up
to 80 chars, Tab, Up/Down, Enter (sets the guide and closes), Escape, Backspace; the status texts
("Live/Known/Waypoint/Group ... N", "Unavailable here"); sources: the catalogue (move
`crates/eqoa/resources/map-data/map-entities.json` + its README/licence notice into the mod's folder;
keep the SHA-256 pin test pointing at the new place; Forge x,y,z coordinates as search.rs converts
them), the journal (shared file, reload on `shared.changed`), live entities, group, places
(`eqoa.map.markers` camps/coaches). Destination: nearest live entity, else nearest recorded spawn;
`eqoa.map.set_guide` with `entity_id` for live (follows; `guide.lost` -> clear as today when a group
member leaves) and `dots` for every known spawn of the name; F4 (mod key) clears. Typing must not
move or attack (held combat/camera state released while the box has input, as
`release_map_search_holds` did). Look: dark surfaces, red accent and the Space Mono font (move
`resources/map-ui/SpaceMono-Regular.ttf` + OFL into the mod; `eqoa.fonts.load`), same layout as
today; pixel-exact not required. Save one PNG capture of the open box over the map and one over the
minimap to `docs/plans/eqoa-api/r4-search-*.png` for the owner (not a golden test).

## Removal from upstream's files and migration

- Delete `map/search.rs`, `map/journal.rs` and their hooks/feeds/routing in `map/mod.rs`,
  `map/chrome.rs` (search_anchor), `map/mini.rs` (search inside minimap), `play.rs` (search holds,
  feed, journal record, key/click routing, busy/move/look locks that only served search),
  `viewer.rs` (F2/F4, text/key, wheel/pointer/touch routing for search), `binds.rs` (`map_search`,
  `map_stop` rows). Restore upstream's text in those places (compare with origin/master 11aa5f0).
  Keep what belongs to the round minimap (circle, hover cards) for Part C.
- Saved binds: a player who rebound map_search/map_stop keeps their key on the mod's keys (one-time
  migration in fork-owned code, like round 3's mod keys).
- `theme.rs`: keep only if the round minimap still needs it (Part C removes the rest).
- Tighten every budget to its new measured value in `docs/fork-footprint.json`; report before/after
  of fork lines in upstream files and the dry-run conflicts (`git merge-tree --write-tree --name-only
  HEAD origin/master`).

## Tests

Mod behaviour tests from Play/protocol-shaped data through the real runtime (as round 3's mod tests
do): recording + filters + dedupe; journal reload > 1 MiB; import of a Forge-format file; export file
on disk; search finds live/known/group/place; Enter sets a guide with dots; guide follows and clears;
F4 clears; typing a name containing M, P, O, G does not trigger map keys or movement; migration of
journal.json and of rebound F2/F4. Each with a negative control you ran. Frame time: measure the
release-mode per-frame cost with both mods on and a 1.4 MB journal (assert the workload ran).

Gates as in the brief (addon suite, full client suite once at the end, wasm check, release build
last, footprint PASS, zero-addon gate, bundled Wereoxxs/ARPG/Modern UI and round 3 mods still pass).
Shared `CARGO_TARGET_DIR=%USERPROFILE%\Paseo\targets/shared`, mkdir lock
`%USERPROFILE%\Paseo\cargo-build.lock`, cargo in the foreground with a timeout, one build at a time.
Long output to a file; read summaries. Commit after each step; LoopRook; no push/merge/tag.

Report: add a "Part B" section to `docs/plans/eqoa-api/round-4-report.md`: what changed, each test
with its negative control, counts per suite (pass/fail/ignored/SKIP), frame-time numbers, footprint
before/after, parity gaps (anything the native code did that the mods do not), not done. Final
message: 6 lines.
