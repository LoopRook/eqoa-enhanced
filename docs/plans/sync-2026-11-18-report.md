# Upstream sync for November 18, 2026

The approved sync brought 133 commits from upstream through `35e2a43b` into
`sync/2026-11-18`, starting at develop `00ee496b`. Merge `90d7b143` retains
both parents. The sync branch includes input follow-ups `f18386df` and
`59ed299a`, a quest fixture update in `e39dcfee` and the settings fixture
updates in `87eccb81`. Develop advanced separately during validation. This report covers the merge from the pinned
starting commit.

## Conflict resolutions

Git kept the fork README and root CLAUDE.md through the existing merge rules.
The sync script merged the one-line `Play` initializer field by field. It
kept the fork's Forge health state and upstream's regional weather, carried
actors and pets, vehicle state, dye picker, and Hardcore fall state. Eight
hunks in six files required manual resolution.

- `Cargo.lock` ? Enhanced added `mlua` and upstream added `memmap2`. Kept both dependencies for addon scripting and the new texture cache.
- `crates/eqoa/Cargo.toml` ? Retained Enhanced's Lua manifest entries and upstream's `Win32_UI_WindowsAndMessaging` feature so Windows can release the clipped pointer on focus loss.
  Opus review follow-up restored the direct `Win32_System_Com` feature for `CoTaskMemFree` alongside `Win32_UI_WindowsAndMessaging`. The footprint passed at +7/-1 and the optimized Windows gate passed using the cached build.
- `crates/eqoa/src/main.rs` ? Kept Enhanced's `fork_ext` module and upstream's `texcache` module so both compile into the client.
- `crates/eqoa/src/options.rs` ? Extended Enhanced's category lookup to upstream's two server restriction notices so they apply to the active page, including Enhanced.
- `crates/eqoa/src/play.rs` ? Kept upstream's independent Stacked Spell Bars position for drawing and both click paths, retained the fork's hide and click guards, and accepted upstream's Hardcore fall formatter and test.
- `crates/eqoa/src/settings.rs` ? Kept both upstream's server restriction reset and Enhanced's client fixture reset so test sessions start with clean settings.

The merge also restored upstream's hidden Quick Menus regression alongside the
fork's compass regression, which moved to `fork_ext/play.rs`. Independent
review verified the six resolutions and measured options budget. It found a
duplicate chat-selection release on cursor exit and a stale addon key owner
when a mercenary window opened between press and release. The follow-ups keep
one cursor-exit cleanup, give the new mercenary and dye windows their native
keyboard input, and still deliver releases to addon bookkeeping. The native
[OG] pointer gate also covers addon and local report mouse input. The live
handoff between addon and native windows remains on the owner checklist.

## Footprint and upstream instructions

- `options.rs` increased from +17/-10 to the measured +20/-12. The existing
  runtime category lookup now serves the normal note and the two server
  restriction checks, keeping the Enhanced-only page clear of an out-of-range
  index into the stock categories. All other limits held.
- The script refreshed `docs/upstream/CLAUDE.md` and advanced
  `reviewed_base` from `a76d8d2` to `35e2a43b` after the footprint check passed.
- Keep these upstream rules when extending the relevant features. The texture
  cache belongs in the platform cache, with isolated test directories and
  generated mip levels matching the uncached path. The server judges
  mercenary inventory and spell moves. Keep the server's ability-quest flags
  on both quest marker and tracker data. For headless character-select QA,
  `EQOA_PREGAME_MOUSE` accepts timed press, move, release and wheel input.
  These rules are specific to their respective systems and are recorded in the
  upstream copy.

## Round 6.5 integration

Compared against `feat/eqoa-api-r6.5` at `3cf74833`. The upstream range left
`npcw.rs`, `bankplay.rs`, `merchant.rs`, `input.rs` and `hud.rs` unchanged.
The four Round 6.5 hook locations still match the native loot, merchant and
bank handlers. The existing `Play` event hooks still bracket dispatch and
can observe the mode before and after each packet.

- Mercenary inventory and spell book now use modes 61 and 62. The dye picker
  uses 63. Round 6.5 takes over loot, merchant and bank in modes 1, 10 and 11.
  In the combined build, open a mercenary or dye window with Bag Viewer
  active and verify arrows, Enter and Escape go to the window. Hold and
  release a key across the transition, then verify it works in the addon
  again. Check that an incoming mercenary reply does not disrupt a pending
  loot, sale or bank action after the takeover returns Play to mode 0.
- Alt-Tab now clears the Windows pointer clip. [OG] account restrictions and
  the two-button run change the viewer's mouse handling. Recheck focus
  loss and return, pointer actions, and manual versus addon-opened windows
  after integrating 6.5. Use both an unrestricted account and an [OG]
  account on a server offering those modes.
- Move HUD treats stacked spell bars as their own piece. Check a moved
  bar's clicks and right-click autocast with the visibility mod on and off.
  Keep the Enhanced page reachable on servers that restrict native HUD
  rows. No Round 6.5 code was merged into this sync.

## What changed for players

- On a server with mercenaries, use the arrows, Enter or E, and Escape in
  their inventory. Open the spell window and use Page Up and Page Down to
  read a spell. Try the new dye picker at a dyer or Pet Groomer.
- On a server that supports Hardcore, create a disposable test character.
  Check the fall notice and memorial on the character list.
