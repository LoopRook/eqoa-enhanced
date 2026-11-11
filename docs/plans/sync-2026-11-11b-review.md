# Review: sync 2026-11-11b (upstream 11aa5f0 into develop acafc33, branch sync/2026-11-11b at 141ed71)

Reviewer: independent, read-only. No cargo run. The footprint checker was run (it only reads git): PASS
against base 11aa5f0.

## Verdict: APPROVE WITH CHANGES

The code merge is sound. No upstream line is lost or altered in the four hand-resolved files, upstream
tests are verbatim, and the fork hooks are reapplied. Two documentation corrections are needed before
merge (one is a wrong claim in the report, one is a CLAUDE.md rule that is now stale), plus one test gap
worth closing. Nothing found blocks the owner's playtest.

## How I checked "nothing of upstream lost"

For every file upstream touched in a3cedec..11aa5f0 (26 files), I took each line upstream added and
checked it is present in HEAD (whitespace-normalised). Only four lines are absent, and all four are
upstream lines the fork deliberately re-wrote to add its hook in place: the three Move HUD hit-test lines
in play.rs (compass, quick menus, chat; the fork keeps its own `compass_visible`, `hud_menu_visible` and
`chat_window_visible` guards) and the `Gfx { .. }` struct literal in viewer.rs (adds `forge_runtime`).
`hudlayout.rs`, `gpulost.rs`, `web/`, `crates/eqoa/src/web/mod.rs` and `UPDATE-NOTES.md` are identical to
upstream. The Move HUD begin/end pairs (Player, Group, Pet, Chat, Target, Minimap, Quests, Meter,
Effects, Breath, QuickMenus) are all present, and the fork's hooks (`draw_active_effects`,
`draw_chat_window`, `draw_hud_menus`) sit inside their begin/end pairs.

## Point 2: the browser guard and device loss

- The removed `web::graphics_lost` exit in `viewer.rs::window_event` was **upstream's code, not the
  fork's.** It is at a3cedec:viewer.rs:4773 (the fork inherited it) and upstream deleted it itself in
  bc7754c ("A lost graphics device is made again in place"). HEAD's viewer.rs matches 11aa5f0 there, so
  this was not a hand-resolution at all, and the report's "obsolete fork browser guard" is the wrong
  attribution.
- Upstream 11aa5f0 no longer exits the main loop on a lost WebGL2 context. It recovers in place:
  `watch_webgl` marks the loss, `viewer::recover` asks `web::remake` for a new device and `reattach`
  rebuilds on it. The loop exits only through `give_up_gfx` (`web::give_up`, with the page's notice) when
  no device can be had or the loss repeats within `gpulost::REPEAT`. The spell-visual preview
  (`fxview.rs:914`) still exits on `web::graphics_lost`, unchanged from upstream.
- CLAUDE.md line 93 ("Browser event loops must exit on web::graphics_lost after a lost WebGL2 context")
  is therefore wrong for the main viewer and right only for the `fxview` preview. Suggested text:
  "A lost graphics device is recovered in place (`gpulost`, `viewer::recover`; a page's new device comes
  from `web::remake`). The main browser loop ends only through `give_up_gfx`/`web::give_up` (no device to
  be had, or a repeat loss within `gpulost::REPEAT`); never add an unconditional exit on
  `web::graphics_lost` to it, that defeats recovery. Only the spell-visual preview (`fxview`) still stops
  on `web::graphics_lost`." (Upstream's own CLAUDE.md, preserved in `docs/upstream/CLAUDE.md:328-330`, still
  says "each event loop ends on `web::graphics_lost`"; upstream did not update it. Leave that file
  verbatim, but note in the sync notes that it is stale on this point.)
