# The eqoa mod API: native data sources, part A

Scout output, round 1 (read-only). Modules: player, entity, entity_list, group, effects, ability,
ability_list, ability_bar, combat, chat, cave, util, icon. Part B (the other scout) covers the rest.

How to read a row: **Forge** is the public surface of the pinned reference module
(`run/forge-compat-reference/scripts/modules/frontiers_forge/*.lua`, Forge `4026d5c`). **Native** is
where the game client itself holds or produces the value. **Adapter** is what the Forge-facing Rust
table does today (`crates/eqoa/src/fork_ext/addons/`), i.e. what a mod actually calls. **Status** is
REAL (value or action comes from live game state and does what it says), PLACEHOLDER (stub, zero, or
fake success — the location is given), MISSING (not there at all).

Line numbers are this worktree at 0.4.14 / develop `e48f0a6`.

Two facts hold for nearly every row and are not repeated:

- Everything is **in-world only**: `forge_snapshot` builds the data only while
  `connected && hud_ready` (`crates/eqoa/src/fork_ext/play.rs:131-133`), otherwise the snapshot is
  `Snapshot::default()`, and `Snapshot::managed` zeroes it again when `!in_game`
  (`crates/eqoa/src/fork_ext/addons/host.rs:35-52`). Queries then return 0/""/nil by design.
- The addon frame pushes a fresh `Snapshot` each frame (`play.rs:701-741`); a mod reading a value is
  reading a copy taken this frame, never a live PS2 record.

## Summary

| Module | REAL / PLACEHOLDER / MISSING (public entries) | Biggest gaps |
|---|---|---|
| player | 54 / 3 / 0 | `GetCMs`, `GetCMsSpent`, `GetCMPct` are hardcoded 0 (`play.rs:315-317`) |
| entity | 22 / 0 / 0 | disposition only for the current target; `GetTargetId` returns the full u32, Forge's a byte |
| entity_list | 5 / 0 / 0 | none; `GetAllEntities` has 24 slots like Forge (records past 24 are extra) |
| group | 11 / 0 / 0 | `IsSelfLeader` never goes false except on leaving; 8-member cap; member position units unverified |
| effects | 1 / 5 / 0 | snapshot is `EffectsSnapshot::default()` (`play.rs:733`), so `GetCount()` is always 0 in game |
| ability | 20 / 0 / 1 | no `Ability.new`; ability objects are frozen copies, not live records |
| ability_list | 15 / 4 / 0 | hook lifecycle (`InstallHook`, `UninstallHook`, `IsHookInstalled`, `GetHookInfo`) are no-op successes |
| ability_bar | 14 / 1 / 0 | `GetSlotUITexId` is a hardcoded 5-entry table, slot 4 -> 0 |
| combat | 10 / 7 / 0 | packet colour trailer is dropped (`combat.rs:44`); `ReadForeignCave` nil; hook info faked |
| chat | 9 / 8 / 0 | channel-type capture, focus dump, channel table are stubs; adapter keeps the last 8 lines only |
| cave | 0 / 0 / 7 | whole module is a MIPS code-cave allocator: raw-memory only, no native counterpart |
| util | 9 / 0 / 37 | `Log`/`LogDebug`, `GetWorldId`, the dialog readers, `ListFiles`, all raw-memory helpers; `GetCompassDegrees` returns the body's heading, not the camera's |
| icon | 6 / 1 / 0 | works only after the addon claims textures (`textures.rs:313`); previews are a stub before that |

---

## player

Forge file: `player.lua` (452 lines, all static PS2 offsets off `0x1FBBA0C`).

### Surface

- Identity and progression: `GetName`, `GetLevel`, `GetExp`, `GetExpDebt`.
- Totals: `GetTotalStr`, `GetTotalSta`, `GetTotalAgi`, `GetTotalDex`, `GetTotalWis`, `GetTotalInt`, `GetTotalCha`.
- Bases: `GetBaseStr` … `GetBaseCha` (same seven).
- Vitals: `GetCurrentHp`, `GetMaxHp`, `GetBaseHp`, `GetCurrentPwr`, `GetMaxPwr`, `GetBasePwr`, `GetAc`.
- Resists: `GetBaseResist` (computed from wisdom), `GetPoisonResistBuff`, `GetDiseaseResistBuff`,
  `GetFireResistBuff`, `GetColdResistBuff`, `GetLightningResistBuff`, `GetArcaneResistBuff`.
- Class Mastery: `GetCMs`, `GetCMsSpent`, `GetCMPct`.
- Movement: `GetCoordinates`, `GetVelocity`, `GetSpeed`, `GetHorizontalSpeed`, `GetHeadingRadians`,
  `GetHeadingDegrees`.
- State: `GetEntityId`, `GetTargetEntityId`, `IsAutoAttacking`, `IsDead`, `GetPetEntityId`, `HasPet`.
- Class and race: `GetClassId`, `GetRaceId`, `GetClassName`, `GetRaceName`, plus the `classes`,
  `races`, `max_class_id`, `max_race_id` fields.
- Place: `GetLocation` ({place, compass, name, cell_x, cell_z}), `GetLocationName`.

53 functions + 4 fields = 57 public entries.

### Where the data is

