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
  (`round_3_stage_a_options_toggle_and_get`, a key leaked by another test) was fixed (`7` for that mod) and
  re-run: `--filter round7e_ round_3_stage_a fork_ext::bindpage` 30 passed, 0 failed.
- CI's first full run (run 44670590646) found 25 more fixtures that used game keys (`k`, `f8`) in
  `api_capture_tests.rs`, `items_tests.rs`, `r6d_tests.rs`, `fork_ext/settings.rs`; fixed, then locally
  `--filter play::fork_hooks:: settings::fork_settings round7e_`: 190 passed, 0 failed.
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

- `python tools/run_gates.py --only wasm,footprint,paths`: wasm PASS, footprint PASS (no upstream file touched,
  no budget change), paths PASS.
- CI run https://github.com/LoopRook/eqoa-enhanced/actions/runs/44674074606 on 77eb403787: success. addons 586
  passed / 0 failed / 25 ignored (17 SKIP); full 3177 passed / 0 failed / 37 ignored (378 SKIP, CI has no disc
  or GPU); wasm, footprint, paths, player-release (20), tools (199) all success. `ci_gates.py cleanup` run.

## Not done / not verified

- Not played in the live client: the `!` mark position (x=20 left of the label) and the note wording are
  tested through the real page drawing with a test font, not seen on screen.
- Macro-trigger keys: handled by refusal and press-time precedence; no UI flow for resolving an old
  macro/mod conflict beyond choosing another key.
- Several bindings of one mod on the same key (older saves): one wins by binding id; the rest are marked.


# Fix round 1 (review of aa4fe9ba: REJECT, findings H1-H4, M5-M8, L9-L11)

Commits, in order: `42f07789` tests first, `c321a564` the fix, `91712e54` + `a6c1c7d0` fixture and
modifier tests, `bcd5c12b` priority simplification and one more test, `bf41426d` the `combo` field and spec
text, `270be2fb` generated docs (hand-edited, see below), then the player note. Then the H3 round (lead-approved):
tests with the two hook lines and stubs, then the implementation. Upstream-file changes in the whole part: exactly
two added lines in `options.rs` (H3 below), budget raised by 2, footprint PASS. **The tests were written and run against the unfixed code first: all 12 failed, each for
the intended reason** (`hits = 1` where a mod fired twice or on the wrong press, `f11` kept as a mod key, a
capture left waiting, a default taken in one load order).

## What changed

- A mod key is matched by the key with the modifiers held written before it (`binds::combo_name`); whether the
  game acts on the same press is decided by `game_press` (fix round 2; this bullet first said the match equalled
  `App::key_name`, which was not true for shifted named keys until then). `Runtime::event` keeps the window's modifiers (`input::observe_modifiers`) and the
  key's place on the keyboard; `route_mod_key_based` names the press; the `input.key` event gets a new field
  `combo` (additive; `key` is unchanged); the bind callback in `native_state.rs` matches by it (a bare event
  without `combo` is its `key`).
- Precedence is in one place (`bindpage.rs`, the block comment "PRECEDENCE" over `never_mod_key`,
  `game_owner`, `priority`, `resolve`, `firing`): game control (a row, a macro, or the row the game takes a
  modified press as when the combo has no row of its own) first; a key the client keeps is never a mod's;
  among mods a saved key beats a default (in `resolve`), then addon id, then binding id.
- `resolve` works every mod's key out again from what is kept and each default, whenever a mod arrives, leaves,
  returns, is given a key or has Defaults pressed: no dependence on load or registration order, and a refused
  default keeps its reason across restarts (nothing empty is written for it; Defaults forgets entries instead
  of writing defaults).
- The surface files `docs/modding/SUPPORT-TABLE.md` and `eqoa-1-surface.json` were **hand-edited** (one new row,
  `eqoa.events.input.key.data.combo`, round 7), not regenerated: `eqoa mod-api` needs a graphics adapter and
  this machine has none, so the generated-docs test skips here and in CI. The lead should regenerate after
  merging (commit `270be2fb`, titled as the brief asks).

