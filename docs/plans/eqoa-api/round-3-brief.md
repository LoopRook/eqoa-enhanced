# Round 3 brief: the HUD extras become mods

Lead: Opus (2026-11-10). Plan: `docs/plans/EQOA-MOD-API-PLAN.md`, section "Round 3". Spec:
`docs/modding/EQOA-API-SPEC.md`. Branch `feat/eqoa-api-r3` from `develop` (after the 2026-11-10 sync).

## Goal (owner-approved)

Stats Box, Target of Target, Mana Bars and HUD numbers stop being code in upstream's files and become
four bundled Lua mods, **on by default, looking and working exactly as they do today**. None of the
four exists in upstream (checked: upstream removed Stats Box, others' mana bars and ToT in 6fc3ec6;
HUD numbers were always fork-only). Upstream's own features stay upstream's and are not wrapped into
mods: the Assist Target action and its F key, the raid window's power bars, Number Colors, Pet
Effects, the DPS meter.

## What the player must see (identical to today; see the inventory at the end)

- Stats Box: off until turned on (its option row or the B key); same box, place, two columns, colours
  (effect-raised blue, lowered red); click opens the Character Summary.
- Target of Target: off until turned on; same bar just above the target's, name, health and power;
  click = Assist Target (upstream's action).
- Mana Bars: on; power gauge under each group member's health, under the target's, and on ToT.
- HUD numbers: on; health %, power cur/max, XP details, target HP % (green/yellow/red), each with its
  own switch.
- The same option rows (labels, defaults) on the fork's "Enhanced HUD" options page, and the B key on
  the Controls page, rebindable. A player's saved choices carry over once (old keys `stats_box`,
  `target_of_target`, `mana_bars`, `hud_numbers`, `xp_details`, `target_hp_percent`, `key.stats_box`).
- Disabling a mod in the Addon Manager removes its drawing and its option rows; nothing else changes.

## Stages (commit after each step; report after each stage in the report file)

**A. API additions** (fork-owned files under `fork_ext/addons/`; each value tested from Play- or
protocol-shaped data with a negative control; update the spec and `eqoa.capabilities`):
1. `eqoa.player.stats()`: each stat's base and current value and the effect change, armour and resists
   as the Summary/Stats Box shows them, Class Mastery bonuses when received. Move the computation out of
   `statbox.rs` into a fork-owned module the API and (until stage C) the native box both use, so there is
   one source.