| Forge | Native source | Adapter today | Event source | Status |
|---|---|---|---|---|
| `GetName` | `self.player.name` (`net::session::Player`, `crates/net/src/session.rs:359-373`), from the world entry dump | `player.rs:195-206` | `Event::WorldEntry` (`session.rs:459`) -> `play.rs:2804` | REAL |
| `GetLevel`, `GetExp` | `self.dump.player.level/exp` (`crates/net/src/dump.rs:20-21`, read `dump.rs:319-337`); snapshot `play.rs:277-278` | `player.rs:211-237` | 0x0020 `Event::LevelExp` (`session.rs:1554-1557`) -> `play.rs:2361` -> `level_exp` (`play.rs:7096`) | REAL |
| `GetExpDebt` | `self.dump.player.p2c` (`dump.rs:22`); snapshot `play.rs:279` | `player.rs:229-237` | 0x00D9 `Event::PlayerP2C` (`session.rs:1564`) -> `play.rs:2705` | REAL |
| totals / bases / `Ac` | `self.stats.stat[]` / `.base[]` (`Stats`, `dump.rs:181-197`); snapshot `play.rs:294-308` | `player.rs:320-424` | state channel 0x42 (`play.rs:2706-2712`) | REAL |
| resist buffs | `self.stats.stat[22..=27]` (`play.rs:309-314`) | `player.rs:439-477` | channel 0x42 | REAL |
| `GetBaseResist` | computed `40 + total_wis/7` | `player.rs:429-435` | n/a (derived) | REAL |
| `GetCMs`, `GetCMsSpent`, `GetCMPct` | `Mastery` (`dump.rs:267-277`) fed by 0x1402 (`play.rs:2498` -> `cm_msgs`, consumed at `play.rs:5927`); the window reads `m.x0c` available and `m.x10` total (`classmastery.rs:874`) and `m.x08` as the percent (`classmastery.rs:762`) | `play.rs:315-317` sets all three to 0; `player.rs:480-501` returns them | 0x1402 -> `play.rs:2498` | PLACEHOLDER (zero; the mapping `CMs/CMSpent/CMPct` -> `x0c/x10/x08` is a strong inference, **not verified**) |
| `GetCoordinates` | walker physics position, `w.cam.walk.phys.position` (`play.rs:141-149`) | `player.rs:504-517` | game tick (`walker`) | REAL |
| `GetVelocity`, `GetSpeed`, `GetHorizontalSpeed` | same, `phys.velocity` (`play.rs:143`) | `player.rs:518-553` | game tick | REAL |
| `GetHeadingRadians`, `GetHeadingDegrees` | `phys.angles.x` (`play.rs:144`) | `player.rs:554-575` | game tick | REAL |
| `GetEntityId` | `self.self_id32()` (`play.rs:321`, `play.rs:2256`) | `player.rs:578-587` | `Event::WorldEntry` | REAL |
| `GetTargetEntityId` | `self.target` (`play.rs:156-160`) | `player.rs:588-601` | target change (`play.rs:3591-3600`) | REAL |
| `IsAutoAttacking` | `self.auto_attack` (`play.rs:324`) | `player.rs:602-611` | attack button (`play.rs:4582-4590`), target change clears (`play.rs:3590`) | REAL |
| `IsDead` | `self.death.is_some()` (`play.rs:325`), set in `msg_005e` (`play.rs:4795`, `4827`) | `player.rs:612-621` | 0x005E -> `play.rs:2349` | REAL |
| `GetPetEntityId`, `HasPet` | `self.pet_id` (`play.rs:161-165`) | `player.rs:622-648` | 0x007E / 0x007F `Event::Pet` (`session.rs:1565`, `1615`) -> `play.rs:2588` | REAL |
| `GetClassId`, `GetRaceId` | `self.dump.player.class/race` (`dump.rs:18-19`), snapshot `play.rs:171-172` | `player.rs:650-697` | `Event::WorldEntry` | REAL |
| `GetClassName`, `GetRaceName`, `classes`, `races`, `max_class_id`, `max_race_id` | static tables (`player.rs:155-184`) | `player.rs:671-714` | n/a | REAL |
| `GetLocation`, `GetLocationName` | scene grid + per-world zone table + words: `play.rs:174-272`, `crate::zones::zone` (`crates/eqoa/src/zones.rs:54`), world id `play.rs:5423` | `player.rs:716-760` | game tick (`walker.scene.grid`, `words`) | REAL |

### Actions

None. `player.lua` is read-only. The target/attack calls a mod would want are in `entity` and in the
native input path (`cycle_target` `play.rs:3518`, `attack` `play.rs:4582-4590`), covered below.

### Notes

- `GetHeadingRadians` wraps to `[0, 2pi)` where Forge returns the raw value; `GetLocation` returns
  nil when the walker or grid is missing (`play.rs:174`), which is also Forge's "out of game" case.
- The three CM values are the only zeros on a live player; the audit's "Class Mastery fields are zero"
  is confirmed at `play.rs:315-317`, and the native values exist (`dump.rs:267-277`), so this is a
  round-4 fill, not a client limitation.

---

## entity

Forge file: `entity.lua` (298 lines; every getter takes a "record" = byte offset in EE RAM).

### Surface

- Constants: `Disposition` {HOSTILE/NEUTRAL/FRIENDLY_MIN/MAX}, `StatusFlag` {NAMEPLATE_HIDDEN,
  POISONED, DISEASED}, `StatusIconTexId` {POISONED = 0x76, DISEASED = 0x77}.
- Free functions: `GetDispositionName`, `GetId`, `GetHealthPercent`, `GetPosition`, `GetName`,
  `GetLevel`, `GetDistanceTo`, `GetClassByte`, `IsPlayerCharacter`, `IsNpc`, `IsPopulated`,
  `IsUnnamed`, `GetStatusFlags`, `HasStatusFlag`, `IsPoisoned`, `IsDiseased`, `GetTargetId`,
  `GetDisposition`.
- Objects: `Entity.new(record)`, then the same 18 as methods plus `IsValid`, and live properties
  `id`, `percent_hp`, `name`, `level`, `target_id`, `class_byte`, `is_player`, `is_npc`,
  `is_populated`, `is_unnamed`, `disposition`, `status_flags`, `is_poisoned`, `is_diseased`, `x`,
  `y`, `z`, `disposition_name`.

22 public entries (3 tables + 19 callables).

### Where the data is

The native record is `net::entity::Entity` (`crates/net/src/entity.rs:112-147`) with its `Creature`
part (`entity.rs:69-110`), read from state channels 0x00-0x3F/0x6C-0xF7 in
`Entities::channel` (`entity.rs:333-377`) from `play.rs:2716-2757`. The snapshot copies 24 records plus
the target/pet/group extras (`play.rs:368-465`).

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `GetId` | `Entity::id` (`entity.rs:114`) | `entity.rs:150-163` | in world | channel (`play.rs:2716`) | REAL |
| `GetHealthPercent` | `Entity::p19` (`entity.rs:123`, read `entity.rs:200`), 0-255 | `entity.rs:164-177` | in world; 0 also means "target out of range" (`entity.rs:121-123`) | channel | REAL |
| `GetPosition` | `Entity::pos` (`entity.rs:116`) | `entity.rs:178-192` | in world | channel | REAL |
| `GetName` | `Creature::name` (`entity.rs:92`, read `entity.rs:257-258`) | `entity.rs:193-211` | in world | channel | REAL |
| `GetLevel` | `Creature::level` (`entity.rs:93`, read `entity.rs:259`) | `entity.rs:212-225` | in world | channel | REAL |
| `GetDistanceTo` | computed from `pos` | `entity.rs:226-242` | in world | n/a | REAL |
| `GetClassByte` | `Creature::p256` (`entity.rs:98`, read `entity.rs:261`) | `entity.rs:243-257` | in world | channel | REAL |
| `IsPlayerCharacter` / `IsNpc` | `p256 == 2` / `< 2` (`entity.rs:125-135`) | `entity.rs:258-277` | in world | channel | REAL |
| `IsPopulated` | `id != 0 && level > 0` (`entity.rs:145-147`) | `entity.rs:278-292` | in world | channel | REAL |
| `IsUnnamed` | empty name (`entity.rs:153-155`) | `entity.rs:293-302` (uses `name_len == 0`) | in world | channel | REAL |
| `GetStatusFlags`, `HasStatusFlag`, `IsPoisoned`, `IsDiseased` | `Creature::flags262` (`entity.rs:107`, read `entity.rs:270`); bits 0x4 / 0x8 | `entity.rs:303-357` | in world | channel | REAL |
| `StatusIconTexId` | constants only; no native HUD reference (`NOT FOUND`, searched `0x76`/`0x77` under `crates/eqoa/src` — only the adapter) | `entity.rs:131-134` | n/a | n/a | REAL (same constants Forge hardcodes) |
| `GetTargetId` | `Creature::target` u32 (`entity.rs:78`, read `entity.rs:220`) | `entity.rs:358-371` | in world | channel 0x0775? no — the record's own field | REAL, but Forge reads a byte (`entity.lua:206`) so values above 255 differ |
| `GetDisposition`, `disposition`, `disposition_name` | `self.target_info` set from 0x0010 (`play.rs:2329-2334`), copied only for the current target (`play.rs:388-390`, `419-421`) | `entity.rs:372-391`, `entity.rs:697-710` | **only after a 0x0010 target-info reply, and only for the current target** | 0x0010 `Event::TargetInfo` (`session.rs:1616-1624`) -> `play.rs:2329`; cleared on target change `play.rs:3596` | REAL (same restriction as Forge) |
| `Entity.new(record)`, methods, `IsValid`, properties | snapshot index | `entity.rs:91-108`, `392-655`, `658-727` | in world | channel | REAL (record is a snapshot slot 0-31, not an EE offset; see Notes) |

