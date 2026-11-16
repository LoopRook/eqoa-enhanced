# Round 6 Read-Only Upstream Seam Proposal

Status: **Approved by the lead (owner-delegated) 2026-11-16 and applied**. Zero field visibility changes were made in upstream files; every seam is a read-only `&self` accessor:
- `npcw.rs`: kept `loot: Loot` private; added `NpcWindows::loot_lists(&self) -> (&WndList, &WndList)`.
- `bankplay.rs`: kept `wnd: Bank` private; added `BankPlay::read_lists(&self) -> (&WndList, &WndList)`.
- `spellbook.rs`: kept `top: i32` private; added `SpellBook::top(&self) -> i32`.
- `loot.rs`, `bank.rs`: `Loot::read_list(&self)` and `Bank::read_lists(&self)` placed in their existing `impl` blocks.
- `classmastery.rs`: `ClassMastery::read_state(&self)` returns `(column, category, list.sel, list.top, shown)`.
- `dialog.rs`, `questlog.rs`, `startmenu.rs`, `trainer.rs`: applied as read-only `&self` accessors.

Final measured delta across all 10 upstream files against `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`: **+42 / -0 lines newly added** (`44 / 2` total vs prior `2 / 2`).

---

## 1. Dialog contents (`crates/eqoa/src/dialog.rs`) — +9 / -0

`Play.priv_chat`, `Play.response`, and `Play.mbox` live on `Play` and are visible to `fork_ext/play.rs` (`mod fork_hooks`), and `EditBox` already has `pub` fields (`text`, `links`, `title`, `purpose`, `password`). However:
- `PrivChat.text: Vec<u16>` (`dialog.rs:19`) is private (`PrivChat` only exposes `set_text` and `draw`), making `Dialog` in `mode == 9` (`text_dialog`) unreadable.
- `Response.dialog`, `Response.text`, `Response.options`, `Response.sel`, `Response.first`, `Response.page` (`dialog.rs:84-91`) and `Option_` fields (`dialog.rs:62-66`) are private (`Response` only exposes `pub cue`), making `Dialog` text/options/selection in `mode == 7` (`response_dialog`) unreadable.
- `MBox.kind` (`dialog.rs:312`) and `MBox.text` (`dialog.rs:313`) are `pub`, but `MBox.sel: i32` (`dialog.rs:314`) is private, leaving `Dialog.selection` in `mode == 14` (`confirm`) unreachable.

```diff
--- a/crates/eqoa/src/dialog.rs
+++ b/crates/eqoa/src/dialog.rs
@@ -24,6 +24,9 @@
 }
 
 impl PrivChat {
+    pub fn read_text(&self) -> &[u16] {
+        &self.text
+    }
     pub fn set_text(&mut self, s: &[u16]) {
```

```diff
--- a/crates/eqoa/src/dialog.rs
+++ b/crates/eqoa/src/dialog.rs
@@ -108,6 +108,9 @@
 }
 
 impl Response {
+    pub fn read_state(&self) -> (&[u16], Vec<(i32, i32, &[u16])>, i32) {
+        (&self.text, self.options.iter().map(|o| (o.index, o.kind, o.text.as_slice())).collect(), self.sel)
+    }
     /// `SetDialog`.
```

```diff
--- a/crates/eqoa/src/dialog.rs
+++ b/crates/eqoa/src/dialog.rs
@@ -340,6 +340,9 @@
 }
 
 impl MBox {
+    pub fn sel(&self) -> i32 {
+        self.sel
+    }
     /// `Prompt(text, type, p1, p2)`: `Init(type, 1)`, the text and the two
```

---

## 2. Loot & Bank list state (`npcw.rs`, `loot.rs`, `bankplay.rs`, `bank.rs`) — +8 / -2

`WndList.sel`, `WndList.top`, and `WndList.items` (`wndlist.rs:58-71`) are already `pub`, as is `Play.npcw.merchant` and `Merchant::read_state` from Round 5. However:
- `NpcWindows.loot: Loot` (`npcw.rs:18`) and `Loot.inv` / `Loot.loot` (`loot.rs:48-49`) are private, hiding loot `selected_row`, `scroll_top`, and `rows`.
- `BankPlay.wnd: Bank` (`bankplay.rs:18`) and `Bank.menu` / `Bank.item` (`bank.rs:111-114`, containing `BankItem.inv`/`BankItem.bank` at `bank.rs:85-86`) are private, hiding bank menu/list `selected_row`, `scroll_top`, and `rows`.

```diff
--- a/crates/eqoa/src/npcw.rs
+++ b/crates/eqoa/src/npcw.rs
@@ -16,7 +16,7 @@
 pub struct NpcWindows {
     /// `HaveCorpse` and `m_corpse`
     pub corpse: Option<ItemList>,
-    loot: Loot,
+    pub(crate) loot: Loot,
     loot_open: bool,
```

```diff
--- a/crates/eqoa/src/loot.rs
+++ b/crates/eqoa/src/loot.rs
@@ -55,6 +55,9 @@
     pub ml: crate::masterloot::MasterLooter,
 }
 
+impl Loot {
+    pub fn read_list(&self) -> (&WndList, &WndList) { (&self.inv, &self.loot) }
+}
 impl Default for Loot {
```

