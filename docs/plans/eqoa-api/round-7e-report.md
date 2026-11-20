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
| 1 HIGH | yes: no refusal for the shifted named keys, and a mod fired together with Jump | `bindpage.rs` `game_press`: one function for the game's side of a key where no press exists, `App::key_name`'s rule with a US-layout table for what Shift types (the claim "for every key and modifier" was wrong: it guessed; fix round 3 replaces the guess at press time): the combo when the game has it bound (`Binds::combo_bound`), else the plain name (the key's own, or with Shift the character it types: `shifted`, US layout). `game_owner`, Controls (marks, refusal, swap/free: the mods now "shadowed" by a game capture are found with it), `default_in_use`, `can_take` and press-time `firing` all go through it. The letter/non-letter special cases are gone | `round7e_fix2_h1_*` (4: five shifted named keys refused; Shift+Space does not fire with Jump while a Shift+U mod fires; a game row on `!` vs Shift+1; a macro on `!`). n=1 (the old rule restored): 4 fail (`round7e_key_tests.rs` :418 x3, :34) |
| 2a MED | yes: `ctrl+u+,u-,u+,ctrl+u-` | `input.rs`: `PRESS_COMBOS` remembers the combo a key went down with (by its own name) and a release and a repeat use it (the focus loss only CLEARS the record: it emits no release), as the game's `key_names` does; a plain press clears it | `round7e_fix2_m2_a_release_is_named_as_its_press_was`. n=2 (release named from the current modifiers): :956 |
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
  what Shift types (`shifted`); [this claim was FALSE, retracted in fix round 3: the game takes what a press typed from the event's logical key, which is right on any layout, and the fork now does the same at press time] (it uses the
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


# Fix round 3 (recheck of 288353f2: items 3, 4, 5 of round 2 confirmed; the root of item 1 rejected)

Commits: `test(input): fix round 3 tests first ...` (the tests and one test-only accessor, run on the old
behaviour), `fix(input): the press decides ...`, two docs commits (spec, player note, corrected claims; the
generated surface files), then this report. No new upstream line: `options.rs` stays +2/-0, footprint PASS.
**The tests were run against the old behaviour first: all 8 failed for the intended reasons** (`hits = 1` where
a mod fired together with a game control on a German and a UK layout and on the numpad, a Shift+7 capture
accepted, release names `1/1+,!/!-` where `1/1+,1/1-` was due, `ctrl+u+,ctrl+u-` where `ctrl+u+,u-` was due,
`shift+ctrl+u` not read as `ctrl+shift+u`, the key ring holding 128 events with no mod running).

## The design (binding decision A, B, C) and what it is in the code

- **A. Press time is exact.** `input.rs` `route_mod_key_based` has the event's own facts: the logical key, its
  place on the keyboard (`base`), the modifiers. `press_names` works out the name the game will act on for this
  press exactly as `App::key_name` does (viewer.rs): the combo when the game has that combo bound
  (`Binds::combo_bound`), else the plain name from the logical key, which is layout-aware. The `input.key` event
  carries it as a new field `game_key` (additive; surface files updated by hand to match the generator, round 7).
  The bind callback asks `bindpage::fires(addon, id, combo, game_key)`, which decides "the game acts on this press"
  from `game_key` (`owner_of`: a row or a macro) with no table. So on a German layout Shift+7 (typing `/`,
  the default command key), a UK Shift+' (typing `@`), Ctrl+Numpad1 and Shift+Numpad1 with NumLock off (arriving as
  `end`) are decided by what the event says. Tests drive synthetic events through `Runtime::scripted_key_based`.
- **B. A capture is exact too.** The window's key reaches the mod runtime before the Controls page sees it, so the
  press has already been seen: `route_mod_key_based` records what a modified press typed (`bindpage::learn`,
  combo -> typed key, in memory) while any mod has a row, and the capture's conflict decision (refuse, swap or
  free, note) uses it: `round7e_fix3_a_capture_uses_what_the_real_press_typed` (a German Shift+7 pressed, then
  captured on a mod row: refused naming the command key; with no press seen the US table says `&` and the key is
  accepted).
