# Round 7 Part A audit fixes

Branch: `feat/eqoa-api-r7a`, base `9ea6b435`. Work remains in progress.

## Reproduction

First run: `python tools/run_gates.py --only round --filter round7a_ --timeout 60`:
1 passed, 11 failed, 0 ignored, 0 SKIP lines. The passing chat test confirms the
native hook has no emulator suspension mechanism. All eleven failures are assertions
or Lua calls exposing the audited missing/refusing behavior, not compilation errors.
The follow-up baseline includes the two GPU/icon tests and all twelve bundled mods.

## Boundaries

No upstream changes, seed-table changes, round-6.5-owned implementation changes,
bundled asset changes, or shared-budget increases. The release notes and author docs
are explicitly reserved for the lead/Part D. No push, merge, tag or rebase.

## Reference

Pinned read-only reference: `C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/modules/frontiers_forge/`.
Camera: `camera.lua:6-15` returns an x/y/z table and defines no missing-camera case.
Chat: `chat.lua:567-569` defines suspended as wanted but detached outside the game;
the native adapter owns a consumer lease, never an emulator patch/cave.
Quest: `quest.lua:21-36` exposes `new` and `GetName` over raw pointers.
Cave: `cave.lua` exposes IsRegionZero, Alloc, Reserve, Get, Free, Release and Info.

## Validation still pending

All fixes, executed mutation controls, bundled before/after diagnostics, full gates,
release build, footprint and final counts.

Follow-up baseline: 3 passed, 12 failed, 0 ignored, 0 SKIP lines.
The production addon icon test and all twelve bundled mods passed; each bundled
mod produced zero manager diagnostics. The source-format descriptor failed as expected.