```diff
--- a/crates/eqoa/src/bankplay.rs
+++ b/crates/eqoa/src/bankplay.rs
@@ -16,7 +16,7 @@
     /// the auction's half (`VIBankControl`'s search state)
     pub auction: crate::auction::Auction,
-    wnd: Bank,
+    pub(crate) wnd: Bank,
     /// `VIWndAuctionSearch` (mode 39) and the kind it was opened for
```

```diff
--- a/crates/eqoa/src/bank.rs
+++ b/crates/eqoa/src/bank.rs
@@ -115,6 +115,9 @@
     sell: crate::auction::SellWnd,
     pub visible: bool,
 }
+impl Bank {
+    pub fn read_lists(&self) -> (&WndList, &WndList) { (&self.item.inv, &self.item.bank) }
+}
```

---

## 3. Core menu/list windows (`questlog.rs`, `startmenu.rs`, `spellbook.rs`, `trainer.rs`, `classmastery.rs`) — +12 / -1

`SpellBook` already exposes `pub visible`, `pub mode`, `pub sel`, `pub tab`, `pub slot`, `pub rows` (`spellbook.rs:99-118`); only `top: i32` (`spellbook.rs:103`) is private. The other primary windows keep selection/scroll private:
- `QuestLog.sel` / `QuestLog.top` (`questlog.rs:83-84`) are private (`Play.quests.titles` is already `pub`).
- `MainMenu.sel` / `top` / `sub` / `visible` (`startmenu.rs:144-156`) are private (`StartMenu.main` and `StartMenu.visible` are already `pub`).
- `Trainer.sel` / `Trainer.mode` (`trainer.rs:145,147`) are private.
- `ClassMastery.column` / `category` / `owned` / `available` (`classmastery.rs:573-578`) are private (`Part` skills and node are already `pub`).

```diff
--- a/crates/eqoa/src/questlog.rs
+++ b/crates/eqoa/src/questlog.rs
@@ -103,6 +103,9 @@
 }
 
 impl QuestLog {
+    pub fn read_state(&self) -> (i32, i32) {
+        (self.sel, self.top)
+    }
```

```diff
--- a/crates/eqoa/src/startmenu.rs
+++ b/crates/eqoa/src/startmenu.rs
@@ -161,6 +161,11 @@
 }
 
 impl MainMenu {
+    pub fn read_state(&self) -> (usize, i32, i32) {
+        match &self.sub {
+            Some(sub) => sub.read_state(),
+            None => (self.menu, self.sel, self.top),
+        }
+    }
```

```diff
--- a/crates/eqoa/src/spellbook.rs
+++ b/crates/eqoa/src/spellbook.rs
@@ -100,7 +100,7 @@
     /// 0 the list, 1 arranging (one held), 2 choosing a slot
     pub mode: i32,
     pub sel: i32,
-    top: i32,
+    pub top: i32,
     pub tab: i32,
```

```diff
--- a/crates/eqoa/src/trainer.rs
+++ b/crates/eqoa/src/trainer.rs
@@ -159,6 +159,9 @@
 }
 
 impl Trainer {
+    pub fn read_state(&self) -> (i32, i32) {
+        (self.sel, self.mode)
+    }
```

```diff
--- a/crates/eqoa/src/classmastery.rs
+++ b/crates/eqoa/src/classmastery.rs
@@ -595,6 +595,12 @@
 }
 
 impl ClassMastery {
+    pub fn read_state(&self) -> (i32, i32, i32, i32) {
+        let list = if self.column == 0 { &self.owned } else { &self.available };
+        (self.column, self.category, list.sel, list.top)
+    }
```

---

## Footprint budget delta (`docs/fork-footprint.json`, measured)

| File | Prior budget (+ / -) | Applied delta | Final measured budget (+ / -) |
|---|---:|---:|---:|
| `crates/eqoa/src/dialog.rs` | 0 / 0 | +9 / -0 | 9 / 0 |
| `crates/eqoa/src/npcw.rs` | 1 / 1 | +4 / -0 | 5 / 1 |
| `crates/eqoa/src/loot.rs` | 0 / 0 | +3 / -0 | 3 / 0 |
| `crates/eqoa/src/bankplay.rs` | 0 / 0 | +4 / -0 | 4 / 0 |
| `crates/eqoa/src/bank.rs` | 0 / 0 | +3 / -0 | 3 / 0 |
| `crates/eqoa/src/questlog.rs` | 0 / 0 | +3 / -0 | 3 / 0 |
| `crates/eqoa/src/startmenu.rs` | 1 / 1 | +6 / -0 | 7 / 1 |
| `crates/eqoa/src/spellbook.rs` | 0 / 0 | +3 / -0 | 3 / 0 |
| `crates/eqoa/src/trainer.rs` | 0 / 0 | +3 / -0 | 3 / 0 |
| `crates/eqoa/src/classmastery.rs` | 0 / 0 | +4 / -0 | 4 / 0 |
| **Total** | **2 / 2** | **+42 / -0** | **44 / 2** |

*(Optional secondary windows not needed for core Part A parity—`FactionWnd`, `WhoList`, `RecipeWnd`, `WndGuild`, `BuddyList`/`IgnoreList`—remain deferred; each can add a 3-line `read_state(&self)` if a bundled mod later inspects their rows.)*
