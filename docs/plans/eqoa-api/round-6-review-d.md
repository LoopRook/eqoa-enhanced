# Round 6 Window/Quest Actions Review (diff `da03af17..43edb0d9`)

**Verdict: REJECT** — 4 HIGH and 4 MEDIUM defects across row/dialog mapping, stale page/sub-dialog gating, Class Mastery node state, edit-box handle validation, `close_window` locking, `drop_quest` stamp checks, and queue-time report-confirmation gating.

## Findings

### HIGH

1. **Bank pages and modal window sub-states are ignored by `live()` and `signature()`, allowing `select_row`/`activate_row`/`menu_command` to act on stale pages or hidden dialogs (`crates/eqoa/src/fork_ext/r6a_capture.rs:229-238,562-572`, `crates/eqoa/src/fork_ext/addons/window_actions.rs:28-50`, `crates/eqoa/src/fork_ext/play.rs:2188-2214`).**
   - `Bank` (`crates/eqoa/src/bank.rs:163,174-215,269-369`) has six pages (`Menu`, `MoneyMenu`, `Deposit`, `Items`, `Quantity`, `Auction`), opening on `Page::Menu` with `inv.x64 == false` and `bank.x64 == false`. However, `r6a_capture.rs:562-572` ignores `Bank::page` and unconditionally exports `play.dump.bank.items.items` as `rows` with `bank_list.sel + 1` as `selected_row`; `windows_fingerprint` (`r6a_capture.rs:229-238`), `window_actions::signature` (`window_actions.rs:28-50`), and `live()` (`fork_ext/play.rs:2188-2214`) never track `Bank::page`.
   - Likewise, `live()` (`fork_ext/play.rs:2207-2209`) only checks `play.mode == 40 && play.cm.node.is_some()`. It ignores active sub-dialogs/popups in other modes where `play.mode` and the list `rows`/`selected_row` stay unchanged:
     - Merchant (`mode == 10`): `Merchant::focus` in `Focus::Quantity | Focus::MBox | Focus::Inspect` (`crates/eqoa/src/merchant.rs:172-180,325-427`).
     - Spellbook (`mode == 5`): `popup.visible`, `inspect.visible`, `inspect_due`, or `mode != 0` (`crates/eqoa/src/spellbook.rs:365-436`).
     - Quest Log (`mode == 22`): `QuestLog::boxed == true` (`crates/eqoa/src/questlog.rs:184-202`).
     - Loot (`mode == 1`): `Loot::ml.busy()` (`crates/eqoa/src/loot.rs:133-151`).
     - Class Mastery (`mode == 40`): `ClassMastery::focus == Focus::Points` (`crates/eqoa/src/classmastery.rs:839-880`, where `play.cm.node` is `None`).
   - **Failure scenario:** Player opens the bank (`mode == 11`, showing `Page::Menu` on "Money"). A mod reads `w = eqoa.windows.find("bank")`, sees bank item #2 in `w.rows` and `selected_row = 1`, and calls `eqoa.actions.activate_row(w.ref, 2)`. `dispatch_window_action` sends `[139 (Down), 134 (Cross)]`, entering `Page::Items` from `Page::Menu` instead of acting on bank item #2, and reports `state = "completed", ok = true`. Similarly, in Merchant (`mode == 10`) with a sell-confirmation `MBox` or `Quantity` picker open, `activate_row(w.ref, 2)` sends `Down + Cross` straight into the `MBox` or `Quantity` picker while reporting `completed`.

2. **Trainer / Character Summary (`mode == 12`) `select_row`/`activate_row`/`menu_command` lands on the wrong row when walking up from `Train` or `Details` (`crates/eqoa/src/fork_ext/r6a_capture.rs:573-601`, `crates/eqoa/src/fork_ext/play.rs:2216-2224,2304-2346`, `crates/eqoa/src/trainer.rs:335-347`).**
   - `r6a_capture.rs:585-590` maps `t_mode == 0` to `sel + 1` (`1..=7`), `t_mode == 1` (`Train`) to `selected_row = 8`, and `t_mode == 2` (`Details`) to `selected_row = 9`. `walk()` (`fork_ext/play.rs:2216-2224`) assumes every `Up (138)` press decrements `selected_row` by 1.
   - In `trainer.rs:335-347`, pressing `Up (138)` from `mode == 1` (`Train`) restores `self.mode = 0` with `self.sel` kept at its **prior** stat (`0..=6`, initially `0` = Strength), not `6` (Charisma).
   - **Failure scenario:** Player opens Character Summary (`mode == 12`, `t_mode = 0, sel = 0`) and tabs down to `Train` (`t_mode = 1, selected_row = 8`). Mod calls `eqoa.actions.select_row(w.ref, 5)` (Wisdom). `walk(Some(8), 5, &[])` sends 3 `Up` keys (`138, 138, 138`): the first returns to `t_mode = 0, sel = 0` (Strength) and the remaining two clamp at `sel = 0`. The window lands on row 1 (Strength) instead of row 5 (Wisdom), while `ActionResult` reports `completed` (`ok = true`).

