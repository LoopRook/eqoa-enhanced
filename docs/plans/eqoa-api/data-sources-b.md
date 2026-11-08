# eqoa API data sources, part B

Scout: inventory, item, bank, loot, merchant, gems, quest, quest_log, camera, input, ui, window, plus the
UiForge / ImGui / DrawList surface. Read-only research against `feat/eqoa-api-r1`; the Forge reference is
`run/forge-compat-reference/` (pinned Forge 4026d5c, UiForge 6f2d125).

"How our adapter uses it" means the native re-implementation in `crates/eqoa/src/fork_ext/addons/`.
Forge modules that are not registered in `host::for_addon` (`host.rs:340-355`) have no adapter at all;
their status is MISSING even where the native data is present and correct.

## Summary

| Module | REAL | PLACEHOLDER | MISSING | Biggest gaps |
|---|---|---|---|---|
| inventory | 0 | 0 | 18 | No adapter. Tabs/selection live in `ItemMenu`, not in a record array. |
| item | 0 | 0 | 69 | No adapter. Live-record identity must become a list index + `inv_index` key. |
| bank | 0 | 0 | 2 | No adapter; `dump.bank` exists at zone-in. |
| loot | 0 | 0 | 7 | No adapter; corpse is `npcw.corpse` while mode 1. |
| merchant | 0 | 0 | 16 | No adapter; wares exist only between 0x00B7 and the next merchant. |
| gems | 0 | 0 | 3 | Pure static table (80 rows); the client carries no gem metadata. |
| quest | 0 | 0 | 3 | Titles only (the Forge module is titles only too). |
| quest_log | 4 | 0 | 0 | Complete. |
| camera | 0 | 0 | 3 | No adapter; `walkCam.cur` has both values. |
| input | 23 | 8 | 0 | Pad buttons/analog are the default snapshot; d-pad rotation is a flag nothing reads. |
| ui | 33 | 3 | 0 | Class emblem and disposition drawing are stubs. |
| window | 4 | 0 | 43 | No window tree; dialog state exists natively but is not exposed. |
| UiForge core | 17 | 0 | 3 | `modules_path`, `profiles_path`, `LogLevel`; JPEG textures absent. |
| ImGui | ~95 | 0 | ~158 | Tooltip/mouse/IO: `BeginTooltip`, `GetMousePos`, `GetMouseWheel`, drag delta, `IsMouseClicked`; `PopStyleVar`, item width. |
| DrawList | 14 | 0 | 13 | `AddImageQuad`, `AddQuad(Filled)`, all `Path*`, texture push/pop. |

Counts are API items (functions, plus the named tables/fields listed in each section), not lines of
code. "MISSING" means no Forge adapter and no `eqoa` module yet, even where the native data is REAL.

---

## inventory (`scripts/modules/frontiers_forge/inventory.lua`)

### Public surface

- Metadata: `max_slots` (40), `GetTunar`, `InventoryUsed`, `SlotsUsed`, `SlotsRemaining`, `IsFull`.
- Slot labels: `GetSlotLabelAndOrder(slot_id)` (the paperdoll order table, `inventory.lua:51`).
- Records: `GetIconRef(slot)`, `GetItem(slot)`, `GetItems()`.
- Window: `IsWindowOpen`, `GetOpenTab`, `GetTabLabel`, `GetTabs`, `GetTabList`, `GetSelectedSlot`,
  `GetSelectedItem`; the `Tab` table (`magic=0, equipment=1, tradeskill=2, misc=3`, `inventory.lua:167`).

### Where the data lives

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| 40 slots, packed list | `net::dump::ItemList { p04, items: Vec<NetItem> }`, `crates/net/src/dump.rs:99-104`; capacity `CAPACITY = 40`, `crates/eqoa/src/inventory.rs:12` | none | in-world (zone-in dump, `dump.rs:512`) | MISSING |
| Tunar carried | `dump.player.p34`, `crates/net/src/dump.rs:24`; set by 0x0052, `crates/eqoa/src/npcw.rs:143` | none | in-world | MISSING |
| Slots used / remaining | `dump.inventory.items.len()` (the list is compacted on take, `inventory.rs:60-69`) | none | in-world | MISSING |
| Slot label + order | paperdoll slot numbering is the *equip* slot (`inventory.rs:16-41` `can_wear_in`); Forge's table is a UI ordering constant, pure data | none | always | MISSING |
| Icon hash | `inventory::item_icon(it, expansion_flags)`, `inventory.rs:49-51` (`icon2` when `flags & 2`, else `icon`) | none | in-world | MISSING |
| Item record | `NetItem`/`Item`, `dump.rs:58-97` | none | in-world | MISSING |
| Window open | start menu (mode 13) with `items.visible`: point routing `crates/eqoa/src/play.rs:4477`, draw `play.rs:8271`; `ItemMenu::open` `itemmenu.rs:378`. The item page is a page of the start menu, not a window | none | only while the start menu's item page is drawn | MISSING |
| Current tab | `ItemMenu.tab`, `crates/eqoa/src/itemmenu.rs:255` (0 usable, 1 equipment, 2 charged, 3 misc; labels words 437/438/1473/439, `itemmenu.rs:5`) | none | window only | MISSING |
| Tab labels | `vui.words.get(437|438|1473|439, language)` — same four localized strings Forge reads (0x1B5/0x1B6/0x5C1/0x1B7) | none | in-game (needs `words`) | MISSING |
| Selected row | `ItemMenu.menus[tab].list.sel`, `itemmenu.rs:254` + `wndlist.rs:58`; each entry's `id` is the inventory index (`itemmenu.rs:90`) | none | window only | MISSING |
| Selected item | `dump.inventory.items[entry.id]` | none | window only | MISSING |