### Actions

None in the module.

For round 4's target action the native call is `Play::cycle_target` (`play.rs:3518-3601`, which sends
0x0010 at `play.rs:3597-3599`) and `Play::target_key` (`play.rs:3617`); auto-attack is
`Play::attack`'s body at `play.rs:4582-4590` (sends 0x000F). Neither is reachable from the addon host
today (no action API).

### Notes

- The `record` an entity object carries is `EntitySnapshot` index (`entity.rs:95-98`), 0..31, while
  Forge's is an EE byte offset. Mods that go through `entity_list` see no difference; a mod that
  stores a numeric record across a zone change is comparing indices, and slot reuse is caught by
  `IsValid` (`entity.rs:646-654`) the same way.
- `GetDispositionName(nil)` returns "Unknown" in both (`entity.rs:82-89`).
- Extra records 24..31 (`entity.rs:20`, `play.rs:440-458`) hold the target, pet, group and
  target-of-target when a server keeps more than the game's 24 slots; `entity_list` reaches them, and
  they are beyond Forge's 0..23.

---

## entity_list

Forge file: `entity_list.lua` (86 lines; `entity_list_offset = 0x1FB6C30`, slots 0-23).

### Surface

`GetEntityByIndex(index)` (errors outside 0..23), `GetEntityById(id)`, `GetAllEntitiesWithName(name)`,
`GetFirstEntityWithName(name)`, `GetAllEntities()` (all 24, empty slots included). 5 public entries.

### Where the data is

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `GetEntityByIndex` | `self.entities.slots` (`net::entity::Entities`, `entity.rs:298-314`), snapshot `play.rs:405-409` | `entity.rs:738-756` (errors 0..23) | in world | entity channels (`play.rs:2716`) | REAL |
| `GetEntityById` | same, scan | `entity.rs:758-777` (searches the 24 plus any populated extra, `entity.rs:78-80`) | in world | channel | REAL |
| `GetAllEntitiesWithName` | same | `entity.rs:779-813` (nil when none, as Forge) | in world | channel | REAL |
| `GetFirstEntityWithName` | same | `entity.rs:815-843` | in world | channel | REAL |
| `GetAllEntities` | same | `entity.rs:844-870` | in world | channel | REAL |

### Actions

None.

### Notes

- Slot 0 is guaranteed to exist: the snapshot synthesizes it from player state when the server's
  slot 0 is not the player (`play.rs:410-435`), matching Forge's "slot 0 is always the player".
- `GetEntityByIndex` re-enters Lua (`require('frontiers_forge.entity').new`,
  `entity.rs:749-753`) per call, so a per-frame loop over 24 slots is 24 `require` calls — worth
  flattening in round 2, not a correctness issue.

---

## group

Forge file: `group.lua` (144 lines; singleton pointer `0x4E37F0`, member stride `0x34`).

### Surface

Group: `IsInGroup`, `IsSelfLeader`, `GetMemberCount`, `GetMemberByIndex`, `Members()`.
Member object (`GroupMember`): `GetEntityId`, `GetName`, `GetHealthPercent255`, `GetHealthPercent`,
`IsActive`, `GetCoordinates`. 11 public entries.

### Where the data is

`crate::group::Group` (`crates/eqoa/src/group.rs:29-44`): `in_group`, `leader`, `members`, `invites`,
`raid`, plus `count()` (`group.rs:48-50`).

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `IsInGroup` | `group.in_group` (`group.rs:31`) | `group.rs:70-80` | in world and in a group | 0x0772 roster (`session.rs:1586-1593` -> `play.rs:2362`) and out-of-group handlers (`play.rs:2386-2402`) | REAL |
| `IsSelfLeader` | `group.leader` (`group.rs:33-34`) | `group.rs:82-92` | in group | roster (`group.rs:93-95`) | REAL, with a caveat: `roster` only ever sets it true (`group.rs:93-95`); it is cleared only by `clear()` (`group.rs:53-59`), so leadership moving to another member without a leave can leave it stale |
| `GetMemberCount` | `count()` (`group.rs:48-50`) | `group.rs:94-108` | in group | roster / leave | REAL (includes the local player, as Forge) |
| `GetMemberByIndex`, `Members` | `group.members[i]` | `group.rs:220-260` | in group | roster | REAL |
| member `GetEntityId`, `GetName`, `GetHealthPercent255`, `GetHealthPercent`, `IsActive` | `Member` (`group.rs:11-19`) | `group.rs:114-194` | health/active only after channel 0x40, so a member's bar can be nil until the server sends one | channel 0x40 (`session.rs` doesn't; `play.rs:2414` -> `group.rs:113-126`) | REAL |
| member `GetCoordinates` | the 0x40 record's three s16 coordinates (`group.rs:121`) | `group.rs:195-216` | after channel 0x40 | `play.rs:2414` | REAL-ish: Forge read cached x/y/z floats at member+0x10 (`group.lua:16-18,83-85`); the scale of the s16 fields against those floats is **NOT VERIFIED** (no conversion in `group.rs`) |

### Actions

None (the module is read-only). Invite/accept/leave live in `play.rs:2367-2402` and
`group_changed` (`play.rs:5377`).

### Notes

- Roster size: the packet is read with `n.min(4)` (`session.rs:1590`, "m_group has four members"); the
  snapshot carries at most 8 (`group.rs:17`). A raid has its own list (`group.raid`, `group.rs:39-44`,
  `Event::RaidRoster` `session.rs:441`) which Forge has no API for — a candidate `eqoa` extension.
- 0x069E group chat is a separate event (`play.rs:2775`), not part of this module; see chat.

---

## effects

Forge file: `effects.lua` (76 lines; singleton `0x1FAF730`, count at `+0x2C170`, 8 entries).

### Surface

`max_effects` (= 8), `GetCount()`, `GetIconRef(index)`, `GetName(index)`, `GetEffect(index)`
({index, icon_ref, name}), `All()` iterator. 6 public entries.

### Where the data is

The native store is `net::dump::Effects` (`crates/net/src/dump.rs:201-206`: `count`, `icon: [u32; 8]`,
`name: [Vec<u16>; 8]`), filled by `read_effects` (`dump.rs:213-240`, which mirrors
`VIClient::Channel43` and clamps the count to 8, `dump.rs:218`).

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `max_effects` | clamp of 8 (`dump.rs:218`, `effects.lua:10`) | `effects.rs:43` | n/a | n/a | REAL |
| `GetCount` | `self.effects.count` (`dump.rs:203`) | `effects.rs:46-56` over `snap.effects`, which `play.rs:733` sets to `EffectsSnapshot::default()` | in world, but always empty | state channel 0x43 (`play.rs:2713`) | PLACEHOLDER: 0 always, even with buffs up |
| `GetIconRef` | `self.effects.icon[i]` (`dump.rs:204`) | `effects.rs:58-73` | as above | channel 0x43 | PLACEHOLDER (nil) |
| `GetName` | `self.effects.name[i]` UTF-16 (`dump.rs:205`) | `effects.rs:75-90` | as above | channel 0x43 | PLACEHOLDER (nil) |
| `GetEffect` | both of the above | `effects.rs:92-112` | as above | channel 0x43 | PLACEHOLDER (nil) |
| `All` | iteration over the count | `effects.rs:114-140` | as above | channel 0x43 | PLACEHOLDER (empty) |

