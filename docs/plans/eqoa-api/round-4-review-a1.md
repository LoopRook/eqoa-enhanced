# Round 4 Part A review 1 (items 1-6, commits 9e7257a..3ffaaa4)

Reviewer: Claude Sonnet, read-only. Reviewed the committed tree at `3ffaaa4` (`git show 3ffaaa4:path`), not the
worktree, because another agent has uncommitted edits there. No cargo was run. I ran only
`tools/check_fork_footprint.py` (read-only).

## Verdict: APPROVE WITH CHANGES (three blocking items: H1, H2, H3)

The design is sound and the upstream footprint is small and neutral. But as committed, the live-play paths for
`map.get`/`map.markers`/`map.changed`, `map.view` projection and the texture UV, and `guide.lost` do not work, and the
named tests hide that because they bypass the live capture path. The footprint gate also fails. All three are
bounded fixes. Do not start Part B on this API until H1-H3 are fixed and re-tested through the live path.

## HIGH

### H1. The map/minimap/marker view is built once and never refreshed in live play
- `fork_ext/api_updates.rs:371-390` (`refresh`): `play.native_view(walker)` plus `CORE.publish(view)` runs only when
  `first || manual || in_world flips`. `native_core.rs` `publish` (the only place `map.changed` and `revision` are
  computed, `native_core.rs:392-405`) is called from nowhere else (`api_updates.rs:383`, `manager.rs:224` only).
  Afterwards `refresh` only patches the player, entities, group and abilities. It never touches `view.map`,
  `view.map_markers` or `view.minimap`. `api_capture.rs:205-213` (where they are computed) is reached only through
  `native_view`.
- Consequences once an addon is running:
  - `eqoa.map.get()` returns the first snapshot for the whole session. A waypoint placed by click or by a mod, or a
    world change inside the session, never shows. `map.changed` essentially never fires (only on the first
    publish/world flip). `revision` never advances.
  - `eqoa.map.markers()` dynamic kinds (quest, community, waypoint, group_waypoint) are frozen at the first snapshot.
    A quest accepted or completed later does not show.
  - `Projection` (`map_views.rs:15`, stored in `View.minimap`) freezes the player centre, scale and window at first
    capture. Scenarios: the player runs on, so `project()` is wrong relative to the drawn terrain. The player zooms
    with the wheel (or `minimap_wheel`) and `project()` still uses the old scale. The player drags the minimap in Move
    HUD, so `uv = outer/window` (`map_views.rs:150-157`) still crops the old rectangle. The window is resized, so
    `canvas.window != p.window` (`map_views.rs:125`) and `view.texture` is nil forever.
- Why the tests miss it: `map_state_comes_from_play_and_changed_event_has_negative_control`,
  `map_markers_use_native_quest_cache_and_filter_other_worlds`, `round4_map_view_projects_native_player_scale_...` and
  `round4_guide_follows_live_entity_...` (`api_capture_tests.rs:1827-1949`) call `p.native_view(..)` and
  `CORE.publish(..)` by hand each time. None goes through `forge_snapshot`/`updates::refresh`, which is the live path.
  `updates::key` (`api_updates.rs:295-340`) also does not include `map.open`-independent map state, mini scale/shift,
  window size or waypoints, so a cached snapshot would also be stale.
- Fix: refresh map state/markers on a map revision (waypoint set/clear, quests set, community, group waypoint, world
  change), and compute the minimap `Projection` live. Best: have `map_views::draw` (the Capture branch has `map`,
  `window`) or `mini_frame` write the `Projection` into a fork-owned thread-local each frame, using the player
  position from CORE, so scale/shift/window/indoor are always current. Add a test that starts a Play, calls
  `refresh` once, mutates (waypoint, quest mark, `mini.scale`, window size, Move HUD shift), calls `refresh` again, and
  asserts Lua sees the change. The negative control is the same test with the new code removed.