Forge's `GetTabList` returns a `WindowObject` (menu descriptor). The native equivalent is the
`WndMenu`/`WndList` pair (`wndlist.rs:645-649`), which is an internal struct, not a Lua object.

### Event source

Inventory changes arrive as `net::session::Event`s in `crates/net/src/session.rs:418-430` (0x003B
`InvAdd`, 0x003C `InvTook`, 0x003D `InvMove`, 0x003E `InvUse`, 0x003F `InvEquip`, 0x0040 `InvUnequip`,
0x000A `QuickSlot`), read at `session.rs:1567-1576` and applied in `play.rs:2415-2452`. A mod
`inventory.changed` event should fire from those arms, not from a poll. Tunar changes are 0x0052
(`npcw.rs:143`).

### Actions

`inventory.lua` performs none. The actions it exposes data for are the item menu's:
`itemmenu::ItemOut` (`itemmenu.rs:43-54`) → `Play::item_outs` (`play.rs:5494-5528`) →
`use_item` 0x003E (`play.rs:5485-5490`), destroy 0x003C (`play.rs:5500-5507`), swap 0x003D
(`play.rs:5508-5514`), quick slot 0x000A (`play.rs:5515-5519`).

---

## item (`scripts/modules/frontiers_forge/item.lua`)

### Public surface

- Field getters (record-offset in): `GetSlotId`, `GetName`, `GetDescription`, `GetAmount`,
  `GetLevelReq`, `GetRange`, `GetMaxStack`, `GetMaxHp`, `GetDur`, `GetCurrentHp`, `GetItemId`,
  `GetValue`, `GetDamage`, `GetAttackType`, `GetIconRef`, `GetEquippedStatus`, `GetClassMask`,
  `GetRaceMask`, `GetStatCount`, `GetStats`, `GetStat`, `GetInventoryIndex`,
  `GetEquippedSlotIndex`, `GetInspectInfo`.
- Predicates/helpers: `IsEquippable`, `IsNoTrade`, `IsLore`, `IsNoRent`, `IsUsableByClass`,
  `IsUsableByRace`, `GetAllowedClasses`, `GetAllowedRaces`, `GetSlotName`, `GetAttackTypeName`.
- Constant tables: `SLOT` (28 ids), `STAT` (18 ids), `ATTACK` (11 ids), `SLOT_NONE`.
- Objects: `Item.new(record)`; 29 methods mirroring the getters plus 34 live properties
  (`item.name`, `item.amount`, …), and `IsValid` (name + slot still match, `item.lua:536-541`).

### Where the data lives

`dump::Item` (definition) and `dump::NetItem` (slot) hold every field; the Forge record is these two
structs flattened.

| Forge field | Native source |
|---|---|
| Item Id +0x00, Value +0x04 | `Item.id`, `Item.value` (`dump.rs:59-60`) |
| Icon +0x0C, Icon Frontiers +0x10 | `Item.icon`, `Item.icon2` (`dump.rs:62-63`) |
| Slot +0x14 | `Item.wear_slot` (`dump.rs:64`) |
| No Trade +0x1C, Rent +0x20, Lore +0x50 | `Item.no_trade`, `Item.rent`, `Item.lore` (`dump.rs:66-79`) |
| Attack +0x28, Damage +0x2C, Range +0x30 | `Item.attack_type`, `Item.damage`, `Item.range` (`dump.rs:69-71`) |
| Level Req +0x34, Max Stack +0x38, Max HP +0x3C, Dur +0x40 | `Item.level_req`, `max_stack`, `max_hp`, `durability` (`dump.rs:72-75`) |
| Class Mask +0x44, Race Mask +0x48 | `Item.class_mask`, `race_mask` (`dump.rs:76-77`) |
| Name +0x5C, Description +0xDC | `Item.name`, `Item.desc` (`dump.rs:82-83`) |
| Stat Count +0x2DC, Stat Array +0x2E0 | `Item.stats: Vec<ItemStat>` (`dump.rs:51-54`, `84`) |
| Amount +0x2E4, Current HP +0x2E8 | `NetItem.quantity`, `NetItem.cur_hp` (`dump.rs:90-91`) |
| Equipped Flag +0x2F0 | `NetItem.equip_slot`; `-1` is "not equipped" (`dump.rs:93`) |
| Inventory Index +0x2F8 | `NetItem.inv_index` (`dump.rs:95`) — the server key used on the wire, not the list position |

Icon selection matches: Frontiers icon first (`inventory.rs:49-51`). Class/race usability is
`inventory::can_use` (`inventory.rs:54-57`). Localized slot/stat/attack names are
`infotext::wear_slot_word`, `STAT_WORDS` and `attack_word` (`crates/eqoa/src/infotext.rs:13,32-39`).