The native client does use the data: the HUD's effect bar is drawn from `self.effects` at
`play.rs:8196` (`fork_hud::draw_active_effects`), and the start menu's status page lists the names at
`play.rs:5439` (`startmenu.rs:502-503`). So this is a wiring gap only: `forge_snapshot` never copies
`self.effects` into `EffectsSnapshot`, although `fork_ext/play.rs:93-98` already hands `self.effects`
to the icon prefetcher.

### Actions

None.

### Notes

- The module's own tests pass because they inject `EffectsSnapshot` by hand
  (`effects.rs:191-243`, `wereoxxs_test.rs:58`), which is exactly the kind of fake the plan forbids
  for round 5. Compare the audit line "effects from real data" in `docs/plans/EQOA-MOD-API-PLAN.md:116`.
- `EffectsSnapshot::name` is 32 bytes (`effects.rs:12`) where the game's field is up to 31 wide
  characters (`dump.rs:208-209`), so the copy will need truncation, not truncation-by-bytes.

---

## ability

Forge file: `ability.lua` (181 lines; `Ability` record stride `0x1D8`, index 0 a sentinel).

### Surface

Module: `Scope` {SELF, TARGET, GROUP, PET, CORPSE, UNKNOWN}, `size` (0x1D8), `Ability.new(address)`, and
18 instance methods: `IsValid`, `GetId`, `GetDisplayIndex`, `GetSpellbookSlot`, `GetLevel`,
`GetRange`, `GetCastTime`, `GetPwrCost`, `GetIconBackgroundRef`, `GetIconForegroundRef`, `GetScope`,
`GetCooldown`, `GetEquipRequirements`, `IsOnCooldown`, `GetCooldownLockoutMs`, `IsInRange(a, b)`,
`GetName`, `GetDescription`. 21 public entries.

### Where the data is

Native record: `net::dump::Ability` (`crates/net/src/dump.rs:150-171`), read by `read_ability`
(`dump.rs:451`) from 0x00B3 (`session.rs:1639-1643`), stored by id in `self.abilities`
(`play.rs:2597`).

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `Scope`, `size` | enum values, record stride | `ability.rs:379-387` | n/a | n/a | REAL |
| `Ability.new` | — | not defined | — | — | MISSING: `require("frontiers_forge.ability").new` is nil; objects come from `ability_list`/`ability_bar` |
| the 18 methods | `Ability` fields (`dump.rs:152-171`) | `create_ability_object` (`ability.rs:197-343`); `IsInRange` at `:273-320` | only on objects produced by ability_list / ability_bar | 0x00B3, 0x00CD | REAL — but each object is a **copy frozen at creation** (`ability.rs:200-341` capture values by value), so `IsOnCooldown` does not update; Forge's object re-reads memory every call |
| `IsInRange` | scope + range (`dump.rs:160`, `165`) | `ability.rs:273-320` (tolerates the method-call extra argument) | objects | n/a | REAL |
| icons | `icon_back` / `icon_front` (`dump.rs:163-164`) | `ability.rs:235-245` | in world | 0x00B3 | REAL |

### Actions

None. The use-an-ability action is `Play::hud_key` -> `send_0032` (`play.rs:7053`, `6785-6794`,
sends 0x0032 with the slot) and `Play::ability_slot` (`play.rs:6764`) for placing one. No mod path
today; that is the round-4 actions API.

### Notes

- `GetCooldown` is the recast in seconds, `GetCooldownLockoutMs` the live lockout, both straight from
  `dump.rs:157`/`dump.rs:166`. Forge's doc comment (that lockout is a constant, not a countdown) holds
  here too: the native field is only written by 0x00CD (`play.rs:2684-2691`).
- A Forge mod that held an `Ability` object across a cooldown will now see a stale one; document
  "re-fetch by id/display index" in the spec.

---

## ability_list

Forge file: `ability_list.lua` (791 lines; records in the client singleton at `[0x4E37F0]+0x2BB30`,
count at `+0x2BB28`, tree root `+0x2BB34`).

### Surface

Queries: `GetCount`, `GetAbilityByIndex`, `Abilities()` (in-order tree = id-sorted), `GetAbilityById`,
`GetAbilityByDisplayIndex`.
Hook lifecycle: `IsHookInstalled`, `InstallHook`, `UninstallHook`, `GetHookInfo`, `GetDroppedCount`,
`Acquire(owner)`, `Release(owner)`, `GetOwnerCount`, `HasClaim(owner)`.
Events: `Events` {OnLockoutStart, OnLockoutEnd}, `On(owner, event, fn)`, `Off`, `GetSubscriberErrors`,
`Update()`. 19 public entries.

### Where the data is

`self.abilities: BTreeMap<i64, Ability>` keyed by ability id (`play.rs:2597` insert, `play.rs:154`
comment), snapshot built at `play.rs:467-665`.

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `GetCount` | `self.abilities.len()` (all valid records) | `ability.rs:441-451` | in world | 0x00B3 / 0x00B8 / 0x00FF | REAL |
| `GetAbilityByIndex` | snapshot array | `ability.rs:453-469` (1-based, nil when invalid) | in world | above | REAL, with a divergence: the index is the snapshot's position, not the game's raw array/tree index |
| `Abilities` | snapshot order | `ability.rs:507-533` | in world | above | REAL but **not id-sorted**: the snapshot keeps `BTreeMap` id order (`play.rs:471`), so it happens to be id order, unlike Forge's tree walk; do not promise "tree order" in the spec |
| `GetAbilityById` | map key | `ability.rs:471-487` | in world | above | REAL |
| `GetAbilityByDisplayIndex` | `Ability::display_index` (`dump.rs:154`) | `ability.rs:489-505` | in world | 0x00B6 reindexes (`play.rs:2636-2657`) | REAL |
| lockout fields | `Ability::lockout` (`dump.rs:157`) | `play.rs:508`, `ability.rs:262-271` | in world | 0x00CD (`play.rs:2684-2691`) | REAL |
| `OnLockoutStart` / `OnLockoutEnd` | `record_lockout` fed unconditionally from the 0x00CD arm (`play.rs:11-14`, `2686`) into a thread-local ring (`ability.rs:33-68`) | `Update` drains it per VM (`ability.rs:698-788`) | in world | 0x00CD | REAL (the one event stream in this part that is complete) |
| `Acquire`, `Release`, `GetOwnerCount`, `HasClaim` | claim set only | `ability.rs:586-633` | any time | n/a | REAL (bookkeeping only; the capture does not depend on it) |
| `IsHookInstalled` | capture is always live | `ability.rs:536-543`: true when in game or claimed | in world | n/a | PLACEHOLDER (reports a hook that does not exist; semantically harmless) |
| `InstallHook` | n/a | `ability.rs:544-554`: seeds the cache, returns true | any time | n/a | PLACEHOLDER (no-op success; "seed" is real work) |
| `UninstallHook` | n/a | `ability.rs:555-564`: clears seeded, returns true | any time | n/a | PLACEHOLDER (no-op success; the stream keeps flowing) |
| `GetHookInfo` | n/a | `ability.rs:565-577`: `installed = true` | any time | n/a | PLACEHOLDER (`installed` invented) |
| `GetDroppedCount` | ring overrun counter | `ability.rs:578-584` | any time | n/a | REAL (a real count, though only a lagging VM can raise it, `ability.rs:717-719`) |