3. **Stale `play.cm.node` in Class Mastery (`mode == 40`) breaks `mastery_buy`/`mastery_refund` gates and permanently locks `mastery` window actions (`crates/eqoa/src/fork_ext/play.rs:2207-2209,2269-2295`, `crates/eqoa/src/classmastery.rs:299-315,935-938`).**
   - `Part::recv` for sub-ops `5`, `6`, and `10` (`classmastery.rs:299-315`) updates `self.skills[k].on`, not `self.node`. `ClassMastery::key` on `Escape`/`Triangle` (`classmastery.rs:935-938`) sets `self.focus = Focus::Main` without clearing `part.node`.
   - `MasteryBuy`/`MasteryRefund` (`fork_ext/play.rs:2269-2288`) only checks `play.cm.node` (`x04`, `x05`), never clears `play.cm.node`, never returns `cm_wnd.focus` to `Main`, and never cross-checks `play.cm.skills` (`on` / `is_known`). Meanwhile `live()` (`fork_ext/play.rs:2207-2209`) rejects any window op on `mode == 40` whenever `play.cm.node.is_some()`.
   - **Failure scenarios:**
     1. Mod calls `mastery_browse(100)` (`node.x04 == 0, node.x05 == 1`) then `mastery_buy(100)`. After the server confirms purchase (`skills[k].on = true`, `pending = 0`), `play.cm.node` still has `x04 == 0`. Calling `mastery_buy(100)` again without re-browsing passes the `node.x04 == 0` gate and sends a duplicate `send02(100)` buy request, while `mastery_refund(100)` is wrongly refused (`"the skill is not owned"`).
     2. Once any skill is browsed in `mode == 40`, even after the player closes the Inspect/Require window back to `Focus::Main`, `play.cm.node.is_some()` stays `true`, permanently blocking `select_row`, `activate_row`, and `close_window` on `mastery` with `busy` (`"a skill window is open over the list"`).

4. **`input_text` signature omits `tag`, `title`, `chat`, and `password`, allowing edits to land in a replaced edit box (`crates/eqoa/src/fork_ext/addons/window_actions.rs:28-50`, `crates/eqoa/src/fork_ext/addons/native_core.rs:515-518`).**
   - In `window_actions::signature` (`window_actions.rs:37-50`), when `w["dialog"]["kind"] == "input"`, `dialog_text` is forced to `Json::Null` and `tag`, `title`, `chat`, and `password` are omitted, so every `mode == 2` edit box produces the identical signature.
   - `assign_window_refs` (`native_core.rs:515-518`) only allocates a new `incarnation` if `"mode:2"` was absent when `r6a::refresh` ran.
   - **Failure scenario:** Chat edit box (`mode == 2`, `purpose == 0x0B0F_0000`) is open; mod captures `w.ref` and calls `input_text(w.ref, "Bob", true)` inside a gesture. Before dispatch (or in the same tick if `mode:2` closes and another `open_edit` — e.g. `0x0B04_0000` Add Buddy or `0x0B05_3001` Ignore — opens without an intervening `r6a::refresh`), `"mode:2"` retains the same `incarnation` and identical `signature(&w)`. `live()` (`fork_ext/play.rs:2200-2203`) accepts the handle and submits `"Bob"` into the Add Buddy / Ignore dialog instead of failing `stale_handle`.

### MEDIUM

5. **`dialog_answer(ref, 1)` on `text_dialog` (`mode == 9`, `PrivChat`) is rejected instead of closing the dialog (`crates/eqoa/src/fork_ext/addons/window_actions.rs:420-431`, `crates/eqoa/src/fork_ext/play.rs:2352-2369`).**
   - Contract (`docs/modding/EQOA-API-SPEC.md:560,648`): Done-only `PrivChat` (`mode == 9`) has `options = []` and `dialog_answer(ref, 1)` invokes its Done action (`play.rs:969-974`, `Cross`/`134`).
   - `window_actions.rs:426-431` reads `options.len() == 0` and rejects with `out_of_range` (`"dialog has no options"`); `fork_ext/play.rs:2352-2369` also rejects `(9, Some("text"))` with `unsupported` (`"this dialog takes no answer"`).
   - **Failure scenario:** An NPC opens `PrivChat` (`mode == 9`). Calling `eqoa.actions.dialog_answer(eqoa.windows.find("text_dialog").ref, 1)` returns `nil, Error{code="out_of_range"}` instead of closing the dialog.