- **C. A static rule only where no press exists.** `game_press` (the old function, now `parse_key`-based) is the
  fallback for defaults, keys read from a file and the marks on Controls: learned combos first, then the key's
  own name, or with Shift the US layout's character (`shifted`). Nothing is written to `mod-keys.opt` or any file,
  no field was added to a saved file (`round7e_fix3_the_marks_use_what_presses_showed_and_nothing_is_written`).
  **Plainly:** on a non-US layout that static rule can give a wrong or missing mark or refusal on Controls
  until the key has been pressed once on this machine; it can never cause a double fire or a dead key at press time,
  because press time does not use it. (The report's earlier line that another layout's Shift symbols are not known
  to the client was false and is retracted above.)

## Findings

| # | Reproduced? | Fix | Tests and negative control (`R7E_BREAK=n`, same binary, failing line) |
|---|---|---|---|
| 1 A, B, C | yes (see the first run above) | as above | `round7e_fix3_a_german_shift_7...`, `..._a_uk_shift_quote...`, `..._the_numpad_with_numlock_off...` (Ctrl and Shift, with controls where the mod must fire: US Shift+7 typing `&`, UK `"`, Ctrl+Digit1), `..._a_capture_uses_what_the_real_press_typed`, `..._the_marks_use_what_presses_showed...`. n=1 alone passes (the press-time decision falls back to the static function, which has by then been taught by the same press: learning and the exact name agree); **n=1,2 (the press decision on the static guess and no learning): 5 fail** (`round7e_key_tests.rs` :34 x3, :418, :1201). n=2 (nothing learned): the capture test :418 and the marks test :1201 fail |
| 2 MED | yes: the key ring held 128 events with no mod; a mod with nothing bound or listening did the naming and event work | `route_mod_key_based` returns when no VM subscribes to `input.key` and no editor has the keyboard (`native_events::interested`); `Runtime::route_key` returns first when no mod runs (`has_consumers`), nothing is held (`Capture::is_empty`) and no editor has the keyboard | `round7e_fix3_nothing_listening_means_no_key_work`: zero mods: 0 allocations over 600 key events and an empty key ring; one enabled mod with no key bound and no listener: 0 allocations; control with a listening mod: allocations seen. n=6 (listener check removed): :1178; n=7 (`route_key`'s early return removed): :1165 |
| 3 MED | yes: `1/1+,!/!-` | `PRESSES` keeps the names of EVERY press (plain ones too) by its own name, and a release and a repeat use them, as `key_names` does (`key`, `combo`, `game_key` all press-time) | `round7e_fix3_a_release_is_named_as_its_press_even_when_unmodified`. n=3: :1108 |
| 4 LOW | yes: `shift+ctrl+u` never matched; `ctrl+super+c` slipped through | `bindpage::parse_key` reads Super, Ctrl, Alt, Shift in any order; `normalize_key` is applied when a name is read (file, default, set_key, capture); `never_mod_key` and `game_press` use `parse_key`. A let-go key is forgotten before the "is anyone listening, is this plain play" checks, so a menu or ImGui having the keyboard no longer leaves it behind | `round7e_fix3_modifier_order_in_a_saved_name_does_not_matter`, `round7e_fix3_a_let_go_key_is_forgotten_wherever_it_went`. n=4 (no normalising): :1138; n=5 (forgotten only where the event is sent): :1128 |
| 5 LOW | yes | corrected: see below | |

Negative-control method as before (switch compiled in with a cached env read so the switch itself allocates
nothing; `<exe> round7e_fix3 --test-threads=1` once per n; restored with `git checkout -- crates`, status clean).
Controls 1 and 2 are the pair that matters: press-time exactness is the second line of defence behind learning,
which is why n=1 alone passes; with both broken the table's wrong answers fail five tests.

## Corrections (item 5)

- Round 2 said `game_press` followed `App::key_name` "for every key and modifier". It guessed what Shift types from
  a US table; the game uses the real event. The line is corrected in place, and so is the sentence "another
  layout's Shift symbols are not known to the client either", which was false.
- Round 2 said a focus loss "uses" the remembered combo. Focus loss CLEARS the record (the window sends no release);
  corrected in place.
- I wrote earlier that the surface/support-table test skips in CI. It does not: CI has an adapter and runs it
  (`round7b_support_table_matches_the_running_client`). It failed on the first CI run of this round because the
  `game_key` field was in the code but not yet in the hand-edited docs; the docs were then edited to match the
  generator exactly. It still cannot run on this machine (no adapter), so the lead should still regenerate after
  merging.

## Gates (fix round 3)

- Local with the disc (`EQOA_ISO`): `--filter round7e_ fork_ext:: options:: settings:: bindpage:: binds::
  play::fork_hooks::`: 1033 passed, 0 failed, 26 ignored (unrelated SKIPs: no graphics adapter, symlink privilege,
  Wereoxxs recordings, UI goldens); 0 SKIP among the `round7e_` tests. wasm, footprint, paths PASS.
- CI run https://github.com/LoopRook/eqoa-enhanced/actions/runs/44721739876 on 95d86cc3: success. addons 621 passed /
  0 failed / 25 ignored (17 SKIP); full 3213 passed / 0 failed / 37 ignored (378 SKIP, no disc or GPU in CI); wasm,
  footprint, paths, player-release (20), tools (199) success; the support-table test ran and passed with the
  hand-edited surface files. The run before it (44718774177, code at d16ba3ad, docs not yet committed) had failed
  only that test, for the reason given under Corrections. `ci_gates.py cleanup` run.
- Not verified: the live client; real keyboards on German or UK layouts, a real numpad (the layouts and the numpad
  are tested with synthetic events carrying the logical key and the key's place that those keyboards send); the
  App's `key_input` ordering, as before.


# Owner test fix 1 (owner: "I was able to assign Shift+Space, however, jump works just fine")

Branch `feat/eqoa-api-r7e`, develop merged first (fast-forward to a3da75d8: the branch was already inside develop,
15163407 included my round-3 work). Upstream lines: none new (`options.rs` stays +2/-0, footprint PASS).

## Cause, in two sentences

**What I could prove:** a mod key on a combo the game does NOT have bound (Shift+Space with nothing on it) is refused
in the live order; I traced and ran that and it holds (below). **What I found that is accepted and wrong:** when a game
control already holds the combo itself (a bar slot, say, on Shift+Space), `mod_row_capture` treated
it as an exact owner and swapped or freed that control for the mod, without asking what the game would act on for the
press *after* the control let go: with the combo free again the game takes Shift+Space as Space, Jump's, so the mod was
accepted and was dead at once (Jump works fine; the row is marked). That matches the owner's words, but **I cannot
see the owner's state**: their saved files end without a `shift+space` (the session was cancelled or overwritten), so I
could not confirm that a control held Shift+Space at that moment. The lead should ask the owner (below).

## The live path, traced (the instructions' step 2)

Order in the client: `window_event` -> `fork_report::event` -> `report_viewer::addon_event` (the mod runtime sees
`ModifiersChanged` and the key FIRST: `Runtime::event` -> `observe_modifiers`, then `route_key` -> `route_mod_key_based`)
-> `handle_window_event` -> `key_input` -> `key_name(capture = true)` -> `Play::capture_key` -> `WndOptions` ->
`ControlsPage::capture_key` -> the mod hook (`bindpage::capture_key`).
- **Which function receives the key / what name:** the hook gets the page's name, which `App::key_name` makes with the
  capture flag: `shift+space` (viewer.rs: the combo whenever `capture` is true). Space is `Key::Named(Space)` on Windows
  in every modifier state (winit 0.30.13 `keyboard_layout.rs`: `vkey_to_non_char_key` runs first and returns it), so it
  takes the same branch as the keys the tests press.
- **Is the learned record filled before the capture decision?** Yes: the mod runtime runs before `key_input`, and the
  learning in `route_mod_key_based` comes BEFORE the "is anyone listening / is this plain play" returns, so
  `plain_play == false` on the Options page does not skip it. (If no VM is active at that moment `route_key` returns
  early and nothing is learned; the capture then falls back to the static rule, which gives the same answer for Space.)
- **Does the Options page's `plain_play` matter at press time?** There is no mod press while Options is up; the
  press-time names are only made in play.
- Verified by running the order in tests: `round7e_owner1_*` (below) enter at the mod runtime (`Runtime::scripted_key_based`
  with the modifiers set through `observe_modifiers`, `plain_play` false), then `GameKeys` for `App::key_name`, then the
  real `WndOptions` (open, rows, ✕, `capture_key`, draw). **The gap that remains** (and where the earlier tests entered):
  `Runtime::event`'s own match on the window event (needs a `Window`), `App::key_input` and `key_name` themselves, the
  real `self.mods` and `typing`, and winit's `KeyEvent`. The earlier tests called the hook with a hand-written name and
  never had a game control already on the combo, which is the state that was accepted wrongly.

## Why the tests missed it

The Shift+Space tests all started from a clean layout (nothing on `shift+space`), the one state where the exact-owner
branch is not taken. The branch that was wrong is the one where a control already has the combo.

## The fix

`mod_row_capture`: when a game row is going to give up the combo (swap or free), the layout AFTER that change is checked
with `game_owner` first; if the game would then act on the plain key and a control has it, the capture is refused naming
that control (`shadow_text`), and nothing has changed (the layout is only a copy until `set_binds`). Also: every Controls
capture, refusal and kept key now writes one line to `eqoa.log` (`controls: mod row N given shift+space: the game acts on
space for that press (it has row jump on it)`, `controls: refused: ...`, `controls: mod X.Y kept ...`), and a saved mod key
the game has the press of says so once (`controls: mod X.Y on shift+space does nothing: the game acts on space for that
press`), so an odd result in the next test can be read from the log.

## Tests

`round7e_owner1_*` (all in `round7e_key_tests.rs`):
- `shift_and_friends_over_a_game_key_are_refused_in_the_live_order`: Shift+Space, Shift+Tab, Shift+Enter, Shift+Up,
  Shift+F5, Ctrl+Space, Alt+Space, each through the live order, each refused. `a_shifted_character_is_what_it_types`:
  Shift+1 (types `!`, nothing has it) is accepted.
- `shift_space_is_refused_after_the_swaps_the_owner_made`: fresh, after a swap then cancel, after Jump taking a mod key
  then cancel, after Defaults then cancel, after a swap saved and swapped back: always refused with Jump on Space.
- `a_saved_shift_space_is_marked_says_jump_and_does_not_fire` (step 4): the saved key is kept as it is, the row is marked
  and says "Jump ... the game's control wins", and a Shift+Space press fires Jump only (the game acts; the mod does not;
  nothing fires twice).
- `a_game_control_on_shift_space_cannot_hand_it_to_a_mod_that_jump_then_beats`: **the reproduction.** On the unfixed code
  it failed (`round7e_key_tests.rs:418`: the capture was accepted); with the fix it passes, including the control that
  a combo whose plain key nothing has (`shift+u`) is still handed over by the usual swap.
- Negative control for the live-order tests: `R7E_BREAK=1` (the combo treated as its own name, no plain-key rule)
  fails three of the four `owner1` tests (:1267, :1345, :1357); with it off all pass.

## What the owner should retest

1. Give a bar slot (or any game control) Shift+Space, then give a MOD Shift+Space: refused, the note names Jump, the
   slot keeps Shift+Space.
2. With nothing on Shift+Space, give a mod Shift+Space: refused naming Jump (this was already so in every test).
3. If it is still accepted somewhere: send `eqoa.log`; the `controls:` lines say which kind of row was given the key, what
   name the page got, what the game acts on for it and what it has there. Please also say which row (game or mod) took
   Shift+Space, and whether any control already had it.
4. A mod already saved on Shift+Space: its row is marked with ! and says Jump's wins; pressing Shift+Space jumps and
   the mod does nothing.

## Gates
- Local with the disc (`EQOA_ISO`): `--filter round7e_ fork_ext:: options:: settings:: bindpage:: binds:: play::fork_hooks::`:
  1120 passed, 0 failed, 26 ignored (unrelated SKIPs); 0 SKIP among the `round7e_` tests. wasm, footprint, paths PASS.
- CI run https://github.com/LoopRook/eqoa-enhanced/actions/runs/44792534526 on b895c0a5: success. addons 663 / 0 failed;
  full 3300 / 0 failed (378 SKIP: no disc or GPU in CI); wasm, footprint, paths, player-release (20), tools (201)
  success. `ci_gates.py cleanup` run.
- Not verified: the live client itself (Runtime::event, App::key_input and key_name are not run by any test), and
  that this is the state the owner was in (see "Cause").


# Owner test fix 1, part 2: the live behaviour is the rule (owner's correction, 2026-11-20)

The section above describes the first reading (a Shift+Space mod key should be refused). **It is superseded**: the owner
says that with a mod on Shift+Space and Jump on Space, Shift+Space fires the mod and does not jump, plain Space still
jumps, and that is wanted. The lead decided the live behaviour is the rule. Commits: `fix(input): a modifier plus a key
the game uses plain is a key of its own ...` and `test(input): a bare mod key is never consumed`, then this report.
Upstream lines: none new (`options.rs` stays +2/-0, footprint PASS).

## 1. The mechanism (file:line), and the honest answer

**I could not find a mechanism in the code that makes the game not act on plain Space for that press.** What I checked:
- `viewer.rs:5667-5686` (`App::key_name`): with the combo not bound in the game and no capture waiting, the press is named
  by the plain key (`space`); `viewer.rs:5870+` sends it on through `KeyEdge` unchanged; `viewer.rs:5018-5027` (the jump
  match) tests `(None, Key::Named(Space))` and does not read the modifiers. So **before any mod holds the combo,
  Shift+Space jumps** (on a server that has jumps).
- `Binds::combo_bound` / `alias` (`binds.rs:594-609, 744-746`) read only the game's own rows and macros: there is no fork
  hook that adds mod keys to them (`grep fork_ext::bindpage binds.rs` finds nothing).
- The press-time code before my change (`input.rs` `route_mod_key_based`, `runtime.rs` `route_key`) never consumed a
  press for a mod: `wants_capture` came from ImGui, text capture, or Forge's `SetKeySuppressed` only
  (`input.rs wants_key_capture`, `capture.rs`), and a consumed event is the only way the game never sees a key
  (`report_viewer.rs addon_event` returns true, `viewer.rs:5480` returns).
- The Controls capture writes game binds only when it swaps or frees a game row (`bindpage.rs mod_row_capture`).
By the code alone Shift+Space jumped as well, so the owner's build behaved differently from what I can read: a mod that
claims keys itself (`eqoa.input.claim_key` with suppress, which Wereoxxs UI in the owner's mod list does call:
`SetKeySuppressed` is in their `eqoa.log`) is the one candidate I can name, and I cannot confirm it. **That is why the
behaviour is now made explicit instead of incidental**: see the rule.

## 2. The rule, now stated once (`bindpage.rs`, the "THE RULE" block)

- A key is a NAME: the modifiers held written before the key (Super first, then Ctrl, Alt, Shift). Shift+Space is not Space.
- A mod may hold a modifier combination over a key the game uses plain. While it does, **that press is the mod's alone**:
  the mod fires and the key event is consumed in `Runtime::route_key` (`input::mod_owns_press` -> `bindpage::owns_press`), so
  the game does not act on the plain key for it. The plain key keeps working for the game. A bare key is never taken.
- A real conflict is the SAME NAME bound in the game: a game row, or a key in a macro's trigger, by that exact name (a bare
  key the game uses, or the combination itself bound there). The game's control wins at press time and Controls marks it. On
  Controls a game row on that name is swapped or freed for the mod as before (a macro's key is refused).
- Controls (capture, marks, defaults, saved keys), the static rule and press-time firing all use that one rule
  (`game_owner`, `firing`). Nothing is decided by what a press types: the layout guessing is gone (`shifted`, `game_press`,
  the learned record, the event field `game_key`, and the surface rows for it).
- German Shift+7 (types `/`, the command key): the rule gives **no conflict**, because the key is the name `shift+7` and the
  game has no `shift+7` bound; the mod owns that press and the command line does not open for it. The bare `/` is the
  game's and untouched. Same for the UK Shift+' (types `@`) and the numpad with NumLock off (Ctrl+Numpad1 is `ctrl+1` by
  its place, though it arrives as End).

## 3. Tests: every assertion changed (before -> after, and why)

All in `round7e_key_tests.rs`. Old = refused/blocked by the plain-key (shadow) rule; new = the owner's rule.
| test | before | after |
|---|---|---|
| `fix_h1_a_ctrl_combo_over_a_plain_game_key_..._never_fires` | Ctrl+W refused on Controls; a saved ctrl+w never fires | renamed `..._is_the_mods_and_w_still_walks`: accepted, no mark; Ctrl+W fires the mod and the press is consumed; W alone is not consumed and still reaches the game |
| `fix2_h1_shift_on_a_named_key_is_the_games_plain_key` | Shift+Space/Tab/F5/Up/` refused naming the control | `..._is_a_key_of_its_own_for_a_mod`: all (and Shift+Enter) accepted, no mark |
| `fix2_h1_shift_space_does_not_fire_a_mod_with_jump` | mod never fires on Shift+Space (Jump acts) | `..._belongs_to_the_mod_and_space_still_jumps`: before a mod holds it the press reaches the game; after, the mod fires and the press is consumed; Space alone reaches the game and the mod does not fire |
| `fix2_h1_a_game_row_on_a_shifted_symbol_is_the_games_press` | Shift+1 refused (game on `!`) | accepted: `!` and `shift+1` are different names; fires, consumed |
| `fix2_h1_a_macro_on_a_shifted_symbol_is_the_games_press_too` | Shift+1 refused (macro on `!`) | `..._same_name_still_refuses_the_mod`: a macro on bare `!` does not hold Shift+1; a macro on `shift+1` itself still refuses |
| `fix3_a_german_shift_7_types_slash_and_the_game_acts` | mod did not fire (game acts on `/`) | `..._is_the_mods_by_its_name_not_by_what_it_types`: fires and consumed; bare `/` untouched; US Shift+7 the same name |
| `fix3_a_uk_shift_quote_types_at_and_the_game_acts` | mod did not fire (slot on `@`) | `..._is_the_mods_by_its_name_even_with_a_game_row_on_at`: fires, consumed |
| `fix3_the_numpad_with_numlock_off_arrives_as_end` | mod blocked by End (Orbit right) | `..._is_named_by_its_place`: ctrl+1 / shift+1 fire and are consumed; End alone not |
| `fix3_a_capture_uses_what_the_real_press_typed` | German Shift+7 capture refused | `..._is_accepted_whatever_the_layout_types` |
| `fix3_the_marks_use_what_presses_showed_and_nothing_is_written` | mark appears once a press showed Shift+7 types `/` | `fix3_a_mark_means_the_same_name_is_the_games`: no mark for shift+7; a game row on exactly `shift+7` marks it and names the row |
| `owner1_shift_and_friends_over_a_game_key_are_refused_in_the_live_order` | 7 combos refused | `..._are_accepted_in_the_live_order` (live order: modifiers, mod runtime, `GameKeys`, real Options window) |
| `owner1_shift_space_is_refused_after_the_swaps_the_owner_made` | refused in 5 states | `..._is_accepted_...` in the same 5 states; Jump keeps Space |
| `owner1_a_saved_shift_space_is_marked_says_jump_and_does_not_fire` | marked, says Jump, mod silent | `..._fires_the_mod_and_not_jump_and_is_not_marked`: this is the owner's live case |
| `owner1_a_game_control_on_shift_space_cannot_hand_it_to_a_mod_that_jump_then_beats` | refused (Jump would beat it) | `..._is_a_real_conflict_and_swaps_as_any_game_key`: a saved mod key loses at press time and is marked; given on Controls the slot is freed and the mod fires (consumed) |
New: `owner1_a_bare_mod_key_is_never_consumed_only_a_modifier_combination_is`. Unchanged and still passing: bare-key conflicts
(`round7e_legacy_conflicts...`, marks, defaults, macro refusal), the client keys, release naming, normalisation, the
zero-allocation key path, H3 cancel/save.

## 4. Negative controls (`R7E_BREAK=n`, same binary, one run per n)
- n=1 (a mod never owns a press): 8 tests fail (the shift+space, ctrl+w, German/UK/numpad, saved shift+space, game-control cases).
- n=2 (the game never has an exact key): 18 tests fail (every conflict, mark, swap and default test).
- n=4 (bare keys consumed too): passed at the time: no test then asserted it; `owner1_a_bare_mod_key_is_never_consumed...` now does
  (control run for it is with the manager-row build below).

## 5. What the owner should retest
1. A mod on Shift+Space, Jump on Space: Shift+Space fires the mod, does not jump; Space jumps. No `!` on the row.
2. Give a game control (a bar slot) Shift+Space: then a mod Shift+Space: the slot loses it (the usual note); and a mod already
   saved on a name a game control has: marked, the game wins.
3. German/UK keyboards if available: a mod on Shift+7 owns that press.
4. Local gates: see Gates.

## Gates (owner test fix 1)
- Local with the disc: `--filter round7e_ fork_ext:: options:: settings:: bindpage:: binds:: play::fork_hooks::`: 1121 passed,
  0 failed, 26 ignored (unrelated SKIPs); wasm, footprint, paths PASS. CI for this and the next section runs once on the final
  commit (see Owner request 1).
