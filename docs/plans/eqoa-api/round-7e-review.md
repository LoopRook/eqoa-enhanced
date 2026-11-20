# Round 7 Part E review (Gemini 4 Hydrogen high, read-only, nothing run) — condensed by the lead

Verdict: **REJECT** at `aa4fe9ba`. All findings "verified by reading the code" by the reviewer; none run.
Paths are relative to `crates/eqoa/src/`. Lead decisions follow each finding ("DECISION").

## HIGH

**H1. Modifier combinations: one press still fires both; combo-bound mod keys are dead.**
`fork_ext/addons/input.rs:162-187` (`route_mod_key`) names the key with `binds::name_of(key)` only and drops
Ctrl/Alt/Shift, while `viewer.rs:5667-5687` (`App::key_name`) builds `ctrl+…`/`alt+…`/`shift+…` names for game
binds and for capture on Controls. `fork_ext/bindpage.rs:238-250` (`fires`, `game_row`) compare exact strings,
so `ctrl+u` and `u` (or `shift+1` and `!`) count as unrelated.
Scenario: game action or macro on Ctrl+U, mod on U: no mark, and pressing Ctrl+U fires both. A mod bound to
Ctrl+U on Controls saves `ctrl+u`, which `route_mod_key`'s `u` never matches: silently dead.
DECISION: a mod key is matched by the same key name the game uses for that press (the combo name, modifiers
included), in dispatch and on Controls alike. A bare `u` binding does not fire on Ctrl+U.

**H2. Reserved client keys and Alt+Enter.**
`fork_ext/bindpage.rs:302-316` (`default_in_use`, `firing`) never check `binds::reserved` (`binds.rs:234-236`:
f3, f11, f12), though `ControlsPage::capture_key` refuses them. Mod dispatch (`fork_report::event` →
`addon_event` → `route_mod_key`) runs before `handle_window_event`/`key_input` (`viewer.rs:5478-5485`,
`5728-5739`, `4254-4262`). Scenario: a mod with `default_key="f11"` (or a hand-edited/old `mod-keys.opt`)
fires together with fullscreen; F3 with the bug-report form; a mod on Enter fires on Alt+Enter (fullscreen).
DECISION: reserved keys are never mod keys: a default or saved reserved key leaves the row empty with the
reason, and never fires. Alt+Enter is solved by H1 (`alt+enter` is not `enter`).

**H3. Save asymmetry and Defaults lose bindings.**
`fork_ext/bindpage.rs:222-236`, `544-549`, `632-638`; `options.rs:731-746`. Mod captures and `reset_defaults()`
write `mod-keys.opt` at once; game binds stay in memory until Save Changes, and cancelling Options (Triangle)
restores `saved_binds` without restoring mod keys. Scenarios: (a) Jump takes a mod's B, then cancel: Jump is
back on Space, the mod was saved onto Space: the mod lost B and is dead. (b) mod takes Space, cancel: Jump back
on Space, mod dead on Space. (c) Defaults then cancel: all saved mod keys already overwritten.
DECISION: mod keys follow the same lifecycle as game keys on the Options pages: changes are pending, written by
Save Changes (or at once when Auto Save Options is on), and restored by cancel together with the game's. Find
a zero-upstream-line way (the existing options/settings hooks: change, activate, settings write). If none
exists, STOP on this item and describe the one hook line needed; do not add it.

**H4. An existing test was made vacuous.**
`fork_ext/addons/round_3_picture_tests.rs:1028-1060`
(`fresh_hud_mod_load_defaults_and_live_stats_key_and_menu_wiring`): both `eqoa_stats_box` and `editor_probe`
moved from f8 to f10; under rule 3 the probe's default is refused, so it starts unbound and
`assert(other_hits==0)` passes even if `capture_text` suppression were broken.
DECISION: restore what the test proved (two VMs really bound, text capture suppresses the other's key), e.g.
distinct free keys with the probe's key pressed, plus a negative control. Re-audit every one of the ~95 moved
fixtures for the same effect (a binding that is now refused, so the assertion no longer exercises anything).

## MEDIUM

**M5. Disable/re-enable and Defaults for disabled mods.** `fork_ext/bindpage.rs:38-47`, `76-83`, `222-236`;
`addons/manager.rs:329`, `843`. (1) A disabled mod keeps its unsaved default in `MOD_BINDS`; if something takes
that key meanwhile, re-enabling does not re-check: it comes back in conflict (rules 3, 5). (2)
`reset_defaults()` checks `default_in_use` only for enabled mods and writes a saved default for disabled ones
even when the key is in use. (3) It iterates in registration order and saves an empty entry for a blocked
default, which clears `refused` after a restart, so the row no longer says why it is empty.
**M6. Two unsaved mods with the same default** are settled by registration order, not `(addon_id, id)`; and
an unsaved default loaded before a mod with that key saved gives a different result from the reverse order.
DECISION (M5, M6): one rule everywhere, independent of load and registration order: saved beats default, then
`(addon_id, id)`; re-check on enable; Defaults obeys rule 3 for every mod; a refused default keeps its reason
across restarts.
**M7. Keys the client consumes before the game's key handling.** `viewer.rs:5795-5814`, `4167-4231`,
`5925-5927`: Escape (deselect chat lines, close an item-link popup) and Ctrl+C (chat copy) are handled before
`KeyEdge`, after mod dispatch. A mod on Escape fires as well; a mod on C fires on Ctrl+C.
DECISION: Ctrl+C is solved by H1. Escape joins the keys a mod can never have (same treatment as H2).
**M8. Report claims the code does not support**: "default never takes a key in use / Defaults same rule" (M5),
"one press does one thing" (H1, H2), "test data only" (H4; also `api_capture_tests.rs`, `items_tests.rs`,
`r6d_tests.rs`, `settings.rs` were edited), and "tests drive the real dispatch": they drive `ControlsPage` and
`KeyEdge` but never `App::window_event`/`key_input`/`key_name`/`addon_event`, which is why H1, H2, M7 were
missed.

## LOW

**L9.** `fork_ext/bindpage.rs:544-548`, `632-635`: when `apply_mod_keys` fails (unwritable `mod-keys.opt`) the
capture stays open and the "Key not kept" note is hidden behind the "Press the key…" prompt. Fix.
**L10.** The Macros page (`macropage.rs:441-452`, upstream) accepts a trigger on a mod's key with no note; the
macro wins at press time and Controls marks the mod row. DECISION: accept (upstream file); say so in the report
and the player note.
**L11.** Capturing on a mod row shows "Press the key for . Esc gives up." (`bindpage.rs:393-398`, `444-446`,
upstream draws it; the label for mod rows comes back empty). Fix only if the label comes from fork code.

## Reviewer's own "not checked"
Nothing built or run; negative controls not confirmed; the three test files `api_capture_tests.rs`,
`items_tests.rs`, `r6d_tests.rs` checked only in their changed hunks (all `"k"` → `"u"`).
