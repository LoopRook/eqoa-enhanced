# Round 7 Part E report: mod keys against game keys

Branch `feat/eqoa-api-r7e`. Fork-owned files only; zero upstream-file changes (footprint below).

## What changed

- `crates/eqoa/src/fork_ext/bindpage.rs`: conflict rules, press-time winner, capture handling for both
  directions, marks and explanations on Controls, defaults that never take a key in use.
- `crates/eqoa/src/fork_ext/addons/native_state.rs`: the bind callback also asks `bindpage::fires(...)`
  (one call) before it runs the mod's function.
- Tests: `round7e_key_tests.rs` (the previous builder's commit d6cb5730, kept and extended) plus test-data
  fixes in existing tests (below). Docs: `FORK-UPDATE-NOTES.md` ("Unreleased"), `EQOA-API-SPEC.md` (`eqoa.input.bind` row).
- No `eqoa/1` surface change, so no `docs(modding): regenerate` commit.

## Behaviour (player terms)

1. Capture (the existing hooks `capture_key`/`draw`/`help_or_defaults` run for game rows and mod rows):
   - mod row given a key a game control has: the mod takes it; the game control takes the key the mod had (swap),
     or has no key if the mod had none. Note: `alpha: Space. Jump: B.` / `alpha: Space. Jump has no key now.`
   - game row given a key a mod has: the game row takes it; the mod takes the row's old key, or has none.
     Note: `Jump: Space. alpha: B.` / `... alpha has no key now.`
   - two mods: same swap/free between the two rows.
   - Same wording pattern as upstream's game-to-game line ("X: key. Y: key." / "Y has no key now."); built in
     fork code, upstream's formatter untouched. Applied at once, no question (upstream has none).
   - A key in a macro's trigger is refused for a mod ("... is in a trigger of Macro 1: choose another key, or
     change that trigger on the Macros page."), as the Macros page refuses a row's key.
2. A mod's default key never takes a key in use (game control, macro trigger, or a mod already on it): its row
   starts empty and the help says whose key it is. Defaults button: same rule.
3. Existing conflicts (old saved settings, hand-edited files) are marked `!` on both rows; nothing is rewritten.
   **Precedence at press time (decision for the lead):** game control > macro trigger > mod. Between mods, a
   saved key beats a default; then lowest addon id, then binding id (so it does not depend on load order). The
   mod row says why ("B is also Jump's, and the game's control wins: ...").
4. One press does one thing: `bindpage::fires` makes the loser's callback not run. Chat typing and menus
   still suppress mod keys as before (`mod_binding_allowed`, `route_mod_key`).
5. Held after restart (saves go to `mod-keys.opt` at capture; game keys in `keys.opt` with Save Changes, as every
   Controls change), after disable/enable (disabled binds neither fire nor count), from either section.

## Mods whose default key starts empty (player-visible)

None change on the default layout: bundled defaults are Stats Box `b`, Map Search `f2` and `f4`, Bag Viewer
none; the game's default layout uses none of them. Forge-imported mods (Wereoxxs, ARPG UI, Modern UI) register
no Controls keys. A mod's default only starts empty for a player whose saved game layout already puts a
control or macro on that key. Saved mod keys are never changed. Visible consequence for old saves: a mod key
already shared with a game control stops firing for the mod (the game control wins) until re-chosen.

Controller buttons: mod keys are keys only (the mod column's pad cell is "-"), so there is nothing to handle.

## Judgement calls to confirm

- Game side of a swap is kept only by Save Changes (like every game control); the mod side saves at capture.
  If the player leaves Controls without saving, the old game key returns next start and the pair is an old
  conflict, resolved by the precedence above.
- Existing tests that bound mod keys to game keys (`k` Camera down, `h` Hide UI, `p`, `m`, `z`, `f8`) now bind
  keys the game leaves free (`u`, `y`, `7`, `f10`); a test with two binds on one default key (`h`) now uses two
  keys. Test data only: `action_broker.rs`, `bag_viewer_tests.rs`, `native_tests.rs`, `round_3_picture_tests.rs`,
  `bindpage.rs` tests. (Rule 3/4 made those fixtures conflicts: 69 addons tests failed on the first run for
  exactly this reason, 0 after.)

## Previous builder's commit d6cb5730 (unverified, then reviewed)

Kept all 8 tests (they drive the real `ControlsPage` capture/draw and `KeyEdge`, `route_mod_key` +
`native_events::dispatch`, `settings::save`/`testing::put`; none tests a copy of the logic). Changed: the
capture helper draws the page first (as the client does), a `page_text` helper reads the help line, and the
file imports `Input`. Added 8: both notes, freed notes, restart after a mod takes a game key, macro-trigger
refusal + press, Defaults button, "says who wins", chat text capture.

## Tests and results

All run through `tools/run_gates.py`, with `EQOA_ISO` set (disc present).

- `python tools/run_gates.py --only round,addons --filter round7e_ fork_ext::bindpage fork_ext::settings`:
  round 23 passed, 0 failed, 0 ignored; addons 585 passed, 1 failed, 25 ignored. The one failure
  (`round_3_stage_a_options_toggle_and_get`, a key leaked by another test) was fixed afterwards (`7` for that
  mod); that test was not re-run locally alone after the fix, see CI.
- SKIP lines in the addons run: 11, all unrelated (no graphics adapter, symlink privilege, Wereoxxs recording,
  UI goldens). Round 7e tests: 0 skips.
- Full set: GitHub CI (below). Oracle tests skip here as always (no decomp on this machine).

### Negative controls (executed)

A temporary env switch `R7E_BREAK=<n>` was compiled into the code, the test binary run once per switch
(`<exe> round7e_ --test-threads=1`, n listed), and the code restored with `git checkout -- crates` (status clean,
no `brk` left). `hits = N` is the Lua assert in `round7e_key_tests.rs:34`.

| n | break | tests that failed (first failing line) |
|---|---|---|
| 0 | none | 16 passed |
| 1 | press ignores game/macro owner | `a_macro_trigger_key...` (`hits = 1`), `legacy_conflicts_have_one_winner...` (`hits = 1`) |
| 2 | loser wins between mods | `disabled_binding...`, `legacy_conflicts...` (`hits = 1`) |
| 3 | default takes a key in use | `default_does_not_take...` :141, `defaults_button...` :341 |
| 4 | game row capture ignores mods | `game_takes_mod_key...` :102, both note tests :256/:279 |
| 5 | mod capture ignores game/mods | 6 tests incl. `mod_takes_game_key_on_controls` :84, `mods_swap_keys` :126, restart :293 |
| 6 | no `!` marks | `controls_marks_both_sides...` :184 |
| 7 | disabled mod still counts | `disabled_binding...` (`hits = 1`) |
| 8 | macro key not refused | `a_macro_trigger_key...` :329 |
| 9 | Defaults button restores blocked default | `defaults_button...` :341 |
| 10 | press-time hook not consulted | macro, disabled, legacy tests (`hits = 1`) |
| 12 | game capture never gives mod the old key | `game_takes_mod_key...` :102, notes :256 |
| 13 | mod capture never gives the game row the old key | 4 tests: :84, :126, :248, :300 |
| 14 | chat text capture not honoured | `typing_in_chat...` (`hits = 1`) |
| 15 | menu/plain-play gate removed | `chat_and_menus...` (`hits = 1`) |
| 16 | capture not saved | `game_takes_mod_key...` :110, restart :293 |

## Footprint, wasm, CI

See the final lines below (filled in after the runs).