### Availability, events, status, actions

- Availability: in-world (the item list is in the zone-in dump).
- Event source: the same `Event::Inv*` arms as inventory; item HP is `Event::ItemHp` (0x0060,
  `play.rs:2447-2453`).
- Status: MISSING — no `frontiers_forge.item` in `host.rs:340-355`. Record-shaped Lua objects should be
  replaced by a stable `{ list = "inventory"|"bank"|"loot"|"merchant", index, inv_index }` handle,
  because the native lists shift on take (`inventory.rs:60-69`) exactly as Forge warns.
- Actions: none in `item.lua`; use/equip/destroy/swap are the item-menu paths above.

---

## bank (`scripts/modules/frontiers_forge/bank.lua`)

### Public surface

`SlotsUsed()`, `GetItems()` returning `{ idx, slot, level, name, amount }` (`bank.lua:75-103`).

### Where the data lives

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Bank items (`VIItemList`) | `dump.bank.items`, `crates/net/src/dump.rs:133-138`, `291`; read with the zone-in dump at `dump.rs:513` and updated by 0x1251/0x1250, `crates/eqoa/src/bankplay.rs:98-115` | none | in-world (contents), even with the window closed | MISSING |
| Slot count | `dump.bank.items.items.len()` (packed list, same `took`/`add_item` helpers, `bankplay.rs:102-111`) | none | in-world | MISSING |
| Bank tunar | `dump.player.p38`, `dump.rs:25`; 0x1253 writes it, `bankplay.rs:91-97` | none | in-world | MISSING |
| Window open | mode 11, opened only by 0x124D (`bankplay.rs:82-90`, `play.rs:753`) | none | only after the banker opens it | MISSING |

Forge's per-record `slot`/`level`/`name`/`amount` map to `NetItem.item.wear_slot`,
`item.level_req`, `item.name`, `quantity`.

### Event source

`Play::bank_opcode` (`bankplay.rs:79-138`): 0x124D open, 0x1250 taken, 0x1251 added, 0x1253 tunar,
0x1206 auction; the requests' answers clear `bank.state` (`bankplay.rs:91-135`).

### Actions

`bank::BankOut` (`crates/eqoa/src/bank.rs:29-45`) → `bankplay.rs:206-250`: move 0x1254
(`bankplay.rs:214-222`), money 0x1255 (`224-228`), auction sell 0x1205 (`bank.rs:41`, `auction.rs`),
close (`bankplay.rs:208`, `bank_close`). None are mod-callable today.

---

## loot (`scripts/modules/frontiers_forge/loot.lua`)

### Public surface

`max_slots` (40), `IsLootWindowOpen`, `GetCount`, `GetIconRef`, `GetItem`, `GetItems`, `Items`
(iterator), all gated on the open flag (`loot.lua:42-120`).

### Where the data lives

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Corpse list | `npcw.corpse: Option<ItemList>`, `crates/eqoa/src/npcw.rs:17` | none | only while a corpse is open (0x0018 sets it, `npcw.rs:108-117`; 0x0016 clears, `npcw.rs:118-125`); the zone-in dump may also carry one (`dump.rs:514-516`) | MISSING |
| Open flag | `Play::mode == 1`, `npcw.rs:111-113`; `owns()` `npcw.rs:33-35` | none | corpse window only | MISSING |
| Count | `corpse.items.len()`, capped at 40 rows when drawn (`crates/eqoa/src/loot.rs:73-88`) | none | corpse window only | MISSING |
| Selected row | `Loot.loot.sel` (`crates/eqoa/src/loot.rs:38`, `wndlist.rs:58`); row 0 is LEAVE ALL | none | corpse window only | MISSING |

Forge's loot index is 1-based over item records (`loot.lua:69-88`) and matches `corpse.items[i-1]`;
native row 0 is the LEAVE ALL row, so do not hand mods the list row.

### Event source

`npcw_opcode` (`npcw.rs:78-172`): 0x0018 opens/replaces the corpse, 0x0019 removes one (server answer,
`npcw.rs:84-92` + `msg_0019`), 0x001A empties it into the inventory (`npcw.rs:126-142`), 0x0016 closes,
0x0085 is the refusal word (`npcw.rs:93-107`).

### Actions

`loot::LootOut` (`loot.rs:26-34`) via `Loot::key` (`loot.rs:117-147`) → `npcw.rs:426-437`: close 0x0016,
take one 0x0019 (item key + count), take all 0x001A. This is the auto-loot hook the plan wants.

---

## merchant (`scripts/modules/frontiers_forge/merchant.lua`)

### Public surface

`max_slots`, `Mode {menu=0,sell=1,buy=2}`, `Option {buy=0,sell=1,exit=2}`; `IsOpen`, `GetEntityId`,
`GetMode`, `GetGreeting`, `GetSelectedOption`, `GetOptionLabel`, `GetOptions`, `GetButtonHint`,
`GetItemCount`, `GetItem`, `GetSelectedItemIndex`, `GetSelectedItem`, `GetItems`.

