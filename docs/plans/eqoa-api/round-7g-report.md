# Round 7 Part G item 2: the `eqoa/1` freeze guard

Not marked as frozen anywhere player-facing; the owner approves the freeze with the release.

## Built
- `docs/modding/eqoa-1-frozen.json`: 1637 `supported` paths, each with a kind (`leaf`, or `group` when the
  path has children). Separate from the regenerated `eqoa-1-surface.json`. Created from the current surface.
  (The surface rows carry no kind field, so "kind" is leaf/group; a leaf that gains children or the reverse fails.)
- `modtools.rs` tests: `eqoa1_frozen_surface_only_grows` (live surface from `reference()`, the same generator and
  `with_graphics` SKIP handling as `round7b_support_table_matches_the_running_client`, windows-only like it) and
  `eqoa1_frozen_surface_controls` (runs everywhere, no GPU: removal, downgrade, kind change fail; a new supported
  name passes). The failure names each path with before -> after status and states the policy (VERSIONING.md is
  not on this branch, so it says "the versioning policy").
- `tools/eqoa1_freeze.py update [--allow-downgrade]` (+ `tools/test_eqoa1_freeze.py`, 3 tests): only adds; refuses
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
