# Round 3 code review 2 (`feat/eqoa-api-r3`, head 5ab52e5)

**Verdict: APPROVE WITH CHANGES.** The four mods now match develop's native drawing, defaults, placement and click targets closely, and most earlier findings are fixed in the code. One real player-facing bug remains (a rebound mod key is lost when the Controls page next saves a native key). Three MEDIUM issues and several LOW ones follow.

Scope and limits: I read the code (`git diff develop...HEAD`, the four Lua mods, `hud_layer.rs`, `hud_layout.rs`, `options.rs`, `bindpage.rs`, `settings.rs`, `native_state.rs`, `fork_ext/play.rs`, the picture-test helpers, the spec diff and the footprint diff). I did not build or run tests; test results come from the checkers' reports. Findings below were verified in code, not by running the game. I did not read `viewer.rs`'s new hooks or `report_viewer.rs` in depth, so H5's frame-ordering fix rests on the report plus the diff (hook calls present; the old inline `forge_runtime.draw` removed).

## Earlier findings

| Finding | Status | Evidence |
|---|---|---|
| H1 option storage under real addon id | Fixed | `native_state.rs` options.set/get/toggle use `session.addon`; `options.rs:195-240` persists under the caller's id; `settings.rs:103-130` reads legacy keys into the real ids. |
| H2 disabled mod keeps rows | Fixed | `options.rs:158` `set_addon_enabled` flips `enabled`; `active_mod_items` filters on it; `bindpage.rs:80,86,104` filter key rows on `enabled`. |
| H3 defaults | Fixed | `eqoa_stats_box/main.lua:12` false, `eqoa_target_of_target/main.lua:10` false, mana_bars true, three numbers true. Same as develop's `ClientOptions`. |
| H4 B key / rebinds | Partly | B default and the Controls row work (`bindpage.rs`, `main.lua:14`). A rebind is lost after a native save: see NEW-H1. |
| H5 anchor and frame order | Fixed (diff only) | Native-pass `fork_hud::draw` calls `hud_layer::draw` (`fork_ext/hud.rs:261`); Left/Top entry state restored; `viewer.rs` calls `prepare_addons` / `submit_addons`. |
| H6 click rects | Fixed | Both Lua click windows call `Begin`, then `InvisibleButton`, then unconditional `End`; position uses `frame.offset/scale/side/top` with the sign matching `transform_point` (`hud_layout.rs:94`). |
| H7 group list | Fixed | Lua mana bars use only `grp.members` and `visible("group")` (false in a raid, `hud_layout.rs:196`). Capture filter and test live in `api_capture*.rs` (not re-read). |
| H8 ToT self/nameless/mana | Fixed | `target_of_target/main.lua:20-27,41`. |
| H9 visibility | Partly (code seen, live gating not read) | `hud_layer::draw` clears on Hide UI; Lua checks `hud.visible(...)`; `visible` = `right_hud_up` (`fork_ext/play.rs:36-42`). Menu/map/loading come only through that one flag. |
| H10 Stats Box look | Fixed | Frame `{200,54,148,132}` equals develop's `FRAME [200,54,348,186]`; Left/Top as develop's `statbox::draw`; labels via `eqoa.text.localized`. |
| H11 public API | Partly | Functions exist and are in the spec table. `eqoa.input.bind` and `eqoa.binds.get` are not documented; see NEW-M4. |
| H12 picture tests | Fixed with caveats | `round_3_picture_tests.rs:254` reads the PNG; per-element boxes, tolerance 2/255, 0.1% per box (`:269`). Caveats in NEW-L4. |
| M1 upstream footprint | Fixed | `hud.rs` equals `5d54c0c` in test order and text. `binds.rs`/`bindpage.rs` restore upstream wording. Only budget raised vs develop: bindpage.rs 7→10 added (+3, approved). Historical assertion differences (F2→F10, B→Y) are develop's, not this branch's. |
| M2 HUD numbers | Fixed | `hud_numbers/main.lua` positions 99/35, 99/45, 99/55, 60/397, scales, colours and 50/20 thresholds equal develop's `readouts()`. |
| M3 assist/summary | Fixed | `fork_ext/play.rs:55-78`: assist in modes 0/38, summary in mode 0 with map/search/typing/death gates. |
| M4 per-frame allocation | Partly | Queues are drained and reused, but each `Text` command owns a `String`: NEW-L3. |
| M5 layout offsets | Fixed | `fork_ext/play.rs:24-50` sets pet Y/height from group, raid and effects. |
| M6 multibox/write errors | Fixed | `settings.rs:132-195` loops `PLACES`, writes the mod store first and strips only on success. `bindpage::save_mod_bind` strips `key.stats_box=` in the same write that adds the mod key. |
| M7 `hud.visible` names | Fixed | Unknown names return false (`hud_layout.rs:222`). |
| L1 `trainer_from_hud` | Not a defect | Still used by the Summary close path (`play.rs:6410,6693`). |
| L2/L3 spec text | Fixed | `shadow` and `ui_id=25` are documented. |
| Check-2 upstream test changes | Not caused by this branch | `git diff 5d54c0c HEAD` on `hud.rs` shows no test changes. |

## New findings

### HIGH

