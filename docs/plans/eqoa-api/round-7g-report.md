# Round 7 Part G item 2: the `eqoa/1` freeze guard

Not marked as frozen anywhere player-facing; the owner approves the freeze with the release.

## Built
- `docs/modding/eqoa-1-frozen.json`: 1637 `supported` paths, names only. Separate from the regenerated `eqoa-1-surface.json`. Created from the current surface.
  Review change: the invented leaf/group "kind" rule was removed (it blocked legitimate additions); a group that
  loses children is caught because each lost child is itself a removal.
- `modtools.rs` tests: `eqoa1_frozen_surface_only_grows` (live surface from `reference()`, the same generator and
  `with_graphics` SKIP handling as `round7b_support_table_matches_the_running_client`, windows-only like it) and
  `eqoa1_frozen_surface_controls` (runs everywhere, no GPU: removal and downgrade fail; a new supported name passes; a frozen leaf gaining a supported child passes). The failure names each path with before -> after status and states the policy (VERSIONING.md is
  not on this branch, so it says "the versioning policy").
- `tools/eqoa1_freeze.py update [--allow-downgrade]` (+ `tools/test_eqoa1_freeze.py`, 3 tests): only adds, leaves the file untouched on a no-op (tested: bytes and mtime); refuses
  and writes nothing if the surface removes/lowers/changes a recorded path unless `--allow-downgrade`.
  Documented in `docs/DEV-TOOLS.md`. Added both files to `docs/fork-footprint.json` fork-owned list.
- `CLAUDE.md`: one rule line.

## How to extend
Regenerate `eqoa-1-surface.json` (`eqoa mod-api --format json --out ...`), then `python tools/eqoa1_freeze.py update`.

## Negative controls (executed on the live surface; each restored, `git status` clean after)
Command: `python tools/run_gates.py --only round --filter eqoa1_frozen`
1. `bind` skips `eqoa.actions.status` (early return in `native.rs::bind`): `eqoa.actions.status: supported -> unsupported`.
   (An earlier try, un-setting `eqoa.ChatHistory` in `chat_history.rs`, did NOT fail: that name's status is seeded
   elsewhere. Recorded for honesty; it is not a control.)
2. A supported field set `unsupported` (`eqoa.Vec2.x` in `describe`): `eqoa.Vec2.x: supported -> unsupported`.
3. A supported event removed (`"timer"` out of `EVENTS`): `eqoa.events.timer: supported -> removed`.
4. Positive: new supported name passes (in `eqoa1_frozen_surface_controls`, green).

## Counts
Focused `--filter eqoa1_frozen round7b_` with a GPU on this PC: 7 passed, 0 failed; 1 SKIP line, the by-design
`round7b_without_adapter_is_an_explicit_skip`. Drift and frozen tests ran for real (no disc needed). wasm check,
footprint, paths PASS; tools tests `test_eqoa1_freeze` 3 OK. Not run: full test set (left to CI).
Footprint: no upstream file touched; no budget change.

## CI
CI run 44872812881 (pushed fbaac72a9f, before the report commit): completed, success.

## What the guard does NOT cover
Names and their `supported` status only. Not covered: a supported function whose body starts refusing while its
registration stays; a record field no longer projected; an event no longer emitted; a changed argument or return
shape; names whose status comes from presence rather than a registration (Forge and UiForge layers,
`ImGui.*`/`eqoa.imgui.*`, `bit.*`, the hand check in `annotate_host`, `native.rs` ~365-412; round 7 H handles it).
The freeze rests on: this guard (names); the round 7C tests (`round7c_play_records_match_declared_fields`,
`round7c_supported_event_inventory_has_fixture_coverage`, `round7c_host_service_events_reach_lua_from_their_producers`,
`round7c_world_events_come_from_play_changes`, `round7c_item_records_and_window_events_use_projected_native_state`)
for fields and events; the spec and reviews for shapes and meanings.
After round 7 H merges, recreate the record with one command: `python tools/eqoa1_freeze.py update` (delete the
record first if the surface legitimately lost names, with the owner's approval).

## Review round (97a7a01a): changes applied
Kind rule removed; coverage limits stated; no-op leaves the file alone; `CLAUDE.md` line moved next to the
API-contract paragraph and reworded to "checks names and status". Controls rerun below.

Control rerun after the review change (record now names only): removing `timer` from `EVENTS` fails with `eqoa.events.timer: supported -> removed`; restored, clean. Focused rerun 7 passed, 0 failed, 1 by-design SKIP line.