### Where the data lives

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Merchant entity id | `Shop.id: Option<u32>`, `crates/eqoa/src/merchant.rs:49`, from 0x00B7 (`read_shop`, `merchant.rs:64-72`); `None` is Forge's 0xFFFFFFFF | none | only after 0x00B7, until `merchant_close` (`npcw.rs:237-243`) | MISSING |
| Wares | `Shop.wares: ItemList`, `merchant.rs:54`; `ItemList` is the same struct as inventory | none | only while talking to that merchant; overwritten by the next 0x00B7 | MISSING |
| Sell/buy rates | `Shop.sell_rate`, `buy_rate`, `merchant.rs:51-52` | none | same | MISSING |
| Greeting | `Shop.greeting: Vec<u16>`, `merchant.rs:56` | none | same | MISSING |
| Window mode | `Merchant.mode` 0 menu / 1 sell / 2 buy, `merchant.rs:193` — identical numbering to Forge's `Mode` | none | window open (mode 10) | MISSING |
| Selected option | `MercMenu.sel` over `[63, 62, 161]`, `merchant.rs:118,160` | none | menu shown | MISSING |
| Selected row | `Merchant.list.sel` (`merchant.rs:458`), 0-based | none | list shown | MISSING |
| Button hint | word 161 with the `|t` glyph, `merchant.rs:165` — Forge's `FormatButtonHint("triangle", …)` | none | menu shown | MISSING |

### Event source

0x00B7 opens and fills the shop (`npcw.rs:144-153`); 0x00D7 clears the pending flags (`npcw.rs:154-158`);
`merchant_close` (`npcw.rs:237-243`) is local (no opcode).

### Actions

`merchant::MerchantOut` (`merchant.rs:100-113`) → `npcw.rs:439-461`: `Buy` 0x004A (index, count, shop
id), `Sell` 0x004B (inventory key, count, shop id), close, chat mode. Prices: `merchant_price` /
`sell_price` (`merchant.rs:80-98`).

---

## gems (`scripts/modules/frontiers_forge/gems.lua`)

### Public surface

`IsGem(name)`, `Get(name)` → `{ type, rarity, stat }`, `Format(name, amount, fn)`; an 80-entry
`LOOKUP` table of gem names (`gems.lua:5-100`).

### Where the data lives

No native equivalent: the client's item records carry no gem type/rarity/stat metadata (searched
`crates/` for gem tables; only item-name strings and quick-slot type 7 exist, e.g.
`crates/eqoa/src/gear.rs:11`). This is pure shipped data, not game state, so `eqoa.gems` can carry the
same 80 rows and stay REAL without reading the game.

### Notes / status

- Status: MISSING (module not registered).
- Forge bug to fix in the `eqoa` version: `Gems.Format` calls `Gems.get` (lowercase) while the function
  is `Gems.Get` (`gems.lua:112-113`), so `Format` raises `attempt to call a nil value` today.
- Availability: always.

---

## quest (`scripts/modules/frontiers_forge/quest.lua`)

### Public surface

`Quest.size` (0x100), `Quest.new(address)`, `Quest:GetName()`. The file's own comment says the client
keeps no other quest data (`quest.lua:4-5`).

### Where the data lives

`Quests.titles: Vec<Vec<u16>>` (`crates/eqoa/src/questlog.rs:17-20`) — the same titles, capped at 8,
plus `stamp`. Native source chain: zone-in dump `d.quests` (`dump.rs:289,505-511`), 0x007C add /
0x007D remove (`questlog.rs:44-62`, dispatched at `play.rs:2488,2542`).

### Status

MISSING (no `frontiers_forge.quest`); the data is REAL. There is nothing more to expose than the title,
and no object identity is needed — an index into the quest log is enough.

---

## quest_log (`scripts/modules/frontiers_forge/quest_log.lua`)

### Public surface

`max_quests` (8), `GetCount`, `GetQuestByIndex(index)` (0-based, returns a Quest object), `Quests`.

### Where the data lives / adapter

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Count | `Quests.titles.len()`, `questlog.rs:17-20` | `JournalSnapshot.count`, `crates/eqoa/src/fork_ext/addons/quest_log.rs:42-48` | in-world (`available` needs `in_game`) | REAL |
| Title by 0-based index | `titles[i]` (UTF-16, 128 chars) | `quest_log.rs:50-68`; object with `GetName` | in-world | REAL |
| Iterator | — | `quest_log.rs:85-100` | in-world | REAL |
| `max_quests` | `questlog.rs:24-28` (cap 8) | `quest_log.rs:73` | always | REAL |

Data is filled by `Play::forge_snapshot` from `self.quests.titles` (`crates/eqoa/src/fork_ext/play.rs:734-737`);
`JournalSnapshot` deliberately bypasses the health gate (`quest_log.rs:104-130`).

### Event source

0x007C/0x007D handlers (`play.rs:2488,2542`) and the zone-in dump. A `quest_log.changed` event belongs
there.

### Actions

Drop a quest: `QuestLog::key` returns `QuestUp::Drop(i)` (`questlog.rs:180-230`) → 0x007D with the
stamped index (`questlog.rs:64-69`, sent at `play.rs:6162-6165`).

---

## camera (`scripts/modules/frontiers_forge/camera.lua`)

### Public surface