- Addon runtime on recovery (`viewer.rs:3500`, `runtime.rs:430`): ordering is right. `for_new_device`
  calls `shutdown()` first (Lua shutdown, `flush_profile(.., true)` so enabled state and mod settings reach
  disk, window detached, graphics dropped), then builds a fresh `Runtime::new`, which re-reads the saved
  profile for the enabled set, and carries `default_server` and `open`. It runs while the old Gfx is still
  alive (old device and renderer drop at the end of `reattach`), and recovery happens between events, so
  no draw data is outstanding. `Drop` runs `shutdown` again on the consumed value; it returns early on
  `stopped`, so no double-drop and no double flush. The failure path (`give_up_gfx` then `exiting`) still
  calls `shutdown` on the intact Gfx. I found no lost settings and no double-drop. What is lost, by
  design: in-memory Lua state of the mods (counters, windows' transient state) restarts on recovery, and
  the new runtime attaches lazily on the next frame, so HUD hide claims are published false for a frame.

## Findings

### MEDIUM

1. **CLAUDE.md line 93 is stale and the report misattributes the removal** (`CLAUDE.md:93`,
   `docs/plans/sync-2026-11-11b-report.md`, viewer.rs row). Failure scenario: a later agent reads "must
   exit on web::graphics_lost", re-adds the guard to the main browser loop in a future sync, and browser
   device recovery (the point of upstream's change) silently stops working; or the footprint reviewer
   believes the fork carried a guard of its own. Fix: replace the line with the text above; correct the
   report to "upstream's own code, removed upstream in bc7754c".
2. **The only test of the fork's recovery hook proves little** (`runtime.rs:838-859`).
   `device_recovery_keeps_paths_server_and_open_intent_but_releases_graphics` builds `old` that never
   launched, so `next.graphics.is_none()` is true with or without the shutdown, and nothing shows that
   mod settings or the enabled set survive a recovery of a runtime that is running. The negative control
   (idle stays dormant) is real but covers only laziness. Failure scenario: a later change to `shutdown`
   that stops flushing the profile passes this test and loses the player's mod settings on a device loss.
   Fix: use the existing `launch_headless` helper to run an active runtime, change an option, call
   `for_new_device`, and assert the new runtime reads it back (and that the old one is stopped).

### LOW

3. **After a native device recovery the local report loses its screenshot and GPU line.**
   `fork_report::surface` (adds `COPY_SRC`, calls `report::set_gpu`) is only called from `resumed`
   (`viewer.rs:4933`); `remake()` (`viewer.rs:3446-3465`) builds the new surface config without it.
   `fork_report::shot` filters on `COPY_SRC`, so it degrades quietly (no crash): a report saved after a
   recovery carries no picture and the pre-loss GPU name. Fix needs one more hook line in `remake`
   (shared file, budget check) or leave and note in `docs/backlog.md`.
2b. Same area, cosmetic: `FORK-UPDATE-NOTES.md` should say recovery restarts addons (their in-memory
   state), so a mod author is not surprised.
4. **Layout cache is not directly tested for hit or for raid/pet invalidation**
   (`fork_ext/addons/hud_layout.rs:34-45, 139-165`). The key is the output itself (window size,
   `HudState` including `in_raid`, `pet_y`, `pet_h`, and the four pieces' shifts) so move, resize, reset
   and raid/pet change all invalidate by construction, and `record_cached` additionally compares the JSON
   it holds. The new test passes through move and reset (key change) but never asserts a hit (same `Rc`)
   or a `set_hud_state` change. Low risk; add two assertions if cheap.
5. **Per-frame cost with mods is raised, the 5 ms debug limit is unchanged but its pre-sync baseline is
   not reported.** With no mods nothing changed (`Runtime` stays `Dormant`; `hud_layout_json` runs only
   when Lua calls it; `runtime.rs` has only the new `for_new_device`). With the three bundled mods, each
   render now calls `eqoa.hud.layout()` (4 shift reads + a deep JSON compare in `record_cached`).
   Final measurement 2.97 ms against 5 ms; earlier attempts measured 8.3 and 6.98 ms. Commit 8b62500
   recorded the four-mod benchmark at "under 0.5 ms" (build type not stated here), so a real cost increase
   may have been absorbed by the limit. Suggest recording the pre-sync number in the report and, if easy,
   returning the cached Lua table by key (skip the JSON compare).
6. **A player-edited manifest with an unedited script** (`bundled.rs::extract_single_file`): each file is
   judged on its own hash. A player who edited only `mod.json` keeps it (no `eqoa.hud.layout`
   permission) while `main.lua` updates to the new script that calls `eqoa.hud.layout()`; that mod then
   faults until the permission is added. Narrow, and the diagnostic is visible; mention in the update
   notes.
7. **Pre-existing, not from this sync:** upstream test `hidden_the_huds_buttons_take_no_click`
   (`play.rs:12279`) is the fork's compass variant, not upstream's quick-menu title-bar test (11aa5f0 has
   the latter; the change predates this merge, already so at acafc33). The upstream hud.rs `pick` test is
   likewise fork-modified. "Upstream tests verbatim" holds for the ones this range touched, not for these
   two. Also, `map/mini.rs` test `border_outer_points_match_the_gpu_circle_at_every_aspect` sits between
   a doc comment and `the_quests_steps_point_the_way_from_the_rim`, so that doc now documents the wrong
   test (cosmetic, pre-existing).
8. `hud_layout.rs:156-161` `unreachable!()` in `to_rect` is safe today (four callers) but a panic if a
   fifth piece is ever passed.

## Point 3: round minimap in a moved rectangle: correct

- `mini.rs:502-519`: `outer`/`inside` come from upstream's `window_rect_moved(.., moved)`; the fork's
  `circle` is `inside`'s inscribed circle, so circle, terrain mask, UI mask, `circular_rim` marks, dots,
  hover (`dot_under` against `circle`), the N label (`r.ui.shift = moved`, restored to zero straight
  after), the search button and guidance (`outer`, `inside`) all follow. The ring border is pushed in window
  coordinates before `ui.shift` is set, so it is not shifted twice.
- Clicks: `Hud::pick` finds the moved rectangle (upstream's begin/end note), then `play.rs:5064` and
  `viewer.rs:599, 5651` test `mini_contains` against the moved circle (`frame_px` = moved `outer` is the
  fallback). `map_move_tests.rs` checks at 640x480, 1280x720 and 768x1024: new centre hits, old centre does
  not, rectangle corner does not, every quad (border, N, hover card, search) translated by the shift
  within 1.1 px, old search position inactive, new active. It skips without a GPU adapter (counted in
  the report). Not weakened. One behaviour that is old, not new: a click in the rectangle's corner outside
  the circle returns false from the `Pick::Minimap` arm and falls through to the world.

## Point 4: addon HUD rectangles: correct

`hud_layout_json` adds each piece's live `hudlayout::shift` to `frame_x/frame_y` before the
side/top/bottom anchor transform, which is how `begin` shifts the stock drawing (frame pixels after
anchoring), so a "bottom" Target moved by dy matches. Group members (4, matching `Group` of 4 others),
Player, Target and Pet are covered; `eqoa.hud.frame()` is unchanged. The bundled mods subtract the
natural origin to get a delta (`frame_x - 10`, `- 348`, etc.), consistent with the numbers in the file.
Mod HUD commands are submitted after Target's `end`, so ui.shift is zero and they are not shifted twice.

## Point 5: bundled hashes: correct

The six hashes added to `bundled_shipped_hashes.txt` are the SHA-256 of the acafc33 (previous, v0.5.0)
copies of the three `main.lua` and `mod.json` (I recomputed them); the new files' own hashes are added
at install time from the embedded data. They already also appear in `bundled-mods/SOURCE.txt`, so the
addition is redundant but harmless. An installed copy is replaced only if its hash is a known shipped
one; anything else is kept and logged as player-edited (`extract_single_file`). Earlier unreleased
revisions (7ebf6a1, 13bcfc6) are in no tag; only dev-channel testers could hold them.

## Point 6: tests

No upstream test line was removed or altered in this range (only upstream's own rename of
`a_tag_is_a_line_under_its_name`). Sol's fork-test edits are legitimate: options fixture accounts for
upstream's new HUD Layout page (indices shifted, new rows asserted), circle radius tolerance 0.001 px for
a 15e-6 float error (negative controls remain: old centre/corner misses), FORK notes test follows the new
bullet, the 5 ms benchmark limit untouched. No `#[ignore]` added; no fork test found that asserts nothing
apart from the weak one in finding 2. Not run by me: cargo tests, the GPU picture tests (the report's
numbers are the only evidence), the footprint script I did run (PASS), the wasm check (not rerun).