6. **`close_window` ignores pending native locks and reports `completed` on modes/states where `Triangle (132)` does nothing (`crates/eqoa/src/fork_ext/play.rs:2297-2303`).**
   - Contract (`docs/modding/EQOA-API-SPEC.md:645,654`): `close_window` must respect pending native locks and only report applied when the native path executes.
   - `fork_ext/play.rs:2297-2303` unconditionally calls `play.press(&[132])` and returns `DispatchOutcome::Completed` without checking `play.bank.state != 0`, `play.npcw.shop.pending`, `play.waiting`, `play.cm.pending != 0`, or whether `window_key` handles `132` for that mode (`play.rs:6541-6567`).
   - **Failure scenarios:** (a) Calling `close_window` while a bank (`bank.state != 0`) or merchant (`shop.pending`) transaction is pending sends `132` instead of refusing `busy`. (b) Calling `close_window` on `mode == 7` when `play.hud.is_none()` (`play.rs:1006`) or on a mode unhandled by `other_window_key` (`play.rs:6541-6567`) does nothing natively yet reports `state = "completed", ok = true`.

7. **`drop_quest` does not capture/verify `play.quests.stamp` directly and relies on deferred `quest_revision` (`crates/eqoa/src/fork_ext/addons/action_broker.rs:140`, `crates/eqoa/src/fork_ext/r6a_capture.rs:1092-1122`, `crates/eqoa/src/fork_ext/play.rs:2226-2239`).**
   - `ActionKind::DropQuest` stores `{ index, title, revision }` without `stamp`. `capture_quests` (`r6a_capture.rs:1099`) bumps `CORE.quest_revision` inside `native_core::defer(...)`, which only runs on `flush_deferred()` (during render/`refresh`), not during `begin_iteration` (`fork_ext/play.rs:321-423`).
   - **Failure scenario:** In a multi-tick frame (two `begin_iteration(true)` game steps without an intervening render/snapshot), `session.pump` processes `0x007D` (updating `play.quests.stamp` from `7` to `8`, e.g., with an out-of-range index or followed by `0x007C` with the same title at `index`). Because `capture_quests`'s deferred revision bump has not flushed before the next `begin_iteration(true)`, `CORE.quest_revision == *revision` and `now_title == title` still pass, sending `0x007D` with the unseen stamp `8`.

8. **Report-confirmation dialog (`mode == 14, mbox.p2 == 0x0B0B_1002`) is accepted at queue time and only rejected at dispatch (`crates/eqoa/src/fork_ext/r6a_capture.rs:616-622`, `crates/eqoa/src/fork_ext/addons/window_actions.rs:62-78`).**
   - Contract (`docs/modding/EQOA-API-SPEC.md:656`): synthetic report guard must reject "before queue acceptance and again at dispatch ... when the relevant native support/report editor or confirmation owns the input".
   - Because `confirm` JSON (`r6a_capture.rs:616-622`) omits `tag`/`p2`, `window_actions::report_guard` cannot identify a report confirmation at queue time: `dialog_answer` / `close_window` consumes the gesture and returns a `RequestId`, and is only refused at dispatch by `fork_ext/play.rs:2173-2186` (cannot submit the report, but fails the queue-time rejection requirement).

## Checklist & Scope Notes

- **Password fields:** Rejected at both queue time (`window_actions.rs:446-448`) and dispatch (`fork_ext/play.rs:2386-2388`).
- **Response dialogs (`mode == 7`, page 2+ More/Back/Close):** Page-local 1-based `selected_row` and `dialog.options` align with `Response::read_state()` (`r6a_capture.rs:506-528`, `dialog.rs:169-245`, `fork_ext/play.rs:2353`).
- **`menu_command` disabled rows:** Rejected by `x["enabled"] == true` at both queue time (`window_actions.rs:476-479`) and dispatch (`fork_ext/play.rs:2335`).
- **Gesture gate:** All 11 new functions route through `enqueue_lua_with` (`window_actions.rs:305-489`, `action_broker.rs:2514-2536`).
- **Packet building / direct writes / upstream files:** No upstream files touched in `da03af17..43edb0d9`; actions use `Quests::send_remove`, `classmastery::Part` methods, `Play::press`, and `EditBox` + `Play::edit_done`.
- **Edited pre-existing tests:**
  - `crates/eqoa/src/fork_ext/addons/action_broker.rs:4605-4616` (`round6b_capabilities_and_descriptors_cover_actions_result_and_events`): removed `drop_quest`, `mastery_share`, `close_window`, `select_row`, `menu_command` from the `not_yet` list now that they are `supported` (covered as `supported` in `r6d_tests.rs:298-311`). **Not weakened.**
  - `crates/eqoa/src/fork_ext/addons/native_tests.rs:11-21` (`native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught`): switched the `not_yet` stub probe from `eqoa.actions.cast` to `eqoa.actions.move`. **Not weakened.**
  - `crates/eqoa/src/fork_ext/api_capture_tests.rs:2170-2171` (`round6b_non_item_chat_command_windows_weapon_sets_and_spellbook`): updated `eqoa.actions.drop_quest` expectation from `not_yet` to `supported`. **Not weakened.**
