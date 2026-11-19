# Round 6.5 Upstream Seams (Window Takeover)

Total upstream lines added: **4** inserted one-line hook calls across 2 files (`0` upstream lines deleted or rewritten; pre-approved budget up to 6).

| File | Location | Inserted line | Reason |
|---|---|---|---|
| `crates/eqoa/src/npcw.rs` | `0x0018` handler (`self.mode == 1`) | `if self.mode == 1 && super::fork_hooks::window_taken(self, 1) { self.npcw.loot_open = false; self.mode = 0; }` | When a mod holds `eqoa.windows.takeover("loot")`, keeps `self.npcw.corpse` live while returning `mode` to `0` so the player can walk and native loot window neither draws nor steals input. |
| `crates/eqoa/src/npcw.rs` | `0x00B7` handler (`self.mode == 10`) | `if self.mode == 10 && super::fork_hooks::window_taken(self, 10) { self.npcw.merchant_open = false; self.mode = 0; }` | When a mod holds `eqoa.windows.takeover("merchant")`, keeps `self.npcw.shop` live while returning `mode` to `0` so the native merchant window neither draws nor steals input. |
| `crates/eqoa/src/bankplay.rs` | `0x124D` handler (`self.mode == 11`) | `if self.mode == 11 && super::fork_hooks::window_taken(self, 11) { self.bank.open = false; self.mode = 0; }` | When a mod holds `eqoa.windows.takeover("bank")`, records the hidden banker session (`HIDDEN_SESSIONS.banker_id = self.target`) while returning `mode` to `0`. |
| `crates/eqoa/src/bankplay.rs` | `Play::run_bank` entry (before `if self.mode != 11`) | `if self.mode != 11 && super::fork_hooks::bank_taken_tick(self, vui) { return; }` | Processes staged `bank_move` / `bank_money` actions through native `run_bank` (`0x1254` / `0x1255`) using a no-op `Raster` when `mode == 0` under bank takeover, without drawing any UI. |

`docs/fork-footprint.json` was updated accordingly (`crates/eqoa/src/npcw.rs`: `+8 -1`, `crates/eqoa/src/bankplay.rs`: `+7 -0`, plus hook pattern counts).

## Review-fix round (M1-M3, L4-L7)

No upstream line was added or changed: the total stays **4 of 6** inserted lines. L4 (never hide a window that was already open natively) is decided fork-side: `fork_ext/play.rs` records `Play::mode` as each game event begins (`begin_game_event`, already called for every event) and `window_taken` declines when the window's mode was already current before the packet, so the existing `npcw.rs`/`bankplay.rs` hook lines needed no edit.

## Owner-test recovery (2026-11-18)

The owner-test fixes add **zero** upstream lines. The total remains **4 of 6**;
the two unused lines remain unused. Merchant requests still call the existing
`Play::interact_with_target`; its flag priority and `0x004C` packet are preserved.
Loot request observation uses the existing `fork_ext::loot_requested` seam.
All request diagnostics, hidden-session matching and cleanup changes live in
fork-owned files. No footprint budget was increased for this recovery.
