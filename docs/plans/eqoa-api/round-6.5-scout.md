# Round 6.5 scout: corpse, merchant and banker sessions without the native windows

Read-only trace for the round 6.5 actions ("loot the nearest corpse", "talk to the nearest merchant/banker").
Line numbers are from the round-6 tree, branch `feat/eqoa-api-r6` at `2e4fb5f`, read without changes. Paths are
repo-relative.

Scope notes:

- The 6.5 spec itself is not in that tree (`docs/plans/EQOA-MOD-API-PLAN.md` has no 6.5 text at `2e4fb5f`); this
  follows the brief's wording. If the spec lands later, re-check the option counts below against it.
- No server is in this repository. Every statement about what the server accepts is marked UNKNOWN; only the
  client's own code is described.
- "Window open" below means `Play.mode` 1 (corpse), 10 (merchant) or 11 (bank). Session data is separate:
  `npcw.corpse`, `npcw.shop`, `dump.bank`.

## 1. Looting

**Request.** `Play::attack_press` builds `0x0015` with the target's u32 id, only when the target entity has
`p18 & 2` (the corpse flag): `crates/eqoa/src/play.rs:5801-5809` (check at 5804, packet at 5806-5808). A click on
a corpse reaches the same function (`play.rs:5775-5795`, corpse test at 5784). There is no loot opcode anywhere
else; `0x0015` is the only place the corpse is asked for.

**Client-side checks before it.** Target exists in `entities.slots` by id (`play.rs:5803`), and the corpse bit
(`play.rs:5804`). That is all: no range test, no owner test, no death/animation test. "Targetable" (`p18 & 1`) is
not checked by the native path; the fork's own target action checks it (`crates/eqoa/src/fork_ext/play.rs:992-1004`).
Master loot is the only permission concept the client has, and it changes the window's menu, not the request
(`npcw.rs:60-70`, `loot.rs:145-151`).

**Reply that opens the window.** `0x0018`: `crates/eqoa/src/npcw.rs:153-164`. It stores the item list
(`read_item_list`, 156), clears the master-looter list (157), and opens only when `mode == 0`: `loot_open` set and
`mode = 1` (159-160). If any other window is up it instead sends `0x0016` back (161-163), i.e. refuses and drops
the corpse. The window itself is drawn from `run_npcw` (`npcw.rs:412-414` open, 463-468 draw), gated by `mode == 1`.

**Refusals.** `0x0085` says why, as a chat line: 1 = "already being looted by another player", 2 = "not
lootable", 3 = "belongs to another player or group" (`npcw.rs:138-152`; the port's reading of the words is in the
test comment at `npcw.rs:810-813`). These are server refusals; the client has no matching local check.

**Take / take-all.**
- `LootOne(i, n)` → `0x0019` with the corpse item's u32 wire key and u32 count (`npcw.rs:483-488`; built from
  `LootOut::One`, `loot.rs:33-45`).
- `RequestLootAll` → `0x001A`, no payload (`npcw.rs:489`).
- Both keys come from the list `0x0018` delivered: the window's rows are built from it (`npcw.corpse`,
  `loot.rs:92-107`), the keys are chosen in `npcw.rs:447-462` and sent in 479-499. The send loop itself does
  **not** read `mode`; only the row selection (`Loot::key`, `loot.rs:144-185`) does. So the request needs the
  session data and the server's corpse session, not the window.
- The server's answers `0x0019`/`0x001A` move the item(s) into the inventory and off `npcw.corpse`
  (`npcw.rs:129-137`, `174-190`). Once `0x0016` arrives (or is sent) the list is dropped (`npcw.rs:165-173`), so
  a later `0x0019` has nothing to name.

**Zoning and respawn** close the corpse the client knows about: the world-entry dump's corpse is kept and
`0x0016` is sent (`npcw.rs:263-268`), and a respawn does the same (`npcw.rs:272-283`, from `play.rs:5229`).

## 2. Merchant and banker