`GetCoordinates()` → `{x,y,z}`, `GetFacingRadians()`, `GetFacingDegrees()` (north 0, west 90, east 270).

### Where the data lives

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Eye position | `cam.cur.camera_pos`, `crates/vi/src/camera.rs:32`; final eye is `walker.eye.position` (`crates/eqoa/src/walker.rs:100`, set at `play.rs:3912`) | none | in-world (walker exists) | MISSING |
| Facing | `cam.cur.orbit.x`, `camera.rs:26` (orbit angle) | none | in-world | MISSING |

The correspondence is exact: `CameraState` is `time` i64 at +0x00, `flags` at +0x08, `orbit` at +0x0C,
`camera_pos` at +0x3C (`camera.rs:21-35`), and Forge reads 0x1FB66AC and 0x1FB66DC — +0x0C and +0x3C,
0x30 apart. In first person the eye sits on the player (`set_camera`, `camera.rs:625-630`), so
`GetCoordinates()` is the camera, not the player; player position/heading is already
`PlayerSnapshot.position/heading_rad` (`fork_ext/play.rs:141-149,318-320`).

### Events / actions

Event source: `cam.process` once per frame (`play.rs:3912,4815,9323`). Forge has no setters. The
player's own camera actions are the right/left stick and mouse (`controls.rs:402-432`,
`mouse.rs:637-751`) and presets (`camstyle.rs:270-283`); an `eqoa.camera.set_orbit_angle/pitch/distance`
action would call `cam.set_orbit_angle`, `set_orbit_pitch_angle`, `set_orbit_distance`
(`camera.rs:243` (pitch), `:256` (distance), `:397` (angle)).

---

## input (`scripts/modules/frontiers_forge/input.lua`)

### Public surface

- Pad: `button_mask`, `IsButtonPressed`, `GetRawAnalogStickState`, `GetNormalizedAnalogStickState`.
- Keys: `Key`, `GetKeyName`, `IsKeyHookInstalled`, `InstallKeyHook`, `UninstallKeyHook`,
  `IsKeyHookSuspended`, `AcquireKeyHook`, `ReleaseKeyHook`, `AcquireKey`, `ReleaseKey`, `GetKeyOwner`,
  `GetKeyOwners`, `SetTextCaptureActive`, `IsTextCaptureActive`, `SetKeySuppressed`, `IsKeySuppressed`,
  `ClearAllSuppressedKeys`, `PollKeyEvents`, `GetDroppedKeyCount`, `Update`.
- D-pad: `IsDpadHookInstalled`, `InstallDpadHook`, `UninstallDpadHook`, `IsDpadHookSuspended`,
  `GetDpadHookError`, `AcquireDpadRotation`, `ReleaseDpadRotation`, `SetDpadRotated`, `IsDpadRotated`.