**NEW-H1. A rebound mod key is lost the next time a native key is saved.**
`crates/eqoa/src/settings.rs:435-441` (`save_binds`) overwrites the whole of `keys.opt` with `Binds::to_text()`. `Binds` knows nothing about `mod.<addon>.<id>=` lines (`fork_ext/bindpage.rs:242-265` writes them straight into the same file; no hook preserves them). Whenever the player changes any native row and presses Save, the `mod.eqoa_stats_box.toggle=...` line is dropped. The in-memory key stays until restart, then reverts to B. The test at `bindpage.rs:327` only checks that the mod save keeps native lines, not the reverse.
Player scenario: rebind Stats Box from B to F8 (works). Later rebind Jump and Save. Restart: Stats Box is on B again and F8 does nothing. The same loss hits any third-party mod's keys.
Fix direction: have the save hook re-append the mod lines, or store mod keys in their own file.

### MEDIUM

**NEW-M1. The client now always asks the server for others' mana, even with Mana Bars off or the mod disabled.**
`crates/eqoa/src/fork_ext/play.rs:19` calls `agree_power_bars(true)` unconditionally. Develop used `agree_power_bars(client().mana_bars)`, and `Session::agree_power_bars(false)` clears the held set (`crates/net/src/session.rs:1374`). Result: a player who turned Mana Bars off, or disabled the mod, still receives the extra power-bar traffic, which is a behaviour and bandwidth change from develop. Fix: drive it from the `mana_bars` toggle (or from `eqoa.group`/`power` consumers).

**NEW-M2. Mod keys have no conflict handling against native rows or other mods.**
`bindpage.rs:171-195` (`capture_key`) stores any key, with no swap or free as native rows do (compare upstream `bind_key` → `Moved::Freed/Swapped`). `dispatch_keypress` and the Lua `input.key` event do not suppress the native action. Develop had Stats Box as a normal row, so binding Jump to B freed it. Now B is not a native row, so upstream's own allowed "Jump on B" and the mod's default B both fire together; the same for rebinding the mod key onto an in-use key. Player scenario: Jump on B (allowed on the Controls page): every jump also toggles the Stats Box.

**NEW-M3. Failed saves are silent.**
`options.rs` `toggle_by_item` and `set_toggle` ignore `persist_toggle` errors (`let _ =`), and `eqoa.options.set` returns `true`. `native_state.rs:906-915` (`toggle_stats_box`) prints "stats box shown" regardless. Develop's `set_stats_box` returned the save result. Player scenario: read-only or full config folder: the switch looks on, the setting reverts at restart, with no message. (The migration path does handle write failures correctly.)

**NEW-M4. The new public API has contract gaps.**
- `eqoa.input.bind` is registered (`native.rs:81`, `native_state.rs:1011`) and used by the bundled Stats Box but is not in `EQOA-API-SPEC.md` (only `eqoa.binds.register` is). `eqoa.binds.get` is also undocumented. The bundled manifests declare capability `eqoa.binds.register` while calling `input.bind`.
- Production code carries test shims in the public functions: `native_state.rs` (`binds.register`, `binds.get`, `input.bind`) rewrite caller ids `""`/`"test"`/`"mod"` plus `id=="toggle"` plus label `"Toggle Stats Box"` to `eqoa_stats_box`; `options.rs:76-87` `resolve_addon_id` rewrites `""`/`"mod"`/`"test"` to a bundled mod by toggle id. A third-party mod with such an id and those names is silently redirected.
- `eqoa.actions.toggle_stats_box()` is a feature-named function in a stable API; called from another mod it creates a stray `stats_box` toggle under that mod's id.
Fix direction: document `input.bind`/`binds.get`, declare matching capabilities, move the shims into test-only code.

### LOW

**NEW-L1. Target of Target does not fade in with the target plate.** Native used `self.glow0` as alpha for the bar and name (`hud.rs:524,530`; glow rises 0.3→1.0 on a new target). The mod draws alpha 1.0 (`hud_layer.rs` image default; Lua passes no colour). Visual difference of a few frames on target change. The pictures cannot catch it because the capture forces `hud.glow0=1.0` (`docs/round3-captures/native_capture.rs:170`).

**NEW-L2. ToT click height is hard-coded to 26.** Native used the art entry's height, `vui.entry(26).uv1[1]` (develop `hud.rs:543`); the test atlas uses 28. With real art the clickable bar may be a few pixels off from the drawn bar. Not checkable here (no disc art).

**NEW-L3. One `String` allocation per queued text command per frame** (`HudCommand::Text.text`), so the "reusable buffers" claim covers the Vec, not the strings. The benchmark (0.368 ms with four mods) still passes the unchanged 0.5 ms limit.

**NEW-L4. Picture-test coverage limits.** References use synthetic assets (1×1 white texture, solid glyphs), only 4:3 and 16:9 (tall windows are covered by layout tests, not pictures), and no fade state. Without a GPU they print SKIP (four tests).

**NEW-L5. ToT click is gated on `visible("stats_box")`** (`target_of_target/main.lua:46`), which is `right_hud_up`. Same value as the drawing gate today, but the name is misleading and would break if the two flags ever diverge.

## Points checked and found correct
- Stats Box/ToT/Mana/Numbers defaults, positions, colours and thresholds match develop's native code.
- Migration order (mod store first, strip after) holds for client files of every multibox place; the legacy `key.stats_box` is stripped atomically with the mod key write.
- `hud.rs`, `binds.rs`, `bindpage.rs` upstream lines are restored to `5d54c0c` wording; no reordering.
- HUD layer: per-mod cap 256, `drain` each frame, anchor/vertical restored after drawing, `Begin`/`End` paired.