**Merchant start.** `Play::interact_with_target` sends `0x004C` with the target id for a creature with
`npc_flags & 1` (`play.rs:1042-1079`; dispatch at 1048-1055, packet at 1076-1078). Checks before it: a target
entity exists and has a creature record (1046-1047); no range test and no window-state test. The client code never
names `0x004C` in `merchant.rs`, so "flag 1 is the merchant" is an inference from the flag order here plus
`0x00B7` being the only merchant-open opcode (see UNKNOWN).

**Reply that opens the shop.** `0x00B7`: `npcw.rs:192-201` reads it with `merchant::read_shop`
(`crates/eqoa/src/merchant.rs:64-72`: merchant id, sell/buy rates, wares, greeting) and opens only from
`mode == 0` (194: always stored; 195-200: window, sound, mode 10). If another window is up the wares are stored
and no window opens, nothing is sent (test: `crates/eqoa/src/fork_ext/items_tests.rs:782-800`). So the shop
"session" is the `0x00B7` payload (`Shop.id`), not the window.

**Buy / sell.**
- `MerchantOut::Buy(i, n)` → `0x004A` (ware index, count, `shop.id`) (`npcw.rs:504-510`).
- `MerchantOut::Sell(i, n)` → `0x004B` (inventory wire key, count, `shop.id`), only while the stack still holds
  `n` (`npcw.rs:511-519`).
- Both use `shop.id` from `0x00B7` and set `shop.pending`; neither reads `mode`. `0x003B`/`0x003C`/`0x00D7`
  clear the pending state (`npcw.rs:202-206`, `243-260`).

**Banker start.** The same `interact_with_target` sends `0x124D` for `npc_flags & 2` (`play.rs:1053-1054`). The
server's `0x124D` opens the window only from `mode == 0` (`crates/eqoa/src/bankplay.rs:92-103`, gate at 97).
Nothing is stored when another window is up.

**Deposit / withdraw.** `0x1254` (banker id, from-bank byte, item wire key, count) and `0x1255` (banker id,
direction, amount) built at `bankplay.rs:229-244`; the banker id is `self.target` (234, 241), so the target must
still be the banker when the request goes. `bank.state` is the busy flag (0 idle). The bank's contents come from
the world-entry dump, not from the window (spec note `docs/modding/EQOA-API-SPEC.md:431`, `806`), so a bank
session can be driven with the window closed as far as the client's model goes.

**Closing the merchant / bank.** Local only, no opcode: `merchant_close` sets `mode` 0, hides, clears `shop.id`
(`npcw.rs:285-292`; `docs/plans/eqoa-api/data-sources.md:926-927`); `bank_close` sets `mode` 0 and hides
(`bankplay.rs:161-167`). Merchant Escape/Leave produce `MerchantOut::Close` (`merchant.rs:333-346`). A respawn
closes the merchant (`npcw.rs:272-283`) and resets the bank's busy state (`bankplay.rs:156-159`, called from
`play.rs:5230`).

## 3. Range checks: what the client code shows

None of the three has a client-side range check.

- Loot: corpse flag only (`play.rs:5804`).
- Merchant / banker: target + `npc_flags` bit only (`play.rs:1048-1055`); the bank and merchant handlers add a
  window-state gate, not a distance gate.
- The entity list itself only holds what the server streamed, bounded by the server's NPC display/distance offer
  (`play.rs:10064-10072`, `OP_ENTITY_VIEW`), so "nearest" is nearest among observed records, not among the world.

Whether the server accepts `0x0015`/`0x0019`/`0x001A`, `0x004C`/`0x004A`/`0x004B` or `0x124D`/`0x1254`/`0x1255`
out of range, or for a corpse the player may not loot, is **UNKNOWN** here: no server code in this repository,
and the client sends no distance with any of these requests.

## 4. Closing cleanly

