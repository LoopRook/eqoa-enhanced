# Round 6 Part A recheck

**Verdict: PASS WITH NOTES.** All six findings (H1, H2, M3-M6) are fixed in `0c9d3785`. None blocks. The notes are test-coverage gaps, one stale line in the Part A report, and two assumptions about M4 that the tests do not check.

Range checked: `git diff 8ff69945..0c9d3785`. Gates run on `0c9d3785` (read-only checker; only this report was added).

## Per finding

| Finding | Verdict | Where (file:line at 0c9d3785) | Evidence |
|---|---|---|---|
| H1 Invented `GetDispositionIconTexture` | FIXED | `addons/visibility.rs` (export removed, former line ~585); `addons/native.rs:330-331` (capability rows) | `GetDispositionIconTexture` now appears only in `r6a_tests.rs:798, 840-841` (asserts it is nil and its capability is `unknown`) and in the review and Part A report text. It is absent from every frontiers_forge export and from every capability row. The pinned `ui.lua` helpers `GetDispositionFaceTexId` (line 469) and `GetClassIconTexId` (line 521) exist; the test constants match (hostile 0x73, neutral 0x74, friendly 0x75; class 14 = 0xFB). |
| H2 `parse_hint` panics on multi-byte after `\|` | FIXED | `addons/native_r6a.rs:63-86` (`text[i + 1..].chars().next()`, escape length from `len_utf8`); `addons/host.rs:227-236` (Forge `ParseButtonHint` uses `encode_utf8`) | Isolated negative control below. |
| M3 Forge hint table wrong | FIXED | `addons/host.rs:191-200` (table), `:201-213` (Format), `:216-240` (Parse) | Table is exactly `window.lua:830-838` (`x,t,s,c,8,-,+`). Format returns the label when the button is unknown, as `window.lua:921-927` does. Parse returns `(nil, rest)` for an unknown escape, and `(nil, text)` when there is no escape, as `window.lua:908-918` does. `dpad_left` gives `\|-`, `\|+Next` gives `dpad_right`. Native glyphs stay in `eqoa.windows.*`. Nit, not a finding: Lua's `.` matches one byte, Rust's matches one char. For a non-ASCII escape both return a nil button, but the label bytes differ. |
| M4 Compass from the wrong source | FIXED, with an assumption | `addons/host.rs:278-296` (raw `camera_orbit_x`, `nil` without a camera, no heading fallback); `r6a_capture.rs:1070, 1075, 1207` set or clear it | `util.lua:563-571` reads the raw value at `0x1FB66AC`, and `Degrees` normalises to `[0,360)` with 0 as the default. The Rust code matches that shape. Assumption: the Rust value is the walker's `cam.cur.orbit.x`, and the tests only check that `Util` equals `Camera` in Rust. Nothing checks that `orbit.x` is the same value that `0x1FB66AC` holds in the game. |
| M5 Stale window rows after item change | FIXED, with coverage gap | `r6a_capture.rs:220-228` (merchant: mode 1 hashes inventory, mode 2 hashes wares), `:229-239` (bank), `:196-208` (loot/inventory), `:853-858` (`hash_window_items` hashes id, name, quantity) | New tests `round6a_fix_m5_merchant_and_bank_item_mutation_fires_windows_changed` cover bank (mode 11) and merchant buy (mode 2). Not covered: merchant sell (mode 1), corpse loot, and the items window (mode 13). |
| M6 Quest stamp ignored when log is full | FIXED | `api_updates.rs:916` (event fingerprint), `r6a_capture.rs:862` (`quests_hash`) | `round6a_fix_m6_quests_full_log_stamp_change_fires_and_stales` covers a full 8-quest log. It checks that `quests.changed` fires and that the old ref is `stale_handle`. |

**Stale text (not a finding against the fix):** `docs/plans/eqoa-api/round-6a-report.md:24` still lists `frontiers_forge.ui.GetDispositionIconTexture` in the original Part A table. The Fix round table supersedes it, but the first table should be updated so the report does not claim the export exists.

## Test edits in the range (step 2)

Only `r6a_tests.rs` has test edits. `native_tests.rs`, `items_tests.rs`, and `player.rs` are not touched in this range. Tests that already existed at `8ff69945` are:

1. `round6a_camera_third_and_first_person_and_tick_coalescing`, third person (around `r6a_tests.rs:493-496`). The Forge `Util` assertions changed from "equals native compass" and "equals 0" to "equals -0.5" and "equals camera degrees". **Not weakening.** The assertions are stricter, and the native `eqoa.camera.compass` check is still there.
2. Same test, first person (`r6a_tests.rs:520`). Added a setup line `cur.orbit.x = FRAC_PI_2`. **Neutral, with a caveat.** The Forge check now passes because of the setup, not because of the eye direction. The native compass assertion is still eye-based. Setting the orbit explicitly is the right approach for M4.
3. Same test, appended block (`:535-547`). The M4 nil case is a new addition. **Strengthens.**
4. `round6a_windows_modes_dialogs_hints_and_outside_world` (around `:575-600`). Only added lines (Forge dpad hints, `ParseButtonHint` cases, the H2 UTF-8 block). **Strengthens.**
5. `round6a_forge_icons_merchant_hint_hotbar_items_and_zero_addon` (around `:794-802`). Removed the `GetDispositionIconTexture` texture, size, and nil assertions, and replaced them with `GetDispositionFaceTexId` and `GetClassIconTexId` id checks. **Justified, with a caveat.** The removed function was invented, so its assertions had nothing to test. A side effect is that no test now checks a Forge disposition texture's size or draw. The native `eqoa.icons.disposition` path is not in this diff and is unchanged.
6. Same test, capability section (around `:840-843`). Only added lines. **Strengthens.**

No test that existed before `8ff69945` was deleted outright. The only real removal is item 5, and it is for a removed API.

## Gates (step 3)

- `python tools/run_gates.py --only round --filter round6a_ --lock-wait 120` on `0c9d3785`: **PASS**, 10 passed, 0 failed, 0 ignored (143.23 s). The review saw 8 earlier; the difference is the new M5 and M6 tests.
- Skips: none reported by the gate summary. The gate does not list skip counts for this filter.

## H2 isolated negative control (step 4)

- Reverted only `crates/eqoa/src/fork_ext/addons/native_r6a.rs` to its `8ff69945` content (the whole-file diff for that path is the H2 change). Kept the M3 fixes in `host.rs`.
- `--filter round6a_windows_modes_dialogs_hints_and_outside_world`: **FAIL**, 0 passed, 1 failed. Failure reproduced, which shows the test catches the H2 panic.
- Gap: the gate summary does not give the panic line, so this report does not confirm that the failure is the `'|é'` assertion rather than an earlier one. The M3 assertions run before the H2 block and pass with the M3 fix in place, so the H2 block is the next thing that can fail.
- Restored with `git checkout -- crates/eqoa/src/fork_ext/addons/native_r6a.rs`. `git status` is clean; `git diff` is empty.

## Open notes

1. Add tests for merchant sell (mode 1), corpse loot, and the items window (mode 13) so M5 covers every row source the fingerprint hashes.
2. Update `round6a-report.md:24` to drop `GetDispositionIconTexture`.
3. Consider a test that pins M4's assumption, or record in the report that `orbit.x` equals the raw `0x1FB66AC` only by construction and has not been checked against the game.
4. Make the H2 negative control print the panic line when the gate runs with a filter, so the next recheck can confirm the exact failure.
