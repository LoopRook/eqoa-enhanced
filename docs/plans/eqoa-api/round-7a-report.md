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

## Step 2: quiet placeholders

The base icon bindings now diagnose `unavailable` through the native sink and keep
Forge's three-nil texture failure shape. The one-off stderr warning is removed.
`hud.hidden_by` refuses without its visibility lease. The remaining install placeholders
were reviewed: player.get/hud.hide already refuse not_in_world; client.get describes a
true pre-world state; input.get's invented state is replaced by an unavailable refusal
(the production input installer replaces it later in the same install).

## Step 3: partial answers

`icons.rs` describes source_format unsupported: SafeLibrary/IconProvider retain only
width, height and decoded RGBA bytes, so the original disc encoding is unavailable.
Accessing the unsupported record field uses the existing guarded-record diagnostic.
`host.rs` diagnoses absent camera state while retaining the pinned table shape.
`chat.rs` records why false is real: the native consumer lease never detaches an
emulator code cave. Its test passed before changes; no behavior fix was applied.

## Step 4: guarded APIs

`package.rs` registers quest/cave diagnostic tables, including every pinned public
function and guarded unknown members. The existing reserved-module test now expects
quest require to succeed and new(0) to refuse; this is the one changed old assertion.
`host.rs` exposes GetWorldId from the native world state and registers util via the
package registry; `bit.rs` guards unknown keys and describes all eight functions.
`fonts.game` refuses: the existing ImGui TrueType atlas service cannot expose the
native bitmap font without an upstream change (excluded by this brief).
Per-member unsupported descriptors are closed in step 7's required hand list.