### Actions

None. Casting is `send_0032` (see above).

### Notes

- The engine's own 0xCD handler updates the record *and* calls the fork hook in the same arm
  (`play.rs:2684-2691`), so the lockout edge is never lost to a frame boundary. That is a genuine
  improvement over Forge's code cave and the model `eqoa.cooldowns` should copy.
- `IsHookSuspended` is a native extra (not in Forge): `ability.rs:635-643`.

---

## ability_bar

Forge file: `ability_bar.lua` (273 lines; 3 bars, 5 slots per bar, slot stride `0x20`).

### Surface

Bar: `num_bars` (3), `slot_size` (0x20), `GetSelectedBarIndex`, `GetSelectedSlotIndex(bar)`,
`GetSlotUITexId(slot)`, `GetSlotCount(bar)`, `GetAbilitySlot(bar, slot)`, `GetAbility(bar, slot)`,
`GetItem(bar, slot)`.
Slot object: `IsEmpty`, `GetAbilityIndex`, `GetAbility`, `GetItemIndex`, `GetItem`, `GetIconRef`.
15 public entries.

### Where the data is

Native: `crate::hudmenu` — `Entry` (`crates/eqoa/src/hudmenu.rs:32-46`: `msg` source tag, `id`),
`MenuDef` (`hudmenu.rs:48-53`: `count`, `entries`), `HudMenus` (`hudmenu.rs:286-287`: `sel`, `menus`,
`defs`). The snapshot is built at `play.rs:557-606`.

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `num_bars`, `slot_size` | 3 bars, 5 slots (`play.rs:561-569`) | `ability.rs:798-799` | n/a | n/a | REAL |
| `GetSelectedBarIndex` | `self.hud_menus.sel` (`hudmenu.rs:287`), snapshot clamps to 0..2 (`play.rs:559`) | `ability.rs:803-817` (nil out of game) | in world | HUD key path (`play.rs:7062`) | REAL for bars 1-3; when the carousel is on the items or the weapon sets the value is clamped, so it can name a bar that is not the one up |
| `GetSelectedSlotIndex` | `self.hud_menus.menus[b].sel` (`hudmenu.rs:126`), `play.rs:564` | `ability.rs:819-839` | in world | key path | REAL |
| `GetSlotCount` | `defs[b].count` (`hudmenu.rs:51`), `play.rs:563` | `ability.rs:841-859` | in world | `hudmenu::build` (`hudmenu.rs:314`) | REAL |
| `GetSlotUITexId` | the disc's `sSlotIcons` table (`hudmenu.rs:57-58`, `[5,5,3,2]`) | `ability.rs:861-874` hardcodes `[5,5,3,2,0]` | in world | n/a | PLACEHOLDER for slot 4 (0) and a duplicated constant; slots 0-3 match (`hudmenu.rs:57`) |
| `GetAbilitySlot`, `GetAbility` | entry source tag 0x8060000 + raw index (`play.rs:570-581`) | `ability.rs:961-1017` | in world | `hudmenu::build` | REAL |
| `GetItem`, slot `GetItem` | tag 0x2030000 + item record index (`play.rs:582-590`) over `dump.inventory.items` (`play.rs:519-555`) | `ability.rs:1019-1045`, `929-950` | in world, item slots only | inventory events (`play.rs:2415-2452`) | REAL |
| `IsEmpty`, `GetAbilityIndex`, `GetItemIndex`, `GetIconRef` | `Entry::msg`/`id` and the slot's icon | `ability.rs:877-958` | in world | `hudmenu::build` | REAL |

### Actions

None in the module, but the slot is the handle a mod would use: the native use path is
`hud_key` -> `send_0032(id)` for an ability and `use_item(id, 1)` for an item (`play.rs:7065-7067`,
`5485-5490`). Both are one call away from the snapshot data and should be the round-4 action surface.

### Notes

- `ItemSnapshot::slot_id` is filled as `k + 1` and `populated` is true for every inventory entry
  (`play.rs:546-554`), so `GetSlotId()` is an index, not the item's own slot field; a mod comparing it
  to a Forge slot id will be off. The Forge slot object only exposes `GetName`, `GetDescription`,
  `GetIconRef`, so this is invisible to existing mods.
- Bars 3 and 4 (menus 4 and 5, `hudmenu.rs:69-77`) exist natively but not in this module's 3.

---

## combat

Forge file: `combat.lua` (760 lines: a code cave on the 0x00DB dispatcher case, ring of 64 events).

### Surface

`IsHookInstalled`, `InstallHook`, `UninstallHook`, `GetHookInfo`, `ReadForeignCave(max_words)`,
`GetEventCount`, `GetDroppedCount`, `Acquire(owner)`, `Release(owner)`, `GetOwnerCount`,
`HasClaim(owner)`, `IsHookSuspended`, `Events` {OnDamageDealt, OnDamageReceived, OnHealingDealt,
OnHealingReceived}, `On`, `Off`, `GetSubscriberErrors`, `Update()`. 17 public entries.

### Where the data is

