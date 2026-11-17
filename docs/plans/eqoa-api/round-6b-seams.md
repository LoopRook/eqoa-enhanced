# Round 6B Upstream Seam Proposals & Approvals

Per `FORK.md` and the Round 6 B/C fix brief (2026-11-16), the lead approved three one-line upstream hook calls under the owner's delegation, and zero other upstream lines were added.

## Approved and Applied One-Line Seams (2026-11-16)

1. **`crates/eqoa/src/npcw.rs` (+1 line before `for o in loot_out {`):**
   ```rust
   super::fork_hooks::npcw_requests(self, &mut loot_out, &mut merch_out);
   ```
   Routes broker loot (`LootOut::One`/`All`) and merchant (`MerchantOut::Buy`/`Sell`) requests through `npcw.rs`'s own packet builders and `shop.pending` state.
2. **`crates/eqoa/src/bankplay.rs` (+1 line before `for o in out {`):**
   ```rust
   super::fork_hooks::bank_requests(self, &mut out);
   ```
   Routes broker bank item (`BankOut::Move`) and money (`BankOut::MoveMoney`) requests through `bankplay.rs`'s own packet builders and `bank.state`.
3. **`crates/eqoa/src/play.rs` (+1 line in `Play::frame` right after `fork_hooks::begin_iteration(self, game_frame);`):**
   ```rust
   fork_hooks::apply_camera(self, walker);
   ```
   Applies one-shot `eqoa.actions.camera` (`WalkCamera` / `camstyle`) setters when queued, and is a no-op otherwise. `eqoa.actions.camera` is now `supported`.

## Read-only accessors that would re-enable more window row actions (proposed, not applied)

Review D showed that row, dialog and close actions are only safe when the window's sub-state is readable. These windows keep it in private fields, so `select_row`, `activate_row`, `menu_command`, `dialog_answer` and `close_window` refuse them with `unsupported` until the lead approves the accessors (each one `&self`, read-only, +3 lines, like round 6 Part A's):

| File | Accessor | Needed for |
|---|---|---|
| `bank.rs` | `pub fn page(&self) -> u8` (the `Page` as a number), `pub fn quantity_up(&self) -> bool` | bank rows only on the items page, no quantity picker |
| `merchant.rs` | `pub fn focus_is_list(&self) -> bool` (`self.focus == Focus::List`) | merchant rows without a quantity picker, MBox or inspect |
| `spellbook.rs` | `pub fn popup_up(&self) -> bool` (`popup.visible \|\| inspect.visible \|\| inspect_due`) | spellbook rows without a popup |
| `questlog.rs` | `pub fn boxed(&self) -> bool` | journal rows without the drop prompt up |
| `classmastery.rs` | `pub fn focus_main(&self) -> bool` (`self.focus == Focus::Main`) | mastery list rows without the points, inspect or requirement window |
