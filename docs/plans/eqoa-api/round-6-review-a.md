# Round 6 Part A review

**Verdict: REJECT** — the Forge compatibility surface, hint parsing, and state/handle invalidation need correction.

## Findings

### HIGH

1. `crates/eqoa/src/fork_ext/addons/visibility.rs:585` exports `frontiers_forge.ui.GetDispositionIconTexture`, which does not exist in the pinned `ui.lua` (the reference exposes `DrawDispositionIcon` at line 486). An addon using this advertised function will fail on Forge. The review brief explicitly classifies invented Forge functions as HIGH.
2. `crates/eqoa/src/fork_ext/addons/native_r6a.rs:65` slices two bytes from a UTF-8 string after `|`. Calling `eqoa.windows.parse_hint('|é')` slices inside the multibyte character and panics instead of returning a result; an addon can trigger a client panic.

### MEDIUM

3. `crates/eqoa/src/fork_ext/addons/host.rs:191` and `:214` use the native button table for Forge hints, but pinned `window.lua:830-837,908-927` defines a different table. For example, formatting `dpad_left` omits `|-`, and parsing `|+Next` fails to return `dpad_right`, breaking Forge addons using those hints.
4. `crates/eqoa/src/fork_ext/addons/host.rs:267` derives Forge `util.GetCompassRadians` from a normalized camera direction (and falls back to body heading), whereas pinned `util.lua:563-565` reads the same raw camera angle as `camera.GetFacingRadians` (`camera.lua:20-21`). Negative angles or an absent camera therefore produce different values from Forge; the fallback can report body heading as camera heading.
5. `crates/eqoa/src/fork_ext/r6a_capture.rs:218` fingerprints merchant state without the merchandise data used to build rows; `:228-232` fingerprints the bank item count rather than item contents. Replacing or renaming an item without changing selection or count leaves `Window.rows` stale and suppresses `windows.changed` until an unrelated window change.
6. `crates/eqoa/src/fork_ext/r6a_capture.rs:842` hashes quest titles but not the server stamp, and `:1075` skips the update when that hash is unchanged. For example, an add while the journal is full updates the stamp (`questlog.rs:46-52`) without changing titles, leaving old QuestRefs valid across a log revision contrary to the contract (`EQOA-API-SPEC.md:38`).

## Scope and existing tests

The footprint check passed against `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`: 486 fork-owned, 42 shared. The ten upstream files contain only the approved read-only accessors; no fields changed visibility, and all +42/-0 budget increases match measured lines.

Pre-existing test edits compared with develop: `addons/native_tests.rs` changes the camera capability expectation to supported and moves the not-yet call, error and log assertions to `actions.cast` (diagnostic coverage retained); `addons/player.rs` changes the three CM assertions from nil to populated fixture values (initial nil covered separately by new tests); `items_tests.rs` changes the merchant hint capability expectation to supported. None of these edits weakens the intended existing coverage. Changes to `r6a_tests.rs` occurred after that file was introduced on this branch, so are not edits to tests pre-existing on develop.

## Independent test results

- `python tools/run_gates.py --only round --filter round6a_`: PASS, 8 passed, 0 failed, 0 ignored (142.58 s). This is the focused gate only; the builder's full suite and release claims were not independently rerun.
- Negative control: changed mastery share from `ee::div(m.x08 as f32, 100.0)` to raw `m.x08 as f32`, then ran `python tools/run_gates.py --only round --filter round6a_mastery_uninitialized_dump_and_reply_subops`. Reproduced: 0 passed, 1 failed; `r6a_tests.rs:282:6`, Lua assertion failed at line 4. Restored and verified no source diff.
- Negative control: removed password redaction from input text, then ran `python tools/run_gates.py --only round --filter round6a_windows_modes_dialogs_hints_and_outside_world`. Reproduced: 0 passed, 1 failed; `r6a_tests.rs:666:6`, Lua assertion failed at line 3. Restored and verified no source diff.