The packet is 0x00DB (`Event::Opcode`, `net/src/session.rs:1690`), handled by
`Play::damage_indication` (`play.rs:5821-5854`), which calls `fork_combat_event`
(`crates/eqoa/src/fork_ext/play.rs:889-902`) -> `record_combat_event`
(`crates/eqoa/src/fork_ext/addons/combat.rs:28-47`), drained by `snapshot_combat_events`
(`combat.rs:49-62`) into the frame snapshot (`play.rs:673-677`).

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `Events`, `On`, `Off`, `GetSubscriberErrors`, `Update` | ring above | `combat.rs:104-110`, `219-383` | in world | 0x00DB (`play.rs:2780-2781`) | REAL |
| event payload `attacker_id`, `defender_id`, `amount`, `seq` | `damage_indication` parses all three (`play.rs:5823`), `fork_combat_event` passes them (`fork_ext/play.rs:891`) | `combat.rs:295-325` | in world | 0x00DB | REAL |
| `color` (the patch trailer) | parsed at `play.rs:5829-5830` but **not passed on** at `play.rs:5843`; `record_combat_event` hardcodes `color: 0` (`combat.rs:44`) | `combat.rs:302`, set only when non-zero (`:317-319`) | — | 0x00DB | PLACEHOLDER (always nil) |
| `outgoing` / `incoming` / `is_heal` / `to_pet` | player and pet ids (`combat.rs:289-290`), sign of `amount` (`play.rs:5840`, `damage.rs:8-9`: positive is a heal) | `combat.rs:304-310` | in world | 0x00DB | REAL |
| `from_pet` | Forge's heuristic is the colour trailer (`combat.lua:456-460`) | `combat.rs:307-308`: pet id, or player id with a colour — the second half can never fire | in world | 0x00DB | PLACEHOLDER (half the test is dead) |
| `GetEventCount` | monotonic seq (`combat.rs:25`, `34-38`) | `combat.rs:202-211` | any time | n/a | REAL (counts since process start, not since install) |
| `GetDroppedCount` | — | `combat.rs:213` always 0 | — | — | PLACEHOLDER (ring silently drops at 64, `combat.rs:31-33`) |
| `Acquire` / `Release` / `GetOwnerCount` / `HasClaim` | claim set | `combat.rs:121-158` | any time | n/a | REAL (bookkeeping) |
| `IsHookInstalled` | capture is unconditional | `combat.rs:160-167`: true when in game or claimed | in world | n/a | PLACEHOLDER (reports a hook that does not exist) |
| `InstallHook` / `UninstallHook` | n/a | `combat.rs:169-170`: true | any time | n/a | PLACEHOLDER (no-op success) |
| `IsHookSuspended` | n/a | `combat.rs:172-179` | any time | n/a | REAL bookkeeping over a fake "hook" |
| `GetHookInfo` | n/a | `combat.rs:181-200`: `installed = true`, `dropped = 0` invented | any time | n/a | PLACEHOLDER |
| `ReadForeignCave` | n/a (PCSX2 patch inspection) | `combat.rs:214-217`: nil | — | n/a | PLACEHOLDER (audit line `EQOA-MOD-API-PLAN.md:49`) |

### Actions

None.

### Notes

- The native handler is better than Forge's cave in one way and worse in another: it sees every 0x00DB
  without patching (`play.rs:2780`), but it throws away the S2 colour trailer that the game itself
  keeps for the damage numbers (`play.rs:5829-5830`, `damage.rs:36-39`). Passing it into
  `fork_combat_event` is a two-line fix and restores `from_pet`.
- Forge's doc says the hook also sees heals; the native path does too (`amount > 0` is the green
  number, `damage.rs:8-9`). A self-heal is attacker == defender == player, which fires both Dealt and
  Received in both implementations (`combat.rs:326-341`, `combat.lua:736-744`).

---

## chat

Forge file: `chat.lua` (798 lines: a 32-slot drawn-line ring, publish/subscribe, a send cave,
channel-type discovery).

### Surface

Receive: `MsgType` {Say, Shout, Party, Tell, Guild}, `GetMessageTypeString`, `Events`
{OnChatMessage}, `On`, `Off`, `GetSubscriberErrors`, `Update()`.
Send: `ChatMode` {Default, Say, Group, Guild, Shout}, `SendChatText(text, mode)`, `IsSendPending`,
`IsSendHookInstalled`, `InstallSendHook`, `UninstallSendHook`, `IsSendHookSuspended`.
PCSX2-only inspection: `CaptureChannelTypeFromGameWindow`, `DebugDumpFocusChain`, `GetChannelTableInfo`.
17 public entries.

### Where the data is

Incoming: the server's chat opcodes in `net/src/session.rs:1626-1638` (0x0A7A white, 0x0A7B with the
server's RGB, 0x069E group, 0x0758 narrow, 0x00CE error red) -> `Play::server_line`/`group_line`
(`play.rs:2773-2775`) -> `add_chat` (`play.rs:4779-4788`, a `VecDeque` capped at 8) plus
`chat_pending` for the window. The client's real line store is `chat::ChatText`
(`crates/eqoa/src/chat.rs:68-140`: 1,001 lines, `ChatLine { text, color, kind, time, seq }` at
`chat.rs:26-36`), fed at draw time (`play.rs:8206-8209`) into `WndChat` (`chat.rs:316-359`).
Outgoing: `Play::chat_line` (`play.rs:4602-4690`: slash commands, `/tell`, `/reply`, channel sends),
reached from the edit box's `edit_done` (`play.rs:6385`, chat purpose at `:6402-6403`) and from the
quick-chat menus (`play.rs:6912-6915`).