## Findings

| # | Reproduced? | Fix | Test and negative control (env switch `R7E_BREAK=n`, same binary; failing line) |
|---|---|---|---|
| H1 | yes: Ctrl+U on a game row also fired a bare-U mod; a mod saved on `ctrl+u` never fired | combo naming above; a mod key with modifiers that the game would take as the plain key is refused on Controls and never fires | `round7e_fix_h1_*` (4 tests through `Runtime::scripted_key`) and `round7e_fix_h1_the_windows_modifier_events...`. n=1 (modifiers ignored): 4 fail (`hits` assert, :34); n=2 (no shadow rule): `a_ctrl_combo_over_a_plain_game_key...` :429; n=12 (modifier events ignored): :648 |
| H2 | yes: default/saved `f11`, `f3`, `escape` all became mod keys | `never_mod_key`: reserved keys, Escape, Alt+Enter, Ctrl+C/V/X, with any other modifiers; a default or saved one leaves the row empty with the reason, file untouched | `round7e_fix_h2_m7_client_keys_are_never_mod_keys`, `..._the_clients_own_combos_are_refused_for_a_mod`. n=3: both fail (:506, :429); n=4 (Escape, Alt+Enter, clipboard only): the combos test fails :429 |
| H3 | yes (read: Options cancel restores `saved_binds` only) | **done, the lead-approved 2-line version** (section below) | `round7e_h3_*` (4 tests through the real `WndOptions` open, cancel and Save Changes keys). Control: the cancel hook call removed from `options.rs`: 3 fail (`a_cancel_puts_back_what_a_game_row_took_from_a_mod` :745, `..._a_mod_took_from_a_game_row` :759, `a_cancel_after_defaults...` :774), the save test passes |
| H4 | yes: with the probe's default refused the `other_hits==0` check passed whatever capture did | `fresh_hud_mod_load_...` restored: stats on `f10`, probe really bound on `y` (asserted), the editor owner has the keyboard (`other_hits==0`), then the probe fires once the editor is off (`other_hits==1`) | n=11 (the gate that keeps another VM's binding quiet while one VM owns the text editor is broken, chat suppression kept): the restored test fails at `round_3_picture_tests.rs:1064` (`the editor owner has the keyboard`); **the old test body from aa4fe9ba, run in the same binary on the final code, passes under n=11**, so it was vacuous |
| M5 | yes: returning mod not re-checked; Defaults wrote defaults for disabled mods and an empty entry that lost the reason | `resolve` on enable/disable/register/capture/Defaults; Defaults forgets entries | `round7e_fix_m5_*` (3). n=7 (no re-check on enable): `a_mod_returning_is_checked_against_the_game_too` :605; n=8 (Defaults writes defaults): :606 |
| M6 | yes: registration order decided | name order in `resolve`, saved before default | `round7e_fix_m6_*` (2). n=6 (no name order): :556; n=5 (saved not before default): 4 tests fail (:184, :350, :570, :591) |
| M7 | yes for Ctrl+C and Escape | Ctrl+C is H1 (a bare `c` mod does not fire on Ctrl+C); Ctrl+C itself and Escape are client keys (H2) | `round7e_fix_m7_a_bare_mod_key_does_not_fire_on_the_clipboard_combo`; n=1 fails it |
| M8 | yes | see "Corrections" | |
| L9 | yes: "Key not kept" was hidden behind the prompt | the capture ends on a failed save | `round7e_fix_l9_...`; n=9 (capture left open): :54 |
| L10 | accepted as decided | the Macros page (upstream) still lets a trigger take a mod's key; the macro wins, the mod row is marked | the player note says so |
| L11 | not fixed | the "Press the key for ." line is drawn by upstream's `ControlsPage::draw` from `label()`, which has no label for a mod row; no fork code is involved | |