### H2. `guide.lost` never fires when an entity leaves in live play; the guide then lingers
- `guides.rs:54-56` `update()` is called only from `Core::publish` and `Core::reset` (`native_core.rs:301,408`).
  Entity despawns/changes in live play go through `api_updates::capture` -> `patch_entity` and do not call `publish`
  (see H1). The test `round4_guide_follows_live_entity_and_lost_is_owner_scoped` calls `publish` by hand, so it
  passes.
- Scenario: a mod follows a creature with `set_guide{entity_id=..}`; the creature dies/despawns. The draw code
  (`guides.rs:235-238`) finds no entity and draws no line, but the `Guide` stays registered and the mod gets no
  `guide.lost`, so it never clears the search UI's "tracking" state. It only fires on logout/zoning (via `reset`).
- Fix: call `guides::update` from the entity patch/remove path (`patch_entity` or after `flush_deferred`), and add a
  live-path test (`refresh`/`capture`, then `flush`) with a negative control.

### H3. Footprint gate fails: `viewer.rs` exceeds its budget
- `python tools/check_fork_footprint.py --base 11aa5f088bb52d5f306b0224e37b782b1bf4503d` prints
  `ERROR: exceeds budget: crates/eqoa/src/viewer.rs: +104 -14` / `FAIL: 1 footprint violation(s)` at `3ffaaa4`.
  The manifest allows +101 -11. The three one-line swaps are `minimap_contains` at `viewer.rs:599,5651` and
  `draw_minimap` at `viewer.rs:6391`. The Part A report claims PASS with no budget change; that was true before items
  4-6 and is stale. The brief says to report each budget change.
- Fix: either raise `viewer.rs` by +3/+3 in `docs/fork-footprint.json` with a reviewed reason, or fold the shot-script
  swap (`viewer.rs:599`, a test-only path) into a neutral helper. The seams themselves are fine (see below).

## MEDIUM