2. `power` (0..1 fraction, nil when the server hasn't sent it) on group members and entities;
   `target_id` on entities. Others' power comes from the existing 0x7E01/0x7E02 reader (keep it native).
3. `eqoa.actions.assist()` through upstream's own `assist_target` path, and
   `eqoa.windows.open("summary")` through the same path as the Stats Box click today. Player-style
   only: one request per call, same rules as the key/click.
4. `eqoa.input.bind{id, label, default_key}`: a mod-owned key shown and rebindable on the Controls
   page, saved in the binds file under a mod namespace, firing an event/callback. Verify first whether
   today's B key actually honours a rebind (the scout saw it hardcoded in viewer.rs).
5. `eqoa.options.toggle{id, label, default}` (or the closest fit to the spec's settings model): a mod's
   On/Off row shown on the fork's Enhanced HUD page, value kept in the mod's settings. Rows disappear
   when the mod is disabled.
6. HUD placement so mods can match the native look: the HUD frame and its transform to output pixels
   (`eqoa.hud.frame()`), the rects of stock parts mods attach to (`eqoa.hud.layout()`: player bars,
   each group member's health bar, target bar, pet strip), and visibility (HUD hidden, menus, loading,
   a stock part hidden by the player or another mod's claim). Keep ImGui as the only drawing API; use
   `eqoa.icons.ui` for the game's gauge/frame art. If the HUD's font cannot be matched through ImGui,
   add `eqoa.fonts.game()` exposing it. Ask the lead (stop and report) before adding any other drawing
   helper. Mod drawing must not cover native windows, dialogs or menus that the native HUD sits under.

**B. Four mods** in `bundled-mods/eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`,
`eqoa_hud_numbers`, written against `eqoa` only (no Forge calls), enabled by default, registered in
`bundled-mods/SOURCE.txt` and the packaging allowlist (`tools/build_player_release.py`). Each follows
the visibility of the stock part it attaches to (if Wereoxxs or another mod hides the group panel,
the member mana bars hide too). **Proof of "identical":** before stage C, a picture test per mod
compares the mod's drawing with the native drawing of the same game-shaped state (save the native
capture as the test's reference image) at two window sizes. Measure the per-frame cost of the four
mods together in an optimized build and report it (aim: well under 0.5 ms/frame; event-driven where
the data allows).

**C. Removal and migration.** Remove from upstream's files (hud.rs, play.rs, settings.rs, binds.rs,
viewer.rs, options rows, net only where it is drawing-related): the Stats Box, ToT and Mana Bars
drawing, their HudState/Play fields, option rows, settings and key rows, the B key, the fork number
labels' hooks. Keep the 0x7E01/0x7E02 power reader. A one-time migration in fork-owned code moves the
old saved values into each mod's settings (and the key into the mod's bind); show it with tests
(old file in, mod settings out, second run does nothing, explicit choices kept). `statbox.rs`'s
drawing goes; its computation lives on in the fork-owned module from A1. Footprint must go DOWN:
report fork lines in upstream files before/after and lower each budget in docs/fork-footprint.json to
its measured value. Dry-run conflicts vs origin/master must stay 0.

## Rules

- Work only in your own worktree and branch; commit there; never push, merge or tag. Managed
  worktrees need `git -c safe.directory=*`.
- New code in fork-owned files; upstream files get hook calls only, and in this round mostly lose
  lines. `python tools/check_fork_footprint.py --base <reviewed_base from docs/fork-footprint.json>`
  must PASS; report any budget change with its reason. Never reformat or reorder upstream's lines;
  never delete or weaken an upstream test (diff your branch's tests against the upstream sha).
- Game-logic floats through `ee::*`; game counters step on `frames.rs`'s 30 Hz clock.
- Builds: ONE shared target `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/shared` (never a
  per-worktree target). Take the lock `mkdir C:/Users/hocka/Paseo/cargo-build.lock` before every cargo
  command (wait/retry if it exists), give every cargo command a timeout, remove the lock after.
- Tests: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1` with a scratch
  `EQOA_CONFIG`; the zero-addon allocation gate (CI); `cargo check --locked -p eqoa --target
  wasm32-unknown-unknown`; CI's tools tests, notes test and packaging check (see
  .github/workflows/enhanced-dev.yml); the full client tests at the end; a release build after the final
  commit. Behaviour tests with a negative control. Count SKIP lines. Known: two upstream GPU (AO) tests
  fail on this machine on untouched upstream; report, don't fix.
- Bundled Wereoxxs, ARPG and Modern UI must still load and pass their tests, and must not double-draw
  with the new mods (e.g. a UI that replaces the group panel).
- Player notes: add a short entry to FORK-UPDATE-NOTES.md (`## Unreleased`).
- Report: `docs/plans/eqoa-api/round-3-report.md`: per stage what changed, tests (pass/fail/skip
  counts), per-frame cost, footprint before/after, anything not done or different from this brief.

## Inventory of today's features (scout, develop abd47db; line numbers moved in the sync)

- Stats Box: `statbox.rs` (frame [200,54,348,186], 2 columns x 8 rows Str..AC / HPMax, POW, FR..DR;
  labels gold, values off-white, raised [0.45,0.78,1.0], lowered [1,0.45,0.4]); hud.rs hook and click
  rect; play.rs Pick::Stats -> Summary mode 12; viewer.rs B key; binds row; option row
  `fork_ext/options.rs`; key `stats_box=on` only when on. Data: net::dump::Stats, worn items, mastery,
  p44 cap, trainer.rs resists. Tests in statbox.rs and fork_ext/settings.rs, fork_ext/binds.rs.
- ToT: hud.rs fields/draw (174 wide at [20,300], 26 high, above Target [20,348]); play.rs assist on
  click, source filter, entity `target`; option row; `target_of_target=on` only when on. Tests: hud.rs
  `the_target_of_target_bar_sits_above_the_targets`, play.rs `the_target_of_target_is_the_targets_channels`.
  Note from the sync: with four group members and a pet, upstream's new Pet Effects strip reaches y=285/305
  and meets ToT at y=300; keep today's placement, report the overlap.
- Mana Bars: hud.rs member/target gauges (art 9 under health, frame 136, fill p/255, dim 0.3 unless
  target); play.rs `bar_power`; net session.rs OP_POWER_BARS 0x7E01/0x7E02 (`power_of`,
  `agree_power_bars`); `mana_bars=off` only when off. Tests: hud.rs `mana_bars_sit_under_the_health_they_go_with`,
  session.rs `power_bars_offer_answer_and_set`.
- HUD numbers: `fork_ext/hud.rs` readouts (HP "NN%" white scale .6 at [99,35]; power "cur/max" blue
  [0.4,0.7,1.0] at [99,45]; XP "exp/need (x.x%)" gold [1,.85,.35] at [99,55]; target "NN%" at [60,397]
  scale .65, green >50%, yellow >20%, else red, 1px black outline); three option rows default On.
