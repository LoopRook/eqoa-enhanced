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
text, `270be2fb` generated docs (hand-edited, see below), then the player note. Zero upstream-file changes
(footprint below). **The tests were written and run against the unfixed code first: all 12 failed, each for
the intended reason** (`hits = 1` where a mod fired twice or on the wrong press, `f11` kept as a mod key, a
capture left waiting, a default taken in one load order).

## What changed

- A mod key is matched by the key name the game gives the press: the modifiers held written before the key
  (`binds::combo_name`). `Runtime::event` keeps the window's modifiers (`input::observe_modifiers`) and the
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
| H3 | yes (read: Options cancel restores `saved_binds` only) | **NOT DONE: needs an upstream hook, see below** | none |
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

## H3 (STOP): mod keys following Save Changes / cancel

Not doable without an upstream line. What exists: `Options` keeps `saved_binds`, restores it on cancel
(`options.rs`, the `27 | 132 =>` arm, `crate::settings::set_binds(self.saved_binds.clone())`), and writes keys in
`settings::save` through `save_binds()` only when the game's own `Binds` changed. Mod keys are not in `Binds`
or `ClientOptions`, so no existing hook (`fork_options::change/activate`, `fork_settings::write/parse`,
`bindpage::draw/capture_key/help_or_defaults`) sees a cancel, a save, or even that Options is open; a change
to a mod key alone does not make Options think anything changed. Two lines would give restore-on-cancel
(keys stay written at once, which a cancel then rewrites):

1. `crates/eqoa/src/options.rs`, in `open()`, after `self.saved_binds = crate::settings::binds();`:
   `crate::fork_ext::bindpage::session_begin();` (remembers every mod key as it is)
2. same file, in the cancel arm, after `crate::settings::set_binds(self.saved_binds.clone());`:
   `crate::fork_ext::bindpage::session_cancel();` (puts the remembered keys back and rewrites `mod-keys.opt`)

For the full "pending until Save Changes, written at once with Auto Save Options" lifecycle also:

3. `crates/eqoa/src/settings.rs`, in `save()` after `let keys = save_binds();`:
   `crate::fork_ext::bindpage::session_commit();`, and in `options.rs` `keep()`'s unchanged check,
   `&& !crate::fork_ext::bindpage::session_pending()`.

The lead decides. Until then: a cross change (a mod and a game row swapping keys) that is cancelled leaves the
saved mod key and the old game key; the press-time rule settles it (game wins, mod row marked). Nothing is lost
except the mod's previous key choice.

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
  which `Runtime::event` also calls for a window key; the game's side of a press is `game_sends` in the tests:
  `App::key_name`'s rule written out (viewer.rs) and fed to the real `KeyEdge`. Not covered by any test:
  `Runtime::event` itself (needs a `Window`), `App::key_name` and `key_input` (that mod dispatch runs before the
  game's handling is read from `report_viewer.rs`, not run).

## What a player would notice (fix round)

- F3, F11, F12, Escape, Alt+Enter and Ctrl+C/V/X can no longer be mod keys (old saved ones stay in the file, do
  nothing, and the row says why).
- A mod on a key means the key as pressed: a mod on U does not fire on Ctrl+U (or Shift+U, Alt+U); a mod can be
  put on Ctrl+U on Controls; a modifier combination over a key the game uses as its plain key is refused.
- A mod's refused default keeps saying why after a restart; Defaults no longer writes empty entries.
- Mod key changes still write at once (H3 not done).

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
  H3 (not built).