- Compare a normal account and an [OG] account. Verify keyboard and
  controller input still work for [OG], and check the locked map and item
  link rows. Before entering the world, verify mouse and touch work.
- Enable a large texture pack, cross into a different zone, then restart
  and compare how quickly pack textures appear. In Move HUD, move stacked
  spell bars independently of chat and check that both click paths follow.

## Automated validation

The first Rust compile exposed a fork fixture without the new ability-quest
field. The next run reused incompatible shared Cargo artifacts from another
worktree. Refreshing this worktree's crate entry-point timestamps under the
shared Cargo lock rebuilt the correct libraries without a separate cache.
The first completed client suite reported 2,574 passes, two failures,
34 ignored tests and 51 SKIP messages. Both failures were fixture
expectations for NPC/PC display. The upstream protocol fixture now uses the
current 200 distance default. The Enhanced migration test still exercises
the first-run 64/650 upgrade and now expects the upstream 48/200 stock
defaults once the migration marker exists. Both focused tests passed.

The final `--continue --tests-only` run passed with 2,723 Rust tests,
zero failures, 34 ignored tests and 53 SKIP messages. The Python tools
passed all 157 tests. The run used the linked Frontiers disc and scratch
configuration. Missing optional references included the 2002 disc,
extracted modules and live saves, Forge and visual golden fixtures, and
Windows symlink privileges.

All seven release gates passed.

| Gate | Result | SKIP messages |
| --- | --- | ---: |
| Addons | 514 passed, 25 ignored | 9 |
| Full client | 2,529 passed, 31 ignored, 50 filtered by the gate | 51 |
| WebAssembly | Locked client check passed | |
| Footprint | Passed at `35e2a43b` for 42 shared files | |
| Tracked paths | Passed | |
| Player release | 20 tests passed | |
| Optimized Windows client | Locked release build passed | |

Addon SKIP counts follow the gate's test parser. The raw addon log has 12
lines containing the word SKIP. The documented renderer AO and particles16
fixtures belong to separate renderer tests outside these commands.

Detailed logs are under the sync worktree's `target/` and the shared Cargo
target's `gates/`. The owner play test and the combined Round 6.5 test are
pending.

## Develop merge (develop `91a66d7d`)

Develop brought Round 6.5 window takeover, Round 7 A/B, the session
recorder, scripted input, CI gates, Wereoxxs performance and the gate
runner's stale-binary guard. Three files needed hand resolution.

- `crates/net/src/lib.rs`: kept develop's `fork_record` module and upstream's `hardcore` module, in alphabetical order.
- `crates/eqoa/src/fork_ext/report_viewer.rs`: kept the sync's input order (an [OG] pointer gate first, then the report, then addons), its `addon_event` and upstream's mercenary and dye-picker keyboard ownership; kept develop's `report_key` and the per-frame scripted-input step. The ownership rule is now `native_owns_keys`, and scripted keys follow it in `scripted_input.rs`: when a native window owns the keyboard the key reaches the game, and addons still receive its release.
- `crates/eqoa/src/play.rs` (a compile conflict Git did not flag): upstream added `impl Drop for Play` to restore server-restricted options when Play ends. Develop's session recorder had its own `Drop` in `fork_ext/play.rs`. Upstream's `Drop` keeps its body and calls the new `fork_hooks::dropped(self)`, which drains the recorder. The new hook is recorded as `session recorder drop` in `fork-footprint.json`. The play.rs diff measures +80/-40, within its unchanged budget.
- `FORK-UPDATE-NOTES.md`: kept the sync entry and develop's Bag Viewer, Wereoxxs and recording entries.

### Test stack and gates after the develop merge

CI run 44582748614 aborted the addons and full gates. Test
`round65_fix4_item1_bags_window_costs_under_a_millisecond_a_frame`
overflowed the 2 MiB default test-thread stack (STATUS_STACK_OVERFLOW). The
GPU addon test harnesses hold `Play` (about 110 KB), `Manager` and other
state by value: `Bv` is about 170 KB and `Rm` about 190 KB. A debug build
copies them while it builds one, so every test using them runs close to the
limit. With a 1 MiB test stack, 51 addon tests overflowed. They now run
their bodies through the test-only `fork_ext::addons::big_stack`, a 16 MiB
thread. At 1 MiB, 568 addon tests then passed with no overflow. No game
code changed.

- CI on `8f8ef6ac`: every gate passed. Addons: 568 passed, 25 ignored,
  16 SKIP. Full: 3,159 passed, 37 ignored, 377 SKIP. Tools: 199 passed. The
  tools job's `unittest: error` lines come from argparse rejection tests in
  `tools/test_ci_gates.py`, not from failures.
- Local `run_gates.py --release` on `8f8ef6ac`: full, wasm, footprint,
  paths, player release and the optimized Windows build passed (full: 2,599
  passed, 31 ignored). The addons gate reported 566 passed and two failed
  timing thresholds, under heavier load (16 ms a frame against 8 ms in the
  same run's full gate). One run had 3.42 creature-list calls a second
  against a limit of 3.0. The other measured 5.57 ms against a 5 ms
  debug-build limit. Both tests passed in the full gate and in CI. A
  rerun of the addons gate alone passed all 568 tests, with 25 ignored
  and 10 SKIP.