| Session | Ends how | Client state to leave clean |
|---|---|---|
| Corpse | `0x0016` sent by the client (`npcw.rs:482`, `LootOut::Close`), or received (`npcw.rs:165-173`); also sent at world entry and respawn (`263-268`, `272-283`) | `mode` back to 0, `loot_open` false, `loot.visible` false, `npcw.corpse = None`, `npcw.loot.ml = None`, and the fork's loot-source/busy state (`fork_ext/loot_source.rs:52-62`, `fork_ext/play.rs:254-268`) |
| Merchant | local `merchant_close` only, no opcode (`npcw.rs:285-292`) | `mode` 0, `npcw.shop` cleared (`id`/wares/rates/greeting) or at least `id = None`, `shop.pending` false, `waiting` false, `merchant.visible` false; the chat-box mode is only touched when the window's list ran (`MerchantOut::ChatMode`) |
| Bank | local `bank_close` only (`bankplay.rs:161-167`) | `mode` 0, `bank.state` 0, `bank.wnd.visible` false; `0x00D7` also clears the state (`bankplay.rs:147-150`) |

`0x00D7` is the generic "transaction over/refused" that clears the merchant's and bank's pending flags
(`npcw.rs:202-206`, `bankplay.rs:147-150`).

## 5. How round 6 sends loot/sell/bank today, and the smallest change for 6.5

**Today (round 6).** Three one-line hook calls, of which two carry these actions (the third is camera):

- `crates/eqoa/src/npcw.rs:478` `super::fork_hooks::npcw_requests(self, &mut loot_out, &mut merch_out);` — runs
  after the native keys/draw have produced their own outputs and before the send loops at 479-522.
- `crates/eqoa/src/bankplay.rs:220` `super::fork_hooks::bank_requests(self, &mut out);` — before the loop at
  221-260.
- `crates/eqoa/src/play.rs:3326` `fork_hooks::apply_camera(...)` — camera only.

The fork stages a request in `dispatch_broker_action` (`fork_ext/play.rs:1754-2118`) and the hook pushes it into
the native vector (`fork_ext/play.rs:506-650` for loot/merchant, `653-711` for bank); the native loops build the
exact packets and set `shop.pending`/`bank.state`. Every one of these refuses unless the window is up:
`fork_ext/play.rs:1755`, `1820`, `1859`, `1941`, `2005`, `2080`, plus the window-closed resolver at 384-414 and
the snapshot at 894-911.

**Opening a session is fork-only.** `attack_press` (pub), `interact_with_target` (pub) and `target_id` (pub,
`play.rs:5325`) are all reachable from the fork, and they already apply the client's own checks. A 6.5 action
needs only: choose the entity, `target_id(id)`, then `attack_press()` (corpse) or `interact_with_target()`
(merchant/banker) — the game's own request, no fabricated position, 0 upstream lines. The server's replies then
arrive exactly as they do for a player.

**Hiding the native window** is the only part that needs upstream lines. Options:

| # | Change | Upstream lines | Notes |
|---|---|---|---|
| O1 | Do nothing; let the window show | 0 | Fails 6.5 ("without the native windows"). |
| O2 | Fork-only: revert `mode` to 0 in `begin_iteration` while a fork-opened session is live, and relax the fork's gates | 0 | Events are pumped after `begin_iteration` in the same frame (`play.rs:3325`, `3335`) and `run_npcw`/`run_bank` draw later in that frame (`9828`, `9830`), so the window is drawn for one frame. Needs a per-session flag so it never closes a window the player opened; fights the native state machine. |
| O3 | 3 guards on the open branches, one fork hook `window_hidden(kind)` (false unless the fork asked for a hidden open) | `npcw.rs:158` and `161-163` (2 changed lines, 0x0018); `npcw.rs:195` (1, 0x00B7); `bankplay.rs:97` (1, 0x124D). Total **4 changed lines, 0 added** (`+4/-4` in git terms), 2 upstream files | Recommended. No flash: with `mode` 0 the window never draws, never takes keys or the pointer, never changes the chat mode, and the game's HUD/input stay normal. A suppressed corpse open also leaves `mode` 0, so the reply's "another window is up, leave the corpse" branch (`npcw.rs:161-163`) never fires and a second `0x0018` opens again. The guard must be per session, not one-shot (a second `0x0018`/`0x00B7` while hidden must also stay hidden). Note: this is a behaviour branch, not a read-only accessor, so it needs the lead's explicit approval and a `docs/fork-footprint.json` reason + a hook-pattern entry; it is a no-op when the hook returns false. |
| O3′ | O3 except the bank: send `0x1254`/`0x1255` to the banker as target without `0x124D` | `npcw.rs` 2 changed lines (**+2/-2**) | Smallest. Bank data is in the dump and the requests carry the banker id, so the client model needs no bank window at all; whether the server requires `0x124D` first is UNKNOWN. |
| O4 | Keep `mode` (window logically open) and skip only the drawing | `npcw.rs:463-472` (the corpse and merchant draw arms), `bankplay.rs:217` (1) = **3 changed lines** (`+3/-3`) | Worse: the window still owns keys (`npcw.rs:447-462`) and pointer hits (`npcw_point`, `npcw.rs:51-58`), `wants_ui` hides the playing HUD (`play.rs:4846-4848`), and skipping `draw` leaves stale `Hits` because `hits.clear()` lives in the draw. |
| O5 | Use the client's hide-UI | 0 | Does not work: a window coming up shows the UI again for good (`crates/eqoa/src/hideui.rs:5-17`, `frame` at 44). |