| Forge | Native source | Adapter today | Availability | Event source | Status |
|---|---|---|---|---|---|
| `MsgType`, `GetMessageTypeString` | the red byte of the line's colour: `[f32;4]` from `session.rs:1626-1632`, and the constants are exactly 128/255, 63/255, 0.5, 0.25, 1.0 | `chat.rs:23-27`, `142-151`, `176-179` | in world | 0x0A7A/0x0A7B/0x0758/0x00CE/0x069E | REAL (same red-channel-only identity Forge has) |
| `Events`, `On`, `Off`, `GetSubscriberErrors`, `Update` | `snapshot_chat` (`chat.rs:43-104`) diffs `self.chat` by prefix and assigns a monotonic `seq` | `chat.rs:181-319` | in world | per frame (`play.rs:668`) | REAL |
| message payload `text`, `msg_type`, `type_name`, `committed_ms` | native line text/colour/`ChatLine::time` | `chat.rs:278-283` (`start_index` is the snapshot index, `committed_ms` is `session.now()`, not the line's commit time) | in world | above | REAL with caveats: the diff sees only the **last 8 lines** (`play.rs:4784-4787`), so a burst of more than 8 lines between two addon frames loses the earliest; `committed_ms` is set at snapshot time (`chat.rs:94`) |
| `ChatMode` | channel word: `Play::channel` (`play.rs:4613`), set by `group_changed` (`play.rs:5377-5383`) and the slash commands (`play.rs:4639-4657`) | `chat.rs:166-173` (strings) | in world | 0x000E sends | REAL (constants) |
| `SendChatText` | `Play::chat_line` | `chat.rs:321-349` queues -> `fork_ext/play.rs:812-814` -> `chat_line` | in world only (`chat.rs:327-329`) | per frame | REAL, including slash commands, because it goes through the game's own line handler |
| `IsSendPending` | nothing is queued beyond the frame | `chat.rs:362`: false | — | — | PLACEHOLDER |
| `IsSendHookInstalled`, `InstallSendHook`, `UninstallSendHook`, `IsSendHookSuspended` | the native client needs no hook to send | `chat.rs:351-361` | — | — | PLACEHOLDER (all return a fixed success/false) |
| `CaptureChannelTypeFromGameWindow` | the live byte is `Play::channel`; no window walk needed | `chat.rs:365-368`: `(0, nil, 0)` | — | n/a | PLACEHOLDER (raw-memory only in Forge, `chat.lua:665-738`) |
| `DebugDumpFocusChain`, `GetChannelTableInfo` | n/a | `chat.rs:369-376`: empty tables | — | n/a | PLACEHOLDER (raw-memory only in Forge, `chat.lua:740-796`) |

### Actions

`SendChatText` is the one real action here, and its native path is complete: the adapter hands the
line to `Play::chat_line` (`fork_ext/play.rs:813`), which is the same function the keyboard's edit box
calls (`play.rs:6402-6403`). Behaviour lives at `play.rs:4638-4689` (says, group, guild, shout, tell,
reply, `/alarm`, `/time`, `/follow`, `/bug`, GM commands); plain text goes to the current channel at
`play.rs:4613-4617`.

### Notes

- The 32-slot ring Forge read is `VIChatText`; our `ChatText` is the same object rebuilt with 1,001
  lines (`chat.rs:20-24`). The adapter bypasses it and diffs the 8-line `self.chat` queue, because the
  lines reach `ChatText` only when the HUD draws (`play.rs:8201-8209`). Round 2 should publish from
  `ChatText` (which already has a per-line `seq`, `chat.rs:26-36`) or give `add_chat` a seq of its own;
  either removes the 8-line hole.
- 0x069E arrives as its own event (`session.rs:1634-1636`, `play.rs:2775`) but is folded into the same
  "line" stream, so a mod cannot tell group chat from say by `msg_type` (Forge could not either: group
  lines are coloured like party, `play.rs:2775` -> `group_line`).
- `ChatLine.kind` (fresh for 20 s, `chat.rs:30-31`) and the game's own wrapping (`chat.rs:126-140`) are
  the closest native equivalent of Forge's "continuation slot starts with a space" test
  (`chat.lua:55-60`).

---

## cave

Forge file: `cave.lua` (246 lines: a best-fit allocator for MIPS code caves in EE RAM).

### Surface

`IsRegionZero(base, size)`, `Alloc(size, id)`, `Reserve(base, size, id)`, `Get(id)`, `Free(id)`,
`Release(id)`, `Info()`. 7 public entries.

### Where the data is

Nowhere: this module exists only to place and free executable memory in the emulator
(`cave.lua:5-40`, safe tiers at `0x000F1000-0x000F8000`). Our client runs its own code, has no code
caves and no patched call sites. **Raw-memory only.**

| Forge | Native source | Adapter today | Status |
|---|---|---|---|
| `Alloc`, `Reserve`, `Get`, `Free`, `Release`, `Info`, `IsRegionZero` | none | not registered as a module (`host.rs:339-355` installs no cave table) | MISSING (all 7), by design |

What cave was *for* is already native and needs no replacement:

- the cooldown hook: the engine's 0x00CD arm calls `fork_hooks::ability_lockout` directly
  (`play.rs:2684-2691`, `play.rs:11-14`);
- the combat capture: 0x00DB is a normal event (`play.rs:2780-2781`);
- the chat send hook: `Play::chat_line` is directly callable from Rust (`fork_ext/play.rs:813`).

### Actions

None.

### Notes

- A mod that calls `Cave.*` today gets `attempt to index a nil value` (the module is not even in
  `package.loaded`). Round 2 should either omit it from the Forge compat surface with a diagnostic, or
  ship a table whose functions all report "not supported: the native client needs no code caves" —
  the plan's no-fake-success rule (`EQOA-MOD-API-PLAN.md:23-25`).

---

## util

Forge file: `util.lua` (677 lines). Most of it is raw EE-memory plumbing and Windows file APIs.

### Surface

Memory (raw): `EEmem`, `EE_RAM_SIZE`, `IsValidEEPointer`, `ReadFromOffset`, `WriteToOffset`,
`FillRange`, `WriteWords`, `GetOffsetFromPointerChain`, `ReadFromPointerChain`.
Text: `utf16_to_utf8`, `utf8_to_utf16`, `GetLocalizedString`.
Math/game: `GetExpRequiredForLevel`, `GetDistanceBetween`, `IsBitZero`, `RadiansToDegrees`,
`GetCompassRadians`, `GetCompassDegrees`, `NormalizeDegrees`, `SignedDegreesBetween`,
`GetBearingToPoint`, `IsFacingPoint`.
State: `IsInGame`, `GetWorldId`, `IsBattleMusicPlaying`, `IsStartMenuOpen`, `GetUiMode`, `IsUiBusy`,
`UiMode`, `IsTextDialogOpen`, `GetTextDialogText`, `IsConfirmDialogOpen`, `GetConfirmDialogText`,
`GetConfirmDialogStyle`, `GetConfirmDialogSelection`, `IsInputDialogOpen`, `GetInputDialogTag`,
`GetInputDialogText`, `IsChatKeyboardOpen`, `InputDialogTag`, `ConfirmDialogStyle`.
Log: `Log(level, msg)`, `LogDebug`, `LogError`.
Files: `ListFiles(pattern, options)`, `ListFilesInDir(dir, glob, options)`.
46 public entries.

### What our host provides

The native `frontiers_forge.util` table is built in `create_base_host` (`host.rs:126-240`) and any
`util.lua` on disk is ignored (`package.rs:215-253`, test at `package.rs:695-699`). It sets exactly:
`IsInGame` (`host.rs:128`), `IsStartMenuOpen` (`:136`), `IsUiBusy` (`:141`), `GetUiMode` (`:146`),
`UiMode` (`:151-162`), `GetCompassDegrees` (`:183`), `GetBearingToPoint` (`:190`),
`GetExpRequiredForLevel` (`:203`), `LogError` (`:231`). 9 of 46.

| Forge | Native source | Adapter today | Status |
|---|---|---|---|
| `IsInGame` | `connected && hud_ready` (`play.rs:131-133`) | `host.rs:128-131` | REAL |
| `IsStartMenuOpen` | any menu mode, start menu, or map (`play.rs:680-683`) | `host.rs:133-139`: deliberately true for **any** game window, not only the pause menu | REAL, documented divergence (`host.rs:133-135`) |
| `IsUiBusy`, `GetUiMode`, `UiMode` | `play.rs:684-699`, mode table `play.rs:745-783` | `host.rs:141-162` | REAL |
| `GetCompassDegrees` | the camera's facing: `walker.eye.direction` (`play.rs:3609`) | `host.rs:183-188` returns **the body's** `player.heading_rad`, not the camera's | REAL but wrong value; Forge's `GetCompassRadians` reads the compass heading (`util.lua:558-566`) |
| `GetBearingToPoint` | pure math, same formula as `util.lua:597-601` | `host.rs:190-202` | REAL |
| `GetExpRequiredForLevel` | `hud::exp_for_level` (`crates/eqoa/src/hud.rs:124-131`) | `host.rs:203-215` uses the `< 51` branch only; the native formula changes at 51 (`hud.rs:126-130`) | REAL below 51, PLACEHOLDER above (`host.rs:207-208` says so) |
| `LogError` | `UiForge.Log` sink (`host.rs:220-230`) | `host.rs:231-240` | REAL |
| `Log`, `LogDebug` | same sink | not set; `UiForge.Log` itself works | MISSING (a mod calling `Util.Log(5, ...)` errors) |
| `GetWorldId` | `Play::world_id` (`play.rs:5423-5425`, from `self.player.world`) | not set | MISSING (plan already lists it, `EQOA-MOD-API-PLAN.md:45`) |
| `IsBattleMusicPlaying` | `Play::self_fighting` (`play.rs:8504-8507`, the player's `Creature::p263`, `net/src/entity.rs:104`) | not set | MISSING |
| `GetLocalizedString` | `self.words: Option<vi::strtab::StringTable>` (`play.rs:267`), read as `w.get(id, 0)` (`play.rs:222-226`) | not set | MISSING |
| `GetTextDialogText`, `IsTextDialogOpen` | `self.dialogs: VecDeque<Vec<u16>>` (`play.rs:311`) shown in mode 9 (`play.rs:749`) | not set | MISSING |
| `IsConfirmDialogOpen`, `GetConfirmDialogText`, `GetConfirmDialogStyle`, `GetConfirmDialogSelection`, `ConfirmDialogStyle` | `dialog::MBox` (`crates/eqoa/src/dialog.rs:241-256`: `kind` 0/1/2 exactly Forge's style enum, `text`, `sel`; mode 14) | not set | MISSING |
| `IsInputDialogOpen`, `GetInputDialogTag`, `GetInputDialogText`, `IsChatKeyboardOpen`, `InputDialogTag` | `self.edit: editbox::EditBox` (`play.rs:479`, `editbox.rs:156-168`: `text`, `title`, `purpose` = the tag, read at `play.rs:6373`, tags dispatched at `play.rs:6400-6439`) | not set | MISSING (the native client has every field) |
| `GetDistanceBetween`, `IsBitZero`, `RadiansToDegrees`, `NormalizeDegrees`, `SignedDegreesBetween`, `IsFacingPoint` | trivial math | not set | MISSING (pure helpers, cheap to add) |
| `GetCompassRadians` | as `GetCompassDegrees` | not set | MISSING |
| `EMem`, `EE_RAM_SIZE`, `IsValidEEPointer`, `ReadFromOffset`, `WriteToOffset`, `FillRange`, `WriteWords`, `GetOffsetFromPointerChain`, `ReadFromPointerChain`, `utf16_to_utf8`, `utf8_to_utf16` | none | not set | MISSING, and must stay missing: raw-memory only. Text conversion has no mod use (the host converts) |
| `ListFiles`, `ListFilesInDir` | discovery's own `read_dir` (`discovery.rs:172`, `270`); an addon gets only `io.open` rooted at its own folder (`file_io.rs:334-355`) | not set | MISSING; if wanted, add a rooted directory listing to `io`, never a raw Windows wildcard |

### Actions

None (the dialog/window readers are reads).

### Notes

- The `GetCompassDegrees` bug matters: every compass/heading mod (Wereoxxs' compass face) gets the
  body's facing, so it is wrong whenever the camera is not behind the player. The camera's own angle is
  in hand as `walker.eye.direction` (`play.rs:3609`).
- `Util.Log` is the most likely silent breakage: it is a plain Lua function in Forge
  (`util.lua:103-107`) but a missing field here, and bundled mods call it for diagnostics.
- The dialog trio (text/confirm/input) is the whole "windows/dialogs" part of round 4
  (`EQOA-MOD-API-PLAN.md:129-130`) and every field already exists natively.

---

## icon

Forge file: `icon.lua` (569 lines: decodes PS2 `VISurface` pixels out of emulated memory).

### Surface

`GetTexture(hash, options)` -> texture, w, h; `GetUITexture(tex_id)` -> texture, w, h;
`GetSurfaceInfo(hash)` -> {width, height, format, palette_count, palette_format};
`Acquire(owner)`, `Release(owner)`, `GetOwnerCount`, `HasClaim(owner)`. 7 public entries.

### Where the data is

The client's icon art is the disc's ESF surface library, keyed by the game's resource hash:

- hash of a UI texture name: `vi::ui::icon_id` (`crates/vi/src/ui.rs:67`), the native
  equivalent of Forge's `HashResourceID` (`icon.lua:390-397`);
- surface lookup: `esf::Library::surface(dict_id)` (`crates/esf/src/library.rs:487`,
  `crates/esf/src/lib.rs:495`); the client's own `crate::ui::Vui::surface`
  (`crates/eqoa/src/ui.rs:228`);
- the HUD's built-in texture atlas: `Vui::entry` (`vi/src/ui.rs:131`), from `play.hud` or
  `play.ui_data` (`fork_ext/play.rs:50-83`);
- per-frame feed: `fork_hooks::frame_library` -> `icons::update_live_data`
  (`crates/eqoa/src/fork_ext/play.rs:42-100`), invalidated by `art_changed` (`:103-106`);
- decode and crop: `LiveLibraryProvider` (`icons.rs:667-733`), `ui_texture` cropping
  (`icons.rs:683-724`), trim (`icons.rs:123`), `IconCache` (`icons.rs:255`).

| Forge | Native source | Adapter today | Availability | Status |
|---|---|---|---|---|
| `GetTexture`, `GetUITexture` | atlas + library above | stub in the base table (`icons.rs:1130-1145`: nil, one diagnostic line for `GetUITexture`); replaced with the real cache by `attach_icon_cache` (`icons.rs:1271-1344`), installed from `Textures::install_for_addon` (`textures.rs:298-313`) | in world, after the addon's textures are installed | PLACEHOLDER before, REAL after |
| `GetSurfaceInfo` | decoded `DecodedIcon` (`icons.rs:25-44`) | `icons.rs:1147-1170`, `1245-1269`: width/height are real, `format = 2`, `palette_count = 0`, `palette_format = 0` are synthesized | in world | PLACEHOLDER (the fields exist because Forge reported them, not because the client has them) |
| `Acquire`, `Release`, `GetOwnerCount`, `HasClaim` | `IconCache` owners (`icons.rs:255`, `403`), freeing on the last release | base table records claims (`icons.rs:1090-1128`); the real cache refcounts at `icons.rs:1202-1243` | any time | REAL |
| options `trim`, `trim_transparent`, `trim_color` | `TrimOptions` (`icons.rs:46-69`) + `trim_rgba` (`icons.rs:123-233`) + padding-colour detection (`:71-121`) | `icons.rs:1284-1287` | in world | REAL |

### Actions

None.

### Notes

- The base table is what an addon sees when it never installs textures (no `UiForge.*` texture use):
  `GetTexture` then returns nil forever with no log line, which is the "silent nil" the plan calls out.
  Add the diagnostic that `GetUITexture` has (`icons.rs:1140-1142`) to `GetTexture` too.
- Forge's options are decoded against live PS2 surfaces (CLUT4/CLUT8/RGBA32/RGB16, palette swizzle
  `icon.lua:78-181`). Our ESF decoder already returns RGBA8, so the palette formats `GetSurfaceInfo`
  reports cannot exist natively — return only what the library knows (a source format if `esf::Surface`
  exposes it) or omit the field.
- This is Forge's only path to game art besides `UiForge.CreateTextureFromFile`; with ESF decoding it
  is strictly better (no dictionary scan, no 64 MB of emulated RAM), so `eqoa.icons` should be the
  supported surface and the Forge shim stays a shape adapter.

---

## Coverage notes

- All 13 modules assigned to part A are covered; none was skipped.
- The 5 native event queues that feed these modules exist and are complete: chat (per frame,
  `play.rs:668`), combat (0x00DB, `play.rs:2780`), cooldown edges (0x00CD, `play.rs:2684`), target
  info (0x0010, `play.rs:2329`), entity channels (`play.rs:2716`). Everything else in this part is
  polled from `Snapshot`.
- Verified-not-guessed open items, for the spec author: (1) the `CMs/CMSpent/CMPct` mapping to
  `Mastery.x0c/x10/x08`; (2) the scale of group-member s16 coordinates against Forge's float cache;
  (3) whether `Ability.display_index`-as-server-key stays stable across a zone (it is reindexed only
  by 0x00B6, `play.rs:2636-2657`).