### Where the data lives

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Key events (down/up, code) | winit key → `record_key_event`, `crates/eqoa/src/fork_ext/addons/runtime.rs:192-204`, ring in `input.rs:96-108` | `PollKeyEvents`, `input.rs:582-644` | always | REAL |
| Key capture/suppression | `wants_key_capture`, `input.rs:169-182`; used at `runtime.rs:205-212` | `Acquire*`/`SetKeySuppressed` | plain play only | REAL |
| Button bitfield | **not fed**: `InputSnapshot::default()` at `crates/eqoa/src/fork_ext/play.rs:732`; the real state is `input::Report { buttons, sticks }` (`crates/eqoa/src/input.rs:82-88`), merged in `viewer.rs:462-477` | `IsButtonPressed`, `input.rs:240-249` | would be always | PLACEHOLDER |
| Analog sticks | same default; real `Report.sticks` | `GetRaw/GetNormalizedAnalogStickState`, `input.rs:251-284` | would be always | PLACEHOLDER |
| D-pad rotation | flag only, `input.rs:52-57,110-113`; nothing consumes it (the Forge code cave rotated the game's pad report) | `Set/IsDpadRotated` | — | PLACEHOLDER |
| D-pad hook error | `dpad_error` is only ever set to `None` (`input.rs:666`), so `GetDpadHookError` always returns nil | `input.rs:702-706` | — | PLACEHOLDER |

The controller report the game itself uses is built in `viewer.rs:462-477` from `crate::input`
(buttons per `binds` remap, sticks from `console.sticks()`); a native snapshot field there fixes the
three pad placeholders without touching the game.

### Events and actions

Event source: `runtime.rs:192-212` (raw key events), `viewer.rs:460-492` (pad reports). `input` performs
no game action; it only captures keys. Game input goes through `App::set_key` (`viewer.rs:483`) into
`Play::hud_key`.

---

## ui (`scripts/modules/frontiers_forge/ui.lua`)

### Public surface (36 functions)

- Per-element hide/show with owner counting (14 pairs): HealthBar, PowerBar, MainExpBar,
  SecondaryExpBar, ExperienceBars, Compass, TargetNameplate, GroupMemberPanel, GroupCompassMarkers,
  GroupDisplay, PetPanel, ActiveEffectsDisplay, AbilityBar, ChatWindow.
- `DisableUI(owner)`, `EnableUI(owner)`.
- `GetHiddenBy(element_name)`.
- `GetDispositionFaceTexId`, `DrawDispositionIcon`, `GetClassIconTexId`, `GetClassIconTexture`,
  `DrawClassIcon`.

### Where the data lives

`HudClaimsRegistry` (`crates/eqoa/src/fork_ext/addons/visibility.rs:31-98`) holds
element → instance → owner sets; `ALL_UI_ELEMENTS` is the 14 names (`visibility.rs:14-29`). Claims are
projected only while the health snapshot is available (`visibility.rs:109-128`), and published into the
HUD (`publish_claims`, `visibility.rs:593-616`), which every HUD draw consults
(`crates/eqoa/src/fork_ext/hud.rs:21-119`).

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| Element hide/show | claims registry + `publish_claims` | `bind_element!`, `visibility.rs:240-294`; compounds `296-399`; `DisableUI` `404-431` | in-world (needs the HP snapshot) | REAL |
| `GetHiddenBy` | `get_hidden_by`, `visibility.rs:76-89` | `visibility.rs:433-443` | in-world | REAL |
| Disposition face id | Forge-only mapping (0x73/0x74/0x75, `ui.lua:462-480`); `EntityEntry.disposition: Option<u8>`, `fork_ext/addons/entity.rs:34` | `visibility.rs:446-465` | always | REAL |
| Class icon id | `CLASS_ICON_TEX_IDS`, Forge data (`ui.lua:500-516`) | `visibility.rs:467-503` | always | REAL |
| `GetClassIconTexture` | `Icon.GetUITexture` is implemented (`crates/eqoa/src/fork_ext/addons/icons.rs:1308-1343`); the ui stub returns `nil,nil,nil` | `visibility.rs:505-509` | in-world with the UI atlas | PLACEHOLDER |
| `DrawDispositionIcon` / `DrawClassIcon` | would be `ImGui.Image(texture, w*scale, h*scale)` | `visibility.rs:510-519` return `false` | in-world | PLACEHOLDER |

### Events / actions

No events. Hide/show is the action; it is an intent, not game state, so keep it out of the event stream.

---

## window (`scripts/modules/frontiers_forge/window.lua`)

### Public surface

- Tree/class identity: `Classes` (74 names), `GetRoot`, `GetRootWindow`, `GetChildren`, `Find(class)`,
  `FindSlot(class)`, `GetActiveWindow`, `GetActiveClassName`; object `Child`, `Parent`, `GetClassName`,
  `GetChildWindows`, `IsActive`, `IsVisible`, `GetDrawAddress`, `GetDrawSlotAddress`.
- Address relocation: `ToLiveAddress`, `ToStaticAddress`.
- Raw reads: `Window.new`, object `Read`, `ReadText`, `ReadTextPointer`, `Window.ReadTextAt`.
- Mode: `Mode`, `GetMode`, `IsBusy`, `IsStartMenuOpen`, `GetOverlaySlide`.
- Dialogs: `IsTextDialogOpen`, `GetTextDialogText`, `IsConfirmDialogOpen`, `GetConfirmDialogText`,
  `GetConfirmDialogStyle`, `GetConfirmDialogSelection`, `IsInputDialogOpen`, `InputTag`,
  `GetInputDialogTag`, `GetInputDialogText`, `IsChatKeyboardOpen`.
- Lists: `ReadMenu`, object `GetMenu`, `GetSelectedRow`, `GetScrollTop`.
- Hints: `GetButtonHints`, `ParseButtonHint`, `FormatButtonHint`, `ConfirmStyle`.

### What maps, what cannot

| Capability | Native source | Adapter | Availability | Status |
|---|---|---|---|---|
| UI mode | `Play::mode` (0..44, one per window; map at `crates/eqoa/src/fork_ext/play.rs:745-783`), snapshotted as `ui_mode` (`play.rs:684-692`) | `GetMode`, `crates/eqoa/src/fork_ext/addons/host.rs:170-174`; `Mode` table `host.rs:151-162` | in-world | REAL |
| Busy | `ui_busy` = any mode, map, search or typing (`play.rs:693-699`) | `IsBusy`, `host.rs:175-179` | in-world | REAL |
| Start menu | `start.menu.visible`/`map.open`/menu modes (`play.rs:680-683`) | `IsStartMenuOpen`, `host.rs:165-169` | in-world | REAL, with a deliberate divergence: true for *any* game window, not only pause pages (`host.rs:133-139`) |
| Window tree, class scanning, vtables, address relocation | None: the native client has no generic window objects; each window is a struct drawn by a function (`merchant.rs`, `bank.rs`, `loot.rs`, `itemmenu.rs`, …) | — | — | MISSING / Forge-only mechanism (raw-memory only) |
| Active window | `mode` identifies it; a mode→name table exists at `play.rs:745-783` | — | in-world | MISSING (easy: mode → enum) |
| GetActiveClassName / Find | same table; no live class objects | — | — | MISSING |
| Text dialog open | `mode == 9` (`play.rs:861-867` `open_dialog`) | — | in-world | MISSING (data exists) |
| Text dialog text | `PrivChat.text`, `crates/eqoa/src/dialog.rs:9-23` (private; set at `play.rs:865`), plus `DialogBox { id, text, options }` for the answer page, `dialog.rs:45-51`, set at `play.rs:2348` | — | while mode 9 | MISSING |
| Confirm open / text / style / selection | `mode == 14` (e.g. `play.rs:5339-5344`); `MBox { kind 0/1/2, text, p1, p2, sel }`, `dialog.rs:237-256`; `kind` is Forge's `ConfirmStyle` | — | while mode 14 | MISSING (`sel` and `PrivChat.text` are private) |
| Input widget open / text / tag | `mode == 2`; `EditBox { text, purpose }`, `crates/eqoa/src/editbox.rs:156-168` (`purpose` is Forge's +0x24 tag), held in `Play::edit` `play.rs:479` | — | while mode 2 | MISSING |
| Chat keyboard open | `Play::typing: Option<String>`, `play.rs:204` (or `mode == 2` chat purpose) | — | while typing | MISSING |
| Menu rows / selected row / scroll | per-window `WndList { sel, top }`, `crates/eqoa/src/wndlist.rs:53-76`; `WndMenu` `wndlist.rs:645-649` | — | per window | MISSING (no generic reader by design) |
| Button hints | each window builds its own glyph line (e.g. `merchant.rs:165`); `crate::startmenu`/`dialog` have the glyph helpers | — | per window | MISSING |

Of the 43 functions only `GetMode`, `IsBusy` and `IsStartMenuOpen` exist, alongside the `Mode` table —
4 of the 47 items. `Classes`, `InputTag` and `ConfirmStyle` are static data that can be shipped.

Note: Forge's `IsStartMenuOpen` is a separate pointer chain because pause-page sub-modes take the mode
word; the native `start_menu` flag already models that (and more, see the divergence above), so it is the
right value for overlay hiding.

### Events / actions

The only event source is the mode word changing (`Play::mode`, assigned throughout `play.rs` and the
window modules, e.g. `npcw.rs:113,121,150`, `bankplay.rs:87`). A `window.opened/closed` event should be
emitted from a central `set_mode` seam rather than polling. No actions in `window.lua`.

---

## UiForge core

Reference bindings: `run/forge-compat-reference/UiForge/src/core/core.cpp:672-943`.

| Reference item | Native | Status |
|---|---|---|
| `LoadTexture`, `CreateTextureFromMemory`, `ReleaseTexture` | `textures.rs:257-461` | REAL (PNG/raw RGBA only) |
| `IGraphicsApi.CreateTextureFromFile` | `textures.rs:465-473` | REAL |
| `LoadFont` | `fonts.rs:336` | REAL |
| `LoadSound`, `PlaySound`, `StopSound`, `IsSoundPlaying`, `SetSoundVolume`, `ReleaseSound` | `sound.rs:933-1009` | REAL |
| `CallbackType`, `RegisterCallback`, `Log` | `host.rs:217-259` | REAL |
| `scripts_path`, `resources_path` | `host.rs:356-368` (also `textures.rs:317-323`) | REAL |
| `modules_path`, `profiles_path` | — | MISSING |
| `LogLevel` | — | MISSING (Log accepts a level; no table) |

Notable capability gaps named in the plan:

- **JPEG**: `decode_png_to_rgba8` only (`textures.rs:583-612`); `LoadTexture` rejects anything but PNG
  (`textures.rs:438`). `world_map`'s JPEG tiles cannot load until a JPEG path is added.
- **File listing**: `Util.ListFiles`/`ListFilesInDir` use LuaJIT `ffi` + `FindFirstFileW`
  (`util.lua:618-675`) — Forge-only raw FFI, out of scope. The native replacement should be a resource
  listing confined to the addon root, matching the `io.open` sandbox in `file_io.rs:203-233`.
- `modules_path`/`profiles_path` are paths, not capabilities; exposing them is trivial but must point at
  real sandboxed directories.

---

## ImGui

Counts are from the pinned binding: 253 `ImGui.set_function` entries in
`UiForge/externals/sol2_ImGui_Bindings/sol_ImGui.h`. Our bridge binds about 95 (24 in `bridge.rs`,
~69 in `bridge_widgets.rs` including the expanded loops, plus 2 draw-list getters). That is roughly 38%.

Implemented, by area:

| Area | Functions (file) |
|---|---|
| Windows / layout | `Begin`, `End`, `BeginChild`, `EndChild`, `BeginMenuBar`, `EndMenuBar`, `BeginMainMenuBar`, `EndMainMenuBar`, `BeginMenu`, `EndMenu`, `MenuItem`, `BeginTabBar`, `EndTabBar`, `BeginTabItem`, `EndTabItem`, `BeginGroup`, `EndGroup`, `Indent`, `Unindent`, `SameLine`, `Dummy`, `SetCursorPos`, `SetCursorScreenPos`, `GetCursorPos`, `GetCursorScreenPos`, `GetContentRegionAvail`, `GetWindowPos`, `SetWindowPos`, `SetNextWindowPos`, `SetNextWindowSize`, `GetScrollY`, `GetScrollMaxY`, `SetScrollY`, `SetScrollHereY`, `IsWindowFocused`, `IsWindowHovered`, `PushID`, `PopID`, `PushClipRect`, `PopClipRect`, `SetNextItemWidth` (`bridge.rs`, `bridge_widgets.rs`) |
| Text | `Text`, `TextColored`, `TextDisabled`, `TextWrapped`, `TextUnformatted`, `BulletText`, `Separator`, `SetTooltip`, `CalcTextSize` |
| Widgets | `Button`, `InvisibleButton`, `Selectable`, `Checkbox`, `RadioButton`, `SliderInt`, `SliderFloat`, `ColorEdit3`, `ColorEdit4`, `InputText`, `InputTextWithHint`, `InputInt`, `Combo`, `BeginCombo`, `EndCombo`, `BeginPopup`, `EndPopup`, `OpenPopup`, `TreeNode`, `TreePop`, `CollapsingHeader`, `ProgressBar`, `Image`, `PushTextWrapPos`, `PopTextWrapPos` |
| Style / fonts | `PushStyleColor`, `PopStyleColor`, `PushFont`, `PopFont`, `SetWindowFontScale`, `GetColorU32` |
| Queries / state | `GetDisplaySize`, `GetTime`, `IsItemActive`, `IsItemHovered`, `IsMouseDragging`, `IsMouseHoveringRect`, `SetKeyboardFocusHere`, `GetFrameHeight`, `GetTextLineHeight`, `GetTextLineHeightWithSpacing`, `GetBackgroundDrawList`, `GetWindowDrawList` |

Notable MISSING (all present in the reference header):

- **Tooltips**: `BeginTooltip`, `EndTooltip`, `BeginItemTooltip` (only `SetTooltip` exists).
- **Mouse**: `GetMousePos`, `GetMousePosOnOpeningCurrentPopup`, `GetMouseWheel`, `GetMouseDragDelta`,
  `ResetMouseDragDelta`, `SetMousePos`, `SetMouseCursor`, `IsMouseDown`, `IsMouseClicked`,
  `IsMouseReleased`, `IsMouseDoubleClicked`, `IsMousePosValid`. The events are already captured and
  forwarded to ImGui (`runtime.rs:176-188,214-223`), so these are cheap bindings that unblock
  `world_map`/`mini_map` debris.
- **IO / keyboard**: `GetIO`, `IsKeyDown`, `IsKeyPressed`, `IsKeyReleased`, `GetKeyName`,
  `SetNextWindowFocus`, `SetWindowFocus`.
- **Stack / layout extras**: `PushStyleVar`, `PopStyleVar`, `PushItemWidth`, `PopItemWidth`,
  `PushButtonRepeat`, `PopButtonRepeat`, `Columns`, `BeginTable`/`EndTable`, `SetNextWindowCollapsed`,
  `SetNextWindowSizeConstraints`.
- **Widgets**: `DragFloat`, `DragInt`, `InputFloat`, `InputFloat2/3/4`, `InputInt2/3/4`,
  `MultiLineTextInput`, `ListBox`, `BeginListBox`/`EndListBox`, `PlotLines`, `PlotHistogram`,
  `BeginDisabled`/`EndDisabled`, `GetForegroundDrawList`.
- `GetWorldId` is not ImGui; it is `Util.GetWorldId` (`util.lua:400-403`) and is MISSING from our util
  adapter. Native source: `Play::world_id()` (`play.rs:5423`), or `dump.player.world` (`dump.rs:28`).

---

## DrawList

Reference binds 27 methods on `ImDrawList`
(`sol_ImGui.h`); ours binds 14 (`bridge_drawlist.rs:220-556`).

| Implemented | Missing |
|---|---|
| `GetBackgroundDrawList`, `GetWindowDrawList`, `PushClipRect`, `PopClipRect`, `AddLine`, `AddRect`, `AddRectFilled`, `AddCircle`, `AddCircleFilled`, `AddNgon`, `AddNgonFilled`, `AddTriangle`, `AddTriangleFilled`, `AddText`, `AddImage`, `AddImageRounded` | `AddImageQuad` (the map mods use it for rotated/curved tiles, `mini_map.lua:950,1134`, `world_map.lua:614`), `AddQuad`, `AddQuadFilled`, `PushClipRectFullScreen`, `PushTextureID`, `PopTextureID`, `PathClear`, `PathLineTo`, `PathRect`, `PathArcTo`, `PathBezierCubicCurveTo`, `PathStroke`, `PathFillConvex`, `GetForegroundDrawList` |

Also: our `AddText` accepts only the 3-arg (pos, color, text) and 5-arg (font, size, pos, color, text)
forms (`bridge_drawlist.rs:250-343`); the reference also exposes the wrap-width parameter, which text
blocks need. All 14 implemented methods delegate to the real ImGui draw list under the frame/epoch guard
(`bridge_drawlist.rs:73-120`), so they are REAL, not stubs.

---

## Cross-cutting notes for the spec

1. The zone-in dump (`net::dump::read`, `dump.rs:493-529`) already contains inventory, bank, corpse,
   quests and player fields, so most "items" data is available the moment the player is in the world —
   the *windows* are what are gated on modes.
2. Only the merchant is truly transient (0x00B7 overwrites the previous shop, `npcw.rs:144-153`).
3. Forge's window/class scanning and `Util.EEmem`/`FindFirstFileW` are the raw-memory mechanisms that
   must be replaced by native state, not ported. Everything else in this part has a direct native source.