**Fork-side work needed with any of O2-O4** (all fork-owned, no upstream lines): relax the six `mode`/window
gates and the window-closed resolver (above), change `native_action_snapshot`'s `loot_open`/`merchant_open`/
`bank_open` to session state (`fork_ext/play.rs:894-911`), and make the item containers visible while a hidden
session is live (`crates/eqoa/src/fork_ext/items_capture.rs:205`, `211-217`, `220-226`, key at `259`), or the mod
cannot read the corpse/wares it just opened (they are nil unless `mode` is 1/10/11 today). New action kinds for
"open nearest corpse" / "talk nearest merchant/banker" and the nearest search also live there.

## 6. "Nearest corpse / merchant / banker" today

- **Entity list**: `eqoa.entities.list()` / `find` / `named` / `dots` / `pick_dot` / `distance`
  (`docs/modding/EQOA-API-SPEC.md:223-229`), built from `Play.entities` in
  `crates/eqoa/src/fork_ext/api_capture.rs:173-182` with `id`, `name`, `level`, `position`, `actor_position`,
  `kind`, `class_code`, `search_flags`, `status_flags`, and `corpse`.
- **Corpse state**: the `corpse` boolean there is protocol `p18 & 2` (`api_capture.rs:181`, spec 861-867). It is
  the same bit `attack_press` tests (`play.rs:5804`). Death cannot be inferred from `hp_fraction` (spec 247).
  The corpse's items are only readable while the loot window is up (`items_capture.rs:211-217`), i.e. after an
  `0x0018`.
- **Merchant/banker NPC type**: the flags are read off the wire into `Creature.npc_flags`
  (`crates/net/src/entity.rs:190`, parsed at 353) and used at `play.rs:1048-1055` (1 → `0x004C`, 2 → `0x124D`,
  8 → `0x0061` smith) and in `crates/eqoa/src/autoplay.rs:39-41` (flags `0b1011` = merchant/banker/smith's).
  They are **not** exported by any `eqoa.*` read: `api_capture.rs:181` has no `npc_flags` field. Finding the
  nearest merchant or banker needs one added field in that fork-owned snapshot (`"npc_flags": c.map(|c|
  c.npc_flags)`), 0 upstream lines.
- **Nearest**: no native "nearest" read exists; it is a Lua-side scan of `eqoa.entities.list()` distances to
  `eqoa.player.get().position` (or `eqoa.entities.distance`). Bounded by the server's entity view (section 3).

## UNKNOWN / not verified

- Server acceptance at range or with the window closed for every request above; the server is not in this repo.
- That `0x004C` is the merchant's request by the game's own naming. The client dispatches it for `npc_flags & 1`
  and has no other merchant open path than `0x00B7`; the pairing is inferred.
- Whether the banker's `0x1254`/`0x1255` succeed without a preceding `0x124D` on a live server.
- Whether the server keeps a corpse/merchant session open after the client sends no close opcode (merchant and
  bank have none; only the corpse has `0x0016`).
- The exact text of words 673-675 is the port's comment (`npcw.rs:810-813`), not read from the client's words
  table here.