Negative-control commands: build with the switch compiled in, then
`<exe> round7e_fix fresh_hud_mod_load zz_h4 --test-threads=1` with `R7E_BREAK=<n>` set; restore with
`git checkout -- crates` (status clean afterwards, no `brk` left, the temporary `zz_h4_old_version...` test
gone). Controls n=5 and n=7 were re-run on the final code after the priority simplification (n=5: four tests fail;
n=7: one fails).

On the H4 control: n=10 (the whole text-capture gate bypassed) fails both versions at the earlier chat step, so it shows nothing about the probe; n=11 is the control that matters (command: build with the switch, then `<exe> fresh_hud_mod_load zz_h4 --test-threads=1` with `R7E_BREAK=0` (both pass) and `R7E_BREAK=11` (restored fails, old passes)).

## H3: a cancelled Options session puts mod keys back (done, lead-approved 2-line version)

Two plain hook calls in `crates/eqoa/src/options.rs` (diff of that file against the base is exactly +2/-0),
nothing else upstream:

1. in `open()`, right after `self.saved_binds = crate::settings::binds();`:
   `crate::fork_ext::bindpage::session_begin();`
2. in the cancel arm (`27 | 132 =>`), right after `crate::settings::set_binds(self.saved_binds.clone());`:
   `crate::fork_ext::bindpage::session_cancel();`

`session_begin` snapshots what `mod-keys.opt` holds for every mod (including "nothing", which is what Defaults
forgets). `session_cancel` puts every changed entry back, in memory and in the file, then `resolve`s against
the game's keys that `options.rs` has just restored. Mod keys are still written as they are changed; only a
cancel rewrites them. NOT built, as decided: `session_commit()` in `settings::save()` and the `session_pending()`
term in `keep()`.

The `options.rs` budget in `docs/fork-footprint.json` is raised by the measured 2 lines (20 to 22 added) with
the reason recorded in `review_state` and in the file's own reason. Footprint PASS.

Tests (through the real `WndOptions`: `open_controls`, keys, `capture_key`, Triangle twice, Save Changes):
(a) Jump takes a mod's key, cancel: both back, nothing of the swap left in the file; (b) the mod takes Jump's
key, cancel: both back; (c) Defaults then cancel: the saved mod key and its file entry are back; (d) Save
Changes: a swap stays swapped in memory, in `keys.opt` (`key.jump=b`) and in `mod-keys.opt`, and a later window
cancelled changes nothing of it.

Known residual (also in the player note): with Auto Save Options off, leaving Options by a route that neither
saves nor cancels, then restarting, leaves the mod's new key saved beside the game's old key. The who-wins
order (the owner confirmed it 2026-11-20: game control, then macro, then mod; a chosen key beats a default;
then addon id and binding id) settles it and the mod's row is marked.

## Existing tests changed (H4 re-audit)

Each needed a key the game leaves free, or two mods on two keys. In each, the key pressed is also asserted to
do something (a gesture, a count, `dispatch_keypress` Some), so a refused binding cannot pass.

| test | proved before | proves now |
|---|---|---|
| `action_broker.rs` (4 tests, `k`) | a bound key arms the gesture for broker actions | same, key `u` (the gesture result is asserted) |
| `action_broker.rs` multiple listeners (`h`,`h`) | two bindings on one key both run actions | two bindings on `y` and `u` both run (a key two mods share does one thing now; the test is about listeners) |
| `bag_viewer_tests.rs` (`k`) | the bound key opens and closes the window, another key does not | same with `u`/`j`; the disabled mod's key fires nothing, after the opening with it was asserted |
| `native_tests.rs` assist (`k`), controls rebind (`p`,`z`), custom mod (`k`) | gesture; rebind (old key dead, new key fires); a disabled mod's key does nothing | same with `u`,`y`; the custom-mod test now also asserts the key fires while the mod is on (`7`) |
| `native_tests.rs` option_and_bind_names (3 mods on `b`) | each mod's bind sits under its own identity | each on its own key (`b`,`u`,`y`); same identity assertions and the permission result of the key press |
| `round_3_picture_tests.rs` fresh_hud (`f8`) | stats key via Controls; chat capture; an editor owner keeps the keyboard from another VM's binding | restored, see H4 |
| `api_capture_tests.rs`, `items_tests.rs`, `r6d_tests.rs` (`k`) | a bound key arms the gesture for item and window actions | same with `u` (their action results are asserted; they failed with the key refused) |
| `fork_ext/settings.rs` (`f8`,`f11`,`f12`) | stats key and a third party's key survive a native Save and reload | `f10`, `8`, `7` (`f11`, `f12` are client keys now); the third party's key is asserted to work |
| `bindpage.rs` unit tests (`k`,`m`,`f8`) | register, rebind, restart keep keys | `u`,`y`,`f10`; that test now starts from an empty registry (another test leaked a binding) |