### M1. `DisableUI` / `hide("all")` now hides the minimap and its clicks (behaviour change for existing mods)
- `visibility.rs:14-29` appends `"minimap"` to `ALL_UI_ELEMENTS`; `native_leaves("all")` (`visibility.rs:581`) and
  `hud_layout.rs:243` use it. The Forge `DisableUI`/`EnableUI` loops (`visibility.rs:431,445`) and
  `GetHiddenBy` therefore now include it. Scenario: Wereoxxs (or any Forge mod) calls `UI.DisableUI`. Before: stock
  HUD hidden, minimap stays. Now: minimap hidden too, and its click and wheel stop working, with no replacement
  (Forge's own mini_map is unsupported). The bundled Wereoxxs test was edited to expect `minimap: true`
  (`wereoxxs_test.rs:1044`), which bends the existing test to the change instead of guarding the behaviour.
- Fix: keep `minimap` as an explicit-only element (do not add it to `ALL_UI_ELEMENTS`/`"all"`/`DisableUI`), or get the
  owner's decision. `REPORT_CLAIMS` is a `u16` and `ALL_UI_ELEMENTS` is now 15 long, so one more element would
  overflow the bit shift (`visibility.rs:819-823`).

### M2. Tests for the minimap view lifecycle and pointer consumption are weak
- `map_views.rs:410-416` tests only the pure `route()` function. It would still pass if `draw()` were changed to draw
  stock and capture in the same frame, or if the dedupe were removed. Nothing asserts that exactly one `draw_mini`
  runs per frame, that no capture happens with no lease, that the canvas is dropped, or that `info()` returns nil
  after resize/disable. These need either a headless-GPU test (count passes) or an injectable draw callback.
- `pointer_api.rs:222-245` drives `position()` and `claimed()` directly. It never goes through `observe(event)` or
  `Runtime::event`, so "claimed press/wheel is consumed, unclaimed is passed on, release after a claimed press goes to
  the addon, focus loss clears claims, Move HUD takes precedence" is untested. The ownership state machine
  (`capture.press/release`) looks right by reading, but nothing proves it.
- No test for device loss/`for_new_device` freeing the canvas, leases, claims and guides (they are static
  thread-locals; they are freed only through `native_events::clear` via `Host` drop/disable, `host.rs:68`,
  `manager.rs:520,662`). Reading says it works; add a test.

### M3. Pointer coordinates are central-frame, not the layout's `frame_x` space
- `pointer()` and `claim_pointer` plain rectangles are in central 640x448 frame units (`frame_from_window`,
  `pointer_api.rs:21`). `hud.layout().minimap.frame_x/frame_y` are right-/top-anchored (they include `side`/`top`,
  `hud_layout.rs:140-175`). A mod that compares `pointer().x` to `layout.minimap.frame_x` is off by `side` on any
  window wider than 4:3. The spec (§7, line 571) says only "HUD coordinates ... a native layout rect may be passed
  directly", which is true for `claim_pointer` (it detects `frame_x` and treats `x,y` as window pixels) but not for
  comparing against `pointer()`. This is workable but a trap. Either document it with an example, or also return
  `window_x/window_y` in `Pointer`.

### M4. `info()` (the `view.texture` accessor) raises a Lua error on quota failure
- `map_views.rs:139-142`: `register_target(...).map_err(mlua::Error::runtime)?` inside the `__index` metamethod. If
  the VM texture quota (128 handles / 64 MiB / `max_dimension`) is exceeded, every frame's `view.texture` read throws
  instead of returning nil plus a visible diagnostic, and a mod calling it per frame is disabled by the error budget.
  A full-window RGBA canvas is charged to each VM's bank (up to 2048x1152x4 = 9 MiB, per VM holding a view).

### M5. `Projection.inside` is the square inner rect, not the visible circle
- `map_views.rs:32-35`. The stock minimap draws and masks a circle (`mini.circle`). A mark in a corner is reported
  `inside=true` but is not visible (it is masked in the capture and by any circular HUD image crop). The round minimap
  mod's rim logic depends on this. Return the circle radius in the projection and test by distance.

## LOW

- L1. The capture canvas is the whole window (`map_views.rs:342-346`) cleared and re-rendered each frame, while the
  minimap occupies 112/640 of it. Above a 2048 px long side `k<1` downscales the whole window, so a 4K minimap is
  rendered at half resolution (blurry). Cheaper and sharper: render only the `outer` rect into a minimap-sized
  texture. Only costs frames while a mod holds a view, so not blocking.
- L2. There is no explicit release for a view. The lease lives until Lua GC drops the `Rc<Lease>`
  (`map_views.rs:53-65`), so a mod that discards its view keeps the capture running until a GC cycle, and re-creating
  views each frame would hit the 4-per-VM quota. Add `view:close()` or document.
- L3. `guides.rs:122-133`: `dots` of up to 80,000 are allocated per `set_guide` call and each is projected every frame
  on the minimap and the M map with no culling before projection. `point()` throws a Lua runtime error on a missing or
  non-finite coordinate (`guides.rs:18-24`), unlike the other validations that return `nil, err`.
- L4. Point (non-entity) guides survive world loss/zoning (`update` only removes entity guides). They are drawn only
  in their own world, so this is cosmetic, but "claims released on world loss" is not literally true for guides.
- L5. `pointer.pressed` fires for any click anywhere in the window to every subscribed mod. That is by design, but the
  spec should say so. Touch input (`WindowEvent::Touch`) is not observed, so a touchscreen tap on a claimed rect
  reaches the game.
- L6. With the minimap hidden by a mod, `mini_frame` still composes all the minimap UI (and the fork's search button
  hit areas) each frame; only the draw is skipped, and the search button under the hidden minimap remains clickable
  until Part B removes it. `play.rs` `Pick::Minimap` is correctly neutralised by `minimap_contains`.
- L7. `native_state.rs` and `hud_layout.rs` contain unrelated rustfmt reflows (e.g. `SPEC_KEYS`, `parse_anchor_vert`).
  Harmless but noisy diffs in fork files; they make a later review harder.
- L8. `fork_ext/map.rs` adds `use render::wgpu;` at the top for all targets; fine on wasm because `draw_minimap`'s
  wasm branch uses it, but `_format` is only used natively.

## Checked and found good

1. Upstream files changed, `git diff --stat 9e7257a 3ffaaa4` (shared files only): `map/mini.rs` +1 (overlay call),
   `map/mod.rs` +4 (a `#[cfg(not wasm)] #[path] mod extension` declaration and one overlay call), `play.rs` +1/-1
   (`minimap_contains`), `viewer.rs` +3/-3 (two `minimap_contains`, one `draw_minimap`). `map/chrome.rs`, `markers.rs`,
   `search.rs`, `render/src/map.rs`, `map.wgsl` and `render/src/ui.rs` are unchanged in the range. All seams use neutral
   names (`overlay`, `draw_minimap`, `minimap_contains`, `extension`), with no Lua/mod identifiers in the upstream
   files. Each is minimal. The only gate failure is the `viewer.rs` budget (H3). The `extension` module is a private
   access trick via `#[path]`, with the budget at `map/mod.rs` +49/-7 vs +70/-10, fine.
2. Single native draw: `map_views::draw` is a three-way route (`Stock` | `Capture` | `None`), so stock and capture are
   exclusive per frame. With no lease and no hide claim, the cost is one `Cell` read plus a `retain` over an empty
   `Vec` (no allocation). A per-frame dedupe `s.drawn == Some(frame)` guards double capture. Canvas is dropped on
   Stock/None. Device loss goes through `Runtime::for_new_device` -> `shutdown` -> `Host` clear -> `release_vm`.
   M map open gives `route None` (the stock minimap is also off then, as upstream). Indoors, `project`
   uses the same `indoor.scale()` as `mini_frame` (`map_api.rs:99-103`). The capture path is at window size, so render
   scale 2x/4x is not an issue (ImGui draws to the window-sized target after `resolve`, `viewer.rs:3677`;
   `set_window_size` is the real framebuffer size).
3. `hide("minimap")`: suppresses draw and `Pick::Minimap` clicks and the wheel (`minimap_contains` gates both
   `viewer.rs` wheel paths), released through `native_events::clear` on disable/failure/Host drop, and `hide`/`show`
   are tested from Play with a negative control (`visibility.rs` `round4_tests`). The Move HUD rect uses the same
   `shift`/`window_rect_moved` maths as `mini_frame`; test `round4_minimap_layout_tracks_upstream_drag_and_reset`
   moves and resets it.
4. Guides: separate from the player's waypoint (no writes to `waypoints`), one per VM, cleared on `clear_guide` and
   `release_vm`, drawn through the single overlay seam on both the M map and the minimap (inside the masked range, so
   it also shows on views), the line only when the player is in the displayed world. `guide.lost` is correctly
   owner-scoped in `native_events::publish` (but see H2 for when it fires).
5. Pointer: claims need `!hudlayout::unlocked()` (Move HUD wins), expire after one frame, are cleared on focus loss
   and on disable, are capped (32 rects/VM/frame), and the claimed result is OR-ed into the existing `capture`
   ownership machine, so a press that a mod owns also owns its release (no stuck button). Coordinates use the physical
   window size on both sides. `minimap_wheel` is accumulated and applied once on the game step, clamped.
6. Zero-addon allocation: the new per-frame code on the no-addon path (`draw`, `guides::draw`, `observe`) takes no
   allocation. The capture additions in `api_capture.rs` are behind `capture_requested`.

## What I could not check
- No GPU/cargo run: the capture pass, texture registration and `AddImageRounded` circle were not executed. The
  claims above about them are from reading.
- The owner-visible behaviour (round minimap showing, wheel zoom, click-through) has not been seen.

## Process note
While reading I mistakenly ran `git checkout --detach` in the worktree. It only detached HEAD (same commit,
`3ffaaa4`, no file changes, other agent's uncommitted edits untouched). I restored it with
`git switch feat/eqoa-api-r4` straight away, and `git status` now shows the branch with the same modified files as
before.