## Corrections to the first report (M8)

- "One press does one thing" held only for unmodified keys and not for F11, F3, Escape, Alt+Enter, Ctrl+C: fixed
  above, not true at aa4fe9ba.
- "Default never takes a key in use / Defaults same rule" was false for disabled mods, for load order and for
  Defaults (empty entries lost the reason): fixed above.
- "Test data only" was wrong: the edited files also include `api_capture_tests.rs`, `items_tests.rs`,
  `r6d_tests.rs`, `fork_ext/settings.rs`, and one fixture (H4) lost its meaning.
- "Tests drive the real dispatch": they drove `ControlsPage` and `KeyEdge` but not the window path. The App
  itself (`window_event`, `key_input`, `key_name`) cannot be built in a test (it owns the window, the renderer
  and the session). The closest real seam is `Runtime::scripted_key` -> `route_key` -> `route_mod_key_based`,
  which `Runtime::event` also calls for a window key; the game's side of a press is `game_sends` in the tests,
  which in the first fix round was a shortcut of `App::key_name` that left out `ev.base`, the `key_names`
  press/release cache, capture and typing; since fix round 2 it is the branch-by-branch `GameKeys` mirror below. Not covered by any test:
  `Runtime::event` itself (needs a `Window`), `App::key_name` and `key_input` (that mod dispatch runs before the
  game's handling is read from `report_viewer.rs`, not run).

## What a player would notice (fix round)

- F3, F11, F12, Escape, Alt+Enter and Ctrl+C/V/X can no longer be mod keys (old saved ones stay in the file, do
  nothing, and the row says why).
- A mod on a key means the key as pressed: a mod on U does not fire on Ctrl+U (or Shift+U, Alt+U); a mod can be
  put on Ctrl+U on Controls; a modifier combination over a key the game uses as its plain key is refused.
- A mod's refused default keeps saying why after a restart; Defaults no longer writes empty entries.
- Leaving Options with Triangle now puts mod keys back with the game's; mod keys still write as they are changed, so a route that neither saves nor cancels leaves a mod's new key beside the game's old one (marked, settled by the who-wins order).

## Gates (fix round)

- Locally, with the disc (`EQOA_ISO` set), through `tools/run_gates.py`:
  - `--filter round7e_ ...`: all `round7e_` tests pass (30 at the first fix build, 31 with the later one added, 0 SKIP).
  - `--only round --filter fork_ext:: play::fork_hooks:: settings:: bindpage:: binds:: options::`: 1011 passed,
    0 failed, 26 ignored (11 distinct SKIP lines, all unrelated: no graphics adapter, symlink privilege,
    Wereoxxs recordings, UI goldens).
  - `--only wasm,footprint,paths`: all PASS (footprint: no upstream file touched, no budget change).
- CI run https://github.com/LoopRook/eqoa-enhanced/actions/runs/44699338192 on 4f7879bc: success. addons 600
  passed / 0 failed / 25 ignored (17 SKIP); full 3191 passed / 0 failed / 37 ignored (378 SKIP: no disc, no
  GPU in CI); wasm, footprint, paths, player-release (20), tools (199) success. `ci_gates.py cleanup` run.
- Not verified: the live client (the `!` mark, the notes, Ctrl and Alt presses on a real keyboard);
  `Runtime::event` and the App's key path (see Corrections); the generated docs (hand-edited, see above);
  that a *real* Save Changes with Auto Save Options on is unchanged (only the manual Save Changes path is tested).

### H3 round gates

- `--only round --filter round7e_ options::`: 69 passed, 0 failed (upstream's own Options tests included).
- Negative control: the cancel hook call removed from `options.rs` (restored by `git checkout`; the file's diff
  against the base is again exactly +2/-0): the three cancel tests fail, the save test passes.
- Wide local run `--only round,wasm,footprint,paths --filter round7e_ fork_ext:: options:: settings:: bindpage::
  binds:: play::fork_hooks::`: 1014 passed, 1 failed, 26 ignored. The one failure is
  `round_3_all_four_mods_per_frame_performance_benchmark` (5.081 ms against a 5 ms limit in a debug build while
  other agents were building); it is a timing benchmark that `run_gates.py`'s full step skips, and it passed
  alone straight after (1 passed). wasm PASS, footprint PASS with the new budget, paths PASS.
- CI run https://github.com/LoopRook/eqoa-enhanced/actions/runs/44706426719 on e16e8895: success. addons 604
  passed / 0 failed / 25 ignored (17 SKIP); full 3195 passed / 0 failed / 37 ignored (378 SKIP, no disc or GPU
  in CI); wasm, footprint, paths, player-release (20), tools (199) success. `ci_gates.py cleanup` run.


# Fix round 2 (recheck of b72b197f: H3, H4, M5, M6, L9 confirmed; the rest of H1 rejected)

Commits: `test(input): fix round 2 tests first ...` (tests plus the seams they drive, on the old behaviour),
then `fix(input): the game's own rule for a press decides ...`, then this report. Upstream files: no new line
(`options.rs` stays +2/-0; footprint PASS). Order kept as briefed: **the tests were run against the old
behaviour first: 9 of the 10 new tests failed for the intended reasons** (refusals missing for Shift+Space,
Shift+Tab, Shift+F5, Shift+Up, Shift+`, Shift+1 over `!`; `hits = 1` where a mod fired with Jump; release
combos `ctrl+u+,u-,u+,ctrl+u-` against the expected `ctrl+u+,ctrl+u-,u+,u-`; Super ignored; the cancel
set memory to `b` while the file kept `u`; 3600 allocations for 1200 key events with nothing listening). The
10th, `round7e_fix2_m2_the_game_names_a_release_the_same_way`, passes on both: it checks the test mirror of the
game's own release naming, not the fix.

| # | Reproduced? | Fix | Tests and negative control (`R7E_BREAK=n`, same binary, failing line) |
|---|---|---|---|
| 1 HIGH | yes: no refusal for the shifted named keys, and a mod fired together with Jump | `bindpage.rs` `game_press`: the single function for the game's side of a press, `App::key_name`'s rule for every key and modifier: the combo when the game has it bound (`Binds::combo_bound`), else the plain name (the key's own, or with Shift the character it types: `shifted`, US layout). `game_owner`, Controls (marks, refusal, swap/free: the mods now "shadowed" by a game capture are found with it), `default_in_use`, `can_take` and press-time `firing` all go through it. The letter/non-letter special cases are gone | `round7e_fix2_h1_*` (4: five shifted named keys refused; Shift+Space does not fire with Jump while a Shift+U mod fires; a game row on `!` vs Shift+1; a macro on `!`). n=1 (the old rule restored): 4 fail (`round7e_key_tests.rs` :418 x3, :34) |
| 2a MED | yes: `ctrl+u+,u-,u+,ctrl+u-` | `input.rs`: `PRESS_COMBOS` remembers the combo a key went down with (by its own name) and a release, a repeat and the focus loss use it, as the game's `key_names` does; a plain press clears it | `round7e_fix2_m2_a_release_is_named_as_its_press_was`. n=2 (release named from the current modifiers): :956 |
| 2b MED | yes: 3 allocations per key event with no listener | `route_mod_key_based` returns first when `native_core::capture_requested()` is false: no name made, no map touched, no event built | the zero-addon test `performance_busy_world_zero_addons_frame_and_packet_allocation_bound` now has a key case (modifiers held, press and release x 600): 0 allocations. n=3 (guard removed): 3601 allocations, `api_capture_tests.rs` :730. (The control build used a cached env switch so that the switch itself allocates nothing; with an allocating switch the 0 would not have held, which is how I found out) |
| 3 LOW | yes: Super ignored | `input.rs` tracks Super (`set_super`, `observe_modifiers`); a press with Super held is named `super+...`, which no bare binding matches; `never_mod_key` treats `super+c/v/x/,` like their Ctrl forms; `game_press` drops Super, as the game does | `round7e_fix2_l3_*` (2: Cmd+U does not fire a bare-U mod, window `ModifiersChanged` carries Super; `super+c`, `super+v`, `super+x`, `super+,` refused, a default on `super+c` refused). n=4 (Super ignored in the press name): both fail (:34); n=5 (`never_mod_key` without the Super forms): :418 |
| 4 LOW | yes: memory set though the write failed | `restore_session(write)` (the seam `session_cancel` uses): a key whose write fails keeps its in-memory value (memory and file agree) and the failure is said (`crate::say`) | unit test in `bindpage.rs` `round7e_fix2_a_cancel_whose_write_fails_leaves_memory_and_file_agreeing`, with a working-write control. n=6 (memory set anyway): `bindpage.rs` :1235 |
| 5 LOW | yes | claims corrected (below); `game_sends` is now `GameKeys::press`, a branch-by-branch mirror of `App::key_name` | see below |

Negative-control method as before (switch compiled in, `<exe> round7e_fix2 --test-threads=1` once per n,
restored with `git checkout -- crates`, status clean, no `brk` left).

## Corrections (item 5)

- The "What changed" bullet of the first fix round said the match equalled `App::key_name`. That was false for
  shifted named keys, shifted symbols and Super until this round; the bullet is corrected in place.
- The first fix round described the test helper `game_sends` as "`App::key_name`'s rule written out". It was a
  shortcut that left out `ev.base`, the `key_names` press/release cache, `capture` and `typing`. It is now
  `GameKeys` in `round7e_key_tests.rs`: `name()` is the body of `App::key_name` (viewer.rs) one branch at a time
  with the upstream line quoted in a comment beside each, `ev.base` and the cache included; `capture` and
  `typing` are arguments. It leaves out where the App gets those two flags and the window events, and
  `KeyEdge`'s typing mode. `GameKeys::press` feeds the name to the real `KeyEdge`.
- Still true and still not covered by a test: `Runtime::event` (needs a `Window`), the App's `key_input`, and that
  mod dispatch runs before the game's handling (read from `report_viewer.rs`, not run). The US layout is assumed for
  what Shift types (`shifted`); another layout's Shift symbols are not known to the client either (it uses the
  key's physical place, `ev.base`, for the combo, and the typed character for the plain name).

## Gates (fix round 2)

- Local with the disc (`EQOA_ISO` set), through `run_gates.py`: `--filter round7e_ fork_ext:: options:: settings::
  bindpage:: binds:: play::fork_hooks::`: 1024 passed, 0 failed, 26 ignored (11 distinct SKIP lines, unrelated: no
  graphics adapter, symlink privilege, Wereoxxs recordings, UI goldens); 0 SKIP among the `round7e_` tests. wasm
  PASS, footprint PASS, paths PASS.
- CI run https://github.com/LoopRook/eqoa-enhanced/actions/runs/44712307407 on 885297f9: success. addons 612
  passed / 0 failed / 25 ignored (17 SKIP); full 3204 passed / 0 failed / 37 ignored (378 SKIP, no disc or GPU in
  CI); wasm, footprint, paths, player-release (20), tools (199) success. `ci_gates.py cleanup` run.
- Not verified: the live client; real Shift, Ctrl, Alt, Cmd presses on a keyboard; layouts other than US.
